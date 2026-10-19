use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::{Hash32, ProtocolError, Result};

pub const INVITATION_ALPHABET: &[u8; 31] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

#[derive(Clone, PartialEq, Eq)]
pub struct InvitationCode(String);

impl InvitationCode {
    pub fn generate() -> Result<Self> {
        let mut entropy = [0_u8; 26];
        let mut symbols = [0_u8; 26];
        let mut written = 0;
        while written < symbols.len() {
            if getrandom::fill(&mut entropy).is_err() {
                entropy.zeroize();
                symbols.zeroize();
                return Err(ProtocolError::EntropyUnavailable);
            }
            for byte in entropy.iter().copied().filter(|byte| *byte < 248) {
                symbols[written] = INVITATION_ALPHABET[usize::from(byte % 31)];
                written += 1;
                if written == symbols.len() {
                    break;
                }
            }
        }
        entropy.zeroize();
        let mut formatted = String::with_capacity(30);
        for (index, byte) in symbols.iter().copied().enumerate() {
            if matches!(index, 5 | 10 | 15 | 20) {
                formatted.push('-');
            }
            formatted.push(char::from(byte));
        }
        symbols.zeroize();
        Ok(Self(formatted))
    }

    pub fn parse(value: &str) -> Result<Self> {
        let normalized = normalize_invitation(value)?;
        Ok(Self(format_invitation(&normalized)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn verifier(&self) -> Hash32 {
        let normalized = self.0.replace('-', "");
        hash_invitation(&normalized)
    }
}

pub fn normalize_invitation(value: &str) -> Result<String> {
    let normalized: String = value
        .bytes()
        .filter(|byte| *byte != b'-')
        .map(|byte| char::from(byte.to_ascii_uppercase()))
        .collect();
    if normalized.len() != 26
        || !normalized
            .bytes()
            .all(|byte| INVITATION_ALPHABET.contains(&byte))
    {
        return Err(ProtocolError::InvalidInvitation);
    }
    Ok(normalized)
}

pub fn invitation_verifier(value: &str) -> Result<Hash32> {
    Ok(hash_invitation(&normalize_invitation(value)?))
}

fn hash_invitation(normalized: &str) -> Hash32 {
    let mut digest = Sha256::new();
    digest.update(normalized.as_bytes());
    Hash32::new(digest.finalize().into())
}

fn format_invitation(normalized: &str) -> String {
    let mut formatted = String::with_capacity(30);
    for (index, byte) in normalized.bytes().enumerate() {
        if matches!(index, 5 | 10 | 15 | 20) {
            formatted.push('-');
        }
        formatted.push(char::from(byte));
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_has_about_128_bits_and_safe_normalization() {
        let first = InvitationCode::generate().expect("entropy available");
        let second = InvitationCode::generate().expect("entropy available");
        assert_eq!(first.as_str().len(), 30);
        assert_eq!(first.as_str().matches('-').count(), 4);
        assert_ne!(first.as_str(), second.as_str());
        let unformatted = first.as_str().replace('-', "").to_ascii_lowercase();
        assert_eq!(
            InvitationCode::parse(&unformatted)
                .expect("valid code")
                .as_str(),
            first.as_str()
        );
        assert_eq!(
            first.verifier(),
            invitation_verifier(&unformatted).expect("valid code")
        );
    }

    #[test]
    fn malformed_invitation_is_rejected() {
        assert!(normalize_invitation("00000000000000000000000000").is_err());
        assert!(normalize_invitation("23456789ABCDEFGHJKMNPQRSTUVW").is_err());
        assert!(normalize_invitation("23456789ABCDEFGHJKMNPQRSTUV😀").is_err());
    }
}
