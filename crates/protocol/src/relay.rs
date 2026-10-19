use serde::{Deserialize, Serialize};

use crate::{
    ChallengeId, EventId, Hash32, NoticeId, ParticipantId, ProtocolError, PublicKey, Result,
    SessionId, SignatureBytes, SigningIdentity, crypto::verify_signature,
    event::MAX_SESSION_EVENTS, fingerprint,
};

const RECEIPT_DOMAIN: &[u8] = b"Equalturn\0receipt\0v1\0";
const NOTICE_DOMAIN: &[u8] = b"Equalturn\0relay-notice\0v1\0";
const MANIFEST_DOMAIN: &[u8] = b"Equalturn\0session-manifest\0v1\0";
const RECONNECT_DOMAIN: &[u8] = b"Equalturn\0reconnect\0v1\0";
const HEARTBEAT_DOMAIN: &[u8] = b"Equalturn\0heartbeat\0v1\0";
pub const MAX_SESSION_EVIDENCE_ORDER: u64 = 110_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayReceipt {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub participant_id: ParticipantId,
    pub event_id: EventId,
    pub event_hash: Hash32,
    pub evidence_order: u64,
    pub relay_timestamp_ms: u64,
    pub signature: SignatureBytes,
}

impl RelayReceipt {
    pub fn sign(
        session_id: SessionId,
        participant_id: ParticipantId,
        event_id: EventId,
        event_hash: Hash32,
        evidence_order: u64,
        relay_timestamp_ms: u64,
        identity: &SigningIdentity,
    ) -> Result<Self> {
        let mut receipt = Self {
            protocol_version: 1,
            session_id,
            participant_id,
            event_id,
            event_hash,
            evidence_order,
            relay_timestamp_ms,
            signature: SignatureBytes::new([0; 64]),
        };
        receipt.signature = identity.sign(&receipt.signing_bytes()?);
        Ok(receipt)
    }

    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        validate_version_ids(self.protocol_version, self.session_id, self.participant_id)?;
        if !self.event_id.is_v4()
            || self.evidence_order == 0
            || self.evidence_order > MAX_SESSION_EVIDENCE_ORDER
            || self.relay_timestamp_ms == 0
        {
            return Err(ProtocolError::InvalidEvidenceOrder);
        }
        let mut bytes = Vec::with_capacity(RECEIPT_DOMAIN.len() + 2 + 16 * 3 + 32 + 16);
        bytes.extend_from_slice(RECEIPT_DOMAIN);
        bytes.extend_from_slice(&self.protocol_version.to_be_bytes());
        bytes.extend_from_slice(self.session_id.as_bytes());
        bytes.extend_from_slice(self.participant_id.as_bytes());
        bytes.extend_from_slice(self.event_id.as_bytes());
        bytes.extend_from_slice(self.event_hash.as_bytes());
        bytes.extend_from_slice(&self.evidence_order.to_be_bytes());
        bytes.extend_from_slice(&self.relay_timestamp_ms.to_be_bytes());
        Ok(bytes)
    }

    pub fn verify(&self, relay_key: &PublicKey) -> Result<()> {
        verify_signature(relay_key, &self.signature, &self.signing_bytes()?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum RelayNoticeKind {
    ParticipantOnline = 1,
    ParticipantStale = 2,
    ParticipantDisconnected = 3,
    SessionEnded = 4,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayNotice {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub notice_id: NoticeId,
    pub evidence_order: u64,
    pub relay_timestamp_ms: u64,
    pub kind: RelayNoticeKind,
    pub participant_id: Option<ParticipantId>,
    pub connection_generation: u64,
    pub heartbeat_counter: u64,
    pub signature: SignatureBytes,
}

impl RelayNotice {
    #[expect(
        clippy::too_many_arguments,
        reason = "arguments map directly to signed notice fields"
    )]
    pub fn sign(
        session_id: SessionId,
        evidence_order: u64,
        relay_timestamp_ms: u64,
        kind: RelayNoticeKind,
        participant_id: Option<ParticipantId>,
        connection_generation: u64,
        heartbeat_counter: u64,
        identity: &SigningIdentity,
    ) -> Result<Self> {
        let mut notice = Self {
            protocol_version: 1,
            session_id,
            notice_id: NoticeId::try_new_v4()?,
            evidence_order,
            relay_timestamp_ms,
            kind,
            participant_id,
            connection_generation,
            heartbeat_counter,
            signature: SignatureBytes::new([0; 64]),
        };
        notice.signature = identity.sign(&notice.signing_bytes()?);
        Ok(notice)
    }

    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        if self.protocol_version != 1 {
            return Err(ProtocolError::UnsupportedVersion(self.protocol_version));
        }
        if !self.session_id.is_v4() || !self.notice_id.is_v4() {
            return Err(ProtocolError::NilIdentifier);
        }
        if self.evidence_order == 0
            || self.evidence_order > MAX_SESSION_EVIDENCE_ORDER
            || self.relay_timestamp_ms == 0
        {
            return Err(ProtocolError::InvalidEvidenceOrder);
        }
        match (self.kind, self.participant_id) {
            (RelayNoticeKind::SessionEnded, None) => {}
            (RelayNoticeKind::SessionEnded, Some(_))
            | (RelayNoticeKind::ParticipantOnline, None)
            | (RelayNoticeKind::ParticipantStale, None)
            | (RelayNoticeKind::ParticipantDisconnected, None) => {
                return Err(ProtocolError::InvalidEvidenceOrder);
            }
            (_, Some(_)) if self.connection_generation == 0 => {
                return Err(ProtocolError::InvalidEvidenceOrder);
            }
            (_, Some(_)) => {}
        }
        let mut bytes =
            Vec::with_capacity(NOTICE_DOMAIN.len() + 2 + 16 + 16 + 8 + 8 + 1 + 1 + 16 + 16);
        bytes.extend_from_slice(NOTICE_DOMAIN);
        bytes.extend_from_slice(&self.protocol_version.to_be_bytes());
        bytes.extend_from_slice(self.session_id.as_bytes());
        bytes.extend_from_slice(self.notice_id.as_bytes());
        bytes.extend_from_slice(&self.evidence_order.to_be_bytes());
        bytes.extend_from_slice(&self.relay_timestamp_ms.to_be_bytes());
        bytes.push(self.kind as u8);
        match self.participant_id {
            Some(participant_id) if participant_id.is_v4() => {
                bytes.push(1);
                bytes.extend_from_slice(participant_id.as_bytes());
            }
            Some(_) => return Err(ProtocolError::NilIdentifier),
            None => bytes.push(0),
        }
        if self.participant_id.is_some() {
            bytes.extend_from_slice(&self.connection_generation.to_be_bytes());
            bytes.extend_from_slice(&self.heartbeat_counter.to_be_bytes());
        } else {
            if self.connection_generation != 0 || self.heartbeat_counter != 0 {
                return Err(ProtocolError::InvalidEvidenceOrder);
            }
            bytes.extend_from_slice(&0_u64.to_be_bytes());
            bytes.extend_from_slice(&0_u64.to_be_bytes());
        }
        Ok(bytes)
    }

    pub fn verify(&self, relay_key: &PublicKey) -> Result<()> {
        verify_signature(relay_key, &self.signature, &self.signing_bytes()?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestHead {
    pub participant_id: ParticipantId,
    pub final_sequence: u64,
    pub final_hash: Hash32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionManifest {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub end_event_id: EventId,
    pub end_event_hash: Hash32,
    pub highest_evidence_order: u64,
    pub start_relay_timestamp_ms: u64,
    pub end_relay_timestamp_ms: u64,
    pub participant_heads: Vec<ManifestHead>,
    pub relay_public_key: PublicKey,
    pub relay_key_fingerprint: Hash32,
    pub signature: SignatureBytes,
}

impl SessionManifest {
    #[expect(
        clippy::too_many_arguments,
        reason = "arguments map directly to signed manifest fields"
    )]
    pub fn sign(
        session_id: SessionId,
        end_event_id: EventId,
        end_event_hash: Hash32,
        highest_evidence_order: u64,
        start_relay_timestamp_ms: u64,
        end_relay_timestamp_ms: u64,
        participant_heads: Vec<ManifestHead>,
        identity: &SigningIdentity,
    ) -> Result<Self> {
        let public_key = identity.public_key();
        let mut manifest = Self {
            protocol_version: 1,
            session_id,
            end_event_id,
            end_event_hash,
            highest_evidence_order,
            start_relay_timestamp_ms,
            end_relay_timestamp_ms,
            participant_heads,
            relay_key_fingerprint: fingerprint(&public_key),
            relay_public_key: public_key,
            signature: SignatureBytes::new([0; 64]),
        };
        manifest.signature = identity.sign(&manifest.signing_bytes()?);
        Ok(manifest)
    }

    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        if self.protocol_version != 1 {
            return Err(ProtocolError::UnsupportedVersion(self.protocol_version));
        }
        if !self.session_id.is_v4() || !self.end_event_id.is_v4() {
            return Err(ProtocolError::NilIdentifier);
        }
        if self.highest_evidence_order == 0
            || self.highest_evidence_order > MAX_SESSION_EVIDENCE_ORDER
            || self.start_relay_timestamp_ms == 0
            || self.end_relay_timestamp_ms < self.start_relay_timestamp_ms
            || self.participant_heads.is_empty()
            || self.participant_heads.len() > 8
        {
            return Err(ProtocolError::InvalidManifestHeads);
        }
        if self.relay_key_fingerprint != fingerprint(&self.relay_public_key) {
            return Err(ProtocolError::InvalidManifestHeads);
        }
        let mut previous_id = None;
        let mut bytes = Vec::with_capacity(
            MANIFEST_DOMAIN.len() + 2 + 16 + 16 + 32 + 24 + 1 + self.participant_heads.len() * 56,
        );
        bytes.extend_from_slice(MANIFEST_DOMAIN);
        bytes.extend_from_slice(&self.protocol_version.to_be_bytes());
        bytes.extend_from_slice(self.session_id.as_bytes());
        bytes.extend_from_slice(self.end_event_id.as_bytes());
        bytes.extend_from_slice(self.end_event_hash.as_bytes());
        bytes.extend_from_slice(&self.highest_evidence_order.to_be_bytes());
        bytes.extend_from_slice(&self.start_relay_timestamp_ms.to_be_bytes());
        bytes.extend_from_slice(&self.end_relay_timestamp_ms.to_be_bytes());
        bytes.push(
            u8::try_from(self.participant_heads.len())
                .map_err(|_| ProtocolError::InvalidManifestHeads)?,
        );
        for head in &self.participant_heads {
            if !head.participant_id.is_v4() || head.final_sequence > MAX_SESSION_EVENTS {
                return Err(ProtocolError::InvalidManifestHeads);
            }
            if previous_id.is_some_and(|previous| previous >= head.participant_id) {
                return Err(ProtocolError::InvalidManifestHeads);
            }
            if head.final_sequence == 0 && head.final_hash != Hash32::ZERO {
                return Err(ProtocolError::InvalidManifestHeads);
            }
            previous_id = Some(head.participant_id);
            bytes.extend_from_slice(head.participant_id.as_bytes());
            bytes.extend_from_slice(&head.final_sequence.to_be_bytes());
            bytes.extend_from_slice(head.final_hash.as_bytes());
        }
        Ok(bytes)
    }

    pub fn verify(&self) -> Result<()> {
        if self.relay_key_fingerprint != fingerprint(&self.relay_public_key) {
            return Err(ProtocolError::InvalidManifestHeads);
        }
        verify_signature(
            &self.relay_public_key,
            &self.signature,
            &self.signing_bytes()?,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconnectChallenge {
    pub protocol_version: u16,
    pub challenge_id: ChallengeId,
    pub nonce: Hash32,
    pub session_id: SessionId,
    pub participant_id: ParticipantId,
}

impl ReconnectChallenge {
    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        validate_version_ids(self.protocol_version, self.session_id, self.participant_id)?;
        if !self.challenge_id.is_v4() || self.nonce == Hash32::ZERO {
            return Err(ProtocolError::InvalidChallenge);
        }
        let mut bytes = Vec::with_capacity(RECONNECT_DOMAIN.len() + 2 + 16 + 32 + 16 + 16);
        bytes.extend_from_slice(RECONNECT_DOMAIN);
        bytes.extend_from_slice(&self.protocol_version.to_be_bytes());
        bytes.extend_from_slice(self.challenge_id.as_bytes());
        bytes.extend_from_slice(self.nonce.as_bytes());
        bytes.extend_from_slice(self.session_id.as_bytes());
        bytes.extend_from_slice(self.participant_id.as_bytes());
        Ok(bytes)
    }

    pub fn sign(&self, identity: &SigningIdentity) -> Result<SignatureBytes> {
        Ok(identity.sign(&self.signing_bytes()?))
    }

    pub fn verify(&self, signature: &SignatureBytes, public_key: &PublicKey) -> Result<()> {
        verify_signature(public_key, signature, &self.signing_bytes()?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heartbeat {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub participant_id: ParticipantId,
    pub connection_generation: u64,
    pub heartbeat_counter: u64,
    pub client_timestamp_ms: u64,
    pub last_event_sequence: u64,
    pub last_event_hash: Hash32,
    pub signature: SignatureBytes,
}

impl Heartbeat {
    #[expect(
        clippy::too_many_arguments,
        reason = "arguments map directly to signed heartbeat fields"
    )]
    pub fn new_signed(
        session_id: SessionId,
        participant_id: ParticipantId,
        connection_generation: u64,
        heartbeat_counter: u64,
        client_timestamp_ms: u64,
        last_event_sequence: u64,
        last_event_hash: Hash32,
        identity: &SigningIdentity,
    ) -> Result<Self> {
        let mut heartbeat = Self {
            protocol_version: 1,
            session_id,
            participant_id,
            connection_generation,
            heartbeat_counter,
            client_timestamp_ms,
            last_event_sequence,
            last_event_hash,
            signature: SignatureBytes::new([0; 64]),
        };
        heartbeat.signature = identity.sign(&heartbeat.signing_bytes()?);
        Ok(heartbeat)
    }

    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        validate_version_ids(self.protocol_version, self.session_id, self.participant_id)?;
        if self.connection_generation == 0
            || self.heartbeat_counter == 0
            || self.client_timestamp_ms == 0
            || self.last_event_sequence > MAX_SESSION_EVENTS
            || (self.last_event_sequence == 0 && self.last_event_hash != Hash32::ZERO)
        {
            return Err(ProtocolError::InvalidHeartbeat);
        }
        let mut bytes = Vec::with_capacity(HEARTBEAT_DOMAIN.len() + 2 + 16 * 2 + 8 * 4 + 32);
        bytes.extend_from_slice(HEARTBEAT_DOMAIN);
        bytes.extend_from_slice(&self.protocol_version.to_be_bytes());
        bytes.extend_from_slice(self.session_id.as_bytes());
        bytes.extend_from_slice(self.participant_id.as_bytes());
        bytes.extend_from_slice(&self.connection_generation.to_be_bytes());
        bytes.extend_from_slice(&self.heartbeat_counter.to_be_bytes());
        bytes.extend_from_slice(&self.client_timestamp_ms.to_be_bytes());
        bytes.extend_from_slice(&self.last_event_sequence.to_be_bytes());
        bytes.extend_from_slice(self.last_event_hash.as_bytes());
        Ok(bytes)
    }

    pub fn verify(&self, public_key: &PublicKey) -> Result<()> {
        verify_signature(public_key, &self.signature, &self.signing_bytes()?)
    }
}

fn validate_version_ids(
    version: u16,
    session_id: SessionId,
    participant_id: ParticipantId,
) -> Result<()> {
    if version != 1 {
        return Err(ProtocolError::UnsupportedVersion(version));
    }
    if !session_id.is_v4() || !participant_id.is_v4() {
        return Err(ProtocolError::NilIdentifier);
    }
    Ok(())
}
