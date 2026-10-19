use std::{collections::HashMap, time::Duration};

use bytes::Bytes;
use equalturn_protocol::{
    ClientWsMessage, CreateSessionRequest, CreateSessionResponse, EventPayload, Hash32,
    InvitationCode, JoinSessionRequest, JoinSessionResponse, ParticipantEvent, ParticipantId,
    PublicKey, ResolveInvitationRequest, ResolveInvitationResponse, ServerWsMessage, SessionId,
    SessionStatusResponse, SigningIdentity, SubmitEventResponse, fingerprint,
};
use equalturn_report::{EvidenceBundle, MAX_REPORT_EVIDENCE_ORDER};
use futures_util::{SinkExt, StreamExt};
use reqwest::{Method, Response, Url};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};
use url::Host;
use zeroize::Zeroizing;

use crate::persistence::{ClientError, SessionProfile};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_API_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_SMALL_RESPONSE_BYTES: usize = 32 * 1024;
const MAX_WEBSOCKET_FRAME_BYTES: usize = 32 * 1024;

#[derive(Debug, Error)]
pub enum RelayClientError {
    #[error("relay URL is invalid")]
    InvalidUrl,
    #[error("unencrypted relay transport is allowed only for explicit loopback development")]
    InsecureTransport,
    #[error("relay could not be reached")]
    Unavailable,
    #[error("relay rejected the request ({status}: {code})")]
    Rejected { status: u16, code: String },
    #[error("relay returned an oversized response")]
    ResponseTooLarge,
    #[error("relay response does not match protocol version 1")]
    ProtocolMismatch,
    #[error("relay response is malformed or inconsistent")]
    InvalidResponse,
    #[error("relay response JSON is invalid")]
    Json(#[from] serde_json::Error),
    #[error("invitation code is invalid")]
    InvalidInvitation,
    #[error("local session state could not be read or written")]
    Local(#[from] ClientError),
    #[error("session signing identity does not match the local profile")]
    IdentityMismatch,
    #[error("relay sent a malformed or unauthenticated reconnect challenge")]
    AuthenticationFailed,
}

#[derive(Clone)]
pub struct RelayClient {
    base_url: Url,
    http: reqwest::Client,
}

impl RelayClient {
    pub fn new(relay_url: &str, allow_insecure_loopback: bool) -> Result<Self, RelayClientError> {
        let mut base_url = Url::parse(relay_url).map_err(|_| RelayClientError::InvalidUrl)?;
        match base_url.scheme() {
            "https" => {}
            "http" if allow_insecure_loopback && is_loopback_url(&base_url) => {}
            "http" => return Err(RelayClientError::InsecureTransport),
            _ => return Err(RelayClientError::InvalidUrl),
        }
        if base_url.host().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
            || !matches!(base_url.path(), "/" | "")
        {
            return Err(RelayClientError::InvalidUrl);
        }
        base_url.set_path("/");
        let http = reqwest::Client::builder()
            .https_only(base_url.scheme() == "https")
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| RelayClientError::InvalidUrl)?;
        Ok(Self { base_url, http })
    }

    pub fn relay_url(&self) -> &Url {
        &self.base_url
    }

    pub async fn relay_identity(&self) -> Result<(PublicKey, Hash32), RelayClientError> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RelayIdentityResponse {
            public_key: PublicKey,
            fingerprint: Hash32,
        }

        let response: RelayIdentityResponse = self
            .request_json(Method::GET, "v1/relay-key", Option::<&()>::None)
            .await?;
        if response.fingerprint != fingerprint(&response.public_key) {
            return Err(RelayClientError::InvalidResponse);
        }
        Ok((response.public_key, response.fingerprint))
    }

    pub async fn resolve_invitation(
        &self,
        invitation_code: &str,
    ) -> Result<ResolveInvitationResponse, RelayClientError> {
        let invitation_code = InvitationCode::parse(invitation_code)
            .map_err(|_| RelayClientError::InvalidInvitation)?;
        let response: ResolveInvitationResponse = self
            .request_json(
                Method::POST,
                "v1/invitations/resolve",
                Some(&ResolveInvitationRequest {
                    protocol_version: 1,
                    invitation_code,
                }),
            )
            .await?;
        if response.protocol_version != 1 || !response.session_id.is_v4() {
            return Err(RelayClientError::ProtocolMismatch);
        }
        Ok(response)
    }

    pub async fn create_session(
        &self,
        request: &CreateSessionRequest,
        relay_public_key: &PublicKey,
    ) -> Result<CreateSessionResponse, RelayClientError> {
        let response: CreateSessionResponse = self
            .request_json(Method::POST, "v1/sessions", Some(request))
            .await?;
        if response.protocol_version != 1
            || !response.session_id.is_v4()
            || !response.participant_id.is_v4()
        {
            return Err(RelayClientError::ProtocolMismatch);
        }
        if response.relay_public_key != *relay_public_key {
            return Err(RelayClientError::InvalidResponse);
        }
        response
            .receipt
            .verify(relay_public_key)
            .map_err(|_| RelayClientError::InvalidResponse)?;
        if response.receipt.session_id != response.session_id
            || response.receipt.participant_id != response.participant_id
            || response.receipt.event_id != request.creator_event.body.event_id
            || response.receipt.event_hash
                != request
                    .creator_event
                    .event_hash()
                    .map_err(|_| RelayClientError::InvalidResponse)?
        {
            return Err(RelayClientError::InvalidResponse);
        }
        Ok(response)
    }

    pub async fn join_session(
        &self,
        request: &JoinSessionRequest,
        relay_public_key: &PublicKey,
    ) -> Result<JoinSessionResponse, RelayClientError> {
        let path = format!(
            "v1/sessions/{}/join",
            request.join_event.body.session_id.as_uuid()
        );
        let response: JoinSessionResponse = self
            .request_json(Method::POST, &path, Some(request))
            .await?;
        if response.protocol_version != 1
            || response.session_id != request.join_event.body.session_id
            || response.participant_id != request.join_event.body.participant_id
        {
            return Err(RelayClientError::ProtocolMismatch);
        }
        if response.relay_public_key != *relay_public_key {
            return Err(RelayClientError::InvalidResponse);
        }
        response
            .receipt
            .verify(relay_public_key)
            .map_err(|_| RelayClientError::InvalidResponse)?;
        if response.receipt.session_id != response.session_id
            || response.receipt.participant_id != response.participant_id
            || response.receipt.event_id != request.join_event.body.event_id
            || response.receipt.event_hash
                != request
                    .join_event
                    .event_hash()
                    .map_err(|_| RelayClientError::InvalidResponse)?
        {
            return Err(RelayClientError::InvalidResponse);
        }
        Ok(response)
    }

    pub async fn submit_event(
        &self,
        event: &ParticipantEvent,
        relay_public_key: &PublicKey,
    ) -> Result<SubmitEventResponse, RelayClientError> {
        let path = format!("v1/sessions/{}/events", event.body.session_id.as_uuid());
        let response: SubmitEventResponse =
            self.request_json(Method::POST, &path, Some(event)).await?;
        response
            .receipt
            .verify(relay_public_key)
            .map_err(|_| RelayClientError::InvalidResponse)?;
        if response.receipt.session_id != event.body.session_id
            || response.receipt.participant_id != event.body.participant_id
            || response.receipt.event_id != event.body.event_id
            || response.receipt.event_hash
                != event
                    .event_hash()
                    .map_err(|_| RelayClientError::InvalidResponse)?
        {
            return Err(RelayClientError::InvalidResponse);
        }
        Ok(response)
    }

    pub async fn session_status(
        &self,
        session_id: SessionId,
    ) -> Result<SessionStatusResponse, RelayClientError> {
        let path = format!("v1/sessions/{}/status", session_id.as_uuid());
        let response: SessionStatusResponse = self
            .request_json(Method::GET, &path, Option::<&()>::None)
            .await?;
        if response.protocol_version != 1 || response.session_id != session_id {
            return Err(RelayClientError::ProtocolMismatch);
        }
        Ok(response)
    }

    pub async fn session_report(
        &self,
        session_id: SessionId,
    ) -> Result<EvidenceBundle, RelayClientError> {
        let path = format!("v1/sessions/{}/report", session_id.as_uuid());
        let response = self
            .http
            .get(self.endpoint(&path)?)
            .send()
            .await
            .map_err(map_request_error)?;
        let bytes = checked_response_bytes(response, MAX_API_RESPONSE_BYTES).await?;
        EvidenceBundle::from_json(
            std::str::from_utf8(&bytes).map_err(|_| RelayClientError::InvalidResponse)?,
        )
        .map_err(|_| RelayClientError::InvalidResponse)
    }

    pub async fn connect_session_stream(
        &self,
        profile: &SessionProfile,
        identity: SigningIdentity,
        after_evidence_order: u64,
    ) -> Result<SessionStream, RelayClientError> {
        if identity.public_key() != profile.participant_public_key || after_evidence_order != 0 {
            return Err(RelayClientError::IdentityMismatch);
        }
        let mut websocket_url = self.endpoint(&format!(
            "v1/sessions/{}/participants/{}/stream",
            profile.session_id.as_uuid(),
            profile.participant_id.as_uuid()
        ))?;
        let websocket_scheme = match websocket_url.scheme() {
            "http" => "ws",
            "https" => "wss",
            _ => return Err(RelayClientError::InvalidUrl),
        };
        websocket_url
            .set_scheme(websocket_scheme)
            .map_err(|_| RelayClientError::InvalidUrl)?;
        let websocket_config = WebSocketConfig::default()
            .max_message_size(Some(MAX_WEBSOCKET_FRAME_BYTES))
            .max_frame_size(Some(MAX_WEBSOCKET_FRAME_BYTES))
            .max_write_buffer_size(MAX_WEBSOCKET_FRAME_BYTES * 8);
        let (mut websocket, _) = timeout(
            REQUEST_TIMEOUT,
            connect_async_with_config(websocket_url.as_str(), Some(websocket_config), false),
        )
        .await
        .map_err(|_| RelayClientError::Unavailable)?
        .map_err(|_| RelayClientError::Unavailable)?;
        let challenge_message = timeout(REQUEST_TIMEOUT, websocket.next())
            .await
            .map_err(|_| RelayClientError::AuthenticationFailed)?
            .ok_or(RelayClientError::AuthenticationFailed)?
            .map_err(|_| RelayClientError::AuthenticationFailed)?;
        let Message::Text(challenge_json) = challenge_message else {
            return Err(RelayClientError::AuthenticationFailed);
        };
        if challenge_json.len() > MAX_WEBSOCKET_FRAME_BYTES {
            return Err(RelayClientError::ResponseTooLarge);
        }
        let challenge_message: ServerWsMessage = serde_json::from_str(&challenge_json)?;
        let ServerWsMessage::Challenge { challenge, .. } = challenge_message else {
            return Err(RelayClientError::AuthenticationFailed);
        };
        if challenge.protocol_version != 1
            || challenge.session_id != profile.session_id
            || challenge.participant_id != profile.participant_id
        {
            return Err(RelayClientError::ProtocolMismatch);
        }
        let signature = challenge
            .sign(&identity)
            .map_err(|_| RelayClientError::AuthenticationFailed)?;
        let authenticate = ClientWsMessage::Authenticate {
            challenge_id: challenge.challenge_id,
            signature,
            after_evidence_order,
        };
        websocket
            .send(Message::Text(serde_json::to_string(&authenticate)?.into()))
            .await
            .map_err(|_| RelayClientError::Unavailable)?;
        let authenticated_message = timeout(REQUEST_TIMEOUT, websocket.next())
            .await
            .map_err(|_| RelayClientError::AuthenticationFailed)?
            .ok_or(RelayClientError::AuthenticationFailed)?
            .map_err(|_| RelayClientError::AuthenticationFailed)?;
        let Message::Text(authenticated_json) = authenticated_message else {
            return Err(RelayClientError::AuthenticationFailed);
        };
        if authenticated_json.len() > MAX_WEBSOCKET_FRAME_BYTES {
            return Err(RelayClientError::ResponseTooLarge);
        }
        let authenticated_message: ServerWsMessage = serde_json::from_str(&authenticated_json)?;
        let ServerWsMessage::Authenticated {
            connection_generation,
            current_evidence_order,
        } = authenticated_message
        else {
            return Err(RelayClientError::AuthenticationFailed);
        };
        if connection_generation == 0 || current_evidence_order > MAX_REPORT_EVIDENCE_ORDER {
            return Err(RelayClientError::AuthenticationFailed);
        }
        Ok(SessionStream {
            websocket,
            session_id: profile.session_id,
            participant_id: profile.participant_id,
            identity,
            relay_public_key: profile.relay_public_key,
            connection_generation,
            heartbeat_counter: 0,
            last_heartbeat_accepted: 0,
            cursor: 0,
            member_keys: HashMap::new(),
            participant_heads: HashMap::new(),
            event_ids: std::collections::HashSet::new(),
        })
    }

    async fn request_json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<T, RelayClientError> {
        let mut request = self.http.request(method, self.endpoint(path)?);
        if let Some(body) = body {
            let body = Zeroizing::new(serde_json::to_vec(body)?);
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(Bytes::from_owner(body));
        }
        let response = request.send().await.map_err(map_request_error)?;
        let bytes = checked_response_bytes(response, MAX_SMALL_RESPONSE_BYTES).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn endpoint(&self, path: &str) -> Result<Url, RelayClientError> {
        self.base_url
            .join(path)
            .map_err(|_| RelayClientError::InvalidUrl)
    }
}

pub struct SessionStream {
    websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    session_id: SessionId,
    participant_id: ParticipantId,
    identity: SigningIdentity,
    relay_public_key: PublicKey,
    connection_generation: u64,
    heartbeat_counter: u64,
    last_heartbeat_accepted: u64,
    cursor: u64,
    member_keys: HashMap<ParticipantId, PublicKey>,
    participant_heads: HashMap<ParticipantId, (u64, Hash32)>,
    event_ids: std::collections::HashSet<equalturn_protocol::EventId>,
}

impl SessionStream {
    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    pub async fn next_message(&mut self) -> Result<ServerWsMessage, RelayClientError> {
        loop {
            let message = self
                .websocket
                .next()
                .await
                .ok_or(RelayClientError::Unavailable)?
                .map_err(|_| RelayClientError::Unavailable)?;
            let text = match message {
                Message::Text(text) => text,
                Message::Ping(payload) => {
                    self.websocket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|_| RelayClientError::Unavailable)?;
                    continue;
                }
                Message::Pong(_) => continue,
                Message::Close(_) | Message::Binary(_) | Message::Frame(_) => {
                    return Err(RelayClientError::AuthenticationFailed);
                }
            };
            if text.len() > MAX_WEBSOCKET_FRAME_BYTES {
                return Err(RelayClientError::ResponseTooLarge);
            }
            let message: ServerWsMessage = serde_json::from_str(&text)?;
            match &message {
                ServerWsMessage::Event { event, receipt } => {
                    if event.body.session_id != self.session_id
                        || event.body.participant_id != receipt.participant_id
                        || receipt.session_id != self.session_id
                        || receipt.event_id != event.body.event_id
                        || receipt.event_hash
                            != event
                                .event_hash()
                                .map_err(|_| RelayClientError::InvalidResponse)?
                        || receipt.evidence_order != self.cursor.saturating_add(1)
                    {
                        return Err(RelayClientError::InvalidResponse);
                    }
                    receipt
                        .verify(&self.relay_public_key)
                        .map_err(|_| RelayClientError::InvalidResponse)?;
                    let participant_id = event.body.participant_id;
                    let public_key = match &event.body.payload {
                        EventPayload::ParticipantJoined { public_key, .. }
                            if event.body.sequence == 1
                                && !self.member_keys.contains_key(&participant_id) =>
                        {
                            *public_key
                        }
                        EventPayload::ParticipantJoined { .. } => {
                            return Err(RelayClientError::InvalidResponse);
                        }
                        _ => *self
                            .member_keys
                            .get(&participant_id)
                            .ok_or(RelayClientError::InvalidResponse)?,
                    };
                    event
                        .verify_with_key(&public_key)
                        .map_err(|_| RelayClientError::InvalidResponse)?;
                    let expected = self
                        .participant_heads
                        .get(&participant_id)
                        .map_or(1, |(sequence, _)| sequence.saturating_add(1));
                    let expected_previous = self
                        .participant_heads
                        .get(&participant_id)
                        .map_or(Hash32::ZERO, |(_, hash)| *hash);
                    let event_hash = event
                        .event_hash()
                        .map_err(|_| RelayClientError::InvalidResponse)?;
                    if event.body.sequence != expected
                        || event.body.previous_hash != expected_previous
                        || !self.event_ids.insert(event.body.event_id)
                    {
                        return Err(RelayClientError::InvalidResponse);
                    }
                    self.member_keys.insert(participant_id, public_key);
                    self.participant_heads
                        .insert(participant_id, (event.body.sequence, event_hash));
                    self.cursor = receipt.evidence_order;
                }
                ServerWsMessage::Notice { notice } => {
                    if notice.session_id != self.session_id
                        || notice.evidence_order != self.cursor.saturating_add(1)
                    {
                        return Err(RelayClientError::InvalidResponse);
                    }
                    notice
                        .verify(&self.relay_public_key)
                        .map_err(|_| RelayClientError::InvalidResponse)?;
                    self.cursor = notice.evidence_order;
                }
                ServerWsMessage::FinalManifest { manifest } => {
                    if manifest.session_id != self.session_id
                        || manifest.relay_public_key != self.relay_public_key
                        || manifest.highest_evidence_order != self.cursor
                    {
                        return Err(RelayClientError::InvalidResponse);
                    }
                    manifest
                        .verify()
                        .map_err(|_| RelayClientError::InvalidResponse)?;
                    if !manifest_heads_match(manifest, &self.member_keys, &self.participant_heads) {
                        return Err(RelayClientError::InvalidResponse);
                    }
                }
                ServerWsMessage::Error { .. } => {
                    return Err(RelayClientError::AuthenticationFailed);
                }
                ServerWsMessage::Challenge { .. } | ServerWsMessage::Authenticated { .. } => {
                    return Err(RelayClientError::AuthenticationFailed);
                }
                ServerWsMessage::HeartbeatAccepted { heartbeat_counter } => {
                    if *heartbeat_counter <= self.last_heartbeat_accepted
                        || *heartbeat_counter > self.heartbeat_counter
                    {
                        return Err(RelayClientError::InvalidResponse);
                    }
                    self.last_heartbeat_accepted = *heartbeat_counter;
                }
            }
            return Ok(message);
        }
    }

    pub async fn send_heartbeat(
        &mut self,
        client_timestamp_ms: u64,
        last_event_sequence: u64,
        last_event_hash: Hash32,
    ) -> Result<(), RelayClientError> {
        self.heartbeat_counter = self
            .heartbeat_counter
            .checked_add(1)
            .ok_or(RelayClientError::AuthenticationFailed)?;
        let heartbeat = equalturn_protocol::Heartbeat::new_signed(
            self.session_id,
            self.participant_id,
            self.connection_generation,
            self.heartbeat_counter,
            client_timestamp_ms,
            last_event_sequence,
            last_event_hash,
            &self.identity,
        )
        .map_err(|_| RelayClientError::AuthenticationFailed)?;
        let request = ClientWsMessage::Heartbeat { heartbeat };
        self.websocket
            .send(Message::Text(serde_json::to_string(&request)?.into()))
            .await
            .map_err(|_| RelayClientError::Unavailable)
    }

    pub async fn close(mut self) -> Result<(), RelayClientError> {
        self.websocket
            .close(None)
            .await
            .map_err(|_| RelayClientError::Unavailable)
    }
}

fn is_loopback_url(url: &Url) -> bool {
    match url.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

fn map_request_error(_: reqwest::Error) -> RelayClientError {
    RelayClientError::Unavailable
}

async fn checked_response_bytes(
    response: Response,
    max_bytes: usize,
) -> Result<Vec<u8>, RelayClientError> {
    if !response.status().is_success() {
        let status = response.status();
        let bytes = read_response(response, 4 * 1024).await?;
        let code = serde_json::from_slice::<RemoteError>(&bytes)
            .ok()
            .map(|error| sanitize_error_code(&error.code))
            .unwrap_or_else(|| "relay_rejected".to_owned());
        return Err(RelayClientError::Rejected {
            status: status.as_u16(),
            code,
        });
    }
    read_response(response, max_bytes).await
}

async fn read_response(
    mut response: Response,
    max_bytes: usize,
) -> Result<Vec<u8>, RelayClientError> {
    if response
        .content_length()
        .is_some_and(|length| length > u64::try_from(max_bytes).unwrap_or(u64::MAX))
    {
        return Err(RelayClientError::ResponseTooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_request_error)? {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(RelayClientError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[derive(serde::Deserialize)]
struct RemoteError {
    code: String,
}

fn sanitize_error_code(value: &str) -> String {
    if !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
    {
        value.to_owned()
    } else {
        "relay_rejected".to_owned()
    }
}

fn manifest_heads_match(
    manifest: &equalturn_protocol::SessionManifest,
    member_keys: &HashMap<ParticipantId, PublicKey>,
    participant_heads: &HashMap<ParticipantId, (u64, Hash32)>,
) -> bool {
    manifest.participant_heads.len() == member_keys.len()
        && manifest.participant_heads.len() == participant_heads.len()
        && manifest.participant_heads.iter().all(|head| {
            member_keys.contains_key(&head.participant_id)
                && participant_heads.get(&head.participant_id)
                    == Some(&(head.final_sequence, head.final_hash))
        })
}

#[cfg(test)]
mod tests {
    use super::{
        RelayClient, RelayClientError, RemoteError, is_loopback_url, manifest_heads_match,
        sanitize_error_code,
    };
    use equalturn_protocol::{
        EventId, Hash32, ManifestHead, ParticipantId, SessionId, SessionManifest, SigningIdentity,
        SigningSeed,
    };
    use std::collections::HashMap;

    #[test]
    fn plaintext_is_restricted_to_explicit_loopback_urls() {
        assert!(RelayClient::new("http://127.0.0.1:8787", true).is_ok());
        assert!(RelayClient::new("http://localhost:8787", true).is_ok());
        assert!(matches!(
            RelayClient::new("http://example.org", true),
            Err(RelayClientError::InsecureTransport)
        ));
        assert!(RelayClient::new("http://127.0.0.1:8787", false).is_err());
        assert!(RelayClient::new("https://example.org", false).is_ok());
    }

    #[test]
    fn relay_url_rejects_embedded_credentials_and_redirect_surface() {
        assert!(RelayClient::new("https://user:pass@example.org", false).is_err());
        assert!(RelayClient::new("https://example.org/path", false).is_err());
        assert!(RelayClient::new("https://example.org?token=secret", false).is_err());
    }

    #[test]
    fn loopback_detection_uses_ip_semantics() {
        assert!(is_loopback_url(
            &url::Url::parse("http://[::1]:8787").expect("URL")
        ));
        assert!(!is_loopback_url(
            &url::Url::parse("http://192.0.2.1").expect("URL")
        ));
    }

    #[test]
    fn relay_error_codes_cannot_inject_output() {
        assert_eq!(
            sanitize_error_code("invitation_rejected"),
            "invitation_rejected"
        );
        assert_eq!(sanitize_error_code("secret token"), "relay_rejected");
    }

    #[test]
    fn relay_error_parser_accepts_the_documented_message_field() {
        let response = serde_json::from_str::<RemoteError>(
            r#"{"code":"nickname_taken","message":"nickname is already in use"}"#,
        )
        .expect("documented relay error body parses");
        assert_eq!(sanitize_error_code(&response.code), "nickname_taken");
    }

    #[test]
    fn signed_final_manifest_must_match_the_stream_heads() {
        let relay = SigningIdentity::from_seed(&SigningSeed::from_bytes([81; 32]));
        let participant = ParticipantId::try_new_v4().expect("participant ID");
        let signing_identity = SigningIdentity::from_seed(&SigningSeed::from_bytes([82; 32]));
        let member_keys = HashMap::from([(participant, signing_identity.public_key())]);
        let expected_hash = Hash32::new([83; 32]);
        let participant_heads = HashMap::from([(participant, (1, expected_hash))]);
        let manifest = SessionManifest::sign(
            SessionId::try_new_v4().expect("session ID"),
            EventId::try_new_v4().expect("end event ID"),
            Hash32::new([84; 32]),
            3,
            1,
            2,
            vec![ManifestHead {
                participant_id: participant,
                final_sequence: 1,
                final_hash: Hash32::new([85; 32]),
            }],
            &relay,
        )
        .expect("relay signs inconsistent manifest contents");

        assert!(manifest.verify().is_ok());
        assert!(!manifest_heads_match(
            &manifest,
            &member_keys,
            &participant_heads
        ));
    }
}
