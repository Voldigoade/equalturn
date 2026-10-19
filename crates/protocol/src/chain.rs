use std::collections::HashSet;

use crate::{
    EventId, Hash32, MAX_SESSION_EVENTS, ParticipantEvent, ParticipantId, ProtocolError, PublicKey,
    Result, SessionId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainValidation {
    pub missing_sequences: Vec<u64>,
    pub final_sequence: Option<u64>,
    pub final_hash: Option<Hash32>,
}

pub fn validate_event_chain(
    events: &[ParticipantEvent],
    public_key: &PublicKey,
) -> Result<ChainValidation> {
    validate_event_chain_to_head(events, public_key, None)
}

pub fn validate_event_chain_to_head(
    events: &[ParticipantEvent],
    public_key: &PublicKey,
    final_head: Option<(u64, Hash32)>,
) -> Result<ChainValidation> {
    if events.len() > 10_000 {
        return Err(ProtocolError::TooManyEvents);
    }
    let mut missing = Vec::new();
    let mut event_ids = HashSet::<EventId>::with_capacity(events.len());
    let mut identity = None::<(SessionId, ParticipantId)>;
    let mut last: Option<&ParticipantEvent> = None;

    for event in events {
        event.verify_with_key(public_key)?;
        if !event_ids.insert(event.body.event_id) {
            return Err(ProtocolError::DuplicateEvent);
        }
        if let Some((session_id, participant_id)) = identity {
            if event.body.session_id != session_id || event.body.participant_id != participant_id {
                return Err(ProtocolError::WrongEventIdentity);
            }
        } else {
            identity = Some((event.body.session_id, event.body.participant_id));
        }

        match last {
            None => {
                if event.body.sequence == 1 {
                    if event.body.previous_hash != Hash32::ZERO {
                        return Err(ProtocolError::PreviousHashMismatch);
                    }
                } else {
                    missing.extend(1..event.body.sequence);
                }
            }
            Some(previous) => {
                if event.body.sequence < previous.body.sequence {
                    return Err(ProtocolError::SequenceRollback);
                }
                if event.body.sequence == previous.body.sequence {
                    return Err(ProtocolError::DuplicateSequence);
                }
                let expected = previous
                    .body
                    .sequence
                    .checked_add(1)
                    .ok_or(ProtocolError::InvalidSequence)?;
                if event.body.sequence == expected {
                    if event.body.previous_hash != previous.event_hash()? {
                        return Err(ProtocolError::PreviousHashMismatch);
                    }
                } else {
                    missing.extend(expected..event.body.sequence);
                }
            }
        }
        last = Some(event);
    }

    if let Some((manifest_sequence, manifest_hash)) = final_head {
        if let Some(last_event) = last {
            if manifest_sequence > MAX_SESSION_EVENTS
                || manifest_sequence < last_event.body.sequence
            {
                return Err(ProtocolError::SequenceRollback);
            }
            if manifest_sequence == last_event.body.sequence {
                if manifest_hash != last_event.event_hash()? {
                    return Err(ProtocolError::PreviousHashMismatch);
                }
            } else {
                let start = last_event
                    .body
                    .sequence
                    .checked_add(1)
                    .ok_or(ProtocolError::InvalidSequence)?;
                missing.extend(start..=manifest_sequence);
            }
        } else {
            if manifest_sequence > MAX_SESSION_EVENTS {
                return Err(ProtocolError::TooManyEvents);
            }
            missing.extend(1..=manifest_sequence);
        }
    }

    if missing.len() > 10_000 {
        return Err(ProtocolError::TooManyEvents);
    }
    let (final_sequence, final_hash) = match final_head {
        Some((sequence, hash)) => (Some(sequence), Some(hash)),
        None => match last {
            Some(event) => (Some(event.body.sequence), Some(event.event_hash()?)),
            None => (None, None),
        },
    };
    Ok(ChainValidation {
        missing_sequences: missing,
        final_sequence,
        final_hash,
    })
}
