use std::{fs, process::Command};

mod support;

const REGISTRY_BYTES: &[u8] = include_bytes!("../../../registry/registry-v3.json");

#[test]
fn registry_verification_reports_bootstrap_failure_without_relay_configuration() {
    let directory = tempfile::tempdir().expect("test directory is created");
    let repository = directory.path().join("repository");
    let root = directory.path().join("root.json");
    let cache = directory.path().join("cache");
    fs::create_dir(&repository).expect("local repository directory is created");
    fs::write(&root, b"not a trusted root").expect("untrusted root bytes are written");

    let output = Command::new(env!("CARGO_BIN_EXE_equalturn"))
        .arg("registry")
        .arg("verify")
        .arg(&repository)
        .arg("--root")
        .arg(&root)
        .arg("--root-sha256")
        .arg("00".repeat(32))
        .arg("--cache-dir")
        .arg(&cache)
        .env_remove("EQUALTURN_RELAY_URL")
        .output()
        .expect("Equalturn CLI starts");

    assert!(!output.status.success(), "wrong bootstrap pin is rejected");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.contains("INVALID_ROOT"),
        "CLI should give the stable INVALID_ROOT status, got: {output}"
    );
}

#[test]
fn registry_commands_verify_local_data_before_inspection_or_search() {
    let directory = tempfile::tempdir().expect("test directory is created");
    let repository = support::tuf::signed_repository(REGISTRY_BYTES);
    let root = directory.path().join("root.json");
    let cache = directory.path().join("cache");
    fs::write(&root, &repository.root).expect("pinned bootstrap root is written");

    let verify = invoke_registry(
        "verify",
        None,
        repository.path(),
        &root,
        &repository.root_sha256,
        &cache,
    );
    assert_success(&verify);
    assert!(String::from_utf8_lossy(&verify.stdout).contains("Verification: VALID"));

    let inspect = invoke_registry(
        "inspect",
        Some("openai-chatgpt"),
        repository.path(),
        &root,
        &repository.root_sha256,
        &cache,
    );
    assert_success(&inspect);
    let inspect = String::from_utf8_lossy(&inspect.stdout);
    assert!(inspect.contains("Verification: VALID"));
    assert!(inspect.contains("Provider: OpenAI"));
    assert!(inspect.contains("Product: ChatGPT"));
    assert!(inspect.contains("Actionability:"));
    assert!(inspect.contains("False-positive risk:"));
    assert!(inspect.contains("Limitations:"));

    let search = invoke_registry(
        "search",
        Some("openai"),
        repository.path(),
        &root,
        &repository.root_sha256,
        &cache,
    );
    assert_success(&search);
    let search = String::from_utf8_lossy(&search.stdout);
    assert!(search.contains("Verification: VALID"));
    assert!(search.contains("Product: ChatGPT"));
}

fn invoke_registry(
    subcommand: &str,
    query_or_id: Option<&str>,
    repository: &std::path::Path,
    root: &std::path::Path,
    pin: &str,
    cache: &std::path::Path,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_equalturn"));
    command.arg("registry").arg(subcommand).arg(repository);
    if let Some(value) = query_or_id {
        command.arg(value);
    }
    command
        .arg("--root")
        .arg(root)
        .arg("--root-sha256")
        .arg(pin)
        .arg("--cache-dir")
        .arg(cache)
        .env_remove("EQUALTURN_RELAY_URL")
        .output()
        .expect("Equalturn CLI starts")
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
