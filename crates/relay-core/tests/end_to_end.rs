use std::{
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use equalturn_client_core::RelayClientError;
use equalturn_client_core::{ClientStore, RelayClient, SessionProfile, SessionStream};
use equalturn_protocol::{
    ClientWsMessage, EventBody, EventId, EventPayload, Hash32, InvitationCode, LivenessState,
    OsFamily, ParticipantEvent, ParticipantId, PublicKey, ReconnectChallenge, RelayNoticeKind,
    ServerWsMessage, SessionId, SigningIdentity,
};
use equalturn_relay_core::{Relay, router};
use equalturn_report::{VerificationStatus, verify_report_json_with_relay_key};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use sqlx::{AssertSqlSafe, PgPool, postgres::PgPoolOptions};
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
    time::timeout,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use uuid::Uuid;

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn create() -> Self {
        let path = std::env::temp_dir().join(format!("equalturn-e2e-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temporary directory is created");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("temporary directory permissions are restricted");
        }
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct IsolatedDatabase {
    admin_pool: PgPool,
    database_url: String,
    schema: String,
}

impl IsolatedDatabase {
    async fn create() -> Option<Self> {
        let database_url = match std::env::var("TEST_DATABASE_URL") {
            Ok(value) => value,
            Err(_) => {
                eprintln!("skipping relay E2E: TEST_DATABASE_URL is not configured");
                return None;
            }
        };
        let admin_pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("test PostgreSQL service accepts connections");
        let schema = format!("equalturn_test_{}", Uuid::new_v4().simple());
        let create_schema = format!("CREATE SCHEMA \"{schema}\"");
        sqlx::query(AssertSqlSafe(create_schema))
            .execute(&admin_pool)
            .await
            .expect("isolated test schema is created");
        Some(Self {
            admin_pool,
            database_url,
            schema,
        })
    }

    async fn application_pool(&self) -> PgPool {
        let schema = self.schema.clone();
        PgPoolOptions::new()
            .max_connections(4)
            .after_connect(move |connection, _metadata| {
                let schema_name = schema.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path', $1, false)")
                        .bind(schema_name)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&self.database_url)
            .await
            .expect("isolated application pool connects")
    }

    async fn destroy(self) {
        let drop_schema = format!("DROP SCHEMA \"{}\" CASCADE", self.schema);
        sqlx::query(AssertSqlSafe(drop_schema))
            .execute(&self.admin_pool)
            .await
            .expect("isolated test schema is removed");
        self.admin_pool.close().await;
    }
}

#[tokio::test]
async fn clients_create_join_exchange_events_recover_and_verify_report() {
    let Some(database) = IsolatedDatabase::create().await else {
        return;
    };
    let test_directories = [
        TemporaryDirectory::create(),
        TemporaryDirectory::create(),
        TemporaryDirectory::create(),
    ];
    let stores = [
        ClientStore::open(test_directories[0].path())
            .await
            .expect("Alice client store opens"),
        ClientStore::open(test_directories[1].path())
            .await
            .expect("Bob client store opens"),
        ClientStore::open(test_directories[2].path())
            .await
            .expect("duplicate-nickname client store opens"),
    ];
    let relay_seed = [91; 32];
    let relay_identity = SigningIdentity::from_seed(relay_seed);
    let relay_public_key = relay_identity.public_key();
    let application_pool = database.application_pool().await;
    let relay = Relay::with_pool(application_pool.clone(), relay_identity)
        .await
        .expect("relay initializes and migrates the isolated schema");
    let (relay_url, server) = start_server(Arc::clone(&relay)).await;
    let client = RelayClient::new(&relay_url, true).expect("loopback relay client is valid");
    let (pinned_key, _) = client
        .relay_identity()
        .await
        .expect("relay identity is available");
    assert_eq!(pinned_key, relay_public_key);

    let http = reqwest::Client::new();
    let malformed_json = http
        .post(format!("{relay_url}/v1/invitations/resolve"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(vec![0xff, 0xfe])
        .send()
        .await
        .expect("malformed JSON reaches the relay");
    assert_eq!(malformed_json.status(), reqwest::StatusCode::BAD_REQUEST);
    let oversized_json = http
        .post(format!("{relay_url}/v1/invitations/resolve"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(vec![b' '; 32 * 1024 + 1])
        .send()
        .await
        .expect("oversized JSON reaches the relay");
    assert_eq!(
        oversized_json.status(),
        reqwest::StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(matches!(
        client.resolve_invitation("not-an-invitation").await,
        Err(RelayClientError::Rejected { status: 401, .. })
    ));

    let session_id = SessionId::try_new_v4().expect("session ID is generated");
    let alice_id = ParticipantId::try_new_v4().expect("Alice participant ID is generated");
    let alice_identity = stores[0]
        .create_identity(session_id, alice_id)
        .await
        .expect("Alice session key is generated");
    let alice_profile = profile(
        session_id,
        alice_id,
        "alice",
        alice_identity.public_key(),
        relay_public_key,
    );
    stores[0]
        .save_profile(&alice_profile)
        .await
        .expect("Alice profile is stored");
    let alice_join = stores[0]
        .enqueue_event(
            &alice_profile,
            &alice_identity,
            now_ms(),
            joined_payload("alice", alice_identity.public_key()),
        )
        .await
        .expect("Alice join event is signed and queued");
    let invitation = InvitationCode::generate().expect("invitation has cryptographic entropy");
    let created = client
        .create_session(
            &equalturn_protocol::CreateSessionRequest {
                protocol_version: 1,
                invitation_code: invitation.as_str().to_owned(),
                creator_event: alice_join.clone(),
            },
            &relay_public_key,
        )
        .await
        .expect("Alice creates a session");
    stores[0]
        .acknowledge(&created.receipt)
        .await
        .expect("Alice creation event receipt is persisted");

    sqlx::query("UPDATE sessions SET invite_expires_at_ms=1 WHERE session_id=$1")
        .bind(session_id.as_uuid())
        .execute(&application_pool)
        .await
        .expect("test fixture expires the invitation");
    assert!(matches!(
        client.resolve_invitation(invitation.as_str()).await,
        Err(RelayClientError::Rejected { status: 401, .. })
    ));
    sqlx::query("UPDATE sessions SET invite_expires_at_ms=$2 WHERE session_id=$1")
        .bind(session_id.as_uuid())
        .bind(i64::try_from(now_ms() + 60_000).expect("invite expiry fits PostgreSQL bigint"))
        .execute(&application_pool)
        .await
        .expect("test fixture restores the invitation expiry");

    let resolved = client
        .resolve_invitation(invitation.as_str())
        .await
        .expect("Bob resolves the invitation");
    assert_eq!(resolved.session_id, session_id);

    let duplicate_nickname_id =
        ParticipantId::try_new_v4().expect("duplicate-nickname participant ID is generated");
    let duplicate_nickname_identity = stores[2]
        .create_identity(session_id, duplicate_nickname_id)
        .await
        .expect("duplicate-nickname session key is generated");
    let duplicate_nickname_profile = profile(
        session_id,
        duplicate_nickname_id,
        "ALICE",
        duplicate_nickname_identity.public_key(),
        relay_public_key,
    );
    stores[2]
        .save_profile(&duplicate_nickname_profile)
        .await
        .expect("duplicate-nickname profile is stored locally");
    let duplicate_nickname_event = stores[2]
        .enqueue_event(
            &duplicate_nickname_profile,
            &duplicate_nickname_identity,
            now_ms(),
            joined_payload("ALICE", duplicate_nickname_identity.public_key()),
        )
        .await
        .expect("duplicate-nickname event is signed");
    let duplicate_nickname_result = client
        .join_session(
            &equalturn_protocol::JoinSessionRequest {
                protocol_version: 1,
                invitation_code: invitation.as_str().to_owned(),
                join_event: duplicate_nickname_event,
            },
            &relay_public_key,
        )
        .await;
    assert!(
        matches!(
            &duplicate_nickname_result,
            Err(RelayClientError::Rejected {
                status: 409,
                code
            }) if code == "nickname_taken"
        ),
        "unexpected duplicate nickname response: {duplicate_nickname_result:?}"
    );
    let bob_id = ParticipantId::try_new_v4().expect("Bob participant ID is generated");
    let bob_identity = stores[1]
        .create_identity(session_id, bob_id)
        .await
        .expect("Bob session key is generated");
    let bob_profile = profile(
        session_id,
        bob_id,
        "bob",
        bob_identity.public_key(),
        relay_public_key,
    );
    stores[1]
        .save_profile(&bob_profile)
        .await
        .expect("Bob profile is stored");
    let bob_join = stores[1]
        .enqueue_event(
            &bob_profile,
            &bob_identity,
            now_ms(),
            joined_payload("bob", bob_identity.public_key()),
        )
        .await
        .expect("Bob join event is signed and queued");
    let joined = client
        .join_session(
            &equalturn_protocol::JoinSessionRequest {
                protocol_version: 1,
                invitation_code: invitation.as_str().to_owned(),
                join_event: bob_join,
            },
            &relay_public_key,
        )
        .await
        .expect("Bob joins using only the invitation code");
    stores[1]
        .acknowledge(&joined.receipt)
        .await
        .expect("Bob join receipt is persisted");

    let status = client
        .session_status(session_id)
        .await
        .expect("session status is available");
    assert_eq!(status.participants.len(), 2);
    assert_eq!(
        status
            .participants
            .iter()
            .find(|participant| participant.participant_id == alice_id)
            .expect("Alice is a participant")
            .public_key,
        alice_profile.participant_public_key
    );
    assert_eq!(
        status
            .participants
            .iter()
            .find(|participant| participant.participant_id == bob_id)
            .expect("Bob is a participant")
            .public_key,
        bob_profile.participant_public_key
    );
    let status_json = serde_json::to_value(&status).expect("status serializes");
    assert!(!contains_address_field(&status_json));

    let bob_signing_key = stores[1]
        .load_identity(session_id, bob_id)
        .await
        .expect("Bob session key reloads for reconnect probes");
    let mut wrong_key_socket = raw_websocket(&relay_url, session_id, bob_id).await;
    let wrong_key_challenge = receive_challenge(&mut wrong_key_socket).await;
    let wrong_identity = SigningIdentity::generate().expect("wrong key has entropy");
    send_authentication(&mut wrong_key_socket, &wrong_key_challenge, &wrong_identity).await;
    expect_websocket_closed(&mut wrong_key_socket).await;

    let mut first_challenge_socket = raw_websocket(&relay_url, session_id, bob_id).await;
    let first_challenge = receive_challenge(&mut first_challenge_socket).await;
    let stale_authentication = authentication_message(&first_challenge, &bob_signing_key);
    send_ws_json(&mut first_challenge_socket, &stale_authentication).await;
    assert!(matches!(
        receive_server_message(&mut first_challenge_socket).await,
        ServerWsMessage::Authenticated { .. }
    ));
    let mut replay_socket = raw_websocket(&relay_url, session_id, bob_id).await;
    let fresh_challenge = receive_challenge(&mut replay_socket).await;
    assert_ne!(first_challenge.challenge_id, fresh_challenge.challenge_id);
    send_ws_json(&mut replay_socket, &stale_authentication).await;
    expect_websocket_closed(&mut replay_socket).await;
    drop(first_challenge_socket);

    let mut bob_stream = client
        .connect_session_stream(&bob_profile, bob_signing_key, 0)
        .await
        .expect("Bob authenticates with his session key");
    let mut alice_stream = client
        .connect_session_stream(
            &alice_profile,
            stores[0]
                .load_identity(session_id, alice_id)
                .await
                .expect("Alice key reloads"),
            0,
        )
        .await
        .expect("Alice authenticates with her session key");
    for _ in 0..4 {
        receive(&mut bob_stream).await;
        receive(&mut alice_stream).await;
    }

    let start = stores[0]
        .enqueue_event(
            &alice_profile,
            &stores[0]
                .load_identity(session_id, alice_id)
                .await
                .expect("Alice key reloads"),
            now_ms(),
            EventPayload::SessionStarted,
        )
        .await
        .expect("session start is signed and queued");

    let mut tampered_start = start.clone();
    tampered_start.body.client_timestamp_ms =
        tampered_start.body.client_timestamp_ms.saturating_add(1);
    assert!(matches!(
        client
            .submit_event(&tampered_start, &relay_public_key)
            .await,
        Err(RelayClientError::Rejected { status: 400, .. })
    ));

    let alice_signing_key = stores[0]
        .load_identity(session_id, alice_id)
        .await
        .expect("Alice signing key reloads for protocol probes");
    let gapped_event = ParticipantEvent::sign(
        EventBody {
            protocol_version: 1,
            session_id,
            participant_id: alice_id,
            event_id: EventId::try_new_v4().expect("gap event ID is generated"),
            sequence: start.body.sequence + 1,
            previous_hash: start.event_hash().expect("start event hash is valid"),
            client_timestamp_ms: now_ms(),
            payload: EventPayload::SyntheticAiAccessAttempt {
                target: Some("example.org".to_owned()),
            },
        },
        &alice_signing_key,
    )
    .expect("sequence-gap event has a valid participant signature");
    assert!(matches!(
        client.submit_event(&gapped_event, &relay_public_key).await,
        Err(RelayClientError::Rejected { status: 409, .. })
    ));

    let wrong_session_event = ParticipantEvent::sign(
        EventBody {
            protocol_version: 1,
            session_id: SessionId::try_new_v4().expect("unrelated session ID is generated"),
            participant_id: alice_id,
            event_id: EventId::try_new_v4().expect("wrong-session event ID is generated"),
            sequence: start.body.sequence + 1,
            previous_hash: start.event_hash().expect("start event hash is valid"),
            client_timestamp_ms: now_ms(),
            payload: EventPayload::SyntheticAiAccessAttempt { target: None },
        },
        &alice_signing_key,
    )
    .expect("wrong-session event has a valid participant signature");
    assert!(matches!(
        client
            .submit_event(&wrong_session_event, &relay_public_key)
            .await,
        Err(RelayClientError::Rejected { status: 404, .. })
    ));

    let foreign_identity = SigningIdentity::generate().expect("foreign key has entropy");
    let foreign_event = ParticipantEvent::sign(
        EventBody {
            protocol_version: 1,
            session_id,
            participant_id: ParticipantId::try_new_v4()
                .expect("foreign participant ID is generated"),
            event_id: EventId::try_new_v4().expect("foreign event ID is generated"),
            sequence: 1,
            previous_hash: Hash32::ZERO,
            client_timestamp_ms: now_ms(),
            payload: EventPayload::SyntheticAiAccessAttempt { target: None },
        },
        &foreign_identity,
    )
    .expect("foreign event is correctly signed by its non-member key");
    assert!(matches!(
        client.submit_event(&foreign_event, &relay_public_key).await,
        Err(RelayClientError::Rejected { status: 401, .. })
    ));

    let started = client
        .submit_event(&start, &relay_public_key)
        .await
        .expect("session start is accepted by the relay");
    stores[0]
        .acknowledge(&started.receipt)
        .await
        .expect("session start receipt is stored");
    let duplicate_start = client
        .submit_event(&start, &relay_public_key)
        .await
        .expect("exact start event resend is idempotent");
    assert_eq!(duplicate_start.receipt, started.receipt);
    let mut conflicting_start_body = start.body.clone();
    conflicting_start_body.client_timestamp_ms =
        conflicting_start_body.client_timestamp_ms.saturating_add(1);
    let conflicting_start = ParticipantEvent::sign(conflicting_start_body, &alice_signing_key)
        .expect("conflicting event reuses an ID but has a valid signature");
    assert!(matches!(
        client
            .submit_event(&conflicting_start, &relay_public_key)
            .await,
        Err(RelayClientError::Rejected {
            status: 409,
            code
        }) if code == "event_chain_conflict"
    ));
    assert!(matches!(
        client.resolve_invitation(invitation.as_str()).await,
        Err(RelayClientError::Rejected { status: 401, .. })
    ));
    receive_event(&mut bob_stream, start.body.event_id).await;

    let synthetic = stores[0]
        .enqueue_event(
            &alice_profile,
            &stores[0]
                .load_identity(session_id, alice_id)
                .await
                .expect("Alice key reloads"),
            now_ms(),
            EventPayload::SyntheticAiAccessAttempt {
                target: Some("chatgpt.com".to_owned()),
            },
        )
        .await
        .expect("synthetic event is signed and queued");
    let (first_delivery, concurrent_duplicate) = tokio::join!(
        client.submit_event(&synthetic, &relay_public_key),
        client.submit_event(&synthetic, &relay_public_key),
    );
    let synthetic_receipt = first_delivery.expect("synthetic event reaches the relay");
    let concurrent_duplicate =
        concurrent_duplicate.expect("concurrent duplicate delivery is idempotent");
    assert_eq!(synthetic_receipt.receipt, concurrent_duplicate.receipt);
    stores[0]
        .acknowledge(&synthetic_receipt.receipt)
        .await
        .expect("synthetic event receipt is stored");
    receive_event(&mut bob_stream, synthetic.body.event_id).await;

    assert_eq!(
        client
            .session_status(session_id)
            .await
            .expect("online status is available")
            .participants
            .iter()
            .find(|participant| participant.participant_id == bob_id)
            .expect("Bob is a participant")
            .liveness,
        LivenessState::Online
    );

    let previous_generation: i64 = sqlx::query_scalar(
        "SELECT connection_generation FROM participants WHERE session_id=$1 AND participant_id=$2",
    )
    .bind(session_id.as_uuid())
    .bind(bob_id.as_uuid())
    .fetch_one(&application_pool)
    .await
    .expect("Bob's active connection generation is stored");
    let mut duplicate_bob_socket = raw_websocket(&relay_url, session_id, bob_id).await;
    let duplicate_challenge = receive_challenge(&mut duplicate_bob_socket).await;
    send_authentication(
        &mut duplicate_bob_socket,
        &duplicate_challenge,
        &stores[1]
            .load_identity(session_id, bob_id)
            .await
            .expect("Bob key reloads for simultaneous reconnect"),
    )
    .await;
    assert!(matches!(
        receive_server_message(&mut duplicate_bob_socket).await,
        ServerWsMessage::Authenticated { .. }
    ));
    let replaced_generation: i64 = sqlx::query_scalar(
        "SELECT connection_generation FROM participants WHERE session_id=$1 AND participant_id=$2",
    )
    .bind(session_id.as_uuid())
    .bind(bob_id.as_uuid())
    .fetch_one(&application_pool)
    .await
    .expect("replacement connection generation is stored");
    assert_eq!(replaced_generation, previous_generation + 1);
    duplicate_bob_socket
        .send(Message::Binary(vec![0xff].into()))
        .await
        .expect("binary protocol probe is sent");
    expect_websocket_closed(&mut duplicate_bob_socket).await;
    wait_until_stream_closed(&mut bob_stream).await;
    alice_stream
        .close()
        .await
        .expect("Alice disconnects cleanly");
    wait_until_both_disconnected(&client, session_id, alice_id, bob_id).await;
    let offline_event = stores[1]
        .enqueue_event(
            &bob_profile,
            &stores[1]
                .load_identity(session_id, bob_id)
                .await
                .expect("Bob key reloads while offline"),
            now_ms(),
            EventPayload::SyntheticAiAccessAttempt {
                target: Some("claude.ai".to_owned()),
            },
        )
        .await
        .expect("offline event is durably queued");
    assert_eq!(
        stores[1]
            .pending_events(session_id, bob_id)
            .await
            .expect("offline queue is available"),
        vec![offline_event.clone()]
    );

    server.abort();
    let _server_stopped = server.await;
    drop(relay);
    application_pool.close().await;

    let recovery_pool = database.application_pool().await;
    sqlx::query(
        "UPDATE participants SET liveness_state='online' WHERE session_id=$1 AND participant_id=$2",
    )
    .bind(session_id.as_uuid())
    .bind(bob_id.as_uuid())
    .execute(&recovery_pool)
    .await
    .expect("persisted online state models an abrupt relay process loss");
    recovery_pool.close().await;

    let restarted_pool = database.application_pool().await;
    let restarted_relay = Relay::with_pool(
        restarted_pool.clone(),
        SigningIdentity::from_seed(relay_seed),
    )
    .await
    .expect("relay restart reconciles persisted liveness");
    let (restarted_url, restarted_server) = start_server(Arc::clone(&restarted_relay)).await;
    let restarted_client =
        RelayClient::new(&restarted_url, true).expect("restarted loopback relay client is valid");
    let (restarted_key, _) = restarted_client
        .relay_identity()
        .await
        .expect("restarted relay key is available");
    assert_eq!(restarted_key, relay_public_key);

    let alice_reconnected = restarted_client
        .connect_session_stream(
            &alice_profile,
            stores[0]
                .load_identity(session_id, alice_id)
                .await
                .expect("Alice session key persists over relay restart"),
            0,
        )
        .await
        .expect("Alice reconnects with the session signing key");
    let mut bob_reconnected = restarted_client
        .connect_session_stream(
            &bob_profile,
            stores[1]
                .load_identity(session_id, bob_id)
                .await
                .expect("Bob session key persists over relay restart"),
            0,
        )
        .await
        .expect("Bob reconnects with the session signing key");

    let pending = stores[1]
        .pending_events(session_id, bob_id)
        .await
        .expect("queued event is ready for retransmission");
    assert_eq!(pending, vec![offline_event.clone()]);
    let resent = restarted_client
        .submit_event(&offline_event, &relay_public_key)
        .await
        .expect("client resends the signed offline event");
    let duplicate = restarted_client
        .submit_event(&offline_event, &relay_public_key)
        .await
        .expect("relay accepts an idempotent duplicate delivery");
    assert_eq!(resent.receipt, duplicate.receipt);
    stores[1]
        .acknowledge(&resent.receipt)
        .await
        .expect("resend receipt is persisted");
    stores[1]
        .acknowledge(&duplicate.receipt)
        .await
        .expect("duplicate receipt is idempotent locally");
    assert!(
        stores[1]
            .pending_events(session_id, bob_id)
            .await
            .expect("queue state is readable")
            .is_empty()
    );

    let end = stores[0]
        .enqueue_event(
            &alice_profile,
            &stores[0]
                .load_identity(session_id, alice_id)
                .await
                .expect("Alice key reloads after reconnect"),
            now_ms(),
            EventPayload::SessionEnded {
                reason: equalturn_protocol::EndReason::ParticipantEnded,
            },
        )
        .await
        .expect("session end is signed and queued");
    let ended = restarted_client
        .submit_event(&end, &relay_public_key)
        .await
        .expect("creator ends the session");
    stores[0]
        .acknowledge(&ended.receipt)
        .await
        .expect("end receipt is stored");

    let mut bob_saw_offline_event = false;
    let mut bob_saw_recovery_disconnect = false;
    let mut bob_saw_end = false;
    let mut bob_saw_end_notice = false;
    let mut manifest_seen = false;
    for _ in 0..32 {
        match receive(&mut bob_reconnected).await {
            ServerWsMessage::Event { event, .. } => {
                if event.body.event_id == offline_event.body.event_id {
                    bob_saw_offline_event = true;
                }
                if event.body.event_id == end.body.event_id {
                    bob_saw_end = true;
                }
            }
            ServerWsMessage::Notice { notice } => {
                if notice.kind == RelayNoticeKind::ParticipantDisconnected
                    && notice.participant_id == Some(bob_id)
                {
                    bob_saw_recovery_disconnect = true;
                }
                if notice.kind == RelayNoticeKind::SessionEnded {
                    bob_saw_end_notice = true;
                }
            }
            ServerWsMessage::FinalManifest { manifest } => {
                assert_eq!(manifest.session_id, session_id);
                manifest_seen = true;
                break;
            }
            other => panic!("unexpected live session message: {other:?}"),
        }
    }
    assert!(bob_saw_offline_event);
    assert!(bob_saw_recovery_disconnect);
    assert!(bob_saw_end);
    assert!(bob_saw_end_notice);
    assert!(manifest_seen);

    let bundle = restarted_client
        .session_report(session_id)
        .await
        .expect("ended session report is available");
    let report_json = bundle
        .to_json_pretty()
        .expect("report serializes as bounded JSON");
    assert_eq!(
        verify_report_json_with_relay_key(&report_json, &relay_public_key).status,
        VerificationStatus::Valid
    );
    assert!(
        bundle
            .events
            .iter()
            .any(|event| { event.body.event_id == offline_event.body.event_id })
    );
    assert_eq!(
        bundle
            .receipts
            .iter()
            .filter(|receipt| receipt.event_id == offline_event.body.event_id)
            .count(),
        1
    );
    assert!(!contains_address_field(
        &serde_json::to_value(&bundle).expect("report serializes")
    ));

    let mut tampered: Value = serde_json::from_str(&report_json).expect("report JSON parses");
    let events = tampered["events"]
        .as_array_mut()
        .expect("report contains an event array");
    let synthetic = events
        .iter_mut()
        .find(|event| {
            event["body"]["event_id"] == offline_event.body.event_id.as_uuid().to_string()
        })
        .expect("offline event is in the report");
    synthetic["body"]["payload"]["target"] = Value::String("example.org".into());
    let tampered_json = serde_json::to_string(&tampered).expect("tampered JSON serializes");
    assert_eq!(
        verify_report_json_with_relay_key(&tampered_json, &relay_public_key).status,
        VerificationStatus::Invalid
    );

    drop(alice_reconnected);
    drop(bob_reconnected);
    restarted_server.abort();
    let _server_stopped = restarted_server.await;
    drop(restarted_relay);
    restarted_pool.close().await;
    database.destroy().await;
}

#[tokio::test]
async fn relay_restart_and_terminal_capacity_remain_available_at_limits() {
    let Some(database) = IsolatedDatabase::create().await else {
        return;
    };
    let directories = [TemporaryDirectory::create(), TemporaryDirectory::create()];
    let stores = [
        ClientStore::open(directories[0].path())
            .await
            .expect("creator store opens"),
        ClientStore::open(directories[1].path())
            .await
            .expect("joining participant store opens"),
    ];
    let relay_seed = [94; 32];
    let relay_identity = SigningIdentity::from_seed(relay_seed);
    let relay_key = relay_identity.public_key();
    let application_pool = database.application_pool().await;
    let relay = Relay::with_pool(application_pool.clone(), relay_identity)
        .await
        .expect("relay initializes");
    let (relay_url, server) = start_server(Arc::clone(&relay)).await;
    let client = RelayClient::new(&relay_url, true).expect("loopback client is valid");

    let session_id = SessionId::try_new_v4().expect("session ID");
    let creator_id = ParticipantId::try_new_v4().expect("creator ID");
    let creator_identity = stores[0]
        .create_identity(session_id, creator_id)
        .await
        .expect("creator session identity is generated");
    let creator_profile = profile(
        session_id,
        creator_id,
        "capacity_owner",
        creator_identity.public_key(),
        relay_key,
    );
    stores[0]
        .save_profile(&creator_profile)
        .await
        .expect("creator profile is saved");
    let creator_join = stores[0]
        .enqueue_event(
            &creator_profile,
            &creator_identity,
            now_ms(),
            joined_payload("capacity_owner", creator_identity.public_key()),
        )
        .await
        .expect("creator join event is signed");
    let invitation = InvitationCode::generate().expect("invitation has cryptographic entropy");
    let created = client
        .create_session(
            &equalturn_protocol::CreateSessionRequest {
                protocol_version: 1,
                invitation_code: invitation.as_str().to_owned(),
                creator_event: creator_join,
            },
            &relay_key,
        )
        .await
        .expect("creator creates a session");
    stores[0]
        .acknowledge(&created.receipt)
        .await
        .expect("creation receipt is stored");

    let participant_id = ParticipantId::try_new_v4().expect("participant ID");
    let participant_identity = stores[1]
        .create_identity(session_id, participant_id)
        .await
        .expect("participant session identity is generated");
    let participant_profile = profile(
        session_id,
        participant_id,
        "capacity_guest",
        participant_identity.public_key(),
        relay_key,
    );
    stores[1]
        .save_profile(&participant_profile)
        .await
        .expect("participant profile is saved");
    let participant_join = stores[1]
        .enqueue_event(
            &participant_profile,
            &participant_identity,
            now_ms(),
            joined_payload("capacity_guest", participant_identity.public_key()),
        )
        .await
        .expect("participant join event is signed");
    let joined = client
        .join_session(
            &equalturn_protocol::JoinSessionRequest {
                protocol_version: 1,
                invitation_code: invitation.as_str().to_owned(),
                join_event: participant_join,
            },
            &relay_key,
        )
        .await
        .expect("participant joins the session");
    stores[1]
        .acknowledge(&joined.receipt)
        .await
        .expect("join receipt is stored");
    let start = stores[0]
        .enqueue_event(
            &creator_profile,
            &creator_identity,
            now_ms(),
            EventPayload::SessionStarted,
        )
        .await
        .expect("session start is signed");
    let started = client
        .submit_event(&start, &relay_key)
        .await
        .expect("session starts");
    stores[0]
        .acknowledge(&started.receipt)
        .await
        .expect("start receipt is stored");

    let max_order = i64::try_from(equalturn_protocol::MAX_SESSION_EVIDENCE_ORDER)
        .expect("protocol evidence limit fits PostgreSQL bigint");
    let max_events = i64::try_from(equalturn_protocol::MAX_SESSION_EVENTS)
        .expect("protocol event limit fits PostgreSQL bigint");
    sqlx::query("UPDATE sessions SET next_evidence_order=$2, event_count=$3 WHERE session_id=$1")
        .bind(session_id.as_uuid())
        .bind(max_order - 10)
        .bind(max_events - 2)
        .execute(&application_pool)
        .await
        .expect("test fixture places session at the regular evidence boundary");
    sqlx::query(
        "UPDATE participants SET liveness_state='online', connection_generation=1 WHERE session_id=$1",
    )
        .bind(session_id.as_uuid())
        .execute(&application_pool)
        .await
        .expect("test fixture models participants online during relay loss");
    server.abort();
    let _server_stopped = server.await;
    drop(relay);
    application_pool.close().await;

    let recovered_pool = database.application_pool().await;
    let recovered_relay = Relay::with_pool(
        recovered_pool.clone(),
        SigningIdentity::from_seed(relay_seed),
    )
    .await
    .expect("relay restart records all liveness transitions within reserved capacity");
    let recovered_next_order: i64 =
        sqlx::query_scalar("SELECT next_evidence_order FROM sessions WHERE session_id=$1")
            .bind(session_id.as_uuid())
            .fetch_one(&recovered_pool)
            .await
            .expect("recovered evidence cursor is stored");
    assert_eq!(recovered_next_order, max_order - 8);
    let recovered_notices: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM relay_notices WHERE session_id=$1 AND evidence_order >= $2",
    )
    .bind(session_id.as_uuid())
    .bind(max_order - 10)
    .fetch_one(&recovered_pool)
    .await
    .expect("restart liveness notices are stored");
    assert_eq!(recovered_notices, 2);

    sqlx::query("UPDATE sessions SET next_evidence_order=$2, event_count=$3 WHERE session_id=$1")
        .bind(session_id.as_uuid())
        .bind(max_order - 1)
        .bind(max_events - 1)
        .execute(&recovered_pool)
        .await
        .expect("test fixture places session at the terminal evidence boundary");
    let (recovered_url, recovered_server) = start_server(Arc::clone(&recovered_relay)).await;
    let recovered_client =
        RelayClient::new(&recovered_url, true).expect("recovered loopback client is valid");
    let next_sequence = start.body.sequence + 1;
    let ordinary_event = ParticipantEvent::sign(
        EventBody {
            protocol_version: 1,
            session_id,
            participant_id: creator_id,
            event_id: EventId::try_new_v4().expect("event ID"),
            sequence: next_sequence,
            previous_hash: start.event_hash().expect("start event hash"),
            client_timestamp_ms: now_ms(),
            payload: EventPayload::SyntheticAiAccessAttempt { target: None },
        },
        &creator_identity,
    )
    .expect("ordinary event is correctly signed");
    assert!(matches!(
        recovered_client
            .submit_event(&ordinary_event, &relay_key)
            .await,
        Err(RelayClientError::Rejected {
            status: 429,
            ref code
        }) if code == "resource_limit"
    ));

    let end = stores[0]
        .enqueue_event(
            &creator_profile,
            &creator_identity,
            now_ms(),
            EventPayload::SessionEnded {
                reason: equalturn_protocol::EndReason::ParticipantEnded,
            },
        )
        .await
        .expect("reserved terminal event is signed");
    let ended = recovered_client
        .submit_event(&end, &relay_key)
        .await
        .expect("creator can end the session at event and evidence caps");
    assert_eq!(
        ended.receipt.evidence_order,
        u64::try_from(max_order - 1).expect("terminal order fits u64")
    );

    let capacity_report = recovered_client
        .session_report(session_id)
        .await
        .expect("session remains reportable at the cap");
    assert_eq!(
        capacity_report
            .notices
            .iter()
            .find(|notice| notice.kind == RelayNoticeKind::SessionEnded)
            .expect("end notice is included")
            .evidence_order,
        u64::try_from(max_order).expect("terminal order fits u64")
    );
    assert_eq!(
        capacity_report
            .manifest
            .as_ref()
            .expect("final manifest is included")
            .highest_evidence_order,
        equalturn_protocol::MAX_SESSION_EVIDENCE_ORDER
    );
    assert_eq!(
        capacity_report.verify_with_relay_key(&relay_key).status,
        VerificationStatus::ValidWithGaps
    );

    recovered_server.abort();
    let _server_stopped = recovered_server.await;
    drop(recovered_relay);
    recovered_pool.close().await;
    database.destroy().await;
}

fn profile(
    session_id: SessionId,
    participant_id: ParticipantId,
    nickname: &str,
    participant_public_key: PublicKey,
    relay_public_key: PublicKey,
) -> SessionProfile {
    SessionProfile {
        session_id,
        participant_id,
        nickname: nickname.to_owned(),
        participant_public_key,
        relay_public_key,
    }
}

fn joined_payload(nickname: &str, public_key: PublicKey) -> EventPayload {
    EventPayload::ParticipantJoined {
        nickname: nickname.to_owned(),
        public_key,
        client_version: "0.1.0".to_owned(),
        os_family: OsFamily::Linux,
        os_bucket: "test".to_owned(),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_millis()
        .try_into()
        .expect("current Unix milliseconds fit in u64")
}

async fn start_server(relay: Arc<Relay>) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener binds");
    let address: SocketAddr = listener.local_addr().expect("listener has a local address");
    let server = tokio::spawn(async move {
        axum::serve(listener, router(relay))
            .await
            .expect("test relay server runs");
    });
    (format!("http://{address}"), server)
}

type TestWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn raw_websocket(
    relay_url: &str,
    session_id: SessionId,
    participant_id: ParticipantId,
) -> TestWebSocket {
    let websocket_base = relay_url.replacen("http://", "ws://", 1);
    let url = format!(
        "{websocket_base}/v1/sessions/{}/participants/{}/stream",
        session_id.as_uuid(),
        participant_id.as_uuid()
    );
    timeout(Duration::from_secs(3), connect_async(url))
        .await
        .expect("relay accepts the test WebSocket connection")
        .expect("WebSocket handshake succeeds")
        .0
}

async fn receive_challenge(socket: &mut TestWebSocket) -> ReconnectChallenge {
    match receive_server_message(socket).await {
        ServerWsMessage::Challenge { challenge, .. } => challenge,
        other => panic!("expected reconnect challenge, got {other:?}"),
    }
}

async fn receive_server_message(socket: &mut TestWebSocket) -> ServerWsMessage {
    let message = timeout(Duration::from_secs(3), socket.next())
        .await
        .expect("relay WebSocket message arrives")
        .expect("relay WebSocket remains open")
        .expect("relay WebSocket frame is valid");
    let Message::Text(text) = message else {
        panic!("expected a text WebSocket message, got {message:?}");
    };
    serde_json::from_str(text.as_str()).expect("relay WebSocket message is valid JSON")
}

fn authentication_message(
    challenge: &ReconnectChallenge,
    identity: &SigningIdentity,
) -> ClientWsMessage {
    ClientWsMessage::Authenticate {
        challenge_id: challenge.challenge_id,
        signature: challenge
            .sign(identity)
            .expect("challenge signature is created"),
        after_evidence_order: 0,
    }
}

async fn send_authentication(
    socket: &mut TestWebSocket,
    challenge: &ReconnectChallenge,
    identity: &SigningIdentity,
) {
    send_ws_json(socket, &authentication_message(challenge, identity)).await;
}

async fn send_ws_json(socket: &mut TestWebSocket, message: &ClientWsMessage) {
    let json = serde_json::to_string(message).expect("client WebSocket message serializes");
    socket
        .send(Message::Text(json.into()))
        .await
        .expect("client WebSocket message sends");
}

async fn expect_websocket_closed(socket: &mut TestWebSocket) {
    timeout(Duration::from_secs(3), async {
        loop {
            match socket.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(Message::Text(_))) => continue,
                message => panic!("expected connection closure, got {message:?}"),
            }
        }
    })
    .await
    .expect("relay rejects the connection promptly");
}

async fn wait_until_stream_closed(stream: &mut SessionStream) {
    timeout(Duration::from_secs(3), async {
        loop {
            if stream.next_message().await.is_err() {
                return;
            }
        }
    })
    .await
    .expect("a replaced WebSocket connection closes");
}

async fn receive(stream: &mut SessionStream) -> ServerWsMessage {
    timeout(Duration::from_secs(5), stream.next_message())
        .await
        .expect("relay message arrives within test timeout")
        .expect("relay message is authenticated and valid")
}

async fn receive_event(stream: &mut SessionStream, expected_event_id: equalturn_protocol::EventId) {
    timeout(Duration::from_secs(5), async {
        loop {
            match stream
                .next_message()
                .await
                .expect("relay event stream remains authenticated")
            {
                ServerWsMessage::Event { event, .. }
                    if event.body.event_id == expected_event_id =>
                {
                    return;
                }
                _ => {}
            }
        }
    })
    .await
    .expect("expected event arrives before test timeout");
}

async fn wait_until_both_disconnected(
    client: &RelayClient,
    session_id: SessionId,
    alice_id: ParticipantId,
    bob_id: ParticipantId,
) {
    timeout(Duration::from_secs(3), async {
        loop {
            let status = client
                .session_status(session_id)
                .await
                .expect("session status remains available");
            let alice_disconnected = status
                .participants
                .iter()
                .find(|participant| participant.participant_id == alice_id)
                .is_some_and(|participant| participant.liveness == LivenessState::Disconnected);
            let bob_disconnected = status
                .participants
                .iter()
                .find(|participant| participant.participant_id == bob_id)
                .is_some_and(|participant| participant.liveness == LivenessState::Disconnected);
            if alice_disconnected && bob_disconnected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("both clean WebSocket closes persist disconnection");
}

fn contains_address_field(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| {
            matches!(
                key.as_str(),
                "ip" | "ip_address" | "peer_ip" | "source_ip" | "remote_address"
            ) || contains_address_field(child)
        }),
        Value::Array(values) => values.iter().any(contains_address_field),
        _ => false,
    }
}
