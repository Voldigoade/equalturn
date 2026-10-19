use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ProtocolError, Result};

macro_rules! typed_uuid {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub const NIL: Self = Self(Uuid::nil());

            pub fn try_new_v4() -> Result<Self> {
                let mut bytes = [0_u8; 16];
                getrandom::fill(&mut bytes).map_err(|_| ProtocolError::EntropyUnavailable)?;
                bytes[6] = (bytes[6] & 0x0f) | 0x40;
                bytes[8] = (bytes[8] & 0x3f) | 0x80;
                Ok(Self(Uuid::from_bytes(bytes)))
            }

            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(Uuid::from_bytes(bytes))
            }

            pub const fn as_bytes(&self) -> &[u8; 16] {
                self.0.as_bytes()
            }

            pub const fn is_nil(&self) -> bool {
                self.0.is_nil()
            }

            pub const fn is_v4(&self) -> bool {
                let bytes = self.0.as_bytes();
                bytes[6] >> 4 == 4 && bytes[8] & 0xc0 == 0x80
            }

            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
    };
}

typed_uuid!(SessionId);
typed_uuid!(ParticipantId);
typed_uuid!(EventId);
typed_uuid!(ChallengeId);
typed_uuid!(NoticeId);
