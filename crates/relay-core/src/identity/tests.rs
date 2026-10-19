use std::{
    fs,
    path::{Path, PathBuf},
};

use super::load_or_create_relay_identity;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("equalturn-relay-identity-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).expect("create temporary test directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("restrict temporary test directory");
        }
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn relay_identity_survives_reopen_without_replacement() {
    let temp = TempDir::new();
    let path = temp.path().join("relay.key");

    let original = load_or_create_relay_identity(&path).expect("create relay identity");
    let reopened = load_or_create_relay_identity(&path).expect("reload relay identity");

    assert_eq!(original.public_key(), reopened.public_key());
}

#[cfg(unix)]
#[test]
fn newly_created_parent_and_identity_are_owner_only() {
    use std::os::unix::fs::MetadataExt;

    let temp = TempDir::new();
    let directory = temp.path().join("private");
    let path = directory.join("relay.key");

    let _identity = load_or_create_relay_identity(&path).expect("create relay identity");

    assert_eq!(
        fs::metadata(&directory).expect("parent metadata").mode() & 0o7777,
        0o700
    );
    let metadata = fs::metadata(&path).expect("key metadata");
    assert_eq!(metadata.mode() & 0o7777, 0o600);
    assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
}

#[cfg(unix)]
#[test]
fn existing_parent_with_group_or_other_access_is_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new();
    let directory = temp.path().join("shared");
    fs::create_dir(&directory).expect("create shared directory");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))
        .expect("make shared directory");

    let result = load_or_create_relay_identity(&directory.join("relay.key"));

    assert!(result.is_err(), "shared key directory must be rejected");
}

#[cfg(unix)]
#[test]
fn relay_identity_symlink_is_rejected_without_loading_its_target() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let target = temp.path().join("target.key");
    let alias = temp.path().join("relay.key");
    let target_identity = load_or_create_relay_identity(&target).expect("create target key");
    symlink(&target, &alias).expect("create key symlink");

    assert!(load_or_create_relay_identity(&alias).is_err());
    assert_eq!(
        load_or_create_relay_identity(&target)
            .expect("load original target")
            .public_key(),
        target_identity.public_key()
    );
}

#[cfg(unix)]
#[test]
fn relay_identity_parent_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let target_directory = temp.path().join("target");
    let alias_directory = temp.path().join("alias");
    fs::create_dir(&target_directory).expect("create target directory");
    symlink(&target_directory, &alias_directory).expect("create directory symlink");

    assert!(load_or_create_relay_identity(&alias_directory.join("relay.key")).is_err());
    assert!(!target_directory.join("relay.key").exists());
}

#[test]
fn simultaneous_creation_uses_one_persisted_identity() {
    use std::sync::{Arc, Barrier};

    let temp = TempDir::new();
    let path = temp.path().join("relay.key");
    let barrier = Arc::new(Barrier::new(8));
    let threads = (0..8)
        .map(|_| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                load_or_create_relay_identity(&path).map(|identity| identity.public_key())
            })
        })
        .collect::<Vec<_>>();

    let public_keys = threads
        .into_iter()
        .map(|thread| {
            thread
                .join()
                .expect("creation thread does not panic")
                .expect("identity loads")
        })
        .collect::<Vec<_>>();
    assert!(public_keys.iter().all(|key| *key == public_keys[0]));
}

#[cfg(unix)]
#[test]
fn malformed_or_non_regular_relay_identity_files_are_rejected() {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};

    let temp = TempDir::new();
    let truncated = temp.path().join("truncated.key");
    fs::write(&truncated, [1_u8; 31]).expect("write truncated key");
    fs::set_permissions(&truncated, fs::Permissions::from_mode(0o600))
        .expect("restrict truncated key");
    assert!(load_or_create_relay_identity(&truncated).is_err());

    let oversized = temp.path().join("oversized.key");
    fs::write(&oversized, [2_u8; 33]).expect("write oversized key");
    fs::set_permissions(&oversized, fs::Permissions::from_mode(0o600))
        .expect("restrict oversized key");
    assert!(load_or_create_relay_identity(&oversized).is_err());

    let directory = temp.path().join("directory.key");
    fs::create_dir(&directory).expect("create key directory");
    assert!(load_or_create_relay_identity(&directory).is_err());

    let fifo = temp.path().join("fifo.key");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("run mkfifo");
    assert!(status.success(), "create FIFO test fixture");
    assert!(
        fs::symlink_metadata(&fifo)
            .expect("inspect FIFO")
            .file_type()
            .is_fifo()
    );
    assert!(load_or_create_relay_identity(&fifo).is_err());
}

#[cfg(unix)]
#[test]
fn relay_identity_file_with_group_or_other_permissions_is_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new();
    let path = temp.path().join("relay.key");
    let _identity = load_or_create_relay_identity(&path).expect("create relay identity");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("make key readable");

    assert!(load_or_create_relay_identity(&path).is_err());
}
