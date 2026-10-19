use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::{Hash32, ProtocolError, Result};

pub const INVITATION_ALPHABET: &[u8; 31] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

#[derive(PartialEq, Eq)]
pub struct InvitationCode(Zeroizing<String>);

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
        let mut formatted = Zeroizing::new(String::with_capacity(30));
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

    pub fn parse_owned(value: String) -> Result<Self> {
        let value = Zeroizing::new(value);
        Self::parse(&value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn verifier(&self) -> Hash32 {
        hash_formatted_invitation(self.as_str())
    }
}

impl Serialize for InvitationCode {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for InvitationCode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct InvitationCodeVisitor;

        impl Visitor<'_> for InvitationCodeVisitor {
            type Value = InvitationCode;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a valid Equalturn invitation code")
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                InvitationCode::parse(value).map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                InvitationCode::parse_owned(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_string(InvitationCodeVisitor)
    }
}

pub fn normalize_invitation(value: &str) -> Result<Zeroizing<String>> {
    let normalized = Zeroizing::new(
        value
            .bytes()
            .filter(|byte| *byte != b'-')
            .map(|byte| char::from(byte.to_ascii_uppercase()))
            .collect::<String>(),
    );
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

fn hash_formatted_invitation(formatted: &str) -> Hash32 {
    let mut normalized = Zeroizing::new([0_u8; 26]);
    let mut length = 0;
    for byte in formatted.bytes().filter(|byte| *byte != b'-') {
        normalized[length] = byte;
        length += 1;
    }
    debug_assert_eq!(length, normalized.len());
    let mut digest = Sha256::new();
    digest.update(&normalized[..]);
    Hash32::new(digest.finalize().into())
}

fn hash_invitation(normalized: &str) -> Hash32 {
    let mut digest = Sha256::new();
    digest.update(normalized.as_bytes());
    Hash32::new(digest.finalize().into())
}

fn format_invitation(normalized: &str) -> Zeroizing<String> {
    let mut formatted = Zeroizing::new(String::with_capacity(30));
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

    #[test]
    fn invitation_json_remains_a_canonical_string() {
        let code = InvitationCode::parse("23456-789ab-cdefg-hjkmn-pqrstu").expect("valid code");

        assert_eq!(
            serde_json::to_string(&code).expect("serialize code"),
            "\"23456-789AB-CDEFG-HJKMN-PQRSTU\""
        );
        let restored: InvitationCode = serde_json::from_str("\"23456-789AB-CDEFG-HJKMN-PQRSTU\"")
            .expect("deserialize valid code");
        assert_eq!(restored.as_str(), code.as_str());
    }

    #[test]
    fn invitation_json_rejects_invalid_codes() {
        assert!(serde_json::from_str::<InvitationCode>("\"not-an-invitation\"").is_err());
    }
}
