use crate::{
    REGISTRY_MAX_BYTES, REGISTRY_TARGET_PATH, Registry, RegistryError,
    trust::{VerifiedRegistry, verify_bootstrap_root, verify_https_repository, verify_repository},
};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use url::Url;

const POINTER_MAX_BYTES: usize = 4096;
const INSTALLED_MAX_BYTES: usize = 20 * 1024 * 1024;
const METADATA_ROOT_LIMIT: usize = 512 * 1024;
const METADATA_TIMESTAMP_LIMIT: usize = 64 * 1024;
const METADATA_SNAPSHOT_LIMIT: usize = 1024 * 1024;
const METADATA_TARGETS_LIMIT: usize = 2 * 1024 * 1024;
const MAX_ROOT_ROTATIONS: u64 = 32;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledPointer {
    format_version: u32,
    slot: String,
    bootstrap_root_sha256: String,
    target_sha256: String,
    registry_version: String,
    root_version: u64,
    last_successful_refresh: String,
}

pub async fn update_directory_repository(
    repository_path: &Path,
    bootstrap_root: &[u8],
    bootstrap_root_sha256: &[u8; 32],
    state_dir: &Path,
    now: Timestamp,
) -> Result<VerifiedRegistry, RegistryError> {
    prepare_state_dir(state_dir)?;
    let cache = state_dir.join("update-cache");
    let update_lock = open_lock(&state_dir.join("update.lock"))?;
    update_lock.lock().map_err(|_| RegistryError::Repository)?;
    let verified = verify_repository(
        repository_path,
        bootstrap_root,
        bootstrap_root_sha256,
        &cache,
        now,
    )
    .await?;
    install_cache_snapshot(
        state_dir,
        &cache,
        bootstrap_root,
        bootstrap_root_sha256,
        &verified,
        now,
    )
    .await
}

pub async fn update_https_repository(
    base_url: &Url,
    bootstrap_root: &[u8],
    bootstrap_root_sha256: &[u8; 32],
    state_dir: &Path,
    now: Timestamp,
) -> Result<VerifiedRegistry, RegistryError> {
    prepare_state_dir(state_dir)?;
    let cache = state_dir.join("update-cache");
    let update_lock = open_lock(&state_dir.join("update.lock"))?;
    update_lock.lock().map_err(|_| RegistryError::Repository)?;
    let verified =
        verify_https_repository(base_url, bootstrap_root, bootstrap_root_sha256, &cache, now)
            .await?;
    install_cache_snapshot(
        state_dir,
        &cache,
        bootstrap_root,
        bootstrap_root_sha256,
        &verified,
        now,
    )
    .await
}

pub async fn load_installed_registry(
    state_dir: &Path,
    bootstrap_root: &[u8],
    bootstrap_root_sha256: &[u8; 32],
    now: Timestamp,
) -> Result<VerifiedRegistry, RegistryError> {
    verify_bootstrap_root(bootstrap_root, bootstrap_root_sha256)?;
    ensure_real_directory(state_dir, false)?;
    let lock = open_lock(&state_dir.join("install.lock"))?;
    lock.lock().map_err(|_| RegistryError::Repository)?;
    let pointer = read_pointer(state_dir)?;
    if pointer.bootstrap_root_sha256 != hex(bootstrap_root_sha256) {
        return Err(RegistryError::BootstrapPinMismatch);
    }
    let slot_dir = state_dir.join("slots");
    ensure_real_directory(&slot_dir, false)?;
    let slot_path = slot_dir.join(&pointer.slot);
    let cache = tempfile::tempdir().map_err(|_| RegistryError::Repository)?;
    let verified = verify_repository(
        &slot_path,
        bootstrap_root,
        bootstrap_root_sha256,
        cache.path(),
        now,
    )
    .await?;
    verify_pointer_claims(&pointer, &verified)?;
    Ok(verified)
}

async fn install_cache_snapshot(
    state_dir: &Path,
    cache: &Path,
    bootstrap_root: &[u8],
    bootstrap_root_sha256: &[u8; 32],
    verified: &VerifiedRegistry,
    now: Timestamp,
) -> Result<VerifiedRegistry, RegistryError> {
    let candidate = tempfile::Builder::new()
        .prefix(".registry-candidate-")
        .tempdir_in(state_dir)
        .map_err(|_| RegistryError::Repository)?;
    build_repository_snapshot(cache, candidate.path(), bootstrap_root, verified)?;

    let validation_cache = tempfile::tempdir().map_err(|_| RegistryError::Repository)?;
    let candidate_verified = verify_repository(
        candidate.path(),
        bootstrap_root,
        bootstrap_root_sha256,
        validation_cache.path(),
        now,
    )
    .await?;
    if candidate_verified.target_sha256() != verified.target_sha256()
        || candidate_verified.trusted_root_version() != verified.trusted_root_version()
        || candidate_verified.registry().registry_version()
            != verified.registry().registry_version()
    {
        return Err(RegistryError::Repository);
    }

    let lock = open_lock(&state_dir.join("install.lock"))?;
    lock.lock().map_err(|_| RegistryError::Repository)?;
    let slots = state_dir.join("slots");
    ensure_real_directory(&slots, true)?;
    let current = read_pointer_optional(state_dir)?;
    let inactive = match current.as_ref().map(|pointer| pointer.slot.as_str()) {
        Some("a") => "b",
        Some("b") => "a",
        Some(_) => return Err(RegistryError::Repository),
        None => "a",
    };
    let destination = slots.join(inactive);
    remove_existing_slot(&destination)?;
    let candidate_path = candidate.keep();
    fs::rename(&candidate_path, &destination).map_err(|_| RegistryError::Repository)?;
    sync_directory(&slots)?;
    sync_directory(state_dir)?;

    let pointer = InstalledPointer {
        format_version: 1,
        slot: inactive.to_owned(),
        bootstrap_root_sha256: hex(bootstrap_root_sha256),
        target_sha256: hex(verified.target_sha256()),
        registry_version: verified.registry().registry_version().to_owned(),
        root_version: verified.trusted_root_version(),
        last_successful_refresh: now.to_string(),
    };
    write_pointer(state_dir, &pointer)?;
    Ok(candidate_verified)
}

fn build_repository_snapshot(
    cache: &Path,
    destination: &Path,
    bootstrap_root: &[u8],
    verified: &VerifiedRegistry,
) -> Result<(), RegistryError> {
    let root = read_bounded(&cache.join("root.json"), METADATA_ROOT_LIMIT)?;
    let timestamp = read_bounded(&cache.join("timestamp.json"), METADATA_TIMESTAMP_LIMIT)?;
    let snapshot = read_bounded(&cache.join("snapshot.json"), METADATA_SNAPSHOT_LIMIT)?;
    let targets = read_bounded(&cache.join("targets.json"), METADATA_TARGETS_LIMIT)?;
    let target = read_bounded(
        &cache.join("targets").join(REGISTRY_TARGET_PATH),
        REGISTRY_MAX_BYTES,
    )?;
    if <[u8; 32]>::from(Sha256::digest(&target)) != *verified.target_sha256() {
        return Err(RegistryError::Repository);
    }
    let root_value = parse_metadata(&root)?;
    let bootstrap_value = parse_metadata(bootstrap_root)?;
    let timestamp_value = parse_metadata(&timestamp)?;
    let snapshot_value = parse_metadata(&snapshot)?;
    let targets_value = parse_metadata(&targets)?;
    let root_signed = signed(&root_value)?;
    let bootstrap_signed = signed(&bootstrap_value)?;
    let consistent_snapshot = root_signed
        .get("consistent_snapshot")
        .and_then(serde_json::Value::as_bool)
        .ok_or(RegistryError::Repository)?;
    let root_version = version(root_signed)?;
    let bootstrap_version = version(bootstrap_signed)?;
    if root_version != verified.trusted_root_version()
        || root_version < bootstrap_version
        || root_version - bootstrap_version > MAX_ROOT_ROTATIONS
    {
        return Err(RegistryError::Repository);
    }
    let timestamp_signed = signed(&timestamp_value)?;
    let snapshot_version = referenced_version(timestamp_signed, "snapshot.json")?;
    let snapshot_signed = signed(&snapshot_value)?;
    if version(snapshot_signed)? != snapshot_version {
        return Err(RegistryError::Repository);
    }
    let targets_version = referenced_version(snapshot_signed, "targets.json")?;
    let targets_signed = signed(&targets_value)?;
    if version(targets_signed)? != targets_version {
        return Err(RegistryError::Repository);
    }
    let target_metadata = targets_signed
        .get("targets")
        .and_then(|value| value.get(REGISTRY_TARGET_PATH))
        .ok_or(RegistryError::Repository)?;
    let hashes = target_metadata
        .get("hashes")
        .and_then(serde_json::Value::as_object)
        .ok_or(RegistryError::Repository)?;
    let target_name = if let Some(hash) = hashes.get("sha256").and_then(serde_json::Value::as_str) {
        if hash != hex(verified.target_sha256()) {
            return Err(RegistryError::Repository);
        }
        format!("{hash}.{REGISTRY_TARGET_PATH}")
    } else if let Some(hash) = hashes.get("sha512").and_then(serde_json::Value::as_str) {
        if !valid_hex(hash, 128) {
            return Err(RegistryError::Repository);
        }
        format!("{hash}.{REGISTRY_TARGET_PATH}")
    } else {
        return Err(RegistryError::Repository);
    };
    let target_length = target_metadata
        .get("length")
        .and_then(serde_json::Value::as_u64)
        .ok_or(RegistryError::Repository)?;
    if target_length != target.len() as u64 {
        return Err(RegistryError::Repository);
    }
    if Registry::parse(&target)?.registry_version() != verified.registry().registry_version() {
        return Err(RegistryError::Repository);
    }

    let metadata_dir = destination.join("metadata");
    let target_dir = destination.join("targets");
    fs::create_dir(&metadata_dir).map_err(|_| RegistryError::Repository)?;
    fs::create_dir(&target_dir).map_err(|_| RegistryError::Repository)?;
    let mut total = 0usize;
    write_snapshot_file(&metadata_dir.join("root.json"), &root, &mut total)?;
    for next in bootstrap_version.saturating_add(1)..=root_version {
        let history = read_bounded(
            &cache.join("root_history").join(format!("{next}.root.json")),
            METADATA_ROOT_LIMIT,
        )?;
        write_snapshot_file(
            &metadata_dir.join(format!("{next}.root.json")),
            &history,
            &mut total,
        )?;
    }
    write_snapshot_file(&metadata_dir.join("timestamp.json"), &timestamp, &mut total)?;
    let snapshot_name = if consistent_snapshot {
        format!("{snapshot_version}.snapshot.json")
    } else {
        "snapshot.json".to_owned()
    };
    let targets_name = if consistent_snapshot {
        format!("{targets_version}.targets.json")
    } else {
        "targets.json".to_owned()
    };
    write_snapshot_file(&metadata_dir.join(snapshot_name), &snapshot, &mut total)?;
    write_snapshot_file(&metadata_dir.join(targets_name), &targets, &mut total)?;
    let target_path = if consistent_snapshot {
        target_dir.join(target_name)
    } else {
        target_dir.join(REGISTRY_TARGET_PATH)
    };
    write_snapshot_file(&target_path, &target, &mut total)?;
    if total > INSTALLED_MAX_BYTES {
        return Err(RegistryError::Repository);
    }
    sync_directory(&metadata_dir)?;
    sync_directory(&target_dir)?;
    sync_directory(destination)?;
    Ok(())
}

fn write_snapshot_file(path: &Path, bytes: &[u8], total: &mut usize) -> Result<(), RegistryError> {
    *total = total
        .checked_add(bytes.len())
        .ok_or(RegistryError::Repository)?;
    if *total > INSTALLED_MAX_BYTES {
        return Err(RegistryError::Repository);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| RegistryError::Repository)?;
    file.write_all(bytes)
        .map_err(|_| RegistryError::Repository)?;
    file.sync_all().map_err(|_| RegistryError::Repository)?;
    Ok(())
}

fn parse_metadata(bytes: &[u8]) -> Result<serde_json::Value, RegistryError> {
    let value = crate::json::parse_unique(bytes).map_err(|_| RegistryError::Repository)?;
    Ok(value)
}

fn signed(value: &serde_json::Value) -> Result<&serde_json::Value, RegistryError> {
    value.get("signed").ok_or(RegistryError::Repository)
}

fn version(signed: &serde_json::Value) -> Result<u64, RegistryError> {
    signed
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .filter(|version| *version > 0)
        .ok_or(RegistryError::Repository)
}

fn referenced_version(signed: &serde_json::Value, name: &str) -> Result<u64, RegistryError> {
    signed
        .get("meta")
        .and_then(|meta| meta.get(name))
        .and_then(|metadata| metadata.get("version"))
        .and_then(serde_json::Value::as_u64)
        .filter(|version| *version > 0)
        .ok_or(RegistryError::Repository)
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, RegistryError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| RegistryError::Repository)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(RegistryError::Repository);
    }
    let file = File::open(path).map_err(|_| RegistryError::Repository)?;
    let opened = file.metadata().map_err(|_| RegistryError::Repository)?;
    if !opened.is_file() || opened.len() > maximum as u64 {
        return Err(RegistryError::Repository);
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| RegistryError::Repository)?;
    if bytes.len() > maximum {
        return Err(RegistryError::Repository);
    }
    Ok(bytes)
}

fn prepare_state_dir(path: &Path) -> Result<(), RegistryError> {
    fs::create_dir_all(path).map_err(|_| RegistryError::Repository)?;
    ensure_real_directory(path, false)
}

fn ensure_real_directory(path: &Path, create: bool) -> Result<(), RegistryError> {
    if create {
        match fs::create_dir(path) {
            Ok(()) => sync_directory(path.parent().ok_or(RegistryError::Repository)?)?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(RegistryError::Repository),
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| RegistryError::Repository)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RegistryError::Repository);
    }
    Ok(())
}

fn open_lock(path: &Path) -> Result<File, RegistryError> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(RegistryError::Repository);
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| RegistryError::Repository)?;
    let metadata = file.metadata().map_err(|_| RegistryError::Repository)?;
    let path_metadata = fs::symlink_metadata(path).map_err(|_| RegistryError::Repository)?;
    if !metadata.is_file() || path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(RegistryError::Repository);
    }
    Ok(file)
}

fn read_pointer_optional(state_dir: &Path) -> Result<Option<InstalledPointer>, RegistryError> {
    let path = state_dir.join("current.json");
    let bytes = match read_bounded(&path, POINTER_MAX_BYTES) {
        Ok(bytes) => bytes,
        Err(RegistryError::Repository)
            if fs::symlink_metadata(&path)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let value = crate::json::parse_unique(&bytes).map_err(|_| RegistryError::Repository)?;
    let pointer: InstalledPointer =
        serde_json::from_value(value).map_err(|_| RegistryError::Repository)?;
    if pointer.format_version != 1
        || !matches!(pointer.slot.as_str(), "a" | "b")
        || !valid_hex(&pointer.bootstrap_root_sha256, 64)
        || !valid_hex(&pointer.target_sha256, 64)
        || pointer.registry_version.is_empty()
        || pointer.registry_version.len() > 128
        || pointer.root_version == 0
        || pointer.last_successful_refresh.len() > 64
        || pointer
            .last_successful_refresh
            .parse::<Timestamp>()
            .is_err()
    {
        return Err(RegistryError::Repository);
    }
    Ok(Some(pointer))
}

fn read_pointer(state_dir: &Path) -> Result<InstalledPointer, RegistryError> {
    read_pointer_optional(state_dir)?.ok_or(RegistryError::Repository)
}

fn write_pointer(state_dir: &Path, pointer: &InstalledPointer) -> Result<(), RegistryError> {
    let path = state_dir.join("current.json");
    if let Ok(metadata) = fs::symlink_metadata(&path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(RegistryError::Repository);
    }
    let bytes = serde_json::to_vec(pointer).map_err(|_| RegistryError::Repository)?;
    if bytes.len() > POINTER_MAX_BYTES {
        return Err(RegistryError::Repository);
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(state_dir).map_err(|_| RegistryError::Repository)?;
    temporary
        .write_all(&bytes)
        .map_err(|_| RegistryError::Repository)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| RegistryError::Repository)?;
    temporary
        .persist(&path)
        .map_err(|_| RegistryError::Repository)?;
    sync_directory(state_dir)?;
    Ok(())
}

fn remove_existing_slot(path: &Path) -> Result<(), RegistryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(RegistryError::Repository)
        }
        Ok(_) => fs::remove_dir_all(path).map_err(|_| RegistryError::Repository),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(RegistryError::Repository),
    }
}

fn verify_pointer_claims(
    pointer: &InstalledPointer,
    verified: &VerifiedRegistry,
) -> Result<(), RegistryError> {
    if pointer.target_sha256 != hex(verified.target_sha256())
        || pointer.root_version != verified.trusted_root_version()
        || pointer.registry_version != verified.registry().registry_version()
    {
        return Err(RegistryError::Repository);
    }
    Ok(())
}

fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn sync_directory(path: &Path) -> Result<(), RegistryError> {
    #[cfg(unix)]
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| RegistryError::Repository)?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{hex, valid_hex};

    #[test]
    fn hex_is_lowercase_and_fixed_width() {
        assert_eq!(hex(&[0x00, 0x0a, 0xff]), "000aff");
        assert!(valid_hex("000aff", 6));
        assert!(!valid_hex("000AFF", 6));
        assert!(!valid_hex("000af", 6));
    }
}
