use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Path, State, WebSocketUpgrade, rejection::JsonRejection},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use equalturn_protocol::{
    CreateSessionRequest, CreateSessionResponse, JoinSessionRequest, JoinSessionResponse,
    ParticipantEvent, ResolveInvitationRequest, ResolveInvitationResponse, SessionId,
    SessionStatusResponse, SubmitEventResponse,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    error::{RelayError, Result},
    service::Relay,
    websocket::handle_upgrade,
};

const MAX_JSON_REQUEST_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug)]
pub struct TlsConfig {
    pub certificate_path: PathBuf,
    pub private_key_path: PathBuf,
}

pub fn router(relay: Arc<Relay>) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/relay-key", get(relay_key))
        .route("/v1/sessions", post(create_session))
        .route("/v1/invitations/resolve", post(resolve_invitation))
        .route("/v1/sessions/{session_id}/join", post(join_session))
        .route("/v1/sessions/{session_id}/events", post(submit_event))
        .route("/v1/sessions/{session_id}/status", get(session_status))
        .route("/v1/sessions/{session_id}/report", get(session_report))
        .route(
            "/v1/sessions/{session_id}/participants/{participant_id}/stream",
            get(session_stream),
        )
        .layer(DefaultBodyLimit::max(MAX_JSON_REQUEST_BYTES))
        .with_state(relay)
        .layer(axum::middleware::from_fn(limit_headers_and_methods))
}

pub async fn run_server(
    relay: Arc<Relay>,
    listen: SocketAddr,
    tls: Option<TlsConfig>,
    allow_insecure_loopback: bool,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if tls.is_none() && (!allow_insecure_loopback || !listen.ip().is_loopback()) {
        return Err(
            "plaintext relay transport is allowed only with explicit localhost development mode"
                .into(),
        );
    }
    let app = router(relay);
    match tls {
        Some(tls) => {
            let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
                tls.certificate_path,
                tls.private_key_path,
            )
            .await?;
            axum_server::bind_rustls(listen, config)
                .serve(app.into_make_service())
                .await?;
        }
        None => {
            let listener = tokio::net::TcpListener::bind(listen).await?;
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
    }
    Ok(())
}

async fn health() -> impl IntoResponse {
    #[derive(Serialize)]
    struct Health {
        status: &'static str,
        protocol_version: u16,
    }
    Json(Health {
        status: "ok",
        protocol_version: 1,
    })
}

async fn relay_key(State(relay): State<Arc<Relay>>) -> impl IntoResponse {
    let public_key = relay.relay_public_key();
    #[derive(Serialize)]
    struct RelayKeyResponse {
        public_key: equalturn_protocol::PublicKey,
        fingerprint: equalturn_protocol::Hash32,
    }
    Json(RelayKeyResponse {
        fingerprint: equalturn_protocol::fingerprint(&public_key),
        public_key,
    })
}

async fn create_session(
    State(relay): State<Arc<Relay>>,
    body: std::result::Result<Json<CreateSessionRequest>, JsonRejection>,
) -> Result<Json<CreateSessionResponse>> {
    let Json(request) = body.map_err(|_| RelayError::InvalidRequest)?;
    Ok(Json(relay.create_session(request).await?))
}

async fn resolve_invitation(
    State(relay): State<Arc<Relay>>,
    body: std::result::Result<Json<ResolveInvitationRequest>, JsonRejection>,
) -> Result<Json<ResolveInvitationResponse>> {
    let Json(request) = body.map_err(|_| RelayError::InvalidRequest)?;
    Ok(Json(relay.resolve_invitation(request).await?))
}

async fn join_session(
    State(relay): State<Arc<Relay>>,
    Path(session_id): Path<Uuid>,
    body: std::result::Result<Json<JoinSessionRequest>, JsonRejection>,
) -> Result<Json<JoinSessionResponse>> {
    let Json(request) = body.map_err(|_| RelayError::InvalidRequest)?;
    let session_id = session_id_from_uuid(session_id)?;
    if request.join_event.body.session_id != session_id {
        return Err(RelayError::InvalidRequest);
    }
    Ok(Json(relay.join_session(request).await?))
}

async fn submit_event(
    State(relay): State<Arc<Relay>>,
    Path(session_id): Path<Uuid>,
    body: std::result::Result<Json<ParticipantEvent>, JsonRejection>,
) -> Result<Json<SubmitEventResponse>> {
    let Json(event) = body.map_err(|_| RelayError::InvalidRequest)?;
    let session_id = session_id_from_uuid(session_id)?;
    if event.body.session_id != session_id {
        return Err(RelayError::InvalidRequest);
    }
    let accepted = relay.submit_event(event).await?;
    Ok(Json(SubmitEventResponse {
        receipt: accepted.receipt,
    }))
}

async fn session_status(
    State(relay): State<Arc<Relay>>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStatusResponse>> {
    Ok(Json(relay.status(session_id_from_uuid(session_id)?).await?))
}

async fn session_report(
    State(relay): State<Arc<Relay>>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<equalturn_report::EvidenceBundle>> {
    Ok(Json(relay.report(session_id_from_uuid(session_id)?).await?))
}

async fn session_stream(
    State(relay): State<Arc<Relay>>,
    Path((session_id, participant_id)): Path<(Uuid, Uuid)>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Ok(session_id) = session_id_from_uuid(session_id) else {
        return (StatusCode::BAD_REQUEST, "invalid session identifier").into_response();
    };
    let Ok(participant_id) = participant_id_from_uuid(participant_id) else {
        return (StatusCode::BAD_REQUEST, "invalid participant identifier").into_response();
    };
    upgrade
        .max_message_size(MAX_JSON_REQUEST_BYTES)
        .max_frame_size(MAX_JSON_REQUEST_BYTES)
        .on_upgrade(move |socket| handle_upgrade(relay, socket, session_id, participant_id))
        .into_response()
}

async fn limit_headers_and_methods(
    request: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    let allowed = matches!(
        request.method().as_str(),
        "GET" | "POST" | "HEAD" | "OPTIONS"
    );
    if !allowed {
        return (StatusCode::METHOD_NOT_ALLOWED, "method not allowed").into_response();
    }
    if request
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_JSON_REQUEST_BYTES)
    {
        return RelayError::TooLarge.into_response();
    }
    next.run(request).await
}

fn session_id_from_uuid(value: Uuid) -> Result<SessionId> {
    let id = SessionId::from(value);
    if !id.is_v4() {
        return Err(RelayError::InvalidRequest);
    }
    Ok(id)
}

fn participant_id_from_uuid(value: Uuid) -> Result<equalturn_protocol::ParticipantId> {
    let id = equalturn_protocol::ParticipantId::from(value);
    if !id.is_v4() {
        return Err(RelayError::InvalidRequest);
    }
    Ok(id)
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "could not listen for shutdown signal");
    }
}
