use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "equalturn",
    version,
    about = "Pseudonymous integrity sessions for technical competitions"
)]
pub struct Cli {
    #[command(flatten)]
    pub options: Options,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Clone)]
pub struct Options {
    /// HTTPS relay URL. Plain HTTP is accepted only for explicit loopback development.
    #[arg(long, global = true, env = "EQUALTURN_RELAY_URL", value_name = "URL")]
    pub relay_url: Option<String>,

    /// Owner-private local client data directory.
    #[arg(long, global = true, env = "EQUALTURN_DATA_DIR", value_name = "PATH")]
    pub data_dir: Option<PathBuf>,

    /// Permit unencrypted HTTP only when the relay URL is loopback.
    #[arg(long, global = true)]
    pub allow_insecure_loopback: bool,

    /// Increase diagnostic logging. May be repeated.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create or manage a multiplayer session.
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// Submit an explicitly synthetic integrity event.
    Event {
        #[command(subcommand)]
        command: EventCommand,
    },
    /// Export or independently verify an evidence report.
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
    /// Inspect a locally stored registry only after TUF verification.
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
}

#[derive(Subcommand)]
pub enum RegistryCommand {
    /// Explicitly fetch and atomically install a registry for one trust profile.
    Update(RegistryUpdateArgs),
    /// Verify and display the locally installed profile without network access.
    Status(RegistryProfileArgs),
    /// Verify a local TUF repository against an independently pinned root.
    Verify(RegistrySourceArgs),
    /// Inspect one entry from a verified local registry repository.
    Inspect(RegistryInspectArgs),
    /// Search a verified local registry repository.
    Search(RegistrySearchArgs),
    /// Verify, inspect, or search the locally installed profile snapshot.
    Installed {
        #[command(subcommand)]
        command: RegistryInstalledCommand,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum RegistryProfile {
    Staging,
    Production,
}

impl RegistryProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

#[derive(Args)]
pub struct RegistryUpdateArgs {
    /// Select the staging or production registry trust profile.
    #[arg(long, value_enum)]
    pub profile: RegistryProfile,

    /// Local TUF repository. Mutually exclusive with --url.
    #[arg(long, value_name = "PATH")]
    pub repository: Option<PathBuf>,

    /// HTTPS TUF repository base URL. Mutually exclusive with --repository.
    #[arg(long, value_name = "URL")]
    pub url: Option<String>,

    /// Out-of-band trusted root metadata file.
    #[arg(long, value_name = "PATH")]
    pub root: Option<PathBuf>,

    /// Independently distributed SHA-256 pin for --root.
    #[arg(long, value_name = "HEX")]
    pub root_sha256: Option<String>,
}

#[derive(Args)]
pub struct RegistryProfileArgs {
    /// Select the staging or production registry trust profile.
    #[arg(long, value_enum)]
    pub profile: RegistryProfile,

    /// Out-of-band trusted root metadata file.
    #[arg(long, value_name = "PATH")]
    pub root: Option<PathBuf>,

    /// Independently distributed SHA-256 pin for --root.
    #[arg(long, value_name = "HEX")]
    pub root_sha256: Option<String>,
}

#[derive(Subcommand)]
pub enum RegistryInstalledCommand {
    /// Verify the selected installed snapshot offline.
    Verify(RegistryProfileArgs),
    /// Inspect an entry from the selected installed snapshot offline.
    Inspect(RegistryInstalledInspectArgs),
    /// Search the selected installed snapshot offline.
    Search(RegistryInstalledSearchArgs),
}

#[derive(Args)]
pub struct RegistryInstalledInspectArgs {
    #[command(flatten)]
    pub profile: RegistryProfileArgs,
    /// Exact registry entry identifier.
    #[arg(value_name = "ENTRY_ID")]
    pub entry_id: String,
}

#[derive(Args)]
pub struct RegistryInstalledSearchArgs {
    #[command(flatten)]
    pub profile: RegistryProfileArgs,
    /// Case-insensitive provider, product, alias, or indicator query.
    #[arg(value_name = "QUERY")]
    pub query: String,
}

#[derive(Args, Clone)]
pub struct RegistrySourceArgs {
    /// Local directory containing metadata/ and targets/.
    #[arg(value_name = "REPOSITORY")]
    pub repository: PathBuf,

    /// Out-of-band trusted root metadata file.
    #[arg(long, value_name = "PATH")]
    pub root: PathBuf,

    /// Independently distributed SHA-256 pin for --root.
    #[arg(long, value_name = "HEX")]
    pub root_sha256: String,

    /// Persistent metadata cache; defaults below the client data directory.
    #[arg(long, value_name = "PATH")]
    pub cache_dir: Option<PathBuf>,
}

#[derive(Args)]
pub struct RegistryInspectArgs {
    #[command(flatten)]
    pub source: RegistrySourceArgs,

    /// Exact registry entry identifier.
    #[arg(value_name = "ENTRY_ID")]
    pub entry_id: String,
}

#[derive(Args)]
pub struct RegistrySearchArgs {
    #[command(flatten)]
    pub source: RegistrySourceArgs,

    /// Case-insensitive provider, product, alias, or indicator query.
    #[arg(value_name = "QUERY")]
    pub query: String,
}

#[derive(Subcommand)]
pub enum SessionCommand {
    /// Create a session and a high-entropy invitation code.
    Create(CreateArgs),
    /// Join an existing session with an invitation code.
    Join(JoinArgs),
    /// Start the session. Only the creator can do this.
    Start(LocalSessionArgs),
    /// Show session state and pseudonymous participants.
    Status(RemoteSessionArgs),
    /// List participants without network identifiers.
    Participants(RemoteSessionArgs),
    /// Maintain the authenticated connection and heartbeat until it ends.
    Watch(LocalSessionArgs),
    /// Resend locally queued signed events in sequence order.
    Sync(LocalSessionArgs),
    /// End the session. Only the creator can do this.
    End(LocalSessionArgs),
}

#[derive(Args)]
pub struct CreateArgs {
    /// Public session nickname for a new session.
    #[arg(long, required_unless_present = "resume_session")]
    pub nickname: Option<String>,

    /// Resume a locally queued creation after a relay interruption.
    #[arg(long, requires = "invitation_code", value_name = "SESSION_ID")]
    pub resume_session: Option<String>,

    /// Invitation code displayed by the interrupted create command.
    #[arg(long, requires = "resume_session", value_name = "CODE")]
    pub invitation_code: Option<String>,
}

#[derive(Args)]
pub struct JoinArgs {
    /// Invitation code supplied by the session creator.
    pub invitation_code: String,
    /// Public session nickname.
    #[arg(long)]
    pub nickname: String,
}

#[derive(Args)]
pub struct LocalSessionArgs {
    /// Session UUID.
    #[arg(long, value_name = "UUID")]
    pub session: String,
    /// Select a local participant identity when multiple identities exist.
    #[arg(long, value_name = "UUID")]
    pub participant: Option<String>,
}

#[derive(Args)]
pub struct RemoteSessionArgs {
    /// Session UUID.
    #[arg(long, value_name = "UUID")]
    pub session: String,
}

#[derive(Subcommand)]
pub enum EventCommand {
    Simulate {
        /// Session UUID.
        #[arg(long, value_name = "UUID")]
        session: String,
        /// Synthetic event category.
        #[arg(long, value_enum, default_value_t = SyntheticKind::AiAccessAttempt)]
        kind: SyntheticKind,
        /// Optional lowercase hostname. URLs and IP literals are rejected.
        #[arg(long)]
        target: Option<String>,
        /// Select a local participant identity when multiple identities exist.
        #[arg(long, value_name = "UUID")]
        participant: Option<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum SyntheticKind {
    AiAccessAttempt,
}

#[derive(Subcommand)]
pub enum ReportCommand {
    Export {
        /// Session UUID.
        #[arg(long, value_name = "UUID")]
        session: String,
        /// Destination for the machine-readable JSON evidence bundle.
        #[arg(long, value_name = "PATH")]
        json: PathBuf,
        /// Optional destination for a readable Markdown summary.
        #[arg(long, value_name = "PATH")]
        markdown: Option<PathBuf>,
        /// Select a local participant identity when multiple identities exist.
        #[arg(long, value_name = "UUID")]
        participant: Option<String>,
    },
    Verify {
        /// JSON evidence bundle to verify.
        report: PathBuf,
        /// Optional pinned relay public key in lowercase hexadecimal.
        #[arg(long, value_name = "HEX")]
        relay_key: Option<String>,
    },
}
