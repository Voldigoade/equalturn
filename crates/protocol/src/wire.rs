use serde::{Deserialize, Serialize};

use crate::{
    EventId, ParticipantEvent, ParticipantId, PublicKey, ReconnectChallenge, RelayNotice,
    RelayReceipt, SessionId, SessionManifest, SignatureBytes,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSessionRequest {
    pub protocol_version: u16,
    pub invitation_code: String,
    pub creator_event: ParticipantEvent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveInvitationRequest {
    pub protocol_version: u16,
    pub invitation_code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveInvitationResponse {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub invitation_expires_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinSessionRequest {
    pub protocol_version: u16,
    pub invitation_code: String,
    pub join_event: ParticipantEvent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSessionResponse {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub participant_id: ParticipantId,
    pub invitation_expires_at_ms: u64,
    pub receipt: RelayReceipt,
    pub relay_public_key: PublicKey,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinSessionResponse {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub participant_id: ParticipantId,
    pub receipt: RelayReceipt,
    pub relay_public_key: PublicKey,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitEventResponse {
    pub receipt: RelayReceipt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Created,
    Active,
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LivenessState {
    Online,
    Stale,
    Disconnected,
    Ended,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantView {
    pub participant_id: ParticipantId,
    pub nickname: String,
    pub public_key: PublicKey,
    pub key_fingerprint: crate::Hash32,
    pub liveness: LivenessState,
    pub last_heartbeat_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionStatusResponse {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub state: SessionState,
    pub created_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub participants: Vec<ParticipantView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientWsMessage {
    Authenticate {
        challenge_id: crate::ChallengeId,
        signature: SignatureBytes,
        after_evidence_order: u64,
    },
    Heartbeat {
        heartbeat: crate::Heartbeat,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerWsMessage {
    Challenge {
        challenge: ReconnectChallenge,
        expires_in_ms: u32,
    },
    Authenticated {
        connection_generation: u64,
        current_evidence_order: u64,
    },
    Event {
        event: ParticipantEvent,
        receipt: RelayReceipt,
    },
    Notice {
        notice: RelayNotice,
    },
    FinalManifest {
        manifest: SessionManifest,
    },
    Error {
        code: String,
        message: String,
    },
    HeartbeatAccepted {
        heartbeat_counter: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionReportEnvelope {
    pub session_id: SessionId,
    pub manifest: SessionManifest,
    pub end_event_id: EventId,
}
