use std::{sync::Arc, time::Duration};

use axum::extract::ws::{Message, WebSocket};
use equalturn_protocol::{
    ChallengeId, ClientWsMessage, Hash32, LivenessState, ParticipantId, ReconnectChallenge,
    ServerWsMessage, SessionId,
};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    sync::oneshot,
    time::{Instant, interval_at, timeout},
};

use crate::{
    error::RelayError,
    liveness::{LivenessTransition, transition_for_elapsed},
    service::Relay,
};

const CHALLENGE_TTL: Duration = Duration::from_secs(15);
const HEARTBEAT_MIN_INTERVAL: Duration = Duration::from_secs(15);

pub(crate) async fn handle_upgrade(
    relay: Arc<Relay>,
    mut socket: WebSocket,
    session_id: SessionId,
    participant_id: ParticipantId,
) {
    if handle_socket(&relay, &mut socket, session_id, participant_id)
        .await
        .is_err()
    {
        let _ = socket.close().await;
    }
}

async fn handle_socket(
    relay: &Arc<Relay>,
    socket: &mut WebSocket,
    session_id: SessionId,
    participant_id: ParticipantId,
) -> Result<(), RelayError> {
    let _connection_permit = relay.try_connection_slot()?;
    let public_key = relay.challenge_member(session_id, participant_id).await?;
    let challenge = ReconnectChallenge {
        protocol_version: 1,
        challenge_id: ChallengeId::try_new_v4()?,
        nonce: random_nonce()?,
        session_id,
        participant_id,
    };
    send_json(
        socket,
        &ServerWsMessage::Challenge {
            challenge: challenge.clone(),
            expires_in_ms: 15_000,
        },
    )
    .await?;
    let auth_message = timeout(CHALLENGE_TTL, socket.recv())
        .await
        .map_err(|_| RelayError::AuthenticationFailed)?
        .ok_or(RelayError::AuthenticationFailed)?
        .map_err(|_| RelayError::AuthenticationFailed)?;
    let Message::Text(auth_text) = auth_message else {
        return Err(RelayError::AuthenticationFailed);
    };
    let auth: ClientWsMessage =
        serde_json::from_str(auth_text.as_str()).map_err(|_| RelayError::AuthenticationFailed)?;
    let ClientWsMessage::Authenticate {
        challenge_id,
        signature,
        after_evidence_order,
    } = auth
    else {
        return Err(RelayError::AuthenticationFailed);
    };
    if challenge_id != challenge.challenge_id {
        return Err(RelayError::AuthenticationFailed);
    }
    challenge
        .verify(&signature, &public_key)
        .map_err(|_| RelayError::AuthenticationFailed)?;
    let current_order = relay.current_evidence_order(session_id).await?;
    if after_evidence_order != 0 || after_evidence_order > current_order {
        return Err(RelayError::AuthenticationFailed);
    }

    let (close_sender, close_receiver) = oneshot::channel();
    let (generation, mut notifications) = relay
        .authenticate_connection(session_id, participant_id, close_sender)
        .await?;
    let result = handle_authenticated_stream(
        relay,
        socket,
        session_id,
        participant_id,
        generation,
        close_receiver,
        &mut notifications,
    )
    .await;
    relay
        .disconnect(session_id, participant_id, generation)
        .await;
    result
}

async fn handle_authenticated_stream(
    relay: &Arc<Relay>,
    socket: &mut WebSocket,
    session_id: SessionId,
    participant_id: ParticipantId,
    generation: u64,
    mut close_receiver: oneshot::Receiver<()>,
    notifications: &mut tokio::sync::broadcast::Receiver<()>,
) -> Result<(), RelayError> {
    let current_order = relay.current_evidence_order(session_id).await?;
    send_json(
        socket,
        &ServerWsMessage::Authenticated {
            connection_generation: generation,
            current_evidence_order: current_order,
        },
    )
    .await?;
    let (mut sink, mut stream) = socket.split();
    let mut cursor = 0;
    let ended = drain_replay(relay, session_id, &mut sink, &mut cursor).await?;
    if ended {
        return Ok(());
    }

    let mut last_heartbeat = Instant::now();
    let mut last_heartbeat_accepted = Instant::now();
    let mut liveness = LivenessState::Online;
    let mut liveness_tick = interval_at(
        Instant::now() + Duration::from_secs(5),
        Duration::from_secs(5),
    );
    loop {
        tokio::select! {
            _ = &mut close_receiver => break,
            item = stream.next() => {
                let Some(item) = item else { break; };
                let message = item.map_err(|_| RelayError::AuthenticationFailed)?;
                match message {
                    Message::Text(text) => {
                        let request: ClientWsMessage = serde_json::from_str(text.as_str())
                            .map_err(|_| RelayError::AuthenticationFailed)?;
                        let ClientWsMessage::Heartbeat { heartbeat } = request else {
                            return Err(RelayError::AuthenticationFailed);
                        };
                        if Instant::now().duration_since(last_heartbeat_accepted) < HEARTBEAT_MIN_INTERVAL {
                            return Err(RelayError::AuthenticationFailed);
                        }
                        if heartbeat.session_id != session_id
                            || heartbeat.participant_id != participant_id
                            || heartbeat.connection_generation != generation
                        {
                            return Err(RelayError::AuthenticationFailed);
                        }
                        relay.heartbeat(&heartbeat).await?;
                        last_heartbeat = Instant::now();
                        last_heartbeat_accepted = last_heartbeat;
                        liveness = LivenessState::Online;
                        send_json(
                            &mut sink,
                            &ServerWsMessage::HeartbeatAccepted {
                                heartbeat_counter: heartbeat.heartbeat_counter,
                            },
                        )
                        .await?;
                    }
                    Message::Ping(payload) => {
                        sink.send(Message::Pong(payload)).await.map_err(|_| RelayError::AuthenticationFailed)?;
                    }
                    Message::Pong(_) => {}
                    Message::Close(_) => break,
                    Message::Binary(_) => return Err(RelayError::AuthenticationFailed),
                }
            }
            notification = notifications.recv() => {
                match notification {
                    Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if drain_replay(relay, session_id, &mut sink, &mut cursor).await? {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = liveness_tick.tick() => {
                let transition = transition_for_elapsed(liveness, last_heartbeat.elapsed(), false);
                match transition {
                    LivenessTransition::None => {}
                    LivenessTransition::Stale => {
                        relay.set_liveness(session_id, participant_id, generation, LivenessState::Stale).await?;
                        liveness = LivenessState::Stale;
                    }
                    LivenessTransition::Disconnected => {
                        relay.set_liveness(session_id, participant_id, generation, LivenessState::Disconnected).await?;
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn drain_replay<S>(
    relay: &Relay,
    session_id: SessionId,
    sink: &mut S,
    cursor: &mut u64,
) -> Result<bool, RelayError>
where
    S: futures_util::Sink<Message> + Unpin,
{
    loop {
        let page = relay.replay_page(session_id, *cursor).await?;
        if page.is_empty() {
            let Some(manifest) = relay.final_manifest(session_id).await? else {
                return Ok(false);
            };
            if *cursor != manifest.highest_evidence_order {
                return Err(RelayError::InvalidRequest);
            }
            send_json(sink, &ServerWsMessage::FinalManifest { manifest }).await?;
            return Ok(true);
        }
        for (evidence_order, message) in page {
            if evidence_order <= *cursor {
                continue;
            }
            send_json(sink, &message).await?;
            *cursor = evidence_order;
        }
    }
}

async fn send_json<S, T>(sink: &mut S, value: &T) -> Result<(), RelayError>
where
    S: futures_util::Sink<Message> + Unpin,
    T: serde::Serialize,
{
    let text = serde_json::to_string(value).map_err(|_| RelayError::InvalidRequest)?;
    if text.len() > 32 * 1024 {
        return Err(RelayError::TooLarge);
    }
    sink.send(Message::Text(text.into()))
        .await
        .map_err(|_| RelayError::AuthenticationFailed)
}

fn random_nonce() -> Result<Hash32, RelayError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| RelayError::Randomness)?;
    Ok(Hash32::new(bytes))
}
