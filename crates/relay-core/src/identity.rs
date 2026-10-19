#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use unix::load_or_create_relay_identity;

#[cfg(not(unix))]
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

#[cfg(not(unix))]
use equalturn_protocol::SigningIdentity;
#[cfg(not(unix))]
use zeroize::Zeroizing;

#[cfg(not(unix))]
use crate::RelayError;

#[cfg(not(unix))]
pub fn load_or_create_relay_identity(path: &Path) -> Result<SigningIdentity, RelayError> {
    if let Some(identity) = read_existing_identity(path).map_err(RelayError::Identity)? {
        return Ok(identity);
    }

    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::create_dir_all(parent).map_err(RelayError::Identity)?;
    let identity = SigningIdentity::generate().map_err(|_| RelayError::Randomness)?;
    let seed = Zeroizing::new(identity.seed_bytes());
    let temporary_path = parent.join(format!(".equalturn-relay-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(RelayError::Identity)?;
    if let Err(error) = file.write_all(&*seed).and_then(|()| file.sync_all()) {
        drop(file);
        return Err(RelayError::Identity(cleanup_temporary_file(
            &temporary_path,
            error,
        )));
    }
    drop(file);

    match fs::hard_link(&temporary_path, path) {
        Ok(()) => Ok(identity),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(&temporary_path).map_err(RelayError::Identity)?;
            read_existing_identity(path)
                .map_err(RelayError::Identity)?
                .ok_or_else(|| {
                    RelayError::Identity(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "relay identity changed during creation",
                    ))
                })
        }
        Err(error) => Err(RelayError::Identity(cleanup_temporary_file(
            &temporary_path,
            error,
        ))),
    }
}

#[cfg(not(unix))]
fn read_existing_identity(path: &Path) -> io::Result<Option<SigningIdentity>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || is_reparse_point(&metadata) || metadata.len() != 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "relay identity file is not a regular 32-byte file",
        ));
    }
    let mut seed = Zeroizing::new([0_u8; 32]);
    file.read_exact(&mut *seed)?;
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "relay identity file has an invalid length",
        ));
    }
    Ok(Some(SigningIdentity::from_seed(*seed)))
}

#[cfg(all(not(unix), windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
}

#[cfg(all(not(unix), not(windows)))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(not(unix))]
fn cleanup_temporary_file(path: &Path, original_error: io::Error) -> io::Error {
    match fs::remove_file(path) {
        Ok(()) => original_error,
        Err(cleanup_error) => cleanup_error,
    }
}

#[cfg(test)]
mod tests;
