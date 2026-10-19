//! Platform-specific protection for local signing-key material.
//!
//! The only unsafe implementation is the narrow Windows DPAPI boundary in
//! `windows_dpapi`; callers receive opaque, generic errors without OS details.

#[cfg(windows)]
mod windows_dpapi;

#[cfg(windows)]
pub use windows_dpapi::{ProtectionError, protect_seed, unprotect_seed};
