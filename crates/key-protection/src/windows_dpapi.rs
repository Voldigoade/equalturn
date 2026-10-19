use std::{ffi::c_void, ptr};

use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    },
    core::PCWSTR,
};
use zeroize::Zeroizing;

const MAX_PROTECTED_BYTES: usize = 16 * 1024;

#[derive(Debug, Error)]
#[error("current-user key protection failed")]
pub struct ProtectionError;

/// Protects a seed using the current Windows user's DPAPI scope.
///
/// This deliberately omits `CRYPTPROTECT_LOCAL_MACHINE`: the protected value
/// is not intended to be decryptable by other accounts on the same machine.
#[allow(unsafe_code)]
pub fn protect_seed(seed: &[u8; 32], entropy: &[u8]) -> Result<Vec<u8>, ProtectionError> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: seed.len() as u32,
        pbData: seed.as_ptr().cast_mut(),
    };
    let entropy_blob = entropy_blob(entropy)?;
    let mut output = CRYPT_INTEGER_BLOB::default();

    // SAFETY: input and entropy pointers remain valid for the call, reserved
    // and prompt parameters are absent, and output is freed by LocalBlob.
    let result = unsafe {
        CryptProtectData(
            &input,
            PCWSTR::null(),
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    let mut output = LocalBlob(output);
    result.map_err(|_| ProtectionError)?;
    if output.0.cbData == 0
        || output.0.cbData as usize > MAX_PROTECTED_BYTES
        || output.0.pbData.is_null()
    {
        return Err(ProtectionError);
    }
    // SAFETY: DPAPI returned a valid allocation of cbData bytes; LocalBlob
    // owns it until this copy completes and releases it afterward.
    let value =
        unsafe { std::slice::from_raw_parts(output.0.pbData, output.0.cbData as usize).to_vec() };
    output.clear_and_free()?;
    Ok(value)
}

/// Unprotects a seed previously protected for the current Windows user.
#[allow(unsafe_code)]
pub fn unprotect_seed(
    protected: &[u8],
    entropy: &[u8],
) -> Result<Zeroizing<[u8; 32]>, ProtectionError> {
    if protected.is_empty() || protected.len() > MAX_PROTECTED_BYTES {
        return Err(ProtectionError);
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(protected.len()).map_err(|_| ProtectionError)?,
        pbData: protected.as_ptr().cast_mut(),
    };
    let entropy_blob = entropy_blob(entropy)?;
    let mut output = CRYPT_INTEGER_BLOB::default();

    // SAFETY: input and entropy pointers remain valid for the call, reserved,
    // description, and prompt parameters are absent, and LocalBlob owns the
    // resulting allocation. The returned plaintext is length-checked below.
    let result = unsafe {
        CryptUnprotectData(
            &input,
            None,
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    let mut output = LocalBlob(output);
    result.map_err(|_| ProtectionError)?;
    if output.0.cbData != 32 || output.0.pbData.is_null() {
        return Err(ProtectionError);
    }
    let mut seed = Zeroizing::new([0_u8; 32]);
    // SAFETY: the length check establishes that DPAPI returned 32 readable
    // bytes, and the LocalBlob guard keeps that allocation alive until drop.
    unsafe { ptr::copy_nonoverlapping(output.0.pbData, seed.as_mut_ptr(), seed.len()) };
    output.clear_and_free()?;
    Ok(seed)
}

#[allow(unsafe_code)]
fn entropy_blob(entropy: &[u8]) -> Result<CRYPT_INTEGER_BLOB, ProtectionError> {
    let length = u32::try_from(entropy.len()).map_err(|_| ProtectionError)?;
    Ok(CRYPT_INTEGER_BLOB {
        cbData: length,
        pbData: if entropy.is_empty() {
            ptr::null_mut()
        } else {
            entropy.as_ptr().cast_mut()
        },
    })
}

struct LocalBlob(CRYPT_INTEGER_BLOB);

impl LocalBlob {
    #[allow(unsafe_code)]
    fn clear_and_free(&mut self) -> Result<(), ProtectionError> {
        if self.0.pbData.is_null() {
            return Ok(());
        }
        // SAFETY: `pbData` and `cbData` are the output allocation returned by
        // DPAPI, which remains owned by this guard until LocalFree succeeds.
        unsafe { ptr::write_bytes(self.0.pbData, 0, self.0.cbData as usize) };
        // SAFETY: DPAPI documents LocalFree as the matching deallocator.
        let result = unsafe { LocalFree(Some(HLOCAL(self.0.pbData.cast::<c_void>()))) };
        if result.is_invalid() {
            self.0.pbData = ptr::null_mut();
            self.0.cbData = 0;
            Ok(())
        } else {
            Err(ProtectionError)
        }
    }
}

impl Drop for LocalBlob {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // DPAPI plaintext is allocated with LocalAlloc and must be returned
        // with LocalFree. Clear it first; the ciphertext output is cleared too.
        if self.clear_and_free().is_err() {
            // A failed release is only reachable for an invalid DPAPI-owned
            // pointer. The bytes have already been zeroed; abort rather than
            // continue with an unreleased OS allocation of former plaintext.
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProtectionError, protect_seed, unprotect_seed};

    #[test]
    fn current_user_dpapi_round_trip_and_entropy_binding() {
        let seed = [0x5a; 32];
        let entropy = b"Equalturn test envelope binding";
        let protected = protect_seed(&seed, entropy).expect("protect seed");
        assert_ne!(protected, seed);
        assert_eq!(
            unprotect_seed(&protected, entropy)
                .expect("unprotect for same user")
                .as_ref(),
            &seed
        );
        assert!(matches!(
            unprotect_seed(&protected, b"different binding"),
            Err(ProtectionError)
        ));
    }

    #[test]
    fn tampered_dpapi_ciphertext_is_rejected() {
        let seed = [0xa5; 32];
        let entropy = b"Equalturn test envelope binding";
        let mut protected = protect_seed(&seed, entropy).expect("protect seed");
        let middle = protected.len() / 2;
        protected[middle] ^= 0x80;
        assert!(unprotect_seed(&protected, entropy).is_err());
    }
}
