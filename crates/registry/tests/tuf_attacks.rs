use equalturn_registry::{RegistryError, RegistryVerificationStatus, verify_repository};
use jiff::Timestamp;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sigstore_crypto::KeyPair;
use sigstore_tuf::{Key, canonical_json::to_canonical_bytes};
use std::{fs, path::Path};
use tempfile::TempDir;

const FAR_FUTURE: &str = "2999-01-01T00:00:00Z";
const FIXED_NOW: &str = "2026-10-19T12:00:00Z";
const REGISTRY_BYTES: &[u8] = include_bytes!("../../../registry/registry-v3.json");

trait ResultExpectError<T, E> {
    fn require_error(self, message: &str) -> E;
}

impl<T, E> ResultExpectError<T, E> for Result<T, E> {
    fn require_error(self, message: &str) -> E {
        match self {
            Err(error) => error,
            Ok(_) => panic!("{message}"),
        }
    }
}

struct Signer {
    key: KeyPair,
    id: String,
    value: Value,
}

impl Signer {
    fn generate() -> Self {
        let key = KeyPair::generate_ecdsa_p256().expect("test signing key generation succeeds");
        let public_pem = key
            .public_key_der()
            .expect("test signing key has a public key")
            .to_pem();
        let value = json!({
            "keytype": "ecdsa",
            "scheme": "ecdsa-sha2-nistp256",
            "keyval": { "public": public_pem },
        });
        let parsed: Key =
            serde_json::from_value(value.clone()).expect("public key parses as TUF key");
        let id = parsed.key_id().expect("TUF key ID can be computed");
        Self { key, id, value }
    }

    fn sign(&self, signed: &Value) -> Value {
        let canonical = to_canonical_bytes(signed).expect("TUF canonical JSON is serializable");
        let signature = self
            .key
            .sign(&canonical)
            .expect("test metadata signing succeeds");
        json!({ "keyid": self.id, "sig": hex_encode(signature.as_bytes()) })
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn envelope(signed: &Value, signers: &[&Signer]) -> Vec<u8> {
    let signatures: Vec<_> = signers.iter().map(|signer| signer.sign(signed)).collect();
    serde_json::to_vec(&json!({ "signed": signed, "signatures": signatures }))
        .expect("TUF metadata envelope serializes")
}

fn root_signed(signer: &Signer, version: u64) -> Value {
    let role = json!({ "keyids": [signer.id], "threshold": 1 });
    json!({
        "_type": "root",
        "spec_version": "1.0.0",
        "version": version,
        "expires": FAR_FUTURE,
        "consistent_snapshot": false,
        "keys": { signer.id.clone(): signer.value.clone() },
        "roles": {
            "root": role,
            "timestamp": role,
            "snapshot": role,
            "targets": role,
        },
    })
}

fn role(signer: &Signer) -> Value {
    json!({ "keyids": [signer.id], "threshold": 1 })
}

fn root_signed_with_roles(
    root_signer: &Signer,
    timestamp_signer: &Signer,
    snapshot_signer: &Signer,
    targets_signer: &Signer,
    version: u64,
) -> Value {
    let mut keys = Map::new();
    for signer in [
        root_signer,
        timestamp_signer,
        snapshot_signer,
        targets_signer,
    ] {
        keys.insert(signer.id.clone(), signer.value.clone());
    }

    json!({
        "_type": "root",
        "spec_version": "1.0.0",
        "version": version,
        "expires": FAR_FUTURE,
        "consistent_snapshot": false,
        "keys": keys,
        "roles": {
            "root": role(root_signer),
            "timestamp": role(timestamp_signer),
            "snapshot": role(snapshot_signer),
            "targets": role(targets_signer),
        },
    })
}

fn root_bytes(signer: &Signer) -> Vec<u8> {
    let signed = root_signed(signer, 1);
    envelope(&signed, &[signer])
}

fn metadata_file(bytes: &[u8], version: u64) -> Value {
    json!({
        "version": version,
        "length": bytes.len(),
        "hashes": { "sha256": sha256_hex(bytes) },
    })
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_encode(&sha256(bytes))
}

fn signed_repository_files(
    signer: &Signer,
    target: &[u8],
    version: u64,
    timestamp_expires: &str,
) -> Vec<(String, Vec<u8>)> {
    signed_repository_files_with_roles(signer, signer, signer, target, version, timestamp_expires)
}

fn signed_repository_files_with_roles(
    targets_signer: &Signer,
    snapshot_signer: &Signer,
    timestamp_signer: &Signer,
    target: &[u8],
    version: u64,
    timestamp_expires: &str,
) -> Vec<(String, Vec<u8>)> {
    let targets_signed = json!({
        "_type": "targets",
        "spec_version": "1.0.0",
        "version": version,
        "expires": FAR_FUTURE,
        "targets": {
            "registry-v3.json": {
                "length": target.len(),
                "hashes": { "sha256": sha256_hex(target) },
            }
        },
    });
    let targets = envelope(&targets_signed, &[targets_signer]);

    let snapshot_signed = json!({
        "_type": "snapshot",
        "spec_version": "1.0.0",
        "version": version,
        "expires": FAR_FUTURE,
        "meta": { "targets.json": metadata_file(&targets, version) },
    });
    let snapshot = envelope(&snapshot_signed, &[snapshot_signer]);

    let timestamp_signed = json!({
        "_type": "timestamp",
        "spec_version": "1.0.0",
        "version": version,
        "expires": timestamp_expires,
        "meta": { "snapshot.json": metadata_file(&snapshot, version) },
    });
    let timestamp = envelope(&timestamp_signed, &[timestamp_signer]);

    vec![
        ("timestamp.json".into(), timestamp),
        ("snapshot.json".into(), snapshot),
        ("targets.json".into(), targets),
    ]
}

fn write_repository(directory: &Path, files: &[(String, Vec<u8>)], target: &[u8]) {
    fs::create_dir_all(directory.join("metadata")).expect("metadata directory is created");
    fs::create_dir_all(directory.join("targets")).expect("targets directory is created");
    for (name, bytes) in files {
        fs::write(directory.join("metadata").join(name), bytes)
            .expect("signed metadata is written");
    }
    fs::write(directory.join("targets/registry-v3.json"), target)
        .expect("registry target is written");
}

fn pin(root: &[u8]) -> [u8; 32] {
    sha256(root)
}

fn now() -> Timestamp {
    FIXED_NOW.parse().expect("fixed test timestamp parses")
}

fn temporary_repository(signer: &Signer, target: &[u8], version: u64) -> TempDir {
    let directory = tempfile::tempdir().expect("temporary repository directory is created");
    let files = signed_repository_files(signer, target, version, FAR_FUTURE);
    write_repository(directory.path(), &files, target);
    directory
}

#[tokio::test]
async fn accepts_a_valid_signed_update_and_persists_the_verified_floor() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let cache = tempfile::tempdir().expect("cache directory is created");

    let verified = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path().join("trusted").as_path(),
        now(),
    )
    .await
    .expect("valid repository is accepted");

    assert_eq!(verified.registry().schema_version(), 3);
    assert_eq!(verified.registry().entries().len(), 131);
    assert_eq!(verified.trusted_root_version(), 1);
    assert_eq!(verified.target_sha256(), &sha256(REGISTRY_BYTES));
    assert!(cache.path().join("trusted/timestamp.json").is_file());
    assert!(cache.path().join("trusted/snapshot.json").is_file());
    assert!(cache.path().join("trusted/targets.json").is_file());
}

#[tokio::test]
async fn installed_snapshot_survives_reopen_and_is_reverified_offline() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let state = tempfile::tempdir().expect("installed state directory is created");

    let installed = equalturn_registry::update_directory_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        state.path(),
        now(),
    )
    .await
    .expect("valid repository installs");

    assert_eq!(installed.registry().registry_version(), "0.4.0");
    let reopened = equalturn_registry::load_installed_registry(
        state.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        now(),
    )
    .await
    .expect("installed snapshot reopens without network");
    assert_eq!(reopened.target_sha256(), &sha256(REGISTRY_BYTES));
    assert_eq!(reopened.registry(), installed.registry());

    let pointer_path = state.path().join("current.json");
    let mut legacy_pointer: Value =
        serde_json::from_slice(&fs::read(&pointer_path).expect("active pointer is readable"))
            .expect("active pointer parses");
    legacy_pointer["format_version"] = json!(1);
    legacy_pointer
        .as_object_mut()
        .expect("pointer is an object")
        .remove("source");
    legacy_pointer
        .as_object_mut()
        .expect("pointer is an object")
        .remove("timestamp_version");
    legacy_pointer
        .as_object_mut()
        .expect("pointer is an object")
        .remove("snapshot_version");
    legacy_pointer
        .as_object_mut()
        .expect("pointer is an object")
        .remove("targets_version");
    fs::write(
        &pointer_path,
        serde_json::to_vec(&legacy_pointer).expect("legacy pointer serializes"),
    )
    .expect("legacy pointer is written");

    let migrated = equalturn_registry::load_installed_registry(
        state.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        now(),
    )
    .await
    .expect("version 1 installed pointer remains readable");
    assert_eq!(migrated.timestamp_version(), 1);
    assert_eq!(migrated.snapshot_version(), 1);
    assert_eq!(migrated.targets_version(), 1);
    assert_eq!(migrated.source(), "legacy install record");
}

#[tokio::test]
async fn failed_refresh_preserves_the_previous_installed_snapshot() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let state = tempfile::tempdir().expect("installed state directory is created");
    equalturn_registry::update_directory_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        state.path(),
        now(),
    )
    .await
    .expect("initial signed repository installs");

    let invalid_candidate = temporary_repository(&signer, b"invalid registry", 2);
    assert!(
        equalturn_registry::update_directory_repository(
            invalid_candidate.path(),
            &bootstrap_root,
            &pin(&bootstrap_root),
            state.path(),
            now(),
        )
        .await
        .is_err()
    );

    let still_installed = equalturn_registry::load_installed_registry(
        state.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        now(),
    )
    .await
    .expect("failed candidate does not replace the installed snapshot");
    assert_eq!(still_installed.target_sha256(), &sha256(REGISTRY_BYTES));
}

#[tokio::test]
async fn installed_snapshot_rejects_target_tampering() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let state = tempfile::tempdir().expect("installed state directory is created");
    equalturn_registry::update_directory_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        state.path(),
        now(),
    )
    .await
    .expect("valid repository installs");

    let pointer: Value = serde_json::from_slice(
        &fs::read(state.path().join("current.json")).expect("active pointer is readable"),
    )
    .expect("active pointer parses");
    let slot = pointer["slot"].as_str().expect("slot is a string");
    let target = state
        .path()
        .join("slots")
        .join(slot)
        .join("targets/registry-v3.json");
    fs::write(&target, b"tampered installed target").expect("target is corrupted");

    assert!(
        equalturn_registry::load_installed_registry(
            state.path(),
            &bootstrap_root,
            &pin(&bootstrap_root),
            now(),
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn installed_snapshot_preserves_consistent_snapshot_filenames() {
    let signer = Signer::generate();
    let mut root = root_signed(&signer, 1);
    root["consistent_snapshot"] = json!(true);
    let bootstrap_root = envelope(&root, &[&signer]);
    let repository = tempfile::tempdir().expect("repository directory is created");
    let files = signed_repository_files(&signer, REGISTRY_BYTES, 1, FAR_FUTURE);
    write_repository(repository.path(), &files, REGISTRY_BYTES);
    fs::rename(
        repository.path().join("metadata/snapshot.json"),
        repository.path().join("metadata/1.snapshot.json"),
    )
    .expect("snapshot metadata receives a consistent name");
    fs::rename(
        repository.path().join("metadata/targets.json"),
        repository.path().join("metadata/1.targets.json"),
    )
    .expect("targets metadata receives a consistent name");
    fs::rename(
        repository.path().join("targets/registry-v3.json"),
        repository.path().join(format!(
            "targets/{}.registry-v3.json",
            sha256_hex(REGISTRY_BYTES)
        )),
    )
    .expect("target receives its signed hash prefix");
    let state = tempfile::tempdir().expect("installed state directory is created");

    equalturn_registry::update_directory_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        state.path(),
        now(),
    )
    .await
    .expect("consistent-snapshot repository installs");
    let reopened = equalturn_registry::load_installed_registry(
        state.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        now(),
    )
    .await
    .expect("versioned names remain valid for offline verification");
    assert_eq!(reopened.target_sha256(), &sha256(REGISTRY_BYTES));
}

#[tokio::test]
async fn installed_snapshot_preserves_the_authorized_root_rotation_chain() {
    let old_signer = Signer::generate();
    let new_signer = Signer::generate();
    let bootstrap_root = root_bytes(&old_signer);
    let repository = tempfile::tempdir().expect("repository directory is created");
    let mut files = signed_repository_files(&new_signer, REGISTRY_BYTES, 2, FAR_FUTURE);
    let next_root = root_signed(&new_signer, 2);
    files.push((
        "2.root.json".into(),
        envelope(&next_root, &[&old_signer, &new_signer]),
    ));
    write_repository(repository.path(), &files, REGISTRY_BYTES);
    let state = tempfile::tempdir().expect("installed state directory is created");

    let installed = equalturn_registry::update_directory_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        state.path(),
        now(),
    )
    .await
    .expect("authorized root rotation installs");
    assert_eq!(installed.trusted_root_version(), 2);
    let reopened = equalturn_registry::load_installed_registry(
        state.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        now(),
    )
    .await
    .expect("offline repository replays authorized root chain");
    assert_eq!(reopened.trusted_root_version(), 2);
}

#[cfg(unix)]
#[tokio::test]
async fn installed_snapshot_rejects_pointer_and_metadata_symlinks() {
    use std::os::unix::fs::symlink;

    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let state = tempfile::tempdir().expect("installed state directory is created");
    equalturn_registry::update_directory_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        state.path(),
        now(),
    )
    .await
    .expect("valid repository installs");

    let pointer = state.path().join("current.json");
    let pointer_bytes = fs::read(&pointer).expect("pointer is readable");
    let outside = state.path().join("outside");
    fs::write(&outside, &pointer_bytes).expect("outside file is written");
    fs::remove_file(&pointer).expect("pointer is removed");
    symlink(&outside, &pointer).expect("pointer symlink is created");
    assert!(
        equalturn_registry::load_installed_registry(
            state.path(),
            &bootstrap_root,
            &pin(&bootstrap_root),
            now(),
        )
        .await
        .is_err()
    );

    fs::remove_file(&pointer).expect("pointer symlink is removed");
    fs::write(&pointer, pointer_bytes).expect("valid pointer is restored");
    let active: Value =
        serde_json::from_slice(&fs::read(&pointer).expect("pointer is readable after restoration"))
            .expect("pointer parses");
    let slot = active["slot"].as_str().expect("slot is a string");
    let metadata = state
        .path()
        .join("slots")
        .join(slot)
        .join("metadata/timestamp.json");
    let outside_metadata = state.path().join("outside-timestamp");
    fs::write(
        &outside_metadata,
        fs::read(&metadata).expect("metadata is readable"),
    )
    .expect("outside metadata is written");
    fs::remove_file(&metadata).expect("installed timestamp is removed");
    symlink(&outside_metadata, &metadata).expect("metadata symlink is created");
    assert!(
        equalturn_registry::load_installed_registry(
            state.path(),
            &bootstrap_root,
            &pin(&bootstrap_root),
            now(),
        )
        .await
        .is_err()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn installed_snapshot_rejects_symlink_store_root() {
    use std::os::unix::fs::symlink;

    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let parent = tempfile::tempdir().expect("test parent is created");
    let actual = parent.path().join("actual");
    fs::create_dir(&actual).expect("actual directory is created");
    let link = parent.path().join("link");
    symlink(&actual, &link).expect("state symlink is created");

    assert!(
        equalturn_registry::update_directory_repository(
            repository.path(),
            &bootstrap_root,
            &pin(&bootstrap_root),
            &link,
            now(),
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn rejects_target_bytes_that_do_not_match_signed_target_metadata() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    fs::write(
        repository.path().join("targets/registry-v3.json"),
        b"tampered registry target",
    )
    .expect("test mutates target after metadata was signed");
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("target hash mismatch must fail");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidTarget
    );
}

#[tokio::test]
async fn rejects_signed_rollback_after_a_newer_version_was_cached() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let cache = tempfile::tempdir().expect("cache directory is created");
    let current = temporary_repository(&signer, REGISTRY_BYTES, 2);
    verify_repository(
        current.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .expect("newer signed version is accepted");

    let rollback = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let error = verify_repository(
        rollback.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("older signed version must not pass the persisted floor");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::Rollback
    );
}

#[tokio::test]
async fn rejects_expired_metadata_without_waiting_for_wall_clock_time() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = tempfile::tempdir().expect("repository directory is created");
    let files = signed_repository_files(&signer, REGISTRY_BYTES, 1, "2000-01-01T00:00:00Z");
    write_repository(repository.path(), &files, REGISTRY_BYTES);
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("expired timestamp must fail at the injected time");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::Expired
    );
}

#[tokio::test]
async fn accepts_a_root_rotation_authorized_by_both_old_and_new_roots() {
    let old_signer = Signer::generate();
    let new_signer = Signer::generate();
    let bootstrap_root = root_bytes(&old_signer);
    let repository = tempfile::tempdir().expect("repository directory is created");
    let mut files = signed_repository_files(&new_signer, REGISTRY_BYTES, 2, FAR_FUTURE);
    let next_root = root_signed(&new_signer, 2);
    let next_root_bytes = envelope(&next_root, &[&old_signer, &new_signer]);
    files.push(("2.root.json".into(), next_root_bytes));
    write_repository(repository.path(), &files, REGISTRY_BYTES);
    let cache = tempfile::tempdir().expect("cache directory is created");

    let verified = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .expect("dual-authorized root rotation is accepted");

    assert_eq!(verified.trusted_root_version(), 2);
}

#[tokio::test]
async fn targets_key_rotation_accepts_new_key_and_rejects_retired_key() {
    let old_root_and_targets = Signer::generate();
    let new_root = Signer::generate();
    let new_targets = Signer::generate();
    let bootstrap_root = root_bytes(&old_root_and_targets);
    let next_root = root_signed_with_roles(
        &new_root,
        &old_root_and_targets,
        &old_root_and_targets,
        &new_targets,
        2,
    );
    let next_root_envelope = envelope(&next_root, &[&old_root_and_targets, &new_root]);
    let repository = tempfile::tempdir().expect("repository directory is created");
    let mut files = signed_repository_files_with_roles(
        &new_targets,
        &old_root_and_targets,
        &old_root_and_targets,
        REGISTRY_BYTES,
        2,
        FAR_FUTURE,
    );
    files.push(("2.root.json".into(), next_root_envelope.clone()));
    write_repository(repository.path(), &files, REGISTRY_BYTES);
    let cache = tempfile::tempdir().expect("metadata cache is created");

    let verified = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .expect("targets key authorized by the rotated root is accepted");
    assert_eq!(verified.trusted_root_version(), 2);

    let retired_key_repository = tempfile::tempdir().expect("repository directory is created");
    let mut files = signed_repository_files_with_roles(
        &old_root_and_targets,
        &old_root_and_targets,
        &old_root_and_targets,
        REGISTRY_BYTES,
        3,
        FAR_FUTURE,
    );
    files.push(("2.root.json".into(), next_root_envelope));
    write_repository(retired_key_repository.path(), &files, REGISTRY_BYTES);

    let error = verify_repository(
        retired_key_repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("retired targets key must not authorize a new release");
    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidSignature
    );
}

#[tokio::test]
async fn rejects_an_unauthorized_remote_root_rotation() {
    let trusted_signer = Signer::generate();
    let attacker_signer = Signer::generate();
    let bootstrap_root = root_bytes(&trusted_signer);
    let repository = temporary_repository(&trusted_signer, REGISTRY_BYTES, 1);
    let attacker_root = root_signed(&attacker_signer, 2);
    fs::write(
        repository.path().join("metadata/2.root.json"),
        envelope(&attacker_root, &[&attacker_signer]),
    )
    .expect("attacker root is placed in repository");
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("untrusted root is not authorized by the bootstrap root");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidRoot
    );
}

#[tokio::test]
async fn rejects_mix_and_match_snapshot_metadata() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let first = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let second = temporary_repository(&signer, b"different but valid target bytes", 1);
    fs::copy(
        second.path().join("metadata/snapshot.json"),
        first.path().join("metadata/snapshot.json"),
    )
    .expect("test mixes separately signed repository metadata");
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        first.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("timestamp pin must reject a snapshot from another repository state");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidMetadata
    );
}

#[tokio::test]
async fn rejects_malformed_metadata_safely() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    fs::write(
        repository.path().join("metadata/timestamp.json"),
        b"{not json",
    )
    .expect("test replaces timestamp with malformed bytes");
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("malformed signed metadata must fail closed");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidMetadata
    );
}

#[tokio::test]
async fn rejects_missing_required_metadata() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    fs::remove_file(repository.path().join("metadata/timestamp.json"))
        .expect("test removes required timestamp metadata");
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("missing required metadata must fail closed");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidMetadata
    );
}

#[tokio::test]
async fn rejects_a_malformed_pinned_bootstrap_root() {
    let signer = Signer::generate();
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let malformed_root = b"{malformed pinned root";
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        malformed_root,
        &pin(malformed_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("a matching pin cannot make malformed root metadata valid");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidRoot
    );
}

#[tokio::test]
async fn rejects_a_bootstrap_root_with_malformed_public_key_material() {
    let signer = Signer::generate();
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let mut signed = root_signed(&signer, 1);
    signed["keys"][&signer.id]["keyval"]["public"] = json!("not a PEM public key");
    let malformed_key_root = envelope(&signed, &[&signer]);
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &malformed_key_root,
        &pin(&malformed_key_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("malformed pinned public key material must fail root parsing");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidRoot
    );
}

#[tokio::test]
async fn rejects_invalid_timestamp_signatures() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 1);
    let timestamp_path = repository.path().join("metadata/timestamp.json");
    let mut timestamp: Value =
        serde_json::from_slice(&fs::read(&timestamp_path).expect("timestamp metadata is readable"))
            .expect("timestamp metadata parses for mutation");
    timestamp["signatures"][0]["sig"] = json!("00".repeat(64));
    fs::write(
        &timestamp_path,
        serde_json::to_vec(&timestamp).expect("mutated metadata serializes"),
    )
    .expect("mutated metadata replaces original");
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("invalid timestamp signature must fail verification");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidSignature
    );
}

#[tokio::test]
async fn rejects_an_oversized_registry_target_at_the_transport_boundary() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let oversized_target = vec![b' '; equalturn_registry::REGISTRY_MAX_BYTES + 1];
    let repository = temporary_repository(&signer, &oversized_target, 1);
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("target transport limit must reject an oversized signed target");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidTarget
    );
}

#[tokio::test]
async fn attacker_cannot_replace_remote_root_and_all_metadata_after_bootstrap() {
    let trusted_signer = Signer::generate();
    let attacker_signer = Signer::generate();
    let trusted_root = root_bytes(&trusted_signer);
    let repository = temporary_repository(&attacker_signer, REGISTRY_BYTES, 1);
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &trusted_root,
        &pin(&trusted_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("attacker metadata cannot replace the independently pinned bootstrap root");

    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidSignature
    );
}

#[tokio::test]
async fn rejects_a_validly_signed_but_semantically_invalid_registry_target() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let invalid_registry = br#"{"schema_version":3,"registry_version":"1.0.0","last_updated":"2026-10-19","purpose":"research_only","entries":[],"unexpected":true}"#;
    let repository = temporary_repository(&signer, invalid_registry, 1);
    let cache = tempfile::tempdir().expect("cache directory is created");

    let error = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        cache.path(),
        now(),
    )
    .await
    .require_error("TUF authenticity does not bypass registry semantic validation");

    assert!(matches!(&error, RegistryError::Schema(_)));
    assert_eq!(
        error.verification_status(),
        RegistryVerificationStatus::InvalidRegistry
    );
}

#[tokio::test]
async fn rejects_tampered_cached_metadata_instead_of_using_it_as_a_floor() {
    let signer = Signer::generate();
    let bootstrap_root = root_bytes(&signer);
    let repository = temporary_repository(&signer, REGISTRY_BYTES, 2);
    let cache = tempfile::tempdir().expect("cache directory is created");
    let trusted_cache = cache.path().join("trusted");
    verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        &trusted_cache,
        now(),
    )
    .await
    .expect("signed metadata is cached");
    fs::write(trusted_cache.join("timestamp.json"), b"tampered cache")
        .expect("test corrupts local metadata cache");

    let retry = verify_repository(
        repository.path(),
        &bootstrap_root,
        &pin(&bootstrap_root),
        &trusted_cache,
        now(),
    )
    .await;

    assert!(
        retry.is_ok(),
        "fresh signed repository metadata repairs the cache"
    );
}
