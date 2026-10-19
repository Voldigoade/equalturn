use equalturn_client_core::RelayClientError;
use equalturn_protocol::EventPayload;

use crate::{
    app::{AppError, Context, parse_session_id, select_profile, unix_time_ms},
    cli::{EventCommand, SyntheticKind},
    commands::session::post_event,
};

pub async fn run(context: &Context, command: EventCommand) -> Result<(), AppError> {
    match command {
        EventCommand::Simulate {
            session,
            kind: SyntheticKind::AiAccessAttempt,
            target,
            participant,
        } => {
            let session_id = parse_session_id(&session)?;
            let profile =
                select_profile(&context.store, session_id, participant.as_deref()).await?;
            let identity = context
                .store
                .load_identity(session_id, profile.participant_id)
                .await?;
            let event = context
                .store
                .enqueue_event(
                    &profile,
                    &identity,
                    unix_time_ms()?,
                    EventPayload::SyntheticAiAccessAttempt { target },
                )
                .await?;
            match post_event(context, &profile, &event).await {
                Ok(()) => {
                    println!("Synthetic event accepted by relay.");
                    Ok(())
                }
                Err(RelayClientError::Unavailable) => Err(AppError::Queued(
                    "synthetic event is signed and stored locally; run `equalturn session sync --session <UUID>` after reconnecting".into(),
                )),
                Err(error) => Err(AppError::Relay(error)),
            }
        }
    }
}
