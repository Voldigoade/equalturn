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
    drop(store);
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
