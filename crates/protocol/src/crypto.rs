use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{HexBytes, ProtocolError, Result};

pub type Hash32 = HexBytes<32>;
pub type PublicKey = HexBytes<32>;
pub type SignatureBytes = HexBytes<64>;

pub struct SigningSeed(Zeroizing<[u8; 32]>);

impl SigningSeed {
    pub fn from_bytes(seed: [u8; 32]) -> Self {
        Self(Zeroizing::new(seed))
    }

    pub fn from_zeroizing(seed: Zeroizing<[u8; 32]>) -> Self {
        Self(seed)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

pub struct SigningIdentity {
    key: SigningKey,
}

impl SigningIdentity {
    pub fn generate() -> Result<Self> {
        let mut bytes = Zeroizing::new([0_u8; 32]);
        getrandom::fill(&mut *bytes).map_err(|_| ProtocolError::EntropyUnavailable)?;
        let seed = SigningSeed::from_zeroizing(bytes);
        Ok(Self::from_seed(&seed))
    }

    pub fn from_seed(seed: &SigningSeed) -> Self {
        Self {
            key: SigningKey::from_bytes(seed.as_bytes()),
        }
    }

    pub fn export_seed(&self) -> SigningSeed {
        SigningSeed::from_bytes(self.key.to_bytes())
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::new(self.key.verifying_key().to_bytes())
    }

    pub(crate) fn sign(&self, message: &[u8]) -> SignatureBytes {
        SignatureBytes::new(self.key.sign(message).to_bytes())
    }
}

pub fn fingerprint(public_key: &PublicKey) -> crate::Hash32 {
    let mut digest = Sha256::new();
    digest.update(b"Equalturn\0fingerprint\0v1\0");
    digest.update(public_key.as_bytes());
    Hash32::new(digest.finalize().into())
}

pub(crate) fn verifying_key(public_key: &PublicKey) -> Result<VerifyingKey> {
    VerifyingKey::from_bytes(public_key.as_bytes()).map_err(|_| ProtocolError::InvalidPublicKey)
}

pub(crate) fn verify_signature(
    public_key: &PublicKey,
    signature: &SignatureBytes,
    message: &[u8],
) -> Result<()> {
    let key = verifying_key(public_key)?;
    let signature = ed25519_dalek::Signature::from_bytes(signature.as_bytes());
    key.verify_strict(message, &signature)
        .map_err(|_| ProtocolError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::{SigningIdentity, SigningSeed};

    #[test]
    fn exported_seed_round_trips_without_returning_a_plain_seed_array() {
        let seed = SigningSeed::from_bytes([0x42; 32]);
        let identity = SigningIdentity::from_seed(&seed);
        let exported = identity.export_seed();
        let restored = SigningIdentity::from_seed(&exported);

        assert!(
            exported.as_bytes() == seed.as_bytes(),
            "seed export round-trips"
        );
        assert_eq!(restored.public_key(), identity.public_key());
    }
}
