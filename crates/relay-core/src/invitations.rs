use std::collections::HashMap;

use equalturn_protocol::{Hash32, invitation_verifier};
use sha2::{Digest, Sha256};

const MAX_TRACKED_INVITATIONS: usize = 512;
const RATE_WINDOW_MS: u64 = 60_000;

#[derive(Default)]
pub(crate) struct InviteRateLimiter {
    attempts: HashMap<Hash32, (u64, u8)>,
}

impl InviteRateLimiter {
    pub(crate) fn allow(&mut self, raw_code: &str, now_ms: u64) -> bool {
        let key = invitation_verifier(raw_code)
            .unwrap_or_else(|_| Hash32::new(Sha256::digest(raw_code.as_bytes()).into()));
        if self.attempts.len() >= MAX_TRACKED_INVITATIONS {
            self.attempts
                .retain(|_, (started, _)| now_ms.saturating_sub(*started) < RATE_WINDOW_MS);
        }
        match self.attempts.get_mut(&key) {
            Some((started, count)) if now_ms.saturating_sub(*started) < RATE_WINDOW_MS => {
                if *count >= 5 {
                    false
                } else {
                    *count += 1;
                    true
                }
            }
            _ => {
                if self.attempts.len() >= MAX_TRACKED_INVITATIONS {
                    let oldest = self
                        .attempts
                        .iter()
                        .min_by_key(|(_, (started, _))| *started)
                        .map(|(key, _)| *key);
                    if let Some(oldest) = oldest {
                        self.attempts.remove(&oldest);
                    }
                }
                self.attempts.insert(key, (now_ms, 1));
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use equalturn_protocol::InvitationCode;

    #[test]
    fn rate_limit_is_bounded_and_time_windowed() {
        let code = InvitationCode::generate().expect("random code");
        let mut limiter = InviteRateLimiter::default();
        for _ in 0..5 {
            assert!(limiter.allow(code.as_str(), 1_000));
        }
        assert!(!limiter.allow(code.as_str(), 1_001));
        assert!(limiter.allow(code.as_str(), 61_001));
    }

    #[test]
    fn unique_invalid_codes_cannot_lock_out_a_valid_invitation() {
        let mut limiter = InviteRateLimiter::default();
        for index in 0..MAX_TRACKED_INVITATIONS {
            assert!(limiter.allow(&format!("invalid-{index}"), 1_000));
        }
        let valid_code = InvitationCode::generate().expect("random code");

        assert!(limiter.allow(valid_code.as_str(), 1_001));
        assert_eq!(limiter.attempts.len(), MAX_TRACKED_INVITATIONS);
    }
}
