use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use equalturn_protocol::{
    EventBody, EventPayload, Hash32, MAX_EVENT_BYTES, MAX_SESSION_EVENTS, PROTOCOL_VERSION,
    ParticipantEvent, ParticipantId, ProtocolError, PublicKey, RelayReceipt, SessionId,
    SigningIdentity,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    Connection, Row,
    sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow, SqliteSynchronous,
    },
};
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;
use zeroize::Zeroize;

const DATABASE_FILE: &str = "client.sqlite3";
const KEY_DIRECTORY: &str = "identities";
const KEY_MAGIC: &[u8; 8] = b"ETKEY\0\0\x01";
const KEY_FILE_BYTES: usize = 8 + 16 + 16 + 16 + 32;
const SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionProfile {
    pub session_id: SessionId,
    pub participant_id: ParticipantId,
    pub nickname: String,
    pub participant_public_key: PublicKey,
    pub relay_public_key: PublicKey,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("invalid session profile")]
    InvalidProfile,
    #[error("session profile was not found")]
    ProfileNotFound,
    #[error("session profile conflicts with an existing local profile")]
    ProfileConflict,
    #[error("local database is corrupt or has an unsupported schema")]
    CorruptDatabase,
    #[error(
        "local database is missing while session key material remains; rejoin to establish a new identity"
    )]
    DatabaseLost,
    #[error("local database schema version is unsupported")]
    UnsupportedSchemaVersion,
    #[error("session signing key is missing; rejoin to establish a new identity")]
    LostKey,
    #[error("session signing key file is corrupt")]
    CorruptKey,
    #[error(
        "session key belongs to a different local database; rejoin to establish a new identity"
    )]
    IdentityDiscontinuity,
    #[error("session signing key does not match the stored participant profile")]
    IdentityMismatch,
    #[error("session key file permissions are broader than owner-only")]
    InsecureKeyPermissions,
    #[error("local data directory permissions are broader than owner-only")]
    InsecureDataDirectory,
    #[error("event receipt is invalid or does not match the local event")]
    InvalidReceipt,
    #[error("event is not present in the local outbox")]
    UnknownEvent,
    #[error("receipt conflicts with an acknowledgement already stored")]
    ReceiptConflict,
    #[error("participant sequence limit reached")]
    SequenceLimit,
    #[error("serialized local event evidence is corrupt")]
    CorruptEvidence,
    #[error("event envelope exceeds the protocol size limit")]
    EventTooLarge,
    #[error("local storage operation failed")]
    Storage(#[source] sqlx::Error),
    #[error("local key file operation failed")]
    KeyIo(#[source] io::Error),
    #[error("protocol event is invalid")]
    Protocol(#[from] ProtocolError),
}

pub struct ClientStore {
    pool: sqlx::SqlitePool,
    data_dir: PathBuf,
    store_id: [u8; 16],
    write_gate: Mutex<()>,
}

impl ClientStore {
    pub async fn open(data_dir: impl AsRef<Path>) -> Result<Self, ClientError> {
        let data_dir = data_dir.as_ref().to_path_buf();
        prepare_data_directory(&data_dir)?;
        let database_path = data_dir.join(DATABASE_FILE);
        let database_exists = database_path.exists();
        let fresh_database = !database_exists
            || fs::metadata(&database_path)
                .map_err(ClientError::KeyIo)?
                .len()
                == 0;

        if fresh_database && has_session_keys(&data_dir.join(KEY_DIRECTORY))? {
            return Err(if database_exists {
                ClientError::CorruptDatabase
            } else {
                ClientError::DatabaseLost
            });
        }

        let options = SqliteConnectOptions::new()
            .filename(&database_path)
            .create_if_missing(fresh_database)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(map_sqlx)?;

        let store_id = if fresh_database {
            initialize_schema(&pool).await?
        } else {
            validate_existing_schema(&pool).await?
        };

        Ok(Self {
            pool,
            data_dir,
            store_id,
            write_gate: Mutex::new(()),
        })
    }

    pub async fn save_profile(&self, profile: &SessionProfile) -> Result<(), ClientError> {
        validate_profile(profile)?;
        let value = serde_json::to_vec(profile).map_err(|_| ClientError::CorruptEvidence)?;
        let _guard = self.write_gate.lock().await;
        let mut connection = self.pool.acquire().await.map_err(map_sqlx)?;
        let mut transaction = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(map_sqlx)?;

        let existing =
            sqlx::query("SELECT profile_json FROM session_profiles WHERE session_id = ?1 AND participant_id = ?2")
                .bind(uuid_string(profile.session_id.as_uuid()))
                .bind(uuid_string(profile.participant_id.as_uuid()))
                .fetch_optional(&mut *transaction)
                .await
                .map_err(map_sqlx)?;
        if let Some(row) = existing {
            let stored: Vec<u8> = row.try_get(0).map_err(map_sqlx)?;
            if stored != value {
                return Err(ClientError::ProfileConflict);
            }
            transaction.commit().await.map_err(map_sqlx)?;
            return Ok(());
        }

        sqlx::query(
            "INSERT INTO session_profiles (session_id, participant_id, profile_json) VALUES (?1, ?2, ?3)",
        )
        .bind(uuid_string(profile.session_id.as_uuid()))
        .bind(uuid_string(profile.participant_id.as_uuid()))
        .bind(value)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        transaction.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn load_profile(
        &self,
        session_id: SessionId,
        participant_id: ParticipantId,
    ) -> Result<Option<SessionProfile>, ClientError> {
        let row = sqlx::query(
            "SELECT participant_id, profile_json FROM session_profiles WHERE session_id = ?1 AND participant_id = ?2",
        )
        .bind(uuid_string(session_id.as_uuid()))
        .bind(uuid_string(participant_id.as_uuid()))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| {
            let stored_participant_id: String = row.try_get(0).map_err(map_sqlx)?;
            let value: Vec<u8> = row.try_get(1).map_err(map_sqlx)?;
            let profile: SessionProfile =
                serde_json::from_slice(&value).map_err(|_| ClientError::CorruptEvidence)?;
            validate_profile(&profile)?;
            if profile.session_id != session_id
                || profile.participant_id != participant_id
                || uuid_string(profile.participant_id.as_uuid()) != stored_participant_id
            {
                return Err(ClientError::CorruptEvidence);
            }
            Ok(profile)
        })
        .transpose()
    }

    pub async fn profiles_for_session(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<SessionProfile>, ClientError> {
        if !session_id.is_v4() {
            return Err(ClientError::InvalidProfile);
        }
        let rows = sqlx::query(
            "SELECT participant_id, profile_json FROM session_profiles WHERE session_id = ?1 ORDER BY participant_id",
        )
        .bind(uuid_string(session_id.as_uuid()))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(|row| {
                let participant_id: String = row.try_get(0).map_err(map_sqlx)?;
                let value: Vec<u8> = row.try_get(1).map_err(map_sqlx)?;
                let profile: SessionProfile =
                    serde_json::from_slice(&value).map_err(|_| ClientError::CorruptEvidence)?;
                validate_profile(&profile)?;
                if profile.session_id != session_id
                    || uuid_string(profile.participant_id.as_uuid()) != participant_id
                {
                    return Err(ClientError::CorruptEvidence);
                }
                Ok(profile)
            })
            .collect()
    }

    pub async fn event_head(
        &self,
        session_id: SessionId,
        participant_id: ParticipantId,
    ) -> Result<(u64, Hash32), ClientError> {
        let row = sqlx::query(
            "SELECT last_sequence, last_hash FROM participant_heads WHERE session_id = ?1 AND participant_id = ?2",
        )
        .bind(uuid_string(session_id.as_uuid()))
        .bind(uuid_string(participant_id.as_uuid()))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        match row {
            Some(row) => {
                let sequence: i64 = row.try_get(0).map_err(map_sqlx)?;
                let hash: String = row.try_get(1).map_err(map_sqlx)?;
                Ok((
                    u64::try_from(sequence).map_err(|_| ClientError::CorruptEvidence)?,
                    parse_hash(&hash)?,
                ))
            }
            None => Ok((0, Hash32::ZERO)),
        }
    }

    pub async fn create_identity(
        &self,
        session_id: SessionId,
        participant_id: ParticipantId,
    ) -> Result<SigningIdentity, ClientError> {
        if session_id.is_nil()
            || !session_id.is_v4()
            || participant_id.is_nil()
            || !participant_id.is_v4()
        {
            return Err(ClientError::InvalidProfile);
        }
        let identity = SigningIdentity::generate()?;
        let path = self.key_path(session_id, participant_id);
        if let Some(parent) = path.parent() {
            create_private_directory(parent)?;
        }

        let mut seed = identity.seed_bytes();
        let mut bytes = Vec::with_capacity(KEY_FILE_BYTES);
        bytes.extend_from_slice(KEY_MAGIC);
        bytes.extend_from_slice(session_id.as_bytes());
        bytes.extend_from_slice(participant_id.as_bytes());
        bytes.extend_from_slice(&self.store_id);
        bytes.extend_from_slice(&seed);
        seed.zeroize();

        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                bytes.zeroize();
                return Err(ClientError::ProfileConflict);
            }
            Err(error) => {
                bytes.zeroize();
                return Err(ClientError::KeyIo(error));
            }
        };
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            bytes.zeroize();
            return Err(ClientError::KeyIo(error));
        }
        bytes.zeroize();
        sync_parent_directory(&path)?;
        Ok(identity)
    }

    pub async fn load_identity(
        &self,
        session_id: SessionId,
        participant_id: ParticipantId,
    ) -> Result<SigningIdentity, ClientError> {
        if session_id.is_nil()
            || !session_id.is_v4()
            || participant_id.is_nil()
            || !participant_id.is_v4()
        {
            return Err(ClientError::InvalidProfile);
        }
        let path = self.key_path(session_id, participant_id);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(ClientError::LostKey);
            }
            Err(error) => return Err(ClientError::KeyIo(error)),
        };
        if !metadata.file_type().is_file() {
            return Err(ClientError::CorruptKey);
        }
        validate_key_permissions(&metadata)?;
        if metadata.len() != KEY_FILE_BYTES as u64 {
            return Err(ClientError::CorruptKey);
        }

        let file = File::open(&path).map_err(ClientError::KeyIo)?;
        let mut bytes = Vec::with_capacity(KEY_FILE_BYTES);
        if let Err(error) = file
            .take((KEY_FILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
        {
            bytes.zeroize();
            return Err(ClientError::KeyIo(error));
        }
        if bytes.len() != KEY_FILE_BYTES || &bytes[..KEY_MAGIC.len()] != KEY_MAGIC {
            bytes.zeroize();
            return Err(ClientError::CorruptKey);
        }
        if &bytes[8..24] != session_id.as_bytes() {
            bytes.zeroize();
            return Err(ClientError::CorruptKey);
        }
        if &bytes[24..40] != participant_id.as_bytes() {
            bytes.zeroize();
            return Err(ClientError::CorruptKey);
        }
        if bytes[40..56] != self.store_id[..] {
            bytes.zeroize();
            return Err(ClientError::IdentityDiscontinuity);
        }
        let mut seed = [0_u8; 32];
        seed.copy_from_slice(&bytes[56..88]);
        bytes.zeroize();
        let identity = SigningIdentity::from_seed(seed);
        if let Some(profile) = self.load_profile(session_id, participant_id).await?
            && identity.public_key() != profile.participant_public_key
        {
            return Err(ClientError::IdentityMismatch);
        }
        Ok(identity)
    }

    pub async fn enqueue_event(
        &self,
        profile: &SessionProfile,
        identity: &SigningIdentity,
        client_timestamp_ms: u64,
        payload: EventPayload,
    ) -> Result<ParticipantEvent, ClientError> {
        validate_profile(profile)?;
        if identity.public_key() != profile.participant_public_key {
            return Err(ClientError::IdentityMismatch);
        }
        let stored_profile = self
            .load_profile(profile.session_id, profile.participant_id)
            .await?
            .ok_or(ClientError::ProfileNotFound)?;
        if stored_profile != *profile {
            return Err(ClientError::ProfileConflict);
        }

        let _guard = self.write_gate.lock().await;
        let mut connection = self.pool.acquire().await.map_err(map_sqlx)?;
        let mut transaction = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(map_sqlx)?;
        let prior_rows = sqlx::query(
            "SELECT sequence, event_id, previous_hash, event_hash, event_json, evidence_order, receipt_json FROM outbox WHERE session_id = ?1 AND participant_id = ?2 ORDER BY sequence ASC",
        )
        .bind(uuid_string(profile.session_id.as_uuid()))
        .bind(uuid_string(profile.participant_id.as_uuid()))
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        let verified_prior = verify_outbox_rows(prior_rows, profile)?;
        let expected_head_sequence =
            i64::try_from(verified_prior.len()).map_err(|_| ClientError::CorruptEvidence)?;
        let expected_head_hash = verified_prior
            .last()
            .map(|entry| entry.0.event_hash())
            .transpose()?
            .unwrap_or(Hash32::ZERO);
        let head = sqlx::query(
            "SELECT last_sequence, last_hash FROM participant_heads WHERE session_id = ?1 AND participant_id = ?2",
        )
        .bind(uuid_string(profile.session_id.as_uuid()))
        .bind(uuid_string(profile.participant_id.as_uuid()))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        let (last_sequence, previous_hash) = match head {
            Some(row) => {
                let sequence: i64 = row.try_get(0).map_err(map_sqlx)?;
                let encoded_hash: String = row.try_get(1).map_err(map_sqlx)?;
                let hash = parse_hash(&encoded_hash)?;
                (sequence, hash)
            }
            None => (0, Hash32::ZERO),
        };
        if last_sequence != expected_head_sequence || previous_hash != expected_head_hash {
            return Err(ClientError::CorruptEvidence);
        }
        let next_sequence = u64::try_from(last_sequence)
            .map_err(|_| ClientError::CorruptEvidence)?
            .checked_add(1)
            .filter(|sequence| *sequence <= MAX_SESSION_EVENTS)
            .ok_or(ClientError::SequenceLimit)?;
        let event = ParticipantEvent::sign(
            EventBody {
                protocol_version: PROTOCOL_VERSION,
                session_id: profile.session_id,
                participant_id: profile.participant_id,
                event_id: equalturn_protocol::EventId::try_new_v4()?,
                sequence: next_sequence,
                previous_hash,
                client_timestamp_ms,
                payload,
            },
            identity,
        )?;
        let encoded = serde_json::to_vec(&event).map_err(|_| ClientError::CorruptEvidence)?;
        if encoded.len() > MAX_EVENT_BYTES {
            return Err(ClientError::EventTooLarge);
        }
        let event_hash = event.event_hash()?;
        let event_hash_hex = event_hash.to_hex();
        let previous_hash_hex = previous_hash.to_hex();

        sqlx::query(
            "INSERT INTO outbox (session_id, participant_id, sequence, event_id, previous_hash, event_hash, event_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(uuid_string(profile.session_id.as_uuid()))
        .bind(uuid_string(profile.participant_id.as_uuid()))
        .bind(i64::try_from(next_sequence).map_err(|_| ClientError::SequenceLimit)?)
        .bind(uuid_string(event.body.event_id.as_uuid()))
        .bind(previous_hash_hex)
        .bind(event_hash_hex.clone())
        .bind(encoded)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            "INSERT INTO participant_heads (session_id, participant_id, last_sequence, last_hash) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(session_id, participant_id) DO UPDATE SET last_sequence = excluded.last_sequence, last_hash = excluded.last_hash",
        )
        .bind(uuid_string(profile.session_id.as_uuid()))
        .bind(uuid_string(profile.participant_id.as_uuid()))
        .bind(i64::try_from(next_sequence).map_err(|_| ClientError::SequenceLimit)?)
        .bind(event_hash_hex)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        transaction.commit().await.map_err(map_sqlx)?;
        Ok(event)
    }

    pub async fn pending_events(
        &self,
        session_id: SessionId,
        participant_id: ParticipantId,
    ) -> Result<Vec<ParticipantEvent>, ClientError> {
        let _guard = self.write_gate.lock().await;
        let profile = self
            .load_profile(session_id, participant_id)
            .await?
            .ok_or(ClientError::ProfileNotFound)?;
        let mut connection = self.pool.acquire().await.map_err(map_sqlx)?;
        let mut transaction = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(map_sqlx)?;
        let rows = sqlx::query(
            "SELECT sequence, event_id, previous_hash, event_hash, event_json, evidence_order, receipt_json FROM outbox WHERE session_id = ?1 AND participant_id = ?2 ORDER BY sequence ASC",
        )
        .bind(uuid_string(session_id.as_uuid()))
        .bind(uuid_string(participant_id.as_uuid()))
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        let entries = verify_outbox_rows(rows, &profile)?;
        verify_head(&mut transaction, session_id, participant_id, &entries).await?;
        transaction.commit().await.map_err(map_sqlx)?;
        Ok(entries
            .into_iter()
            .filter_map(|(event, receipt)| receipt.is_none().then_some(event))
            .collect())
    }

    pub async fn acknowledge(&self, receipt: &RelayReceipt) -> Result<(), ClientError> {
        let _guard = self.write_gate.lock().await;
        let profile = self
            .load_profile(receipt.session_id, receipt.participant_id)
            .await?
            .ok_or(ClientError::ProfileNotFound)?;
        receipt
            .verify(&profile.relay_public_key)
            .map_err(|_| ClientError::InvalidReceipt)?;

        let mut connection = self.pool.acquire().await.map_err(map_sqlx)?;
        let mut transaction = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(map_sqlx)?;
        let outbox_rows = sqlx::query(
            "SELECT sequence, event_id, previous_hash, event_hash, event_json, evidence_order, receipt_json FROM outbox WHERE session_id = ?1 AND participant_id = ?2 ORDER BY sequence ASC",
        )
        .bind(uuid_string(receipt.session_id.as_uuid()))
        .bind(uuid_string(receipt.participant_id.as_uuid()))
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        let entries = verify_outbox_rows(outbox_rows, &profile)?;
        verify_head(
            &mut transaction,
            receipt.session_id,
            receipt.participant_id,
            &entries,
        )
        .await?;
        let row = sqlx::query(
            "SELECT event_id, event_hash, receipt_json FROM outbox WHERE session_id = ?1 AND participant_id = ?2 AND sequence >= 1 AND event_id = ?3",
        )
        .bind(uuid_string(receipt.session_id.as_uuid()))
        .bind(uuid_string(receipt.participant_id.as_uuid()))
        .bind(uuid_string(receipt.event_id.as_uuid()))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx)?
        .ok_or(ClientError::UnknownEvent)?;
        let event_hash: String = row.try_get(1).map_err(map_sqlx)?;
        let prior_receipt: Option<Vec<u8>> = row.try_get(2).map_err(map_sqlx)?;
        let expected_hash = parse_hash(&event_hash)?;
        if receipt.event_hash != expected_hash {
            return Err(ClientError::InvalidReceipt);
        }
        let encoded_receipt =
            serde_json::to_vec(receipt).map_err(|_| ClientError::InvalidReceipt)?;
        if let Some(prior) = prior_receipt {
            if prior == encoded_receipt {
                transaction.commit().await.map_err(map_sqlx)?;
                return Ok(());
            }
            return Err(ClientError::ReceiptConflict);
        }

        let occupied_order = sqlx::query(
            "SELECT event_id FROM outbox WHERE session_id = ?1 AND evidence_order = ?2",
        )
        .bind(uuid_string(receipt.session_id.as_uuid()))
        .bind(i64::try_from(receipt.evidence_order).map_err(|_| ClientError::InvalidReceipt)?)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        if occupied_order.is_some() {
            return Err(ClientError::ReceiptConflict);
        }

        sqlx::query(
            "UPDATE outbox SET evidence_order = ?1, receipt_json = ?2 WHERE session_id = ?3 AND participant_id = ?4 AND event_id = ?5 AND receipt_json IS NULL",
        )
        .bind(i64::try_from(receipt.evidence_order).map_err(|_| ClientError::InvalidReceipt)?)
        .bind(encoded_receipt)
        .bind(uuid_string(receipt.session_id.as_uuid()))
        .bind(uuid_string(receipt.participant_id.as_uuid()))
        .bind(uuid_string(receipt.event_id.as_uuid()))
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        transaction.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    fn key_path(&self, session_id: SessionId, participant_id: ParticipantId) -> PathBuf {
        self.data_dir.join(KEY_DIRECTORY).join(format!(
            "{}-{}.key",
            session_id.as_uuid(),
            participant_id.as_uuid()
        ))
    }
}

fn verify_outbox_rows(
    rows: Vec<SqliteRow>,
    profile: &SessionProfile,
) -> Result<Vec<(ParticipantEvent, Option<RelayReceipt>)>, ClientError> {
    let mut verified = Vec::with_capacity(rows.len());
    let mut expected_sequence = 1_u64;
    let mut previous_hash = Hash32::ZERO;
    for row in rows {
        let sequence: i64 = row.try_get(0).map_err(map_sqlx)?;
        let event_id: String = row.try_get(1).map_err(map_sqlx)?;
        let stored_previous_hash: String = row.try_get(2).map_err(map_sqlx)?;
        let stored_event_hash: String = row.try_get(3).map_err(map_sqlx)?;
        let event_json: Vec<u8> = row.try_get(4).map_err(map_sqlx)?;
        let evidence_order: Option<i64> = row.try_get(5).map_err(map_sqlx)?;
        let receipt_json: Option<Vec<u8>> = row.try_get(6).map_err(map_sqlx)?;
        let event: ParticipantEvent =
            serde_json::from_slice(&event_json).map_err(|_| ClientError::CorruptEvidence)?;
        let stored_previous_hash = parse_hash(&stored_previous_hash)?;
        let stored_event_hash = parse_hash(&stored_event_hash)?;
        if sequence < 1
            || u64::try_from(sequence).ok() != Some(expected_sequence)
            || event.body.sequence != expected_sequence
            || event.body.session_id != profile.session_id
            || event.body.participant_id != profile.participant_id
            || uuid_string(event.body.event_id.as_uuid()) != event_id
            || event.body.previous_hash != previous_hash
            || stored_previous_hash != previous_hash
        {
            return Err(ClientError::CorruptEvidence);
        }
        event.verify_with_key(&profile.participant_public_key)?;
        let event_hash = event.event_hash()?;
        if event_hash != stored_event_hash {
            return Err(ClientError::CorruptEvidence);
        }
        let receipt = match (receipt_json, evidence_order) {
            (Some(value), Some(order)) => {
                let receipt: RelayReceipt =
                    serde_json::from_slice(&value).map_err(|_| ClientError::CorruptEvidence)?;
                receipt
                    .verify(&profile.relay_public_key)
                    .map_err(|_| ClientError::CorruptEvidence)?;
                if order <= 0
                    || u64::try_from(order).ok() != Some(receipt.evidence_order)
                    || receipt.session_id != profile.session_id
                    || receipt.participant_id != profile.participant_id
                    || receipt.event_id != event.body.event_id
                    || receipt.event_hash != event_hash
                {
                    return Err(ClientError::CorruptEvidence);
                }
                Some(receipt)
            }
            (None, None) => None,
            _ => {
                return Err(ClientError::CorruptEvidence);
            }
        };
        previous_hash = event_hash;
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(ClientError::CorruptEvidence)?;
        verified.push((event, receipt));
    }
    Ok(verified)
}

async fn verify_head(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session_id: SessionId,
    participant_id: ParticipantId,
    entries: &[(ParticipantEvent, Option<RelayReceipt>)],
) -> Result<(), ClientError> {
    let row = sqlx::query(
        "SELECT last_sequence, last_hash FROM participant_heads WHERE session_id = ?1 AND participant_id = ?2",
    )
    .bind(uuid_string(session_id.as_uuid()))
    .bind(uuid_string(participant_id.as_uuid()))
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_sqlx)?;
    let expected_sequence =
        i64::try_from(entries.len()).map_err(|_| ClientError::CorruptEvidence)?;
    let expected_hash = entries
        .last()
        .map(|(event, _)| event.event_hash())
        .transpose()?
        .unwrap_or(Hash32::ZERO);
    match row {
        Some(row) => {
            let sequence: i64 = row.try_get(0).map_err(map_sqlx)?;
            let hash: String = row.try_get(1).map_err(map_sqlx)?;
            if sequence != expected_sequence || parse_hash(&hash)? != expected_hash {
                return Err(ClientError::CorruptEvidence);
            }
        }
        None if expected_sequence == 0 => {}
        None => return Err(ClientError::CorruptEvidence),
    }
    Ok(())
}

fn parse_hash(value: &str) -> Result<Hash32, ClientError> {
    let decoded: [u8; 32] =
        equalturn_protocol::decode_hex(value).map_err(|_| ClientError::CorruptEvidence)?;
    Ok(Hash32::new(decoded))
}

async fn initialize_schema(pool: &sqlx::SqlitePool) -> Result<[u8; 16], ClientError> {
    let store_id = *Uuid::new_v4().as_bytes();
    let mut connection = pool.acquire().await.map_err(map_sqlx)?;
    let mut transaction = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(map_sqlx)?;
    sqlx::query(
        "CREATE TABLE store_metadata (singleton INTEGER PRIMARY KEY CHECK (singleton = 1), schema_version INTEGER NOT NULL, store_id BLOB NOT NULL CHECK (length(store_id) = 16))",
    )
    .execute(&mut *transaction)
    .await
    .map_err(map_sqlx)?;
    sqlx::query(
        "CREATE TABLE session_profiles (session_id TEXT NOT NULL, participant_id TEXT NOT NULL, profile_json BLOB NOT NULL, PRIMARY KEY (session_id, participant_id))",
    )
    .execute(&mut *transaction)
    .await
    .map_err(map_sqlx)?;
    sqlx::query(
        "CREATE TABLE participant_heads (session_id TEXT NOT NULL, participant_id TEXT NOT NULL, last_sequence INTEGER NOT NULL CHECK (last_sequence BETWEEN 1 AND 10000), last_hash TEXT NOT NULL, PRIMARY KEY (session_id, participant_id), FOREIGN KEY (session_id, participant_id) REFERENCES session_profiles(session_id, participant_id) ON DELETE RESTRICT)",
    )
    .execute(&mut *transaction)
    .await
    .map_err(map_sqlx)?;
    sqlx::query(
        "CREATE TABLE outbox (session_id TEXT NOT NULL, participant_id TEXT NOT NULL, sequence INTEGER NOT NULL CHECK (sequence BETWEEN 1 AND 10000), event_id TEXT NOT NULL, previous_hash TEXT NOT NULL, event_hash TEXT NOT NULL, event_json BLOB NOT NULL, evidence_order INTEGER CHECK (evidence_order IS NULL OR evidence_order > 0), receipt_json BLOB, PRIMARY KEY (session_id, participant_id, sequence), UNIQUE (session_id, event_id), UNIQUE (session_id, evidence_order), FOREIGN KEY (session_id, participant_id) REFERENCES session_profiles(session_id, participant_id) ON DELETE RESTRICT)",
    )
    .execute(&mut *transaction)
    .await
    .map_err(map_sqlx)?;
    sqlx::query("CREATE INDEX outbox_pending_order ON outbox(session_id, participant_id, sequence) WHERE receipt_json IS NULL")
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
    sqlx::query(
        "INSERT INTO store_metadata (singleton, schema_version, store_id) VALUES (1, ?1, ?2)",
    )
    .bind(SCHEMA_VERSION)
    .bind(store_id.as_slice())
    .execute(&mut *transaction)
    .await
    .map_err(map_sqlx)?;
    transaction.commit().await.map_err(map_sqlx)?;
    Ok(store_id)
}

async fn validate_existing_schema(pool: &sqlx::SqlitePool) -> Result<[u8; 16], ClientError> {
    let check: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(pool)
        .await
        .map_err(map_sqlx)?;
    if check != "ok" {
        return Err(ClientError::CorruptDatabase);
    }
    let table_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN ('store_metadata', 'session_profiles', 'participant_heads', 'outbox')",
    )
    .fetch_one(pool)
    .await
    .map_err(map_sqlx)?;
    if table_count != 4 {
        return Err(ClientError::CorruptDatabase);
    }
    let row =
        sqlx::query("SELECT schema_version, store_id FROM store_metadata WHERE singleton = 1")
            .fetch_optional(pool)
            .await
            .map_err(map_sqlx)?
            .ok_or(ClientError::CorruptDatabase)?;
    let version: i64 = row.try_get(0).map_err(map_sqlx)?;
    if version != SCHEMA_VERSION {
        return Err(ClientError::UnsupportedSchemaVersion);
    }
    let store_id: Vec<u8> = row.try_get(1).map_err(map_sqlx)?;
    let store_id: [u8; 16] = store_id
        .as_slice()
        .try_into()
        .map_err(|_| ClientError::CorruptDatabase)?;
    Ok(store_id)
}

fn validate_profile(profile: &SessionProfile) -> Result<(), ClientError> {
    if profile.session_id.is_nil()
        || !profile.session_id.is_v4()
        || profile.participant_id.is_nil()
        || !profile.participant_id.is_v4()
        || !valid_nickname(&profile.nickname)
    {
        return Err(ClientError::InvalidProfile);
    }
    Ok(())
}

fn valid_nickname(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
}

fn uuid_string(value: Uuid) -> String {
    value.hyphenated().to_string()
}

fn has_session_keys(directory: &Path) -> Result<bool, ClientError> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(ClientError::KeyIo(error)),
    };
    for entry in entries {
        let entry = entry.map_err(ClientError::KeyIo)?;
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "key")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn create_private_directory(directory: &Path) -> Result<(), ClientError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(directory).map_err(ClientError::KeyIo)?;
        let metadata = fs::symlink_metadata(directory).map_err(ClientError::KeyIo)?;
        let mode = metadata.permissions().mode() & 0o777;
        if !metadata.file_type().is_dir() || mode & 0o077 != 0 || mode & 0o700 != 0o700 {
            return Err(ClientError::InsecureKeyPermissions);
        }
    }
    #[cfg(not(unix))]
    fs::create_dir_all(directory).map_err(ClientError::KeyIo)?;
    Ok(())
}

fn prepare_data_directory(directory: &Path) -> Result<(), ClientError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        if !directory.exists() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(directory).map_err(ClientError::KeyIo)?;
        }
        let metadata = fs::symlink_metadata(directory).map_err(ClientError::KeyIo)?;
        let mode = metadata.permissions().mode() & 0o777;
        if !metadata.file_type().is_dir() || mode & 0o077 != 0 || mode & 0o700 != 0o700 {
            return Err(ClientError::InsecureDataDirectory);
        }
    }
    #[cfg(not(unix))]
    fs::create_dir_all(directory).map_err(ClientError::KeyIo)?;
    Ok(())
}

fn validate_key_permissions(metadata: &fs::Metadata) -> Result<(), ClientError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o077 != 0 || mode & 0o400 == 0 {
            return Err(ClientError::InsecureKeyPermissions);
        }
    }
    #[cfg(not(unix))]
    let _ = metadata;
    Ok(())
}

fn sync_parent_directory(path: &Path) -> Result<(), ClientError> {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(ClientError::KeyIo)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn map_sqlx(error: sqlx::Error) -> ClientError {
    if let sqlx::Error::Database(database_error) = &error {
        let code = database_error.code();
        let message = database_error.message().to_ascii_lowercase();
        if matches!(
            code.as_deref(),
            Some("11" | "26" | "SQLITE_CORRUPT" | "SQLITE_NOTADB")
        ) || message.contains("malformed")
            || message.contains("not a database")
            || message.contains("no such table")
            || message.contains("no such column")
        {
            return ClientError::CorruptDatabase;
        }
    }
    ClientError::Storage(error)
}
