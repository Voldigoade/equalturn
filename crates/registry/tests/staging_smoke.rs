use equalturn_registry::{load_installed_registry, update_https_repository};
use jiff::Timestamp;
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use url::Url;

const STAGING_ROOT: &[u8] = include_bytes!("fixtures/staging-root.json");
const STAGING_ROOT_SHA256: [u8; 32] = [
    0x71, 0x9e, 0x37, 0x2e, 0xcc, 0x71, 0xe8, 0xf5, 0x09, 0xf5, 0x80, 0x58, 0x89, 0x56, 0x03, 0x82,
    0xd3, 0xd5, 0x99, 0x97, 0x1b, 0x82, 0x94, 0x7f, 0x9d, 0xa8, 0x25, 0x10, 0x1f, 0x32, 0x31, 0x44,
];
const EXPECTED_TARGET_SHA256: [u8; 32] = [
    0xe6, 0xcc, 0xf5, 0x7e, 0xca, 0x33, 0x95, 0x1c, 0xd6, 0xb7, 0xa0, 0x4e, 0x7e, 0x60, 0x9f, 0x98,
    0x1c, 0x87, 0xc7, 0x32, 0x98, 0xb7, 0x92, 0x3b, 0x4d, 0x5a, 0x9a, 0x84, 0x5c, 0xaa, 0x02, 0xa0,
];

#[tokio::test]
#[ignore = "requires access to the public staging TUF repository; run explicitly for a live smoke test"]
async fn verifies_published_staging_registry_with_pinned_root() {
    sigstore_tuf::TrustedMetadataSet::from_root(STAGING_ROOT)
        .expect("pinned staging root parses and verifies");
    let state = tempdir().expect("installed registry directory is created");
    let base_url = Url::parse("https://voldigoade.github.io/equalturn-registry-staging/")
        .expect("staging repository URL is valid");

    let installed = update_https_repository(
        &base_url,
        STAGING_ROOT,
        &STAGING_ROOT_SHA256,
        state.path(),
        Timestamp::now(),
    )
    .await
    .expect("published repository verifies and installs against the pinned staging root");

    assert_eq!(installed.trusted_root_version(), 1);
    assert_eq!(installed.registry().schema_version(), 3);
    assert_eq!(installed.registry().entries().len(), 131);
    assert_eq!(installed.target_sha256(), &EXPECTED_TARGET_SHA256);
    assert_eq!(Sha256::digest(STAGING_ROOT).as_slice(), STAGING_ROOT_SHA256);

    let reopened = load_installed_registry(
        state.path(),
        STAGING_ROOT,
        &STAGING_ROOT_SHA256,
        Timestamp::now(),
    )
    .await
    .expect("staging registry reopens offline after installation");
    assert_eq!(reopened.target_sha256(), &EXPECTED_TARGET_SHA256);
}
