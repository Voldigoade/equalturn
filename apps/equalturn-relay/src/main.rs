use std::time::Duration;
use std::{net::SocketAddr, path::PathBuf, process::ExitCode, sync::Arc};

use clap::{Args, Parser, Subcommand};
use equalturn_relay_core::{
    Relay, RelayConfig, TlsConfig, load_or_create_relay_identity, run_server,
};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "equalturn-relay", version, about = "Equalturn session relay")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the PostgreSQL-backed session relay.
    Serve(ServeArgs),
}

#[derive(Args)]
struct ServeArgs {
    /// PostgreSQL connection URL. May also be set with EQUALTURN_DATABASE_URL.
    #[arg(long, env = "EQUALTURN_DATABASE_URL", value_name = "URL")]
    database_url: Option<String>,

    /// Relay signing identity file. Created with owner-only permissions when absent.
    #[arg(long, default_value = "data/relay-ed25519.key", value_name = "PATH")]
    identity_file: PathBuf,

    /// Bind address. Plain HTTP is permitted only on loopback in explicit development mode.
    #[arg(long, default_value = "127.0.0.1:8787", value_name = "IP:PORT")]
    listen: SocketAddr,

    /// TLS certificate in PEM format. Must be provided together with --tls-key.
    #[arg(long, value_name = "PATH")]
    tls_cert: Option<PathBuf>,

    /// TLS private key in PEM format. Must be provided together with --tls-cert.
    #[arg(long, value_name = "PATH")]
    tls_key: Option<PathBuf>,

    /// Permit unencrypted HTTP only when --listen uses a loopback address.
    #[arg(long)]
    allow_insecure_loopback: bool,

    /// Maximum PostgreSQL connection pool size.
    #[arg(long, default_value_t = 32, value_parser = clap::value_parser!(u32).range(1..=256))]
    max_connections: u32,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("equalturn-relay: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), &'static str> {
    let cli = Cli::parse();
    init_logging()?;

    match cli.command {
        Command::Serve(args) => serve(args).await,
    }
}

async fn serve(args: ServeArgs) -> Result<(), &'static str> {
    let database_url = args
        .database_url
        .ok_or("database URL is required via --database-url or EQUALTURN_DATABASE_URL")?;
    let tls = transport_config(
        args.listen,
        args.tls_cert,
        args.tls_key,
        args.allow_insecure_loopback,
    )?;

    let identity = load_or_create_relay_identity(&args.identity_file)
        .map_err(|_| "could not load or create the relay signing identity")?;
    let relay = Relay::connect(
        &RelayConfig {
            database_url,
            max_connections: args.max_connections,
        },
        identity,
    )
    .await
    .map_err(|_| "could not initialize relay storage or identity")?;

    let maintenance_relay = Arc::clone(&relay);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            match maintenance_relay.cleanup_retained_sessions().await {
                Ok(removed) if removed > 0 => {
                    tracing::info!(removed_sessions = removed, "expired session data removed");
                }
                Ok(_) => {}
                Err(_) => tracing::error!("relay retention cleanup failed"),
            }
        }
    });

    tracing::info!(
        tls_enabled = tls.is_some(),
        max_connections = args.max_connections,
        "relay is starting"
    );
    run_server(
        Arc::clone(&relay),
        args.listen,
        tls,
        args.allow_insecure_loopback,
    )
    .await
    .map_err(|_| "relay listener failed; check the bind address, TLS files, and permissions")
}

fn init_logging() -> Result<(), &'static str> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_current_span(false)
        .with_span_list(false)
        .try_init()
        .map_err(|_| "could not initialize structured logging")
}

fn transport_config(
    listen: SocketAddr,
    certificate: Option<PathBuf>,
    private_key: Option<PathBuf>,
    allow_insecure_loopback: bool,
) -> Result<Option<TlsConfig>, &'static str> {
    match (certificate, private_key) {
        (Some(certificate_path), Some(private_key_path)) => {
            if allow_insecure_loopback {
                return Err("choose TLS or explicit insecure loopback mode, not both");
            }
            Ok(Some(TlsConfig {
                certificate_path,
                private_key_path,
            }))
        }
        (None, None) if allow_insecure_loopback && listen.ip().is_loopback() => Ok(None),
        (None, None) if allow_insecure_loopback => {
            Err("insecure HTTP is allowed only on a loopback listen address")
        }
        (None, None) => Err("TLS is required unless --allow-insecure-loopback is used on loopback"),
        _ => Err("--tls-cert and --tls-key must be provided together"),
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, path::PathBuf};

    use super::{Cli, transport_config};
    use clap::Parser;

    fn address(value: &str) -> SocketAddr {
        value.parse().expect("test address parses")
    }

    #[test]
    fn plaintext_requires_explicit_loopback_mode() {
        assert!(transport_config(address("127.0.0.1:8787"), None, None, false).is_err());
        assert!(transport_config(address("127.0.0.1:8787"), None, None, true).is_ok());
        assert!(transport_config(address("0.0.0.0:8787"), None, None, true).is_err());
    }

    #[test]
    fn tls_paths_must_be_a_pair() {
        assert!(
            transport_config(
                address("0.0.0.0:443"),
                Some(PathBuf::from("relay.crt")),
                None,
                false,
            )
            .is_err()
        );

        assert!(
            transport_config(
                address("0.0.0.0:443"),
                Some(PathBuf::from("relay.crt")),
                Some(PathBuf::from("relay.key")),
                false,
            )
            .is_ok()
        );
    }

    #[test]
    fn cli_accepts_database_url_flag() {
        let parsed = Cli::try_parse_from([
            "equalturn-relay",
            "serve",
            "--database-url",
            "postgres://user:secret@localhost/db",
            "--tls-cert",
            "relay.crt",
            "--tls-key",
            "relay.key",
        ]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn help_does_not_include_database_url_values() {
        let Err(help) = Cli::try_parse_from(["equalturn-relay", "serve", "--help"]) else {
            panic!("help flag should exit argument parsing");
        };
        let rendered = help.to_string();
        assert!(!rendered.contains("postgres://"));
    }
}
