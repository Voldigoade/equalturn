use crate::{
    REGISTRY_TARGET_PATH, ROOT_MAX_BYTES, RegistryError, TufFailureKind, model::Registry,
    repository::DirectoryRepository,
};
use jiff::Timestamp;
use sha2::{Digest, Sha256};
use sigstore_tuf::{Error as TufError, MetadataStore, Updater, UpdaterConfig};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const CACHE_MAX_BYTES: usize = crate::REGISTRY_MAX_BYTES;

pub struct VerifiedRegistry {
    registry: Registry,
    target_sha256: [u8; 32],
    trusted_root_version: u64,
}

impl VerifiedRegistry {
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn target_sha256(&self) -> &[u8; 32] {
        &self.target_sha256
    }

    pub fn trusted_root_version(&self) -> u64 {
        self.trusted_root_version
    }
}

pub fn verify_bootstrap_root(
    root_bytes: &[u8],
    expected_sha256: &[u8; 32],
) -> Result<(), RegistryError> {
    if root_bytes.len() > ROOT_MAX_BYTES {
        return Err(RegistryError::RootTooLarge {
            max_bytes: ROOT_MAX_BYTES,
        });
    }
    let actual = Sha256::digest(root_bytes);
    if actual.as_slice() != expected_sha256 {
        return Err(RegistryError::BootstrapPinMismatch);
    }
    Ok(())
}

pub async fn verify_repository(
    repository_path: &Path,
    bootstrap_root: &[u8],
    bootstrap_root_sha256: &[u8; 32],
    metadata_cache: &Path,
    now: Timestamp,
) -> Result<VerifiedRegistry, RegistryError> {
    verify_bootstrap_root(bootstrap_root, bootstrap_root_sha256)?;
    let repository =
        DirectoryRepository::open(repository_path).map_err(|_| RegistryError::Repository)?;
    let store = Arc::new(
        PersistentMetadataStore::open(metadata_cache).map_err(|_| RegistryError::Repository)?,
    );
    let mut config = UpdaterConfig::default();
    config.root_max_length = 512 * 1024;
    config.timestamp_max_length = 64 * 1024;
    config.snapshot_max_length = 1024 * 1024;
    config.targets_max_length = 2 * 1024 * 1024;
    config.target_max_length = crate::REGISTRY_MAX_BYTES as u64;
    config.max_root_rotations = 32;
    config.max_delegations = 8;
    let mut updater = Updater::new(repository, bootstrap_root)
        .map_err(|_| RegistryError::Tuf(TufFailureKind::InvalidRoot))?
        .with_config(config)
        .with_store(Arc::clone(&store));
    updater.refresh(now).await.map_err(classify_tuf_error)?;
    let target = updater
        .get_target(REGISTRY_TARGET_PATH, now)
        .await
        .map_err(classify_target_error)?;
    verify_persisted_floor(store.as_ref(), &updater, &target)?;
    let registry = Registry::parse(&target)?;
    let target_sha256: [u8; 32] = Sha256::digest(&target).into();
    let trusted_root_version = updater.trusted().root().version;
    Ok(VerifiedRegistry {
        registry,
        target_sha256,
        trusted_root_version,
    })
}

fn classify_tuf_error(error: TufError) -> RegistryError {
    let kind = match error {
        TufError::Expired { .. } => TufFailureKind::Expired,
        TufError::Rollback { .. } => TufFailureKind::Rollback,
        TufError::BadRootVersion { .. } => TufFailureKind::InvalidRoot,
        TufError::ThresholdNotMet { role, .. } if role == "root" => TufFailureKind::InvalidRoot,
        TufError::ThresholdNotMet { .. } | TufError::Crypto(_) => TufFailureKind::InvalidSignature,
        _ => TufFailureKind::InvalidMetadata,
    };
    RegistryError::Tuf(kind)
}

fn classify_target_error(error: TufError) -> RegistryError {
    let classified = classify_tuf_error(error);
    match classified {
        RegistryError::Tuf(
            TufFailureKind::Expired | TufFailureKind::Rollback | TufFailureKind::InvalidSignature,
        ) => classified,
        _ => RegistryError::Tuf(TufFailureKind::InvalidTarget),
    }
}

struct PersistentMetadataStore {
    root: PathBuf,
    expected_writes: Mutex<std::collections::HashMap<String, [u8; 32]>>,
    failed_write: AtomicBool,
}

impl PersistentMetadataStore {
    fn open(root: &Path) -> Result<Self, TufError> {
        fs::create_dir_all(root)
            .map_err(|_| TufError::transport("metadata cache directory cannot be created"))?;
        let metadata = fs::symlink_metadata(root)
            .map_err(|_| TufError::transport("metadata cache directory cannot be inspected"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(TufError::transport(
                "metadata cache must be a real directory",
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            expected_writes: Mutex::new(std::collections::HashMap::new()),
            failed_write: AtomicBool::new(false),
        })
    }

    fn path(&self, name: &str, create_parents: bool) -> Result<PathBuf, TufError> {
        if !safe_cache_name(name) {
            return Err(TufError::transport("unsafe metadata cache name"));
        }
        let mut path = self.root.clone();
        let components: Vec<_> = Path::new(name).components().collect();
        for (index, component) in components.iter().enumerate() {
            let Component::Normal(part) = component else {
                return Err(TufError::transport("unsafe metadata cache name"));
            };
            path.push(part);
            if index + 1 < components.len() {
                match fs::symlink_metadata(&path) {
                    Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                        return Err(TufError::transport(
                            "metadata cache path is not a real directory",
                        ));
                    }
                    Ok(_) => {}
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound && create_parents =>
                    {
                        fs::create_dir(&path).map_err(|_| {
                            TufError::transport("metadata cache directory cannot be created")
                        })?;
                        #[cfg(unix)]
                        fs::File::open(path.parent().ok_or_else(|| {
                            TufError::transport("metadata cache directory has no parent")
                        })?)
                        .and_then(|directory| directory.sync_all())
                        .map_err(|_| {
                            TufError::transport("metadata cache parent cannot be synchronized")
                        })?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(path),
                    Err(_) => {
                        return Err(TufError::transport(
                            "metadata cache path cannot be inspected",
                        ));
                    }
                }
            }
        }
        Ok(path)
    }
}

impl MetadataStore for PersistentMetadataStore {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, TufError> {
        let path = self.path(name, false)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(TufError::transport("cached metadata cannot be inspected")),
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > CACHE_MAX_BYTES as u64
        {
            return Err(TufError::transport(
                "cached metadata has an unsafe type or size",
            ));
        }
        let file = fs::File::open(&path)
            .map_err(|_| TufError::transport("cached metadata cannot be opened"))?;
        let opened = file
            .metadata()
            .map_err(|_| TufError::transport("cached metadata cannot be inspected"))?;
        if !opened.is_file() || opened.len() > CACHE_MAX_BYTES as u64 {
            return Err(TufError::transport(
                "cached metadata has an unsafe type or size",
            ));
        }
        let mut bytes = Vec::with_capacity(opened.len() as usize);
        file.take((CACHE_MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| TufError::transport("cached metadata cannot be read"))?;
        if bytes.len() > CACHE_MAX_BYTES {
            return Err(TufError::transport(
                "cached metadata exceeds the size limit",
            ));
        }
        Ok(Some(bytes))
    }

    fn store(&self, name: &str, bytes: &[u8]) -> Result<(), TufError> {
        let result = self.store_bytes(name, bytes);
        if let Err(error) = result {
            self.failed_write.store(true, Ordering::Relaxed);
            return Err(error);
        }
        let mut expected = self.expected_writes.lock().map_err(|_| {
            self.failed_write.store(true, Ordering::Relaxed);
            TufError::transport("metadata cache state is unavailable")
        })?;
        expected.insert(name.to_owned(), Sha256::digest(bytes).into());
        Ok(())
    }
}

impl PersistentMetadataStore {
    fn store_bytes(&self, name: &str, bytes: &[u8]) -> Result<(), TufError> {
        if bytes.len() > CACHE_MAX_BYTES {
            return Err(TufError::transport(
                "metadata cache write exceeds the size limit",
            ));
        }
        let path = self.path(name, true)?;
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err(TufError::transport(
                "metadata cache destination has an unsafe type",
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| TufError::transport("metadata cache path has no parent"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| TufError::transport("metadata cache temporary file cannot be created"))?;
        temporary
            .write_all(bytes)
            .map_err(|_| TufError::transport("metadata cache temporary file cannot be written"))?;
        temporary.as_file().sync_all().map_err(|_| {
            TufError::transport("metadata cache temporary file cannot be synchronized")
        })?;
        temporary
            .persist(&path)
            .map_err(|_| TufError::transport("metadata cache update cannot be committed"))?;
        #[cfg(unix)]
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| TufError::transport("metadata cache directory cannot be synchronized"))?;
        Ok(())
    }

    fn expected_digest(&self, name: &str) -> Result<Option<[u8; 32]>, RegistryError> {
        self.expected_writes
            .lock()
            .map(|expected| expected.get(name).copied())
            .map_err(|_| RegistryError::Repository)
    }
}

fn safe_cache_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && value.is_ascii()
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
}

fn verify_persisted_floor(
    store: &PersistentMetadataStore,
    updater: &Updater,
    target: &[u8],
) -> Result<(), RegistryError> {
    if store.failed_write.load(Ordering::Relaxed) {
        return Err(RegistryError::Repository);
    }
    let root = updater.trusted().root();
    let root_history_name = format!("root_history/{}.root.json", root.version);
    for name in ["root.json", root_history_name.as_str()] {
        let cached = store
            .load(name)
            .map_err(|_| RegistryError::Repository)?
            .ok_or(RegistryError::Repository)?;
        if cached.as_slice() != updater.trusted().root_bytes() {
            return Err(RegistryError::Repository);
        }
        verify_expected_digest(store, name, &cached)?;
    }
    let timestamp_version = updater
        .trusted()
        .timestamp()
        .ok_or(RegistryError::Repository)?
        .version;
    let snapshot_version = updater
        .trusted()
        .snapshot()
        .ok_or(RegistryError::Repository)?
        .version;
    let targets_version = updater
        .trusted()
        .targets_role("targets")
        .ok_or(RegistryError::Repository)?
        .version;
    for (name, version) in [
        ("timestamp.json", timestamp_version),
        ("snapshot.json", snapshot_version),
        ("targets.json", targets_version),
    ] {
        let bytes = store
            .load(name)
            .map_err(|_| RegistryError::Repository)?
            .ok_or(RegistryError::Repository)?;
        let metadata: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| RegistryError::Repository)?;
        if metadata
            .get("signed")
            .and_then(|value| value.get("version"))
            .and_then(serde_json::Value::as_u64)
            != Some(version)
        {
            return Err(RegistryError::Repository);
        }
        verify_expected_digest(store, name, &bytes)?;
    }
    let cached_target = store
        .load(&format!("targets/{REGISTRY_TARGET_PATH}"))
        .map_err(|_| RegistryError::Repository)?
        .ok_or(RegistryError::Repository)?;
    if cached_target != target {
        return Err(RegistryError::Repository);
    }
    verify_expected_digest(
        store,
        &format!("targets/{REGISTRY_TARGET_PATH}"),
        &cached_target,
    )?;
    Ok(())
}

fn verify_expected_digest(
    store: &PersistentMetadataStore,
    name: &str,
    bytes: &[u8],
) -> Result<(), RegistryError> {
    if let Some(expected) = store.expected_digest(name)? {
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        if actual != expected {
            return Err(RegistryError::Repository);
        }
    }
    Ok(())
}
