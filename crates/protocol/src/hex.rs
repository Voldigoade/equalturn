use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::HexError;

pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub fn decode_hex<const N: usize>(value: &str) -> std::result::Result<[u8; N], HexError> {
    if value.len() != N * 2 {
        return Err(HexError::WrongLength);
    }
    let mut bytes = [0_u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let high = decode_nibble(value.as_bytes()[index * 2]).ok_or(HexError::InvalidCharacter)?;
        let low =
            decode_nibble(value.as_bytes()[index * 2 + 1]).ok_or(HexError::InvalidCharacter)?;
        *byte = (high << 4) | low;
    }
    Ok(bytes)
}

fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

pub(crate) fn deserialize_fixed_hex<'de, const N: usize, D>(
    deserializer: D,
) -> std::result::Result<[u8; N], D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    decode_hex(&encoded).map_err(serde::de::Error::custom)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HexBytes<const N: usize>(pub [u8; N]);

impl<const N: usize> HexBytes<N> {
    pub const fn new(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; N] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        encode_hex(&self.0)
    }
}

impl HexBytes<32> {
    pub const ZERO: Self = Self([0; 32]);
}

impl<const N: usize> std::fmt::Debug for HexBytes<N> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("HexBytes")
            .field(&self.to_hex())
            .finish()
    }
}

impl<const N: usize> Serialize for HexBytes<N> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de, const N: usize> Deserialize<'de> for HexBytes<N> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_fixed_hex(deserializer).map(Self)
    }
}
