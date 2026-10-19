use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sigstore_crypto::KeyPair;
use sigstore_tuf::{Key, canonical_json::to_canonical_bytes};
use std::{fs, path::Path};
use tempfile::TempDir;

const FAR_FUTURE: &str = "2999-01-01T00:00:00Z";

pub struct LocalRepository {
    directory: TempDir,
    pub root: Vec<u8>,
    pub root_sha256: String,
}

impl LocalRepository {
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
}

struct Signer {
    key: KeyPair,
    id: String,
    value: Value,
}

impl Signer {
    fn generate() -> Self {
        let key = KeyPair::generate_ecdsa_p256().expect("test signing key generation succeeds");
        let public_pem = key
            .public_key_der()
            .expect("test signing key has a public key")
            .to_pem();
        let value = json!({
            "keytype": "ecdsa",
            "scheme": "ecdsa-sha2-nistp256",
            "keyval": { "public": public_pem },
        });
        let parsed: Key =
            serde_json::from_value(value.clone()).expect("test public key parses as a TUF key");
        let id = parsed.key_id().expect("test TUF key ID can be computed");
        Self { key, id, value }
    }

    fn sign(&self, signed: &Value) -> Value {
        let canonical = to_canonical_bytes(signed).expect("TUF metadata can be canonicalized");
        let signature = self
            .key
            .sign(&canonical)
            .expect("test metadata signature is created");
        json!({ "keyid": self.id, "sig": encode_hex(signature.as_bytes()) })
    }
}

pub fn signed_repository(target: &[u8]) -> LocalRepository {
    let signer = Signer::generate();
    let root_role = json!({ "keyids": [signer.id], "threshold": 1 });
    let root_signed = json!({
        "_type": "root",
        "spec_version": "1.0.0",
        "version": 1,
        "expires": FAR_FUTURE,
        "consistent_snapshot": false,
        "keys": { signer.id.clone(): signer.value.clone() },
        "roles": {
            "root": root_role,
            "timestamp": root_role,
            "snapshot": root_role,
            "targets": root_role,
        },
    });
    let root = envelope(&root_signed, &[&signer]);

    let targets_signed = json!({
        "_type": "targets",
        "spec_version": "1.0.0",
        "version": 1,
        "expires": FAR_FUTURE,
        "targets": {
            "registry-v3.json": {
                "length": target.len(),
                "hashes": { "sha256": sha256_hex(target) },
            }
        },
    });
    let targets = envelope(&targets_signed, &[&signer]);

    let snapshot_signed = json!({
        "_type": "snapshot",
        "spec_version": "1.0.0",
        "version": 1,
        "expires": FAR_FUTURE,
        "meta": { "targets.json": meta(&targets) },
    });
    let snapshot = envelope(&snapshot_signed, &[&signer]);

    let timestamp_signed = json!({
        "_type": "timestamp",
        "spec_version": "1.0.0",
        "version": 1,
        "expires": FAR_FUTURE,
        "meta": { "snapshot.json": meta(&snapshot) },
    });
    let timestamp = envelope(&timestamp_signed, &[&signer]);

    let directory = tempfile::tempdir().expect("local TUF repository directory is created");
    fs::create_dir(directory.path().join("metadata")).expect("metadata directory is created");
    fs::create_dir(directory.path().join("targets")).expect("targets directory is created");
    fs::write(directory.path().join("metadata/timestamp.json"), timestamp)
        .expect("timestamp metadata is written");
    fs::write(directory.path().join("metadata/snapshot.json"), snapshot)
        .expect("snapshot metadata is written");
    fs::write(directory.path().join("metadata/targets.json"), targets)
        .expect("targets metadata is written");
    fs::write(directory.path().join("targets/registry-v3.json"), target)
        .expect("registry target is written");

    let root_sha256 = sha256_hex(&root);
    LocalRepository {
        directory,
        root,
        root_sha256,
    }
}

fn envelope(signed: &Value, signers: &[&Signer]) -> Vec<u8> {
    let signatures: Vec<_> = signers.iter().map(|signer| signer.sign(signed)).collect();
    serde_json::to_vec(&json!({ "signed": signed, "signatures": signatures }))
        .expect("TUF metadata envelope serializes")
}

fn meta(bytes: &[u8]) -> Value {
    json!({
        "version": 1,
        "length": bytes.len(),
        "hashes": { "sha256": sha256_hex(bytes) },
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    encode_hex(&Sha256::digest(bytes))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
