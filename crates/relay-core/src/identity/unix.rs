use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};

use equalturn_protocol::SigningIdentity;
use rustix::{
    fs::{self as unix_fs, Mode, OFlags},
    io::Errno,
    process,
};
use zeroize::Zeroizing;

use crate::RelayError;

pub fn load_or_create_relay_identity(path: &Path) -> Result<SigningIdentity, RelayError> {
    let name = path.file_name().ok_or_else(|| {
        identity_error(
            io::ErrorKind::InvalidInput,
            "relay identity path has no file name",
        )
    })?;
    let parent_path = parent_path(path);
    let directory = open_or_create_private_directory(&parent_path)?;

    if let Some(identity) = read_existing_identity(&directory, name)? {
        return Ok(identity);
    }

    let identity = SigningIdentity::generate().map_err(|_| RelayError::Randomness)?;
    let seed = Zeroizing::new(identity.seed_bytes());
    match create_identity_file(&directory, name, &seed) {
        Ok(()) => Ok(identity),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            read_existing_identity(&directory, name)?.ok_or_else(|| {
                identity_error(
                    io::ErrorKind::AlreadyExists,
                    "relay identity changed during creation",
                )
            })
        }
        Err(error) => Err(RelayError::Identity(error)),
    }
}

fn parent_path(path: &Path) -> PathBuf {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn open_or_create_private_directory(path: &Path) -> Result<File, RelayError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder.create(path).map_err(RelayError::Identity)?;

    let descriptor = unix_fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|error| RelayError::Identity(io::Error::from(error)))?;
    let directory = File::from(descriptor);
    validate_private_directory(&directory)?;
    Ok(directory)
}

fn validate_private_directory(directory: &File) -> Result<(), RelayError> {
    let metadata = directory.metadata().map_err(RelayError::Identity)?;
    let mode = metadata.mode();
    if !metadata.is_dir()
        || metadata.uid() != process::geteuid().as_raw()
        || mode & 0o7077 != 0
        || mode & 0o700 != 0o700
    {
        return Err(identity_error(
            io::ErrorKind::PermissionDenied,
            "relay identity directory must be owner-controlled with mode 0700",
        ));
    }
    Ok(())
}

fn read_existing_identity(
    directory: &File,
    name: &std::ffi::OsStr,
) -> Result<Option<SigningIdentity>, RelayError> {
    let descriptor = match unix_fs::openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(Errno::NOENT) => return Ok(None),
        Err(error) => return Err(RelayError::Identity(io::Error::from(error))),
    };
    let mut file = File::from(descriptor);
    let metadata = file.metadata().map_err(RelayError::Identity)?;
    if !metadata.is_file()
        || metadata.uid() != process::geteuid().as_raw()
        || metadata.mode() & 0o7077 != 0
        || metadata.mode() & 0o400 == 0
        || metadata.len() != 32
    {
        return Err(identity_error(
            io::ErrorKind::InvalidData,
            "relay identity file is not a private 32-byte regular file",
        ));
    }

    let mut seed = Zeroizing::new([0_u8; 32]);
    file.read_exact(&mut *seed).map_err(RelayError::Identity)?;
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing).map_err(RelayError::Identity)? != 0 {
        return Err(identity_error(
            io::ErrorKind::InvalidData,
            "relay identity file has an invalid length",
        ));
    }
    Ok(Some(SigningIdentity::from_seed(*seed)))
}

fn create_identity_file(
    directory: &File,
    name: &std::ffi::OsStr,
    seed: &[u8; 32],
) -> io::Result<()> {
    let (temporary_name, descriptor) = (0..8)
        .find_map(|_| {
            let temporary_name = format!(".equalturn-relay-{}.tmp", uuid::Uuid::new_v4());
            match unix_fs::openat(
                directory,
                temporary_name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::from(0o600),
            ) {
                Ok(descriptor) => Some(Ok((temporary_name, descriptor))),
                Err(Errno::EXIST) => None,
                Err(error) => Some(Err(io::Error::from(error))),
            }
        })
        .transpose()?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not allocate a unique relay identity temporary file",
            )
        })?;
    let mut file = File::from(descriptor);
    let result = (|| {
        unix_fs::fchmod(&file, Mode::from(0o600)).map_err(io::Error::from)?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != process::geteuid().as_raw()
            || metadata.mode() & 0o7077 != 0
            || metadata.mode() & 0o600 != 0o600
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "created relay identity file is not private",
            ));
        }
        file.write_all(seed)?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(error) = result {
        drop(file);
        let cleanup = cleanup_temporary_file(directory, &temporary_name);
        return match cleanup {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(cleanup_error),
        };
    }

    drop(file);
    match unix_fs::linkat(
        directory,
        temporary_name.as_str(),
        directory,
        name,
        unix_fs::AtFlags::empty(),
    ) {
        Ok(()) => {
            unix_fs::unlinkat(
                directory,
                temporary_name.as_str(),
                unix_fs::AtFlags::empty(),
            )
            .map_err(io::Error::from)?;
            directory.sync_all()
        }
        Err(error) => {
            let cleanup = cleanup_temporary_file(directory, &temporary_name);
            match cleanup {
                Ok(()) => Err(io::Error::from(error)),
                Err(cleanup_error) => Err(cleanup_error),
            }
        }
    }
}

fn cleanup_temporary_file(directory: &File, name: &str) -> io::Result<()> {
    unix_fs::unlinkat(directory, name, unix_fs::AtFlags::empty())
        .map_err(io::Error::from)
        .and_then(|()| directory.sync_all())
}

fn identity_error(kind: io::ErrorKind, message: &'static str) -> RelayError {
    RelayError::Identity(io::Error::new(kind, message))
}
