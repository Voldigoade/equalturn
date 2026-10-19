use std::{env, io, path::PathBuf, time::SystemTime};

use equalturn_client_core::{
    ClientError, ClientStore, RelayClient, RelayClientError, SessionProfile,
};
use equalturn_protocol::{ParticipantId, ProtocolError, SessionId};
use thiserror::Error;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use crate::{
    cli::{Cli, Command, ReportCommand},
    commands::{event, report, session},
};

pub struct Context {
    pub store: ClientStore,
    pub relay: RelayClient,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("{0}")]
    Queued(String),
    #[error(transparent)]
    Relay(#[from] RelayClientError),
    #[error(transparent)]
    Store(#[from] ClientError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Report(#[from] equalturn_report::ReportError),
    #[error("local file operation failed")]
    Io(#[from] io::Error),
    #[error("report JSON is malformed")]
    Json(#[from] serde_json::Error),
}

impl Context {
    pub async fn open(options: &crate::cli::Options) -> Result<Self, AppError> {
        let relay_url = options
            .relay_url
            .as_deref()
            .ok_or_else(|| AppError::Message("set --relay-url or EQUALTURN_RELAY_URL".into()))?;
        let relay = RelayClient::new(relay_url, options.allow_insecure_loopback)?;
        let store = ClientStore::open(data_directory(options)?).await?;
        Ok(Self { store, relay })
    }
}

pub async fn execute(cli: Cli) -> Result<(), AppError> {
    init_logging(cli.options.verbose)?;
    match cli.command {
        Command::Report {
            command: ReportCommand::Verify { report, relay_key },
        } => report::verify(&report, relay_key.as_deref()),
        command => {
            let context = Context::open(&cli.options).await?;
            match command {
                Command::Session { command } => session::run(&context, command).await,
                Command::Event { command } => event::run(&context, command).await,
                Command::Report { command } => report::run(&context, command).await,
            }
        }
    }
}

pub fn parse_session_id(value: &str) -> Result<SessionId, AppError> {
    let uuid = Uuid::parse_str(value)
        .map_err(|_| AppError::Message("session ID must be a UUIDv4".into()))?;
    let session_id = SessionId::from(uuid);
    if !session_id.is_v4() {
        return Err(AppError::Message("session ID must be a UUIDv4".into()));
    }
    Ok(session_id)
}

pub fn parse_participant_id(value: &str) -> Result<ParticipantId, AppError> {
    let uuid = Uuid::parse_str(value)
        .map_err(|_| AppError::Message("participant ID must be a UUIDv4".into()))?;
    let participant_id = ParticipantId::from(uuid);
    if !participant_id.is_v4() {
        return Err(AppError::Message("participant ID must be a UUIDv4".into()));
    }
    Ok(participant_id)
}

pub async fn select_profile(
    store: &ClientStore,
    session_id: SessionId,
    participant_selector: Option<&str>,
) -> Result<SessionProfile, AppError> {
    let profiles = store.profiles_for_session(session_id).await?;
    if let Some(selector) = participant_selector {
        let selected = parse_participant_id(selector)?;
        return profiles
            .into_iter()
            .find(|profile| profile.participant_id == selected)
            .ok_or_else(|| AppError::Message("local participant profile was not found".into()));
    }
    match profiles.as_slice() {
        [profile] => Ok(profile.clone()),
        [] => Err(AppError::Message(
            "no local participant profile exists for this session".into(),
        )),
        _ => Err(AppError::Message(
            "multiple local profiles exist; select one with --participant".into(),
        )),
    }
}

pub fn validate_nickname(value: &str) -> Result<(), AppError> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 32
        || !bytes[0].is_ascii_alphanumeric()
        || bytes[1..]
            .iter()
            .any(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_' && *byte != b'-')
    {
        return Err(AppError::Message(
            "nickname must match [A-Za-z0-9][A-Za-z0-9_-]{0,31}".into(),
        ));
    }
    Ok(())
}

pub fn client_metadata() -> (equalturn_protocol::OsFamily, &'static str) {
    #[cfg(target_os = "windows")]
    {
        (equalturn_protocol::OsFamily::Windows, "windows")
    }
    #[cfg(target_os = "linux")]
    {
        (equalturn_protocol::OsFamily::Linux, "linux")
    }
    #[cfg(target_os = "macos")]
    {
        (equalturn_protocol::OsFamily::Other, "macos")
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        (equalturn_protocol::OsFamily::Unknown, "unknown")
    }
}

pub fn unix_time_ms() -> Result<u64, AppError> {
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|_| AppError::Message("system clock is before the Unix epoch".into()))?;
    u64::try_from(duration.as_millis())
        .map_err(|_| AppError::Message("system clock value is out of range".into()))
}

fn data_directory(options: &crate::cli::Options) -> Result<PathBuf, AppError> {
    if let Some(path) = &options.data_dir {
        return Ok(path.clone());
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(path) = env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(path).join("Equalturn"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(path) = env::var_os("HOME") {
            return Ok(PathBuf::from(path)
                .join("Library")
                .join("Application Support")
                .join("Equalturn"));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(path) = env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(path).join("equalturn"));
        }
        if let Some(path) = env::var_os("HOME") {
            return Ok(PathBuf::from(path)
                .join(".local")
                .join("share")
                .join("equalturn"));
        }
    }
    Ok(env::current_dir()?.join(".equalturn-data"))
}

fn init_logging(verbose: u8) -> Result<(), AppError> {
    let default_level = match verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|_| AppError::Message("could not initialize diagnostic logging".into()))
}
