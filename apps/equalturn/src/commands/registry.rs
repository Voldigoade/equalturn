use std::{fs, io::Read, path::Path};

use equalturn_registry::{
    Entry, RegistryVerificationStatus, Source, VerifiedRegistry, verify_repository,
};
use jiff::Timestamp;

use crate::{
    app::{AppError, data_directory},
    cli::{Options, RegistryCommand, RegistrySourceArgs},
};

pub async fn run(options: &Options, command: RegistryCommand) -> Result<(), AppError> {
    match command {
        RegistryCommand::Verify(source) => {
            let verified = load(options, &source).await.map_err(failure)?;
            print_verified(&verified);
            Ok(())
        }
        RegistryCommand::Inspect(args) => {
            let verified = load(options, &args.source).await.map_err(failure)?;
            print_verified(&verified);
            let entry = verified
                .registry()
                .entry(&args.entry_id)
                .ok_or_else(|| AppError::Message("registry entry was not found".into()))?;
            print_entry(entry);
            Ok(())
        }
        RegistryCommand::Search(args) => {
            let verified = load(options, &args.source).await.map_err(failure)?;
            print_verified(&verified);
            let entries = verified.registry().search(&args.query);
            if entries.is_empty() {
                println!("No registry entries matched the query.");
            } else {
                for (index, entry) in entries.iter().enumerate() {
                    if index > 0 {
                        println!();
                    }
                    print_entry(entry);
                }
            }
            Ok(())
        }
    }
}

async fn load(
    options: &Options,
    source: &RegistrySourceArgs,
) -> Result<VerifiedRegistry, RegistryVerificationStatus> {
    let root =
        read_bootstrap_root(&source.root).map_err(|_| RegistryVerificationStatus::InvalidRoot)?;
    let root_sha256 = equalturn_protocol::decode_hex::<32>(&source.root_sha256)
        .map_err(|_| RegistryVerificationStatus::InvalidRoot)?;
    let cache = match &source.cache_dir {
        Some(path) => path.clone(),
        None => data_directory(options)
            .map_err(|_| RegistryVerificationStatus::InvalidRepository)?
            .join("registry")
            .join("trusted"),
    };

    verify_repository(
        &source.repository,
        &root,
        &root_sha256,
        &cache,
        Timestamp::now(),
    )
    .await
    .map_err(|error| error.verification_status())
}

fn read_bootstrap_root(path: &Path) -> std::io::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink()
        || !before.is_file()
        || before.len() > equalturn_registry::ROOT_MAX_BYTES as u64
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bootstrap root is not a bounded regular file",
        ));
    }
    let file = fs::File::open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() > equalturn_registry::ROOT_MAX_BYTES as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bootstrap root changed to an unsafe file",
        ));
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take((equalturn_registry::ROOT_MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > equalturn_registry::ROOT_MAX_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bootstrap root exceeds the input limit",
        ));
    }
    Ok(bytes)
}

fn failure(status: RegistryVerificationStatus) -> AppError {
    eprintln!("Verification: {}", status.as_str());
    AppError::Message("registry verification failed".into())
}

fn print_verified(verified: &VerifiedRegistry) {
    println!("Verification: VALID");
    println!(
        "Registry version: {}",
        verified.registry().registry_version()
    );
    println!("Last updated: {}", verified.registry().last_updated());
    println!("Entries: {}", verified.registry().entries().len());
    println!("Indicators: {}", verified.registry().indicator_count());
    println!("Trusted root version: {}", verified.trusted_root_version());
    println!("Target SHA-256: {}", encode_hex(verified.target_sha256()));
}

fn print_entry(entry: &Entry) {
    println!("Entry: {}", entry.id());
    println!(
        "Provider: {} ({})",
        entry.provider().name(),
        entry.provider().id()
    );
    println!("Product: {}", entry.product());
    if !entry.provider().aliases().is_empty() {
        println!("Aliases: {}", entry.provider().aliases().join(", "));
    }
    println!(
        "Categories: {}",
        entry
            .categories()
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "Surfaces: {}",
        entry
            .surfaces()
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Status: {}", entry.status().as_str());
    for indicator in entry.indicators() {
        println!(
            "Indicator: {} — {}",
            indicator.kind().as_str(),
            indicator.value()
        );
        println!(
            "  Platforms: {}",
            indicator
                .platforms()
                .iter()
                .map(|value| value.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "  Surfaces: {}",
            indicator
                .surfaces()
                .iter()
                .map(|value| value.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("  Scope: {}", indicator.scope());
        println!("  Confidence: {}", indicator.confidence().as_str());
        println!("  Actionability: {}", indicator.actionability().as_str());
        println!(
            "  False-positive risk: {}",
            indicator.false_positive_risk().as_str()
        );
        println!("  Lifecycle: {}", indicator.lifecycle().as_str());
        println!("  Verified: {}", indicator.verified_on());
        if let Some(source) = entry
            .sources()
            .iter()
            .find(|source| source.id() == indicator.source_id())
        {
            print_source(source);
        }
        println!("  Limitations: {}", indicator.limitations());
    }
}

fn print_source(source: &Source) {
    println!(
        "  Source: {} ({}, checked {})",
        source.url(),
        source.id(),
        source.checked_on()
    );
    println!("  Source assessment: {}", source.indicator_assessment());
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}
