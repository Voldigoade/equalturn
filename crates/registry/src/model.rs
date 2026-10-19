use crate::json::{UniqueJsonError, parse_unique};
use jiff::{Timestamp, civil::Date, tz::TimeZone};
use jsonschema::Draft;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const REGISTRY_SCHEMA: &[u8] = include_bytes!("../../../registry/schema-v3.schema.json");

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("registry exceeds the {max_bytes}-byte input limit")]
    TooLarge { max_bytes: usize },
    #[error("registry is not valid UTF-8")]
    InvalidUtf8,
    #[error("registry JSON contains a duplicate object key")]
    DuplicateJsonKey,
    #[error("registry JSON is malformed")]
    MalformedJson,
    #[error("registry does not match the v3 schema: {0}")]
    Schema(String),
    #[error("registry semantic validation failed: {0}")]
    Semantic(String),
    #[error("bootstrap root exceeds the {max_bytes}-byte input limit")]
    RootTooLarge { max_bytes: usize },
    #[error("bootstrap root SHA-256 does not match the out-of-band pin")]
    BootstrapPinMismatch,
    #[error("local TUF repository is invalid or unavailable")]
    Repository,
    #[error("TUF metadata or target verification failed")]
    Tuf,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    schema_version: u32,
    registry_version: String,
    last_updated: String,
    purpose: RegistryPurpose,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RegistryPurpose {
    ResearchOnly,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    id: String,
    provider: Provider,
    product: String,
    categories: Vec<Category>,
    surfaces: Vec<RegistrySurface>,
    status: EntryStatus,
    sources: Vec<Source>,
    indicators: Vec<Indicator>,
    relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    id: String,
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    id: String,
    url: String,
    checked_on: String,
    indicator_assessment: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Indicator {
    kind: IndicatorKind,
    value: String,
    platforms: Vec<Platform>,
    surfaces: Vec<RegistrySurface>,
    scope: String,
    source_id: String,
    verified_on: String,
    confidence: Confidence,
    actionability: Actionability,
    false_positive_risk: FalsePositiveRisk,
    lifecycle: IndicatorLifecycle,
    limitations: String,
    ecosystem: Option<Ecosystem>,
    bind_scope: Option<BindScope>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    kind: RelationshipKind,
    target_id: String,
    notes: String,
}

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
        pub enum $name {
            $(#[serde(rename = $value)] $variant),+
        }
    };
}

string_enum!(Category {
    HostedChat => "hosted_chat",
    ProviderApi => "provider_api",
    Gateway => "gateway",
    CodingAssistant => "coding_assistant",
    IdeAssistant => "ide_assistant",
    BrowserExtension => "browser_extension",
    DesktopApp => "desktop_app",
    CliAgent => "cli_agent",
    SecurityTool => "security_tool",
    LocalRunner => "local_runner",
    InferenceServer => "inference_server",
    LocalWebUi => "local_web_ui",
    SelfHostedStack => "self_hosted_stack",
    CloudMarketplace => "cloud_marketplace",
    Multimodal => "multimodal",
    SearchResearch => "search_research",
    RegionalProvider => "regional_provider",
    Router => "router",
});

string_enum!(RegistrySurface {
    Web => "web",
    Api => "api",
    Desktop => "desktop",
    Ide => "ide",
    BrowserExtension => "browser_extension",
    Cli => "cli",
    LocalRuntime => "local_runtime",
    Server => "server",
    LocalWebUi => "local_web_ui",
    Cloud => "cloud",
    Mobile => "mobile",
    Browser => "browser",
    LocalServer => "local_server",
    PythonLibrary => "python_library",
    Unknown => "unknown",
});

string_enum!(EntryStatus {
    Observed => "observed",
    Unknown => "unknown",
    Deprecated => "deprecated",
    Archived => "archived",
    Disputed => "disputed",
});

string_enum!(IndicatorKind {
    WebHostname => "web_hostname",
    ApiHostname => "api_hostname",
    UrlPath => "url_path",
    ExecutableName => "executable_name",
    PackageName => "package_name",
    WindowsPackageIdentity => "windows_package_identity",
    MacosBundleId => "macos_bundle_id",
    LinuxDesktopId => "linux_desktop_id",
    VscodeExtensionId => "vscode_extension_id",
    JetbrainsPluginId => "jetbrains_plugin_id",
    BrowserExtensionId => "browser_extension_id",
    LocalPort => "local_port",
    ServiceName => "service_name",
    ContainerImage => "container_image",
    ProtocolEndpoint => "protocol_endpoint",
});

string_enum!(Platform {
    Windows => "windows",
    Linux => "linux",
    Macos => "macos",
    Android => "android",
    Ios => "ios",
    Freebsd => "freebsd",
    Other => "other",
    Unknown => "unknown",
});

string_enum!(Confidence {
    High => "high",
    Medium => "medium",
    Low => "low",
});

string_enum!(Actionability {
    ResearchOnly => "research_only",
    Advisory => "advisory",
    DetectionCandidate => "detection_candidate",
    BlockingCandidate => "blocking_candidate",
    UnsafeToBlock => "unsafe_to_block",
});

string_enum!(FalsePositiveRisk {
    Low => "low",
    Medium => "medium",
    High => "high",
    Unknown => "unknown",
});

string_enum!(IndicatorLifecycle {
    Current => "current",
    Deprecated => "deprecated",
    Archived => "archived",
    Disputed => "disputed",
    Unknown => "unknown",
});

string_enum!(RelationshipKind {
    RoutesTo => "routes_to",
    ProvidedBy => "provided_by",
    IntegratesWith => "integrates_with",
});

string_enum!(Ecosystem {
    Cargo => "cargo",
    Go => "go",
    Npm => "npm",
    Nuget => "nuget",
    Pypi => "pypi",
    Rubygems => "rubygems",
    Unknown => "unknown",
});

string_enum!(BindScope {
    Loopback => "loopback",
    SpecificInterface => "specific_interface",
    AnyInterface => "any_interface",
    Unknown => "unknown",
});

impl Registry {
    pub fn parse(bytes: &[u8]) -> Result<Self, RegistryError> {
        if bytes.len() > crate::REGISTRY_MAX_BYTES {
            return Err(RegistryError::TooLarge {
                max_bytes: crate::REGISTRY_MAX_BYTES,
            });
        }
        std::str::from_utf8(bytes).map_err(|_| RegistryError::InvalidUtf8)?;
        let value = parse_unique(bytes).map_err(|error| match error {
            UniqueJsonError::DuplicateKey => RegistryError::DuplicateJsonKey,
            UniqueJsonError::Malformed => RegistryError::MalformedJson,
        })?;
        validate_schema(&value)?;
        let registry: Self = serde_json::from_value(value)
            .map_err(|_| RegistryError::Schema("registry fields are malformed".into()))?;
        validate_semantics(&registry)?;
        Ok(registry)
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn registry_version(&self) -> &str {
        &self.registry_version
    }

    pub fn last_updated(&self) -> &str {
        &self.last_updated
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn indicator_count(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| entry.indicators.len())
            .sum()
    }

    pub fn search(&self, query: &str) -> Vec<&Entry> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        self.entries
            .iter()
            .filter(|entry| {
                entry.id.to_lowercase().contains(&needle)
                    || entry.product.to_lowercase().contains(&needle)
                    || entry.provider.id.to_lowercase().contains(&needle)
                    || entry.provider.name.to_lowercase().contains(&needle)
                    || entry
                        .provider
                        .aliases
                        .iter()
                        .any(|alias| alias.to_lowercase().contains(&needle))
                    || entry
                        .indicators
                        .iter()
                        .any(|indicator| indicator.value.to_lowercase().contains(&needle))
            })
            .collect()
    }
}

impl Entry {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn provider(&self) -> &Provider {
        &self.provider
    }
    pub fn product(&self) -> &str {
        &self.product
    }
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }
    pub fn surfaces(&self) -> &[RegistrySurface] {
        &self.surfaces
    }
    pub fn status(&self) -> EntryStatus {
        self.status
    }
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }
    pub fn indicators(&self) -> &[Indicator] {
        &self.indicators
    }
    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }
}

impl Provider {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }
}

impl Source {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn checked_on(&self) -> &str {
        &self.checked_on
    }
    pub fn indicator_assessment(&self) -> &str {
        &self.indicator_assessment
    }
}

impl Indicator {
    pub fn kind(&self) -> IndicatorKind {
        self.kind
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn platforms(&self) -> &[Platform] {
        &self.platforms
    }
    pub fn surfaces(&self) -> &[RegistrySurface] {
        &self.surfaces
    }
    pub fn scope(&self) -> &str {
        &self.scope
    }
    pub fn source_id(&self) -> &str {
        &self.source_id
    }
    pub fn verified_on(&self) -> &str {
        &self.verified_on
    }
    pub fn confidence(&self) -> Confidence {
        self.confidence
    }
    pub fn actionability(&self) -> Actionability {
        self.actionability
    }
    pub fn false_positive_risk(&self) -> FalsePositiveRisk {
        self.false_positive_risk
    }
    pub fn lifecycle(&self) -> IndicatorLifecycle {
        self.lifecycle
    }
    pub fn limitations(&self) -> &str {
        &self.limitations
    }
    pub fn ecosystem(&self) -> Option<Ecosystem> {
        self.ecosystem
    }
    pub fn bind_scope(&self) -> Option<BindScope> {
        self.bind_scope
    }
}

impl Relationship {
    pub fn kind(&self) -> RelationshipKind {
        self.kind
    }
    pub fn target_id(&self) -> &str {
        &self.target_id
    }
    pub fn notes(&self) -> &str {
        &self.notes
    }
}

fn validate_schema(value: &Value) -> Result<(), RegistryError> {
    let schema: Value = serde_json::from_slice(REGISTRY_SCHEMA)
        .map_err(|_| RegistryError::Schema("embedded registry schema is invalid".into()))?;
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|_| {
            RegistryError::Schema("embedded registry schema could not be compiled".into())
        })?;
    validator
        .validate(value)
        .map_err(|error| RegistryError::Schema(safe_diagnostic(error.to_string())))
}

fn safe_diagnostic(message: String) -> String {
    let mut characters = message.chars();
    let limited: String = characters.by_ref().take(400).collect();
    if characters.next().is_some() {
        format!("{limited}…")
    } else {
        limited
    }
}

fn validate_semantics(registry: &Registry) -> Result<(), RegistryError> {
    if registry.schema_version != 3
        || registry.purpose != RegistryPurpose::ResearchOnly
        || !is_semver(&registry.registry_version)
    {
        return Err(RegistryError::Semantic(
            "unsupported schema or non-canonical semantic version".into(),
        ));
    }
    let today = Timestamp::now().to_zoned(TimeZone::UTC).date();
    validate_date(&registry.last_updated, today)?;
    let mut ids = BTreeSet::new();
    let mut indicators = BTreeMap::new();
    for entry in &registry.entries {
        if !ids.insert(entry.id.as_str()) {
            return Err(RegistryError::Semantic("duplicate product entry ID".into()));
        }
    }
    for entry in &registry.entries {
        let source_ids: BTreeMap<_, _> = entry
            .sources
            .iter()
            .map(|source| (source.id.as_str(), source))
            .collect();
        if source_ids.len() != entry.sources.len() {
            return Err(RegistryError::Semantic(
                "duplicate source ID within product entry".into(),
            ));
        }
        for source in &entry.sources {
            validate_date(&source.checked_on, today)?;
        }
        for indicator in &entry.indicators {
            if indicators
                .insert((indicator.kind, indicator.value.to_lowercase()), ())
                .is_some()
            {
                return Err(RegistryError::Semantic(
                    "duplicate normalized indicator".into(),
                ));
            }
            let source = source_ids
                .get(indicator.source_id.as_str())
                .ok_or_else(|| {
                    RegistryError::Semantic(
                        "indicator provenance does not resolve in its product entry".into(),
                    )
                })?;
            validate_date(&indicator.verified_on, today)?;
            if indicator.verified_on != source.checked_on {
                return Err(RegistryError::Semantic(
                    "indicator verification date does not match its cited source date".into(),
                ));
            }
            validate_indicator(indicator)?;
        }
        for relationship in &entry.relationships {
            if relationship.target_id == entry.id || !ids.contains(relationship.target_id.as_str())
            {
                return Err(RegistryError::Semantic(
                    "relationship target is unresolved or self-referential".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_date(value: &str, today: Date) -> Result<(), RegistryError> {
    let parsed: Date = value
        .parse()
        .map_err(|_| RegistryError::Semantic("invalid ISO calendar date".into()))?;
    if parsed.to_string() != value || parsed > today {
        return Err(RegistryError::Semantic(
            "date is non-canonical or in the future".into(),
        ));
    }
    Ok(())
}

fn is_semver(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && [major, minor, patch].iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == &"0" || !part.starts_with('0'))
        })
}

fn validate_indicator(indicator: &Indicator) -> Result<(), RegistryError> {
    if indicator.platforms.contains(&Platform::Unknown) && indicator.platforms.len() != 1
        || indicator.surfaces.contains(&RegistrySurface::Unknown) && indicator.surfaces.len() != 1
    {
        return Err(RegistryError::Semantic(
            "unknown indicator scope cannot be mixed with known values".into(),
        ));
    }
    if indicator.actionability == Actionability::BlockingCandidate
        && (indicator.confidence != Confidence::High
            || indicator.false_positive_risk != FalsePositiveRisk::Low
            || indicator.lifecycle != IndicatorLifecycle::Current
            || indicator.platforms.contains(&Platform::Unknown)
            || indicator.surfaces.contains(&RegistrySurface::Unknown))
    {
        return Err(RegistryError::Semantic(
            "blocking candidate lacks the required evidence and scope".into(),
        ));
    }
    if indicator.actionability == Actionability::UnsafeToBlock
        && indicator.false_positive_risk != FalsePositiveRisk::High
    {
        return Err(RegistryError::Semantic(
            "unsafe_to_block must carry high false-positive risk".into(),
        ));
    }
    if matches!(
        indicator.kind,
        IndicatorKind::WebHostname | IndicatorKind::ApiHostname
    ) {
        validate_hostname(&indicator.value)?;
    }
    if indicator.kind == IndicatorKind::LocalPort {
        let port = indicator.value.parse::<u16>().ok();
        if port.is_none_or(|port| port == 0 || port.to_string() != indicator.value) {
            return Err(RegistryError::Semantic(
                "local port is not a canonical port number".into(),
            ));
        }
    }
    Ok(())
}

fn validate_hostname(value: &str) -> Result<(), RegistryError> {
    let host = value.strip_prefix("*.").unwrap_or(value);
    if value.contains('*') && value != format!("*.{host}")
        || value.contains("://")
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
        || !host.is_ascii()
        || host != host.to_ascii_lowercase()
        || host.ends_with('.')
        || host.len() > 253
        || host.parse::<std::net::IpAddr>().is_ok()
        || host.split('.').count() < 2
    {
        return Err(RegistryError::Semantic(
            "hostname indicator is not normalized".into(),
        ));
    }
    for label in host.split('.') {
        let bytes = label.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 63
            || !bytes[0].is_ascii_alphanumeric()
            || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
            || !bytes
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
        {
            return Err(RegistryError::Semantic(
                "hostname indicator has an invalid DNS label".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_semver;

    #[test]
    fn accepts_only_canonical_semver_triplets() {
        assert!(is_semver("0.4.0"));
        assert!(!is_semver("01.4.0"));
        assert!(!is_semver("1.0.0-alpha"));
        assert!(!is_semver("1.0"));
    }
}
