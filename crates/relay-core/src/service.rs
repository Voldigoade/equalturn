use std::{collections::HashMap, sync::Arc, time::Duration};

use equalturn_protocol::{
    EndReason, EventPayload, Hash32, InvitationCode, LivenessState, ManifestHead, ParticipantEvent,
    ParticipantId, ParticipantView, PublicKey, RelayNotice, RelayNoticeKind, RelayReceipt,
    SessionId, SessionManifest, SessionState, SigningIdentity, fingerprint, invitation_verifier,
};
use equalturn_report::{EvidenceBundle, ParticipantSummary};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgPoolOptions};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, broadcast, oneshot};

use crate::{
    clock::unix_time_ms,
    error::{RelayError, Result, storage},
    invitations::InviteRateLimiter,
};

const INVITATION_TTL_MS: u64 = 10 * 60 * 1_000;
const MAX_SESSION_EVENTS: i64 = 10_000;
const MAX_SESSION_EVIDENCE_ORDER: i64 = 110_000;
const MAX_SESSION_PARTICIPANTS: i64 = 8;
const TERMINAL_EVIDENCE_RECORDS: i64 = 2;
const RECOVERY_EVIDENCE_RESERVE: i64 = MAX_SESSION_PARTICIPANTS + TERMINAL_EVIDENCE_RECORDS;
const MAX_RETAINED_SESSIONS: i64 = 5_000;
const SESSION_QUOTA_LOCK: i64 = 0x0045_5155_414c_5455;

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub database_url: String,
    pub max_connections: u32,
}

pub struct Relay {
    pool: PgPool,
    identity: Arc<SigningIdentity>,
    fanout: Mutex<HashMap<SessionId, broadcast::Sender<()>>>,
    active: Mutex<HashMap<(SessionId, ParticipantId), ActiveSlot>>,
    invitation_limiter: Mutex<InviteRateLimiter>,
    creation_limiter: Mutex<CreationRateLimit>,
    connection_slots: Arc<Semaphore>,
}

struct ActiveSlot {
    generation: u64,
    close: oneshot::Sender<()>,
}

#[derive(Default)]
struct CreationRateLimit {
    window_start_ms: u64,
    attempts: u8,
}

impl CreationRateLimit {
    fn allow(&mut self, now_ms: u64) -> bool {
        if now_ms.saturating_sub(self.window_start_ms) >= 60_000 {
            self.window_start_ms = now_ms;
            self.attempts = 0;
        }
        if self.attempts >= 20 {
            return false;
        }
        self.attempts += 1;
        true
    }
}

pub struct AcceptedEvent {
    pub event: ParticipantEvent,
    pub receipt: RelayReceipt,
    pub notice: Option<RelayNotice>,
    pub manifest: Option<SessionManifest>,
}

impl Relay {
    pub async fn connect(config: &RelayConfig, identity: SigningIdentity) -> Result<Arc<Self>> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections.max(1))
            .acquire_timeout(Duration::from_secs(5))
            .connect(&config.database_url)
            .await
            .map_err(storage)?;
        Self::with_pool(pool, identity).await
    }

    pub async fn with_pool(pool: PgPool, identity: SigningIdentity) -> Result<Arc<Self>> {
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|_| {
                RelayError::Storage(sqlx::Error::Protocol("database migration failed".into()))
            })?;
        let relay = Arc::new(Self {
            pool,
            identity: Arc::new(identity),
            fanout: Mutex::new(HashMap::new()),
            active: Mutex::new(HashMap::new()),
            invitation_limiter: Mutex::new(InviteRateLimiter::default()),
            creation_limiter: Mutex::new(CreationRateLimit::default()),
            connection_slots: Arc::new(Semaphore::new(256)),
        });
        relay.verify_or_store_identity().await?;
        relay.recover_persisted_liveness().await?;
        Ok(relay)
    }

    pub fn relay_public_key(&self) -> PublicKey {
        self.identity.public_key()
    }

    pub(crate) fn try_connection_slot(&self) -> Result<OwnedSemaphorePermit> {
        Arc::clone(&self.connection_slots)
            .try_acquire_owned()
            .map_err(|_| RelayError::ResourceLimit)
    }

    pub async fn cleanup_retained_sessions(&self) -> Result<u64> {
        let now_ms = unix_time_ms().map_err(|_| RelayError::Clock)?;
        self.cleanup_retained_sessions_at(now_ms).await
    }

    pub async fn cleanup_retained_sessions_at(&self, now_ms: u64) -> Result<u64> {
        const CREATED_RETENTION_MS: u64 = 24 * 60 * 60 * 1_000;
        const ENDED_RETENTION_MS: u64 = 7 * 24 * 60 * 60 * 1_000;
        const INACTIVE_RETENTION_MS: u64 = 7 * 24 * 60 * 60 * 1_000;

        let created_cutoff = to_i64(now_ms.saturating_sub(CREATED_RETENTION_MS))?;
        let ended_cutoff = to_i64(now_ms.saturating_sub(ENDED_RETENTION_MS))?;
        let inactive_cutoff = to_i64(now_ms.saturating_sub(INACTIVE_RETENTION_MS))?;
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        let deleted = sqlx::query(
            "DELETE FROM sessions s WHERE (s.state='created' AND s.created_at_ms <= $1) OR (s.state='ended' AND s.ended_at_ms <= $2) OR (s.state='active' AND GREATEST(COALESCE(s.started_at_ms, s.created_at_ms), COALESCE((SELECT MAX(e.accepted_at_ms) FROM events e WHERE e.session_id=s.session_id), 0), COALESCE((SELECT MAX(p.last_heartbeat_ms) FROM participants p WHERE p.session_id=s.session_id), 0)) <= $3) RETURNING session_id",
        )
        .bind(created_cutoff)
        .bind(ended_cutoff)
        .bind(inactive_cutoff)
        .fetch_all(&mut *transaction)
        .await
        .map_err(storage)?;
        transaction.commit().await.map_err(storage)?;
        let deleted_ids: Vec<SessionId> = deleted
            .iter()
            .map(|row| {
                row.try_get::<uuid::Uuid, _>("session_id")
                    .map(SessionId::from)
                    .map_err(storage)
            })
            .collect::<Result<_>>()?;
        if !deleted_ids.is_empty() {
            let mut active = self.active.lock().await;
            let mut fanout = self.fanout.lock().await;
            for session_id in &deleted_ids {
                let matching: Vec<_> = active
                    .keys()
                    .filter(|(active_session, _)| active_session == session_id)
                    .copied()
                    .collect();
                for key in matching {
                    if let Some(slot) = active.remove(&key) {
                        let _ = slot.close.send(());
                    }
                }
                fanout.remove(session_id);
            }
        }
        u64::try_from(deleted_ids.len()).map_err(|_| RelayError::ResourceLimit)
    }

    async fn recover_persisted_liveness(&self) -> Result<()> {
        let sessions = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT session_id FROM sessions WHERE state IN ('created', 'active') ORDER BY session_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        for session_uuid in sessions {
            let session_id = SessionId::from(session_uuid);
            let mut transaction = self.pool.begin().await.map_err(storage)?;
            let session = sqlx::query(
                "SELECT state, next_evidence_order FROM sessions WHERE session_id=$1 FOR UPDATE",
            )
            .bind(session_uuid)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(storage)?;
            let Some(session) = session else {
                transaction.commit().await.map_err(storage)?;
                continue;
            };
            let state: String = session.try_get("state").map_err(storage)?;
            if state == "ended" {
                transaction.commit().await.map_err(storage)?;
                continue;
            }
            let participants = sqlx::query(
                "SELECT participant_id, connection_generation, heartbeat_counter FROM participants WHERE session_id=$1 AND liveness_state IN ('online', 'stale') ORDER BY participant_id FOR UPDATE",
            )
            .bind(session_uuid)
            .fetch_all(&mut *transaction)
            .await
            .map_err(storage)?;
            let mut evidence_order: i64 =
                session.try_get("next_evidence_order").map_err(storage)?;
            let recovery_count =
                i64::try_from(participants.len()).map_err(|_| RelayError::ResourceLimit)?;
            ensure_recovery_capacity(evidence_order, recovery_count)?;
            for participant in participants {
                let participant_uuid: uuid::Uuid =
                    participant.try_get("participant_id").map_err(storage)?;
                let participant_id = ParticipantId::from(participant_uuid);
                let generation: i64 = participant
                    .try_get("connection_generation")
                    .map_err(storage)?;
                let counter: i64 = participant.try_get("heartbeat_counter").map_err(storage)?;
                let notice = RelayNotice::sign(
                    session_id,
                    u64::try_from(evidence_order).map_err(|_| RelayError::ResourceLimit)?,
                    unix_time_ms().map_err(|_| RelayError::Clock)?,
                    RelayNoticeKind::ParticipantDisconnected,
                    Some(participant_id),
                    u64::try_from(generation).map_err(|_| RelayError::ResourceLimit)?,
                    u64::try_from(counter).map_err(|_| RelayError::ResourceLimit)?,
                    &self.identity,
                )?;
                self.store_notice_tx(&mut transaction, &notice).await?;
                sqlx::query("UPDATE participants SET liveness_state='disconnected' WHERE session_id=$1 AND participant_id=$2")
                    .bind(session_uuid)
                    .bind(participant_uuid)
                    .execute(&mut *transaction)
                    .await
                    .map_err(storage)?;
                evidence_order = evidence_order
                    .checked_add(1)
                    .ok_or(RelayError::ResourceLimit)?;
            }
            sqlx::query("UPDATE sessions SET next_evidence_order=$2 WHERE session_id=$1")
                .bind(session_uuid)
                .bind(evidence_order)
                .execute(&mut *transaction)
                .await
                .map_err(storage)?;
            transaction.commit().await.map_err(storage)?;
        }
        Ok(())
    }

    pub(crate) async fn channel(&self, session_id: SessionId) -> broadcast::Receiver<()> {
        let mut channels = self.fanout.lock().await;
        channels
            .entry(session_id)
            .or_insert_with(|| broadcast::channel(128).0)
            .subscribe()
    }

    async fn notify(&self, session_id: SessionId) {
        if let Some(sender) = self.fanout.lock().await.get(&session_id) {
            // Persisted replay is authoritative; this channel is only a wake-up hint.
            let _ = sender.send(());
        }
    }

    pub async fn create_session(
        &self,
        request: equalturn_protocol::CreateSessionRequest,
    ) -> Result<equalturn_protocol::CreateSessionResponse> {
        require_v1(request.protocol_version)?;
        let invite = InvitationCode::parse(&request.invitation_code)
            .map_err(|_| RelayError::InvitationRejected)?;
        let (nickname, public_key) = validate_join_event(&request.creator_event)?;
        let session_id = request.creator_event.body.session_id;
        let participant_id = request.creator_event.body.participant_id;
        let event_hash = request.creator_event.event_hash()?;
        let now_ms = unix_time_ms().map_err(|_| RelayError::Clock)?;
        let expires_at_ms = now_ms.saturating_add(INVITATION_TTL_MS);
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        if let Some(receipt) = self
            .existing_receipt(
                &mut transaction,
                session_id,
                request.creator_event.body.event_id,
                event_hash,
            )
            .await?
        {
            transaction.commit().await.map_err(storage)?;
            return Ok(equalturn_protocol::CreateSessionResponse {
                protocol_version: 1,
                session_id,
                participant_id,
                invitation_expires_at_ms: expires_at_ms,
                receipt,
                relay_public_key: self.relay_public_key(),
            });
        }
        if !self.creation_limiter.lock().await.allow(now_ms) {
            return Err(RelayError::ResourceLimit);
        }
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(SESSION_QUOTA_LOCK)
            .execute(&mut *transaction)
            .await
            .map_err(storage)?;
        let retained_sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
            .fetch_one(&mut *transaction)
            .await
            .map_err(storage)?;
        if retained_sessions >= MAX_RETAINED_SESSIONS {
            return Err(RelayError::ResourceLimit);
        }
        sqlx::query(
            "INSERT INTO sessions (session_id, creator_participant_id, state, invite_hash, invite_expires_at_ms, created_at_ms, event_count) VALUES ($1, $2, 'created', $3, $4, $5, 1)",
        )
        .bind(session_id.as_uuid())
        .bind(participant_id.as_uuid())
        .bind(invite.verifier().as_bytes().as_slice())
        .bind(to_i64(expires_at_ms)?)
        .bind(to_i64(now_ms)?)
        .execute(&mut *transaction)
        .await
        .map_err(map_insert_error)?;
        sqlx::query(
            "INSERT INTO participants (session_id, participant_id, nickname, public_key, head_sequence, head_hash, liveness_state, joined_at_ms) VALUES ($1, $2, $3, $4, 0, $5, 'disconnected', $6)",
        )
        .bind(session_id.as_uuid())
        .bind(participant_id.as_uuid())
        .bind(&nickname)
        .bind(public_key.as_bytes().as_slice())
        .bind(Hash32::ZERO.as_bytes().as_slice())
        .bind(to_i64(now_ms)?)
        .execute(&mut *transaction)
        .await
        .map_err(map_insert_error)?;
        let accepted = self
            .store_event_tx(
                &mut transaction,
                request.creator_event.clone(),
                now_ms,
                1,
                false,
            )
            .await?;
        transaction.commit().await.map_err(storage)?;
        self.publish(session_id).await;
        Ok(equalturn_protocol::CreateSessionResponse {
            protocol_version: 1,
            session_id,
            participant_id,
            invitation_expires_at_ms: expires_at_ms,
            receipt: accepted.receipt,
            relay_public_key: self.relay_public_key(),
        })
    }

    pub async fn resolve_invitation(
        &self,
        request: equalturn_protocol::ResolveInvitationRequest,
    ) -> Result<equalturn_protocol::ResolveInvitationResponse> {
        require_v1(request.protocol_version)?;
        let now_ms = unix_time_ms().map_err(|_| RelayError::Clock)?;
        if !self
            .invitation_limiter
            .lock()
            .await
            .allow(&request.invitation_code, now_ms)
        {
            return Err(RelayError::InvitationRejected);
        }
        let verifier = invitation_verifier(&request.invitation_code)
            .map_err(|_| RelayError::InvitationRejected)?;
        let row = sqlx::query(
            "SELECT session_id, invite_expires_at_ms, invite_join_count, state, (SELECT count(*) FROM participants p WHERE p.session_id = sessions.session_id) AS participant_count FROM sessions WHERE invite_hash = $1",
        )
        .bind(verifier.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .ok_or(RelayError::InvitationRejected)?;
        let expires_at_ms: i64 = row.try_get("invite_expires_at_ms").map_err(storage)?;
        let joins: i16 = row.try_get("invite_join_count").map_err(storage)?;
        let participant_count: i64 = row.try_get("participant_count").map_err(storage)?;
        let state: String = row.try_get("state").map_err(storage)?;
        if now_ms >= u64::try_from(expires_at_ms).unwrap_or(0)
            || joins >= 7
            || participant_count >= 8
            || state != "created"
        {
            return Err(RelayError::InvitationRejected);
        }
        let session_uuid: uuid::Uuid = row.try_get("session_id").map_err(storage)?;
        Ok(equalturn_protocol::ResolveInvitationResponse {
            protocol_version: 1,
            session_id: SessionId::from(session_uuid),
            invitation_expires_at_ms: u64::try_from(expires_at_ms).unwrap_or(0),
        })
    }

    pub async fn join_session(
        &self,
        request: equalturn_protocol::JoinSessionRequest,
    ) -> Result<equalturn_protocol::JoinSessionResponse> {
        require_v1(request.protocol_version)?;
        let invite = InvitationCode::parse(&request.invitation_code)
            .map_err(|_| RelayError::InvitationRejected)?;
        let (nickname, public_key) = validate_join_event(&request.join_event)?;
        let session_id = request.join_event.body.session_id;
        let participant_id = request.join_event.body.participant_id;
        let event_hash = request.join_event.event_hash()?;
        let now_ms = unix_time_ms().map_err(|_| RelayError::Clock)?;
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        if let Some(receipt) = self
            .existing_receipt(
                &mut transaction,
                session_id,
                request.join_event.body.event_id,
                event_hash,
            )
            .await?
        {
            transaction.commit().await.map_err(storage)?;
            return Ok(equalturn_protocol::JoinSessionResponse {
                protocol_version: 1,
                session_id,
                participant_id,
                receipt,
                relay_public_key: self.relay_public_key(),
            });
        }
        let session = sqlx::query(
            "SELECT state, invite_hash, invite_expires_at_ms, invite_join_count, next_evidence_order, event_count FROM sessions WHERE session_id = $1 FOR UPDATE",
        )
        .bind(session_id.as_uuid())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage)?
        .ok_or(RelayError::InvitationRejected)?;
        let invite_hash: Vec<u8> = session.try_get("invite_hash").map_err(storage)?;
        if !constant_time_equal(&invite_hash, invite.verifier().as_bytes()) {
            return Err(RelayError::InvitationRejected);
        }
        let invite_expires: i64 = session.try_get("invite_expires_at_ms").map_err(storage)?;
        let invite_joins: i16 = session.try_get("invite_join_count").map_err(storage)?;
        let state: String = session.try_get("state").map_err(storage)?;
        let event_count: i32 = session.try_get("event_count").map_err(storage)?;
        let next_order: i64 = session.try_get("next_evidence_order").map_err(storage)?;
        let participant_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM participants WHERE session_id=$1")
                .bind(session_id.as_uuid())
                .fetch_one(&mut *transaction)
                .await
                .map_err(storage)?;
        if now_ms >= u64::try_from(invite_expires).unwrap_or(0)
            || invite_joins >= 7
            || participant_count >= 8
            || state != "created"
        {
            return Err(RelayError::InvitationRejected);
        }
        ensure_event_capacity(i64::from(event_count), next_order, false)?;
        sqlx::query(
            "INSERT INTO participants (session_id, participant_id, nickname, public_key, head_sequence, head_hash, liveness_state, joined_at_ms) VALUES ($1, $2, $3, $4, 0, $5, 'disconnected', $6)",
        )
        .bind(session_id.as_uuid())
        .bind(participant_id.as_uuid())
        .bind(&nickname)
        .bind(public_key.as_bytes().as_slice())
        .bind(Hash32::ZERO.as_bytes().as_slice())
        .bind(to_i64(now_ms)?)
        .execute(&mut *transaction)
        .await
        .map_err(map_insert_error)?;
        sqlx::query(
            "UPDATE sessions SET invite_join_count = invite_join_count + 1 WHERE session_id = $1",
        )
        .bind(session_id.as_uuid())
        .execute(&mut *transaction)
        .await
        .map_err(storage)?;
        let accepted = self
            .store_event_tx(
                &mut transaction,
                request.join_event.clone(),
                now_ms,
                next_order,
                true,
            )
            .await?;
        transaction.commit().await.map_err(storage)?;
        self.publish(session_id).await;
        Ok(equalturn_protocol::JoinSessionResponse {
            protocol_version: 1,
            session_id,
            participant_id,
            receipt: accepted.receipt,
            relay_public_key: self.relay_public_key(),
        })
    }

    pub async fn submit_event(&self, event: ParticipantEvent) -> Result<AcceptedEvent> {
        let body = event.body.clone();
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        let session = sqlx::query(
            "SELECT state, creator_participant_id, next_evidence_order, event_count, started_at_ms FROM sessions WHERE session_id = $1 FOR UPDATE",
        )
        .bind(body.session_id.as_uuid())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage)?
        .ok_or(RelayError::SessionNotFound)?;
        let state: String = session.try_get("state").map_err(storage)?;
        let creator: uuid::Uuid = session.try_get("creator_participant_id").map_err(storage)?;
        let next_order: i64 = session.try_get("next_evidence_order").map_err(storage)?;
        let event_count: i32 = session.try_get("event_count").map_err(storage)?;
        let participant = sqlx::query(
            "SELECT public_key, head_sequence, head_hash FROM participants WHERE session_id=$1 AND participant_id=$2 FOR UPDATE",
        )
        .bind(body.session_id.as_uuid())
        .bind(body.participant_id.as_uuid())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage)?
        .ok_or(RelayError::AuthenticationFailed)?;
        let public_key_bytes: Vec<u8> = participant.try_get("public_key").map_err(storage)?;
        let public_key = bytes32(&public_key_bytes)?;
        let event_hash = event.event_hash()?;
        if let Some(receipt) = self
            .existing_receipt(&mut transaction, body.session_id, body.event_id, event_hash)
            .await?
        {
            transaction.commit().await.map_err(storage)?;
            return Ok(AcceptedEvent {
                event,
                receipt,
                notice: None,
                manifest: None,
            });
        }
        let terminal_event = matches!(&body.payload, EventPayload::SessionEnded { .. });
        ensure_event_capacity(i64::from(event_count), next_order, terminal_event)?;
        event.verify_with_key(&public_key)?;
        let stored_head: i64 = participant.try_get("head_sequence").map_err(storage)?;
        let expected_sequence = stored_head
            .checked_add(1)
            .ok_or(RelayError::ChainConflict)?;
        let stored_hash = bytes32(
            &participant
                .try_get::<Vec<u8>, _>("head_hash")
                .map_err(storage)?,
        )?;
        if i64::try_from(body.sequence).unwrap_or(i64::MAX) != expected_sequence
            || body.previous_hash != stored_hash
        {
            return Err(RelayError::ChainConflict);
        }
        if creator != body.participant_id.as_uuid() {
            match (&*state, &body.payload) {
                ("active", EventPayload::SyntheticAiAccessAttempt { .. }) => {}
                ("created", EventPayload::SessionStarted) => {
                    return Err(RelayError::InvalidLifecycle);
                }
                ("ended", _) => return Err(RelayError::SessionEnded),
                _ => return Err(RelayError::InvalidLifecycle),
            }
        } else {
            match (&*state, &body.payload) {
                ("created", EventPayload::SessionStarted) => {
                    let participant_count: i64 =
                        sqlx::query_scalar("SELECT count(*) FROM participants WHERE session_id=$1")
                            .bind(body.session_id.as_uuid())
                            .fetch_one(&mut *transaction)
                            .await
                            .map_err(storage)?;
                    if participant_count < 2 {
                        return Err(RelayError::InvalidLifecycle);
                    }
                }
                ("active", EventPayload::SyntheticAiAccessAttempt { .. }) => {}
                (
                    "active",
                    EventPayload::SessionEnded {
                        reason: EndReason::ParticipantEnded,
                    },
                ) => {}
                ("ended", _) => return Err(RelayError::SessionEnded),
                _ => return Err(RelayError::InvalidLifecycle),
            }
        }
        let now_ms = unix_time_ms().map_err(|_| RelayError::Clock)?;
        let mut accepted = self
            .store_event_tx(&mut transaction, event, now_ms, next_order, true)
            .await?;
        match accepted.event.body.payload {
            EventPayload::SessionStarted => {
                sqlx::query(
                    "UPDATE sessions SET state='active', started_at_ms=$2 WHERE session_id=$1",
                )
                .bind(body.session_id.as_uuid())
                .bind(to_i64(now_ms)?)
                .execute(&mut *transaction)
                .await
                .map_err(storage)?;
            }
            EventPayload::SessionEnded { .. } => {
                let order = next_order.checked_add(1).ok_or(RelayError::ResourceLimit)?;
                let notice = RelayNotice::sign(
                    body.session_id,
                    u64::try_from(order).map_err(|_| RelayError::ResourceLimit)?,
                    now_ms,
                    RelayNoticeKind::SessionEnded,
                    None,
                    0,
                    0,
                    &self.identity,
                )?;
                self.store_notice_tx(&mut transaction, &notice).await?;
                let heads = self
                    .load_manifest_heads(&mut transaction, body.session_id)
                    .await?;
                let start_ms: i64 = session
                    .try_get::<Option<i64>, _>("started_at_ms")
                    .map_err(storage)?
                    .ok_or(RelayError::InvalidLifecycle)?;
                let manifest = SessionManifest::sign(
                    body.session_id,
                    body.event_id,
                    accepted.receipt.event_hash,
                    u64::try_from(order).map_err(|_| RelayError::ResourceLimit)?,
                    u64::try_from(start_ms).map_err(|_| RelayError::Clock)?,
                    now_ms,
                    heads,
                    &self.identity,
                )?;
                let manifest_json =
                    serde_json::to_value(&manifest).map_err(|_| RelayError::InvalidRequest)?;
                sqlx::query("UPDATE sessions SET state='ended', ended_at_ms=$2, next_evidence_order=$3, final_manifest=$4 WHERE session_id=$1")
                    .bind(body.session_id.as_uuid())
                    .bind(to_i64(now_ms)?)
                    .bind(order.checked_add(1).ok_or(RelayError::ResourceLimit)?)
                    .bind(manifest_json)
                    .execute(&mut *transaction)
                    .await
                    .map_err(storage)?;
                sqlx::query("UPDATE participants SET liveness_state='ended' WHERE session_id=$1")
                    .bind(body.session_id.as_uuid())
                    .execute(&mut *transaction)
                    .await
                    .map_err(storage)?;
                accepted.notice = Some(notice);
                accepted.manifest = Some(manifest);
            }
            _ => {}
        }
        transaction.commit().await.map_err(storage)?;
        self.publish(body.session_id).await;
        Ok(accepted)
    }

    pub async fn status(
        &self,
        session_id: SessionId,
    ) -> Result<equalturn_protocol::SessionStatusResponse> {
        let session = sqlx::query("SELECT state, created_at_ms, started_at_ms, ended_at_ms FROM sessions WHERE session_id=$1")
            .bind(session_id.as_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .ok_or(RelayError::SessionNotFound)?;
        let state: String = session.try_get("state").map_err(storage)?;
        let participants_rows = sqlx::query("SELECT participant_id, nickname, public_key, liveness_state, last_heartbeat_ms FROM participants WHERE session_id=$1 ORDER BY joined_at_ms, participant_id")
            .bind(session_id.as_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        let mut participants = Vec::with_capacity(participants_rows.len());
        for row in participants_rows {
            let participant_id: uuid::Uuid = row.try_get("participant_id").map_err(storage)?;
            let public_key: Vec<u8> = row.try_get("public_key").map_err(storage)?;
            let last_heartbeat: Option<i64> = row.try_get("last_heartbeat_ms").map_err(storage)?;
            let liveness: String = row.try_get("liveness_state").map_err(storage)?;
            let public_key = bytes32(&public_key)?;
            participants.push(ParticipantView {
                participant_id: ParticipantId::from(participant_id),
                nickname: row.try_get("nickname").map_err(storage)?,
                key_fingerprint: fingerprint(&public_key),
                public_key,
                liveness: parse_liveness(&liveness)?,
                last_heartbeat_ms: last_heartbeat.map(|value| u64::try_from(value).unwrap_or(0)),
            });
        }
        Ok(equalturn_protocol::SessionStatusResponse {
            protocol_version: 1,
            session_id,
            state: parse_session_state(&state)?,
            created_at_ms: row_u64(&session, "created_at_ms")?,
            started_at_ms: row_opt_u64(&session, "started_at_ms")?,
            ended_at_ms: row_opt_u64(&session, "ended_at_ms")?,
            participants,
        })
    }

    pub async fn report(&self, session_id: SessionId) -> Result<EvidenceBundle> {
        let session = sqlx::query("SELECT state, created_at_ms, started_at_ms, ended_at_ms, final_manifest FROM sessions WHERE session_id=$1")
            .bind(session_id.as_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .ok_or(RelayError::SessionNotFound)?;
        let state: String = session.try_get("state").map_err(storage)?;
        if state != "ended" {
            return Err(RelayError::InvalidLifecycle);
        }
        let manifest_value: Value = session.try_get("final_manifest").map_err(storage)?;
        let manifest: SessionManifest =
            serde_json::from_value(manifest_value).map_err(|_| RelayError::InvalidRequest)?;
        let event_rows = sqlx::query("SELECT event_json, receipt_json FROM events WHERE session_id=$1 ORDER BY evidence_order")
            .bind(session_id.as_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        let mut events = Vec::with_capacity(event_rows.len());
        let mut receipts = Vec::with_capacity(event_rows.len());
        for row in event_rows {
            let event: Value = row.try_get("event_json").map_err(storage)?;
            let receipt: Value = row.try_get("receipt_json").map_err(storage)?;
            events.push(serde_json::from_value(event).map_err(|_| RelayError::InvalidRequest)?);
            receipts.push(serde_json::from_value(receipt).map_err(|_| RelayError::InvalidRequest)?);
        }
        let notice_rows = sqlx::query(
            "SELECT notice_json FROM relay_notices WHERE session_id=$1 ORDER BY evidence_order",
        )
        .bind(session_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        let mut relay_notices = Vec::with_capacity(notice_rows.len());
        for row in notice_rows {
            let value: Value = row.try_get("notice_json").map_err(storage)?;
            relay_notices
                .push(serde_json::from_value(value).map_err(|_| RelayError::InvalidRequest)?);
        }
        let participants = participant_summaries(&events)?;
        Ok(EvidenceBundle {
            protocol_version: 1,
            report_schema_version: 1,
            session_id,
            participants,
            events,
            receipts,
            notices: relay_notices,
            relay: equalturn_report::RelayIdentity::new(self.relay_public_key()),
            manifest: Some(manifest),
            local_only_events: Vec::new(),
        })
    }

    pub(crate) async fn authenticate_connection(
        &self,
        session_id: SessionId,
        participant_id: ParticipantId,
        close: oneshot::Sender<()>,
    ) -> Result<(u64, broadcast::Receiver<()>)> {
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        let session_state: String =
            sqlx::query_scalar("SELECT state FROM sessions WHERE session_id=$1 FOR UPDATE")
                .bind(session_id.as_uuid())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(storage)?
                .ok_or(RelayError::SessionNotFound)?;
        if session_state == "ended" {
            return Err(RelayError::SessionEnded);
        }
        let order: i64 = sqlx::query_scalar(
            "SELECT next_evidence_order FROM sessions WHERE session_id=$1 FOR UPDATE",
        )
        .bind(session_id.as_uuid())
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage)?;
        ensure_regular_evidence_capacity(order)?;
        let generation: i64 = sqlx::query_scalar("UPDATE participants SET connection_generation=connection_generation+1, heartbeat_counter=0, liveness_state='online', last_heartbeat_ms=$3 WHERE session_id=$1 AND participant_id=$2 RETURNING connection_generation")
            .bind(session_id.as_uuid())
            .bind(participant_id.as_uuid())
            .bind(to_i64(unix_time_ms().map_err(|_| RelayError::Clock)?)?)
            .fetch_one(&mut *transaction)
            .await
            .map_err(storage)?;
        let notice = RelayNotice::sign(
            session_id,
            u64::try_from(order).map_err(|_| RelayError::ResourceLimit)?,
            unix_time_ms().map_err(|_| RelayError::Clock)?,
            RelayNoticeKind::ParticipantOnline,
            Some(participant_id),
            u64::try_from(generation).map_err(|_| RelayError::ResourceLimit)?,
            0,
            &self.identity,
        )?;
        self.store_notice_tx(&mut transaction, &notice).await?;
        sqlx::query(
            "UPDATE sessions SET next_evidence_order=next_evidence_order+1 WHERE session_id=$1",
        )
        .bind(session_id.as_uuid())
        .execute(&mut *transaction)
        .await
        .map_err(storage)?;
        transaction.commit().await.map_err(storage)?;
        self.notify(session_id).await;
        let mut active = self.active.lock().await;
        let key = (session_id, participant_id);
        let new_generation = u64::try_from(generation).map_err(|_| RelayError::ResourceLimit)?;
        if active
            .get(&key)
            .is_some_and(|previous| previous.generation > new_generation)
        {
            let _ = close.send(());
            return Err(RelayError::AuthenticationFailed);
        }
        if let Some(previous) = active.remove(&key) {
            let _ = previous.close.send(());
        }
        active.insert(
            key,
            ActiveSlot {
                generation: new_generation,
                close,
            },
        );
        let receiver = self.channel(session_id).await;
        Ok((new_generation, receiver))
    }

    pub(crate) async fn heartbeat(
        &self,
        heartbeat: &equalturn_protocol::Heartbeat,
    ) -> Result<Option<RelayNotice>> {
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        let session_state: String =
            sqlx::query_scalar("SELECT state FROM sessions WHERE session_id=$1 FOR UPDATE")
                .bind(heartbeat.session_id.as_uuid())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(storage)?
                .ok_or(RelayError::SessionNotFound)?;
        if session_state == "ended" {
            return Err(RelayError::SessionEnded);
        }
        let row = sqlx::query("SELECT public_key, connection_generation, heartbeat_counter, liveness_state FROM participants WHERE session_id=$1 AND participant_id=$2 FOR UPDATE")
            .bind(heartbeat.session_id.as_uuid())
            .bind(heartbeat.participant_id.as_uuid())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(storage)?
            .ok_or(RelayError::AuthenticationFailed)?;
        let key = bytes32(&row.try_get::<Vec<u8>, _>("public_key").map_err(storage)?)?;
        heartbeat
            .verify(&key)
            .map_err(|_| RelayError::AuthenticationFailed)?;
        let generation: i64 = row.try_get("connection_generation").map_err(storage)?;
        let counter: i64 = row.try_get("heartbeat_counter").map_err(storage)?;
        if i64::try_from(heartbeat.connection_generation).unwrap_or(i64::MAX) != generation
            || i64::try_from(heartbeat.heartbeat_counter).unwrap_or(i64::MAX) <= counter
        {
            return Err(RelayError::AuthenticationFailed);
        }
        let state: String = row.try_get("liveness_state").map_err(storage)?;
        let now_ms = unix_time_ms().map_err(|_| RelayError::Clock)?;
        let order = if state != "online" {
            let order: i64 = sqlx::query_scalar(
                "SELECT next_evidence_order FROM sessions WHERE session_id=$1 FOR UPDATE",
            )
            .bind(heartbeat.session_id.as_uuid())
            .fetch_one(&mut *transaction)
            .await
            .map_err(storage)?;
            ensure_regular_evidence_capacity(order)?;
            Some(order)
        } else {
            None
        };
        sqlx::query("UPDATE participants SET heartbeat_counter=$3, last_heartbeat_ms=$4, liveness_state='online' WHERE session_id=$1 AND participant_id=$2")
            .bind(heartbeat.session_id.as_uuid())
            .bind(heartbeat.participant_id.as_uuid())
            .bind(i64::try_from(heartbeat.heartbeat_counter).map_err(|_| RelayError::AuthenticationFailed)?)
            .bind(to_i64(now_ms)?)
            .execute(&mut *transaction)
            .await
            .map_err(storage)?;
        let notice = if state != "online" {
            let order = order.ok_or(RelayError::InvalidRequest)?;
            let notice = RelayNotice::sign(
                heartbeat.session_id,
                u64::try_from(order).map_err(|_| RelayError::ResourceLimit)?,
                now_ms,
                RelayNoticeKind::ParticipantOnline,
                Some(heartbeat.participant_id),
                heartbeat.connection_generation,
                heartbeat.heartbeat_counter,
                &self.identity,
            )?;
            self.store_notice_tx(&mut transaction, &notice).await?;
            sqlx::query(
                "UPDATE sessions SET next_evidence_order=next_evidence_order+1 WHERE session_id=$1",
            )
            .bind(heartbeat.session_id.as_uuid())
            .execute(&mut *transaction)
            .await
            .map_err(storage)?;
            Some(notice)
        } else {
            None
        };
        transaction.commit().await.map_err(storage)?;
        if notice.is_some() {
            self.notify(heartbeat.session_id).await;
        }
        Ok(notice)
    }

    pub(crate) async fn disconnect(
        &self,
        session_id: SessionId,
        participant_id: ParticipantId,
        generation: u64,
    ) {
        let mut active = self.active.lock().await;
        if active
            .get(&(session_id, participant_id))
            .is_some_and(|slot| slot.generation == generation)
        {
            active.remove(&(session_id, participant_id));
        } else {
            return;
        }
        drop(active);
        if self
            .set_liveness(
                session_id,
                participant_id,
                generation,
                LivenessState::Disconnected,
            )
            .await
            .is_err()
        {
            tracing::warn!("could not persist participant disconnect transition");
        }
        let active = self.active.lock().await;
        if !active
            .keys()
            .any(|(active_session, _)| *active_session == session_id)
        {
            self.fanout.lock().await.remove(&session_id);
        }
    }

    pub(crate) async fn set_liveness(
        &self,
        session_id: SessionId,
        participant_id: ParticipantId,
        generation: u64,
        target: LivenessState,
    ) -> Result<()> {
        let kind = match target {
            LivenessState::Stale => RelayNoticeKind::ParticipantStale,
            LivenessState::Disconnected => RelayNoticeKind::ParticipantDisconnected,
            LivenessState::Online | LivenessState::Ended => return Ok(()),
        };
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        let session_state: String =
            sqlx::query_scalar("SELECT state FROM sessions WHERE session_id=$1 FOR UPDATE")
                .bind(session_id.as_uuid())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(storage)?
                .ok_or(RelayError::SessionNotFound)?;
        if session_state == "ended" {
            return Ok(());
        }
        let row = sqlx::query("SELECT heartbeat_counter, connection_generation, liveness_state FROM participants WHERE session_id=$1 AND participant_id=$2 FOR UPDATE")
            .bind(session_id.as_uuid())
            .bind(participant_id.as_uuid())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(storage)?
            .ok_or(RelayError::AuthenticationFailed)?;
        let current_generation: i64 = row.try_get("connection_generation").map_err(storage)?;
        if u64::try_from(current_generation).unwrap_or(0) != generation {
            return Err(RelayError::AuthenticationFailed);
        }
        let current_counter: i64 = row.try_get("heartbeat_counter").map_err(storage)?;
        let current_state: String = row.try_get("liveness_state").map_err(storage)?;
        if current_state == "ended" {
            transaction.commit().await.map_err(storage)?;
            return Ok(());
        }
        let target_str = match target {
            LivenessState::Stale if current_state == "online" => "stale",
            LivenessState::Disconnected
                if current_state == "online" || current_state == "stale" =>
            {
                "disconnected"
            }
            LivenessState::Stale | LivenessState::Disconnected => {
                transaction.commit().await.map_err(storage)?;
                return Ok(());
            }
            _ => return Ok(()),
        };
        let order: i64 = sqlx::query_scalar(
            "SELECT next_evidence_order FROM sessions WHERE session_id=$1 FOR UPDATE",
        )
        .bind(session_id.as_uuid())
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage)?;
        ensure_regular_evidence_capacity(order)?;
        let changed = sqlx::query("UPDATE participants SET liveness_state=$3 WHERE session_id=$1 AND participant_id=$2 AND liveness_state <> $3")
            .bind(session_id.as_uuid())
            .bind(participant_id.as_uuid())
            .bind(target_str)
            .execute(&mut *transaction)
            .await
            .map_err(storage)?
            .rows_affected() > 0;
        if !changed {
            transaction.commit().await.map_err(storage)?;
            return Ok(());
        }
        let notice = RelayNotice::sign(
            session_id,
            u64::try_from(order).map_err(|_| RelayError::ResourceLimit)?,
            unix_time_ms().map_err(|_| RelayError::Clock)?,
            kind,
            Some(participant_id),
            generation,
            u64::try_from(current_counter).map_err(|_| RelayError::ResourceLimit)?,
            &self.identity,
        )?;
        self.store_notice_tx(&mut transaction, &notice).await?;
        sqlx::query(
            "UPDATE sessions SET next_evidence_order=next_evidence_order+1 WHERE session_id=$1",
        )
        .bind(session_id.as_uuid())
        .execute(&mut *transaction)
        .await
        .map_err(storage)?;
        transaction.commit().await.map_err(storage)?;
        self.notify(session_id).await;
        Ok(())
    }

    pub(crate) async fn replay_page(
        &self,
        session_id: SessionId,
        after_evidence_order: u64,
    ) -> Result<Vec<(u64, equalturn_protocol::ServerWsMessage)>> {
        let after = i64::try_from(after_evidence_order).map_err(|_| RelayError::InvalidRequest)?;
        let event_rows = sqlx::query("SELECT event_json, receipt_json, evidence_order FROM events WHERE session_id=$1 AND evidence_order>$2 ORDER BY evidence_order LIMIT 128")
            .bind(session_id.as_uuid())
            .bind(after)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        let notice_rows = sqlx::query("SELECT notice_json, evidence_order FROM relay_notices WHERE session_id=$1 AND evidence_order>$2 ORDER BY evidence_order LIMIT 128")
            .bind(session_id.as_uuid())
            .bind(after)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        let mut ordered = Vec::with_capacity(event_rows.len() + notice_rows.len());
        for row in event_rows {
            let order: i64 = row.try_get("evidence_order").map_err(storage)?;
            let event_json: Value = row.try_get("event_json").map_err(storage)?;
            let receipt_json: Value = row.try_get("receipt_json").map_err(storage)?;
            let event: ParticipantEvent =
                serde_json::from_value(event_json).map_err(|_| RelayError::InvalidRequest)?;
            let receipt: RelayReceipt =
                serde_json::from_value(receipt_json).map_err(|_| RelayError::InvalidRequest)?;
            ordered.push((
                u64::try_from(order).map_err(|_| RelayError::InvalidRequest)?,
                equalturn_protocol::ServerWsMessage::Event { event, receipt },
            ));
        }
        for row in notice_rows {
            let order: i64 = row.try_get("evidence_order").map_err(storage)?;
            let notice_json: Value = row.try_get("notice_json").map_err(storage)?;
            let notice: RelayNotice =
                serde_json::from_value(notice_json).map_err(|_| RelayError::InvalidRequest)?;
            ordered.push((
                u64::try_from(order).map_err(|_| RelayError::InvalidRequest)?,
                equalturn_protocol::ServerWsMessage::Notice { notice },
            ));
        }
        ordered.sort_by_key(|(order, _)| *order);
        ordered.truncate(128);
        Ok(ordered)
    }

    pub(crate) async fn current_evidence_order(&self, session_id: SessionId) -> Result<u64> {
        let next: i64 =
            sqlx::query_scalar("SELECT next_evidence_order FROM sessions WHERE session_id=$1")
                .bind(session_id.as_uuid())
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?
                .ok_or(RelayError::SessionNotFound)?;
        u64::try_from(next.saturating_sub(1)).map_err(|_| RelayError::InvalidRequest)
    }

    pub(crate) async fn final_manifest(
        &self,
        session_id: SessionId,
    ) -> Result<Option<SessionManifest>> {
        let value: Option<Value> =
            sqlx::query_scalar("SELECT final_manifest FROM sessions WHERE session_id=$1")
                .bind(session_id.as_uuid())
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?
                .ok_or(RelayError::SessionNotFound)?;
        value
            .map(|value| serde_json::from_value(value).map_err(|_| RelayError::InvalidRequest))
            .transpose()
    }

    pub(crate) async fn challenge_member(
        &self,
        session_id: SessionId,
        participant_id: ParticipantId,
    ) -> Result<PublicKey> {
        let row = sqlx::query(
            "SELECT public_key FROM participants WHERE session_id=$1 AND participant_id=$2",
        )
        .bind(session_id.as_uuid())
        .bind(participant_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .ok_or(RelayError::AuthenticationFailed)?;
        bytes32(&row.try_get::<Vec<u8>, _>("public_key").map_err(storage)?)
    }

    async fn verify_or_store_identity(&self) -> Result<()> {
        let public_key = self.relay_public_key();
        let key_fingerprint = fingerprint(&public_key);
        let existing = sqlx::query(
            "SELECT public_key, fingerprint FROM relay_key_metadata WHERE singleton=TRUE",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if let Some(row) = existing {
            let stored_key: Vec<u8> = row.try_get("public_key").map_err(storage)?;
            let stored_fingerprint: Vec<u8> = row.try_get("fingerprint").map_err(storage)?;
            if !constant_time_equal(&stored_key, public_key.as_bytes())
                || !constant_time_equal(&stored_fingerprint, key_fingerprint.as_bytes())
            {
                return Err(RelayError::Identity(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "relay signing identity does not match database metadata",
                )));
            }
        } else {
            sqlx::query("INSERT INTO relay_key_metadata (singleton, public_key, fingerprint, created_at_ms) VALUES (TRUE, $1, $2, $3)")
                .bind(public_key.as_bytes().as_slice())
                .bind(key_fingerprint.as_bytes().as_slice())
                .bind(to_i64(unix_time_ms().map_err(|_| RelayError::Clock)?)?)
                .execute(&self.pool)
                .await
                .map_err(storage)?;
        }
        Ok(())
    }

    async fn existing_receipt(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        session_id: SessionId,
        event_id: equalturn_protocol::EventId,
        event_hash: Hash32,
    ) -> Result<Option<RelayReceipt>> {
        let row = sqlx::query(
            "SELECT event_hash, receipt_json FROM events WHERE session_id=$1 AND event_id=$2",
        )
        .bind(session_id.as_uuid())
        .bind(event_id.as_uuid())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(storage)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let stored_hash: Vec<u8> = row.try_get("event_hash").map_err(storage)?;
        if !constant_time_equal(&stored_hash, event_hash.as_bytes()) {
            return Err(RelayError::ChainConflict);
        }
        let receipt: Value = row.try_get("receipt_json").map_err(storage)?;
        Ok(Some(
            serde_json::from_value(receipt).map_err(|_| RelayError::InvalidRequest)?,
        ))
    }

    async fn store_event_tx(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        event: ParticipantEvent,
        received_ms: u64,
        evidence_order: i64,
        increment_event_count: bool,
    ) -> Result<AcceptedEvent> {
        let event_hash = event.event_hash()?;
        let receipt = RelayReceipt::sign(
            event.body.session_id,
            event.body.participant_id,
            event.body.event_id,
            event_hash,
            u64::try_from(evidence_order).map_err(|_| RelayError::ResourceLimit)?,
            received_ms,
            &self.identity,
        )?;
        let event_json = serde_json::to_value(&event).map_err(|_| RelayError::InvalidRequest)?;
        let receipt_json =
            serde_json::to_value(&receipt).map_err(|_| RelayError::InvalidRequest)?;
        sqlx::query("INSERT INTO events (session_id, participant_id, sequence, event_id, event_hash, event_json, evidence_order, accepted_at_ms, receipt_json) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)")
            .bind(event.body.session_id.as_uuid())
            .bind(event.body.participant_id.as_uuid())
            .bind(i64::try_from(event.body.sequence).map_err(|_| RelayError::ChainConflict)?)
            .bind(event.body.event_id.as_uuid())
            .bind(event_hash.as_bytes().as_slice())
            .bind(event_json)
            .bind(evidence_order)
            .bind(to_i64(received_ms)?)
            .bind(receipt_json)
            .execute(&mut **transaction)
            .await
            .map_err(map_insert_error)?;
        sqlx::query("UPDATE participants SET head_sequence=$3, head_hash=$4 WHERE session_id=$1 AND participant_id=$2")
            .bind(event.body.session_id.as_uuid())
            .bind(event.body.participant_id.as_uuid())
            .bind(i64::try_from(event.body.sequence).map_err(|_| RelayError::ChainConflict)?)
            .bind(event_hash.as_bytes().as_slice())
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
        if increment_event_count {
            sqlx::query("UPDATE sessions SET next_evidence_order=$2, event_count=event_count+1 WHERE session_id=$1")
                .bind(event.body.session_id.as_uuid())
                .bind(evidence_order.checked_add(1).ok_or(RelayError::ResourceLimit)?)
                .execute(&mut **transaction)
                .await
                .map_err(storage)?;
        } else {
            sqlx::query("UPDATE sessions SET next_evidence_order=$2 WHERE session_id=$1")
                .bind(event.body.session_id.as_uuid())
                .bind(
                    evidence_order
                        .checked_add(1)
                        .ok_or(RelayError::ResourceLimit)?,
                )
                .execute(&mut **transaction)
                .await
                .map_err(storage)?;
        }
        Ok(AcceptedEvent {
            event,
            receipt,
            notice: None,
            manifest: None,
        })
    }

    async fn store_notice_tx(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        notice: &RelayNotice,
    ) -> Result<()> {
        if notice.evidence_order > u64::try_from(MAX_SESSION_EVIDENCE_ORDER).unwrap_or(0) {
            return Err(RelayError::ResourceLimit);
        }
        let value = serde_json::to_value(notice).map_err(|_| RelayError::InvalidRequest)?;
        sqlx::query("INSERT INTO relay_notices (session_id, notice_id, evidence_order, notice_json) VALUES ($1, $2, $3, $4)")
            .bind(notice.session_id.as_uuid())
            .bind(notice.notice_id.as_uuid())
            .bind(i64::try_from(notice.evidence_order).map_err(|_| RelayError::ResourceLimit)?)
            .bind(value)
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn load_manifest_heads(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        session_id: SessionId,
    ) -> Result<Vec<ManifestHead>> {
        let rows = sqlx::query(
            "SELECT participant_id, head_sequence, head_hash FROM participants WHERE session_id=$1",
        )
        .bind(session_id.as_uuid())
        .fetch_all(&mut **transaction)
        .await
        .map_err(storage)?;
        let mut heads = Vec::with_capacity(rows.len());
        for row in rows {
            let participant_id: uuid::Uuid = row.try_get("participant_id").map_err(storage)?;
            let sequence: i64 = row.try_get("head_sequence").map_err(storage)?;
            let hash: Vec<u8> = row.try_get("head_hash").map_err(storage)?;
            heads.push(ManifestHead {
                participant_id: ParticipantId::from(participant_id),
                final_sequence: u64::try_from(sequence).map_err(|_| RelayError::ChainConflict)?,
                final_hash: bytes32(&hash)?,
            });
        }
        heads.sort_by_key(|head| head.participant_id);
        Ok(heads)
    }

    async fn publish(&self, session_id: SessionId) {
        self.notify(session_id).await;
    }
}

fn ensure_event_capacity(event_count: i64, next_order: i64, terminal: bool) -> Result<()> {
    let event_limit = if terminal {
        MAX_SESSION_EVENTS
    } else {
        MAX_SESSION_EVENTS - 1
    };
    let evidence_limit = if terminal {
        MAX_SESSION_EVIDENCE_ORDER - 1
    } else {
        MAX_SESSION_EVIDENCE_ORDER - RECOVERY_EVIDENCE_RESERVE
    };
    if event_count < event_limit && next_order <= evidence_limit {
        Ok(())
    } else {
        Err(RelayError::ResourceLimit)
    }
}

fn ensure_regular_evidence_capacity(next_order: i64) -> Result<()> {
    if next_order <= MAX_SESSION_EVIDENCE_ORDER - RECOVERY_EVIDENCE_RESERVE {
        Ok(())
    } else {
        Err(RelayError::ResourceLimit)
    }
}

fn ensure_recovery_capacity(next_order: i64, participant_count: i64) -> Result<()> {
    if !(0..=MAX_SESSION_PARTICIPANTS).contains(&participant_count) {
        return Err(RelayError::ResourceLimit);
    }
    let last_recovery_order = next_order
        .checked_add(participant_count.saturating_sub(1))
        .ok_or(RelayError::ResourceLimit)?;
    if participant_count == 0
        || last_recovery_order <= MAX_SESSION_EVIDENCE_ORDER - TERMINAL_EVIDENCE_RECORDS
    {
        Ok(())
    } else {
        Err(RelayError::ResourceLimit)
    }
}

fn validate_join_event(event: &ParticipantEvent) -> Result<(String, PublicKey)> {
    event.body.signing_bytes()?;
    if event.body.protocol_version != 1
        || event.body.sequence != 1
        || event.body.previous_hash != Hash32::ZERO
    {
        return Err(RelayError::InvalidRequest);
    }
    let EventPayload::ParticipantJoined {
        nickname,
        public_key,
        ..
    } = &event.body.payload
    else {
        return Err(RelayError::InvalidRequest);
    };
    event
        .verify_with_key(public_key)
        .map_err(|_| RelayError::AuthenticationFailed)?;
    Ok((nickname.clone(), *public_key))
}

fn participant_summaries(events: &[ParticipantEvent]) -> Result<Vec<ParticipantSummary>> {
    let mut summaries = Vec::new();
    for event in events.iter().filter(|event| event.body.sequence == 1) {
        let EventPayload::ParticipantJoined {
            nickname,
            public_key,
            client_version,
            os_family,
            os_bucket,
        } = &event.body.payload
        else {
            return Err(RelayError::InvalidRequest);
        };
        summaries.push(ParticipantSummary {
            participant_id: event.body.participant_id,
            nickname: nickname.clone(),
            public_key: *public_key,
            key_fingerprint: fingerprint(public_key),
            client_version: client_version.clone(),
            os_family: *os_family,
            os_bucket: os_bucket.clone(),
        });
    }
    summaries.sort_by_key(|participant| participant.participant_id);
    Ok(summaries)
}

fn parse_session_state(value: &str) -> Result<SessionState> {
    match value {
        "created" => Ok(SessionState::Created),
        "active" => Ok(SessionState::Active),
        "ended" => Ok(SessionState::Ended),
        _ => Err(RelayError::InvalidRequest),
    }
}

fn parse_liveness(value: &str) -> Result<LivenessState> {
    match value {
        "online" => Ok(LivenessState::Online),
        "stale" => Ok(LivenessState::Stale),
        "disconnected" => Ok(LivenessState::Disconnected),
        "ended" => Ok(LivenessState::Ended),
        _ => Err(RelayError::InvalidRequest),
    }
}

fn require_v1(version: u16) -> Result<()> {
    if version == 1 {
        Ok(())
    } else {
        Err(RelayError::UnsupportedVersion)
    }
}

fn to_i64(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| RelayError::ResourceLimit)
}

fn bytes32(value: &[u8]) -> Result<Hash32> {
    let bytes: [u8; 32] = value.try_into().map_err(|_| RelayError::InvalidRequest)?;
    Ok(Hash32::new(bytes))
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0_u8, |accumulator, (a, b)| accumulator | (a ^ b))
        == 0
}

fn row_u64(row: &sqlx::postgres::PgRow, column: &str) -> Result<u64> {
    let value: i64 = row.try_get(column).map_err(storage)?;
    u64::try_from(value).map_err(|_| RelayError::InvalidRequest)
}

fn row_opt_u64(row: &sqlx::postgres::PgRow, column: &str) -> Result<Option<u64>> {
    let value: Option<i64> = row.try_get(column).map_err(storage)?;
    value
        .map(|item| u64::try_from(item).map_err(|_| RelayError::InvalidRequest))
        .transpose()
}

fn map_insert_error(error: sqlx::Error) -> RelayError {
    if let Some(database_error) = error.as_database_error()
        && database_error.code().as_deref() == Some("23505")
    {
        return RelayError::NicknameConflict;
    }
    storage(error)
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_SESSION_EVENTS, MAX_SESSION_EVIDENCE_ORDER, RECOVERY_EVIDENCE_RESERVE,
        ensure_event_capacity, ensure_recovery_capacity, ensure_regular_evidence_capacity,
    };

    #[test]
    fn terminal_event_and_end_notice_capacity_are_reserved() {
        let last_regular_order = MAX_SESSION_EVIDENCE_ORDER - RECOVERY_EVIDENCE_RESERVE;
        assert!(ensure_event_capacity(MAX_SESSION_EVENTS - 2, last_regular_order, false).is_ok());
        assert!(ensure_event_capacity(MAX_SESSION_EVENTS - 1, last_regular_order, false).is_err());
        assert!(ensure_regular_evidence_capacity(last_regular_order + 1).is_err());
        assert!(
            ensure_event_capacity(MAX_SESSION_EVENTS - 1, MAX_SESSION_EVIDENCE_ORDER - 1, true)
                .is_ok()
        );
        assert!(
            ensure_event_capacity(MAX_SESSION_EVENTS, MAX_SESSION_EVIDENCE_ORDER - 1, true)
                .is_err()
        );
        assert!(
            ensure_event_capacity(MAX_SESSION_EVENTS - 1, MAX_SESSION_EVIDENCE_ORDER, true)
                .is_err()
        );
    }

    #[test]
    fn relay_restart_capacity_covers_every_participant_and_terminal_records() {
        let last_regular_order = MAX_SESSION_EVIDENCE_ORDER - RECOVERY_EVIDENCE_RESERVE;
        let after_regular_record = last_regular_order + 1;
        assert!(ensure_recovery_capacity(after_regular_record, 8).is_ok());
        assert!(ensure_recovery_capacity(after_regular_record + 1, 8).is_err());
        assert!(ensure_recovery_capacity(MAX_SESSION_EVIDENCE_ORDER, 0).is_ok());
    }
}
