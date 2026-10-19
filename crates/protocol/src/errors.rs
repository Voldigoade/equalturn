use thiserror::Error;

pub type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u16),
    #[error("identifier must not be nil")]
    NilIdentifier,
    #[error("identifier must be a canonical UUIDv4")]
    InvalidIdentifier,
    #[error("sequence number must be greater than zero")]
    InvalidSequence,
    #[error("timestamp must be greater than zero")]
    InvalidTimestamp,
    #[error("event payload exceeds the protocol limit")]
    PayloadTooLarge,
    #[error("nickname does not match the v1 ASCII syntax")]
    InvalidNickname,
    #[error("client metadata is invalid or too long")]
    InvalidClientMetadata,
    #[error("event target is not a valid lowercase DNS hostname")]
    InvalidTarget,
    #[error("public key is malformed")]
    InvalidPublicKey,
    #[error("signature is invalid")]
    InvalidSignature,
    #[error("operating-system cryptographic randomness is unavailable")]
    EntropyUnavailable,
    #[error("duplicate event identifier")]
    DuplicateEvent,
    #[error("duplicate participant sequence")]
    DuplicateSequence,
    #[error("participant sequence moved backwards")]
    SequenceRollback,
    #[error("event previous hash does not match the preceding event")]
    PreviousHashMismatch,
    #[error("event chain contains more events than the session limit")]
    TooManyEvents,
    #[error("event chain mixes sessions or participants")]
    WrongEventIdentity,
    #[error("invalid hexadecimal field")]
    MalformedHex,
    #[error("relay evidence order is invalid")]
    InvalidEvidenceOrder,
    #[error("manifest participant heads are not canonical")]
    InvalidManifestHeads,
    #[error("reconnect challenge is invalid")]
    InvalidChallenge,
    #[error("heartbeat is invalid")]
    InvalidHeartbeat,
    #[error("invitation code is invalid")]
    InvalidInvitation,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HexError {
    #[error("hex string has the wrong length")]
    WrongLength,
    #[error("hex string contains a non-lowercase-hex character")]
    InvalidCharacter,
}
