use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;

use equalturn_protocol::ProtocolError;

pub type Result<T> = std::result::Result<T, RelayError>;

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("unsupported protocol version")]
    UnsupportedVersion,
    #[error("invitation rejected")]
    InvitationRejected,
    #[error("session not found")]
    SessionNotFound,
    #[error("session conflict")]
    SessionConflict,
    #[error("nickname already exists in this session")]
    NicknameConflict,
    #[error("session has ended")]
    SessionEnded,
    #[error("event sequence or hash does not match the participant head")]
    ChainConflict,
    #[error("event is not allowed in the current session state")]
    InvalidLifecycle,
    #[error("authentication failed")]
    AuthenticationFailed,
    #[error("request is too large")]
    TooLarge,
    #[error("request is malformed")]
    InvalidRequest,
    #[error("server storage operation failed")]
    Storage(#[source] sqlx::Error),
    #[error("server cryptographic identity operation failed")]
    Identity(#[source] std::io::Error),
    #[error("operating-system cryptographic randomness is unavailable")]
    Randomness,
    #[error("protocol validation failed")]
    Protocol(#[from] ProtocolError),
    #[error("server time is unavailable")]
    Clock,
    #[error("session resource limit reached")]
    ResourceLimit,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

impl IntoResponse for RelayError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self {
            Self::UnsupportedVersion => (
                StatusCode::UPGRADE_REQUIRED,
                "unsupported_protocol_version",
                "supported protocol version is 1",
            ),
            Self::InvitationRejected => (
                StatusCode::UNAUTHORIZED,
                "invitation_rejected",
                "invitation is invalid, expired, full, or closed",
            ),
            Self::SessionNotFound => (
                StatusCode::NOT_FOUND,
                "session_not_found",
                "session was not found",
            ),
            Self::SessionConflict => (
                StatusCode::CONFLICT,
                "session_conflict",
                "session state conflicts with this request",
            ),
            Self::NicknameConflict => (
                StatusCode::CONFLICT,
                "nickname_taken",
                "nickname is already in use in this session",
            ),
            Self::SessionEnded => (StatusCode::GONE, "session_ended", "session has ended"),
            Self::ChainConflict => (
                StatusCode::CONFLICT,
                "event_chain_conflict",
                "event does not extend the accepted participant chain",
            ),
            Self::InvalidLifecycle => (
                StatusCode::CONFLICT,
                "invalid_session_lifecycle",
                "event is not allowed in the current session state",
            ),
            Self::AuthenticationFailed => (
                StatusCode::UNAUTHORIZED,
                "authentication_failed",
                "session authentication failed",
            ),
            Self::TooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "request_too_large",
                "request exceeds the protocol limit",
            ),
            Self::InvalidRequest | Self::Protocol(_) => (
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "request does not match the protocol",
            ),
            Self::Storage(_) | Self::Identity(_) | Self::Randomness | Self::Clock => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "relay could not complete the request",
            ),
            Self::ResourceLimit => (
                StatusCode::TOO_MANY_REQUESTS,
                "resource_limit",
                "session resource limit reached",
            ),
        };
        (status, Json(ErrorBody { code, message })).into_response()
    }
}

pub(crate) fn storage(error: sqlx::Error) -> RelayError {
    RelayError::Storage(error)
}
