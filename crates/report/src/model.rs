use equalturn_protocol::{
    EventId, Hash32, MAX_SESSION_EVIDENCE_ORDER, NoticeId, OsFamily, ParticipantEvent,
    ParticipantId, PublicKey, RelayNotice, RelayReceipt, SessionId, SessionManifest, fingerprint,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const REPORT_SCHEMA_VERSION: u16 = 1;
pub const MAX_REPORT_JSON_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_REPORT_PARTICIPANTS: usize = 8;
pub const MAX_REPORT_EVENTS: usize = 10_000;
pub const MAX_REPORT_LOCAL_EVENTS: usize = 10_000;
pub const MAX_REPORT_RECEIPTS: usize = 10_000;
pub const MAX_REPORT_NOTICES: usize = 100_000;
pub const MAX_REPORT_EVIDENCE_ORDER: u64 = MAX_SESSION_EVIDENCE_ORDER;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBundle {
    pub report_schema_version: u16,
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub participants: Vec<ParticipantSummary>,
    /// Participant-signed events presented as relay-accepted evidence.
    pub events: Vec<ParticipantEvent>,
    pub receipts: Vec<RelayReceipt>,
    pub notices: Vec<RelayNotice>,
    pub relay: RelayIdentity,
    /// Absent only for an interim/incomplete export; verification then reports a gap.
    pub manifest: Option<SessionManifest>,
    /// Signed events that remain local because the relay did not acknowledge them.
    pub local_only_events: Vec<ParticipantEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantSummary {
    pub participant_id: ParticipantId,
    pub nickname: String,
    pub public_key: PublicKey,
    pub key_fingerprint: Hash32,
    pub client_version: String,
    pub os_family: OsFamily,
    pub os_bucket: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayIdentity {
    pub public_key: PublicKey,
    pub key_fingerprint: Hash32,
}

impl RelayIdentity {
    pub fn new(public_key: PublicKey) -> Self {
        Self {
            key_fingerprint: fingerprint(&public_key),
            public_key,
        }
    }
}

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("report JSON is malformed")]
    Json(#[from] serde_json::Error),
    #[error("report exceeds the 64 MiB size limit")]
    ReportTooLarge,
    #[error("report JSON is not in canonical v1 form")]
    NoncanonicalJson,
}

pub(crate) fn valid_id(id: SessionId) -> bool {
    !id.is_nil() && id.is_v4()
}

pub(crate) fn valid_participant_id(id: ParticipantId) -> bool {
    !id.is_nil() && id.is_v4()
}

pub(crate) fn valid_event_id(id: EventId) -> bool {
    !id.is_nil() && id.is_v4()
}

pub(crate) fn valid_notice_id(id: NoticeId) -> bool {
    !id.is_nil() && id.is_v4()
}

pub(crate) fn valid_nickname(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
}

pub(crate) fn valid_client_metadata(client_version: &str, os_bucket: &str) -> bool {
    client_version.len() <= 64
        && client_version
            .bytes()
            .all(|byte| (0x20..=0x7e).contains(&byte))
        && os_bucket.len() <= 32
        && os_bucket
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}
