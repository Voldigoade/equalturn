use std::time::Duration;

use equalturn_client_core::{RelayClientError, SessionProfile};
use equalturn_protocol::{
    CreateSessionRequest, EndReason, EventPayload, InvitationCode, JoinSessionRequest,
    ParticipantEvent, ParticipantId, ServerWsMessage, SessionId, SessionState,
};
use tokio::time::Instant;

use crate::{
    app::{
        AppError, Context, client_metadata, parse_session_id, select_profile, unix_time_ms,
        validate_nickname,
    },
    cli::{CreateArgs, JoinArgs, LocalSessionArgs, RemoteSessionArgs, SessionCommand},
};

pub async fn run(context: &Context, command: SessionCommand) -> Result<(), AppError> {
    match command {
        SessionCommand::Create(args) => create(context, args).await,
        SessionCommand::Join(args) => join(context, args).await,
        SessionCommand::Start(args) => submit_lifecycle_event(context, args, true).await,
        SessionCommand::Status(args) => status(context, args).await,
        SessionCommand::Participants(args) => participants(context, args).await,
        SessionCommand::Watch(args) => watch(context, args).await,
        SessionCommand::Sync(args) => sync(context, args).await,
        SessionCommand::End(args) => submit_lifecycle_event(context, args, false).await,
    }
}

async fn create(context: &Context, args: CreateArgs) -> Result<(), AppError> {
    let (session_id, profile, invitation_code, event) = if let Some(session) = args.resume_session {
        let session_id = parse_session_id(&session)?;
        let invitation_code =
            InvitationCode::parse(args.invitation_code.as_deref().ok_or_else(|| {
                AppError::Message("--invitation-code is required to resume".into())
            })?)?;
        let profile = select_profile(&context.store, session_id, None).await?;
        let event = context
            .store
            .pending_events(session_id, profile.participant_id)
            .await?
            .into_iter()
            .find(|event| {
                event.body.sequence == 1
                    && matches!(event.body.payload, EventPayload::ParticipantJoined { .. })
            })
            .ok_or_else(|| {
                AppError::Message(
                    "there is no unacknowledged creator event to resume; check session status"
                        .into(),
                )
            })?;
        (session_id, profile, invitation_code, event)
    } else {
        let nickname = args
            .nickname
            .as_deref()
            .ok_or_else(|| AppError::Message("--nickname is required".into()))?;
        validate_nickname(nickname)?;
        let (relay_public_key, relay_fingerprint) = context.relay.relay_identity().await?;
        let session_id = SessionId::try_new_v4()?;
        let participant_id = ParticipantId::try_new_v4()?;
        let identity = context
            .store
            .create_identity(session_id, participant_id)
            .await?;
        let profile = SessionProfile {
            session_id,
            participant_id,
            nickname: nickname.to_owned(),
            participant_public_key: identity.public_key(),
            relay_public_key,
        };
        context.store.save_profile(&profile).await?;
        let (os_family, os_bucket) = client_metadata();
        let event = context
            .store
            .enqueue_event(
                &profile,
                &identity,
                unix_time_ms()?,
                EventPayload::ParticipantJoined {
                    nickname: nickname.to_owned(),
                    public_key: identity.public_key(),
                    client_version: env!("CARGO_PKG_VERSION").to_owned(),
                    os_family,
                    os_bucket: os_bucket.to_owned(),
                },
            )
            .await?;
        let invitation_code = InvitationCode::generate()?;
        println!("Relay key fingerprint: {}", relay_fingerprint.to_hex());
        (session_id, profile, invitation_code, event)
    };

    let request = CreateSessionRequest {
        protocol_version: 1,
        invitation_code: invitation_code.as_str().to_owned(),
        creator_event: event,
    };
    match context
        .relay
        .create_session(&request, &profile.relay_public_key)
        .await
    {
        Ok(response) => {
            context.store.acknowledge(&response.receipt).await?;
            println!("Session created");
            println!("Code: {}", invitation_code.as_str());
            println!("Session: {}", session_id.as_uuid());
            println!("Participant: {}", profile.nickname);
            println!("Participant ID: {}", profile.participant_id.as_uuid());
            println!("Invitation expires in 10 minutes or when the session starts.");
            Ok(())
        }
        Err(error) => {
            eprintln!("Session creation was not confirmed by the relay.");
            eprintln!("Session: {}", session_id.as_uuid());
            eprintln!(
                "Invitation code to retain for retry: {}",
                invitation_code.as_str()
            );
            eprintln!(
                "Retry with: equalturn session create --resume-session {} --invitation-code <CODE>",
                session_id.as_uuid()
            );
            if matches!(error, RelayClientError::Unavailable) {
                Err(AppError::Queued(
                    "creator event is stored locally; retry the command above when the relay is reachable".into(),
                ))
            } else {
                Err(AppError::Relay(error))
            }
        }
    }
}

async fn join(context: &Context, args: JoinArgs) -> Result<(), AppError> {
    validate_nickname(&args.nickname)?;
    let (relay_public_key, relay_fingerprint) = context.relay.relay_identity().await?;
    let resolved = context
        .relay
        .resolve_invitation(&args.invitation_code)
        .await?;
    let session_id = resolved.session_id;
    let existing = context.store.profiles_for_session(session_id).await?;
    if let Some(profile) = existing
        .iter()
        .find(|profile| profile.nickname.eq_ignore_ascii_case(&args.nickname))
    {
        let pending = context
            .store
            .pending_events(session_id, profile.participant_id)
            .await?;
        let join_event = pending.into_iter().find(|event| {
            event.body.sequence == 1
                && matches!(event.body.payload, EventPayload::ParticipantJoined { .. })
        });
        if let Some(join_event) = join_event {
            return accept_join(context, profile, &args.invitation_code, join_event).await;
        }
        println!("Already joined session {}", profile.session_id.as_uuid());
        println!("Participant: {}", profile.nickname);
        println!("Participant ID: {}", profile.participant_id.as_uuid());
        return Ok(());
    }

    let participant_id = ParticipantId::try_new_v4()?;
    let identity = context
        .store
        .create_identity(session_id, participant_id)
        .await?;
    let profile = SessionProfile {
        session_id,
        participant_id,
        nickname: args.nickname.clone(),
        participant_public_key: identity.public_key(),
        relay_public_key,
    };
    context.store.save_profile(&profile).await?;
    let (os_family, os_bucket) = client_metadata();
    let join_event = context
        .store
        .enqueue_event(
            &profile,
            &identity,
            unix_time_ms()?,
            EventPayload::ParticipantJoined {
                nickname: args.nickname,
                public_key: identity.public_key(),
                client_version: env!("CARGO_PKG_VERSION").to_owned(),
                os_family,
                os_bucket: os_bucket.to_owned(),
            },
        )
        .await?;
    println!("Relay key fingerprint: {}", relay_fingerprint.to_hex());
    accept_join(context, &profile, &args.invitation_code, join_event).await
}

async fn accept_join(
    context: &Context,
    profile: &SessionProfile,
    invitation_code: &str,
    join_event: ParticipantEvent,
) -> Result<(), AppError> {
    let request = JoinSessionRequest {
        protocol_version: 1,
        invitation_code: invitation_code.to_owned(),
        join_event,
    };
    match context
        .relay
        .join_session(&request, &profile.relay_public_key)
        .await
    {
        Ok(response) => {
            context.store.acknowledge(&response.receipt).await?;
            println!("Joined session {}", profile.session_id.as_uuid());
            println!("Participant: {}", profile.nickname);
            println!("Participant ID: {}", profile.participant_id.as_uuid());
            Ok(())
        }
        Err(RelayClientError::Unavailable) => Err(AppError::Queued(
            "join event is stored locally; retry `equalturn session join <CODE> --nickname <NAME>` when the relay is reachable".into(),
        )),
        Err(error) => Err(AppError::Relay(error)),
    }
}

async fn submit_lifecycle_event(
    context: &Context,
    args: LocalSessionArgs,
    start: bool,
) -> Result<(), AppError> {
    let session_id = parse_session_id(&args.session)?;
    let profile = select_profile(&context.store, session_id, args.participant.as_deref()).await?;
    let identity = context
        .store
        .load_identity(session_id, profile.participant_id)
        .await?;
    let payload = if start {
        EventPayload::SessionStarted
    } else {
        EventPayload::SessionEnded {
            reason: EndReason::ParticipantEnded,
        }
    };
    let event = context
        .store
        .enqueue_event(&profile, &identity, unix_time_ms()?, payload)
        .await?;
    match post_event(context, &profile, &event).await {
        Ok(()) => {
            println!("Session {}", if start { "started" } else { "ended" });
            Ok(())
        }
        Err(RelayClientError::Unavailable) => Err(AppError::Queued(
            "signed lifecycle event is stored locally; reconnect and run `equalturn session sync --session <UUID>`".into(),
        )),
        Err(error) => Err(AppError::Relay(error)),
    }
}

async fn status(context: &Context, args: RemoteSessionArgs) -> Result<(), AppError> {
    let session_id = parse_session_id(&args.session)?;
    let status = context.relay.session_status(session_id).await?;
    println!("Session: {}", status.session_id.as_uuid());
    println!("State: {:?}", status.state);
    println!("Participants: {}", status.participants.len());
    for participant in status.participants {
        println!(
            "{}  {}  {:?}  key {}",
            participant.nickname,
            participant.participant_id.as_uuid(),
            participant.liveness,
            participant.key_fingerprint.to_hex()
        );
    }
    Ok(())
}

async fn participants(context: &Context, args: RemoteSessionArgs) -> Result<(), AppError> {
    let session_id = parse_session_id(&args.session)?;
    let status = context.relay.session_status(session_id).await?;
    for participant in status.participants {
        println!(
            "{}  {}  {:?}",
            participant.nickname,
            participant.participant_id.as_uuid(),
            participant.liveness
        );
    }
    Ok(())
}

async fn sync(context: &Context, args: LocalSessionArgs) -> Result<(), AppError> {
    let session_id = parse_session_id(&args.session)?;
    let profile = select_profile(&context.store, session_id, args.participant.as_deref()).await?;
    let count = flush_pending(context, &profile).await?;
    println!("Acknowledged {count} queued event(s).");
    Ok(())
}

pub async fn flush_pending(context: &Context, profile: &SessionProfile) -> Result<usize, AppError> {
    let events = context
        .store
        .pending_events(profile.session_id, profile.participant_id)
        .await?;
    let mut acknowledged = 0;
    for event in events {
        match event.body.payload {
            EventPayload::ParticipantJoined { .. } => {
                return Err(AppError::Message(
                    "the join event is not acknowledged; retry session create/join with its invitation code".into(),
                ));
            }
            _ => {
                post_event(context, profile, &event).await?;
                acknowledged += 1;
            }
        }
    }
    Ok(acknowledged)
}

pub async fn post_event(
    context: &Context,
    profile: &SessionProfile,
    event: &ParticipantEvent,
) -> Result<(), RelayClientError> {
    let response = context
        .relay
        .submit_event(event, &profile.relay_public_key)
        .await?;
    context
        .store
        .acknowledge(&response.receipt)
        .await
        .map_err(RelayClientError::from)
}

async fn watch(context: &Context, args: LocalSessionArgs) -> Result<(), AppError> {
    let session_id = parse_session_id(&args.session)?;
    let profile = select_profile(&context.store, session_id, args.participant.as_deref()).await?;
    let mut backoff = Duration::from_millis(500);
    loop {
        let status = context.relay.session_status(session_id).await?;
        if status.state == SessionState::Ended {
            println!(
                "Session has ended. Export its report with `equalturn report export --session {}`.",
                session_id.as_uuid()
            );
            return Ok(());
        }
        if !status
            .participants
            .iter()
            .any(|participant| participant.participant_id == profile.participant_id)
        {
            return Err(AppError::Message(
                "this local participant is not registered; retry its create or join command with the invitation code".into(),
            ));
        }
        let identity = context
            .store
            .load_identity(session_id, profile.participant_id)
            .await?;
        let mut stream = match context
            .relay
            .connect_session_stream(&profile, identity, 0)
            .await
        {
            Ok(stream) => stream,
            Err(RelayClientError::Unavailable) => {
                eprintln!("Relay unavailable; retrying in {} ms.", backoff.as_millis());
                tokio::time::sleep(backoff).await;
                backoff = backoff.saturating_mul(2).min(Duration::from_secs(30));
                continue;
            }
            Err(error) => return Err(AppError::Relay(error)),
        };
        backoff = Duration::from_millis(500);
        flush_pending(context, &profile).await?;
        let mut heartbeat_deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match tokio::time::timeout_at(heartbeat_deadline, stream.next_message()).await {
                Ok(Ok(message)) => match &message {
                    ServerWsMessage::Event { event, receipt } => {
                        if event.body.participant_id == profile.participant_id {
                            context.store.acknowledge(receipt).await?;
                        }
                        print_event(event);
                    }
                    ServerWsMessage::Notice { notice } => {
                        println!(
                            "Relay notice: {:?} {}",
                            notice.kind,
                            notice
                                .participant_id
                                .map(|id| id.as_uuid().to_string())
                                .unwrap_or_else(|| "session".to_owned())
                        );
                    }
                    ServerWsMessage::FinalManifest { manifest } => {
                        println!(
                            "Session ended; final evidence order {}.",
                            manifest.highest_evidence_order
                        );
                        return Ok(());
                    }
                    ServerWsMessage::HeartbeatAccepted { .. } => {}
                    ServerWsMessage::Error { code, .. } => {
                        return Err(AppError::Message(format!("relay rejected stream: {code}")));
                    }
                    ServerWsMessage::Challenge { .. } | ServerWsMessage::Authenticated { .. } => {
                        return Err(AppError::Message(
                            "relay sent an invalid post-authentication message".into(),
                        ));
                    }
                },
                Ok(Err(RelayClientError::Unavailable)) => {
                    eprintln!("Connection interrupted; queued events will be retried.");
                    break;
                }
                Ok(Err(error)) => return Err(AppError::Relay(error)),
                Err(_) => {
                    let (sequence, hash) = context
                        .store
                        .event_head(session_id, profile.participant_id)
                        .await?;
                    stream
                        .send_heartbeat(unix_time_ms()?, sequence, hash)
                        .await?;
                    heartbeat_deadline = Instant::now() + Duration::from_secs(30);
                }
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = backoff.saturating_mul(2).min(Duration::from_secs(30));
    }
}

fn print_event(event: &ParticipantEvent) {
    let summary = match &event.body.payload {
        EventPayload::ParticipantJoined { nickname, .. } => format!("{nickname} joined"),
        EventPayload::SessionStarted => "session started".to_owned(),
        EventPayload::SyntheticAiAccessAttempt { target } => {
            format!(
                "synthetic AI access attempt ({})",
                target.as_deref().unwrap_or("unspecified target")
            )
        }
        EventPayload::SessionEnded { .. } => "session end requested".to_owned(),
    };
    println!("[{}] {}", event.body.participant_id.as_uuid(), summary);
}
