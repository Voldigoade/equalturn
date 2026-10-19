use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    EventId, InvitationCode, ParticipantEvent, ParticipantId, PublicKey, ReconnectChallenge,
    RelayNotice, RelayReceipt, SessionId, SessionManifest, SignatureBytes,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSessionRequest {
    pub protocol_version: u16,
    pub invitation_code: InvitationCode,
    pub creator_event: ParticipantEvent,
}

impl fmt::Debug for CreateSessionRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreateSessionRequest")
            .field("protocol_version", &self.protocol_version)
            .field("invitation_code", &"[REDACTED]")
            .field("creator_event_id", &self.creator_event.body.event_id)
            .field("creator_event_sequence", &self.creator_event.body.sequence)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveInvitationRequest {
    pub protocol_version: u16,
    pub invitation_code: InvitationCode,
}

impl fmt::Debug for ResolveInvitationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolveInvitationRequest")
            .field("protocol_version", &self.protocol_version)
            .field("invitation_code", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveInvitationResponse {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub invitation_expires_at_ms: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinSessionRequest {
    pub protocol_version: u16,
    pub invitation_code: InvitationCode,
    pub join_event: ParticipantEvent,
}

impl fmt::Debug for JoinSessionRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JoinSessionRequest")
            .field("protocol_version", &self.protocol_version)
            .field("invitation_code", &"[REDACTED]")
            .field("join_event_id", &self.join_event.body.event_id)
            .field("join_event_sequence", &self.join_event.body.sequence)
            .finish()
    }
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

#[cfg(test)]
mod tests {
    use super::{CreateSessionRequest, JoinSessionRequest, ResolveInvitationRequest};
    use crate::{EventPayload, InvitationCode, ParticipantEvent, SigningIdentity, SigningSeed};

    #[test]
    fn invitation_request_debug_does_not_reveal_the_bearer_code() {
        let invitation_code = "23456-789AB-CDEFG-HJKMN-PQRSTU";
        let resolve_request = ResolveInvitationRequest {
            protocol_version: 1,
            invitation_code: InvitationCode::parse(invitation_code).expect("valid invitation"),
        };
        assert_redacted(&resolve_request, invitation_code);
        let wire_json = serde_json::to_value(&resolve_request).expect("request serializes");
        assert_eq!(wire_json["invitation_code"], invitation_code);
        let decoded: ResolveInvitationRequest =
            serde_json::from_value(wire_json).expect("wire request remains compatible");
        assert_eq!(decoded.invitation_code.as_str(), invitation_code);

        let identity = SigningIdentity::from_seed(&SigningSeed::from_bytes([0x31; 32]));
        let lower_case_code = invitation_code.to_ascii_lowercase();
        let event = ParticipantEvent::sign(test_event_body(&lower_case_code), &identity)
            .expect("event signs");
        let create_request = CreateSessionRequest {
            protocol_version: 1,
            invitation_code: InvitationCode::parse(invitation_code).expect("valid invitation"),
            creator_event: event.clone(),
        };
        let join_request = JoinSessionRequest {
            protocol_version: 1,
            invitation_code: InvitationCode::parse(invitation_code).expect("valid invitation"),
            join_event: event,
        };
        assert_redacted(&create_request, invitation_code);
        assert_redacted(&join_request, invitation_code);
    }

    fn assert_redacted(value: &impl std::fmt::Debug, invitation_code: &str) {
        let rendered = format!("{value:?}");
        assert!(!rendered.contains(invitation_code));
        assert!(!rendered.contains(&invitation_code.to_ascii_lowercase()));
        assert!(rendered.contains("[REDACTED]"));
    }

    fn test_event_body(event_target: &str) -> crate::EventBody {
        crate::EventBody {
            protocol_version: 1,
            session_id: crate::SessionId::from_bytes([
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x40, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ]),
            participant_id: crate::ParticipantId::from_bytes([
                0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x46, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed,
                0xfe, 0x0f,
            ]),
            event_id: crate::EventId::from_bytes([
                0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x49, 0x88, 0xb7, 0x66, 0x55, 0x44, 0x33, 0x22,
                0x11, 0x00,
            ]),
            sequence: 1,
            previous_hash: crate::Hash32::ZERO,
            client_timestamp_ms: 1,
            payload: EventPayload::SyntheticAiAccessAttempt {
                target: Some(event_target.to_owned()),
            },
        }
    }
}
