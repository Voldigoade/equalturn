use std::{fs, io::Read, path::Path};

use equalturn_registry::{
    Entry, InstalledRegistry, RegistryVerificationStatus, Source, VerifiedRegistry,
    load_installed_registry, update_directory_repository, update_https_repository,
    verify_repository,
};
use jiff::Timestamp;
use url::Url;

use crate::{
    app::{AppError, data_directory},
    cli::{
        Options, RegistryCommand, RegistryInstalledCommand, RegistryProfile, RegistryProfileArgs,
        RegistrySourceArgs, RegistryUpdateArgs,
    },
};

pub async fn run(options: &Options, command: RegistryCommand) -> Result<(), AppError> {
    match command {
        RegistryCommand::Update(args) => update(options, args).await,
        RegistryCommand::Status(args) => {
            let installed = load_profile(options, &args).await?;
            print_installed(args.profile, &installed);
            Ok(())
        }
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
        RegistryCommand::Installed { command } => match command {
            RegistryInstalledCommand::Verify(args) => {
                let installed = load_profile(options, &args).await?;
                print_installed(args.profile, &installed);
                Ok(())
            }
            RegistryInstalledCommand::Inspect(args) => {
                let installed = load_profile(options, &args.profile).await?;
                print_installed(args.profile.profile, &installed);
                let entry = installed
                    .registry()
                    .entry(&args.entry_id)
                    .ok_or_else(|| AppError::Message("registry entry was not found".into()))?;
                print_entry(entry);
                Ok(())
            }
            RegistryInstalledCommand::Search(args) => {
                let installed = load_profile(options, &args.profile).await?;
                print_installed(args.profile.profile, &installed);
                let entries = installed.registry().search(&args.query);
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
        },
    }
}

async fn update(options: &Options, args: RegistryUpdateArgs) -> Result<(), AppError> {
    ensure_profile_initialized(args.profile)?;
    let (root, root_sha256) = load_profile_root(args.root.as_deref(), args.root_sha256.as_deref())?;
    let state_dir = profile_state_directory(options, args.profile)?;
    let installed = match (args.repository, args.url) {
        (Some(repository), None) => {
            update_directory_repository(
                &repository,
                &root,
                &root_sha256,
                &state_dir,
                Timestamp::now(),
            )
            .await
        }
        (None, Some(url)) => {
            let url = Url::parse(&url)
                .map_err(|_| AppError::Message("registry URL is invalid".into()))?;
            update_https_repository(&url, &root, &root_sha256, &state_dir, Timestamp::now()).await
        }
        _ => {
            return Err(AppError::Message(
                "select exactly one of --repository or --url".into(),
            ));
        }
    }
    .map_err(|error| failure(error.verification_status()))?;
    print_installed(args.profile, &installed);
    Ok(())
}

async fn load_profile(
    options: &Options,
    args: &RegistryProfileArgs,
) -> Result<InstalledRegistry, AppError> {
    ensure_profile_initialized(args.profile)?;
    let (root, root_sha256) = load_profile_root(args.root.as_deref(), args.root_sha256.as_deref())?;
    let state_dir = profile_state_directory(options, args.profile)?;
    load_installed_registry(&state_dir, &root, &root_sha256, Timestamp::now())
        .await
        .map_err(|error| failure(error.verification_status()))
}

fn ensure_profile_initialized(profile: RegistryProfile) -> Result<(), AppError> {
    match profile {
        RegistryProfile::Staging => Ok(()),
        RegistryProfile::Production => Err(AppError::Message(
            "production registry trust not initialized".into(),
        )),
    }
}

fn load_profile_root(
    root_path: Option<&Path>,
    root_sha256: Option<&str>,
) -> Result<(Vec<u8>, [u8; 32]), AppError> {
    let path = root_path.ok_or_else(|| {
        AppError::Message("staging requires an out-of-band bootstrap root".into())
    })?;
    let pin = root_sha256.ok_or_else(|| {
        AppError::Message("staging requires an independent --root-sha256 pin".into())
    })?;
    let root = read_bootstrap_root(path)
        .map_err(|_| AppError::Message("bootstrap root file is unavailable or invalid".into()))?;
    let root_sha256 = equalturn_protocol::decode_hex::<32>(pin).map_err(|_| {
        AppError::Message("bootstrap root pin must be 64 lowercase hex digits".into())
    })?;
    equalturn_registry::verify_bootstrap_root(&root, &root_sha256).map_err(|_| {
        AppError::Message("bootstrap root does not match its independent pin".into())
    })?;
    Ok((root, root_sha256))
}

fn profile_state_directory(
    options: &Options,
    profile: RegistryProfile,
) -> Result<std::path::PathBuf, AppError> {
    Ok(data_directory(options)?
        .join("registry")
        .join("profiles")
        .join(profile.as_str()))
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

fn print_installed(profile: RegistryProfile, installed: &InstalledRegistry) {
    println!("Profile: {}", profile.as_str().to_ascii_uppercase());
    println!("Verification: VALID");
    println!("Expiration state: valid at the current local clock");
    println!("Locally recorded source: {}", installed.source());
    println!(
        "Registry version: {}",
        installed.registry().registry_version()
    );
    println!(
        "Locally recorded refresh time: {}",
        installed.last_successful_refresh()
    );
    println!("TUF timestamp version: {}", installed.timestamp_version());
    println!("TUF snapshot version: {}", installed.snapshot_version());
    println!("TUF targets version: {}", installed.targets_version());
    println!("Trusted root version: {}", installed.trusted_root_version());
    println!("Entries: {}", installed.registry().entries().len());
    println!("Indicators: {}", installed.registry().indicator_count());
    println!("Target SHA-256: {}", encode_hex(installed.target_sha256()));
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
