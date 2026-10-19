use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use equalturn_protocol::SigningIdentity;
use zeroize::Zeroize;

use crate::RelayError;

pub fn load_or_create_relay_identity(path: &Path) -> Result<SigningIdentity, RelayError> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path).map_err(RelayError::Identity)?;
        if !metadata.file_type().is_file() {
            return Err(RelayError::Identity(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "relay identity path is not a regular file",
            )));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(RelayError::Identity(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "relay identity file permissions must be owner-only",
                )));
            }
        }
        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(RelayError::Identity)?;
        let mut seed = [0_u8; 32];
        file.read_exact(&mut seed).map_err(RelayError::Identity)?;
        let mut trailing = [0_u8; 1];
        if file.read(&mut trailing).map_err(RelayError::Identity)? != 0 {
            seed.zeroize();
            return Err(RelayError::Identity(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "relay identity file has an invalid length",
            )));
        }
        let identity = SigningIdentity::from_seed(seed);
        seed.zeroize();
        return Ok(identity);
    }

    let parent = path.parent().ok_or_else(|| {
        RelayError::Identity(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "relay identity path has no parent directory",
        ))
    })?;
    fs::create_dir_all(parent).map_err(RelayError::Identity)?;
    let identity = SigningIdentity::generate().map_err(|_| RelayError::Randomness)?;
    let mut seed = identity.seed_bytes();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(RelayError::Identity)?;
    let write_result = file.write_all(&seed).and_then(|()| file.sync_all());
    seed.zeroize();
    write_result.map_err(RelayError::Identity)?;
    Ok(identity)
}
