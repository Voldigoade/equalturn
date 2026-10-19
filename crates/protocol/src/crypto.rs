use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::{HexBytes, ProtocolError, Result};

pub type Hash32 = HexBytes<32>;
pub type PublicKey = HexBytes<32>;
pub type SignatureBytes = HexBytes<64>;

pub struct SigningIdentity {
    key: SigningKey,
}

impl SigningIdentity {
    pub fn generate() -> Result<Self> {
        let mut seed = [0_u8; 32];
        getrandom::fill(&mut seed).map_err(|_| ProtocolError::EntropyUnavailable)?;
        let identity = Self::from_seed(seed);
        seed.zeroize();
        Ok(identity)
    }

    pub fn from_seed(mut seed: [u8; 32]) -> Self {
        let key = SigningKey::from_bytes(&seed);
        seed.zeroize();
        Self { key }
    }

    pub fn seed_bytes(&self) -> [u8; 32] {
        self.key.to_bytes()
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
