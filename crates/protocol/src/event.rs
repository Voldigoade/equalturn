use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    EventId, Hash32, ParticipantId, ProtocolError, PublicKey, Result, SessionId, SignatureBytes,
    SigningIdentity, crypto::verify_signature,
};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_EVENT_PAYLOAD_BYTES: usize = 2_048;
pub const MAX_EVENT_BYTES: usize = 16 * 1_024;
pub const MAX_SESSION_EVENTS: u64 = 10_000;

const EVENT_DOMAIN: &[u8] = b"Equalturn\0event\0v1\0";
const EVENT_HASH_DOMAIN: &[u8] = b"Equalturn\0event-hash\0v1\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum OsFamily {
    Unknown = 0,
    Windows = 1,
    Linux = 2,
    Other = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum EndReason {
    ParticipantEnded = 1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventPayload {
    ParticipantJoined {
        nickname: String,
        public_key: PublicKey,
        client_version: String,
        os_family: OsFamily,
        os_bucket: String,
    },
    SessionStarted,
    SyntheticAiAccessAttempt {
        target: Option<String>,
    },
    SessionEnded {
        reason: EndReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventBody {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub participant_id: ParticipantId,
    pub event_id: EventId,
    pub sequence: u64,
    pub previous_hash: Hash32,
    pub client_timestamp_ms: u64,
    pub payload: EventPayload,
}

impl EventBody {
    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        self.validate_header()?;
        let payload = self.encode_payload()?;
        if payload.len() > MAX_EVENT_PAYLOAD_BYTES {
            return Err(ProtocolError::PayloadTooLarge);
        }

        let payload_len =
            u32::try_from(payload.len()).map_err(|_| ProtocolError::PayloadTooLarge)?;
        let mut bytes =
            Vec::with_capacity(EVENT_DOMAIN.len() + 2 + 16 * 3 + 8 * 2 + 32 + 4 + payload.len());
        bytes.extend_from_slice(EVENT_DOMAIN);
        bytes.extend_from_slice(&self.protocol_version.to_be_bytes());
        bytes.extend_from_slice(self.session_id.as_bytes());
        bytes.extend_from_slice(self.participant_id.as_bytes());
        bytes.extend_from_slice(self.event_id.as_bytes());
        bytes.extend_from_slice(&self.sequence.to_be_bytes());
        bytes.extend_from_slice(self.previous_hash.as_bytes());
        bytes.extend_from_slice(&self.client_timestamp_ms.to_be_bytes());
        bytes.extend_from_slice(&payload_len.to_be_bytes());
        bytes.extend_from_slice(&payload);
        if bytes.len() > MAX_EVENT_BYTES {
            return Err(ProtocolError::PayloadTooLarge);
        }
        Ok(bytes)
    }

    fn validate_header(&self) -> Result<()> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(self.protocol_version));
        }
        if self.session_id.is_nil() || self.participant_id.is_nil() || self.event_id.is_nil() {
            return Err(ProtocolError::NilIdentifier);
        }
        if !self.session_id.is_v4() || !self.participant_id.is_v4() || !self.event_id.is_v4() {
            return Err(ProtocolError::InvalidIdentifier);
        }
        if self.sequence == 0 || self.sequence > MAX_SESSION_EVENTS {
            return Err(ProtocolError::InvalidSequence);
        }
        if self.client_timestamp_ms == 0 {
            return Err(ProtocolError::InvalidTimestamp);
        }
        Ok(())
    }

    fn encode_payload(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        match &self.payload {
            EventPayload::ParticipantJoined {
                nickname,
                public_key,
                client_version,
                os_family,
                os_bucket,
            } => {
                validate_nickname(nickname)?;
                validate_client_metadata(client_version, os_bucket)?;
                bytes.push(1);
                encode_string_u8(&mut bytes, nickname)?;
                bytes.extend_from_slice(public_key.as_bytes());
                encode_string_u8(&mut bytes, client_version)?;
                bytes.push(*os_family as u8);
                encode_string_u8(&mut bytes, os_bucket)?;
            }
            EventPayload::SessionStarted => bytes.push(2),
            EventPayload::SyntheticAiAccessAttempt { target } => {
                bytes.push(3);
                if let Some(hostname) = target {
                    validate_hostname(hostname)?;
                    let length =
                        u8::try_from(hostname.len()).map_err(|_| ProtocolError::InvalidTarget)?;
                    bytes.push(length);
                    bytes.extend_from_slice(hostname.as_bytes());
                } else {
                    bytes.push(0);
                }
            }
            EventPayload::SessionEnded { reason } => {
                bytes.push(4);
                bytes.push(*reason as u8);
            }
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantEvent {
    pub body: EventBody,
    pub signature: SignatureBytes,
}

impl ParticipantEvent {
    pub fn sign(body: EventBody, identity: &SigningIdentity) -> Result<Self> {
        let signature = identity.sign(&body.signing_bytes()?);
        Ok(Self { body, signature })
    }

    pub fn verify_with_key(&self, public_key: &PublicKey) -> Result<()> {
        verify_signature(public_key, &self.signature, &self.body.signing_bytes()?)
    }

    pub fn event_hash(&self) -> Result<Hash32> {
        let mut digest = Sha256::new();
        digest.update(EVENT_HASH_DOMAIN);
        digest.update(self.body.signing_bytes()?);
        digest.update(self.signature.as_bytes());
        Ok(Hash32::new(digest.finalize().into()))
    }
}

fn validate_nickname(value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 32
        || !bytes[0].is_ascii_alphanumeric()
        || bytes[1..]
            .iter()
            .any(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_' && *byte != b'-')
    {
        return Err(ProtocolError::InvalidNickname);
    }
    Ok(())
}

fn validate_client_metadata(client_version: &str, os_bucket: &str) -> Result<()> {
    if client_version.len() > 64
        || !client_version
            .bytes()
            .all(|byte| (0x20..=0x7e).contains(&byte))
        || os_bucket.len() > 32
        || !os_bucket
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(ProtocolError::InvalidClientMetadata);
    }
    Ok(())
}

fn validate_hostname(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 253
        || value.ends_with('.')
        || value != value.to_ascii_lowercase()
        || value.parse::<std::net::IpAddr>().is_ok()
    {
        return Err(ProtocolError::InvalidTarget);
    }
    for label in value.split('.') {
        if label.is_empty()
            || label.len() > 63
            || !label.as_bytes()[0].is_ascii_alphanumeric()
            || !label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
            || label
                .bytes()
                .any(|byte| !byte.is_ascii_alphanumeric() && byte != b'-')
        {
            return Err(ProtocolError::InvalidTarget);
        }
    }
    Ok(())
}

fn encode_string_u8(bytes: &mut Vec<u8>, value: &str) -> Result<()> {
    let length = u8::try_from(value.len()).map_err(|_| ProtocolError::InvalidClientMetadata)?;
    bytes.push(length);
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}
