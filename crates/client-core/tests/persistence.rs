use equalturn_client_core::{ClientError, ClientStore, SessionProfile};
use equalturn_protocol::{
    EndReason, EventPayload, ParticipantId, PublicKey, RelayReceipt, SessionId, SigningIdentity,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("equalturn-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("create temporary test directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("restrict temporary test directory");
        }
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn profile(
    session_id: SessionId,
    participant_id: ParticipantId,
    key: PublicKey,
    relay_key: PublicKey,
) -> SessionProfile {
    SessionProfile {
        session_id,
        participant_id,
        nickname: "voldigoade".to_owned(),
        participant_public_key: key,
        relay_public_key: relay_key,
    }
}

async fn setup() -> (
    TempDir,
    ClientStore,
    SessionProfile,
    SigningIdentity,
    SigningIdentity,
) {
    let dir = TempDir::new();
    let store = ClientStore::open(dir.path()).await.expect("open store");
    let session_id = SessionId::try_new_v4().expect("session id");
    let participant_id = ParticipantId::try_new_v4().expect("participant id");
    let identity = store
        .create_identity(session_id, participant_id)
        .await
        .expect("create identity");
    let relay_identity = SigningIdentity::generate().expect("relay identity");
    let profile = profile(
        session_id,
        participant_id,
        identity.public_key(),
        relay_identity.public_key(),
    );
    store.save_profile(&profile).await.expect("save profile");
    (dir, store, profile, identity, relay_identity)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_millis() as u64
}

#[tokio::test]
async fn event_allocation_is_transactional_and_resends_in_sequence_order() {
    let (_dir, store, profile, identity, _) = setup().await;

    let first = store
        .enqueue_event(&profile, &identity, now_ms(), EventPayload::SessionStarted)
        .await
        .expect("queue first event");
    let second = store
        .enqueue_event(
            &profile,
            &identity,
            now_ms(),
            EventPayload::SyntheticAiAccessAttempt {
                target: Some("chatgpt.com".to_owned()),
            },
        )
        .await
        .expect("queue second event");

    assert_eq!(first.body.sequence, 1);
    assert_eq!(second.body.sequence, 2);
    assert_eq!(
        second.body.previous_hash,
        first.event_hash().expect("first hash")
    );
    assert_eq!(
        store
            .pending_events(profile.session_id, profile.participant_id)
            .await
            .expect("pending"),
        vec![first, second]
    );
}

#[tokio::test]
async fn rejected_payload_does_not_consume_a_sequence_number() {
    let (_dir, store, profile, identity, _) = setup().await;
    let rejected = store
        .enqueue_event(
            &profile,
            &identity,
            now_ms(),
            EventPayload::SyntheticAiAccessAttempt {
                target: Some("https://chatgpt.com/".to_owned()),
            },
        )
        .await;
    assert!(rejected.is_err());

    let accepted = store
        .enqueue_event(&profile, &identity, now_ms(), EventPayload::SessionStarted)
        .await
        .expect("queue first valid event");
    assert_eq!(accepted.body.sequence, 1);
}

#[tokio::test]
async fn identity_profile_and_chain_resume_after_reopening_the_store() {
    let (dir, store, profile, _, _) = setup().await;
    let identity = store
        .load_identity(profile.session_id, profile.participant_id)
        .await
        .expect("load stored identity");
    let first = store
        .enqueue_event(&profile, &identity, now_ms(), EventPayload::SessionStarted)
        .await
        .expect("queue first event");
    drop(store);

    let reopened = ClientStore::open(dir.path()).await.expect("reopen store");
    assert_eq!(
        reopened
            .load_profile(profile.session_id, profile.participant_id)
            .await
            .expect("profile"),
        Some(profile.clone())
    );
    let identity = reopened
        .load_identity(profile.session_id, profile.participant_id)
        .await
        .expect("restore session identity");
    let second = reopened
        .enqueue_event(
            &profile,
            &identity,
            now_ms(),
            EventPayload::SyntheticAiAccessAttempt {
                target: Some("claude.ai".to_owned()),
            },
        )
        .await
        .expect("queue after restart");

    assert_eq!(second.body.sequence, 2);
    assert_eq!(
        second.body.previous_hash,
        first.event_hash().expect("first hash")
    );
    assert_eq!(
        reopened
            .pending_events(profile.session_id, profile.participant_id)
            .await
            .expect("ordered pending events"),
        vec![first, second]
    );
}

#[tokio::test]
async fn relay_receipt_is_validated_and_acknowledgement_is_durable_and_idempotent() {
    let (dir, store, profile, identity, relay_identity) = setup().await;
    let event = store
        .enqueue_event(&profile, &identity, now_ms(), EventPayload::SessionStarted)
        .await
        .expect("queue event");
    let receipt = RelayReceipt::sign(
        profile.session_id,
        profile.participant_id,
        event.body.event_id,
        event.event_hash().expect("event hash"),
        1,
        now_ms(),
        &relay_identity,
    )
    .expect("sign receipt");

    store.acknowledge(&receipt).await.expect("acknowledge");
    store
        .acknowledge(&receipt)
        .await
        .expect("repeat exact acknowledgement");
    assert!(
        store
            .pending_events(profile.session_id, profile.participant_id)
            .await
            .expect("pending")
            .is_empty()
    );

    drop(store);
    let reopened = ClientStore::open(dir.path()).await.expect("reopen store");
    assert!(
        reopened
            .pending_events(profile.session_id, profile.participant_id)
            .await
            .expect("pending after restart")
            .is_empty()
    );
}

#[tokio::test]
async fn invalid_or_conflicting_receipts_do_not_acknowledge_events() {
    let (_dir, store, profile, identity, relay_identity) = setup().await;
    let event = store
        .enqueue_event(&profile, &identity, now_ms(), EventPayload::SessionStarted)
        .await
        .expect("queue event");
    let receipt = RelayReceipt::sign(
        profile.session_id,
        profile.participant_id,
        event.body.event_id,
        event.event_hash().expect("event hash"),
        1,
        now_ms(),
        &relay_identity,
    )
    .expect("sign receipt");

    let mut wrong_session = receipt.clone();
    wrong_session.session_id = SessionId::try_new_v4().expect("session id");
    assert!(store.acknowledge(&wrong_session).await.is_err());

    let wrong_relay = SigningIdentity::generate().expect("wrong relay identity");
    let wrong_signature = RelayReceipt::sign(
        profile.session_id,
        profile.participant_id,
        event.body.event_id,
        event.event_hash().expect("event hash"),
        1,
        now_ms(),
        &wrong_relay,
    )
    .expect("sign mismatched receipt");
    assert!(store.acknowledge(&wrong_signature).await.is_err());
    assert_eq!(
        store
            .pending_events(profile.session_id, profile.participant_id)
            .await
            .expect("pending")
            .len(),
        1
    );
}

#[tokio::test]
async fn corrupt_database_fails_with_explicit_error() {
    let dir = TempDir::new();
    fs::write(dir.path().join("client.sqlite3"), b"not a sqlite database")
        .expect("write corrupt db");
    let result = ClientStore::open(dir.path()).await;
    assert!(matches!(result, Err(ClientError::CorruptDatabase)));
}

#[tokio::test]
async fn deleted_database_with_a_surviving_key_is_not_reinitialized() {
    let (dir, store, _profile, _, _) = setup().await;
    store.close().await;
    fs::remove_file(dir.path().join("client.sqlite3")).expect("remove database");
    let result = ClientStore::open(dir.path()).await;
    assert!(matches!(result, Err(ClientError::DatabaseLost)));
}

#[tokio::test]
async fn missing_key_is_reported_instead_of_regenerating_identity() {
    let (dir, store, profile, _, _) = setup().await;
    drop(store);
    fs::remove_file(dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    )))
    .expect("remove identity key");

    let reopened = ClientStore::open(dir.path()).await.expect("reopen store");
    assert!(matches!(
        reopened
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::LostKey)
    ));
}

#[tokio::test]
async fn session_key_uses_a_versioned_platform_protection_envelope() {
    let (dir, store, profile, identity, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let bytes = fs::read(&key_path).expect("read session key envelope");

    assert_eq!(&bytes[..8], b"ETKEY\0\0\0");
    assert_eq!(u16::from_be_bytes([bytes[8], bytes[9]]), 2);
    #[cfg(windows)]
    assert_eq!(bytes[10], 2, "Windows keys use current-user DPAPI");
    #[cfg(unix)]
    assert_eq!(bytes[10], 1, "Unix keys use owner-only file permissions");

    let restored = store
        .load_identity(profile.session_id, profile.participant_id)
        .await
        .expect("load versioned session identity");
    assert_eq!(restored.public_key(), identity.public_key());
}

#[tokio::test]
async fn unsupported_session_key_envelope_version_fails_closed() {
    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let mut bytes = fs::read(&key_path).expect("read session key envelope");
    bytes[8..10].copy_from_slice(&u16::MAX.to_be_bytes());
    fs::write(&key_path, bytes).expect("write unsupported key version");

    assert!(matches!(
        store
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::CorruptKey)
    ));
}

#[tokio::test]
async fn truncated_session_key_envelope_fails_closed() {
    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let bytes = fs::read(&key_path).expect("read session key envelope");
    fs::write(&key_path, &bytes[..bytes.len() - 1]).expect("truncate session key envelope");

    assert!(matches!(
        store
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::CorruptKey)
    ));
}

#[tokio::test]
async fn session_key_store_binding_mismatch_is_reported() {
    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let mut bytes = fs::read(&key_path).expect("read session key envelope");
    bytes[43] ^= 0x80;
    fs::write(&key_path, bytes).expect("change local store binding");

    assert!(matches!(
        store
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::IdentityDiscontinuity)
    ));
}

#[tokio::test]
async fn legacy_plaintext_key_remains_readable_for_existing_sessions() {
    use zeroize::Zeroize;

    let (dir, store, profile, identity, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let mut seed = identity.seed_bytes();
    let current = fs::read(&key_path).expect("read v2 key for local store binding");
    let mut legacy = Vec::new();
    legacy.extend_from_slice(b"ETKEY\0\0\x01");
    legacy.extend_from_slice(profile.session_id.as_bytes());
    legacy.extend_from_slice(profile.participant_id.as_bytes());
    legacy.extend_from_slice(&current[43..59]);
    legacy.extend_from_slice(&seed);
    seed.zeroize();
    fs::write(&key_path, legacy).expect("write legacy key file");

    let restored = store
        .load_identity(profile.session_id, profile.participant_id)
        .await
        .expect("load existing legacy session identity");
    assert_eq!(restored.public_key(), identity.public_key());
}

#[cfg(unix)]
#[tokio::test]
async fn session_key_symlink_is_rejected_without_opening_its_target() {
    use std::os::unix::fs::symlink;

    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let target_path = dir.path().join("saved-key");
    fs::rename(&key_path, &target_path).expect("move key out of the expected name");
    symlink(&target_path, &key_path).expect("replace expected key with symlink");

    assert!(matches!(
        store
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::CorruptKey)
    ));
    assert!(target_path.exists(), "the symlink target remains untouched");
}

#[cfg(unix)]
#[tokio::test]
async fn non_regular_session_key_file_is_rejected() {
    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    fs::remove_file(&key_path).expect("remove regular key file");
    fs::create_dir(&key_path).expect("create directory at key path");

    assert!(matches!(
        store
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::CorruptKey)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn oversized_session_key_file_is_rejected_before_reading_payload() {
    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let oversized = vec![0_u8; 8 + 2 + 1 + 16 + 16 + 16 + 16 * 1024 + 1];
    fs::write(&key_path, oversized).expect("replace key with an oversized file");

    assert!(matches!(
        store
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::CorruptKey)
    ));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_create_new_calls_never_replace_the_winning_identity() {
    let dir = TempDir::new();
    let store = std::sync::Arc::new(ClientStore::open(dir.path()).await.expect("open store"));
    let session_id = SessionId::try_new_v4().expect("session id");
    let participant_id = ParticipantId::try_new_v4().expect("participant id");
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let runtime = tokio::runtime::Handle::current();
    let first_store = std::sync::Arc::clone(&store);
    let first_gate = std::sync::Arc::clone(&gate);
    let first_runtime = runtime.clone();
    let first = tokio::task::spawn_blocking(move || {
        first_gate.wait();
        first_runtime.block_on(first_store.create_identity(session_id, participant_id))
    });
    let second_store = std::sync::Arc::clone(&store);
    let second_gate = std::sync::Arc::clone(&gate);
    let second = tokio::task::spawn_blocking(move || {
        second_gate.wait();
        runtime.block_on(second_store.create_identity(session_id, participant_id))
    });
    let first = first.await.expect("first creation task");
    let second = second.await.expect("second creation task");

    let winner = match (first, second) {
        (Ok(identity), Err(ClientError::ProfileConflict))
        | (Err(ClientError::ProfileConflict), Ok(identity)) => identity,
        _ => panic!("create_new returned an unexpected race result"),
    };
    let restored = store
        .load_identity(session_id, participant_id)
        .await
        .expect("load winning create_new identity");
    assert_eq!(restored.public_key(), winner.public_key());
}

#[cfg(unix)]
#[tokio::test]
async fn data_directory_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let parent = TempDir::new();
    let actual_directory = parent.path().join("actual");
    fs::create_dir(&actual_directory).expect("create actual data directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&actual_directory, fs::Permissions::from_mode(0o700))
            .expect("restrict actual data directory");
    }
    let symlink_path = parent.path().join("linked-store");
    symlink(&actual_directory, &symlink_path).expect("create data-directory symlink");

    assert!(matches!(
        ClientStore::open(&symlink_path).await,
        Err(ClientError::InsecureDataDirectory)
    ));
    let trailing_separator = PathBuf::from(format!("{}/", symlink_path.display()));
    assert!(matches!(
        ClientStore::open(&trailing_separator).await,
        Err(ClientError::InsecureDataDirectory)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn key_creation_uses_the_opened_data_directory_after_path_replacement() {
    use std::os::unix::fs::symlink;

    let parent = TempDir::new();
    let data_path = parent.path().join("store");
    fs::create_dir(&data_path).expect("create data directory");
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&data_path, fs::Permissions::from_mode(0o700))
            .expect("restrict data directory");
    }
    let store = ClientStore::open(&data_path).await.expect("open store");
    let moved_path = parent.path().join("opened-store");
    let replacement_path = parent.path().join("replacement-store");
    fs::create_dir(&replacement_path).expect("create replacement directory");
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&replacement_path, fs::Permissions::from_mode(0o700))
            .expect("restrict replacement directory");
    }
    fs::rename(&data_path, &moved_path).expect("move opened directory");
    symlink(&replacement_path, &data_path).expect("replace original path with symlink");

    let session_id = SessionId::try_new_v4().expect("session id");
    let participant_id = ParticipantId::try_new_v4().expect("participant id");
    store
        .create_identity(session_id, participant_id)
        .await
        .expect("create through the pinned directory descriptor");

    let expected = moved_path.join("identities").join(format!(
        "{}-{}.key",
        session_id.as_uuid(),
        participant_id.as_uuid()
    ));
    assert!(expected.is_file());
    assert!(!replacement_path.join("identities").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn identity_directory_symlink_is_rejected() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let dir = TempDir::new();
    let store = ClientStore::open(dir.path()).await.expect("open store");
    let outside = dir.path().join("outside-identities");
    fs::create_dir(&outside).expect("create target directory");
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o700))
        .expect("restrict target directory");
    symlink(&outside, dir.path().join("identities")).expect("create identity-directory symlink");

    assert!(matches!(
        store
            .create_identity(
                SessionId::try_new_v4().expect("session id"),
                ParticipantId::try_new_v4().expect("participant id")
            )
            .await,
        Err(ClientError::InsecureKeyPermissions)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn identity_directory_with_group_access_is_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let store = ClientStore::open(dir.path()).await.expect("open store");
    let identities = dir.path().join("identities");
    fs::create_dir(&identities).expect("create identity directory");
    fs::set_permissions(&identities, fs::Permissions::from_mode(0o750))
        .expect("broaden identity directory permissions");

    assert!(matches!(
        store
            .create_identity(
                SessionId::try_new_v4().expect("session id"),
                ParticipantId::try_new_v4().expect("participant id")
            )
            .await,
        Err(ClientError::InsecureKeyPermissions)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn lost_database_check_does_not_follow_identity_directory_symlink() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let (dir, store, _, _, _) = setup().await;
    store.close().await;
    fs::remove_file(dir.path().join("client.sqlite3")).expect("remove client database");
    fs::rename(
        dir.path().join("identities"),
        dir.path().join("saved-identities"),
    )
    .expect("move original identities directory");
    let outside = dir.path().join("outside-identities");
    fs::create_dir(&outside).expect("create outside directory");
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o700))
        .expect("restrict outside directory");
    symlink(&outside, dir.path().join("identities")).expect("link identities to outside");

    assert!(matches!(
        ClientStore::open(dir.path()).await,
        Err(ClientError::InsecureKeyPermissions)
    ));
}

#[cfg(windows)]
#[tokio::test]
async fn windows_dpapi_key_file_hides_seed_and_survives_store_reopen() {
    use zeroize::Zeroize;

    let (dir, store, profile, identity, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let bytes = fs::read(&key_path).expect("read DPAPI key envelope");
    let mut seed = identity.seed_bytes();
    assert_eq!(bytes[10], 2, "key uses current-user DPAPI protection");
    assert!(
        !bytes.windows(seed.len()).any(|window| window == seed),
        "raw Ed25519 seed must not appear in the persisted file"
    );
    seed.zeroize();

    store.close().await;
    let reopened = ClientStore::open(dir.path()).await.expect("reopen store");
    let restored = reopened
        .load_identity(profile.session_id, profile.participant_id)
        .await
        .expect("restore DPAPI identity under same test user");
    assert_eq!(restored.public_key(), identity.public_key());
}

#[cfg(windows)]
#[tokio::test]
async fn windows_dpapi_corrupted_ciphertext_is_rejected() {
    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let mut bytes = fs::read(&key_path).expect("read DPAPI key envelope");
    let protected_payload_byte = 59 + (bytes.len() - 59) / 2;
    bytes[protected_payload_byte] ^= 0x80;
    fs::write(&key_path, bytes).expect("write corrupted DPAPI payload");

    assert!(matches!(
        store
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::CorruptKey)
    ));
}

#[cfg(windows)]
#[tokio::test]
async fn windows_dpapi_truncated_key_file_is_rejected() {
    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let bytes = fs::read(&key_path).expect("read DPAPI key envelope");
    fs::write(&key_path, &bytes[..bytes.len() - 1]).expect("truncate DPAPI key file");

    assert!(matches!(
        store
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::CorruptKey)
    ));
}

#[cfg(windows)]
#[tokio::test]
async fn windows_dpapi_key_is_bound_to_its_session_header() {
    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let mut bytes = fs::read(&key_path).expect("read DPAPI key envelope");
    let other_session_id = SessionId::try_new_v4().expect("other session ID");
    bytes[11..27].copy_from_slice(other_session_id.as_bytes());
    let other_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        other_session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    fs::write(&other_path, bytes).expect("write key under changed session binding");

    assert!(matches!(
        store
            .load_identity(other_session_id, profile.participant_id)
            .await,
        Err(ClientError::CorruptKey)
    ));
}

#[cfg(windows)]
#[tokio::test]
async fn windows_upgrades_legacy_plaintext_key_before_returning_identity() {
    use zeroize::Zeroize;

    let (dir, store, profile, identity, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    let mut seed = identity.seed_bytes();
    let mut legacy = Vec::new();
    legacy.extend_from_slice(b"ETKEY\0\0\x01");
    legacy.extend_from_slice(profile.session_id.as_bytes());
    legacy.extend_from_slice(profile.participant_id.as_bytes());
    legacy.extend_from_slice(&[0_u8; 16]);
    // Obtain the local store binding from the existing v2 header.
    let current = fs::read(&key_path).expect("read current key");
    legacy[40..56].copy_from_slice(&current[43..59]);
    legacy.extend_from_slice(&seed);
    seed.zeroize();
    fs::write(&key_path, legacy).expect("write legacy version 1 key");

    let restored = store
        .load_identity(profile.session_id, profile.participant_id)
        .await
        .expect("load and upgrade legacy key");
    assert_eq!(restored.public_key(), identity.public_key());
    let upgraded = fs::read(&key_path).expect("read upgraded key");
    assert_eq!(&upgraded[..8], b"ETKEY\0\0\0");
    assert_eq!(u16::from_be_bytes([upgraded[8], upgraded[9]]), 2);
    assert_eq!(upgraded[10], 2);
    let mut seed = identity.seed_bytes();
    let seed_is_present = upgraded
        .windows(seed.len())
        .any(|window| window == seed.as_slice());
    seed.zeroize();
    assert!(!seed_is_present);
}

#[tokio::test]
async fn rejoining_after_key_loss_keeps_prior_participant_evidence() {
    let (dir, store, old_profile, old_identity, relay_identity) = setup().await;
    let old_event = store
        .enqueue_event(
            &old_profile,
            &old_identity,
            now_ms(),
            EventPayload::SessionStarted,
        )
        .await
        .expect("queue prior participant event");
    let old_key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        old_profile.session_id.as_uuid(),
        old_profile.participant_id.as_uuid()
    ));
    fs::remove_file(old_key_path).expect("remove lost participant key");
    assert!(matches!(
        store
            .load_identity(old_profile.session_id, old_profile.participant_id)
            .await,
        Err(ClientError::LostKey)
    ));

    let new_participant_id = ParticipantId::try_new_v4().expect("new participant id");
    let new_identity = store
        .create_identity(old_profile.session_id, new_participant_id)
        .await
        .expect("create new participant identity");
    let new_profile = profile(
        old_profile.session_id,
        new_participant_id,
        new_identity.public_key(),
        relay_identity.public_key(),
    );
    store
        .save_profile(&new_profile)
        .await
        .expect("save new participant profile");

    assert_eq!(
        store
            .load_profile(old_profile.session_id, old_profile.participant_id)
            .await
            .expect("old profile remains available"),
        Some(old_profile.clone())
    );
    assert_eq!(
        store
            .load_profile(new_profile.session_id, new_profile.participant_id)
            .await
            .expect("new profile is available"),
        Some(new_profile.clone())
    );
    assert_eq!(
        store
            .pending_events(old_profile.session_id, old_profile.participant_id)
            .await
            .expect("old evidence remains available"),
        vec![old_event]
    );
    let new_event = store
        .enqueue_event(
            &new_profile,
            &new_identity,
            now_ms(),
            EventPayload::SessionEnded {
                reason: EndReason::ParticipantEnded,
            },
        )
        .await
        .expect("new participant has independent chain");
    assert_eq!(new_event.body.sequence, 1);
}

#[cfg(unix)]
#[tokio::test]
async fn missing_data_directory_is_created_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let parent = TempDir::new();
    let data_dir = parent.path().join("new-store");
    let _store = ClientStore::open(&data_dir)
        .await
        .expect("create a private data directory");
    let mode = fs::metadata(&data_dir)
        .expect("data directory metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
}

#[cfg(unix)]
#[tokio::test]
async fn session_key_is_created_with_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let store = ClientStore::open(dir.path()).await.expect("open store");
    let session_id = SessionId::try_new_v4().expect("session id");
    let participant_id = ParticipantId::try_new_v4().expect("participant id");
    let _identity = store
        .create_identity(session_id, participant_id)
        .await
        .expect("create identity");
    let mode = fs::metadata(dir.path().join("identities").join(format!(
        "{}-{}.key",
        session_id.as_uuid(),
        participant_id.as_uuid()
    )))
    .expect("key metadata")
    .permissions()
    .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(unix)]
#[tokio::test]
async fn key_with_broader_permissions_is_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let (dir, store, profile, _, _) = setup().await;
    let key_path = dir.path().join("identities").join(format!(
        "{}-{}.key",
        profile.session_id.as_uuid(),
        profile.participant_id.as_uuid()
    ));
    fs::set_permissions(&key_path, fs::Permissions::from_mode(0o644)).expect("loosen key mode");
    assert!(matches!(
        store
            .load_identity(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::InsecureKeyPermissions)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn shared_data_directory_is_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755))
        .expect("make data directory shared");
    assert!(matches!(
        ClientStore::open(dir.path()).await,
        Err(ClientError::InsecureDataDirectory)
    ));
}

#[tokio::test]
async fn tampered_local_event_is_rejected_before_resend() {
    use sqlx::{
        Row,
        sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    };

    let (dir, store, profile, identity, _) = setup().await;
    store
        .enqueue_event(&profile, &identity, now_ms(), EventPayload::SessionStarted)
        .await
        .expect("queue event");
    drop(store);

    let options = SqliteConnectOptions::new()
        .filename(dir.path().join("client.sqlite3"))
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open test database");
    let event_id: String = sqlx::query("SELECT event_id FROM outbox LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("read event ID")
        .try_get(0)
        .expect("decode event ID");
    sqlx::query("UPDATE outbox SET event_json = ?1 WHERE event_id = ?2")
        .bind(b"{}".as_slice())
        .bind(event_id)
        .execute(&pool)
        .await
        .expect("tamper with stored event");
    pool.close().await;

    let reopened = ClientStore::open(dir.path()).await.expect("reopen store");
    assert!(matches!(
        reopened
            .pending_events(profile.session_id, profile.participant_id)
            .await,
        Err(ClientError::CorruptEvidence)
    ));
}
