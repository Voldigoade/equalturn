mod http_repository;
mod installed;
mod json;
mod model;
mod repository;
mod trust;

pub use installed::{
    InstalledRegistry, load_installed_registry, update_directory_repository,
    update_https_repository,
};
pub use model::{
    Actionability, BindScope, Category, Confidence, Ecosystem, Entry, EntryStatus,
    FalsePositiveRisk, Indicator, IndicatorKind, IndicatorLifecycle, Platform, Provider, Registry,
    RegistryError, RegistrySurface, RegistryVerificationStatus, Relationship, RelationshipKind,
    Source, TufFailureKind,
};
pub use trust::{
    VerifiedRegistry, verify_bootstrap_root, verify_https_repository, verify_repository,
};

/// The v3 registry schema limits a registry target to four mebibytes.
pub const REGISTRY_MAX_BYTES: usize = 4 * 1024 * 1024;

/// The largest bootstrap root accepted from an out-of-band trust source.
pub const ROOT_MAX_BYTES: usize = 512 * 1024;

/// The only target path this client recognizes in a TUF repository.
pub const REGISTRY_TARGET_PATH: &str = "registry-v3.json";
