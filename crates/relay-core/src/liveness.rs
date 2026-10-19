use std::time::Duration;

use equalturn_protocol::LivenessState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LivenessTransition {
    None,
    Stale,
    Disconnected,
}

pub fn transition_for_elapsed(
    current: LivenessState,
    elapsed_since_heartbeat: Duration,
    socket_closed: bool,
) -> LivenessTransition {
    if current == LivenessState::Ended {
        return LivenessTransition::None;
    }
    if socket_closed || elapsed_since_heartbeat >= Duration::from_secs(300) {
        return if current == LivenessState::Disconnected {
            LivenessTransition::None
        } else {
            LivenessTransition::Disconnected
        };
    }
    if elapsed_since_heartbeat >= Duration::from_secs(90)
        && matches!(current, LivenessState::Online)
    {
        return LivenessTransition::Stale;
    }
    LivenessTransition::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liveness_boundaries_use_supplied_time_without_sleeping() {
        assert_eq!(
            transition_for_elapsed(LivenessState::Online, Duration::from_secs(89), false),
            LivenessTransition::None
        );
        assert_eq!(
            transition_for_elapsed(LivenessState::Online, Duration::from_secs(90), false),
            LivenessTransition::Stale
        );
        assert_eq!(
            transition_for_elapsed(LivenessState::Stale, Duration::from_secs(300), false),
            LivenessTransition::Disconnected
        );
        assert_eq!(
            transition_for_elapsed(LivenessState::Disconnected, Duration::ZERO, true),
            LivenessTransition::None
        );
        assert_eq!(
            transition_for_elapsed(LivenessState::Ended, Duration::from_secs(1_000), true),
            LivenessTransition::None
        );
    }
}
