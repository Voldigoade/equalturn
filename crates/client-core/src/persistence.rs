#[cfg(not(unix))]
use std::fs::OpenOptions;
#[cfg(windows)]
use std::path::PathBuf;
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::Path,
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
use zeroize::Zeroizing;

const DATABASE_FILE: &str = "client.sqlite3";
const KEY_DIRECTORY: &str = "identities";
const LEGACY_KEY_MAGIC: &[u8; 8] = b"ETKEY\0\0\x01";
const KEY_MAGIC: &[u8; 8] = b"ETKEY\0\0\0";
const KEY_FORMAT_VERSION: u16 = 2;
#[cfg(unix)]
const KEY_SCHEME_UNIX_OWNER_ONLY: u8 = 1;
#[cfg(windows)]
const KEY_SCHEME_WINDOWS_DPAPI_CURRENT_USER: u8 = 2;
const KEY_HEADER_BYTES: usize = 8 + 2 + 1 + 16 + 16 + 16;
const KEY_MAX_PAYLOAD_BYTES: usize = 16 * 1024;
const KEY_MAX_FILE_BYTES: usize = KEY_HEADER_BYTES + KEY_MAX_PAYLOAD_BYTES;
const LEGACY_KEY_FILE_BYTES: usize = 8 + 16 + 16 + 16 + 32;
#[cfg(windows)]
const DPAPI_ENTROPY_DOMAIN: &[u8] = b"Equalturn\0client-key\0v2\0";
const SCHEMA_VERSION: i64 = 1;

#[cfg(not(any(unix, windows)))]
compile_error!("Equalturn participant key storage supports Windows and Unix targets only");

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
    #[error("platform key protection failed")]
    KeyProtection,
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
    #[cfg(windows)]
    data_dir: PathBuf,
    #[cfg(unix)]
    data_dir_handle: File,
    store_id: [u8; 16],
    write_gate: Mutex<()>,
}

impl ClientStore {
    pub async fn open(data_dir: impl AsRef<Path>) -> Result<Self, ClientError> {
        let data_dir = data_dir.as_ref().to_path_buf();
        #[cfg(unix)]
        let data_dir_handle = prepare_data_directory(&data_dir)?;
        #[cfg(not(unix))]
        prepare_data_directory(&data_dir)?;
        let database_path = data_dir.join(DATABASE_FILE);
        let database_exists = database_path.exists();
        let fresh_database = !database_exists
            || fs::metadata(&database_path)
                .map_err(ClientError::KeyIo)?
                .len()
                == 0;

        #[cfg(unix)]
        let keys_survive_database = fresh_database && has_session_keys(&data_dir_handle)?;
        #[cfg(not(unix))]
        let keys_survive_database =
            fresh_database && has_session_keys(&data_dir.join(KEY_DIRECTORY))?;
        if keys_survive_database {
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
            #[cfg(windows)]
            data_dir,
            #[cfg(unix)]
            data_dir_handle,
            store_id,
            write_gate: Mutex::new(()),
        })
    }

    /// Closes SQLite connections and waits until the database files are released.
    pub async fn close(self) {
        self.pool.close().await;
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
        #[cfg(not(unix))]
        let path = self.key_path(session_id, participant_id);
        #[cfg(unix)]
        let key_directory = open_identity_directory(&self.data_dir_handle, true)?;
        #[cfg(not(unix))]
        if let Some(parent) = path.parent() {
            create_private_directory(parent)?;
        }

        let seed = Zeroizing::new(identity.seed_bytes());
        let bytes = encode_key_envelope(&seed, session_id, participant_id, &self.store_id)?;

        #[cfg(unix)]
        persist_new_unix_key(
            &key_directory,
            &identity_key_name(session_id, participant_id),
            &bytes,
        )?;
        #[cfg(not(unix))]
        {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            let mut file = match options.open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    return Err(ClientError::ProfileConflict);
                }
                Err(error) => {
                    return Err(ClientError::KeyIo(error));
                }
            };
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(ClientError::KeyIo)?;
            sync_parent_directory(&path)?;
        }
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
        #[cfg(unix)]
        let (file, metadata) = {
            let directory = open_identity_directory(&self.data_dir_handle, false)?;
            let name = identity_key_name(session_id, participant_id);
            let descriptor = rustix::fs::openat(
                &directory,
                name.as_str(),
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .map_err(map_existing_key_open_error)?;
            let file = File::from(descriptor);
            let metadata = file.metadata().map_err(ClientError::KeyIo)?;
            (file, metadata)
        };
        #[cfg(windows)]
        let (file, metadata) = {
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
            let file = File::open(&path).map_err(ClientError::KeyIo)?;
            (file, metadata)
        };
        if !metadata.file_type().is_file() {
            return Err(ClientError::CorruptKey);
        }
        validate_key_permissions(&metadata)?;
        if !(LEGACY_KEY_FILE_BYTES as u64..=KEY_MAX_FILE_BYTES as u64).contains(&metadata.len()) {
            return Err(ClientError::CorruptKey);
        }

        let mut bytes = Zeroizing::new(Vec::with_capacity(KEY_MAX_FILE_BYTES));
        if let Err(error) = file
            .take((KEY_MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
        {
            return Err(ClientError::KeyIo(error));
        }
        if bytes.len() as u64 != metadata.len() || bytes.len() > KEY_MAX_FILE_BYTES {
            return Err(ClientError::CorruptKey);
        }
        let legacy = bytes.starts_with(LEGACY_KEY_MAGIC);
        let seed = if legacy {
            decode_legacy_key(&bytes, session_id, participant_id, &self.store_id)?
        } else {
            decode_key_envelope(&bytes, session_id, participant_id, &self.store_id)?
        };
        let identity = SigningIdentity::from_seed(*seed);
        if let Some(profile) = self.load_profile(session_id, participant_id).await?
            && identity.public_key() != profile.participant_public_key
        {
            return Err(ClientError::IdentityMismatch);
        }
        #[cfg(windows)]
        if legacy {
            let path = self.key_path(session_id, participant_id);
            let upgraded = encode_key_envelope(&seed, session_id, participant_id, &self.store_id)?;
            replace_legacy_key_file(&path, &upgraded)?;
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

    #[cfg(windows)]
    fn key_path(&self, session_id: SessionId, participant_id: ParticipantId) -> PathBuf {
        self.data_dir.join(KEY_DIRECTORY).join(format!(
            "{}-{}.key",
            session_id.as_uuid(),
            participant_id.as_uuid()
        ))
    }
}

fn encode_key_envelope(
    seed: &[u8; 32],
    session_id: SessionId,
    participant_id: ParticipantId,
    store_id: &[u8; 16],
) -> Result<Zeroizing<Vec<u8>>, ClientError> {
    #[cfg(windows)]
    let scheme = KEY_SCHEME_WINDOWS_DPAPI_CURRENT_USER;
    #[cfg(unix)]
    let scheme = KEY_SCHEME_UNIX_OWNER_ONLY;
    #[cfg(not(any(unix, windows)))]
    return Err(ClientError::KeyProtection);

    let mut envelope = Vec::with_capacity(KEY_HEADER_BYTES + 128);
    envelope.extend_from_slice(KEY_MAGIC);
    envelope.extend_from_slice(&KEY_FORMAT_VERSION.to_be_bytes());
    envelope.push(scheme);
    envelope.extend_from_slice(session_id.as_bytes());
    envelope.extend_from_slice(participant_id.as_bytes());
    envelope.extend_from_slice(store_id);

    let payload = {
        #[cfg(windows)]
        {
            let entropy = key_envelope_entropy(&envelope);
            Zeroizing::new(
                equalturn_key_protection::protect_seed(seed, &entropy)
                    .map_err(|_| ClientError::KeyProtection)?,
            )
        }
        #[cfg(unix)]
        {
            Zeroizing::new(seed.to_vec())
        }
    };
    if payload.is_empty() || payload.len() > KEY_MAX_PAYLOAD_BYTES {
        return Err(ClientError::KeyProtection);
    }
    envelope.extend_from_slice(&payload);
    Ok(Zeroizing::new(envelope))
}

fn decode_key_envelope(
    bytes: &[u8],
    session_id: SessionId,
    participant_id: ParticipantId,
    store_id: &[u8; 16],
) -> Result<Zeroizing<[u8; 32]>, ClientError> {
    if bytes.len() < KEY_HEADER_BYTES + 1
        || bytes.len() > KEY_MAX_FILE_BYTES
        || &bytes[..8] != KEY_MAGIC
        || u16::from_be_bytes([bytes[8], bytes[9]]) != KEY_FORMAT_VERSION
    {
        return Err(ClientError::CorruptKey);
    }
    if &bytes[11..27] != session_id.as_bytes() || &bytes[27..43] != participant_id.as_bytes() {
        return Err(ClientError::CorruptKey);
    }
    if bytes[43..59] != store_id[..] {
        return Err(ClientError::IdentityDiscontinuity);
    }

    let payload = &bytes[KEY_HEADER_BYTES..];
    if payload.is_empty() || payload.len() > KEY_MAX_PAYLOAD_BYTES {
        return Err(ClientError::CorruptKey);
    }
    match bytes[10] {
        #[cfg(unix)]
        KEY_SCHEME_UNIX_OWNER_ONLY if payload.len() == 32 => {
            let mut seed = Zeroizing::new([0_u8; 32]);
            seed.copy_from_slice(payload);
            Ok(seed)
        }
        #[cfg(windows)]
        KEY_SCHEME_WINDOWS_DPAPI_CURRENT_USER => {
            let entropy = key_envelope_entropy(&bytes[..KEY_HEADER_BYTES]);
            equalturn_key_protection::unprotect_seed(payload, &entropy)
                .map_err(|_| ClientError::CorruptKey)
        }
        _ => Err(ClientError::CorruptKey),
    }
}

fn decode_legacy_key(
    bytes: &[u8],
    session_id: SessionId,
    participant_id: ParticipantId,
    store_id: &[u8; 16],
) -> Result<Zeroizing<[u8; 32]>, ClientError> {
    if bytes.len() != LEGACY_KEY_FILE_BYTES || &bytes[..8] != LEGACY_KEY_MAGIC {
        return Err(ClientError::CorruptKey);
    }
    if &bytes[8..24] != session_id.as_bytes() || &bytes[24..40] != participant_id.as_bytes() {
        return Err(ClientError::CorruptKey);
    }
    if bytes[40..56] != store_id[..] {
        return Err(ClientError::IdentityDiscontinuity);
    }
    let mut seed = Zeroizing::new([0_u8; 32]);
    seed.copy_from_slice(&bytes[56..88]);
    Ok(seed)
}

#[cfg(windows)]
fn key_envelope_entropy(header: &[u8]) -> Zeroizing<Vec<u8>> {
    let mut entropy = Zeroizing::new(Vec::with_capacity(
        DPAPI_ENTROPY_DOMAIN.len() + header.len(),
    ));
    entropy.extend_from_slice(DPAPI_ENTROPY_DOMAIN);
    entropy.extend_from_slice(header);
    entropy
}

#[cfg(windows)]
fn replace_legacy_key_file(path: &Path, bytes: &[u8]) -> Result<(), ClientError> {
    let parent = path.parent().ok_or(ClientError::CorruptKey)?;
    let temporary = parent.join(format!(".key-upgrade-{}.tmp", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let result = (|| {
        let mut file = options.open(&temporary).map_err(ClientError::KeyIo)?;
        file.write_all(bytes).map_err(ClientError::KeyIo)?;
        file.sync_all().map_err(ClientError::KeyIo)?;
        fs::rename(&temporary, path).map_err(ClientError::KeyIo)?;
        Ok(())
    })();
    if result.is_err() {
        match fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(cleanup_error) => return Err(ClientError::KeyIo(cleanup_error)),
        }
    }
    result
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

#[cfg(not(unix))]
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

#[cfg(unix)]
fn has_session_keys(data_directory: &File) -> Result<bool, ClientError> {
    let directory = match open_identity_directory(data_directory, false) {
        Ok(directory) => directory,
        Err(ClientError::LostKey) => return Ok(false),
        Err(error) => return Err(error),
    };
    let entries = rustix::fs::Dir::read_from(&directory)
        .map_err(|error| ClientError::KeyIo(io::Error::from(error)))?;
    for entry in entries {
        let entry = entry.map_err(|error| ClientError::KeyIo(io::Error::from(error)))?;
        if entry.file_name().to_bytes().ends_with(b".key") {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(not(unix))]
fn create_private_directory(directory: &Path) -> Result<(), ClientError> {
    fs::create_dir_all(directory).map_err(ClientError::KeyIo)?;
    Ok(())
}

#[cfg(unix)]
fn prepare_data_directory(directory: &Path) -> Result<File, ClientError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    use std::path::PathBuf;

    let normalized_directory = directory
        .components()
        .fold(PathBuf::new(), |mut path, part| {
            path.push(part.as_os_str());
            path
        });
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder
        .create(&normalized_directory)
        .map_err(ClientError::KeyIo)?;
    let descriptor = rustix::fs::open(
        &normalized_directory,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| {
        if matches!(error, rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) {
            ClientError::InsecureDataDirectory
        } else {
            ClientError::KeyIo(io::Error::from(error))
        }
    })?;
    let directory = File::from(descriptor);
    let metadata = directory.metadata().map_err(ClientError::KeyIo)?;
    if !metadata.is_dir()
        || !unix_owner_and_mode_are_private(
            metadata.uid(),
            rustix::process::geteuid().as_raw(),
            metadata.mode(),
            0o700,
        )
    {
        return Err(ClientError::InsecureDataDirectory);
    }
    Ok(directory)
}

#[cfg(not(unix))]
fn prepare_data_directory(directory: &Path) -> Result<(), ClientError> {
    fs::create_dir_all(directory).map_err(ClientError::KeyIo)?;
    Ok(())
}

#[cfg(unix)]
fn open_identity_directory(data_directory: &File, create: bool) -> Result<File, ClientError> {
    use rustix::io::Errno;

    if create {
        match rustix::fs::mkdirat(data_directory, KEY_DIRECTORY, rustix::fs::Mode::from(0o700)) {
            Ok(()) => data_directory.sync_all().map_err(ClientError::KeyIo)?,
            Err(Errno::EXIST) => {}
            Err(error) => return Err(ClientError::KeyIo(io::Error::from(error))),
        }
    }

    let descriptor = rustix::fs::openat(
        data_directory,
        KEY_DIRECTORY,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| match error {
        Errno::NOENT if !create => ClientError::LostKey,
        Errno::LOOP | Errno::NOTDIR => ClientError::InsecureKeyPermissions,
        other => ClientError::KeyIo(io::Error::from(other)),
    })?;
    let directory = File::from(descriptor);
    let metadata = directory.metadata().map_err(ClientError::KeyIo)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if !metadata.is_dir()
            || !unix_owner_and_mode_are_private(
                metadata.uid(),
                rustix::process::geteuid().as_raw(),
                metadata.mode(),
                0o700,
            )
        {
            return Err(ClientError::InsecureKeyPermissions);
        }
    }
    Ok(directory)
}

#[cfg(unix)]
fn persist_new_unix_key(directory: &File, name: &str, bytes: &[u8]) -> Result<(), ClientError> {
    use rustix::io::Errno;

    let descriptor = rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::from(0o600),
    )
    .map_err(|error| {
        if error == Errno::EXIST {
            ClientError::ProfileConflict
        } else {
            ClientError::KeyIo(io::Error::from(error))
        }
    })?;
    let mut file = File::from(descriptor);
    let operation = rustix::fs::fchmod(&file, rustix::fs::Mode::from(0o600))
        .map_err(io::Error::from)
        .map_err(ClientError::KeyIo)
        .and_then(|()| {
            let metadata = file.metadata().map_err(ClientError::KeyIo)?;
            validate_key_permissions(&metadata)?;
            file.write_all(bytes).map_err(ClientError::KeyIo)?;
            file.sync_all().map_err(ClientError::KeyIo)?;
            directory.sync_all().map_err(ClientError::KeyIo)
        });
    if let Err(error) = operation {
        drop(file);
        let cleanup = rustix::fs::unlinkat(directory, name, rustix::fs::AtFlags::empty())
            .map_err(io::Error::from)
            .and_then(|()| directory.sync_all());
        return match cleanup {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(ClientError::KeyIo(cleanup_error)),
        };
    }
    Ok(())
}

#[cfg(unix)]
fn identity_key_name(session_id: SessionId, participant_id: ParticipantId) -> String {
    format!("{}-{}.key", session_id.as_uuid(), participant_id.as_uuid())
}

#[cfg(unix)]
fn map_existing_key_open_error(error: rustix::io::Errno) -> ClientError {
    match error {
        rustix::io::Errno::NOENT => ClientError::LostKey,
        rustix::io::Errno::LOOP | rustix::io::Errno::ISDIR => ClientError::CorruptKey,
        other => ClientError::KeyIo(io::Error::from(other)),
    }
}

#[cfg(unix)]
fn unix_owner_and_mode_are_private(
    owner: u32,
    expected_owner: u32,
    mode: u32,
    required_owner_bits: u32,
) -> bool {
    owner == expected_owner
        && mode & 0o7077 == 0
        && mode & required_owner_bits == required_owner_bits
}

fn validate_key_permissions(metadata: &fs::Metadata) -> Result<(), ClientError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if !unix_owner_and_mode_are_private(
            metadata.uid(),
            rustix::process::geteuid().as_raw(),
            metadata.mode(),
            0o400,
        ) {
            return Err(ClientError::InsecureKeyPermissions);
        }
    }
    #[cfg(not(unix))]
    let _ = metadata;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), ClientError> {
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

#[cfg(all(test, unix))]
mod unix_key_permission_tests {
    use super::unix_owner_and_mode_are_private;

    #[test]
    fn private_storage_requires_the_expected_owner_and_no_group_or_other_access() {
        assert!(unix_owner_and_mode_are_private(1000, 1000, 0o700, 0o700));
        assert!(!unix_owner_and_mode_are_private(1001, 1000, 0o700, 0o700));
        assert!(!unix_owner_and_mode_are_private(1000, 1000, 0o740, 0o700));
        assert!(!unix_owner_and_mode_are_private(1000, 1000, 0o4700, 0o700));
        assert!(!unix_owner_and_mode_are_private(1000, 1000, 0o600, 0o700));
    }
}
