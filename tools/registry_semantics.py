"""Semantic checks for the research-only v3 registry contract."""

from __future__ import annotations

import ipaddress
import re
from dataclasses import dataclass
from datetime import date
from urllib.parse import urlsplit, urlunsplit


@dataclass(frozen=True)
class Diagnostic:
    severity: str
    code: str
    path: str
    message: str

    def render(self) -> str:
        return f"{self.severity} {self.code} {self.path}: {self.message}"


GENERIC_EXECUTABLES = {
    "python", "python3", "node", "powershell", "pwsh", "chrome", "firefox",
    "docker", "localhost",
}
EXECUTABLE_PATTERN = re.compile(r"^[a-z0-9][a-z0-9._+-]{0,127}$")
DNS_LABEL_PATTERN = re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$")
SEMVER_PATTERN = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
EXTENSION_ID_PATTERN = re.compile(r"^[a-z0-9][a-z0-9-]{0,99}\.[a-z0-9][a-z0-9-]{0,99}$")
NPM_PACKAGE_PATTERN = re.compile(r"^(?:@[a-z0-9][a-z0-9._-]{0,99}/)?[a-z0-9][a-z0-9._-]{0,99}$")
PYPI_PACKAGE_PATTERN = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
SIMPLE_PACKAGE_PATTERN = re.compile(r"^[a-z0-9][a-z0-9._-]{0,199}$")
DESKTOP_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+\.desktop$")
WINDOWS_PACKAGE_ID_PATTERN = re.compile(r"^[A-Za-z0-9][A-Za-z0-9.-]*_[A-Za-z0-9.-]+$")
CONTAINER_SEGMENT_PATTERN = re.compile(r"^[a-z0-9]+(?:[._-][a-z0-9]+)*$")
CONTAINER_TAG_PATTERN = re.compile(r"^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$")


def _path(parts: tuple[object, ...] | list[object]) -> str:
    result = "$"
    for part in parts:
        result += f"[{part}]" if isinstance(part, int) else f".{part}"
    return result


def _diag(severity: str, code: str, path: str, message: str) -> Diagnostic:
    return Diagnostic(severity, code, path, message)


def _date_value(value: object, path: str, today: date, diagnostics: list[Diagnostic]) -> date | None:
    if not isinstance(value, str):
        diagnostics.append(_diag("ERROR", "E_DATE_INVALID", path, "expected an ISO-8601 calendar date"))
        return None
    try:
        parsed = date.fromisoformat(value)
    except ValueError:
        diagnostics.append(_diag("ERROR", "E_DATE_INVALID", path, "expected a valid ISO-8601 calendar date"))
        return None
    if parsed.isoformat() != value:
        diagnostics.append(_diag("ERROR", "E_DATE_NORMALIZATION", path, "date must use canonical YYYY-MM-DD form"))
    if parsed > today:
        diagnostics.append(_diag("ERROR", "E_FUTURE_DATE", path, "verification dates cannot be in the future"))
    return parsed


def _valid_hostname(value: str, path: str, diagnostics: list[Diagnostic], allow_wildcard: bool = True) -> str | None:
    original = value
    if "://" in value or "/" in value or "\\" in value or ":" in value:
        diagnostics.append(_diag("ERROR", "E_HOSTNAME_INVALID", path, "hostname fields contain a hostname only, without scheme, path, or port"))
        return None
    if not value.isascii():
        diagnostics.append(_diag("ERROR", "E_HOSTNAME_NORMALIZATION", path, "hostname must use ASCII IDNA form"))
        return None
    if value != value.lower():
        diagnostics.append(_diag("ERROR", "E_HOSTNAME_NORMALIZATION", path, "hostname must be lowercase"))
        return None
    if value.endswith("."):
        diagnostics.append(_diag("ERROR", "E_HOSTNAME_NORMALIZATION", path, "hostname must not have a trailing dot"))
        return None
    if "*" in value:
        if not allow_wildcard or not value.startswith("*.") or value.count("*") != 1:
            diagnostics.append(_diag("ERROR", "E_HOSTNAME_WILDCARD", path, "wildcard is allowed only as one leading `*.` label"))
            return None
        value = value[2:]
    if len(value) > 253 or not value:
        diagnostics.append(_diag("ERROR", "E_HOSTNAME_INVALID", path, "hostname length is outside the DNS limit"))
        return None
    try:
        ipaddress.ip_address(value)
    except ValueError:
        pass
    else:
        diagnostics.append(_diag("ERROR", "E_HOSTNAME_INVALID", path, "IP literals are not hostname indicators"))
        return None
    labels = value.split(".")
    if len(labels) < 2 or any(not DNS_LABEL_PATTERN.fullmatch(label) for label in labels):
        diagnostics.append(_diag("ERROR", "E_HOSTNAME_INVALID", path, "hostname must contain valid DNS labels"))
        return None
    return original


def _canonical_source_url(value: str, path: str, diagnostics: list[Diagnostic]) -> str | None:
    try:
        parsed = urlsplit(value)
        port = parsed.port
    except ValueError:
        diagnostics.append(_diag("ERROR", "E_SOURCE_URL_INVALID", path, "source URL has an invalid authority or port"))
        return None
    if parsed.scheme.lower() not in {"http", "https"} or not parsed.hostname:
        diagnostics.append(_diag("ERROR", "E_SOURCE_URL_INVALID", path, "source URL must be an absolute HTTP(S) URL"))
        return None
    if parsed.username is not None or parsed.password is not None:
        diagnostics.append(_diag("ERROR", "E_SOURCE_URL_CREDENTIALS", path, "source URLs must not contain user information"))
        return None
    hostname = parsed.hostname
    if not hostname.isascii() or hostname != hostname.lower() or hostname.endswith("."):
        diagnostics.append(_diag("ERROR", "E_SOURCE_URL_NORMALIZATION", path, "source URL hostname must be lowercase ASCII without a trailing dot"))
        return None
    authority = hostname if port is None else f"{hostname}:{port}"
    return urlunsplit((parsed.scheme.lower(), authority, parsed.path or "/", parsed.query, parsed.fragment))


def _validate_package(indicator: dict, path: str, diagnostics: list[Diagnostic]) -> None:
    value = indicator["value"]
    ecosystem = indicator["ecosystem"]
    patterns = {
        "npm": NPM_PACKAGE_PATTERN,
        "pypi": PYPI_PACKAGE_PATTERN,
        "cargo": SIMPLE_PACKAGE_PATTERN,
        "go": re.compile(r"^[a-z0-9][a-z0-9._~-]*(?:/[a-z0-9][a-z0-9._~-]*)*$"),
        "nuget": SIMPLE_PACKAGE_PATTERN,
        "rubygems": SIMPLE_PACKAGE_PATTERN,
        "unknown": re.compile(r"^(?:@[a-z0-9][a-z0-9._-]{0,99}/)?[a-z0-9][a-z0-9._/-]{0,199}$"),
    }
    if value != value.lower() or not patterns[ecosystem].fullmatch(value):
        diagnostics.append(_diag("ERROR", "E_PACKAGE_NAME_INVALID", f"{path}.value", f"package name is not normalized for ecosystem {ecosystem}"))


def _validate_container_image(value: str, path: str, diagnostics: list[Diagnostic]) -> None:
    if not value or value != value.lower() or " " in value or "\\" in value or "://" in value:
        diagnostics.append(_diag("ERROR", "E_CONTAINER_IMAGE_INVALID", path, "container image must be a lowercase OCI image reference without a URL scheme"))
        return
    name_and_tag, at, digest = value.partition("@")
    if at and not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
        diagnostics.append(_diag("ERROR", "E_CONTAINER_IMAGE_INVALID", path, "container digest must use sha256 and 64 lowercase hexadecimal digits"))
        return
    last_slash = name_and_tag.rfind("/")
    last_colon = name_and_tag.rfind(":")
    if last_colon > last_slash:
        name, tag = name_and_tag[:last_colon], name_and_tag[last_colon + 1:]
        if not CONTAINER_TAG_PATTERN.fullmatch(tag):
            diagnostics.append(_diag("ERROR", "E_CONTAINER_IMAGE_INVALID", path, "container image tag has invalid syntax"))
            return
    else:
        name = name_and_tag
    parts = name.split("/")
    if not parts or any(not part for part in parts):
        diagnostics.append(_diag("ERROR", "E_CONTAINER_IMAGE_INVALID", path, "container image has an empty path component"))
        return
    first = parts[0]
    image_parts = parts[1:] if ("." in first or ":" in first or first == "localhost") else parts
    if not image_parts or any(not CONTAINER_SEGMENT_PATTERN.fullmatch(part) for part in image_parts):
        diagnostics.append(_diag("ERROR", "E_CONTAINER_IMAGE_INVALID", path, "container image repository path is not normalized"))


def _validate_indicator_value(indicator: dict, path: str, diagnostics: list[Diagnostic]) -> None:
    kind = indicator["kind"]
    value = indicator["value"]
    value_path = f"{path}.value"
    if "\x00" in value or any(0xD800 <= ord(character) <= 0xDFFF for character in value):
        diagnostics.append(_diag("ERROR", "E_INDICATOR_TEXT_INVALID", value_path, "indicator contains NUL or an unpaired Unicode surrogate"))
        return
    if kind in {"web_hostname", "api_hostname"}:
        _valid_hostname(value, value_path, diagnostics)
    elif kind == "url_path":
        try:
            parsed = urlsplit(value)
            port = parsed.port
        except ValueError:
            diagnostics.append(_diag("ERROR", "E_URL_PATH_INVALID", value_path, "URL path indicator has an invalid authority"))
            return
        if parsed.scheme != "https" or not parsed.hostname or parsed.username is not None or parsed.password is not None or parsed.query or parsed.fragment:
            diagnostics.append(_diag("ERROR", "E_URL_PATH_INVALID", value_path, "URL path must be HTTPS without credentials, query, or fragment"))
            return
        if port is not None:
            diagnostics.append(_diag("ERROR", "E_URL_PATH_NORMALIZATION", value_path, "URL path indicators omit explicit ports, including the HTTPS default"))
        _valid_hostname(parsed.hostname, value_path, diagnostics, allow_wildcard=False)
        if parsed.hostname != parsed.hostname.lower() or parsed.path != (parsed.path or "/"):
            diagnostics.append(_diag("ERROR", "E_URL_PATH_NORMALIZATION", value_path, "URL path must use a lowercase host and explicit canonical path"))
    elif kind == "local_port":
        if not value.isascii() or not value.isdecimal() or value != str(int(value)) or not 1 <= int(value) <= 65535:
            diagnostics.append(_diag("ERROR", "E_PORT_RANGE", value_path, "port must be a canonical decimal integer from 1 through 65535"))
    elif kind == "executable_name":
        if value != value.lower() or not EXECUTABLE_PATTERN.fullmatch(value):
            diagnostics.append(_diag("ERROR", "E_EXECUTABLE_INVALID", value_path, "executable name must be a normalized basename without paths or arguments"))
        if value.lower() in GENERIC_EXECUTABLES:
            diagnostics.append(_diag("ERROR", "E_GENERIC_EXECUTABLE", value_path, "generic executable names cannot identify AI products"))
    elif kind == "package_name":
        _validate_package(indicator, path, diagnostics)
    elif kind in {"vscode_extension_id", "jetbrains_plugin_id"}:
        if value != value.lower() or not EXTENSION_ID_PATTERN.fullmatch(value):
            diagnostics.append(_diag("ERROR", "E_EXTENSION_ID_INVALID", value_path, "extension ID must be normalized lowercase publisher.name syntax"))
    elif kind == "browser_extension_id":
        chrome = re.fullmatch(r"chrome:[a-p]{32}", value)
        firefox = re.fullmatch(r"firefox:[a-z0-9._@{}-]{1,128}", value)
        if not (chrome or firefox):
            diagnostics.append(_diag("ERROR", "E_EXTENSION_ID_INVALID", value_path, "browser extension ID must use `chrome:<32 a-p characters>` or `firefox:<add-on ID>` syntax"))
    elif kind == "macos_bundle_id":
        if value != value.lower() or not re.fullmatch(r"[a-z0-9-]+(?:\.[a-z0-9-]+)+", value):
            diagnostics.append(_diag("ERROR", "E_BUNDLE_ID_INVALID", value_path, "macOS bundle ID must be lowercase reverse-DNS syntax"))
    elif kind == "linux_desktop_id":
        if not DESKTOP_ID_PATTERN.fullmatch(value):
            diagnostics.append(_diag("ERROR", "E_DESKTOP_ID_INVALID", value_path, "Linux desktop ID must be reverse-DNS style and end in `.desktop`"))
    elif kind == "windows_package_identity":
        if not WINDOWS_PACKAGE_ID_PATTERN.fullmatch(value):
            diagnostics.append(_diag("ERROR", "E_WINDOWS_PACKAGE_ID_INVALID", value_path, "Windows package identity must contain package and publisher segments"))
    elif kind == "service_name":
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.@-]{0,127}", value):
            diagnostics.append(_diag("ERROR", "E_SERVICE_NAME_INVALID", value_path, "service name must be a basename without whitespace or path separators"))
    elif kind == "container_image":
        _validate_container_image(value, value_path, diagnostics)
    elif kind == "protocol_endpoint":
        try:
            parsed = urlsplit(value)
            port = parsed.port
        except ValueError:
            diagnostics.append(_diag("ERROR", "E_PROTOCOL_ENDPOINT_INVALID", value_path, "protocol endpoint has an invalid authority or port"))
            return
        if parsed.scheme not in {"http", "https", "ws", "wss", "tcp", "udp"} or not parsed.hostname or parsed.username is not None or parsed.password is not None or parsed.fragment:
            diagnostics.append(_diag("ERROR", "E_PROTOCOL_ENDPOINT_INVALID", value_path, "protocol endpoint must use a supported scheme and have no credentials or fragment"))
        if port is not None and not 1 <= port <= 65535:
            diagnostics.append(_diag("ERROR", "E_PROTOCOL_ENDPOINT_INVALID", value_path, "protocol endpoint port must be from 1 through 65535"))


def _validate_platform_surface(indicator: dict, path: str, diagnostics: list[Diagnostic]) -> None:
    for field in ("platforms", "surfaces"):
        values = indicator[field]
        if "unknown" in values and len(values) != 1:
            diagnostics.append(_diag("ERROR", "E_UNKNOWN_SCOPE_MIXED", f"{path}.{field}", "unknown must be the only value when evidence cannot establish scope"))
    required_platform = {
        "windows_package_identity": "windows",
        "macos_bundle_id": "macos",
        "linux_desktop_id": "linux",
    }.get(indicator["kind"])
    if required_platform and required_platform not in indicator["platforms"] and "unknown" not in indicator["platforms"]:
        diagnostics.append(_diag("ERROR", "E_PLATFORM_KIND_MISMATCH", f"{path}.platforms", f"{indicator['kind']} requires platform {required_platform}"))


def semantic_diagnostics(registry: dict, today: date | None = None) -> list[Diagnostic]:
    """Return deterministic semantic diagnostics after JSON Schema validation."""
    today = today or date.today()
    diagnostics: list[Diagnostic] = []
    version = registry["registry_version"]
    if not SEMVER_PATTERN.fullmatch(version):
        diagnostics.append(_diag("ERROR", "E_SEMVER_INVALID", "$.registry_version", "registry version must use canonical semantic version X.Y.Z"))

    _date_value(registry["last_updated"], "$.last_updated", today, diagnostics)
    entries = registry["entries"]
    if len(entries) > 1000:
        diagnostics.append(_diag("ERROR", "E_COLLECTION_LIMIT", "$.entries", "registry exceeds the 1000-entry limit"))
    if [entry["id"] for entry in entries] != sorted(entry["id"] for entry in entries):
        diagnostics.append(_diag("ERROR", "E_ORDER_ENTRIES", "$.entries", "entries must be sorted by ID"))

    entry_ids: set[str] = set()
    entry_paths: dict[str, str] = {}
    all_indicators: dict[tuple[str, str], str] = {}
    total_sources = 0
    total_indicators = 0
    for entry_index, entry in enumerate(entries):
        entry_path = f"$.entries[{entry_index}]"
        entry_id = entry["id"]
        if entry_id in entry_ids:
            diagnostics.append(_diag("ERROR", "E_DUPLICATE_ENTRY_ID", f"{entry_path}.id", f"duplicate entry ID {entry_id!r}; first seen at {entry_paths[entry_id]}"))
        else:
            entry_ids.add(entry_id)
            entry_paths[entry_id] = f"{entry_path}.id"
        if entry["categories"] != sorted(entry["categories"]):
            diagnostics.append(_diag("ERROR", "E_ORDER_CATEGORIES", f"{entry_path}.categories", "categories must use canonical sorted order"))
        if entry["surfaces"] != sorted(entry["surfaces"]):
            diagnostics.append(_diag("ERROR", "E_ORDER_SURFACES", f"{entry_path}.surfaces", "product surfaces must use canonical sorted order"))

        aliases = entry["provider"].get("aliases", [])
        folded_aliases = [alias.casefold() for alias in aliases]
        if len(folded_aliases) != len(set(folded_aliases)):
            diagnostics.append(_diag("ERROR", "E_DUPLICATE_ALIAS", f"{entry_path}.provider.aliases", "provider aliases must be unique without case sensitivity"))
        if aliases != sorted(aliases, key=str.casefold):
            diagnostics.append(_diag("ERROR", "E_ORDER_ALIASES", f"{entry_path}.provider.aliases", "provider aliases must use canonical case-folded order"))

        sources = entry["sources"]
        total_sources += len(sources)
        if len(sources) > 100:
            diagnostics.append(_diag("ERROR", "E_COLLECTION_LIMIT", f"{entry_path}.sources", "entry exceeds the 100-source limit"))
        source_map: dict[str, dict] = {}
        source_order: list[int] = []
        source_urls: set[tuple[str, str]] = set()
        for source_index, source in enumerate(sources):
            source_path = f"{entry_path}.sources[{source_index}]"
            source_id = source["id"]
            if source_id in source_map:
                diagnostics.append(_diag("ERROR", "E_DUPLICATE_SOURCE_ID", f"{source_path}.id", f"duplicate entry-local source ID {source_id!r}"))
            else:
                source_map[source_id] = source
            source_order.append(int(source_id.removeprefix("source-")))
            source_date = _date_value(source["checked_on"], f"{source_path}.checked_on", today, diagnostics)
            canonical_url = _canonical_source_url(source["url"], f"{source_path}.url", diagnostics)
            if canonical_url is not None and source_date is not None:
                key = (canonical_url, source["checked_on"])
                if key in source_urls:
                    diagnostics.append(_diag("ERROR", "E_DUPLICATE_SOURCE", source_path, "same source URL and check date appear more than once in this entry"))
                source_urls.add(key)
        if source_order != sorted(source_order):
            diagnostics.append(_diag("ERROR", "E_ORDER_SOURCES", f"{entry_path}.sources", "sources must be sorted by numeric entry-local source ID"))

        indicators = entry["indicators"]
        total_indicators += len(indicators)
        if len(indicators) > 100:
            diagnostics.append(_diag("ERROR", "E_COLLECTION_LIMIT", f"{entry_path}.indicators", "entry exceeds the 100-indicator limit"))
        indicator_order = [(item["kind"], item["value"]) for item in indicators]
        if indicator_order != sorted(indicator_order):
            diagnostics.append(_diag("ERROR", "E_ORDER_INDICATORS", f"{entry_path}.indicators", "indicators must be sorted by kind and value"))
        for indicator_index, indicator in enumerate(indicators):
            indicator_path = f"{entry_path}.indicators[{indicator_index}]"
            key = (indicator["kind"], indicator["value"].casefold())
            if key in all_indicators:
                diagnostics.append(_diag("ERROR", "E_DUPLICATE_INDICATOR", f"{indicator_path}.value", f"normalized indicator duplicates {all_indicators[key]}"))
            else:
                all_indicators[key] = f"{indicator_path}.value"
            source = source_map.get(indicator["source_id"])
            if source is None:
                diagnostics.append(_diag("ERROR", "E_SOURCE_UNRESOLVED", f"{indicator_path}.source_id", "indicator source_id does not resolve within this product entry"))
            else:
                verified_date = _date_value(indicator["verified_on"], f"{indicator_path}.verified_on", today, diagnostics)
                source_date = _date_value(source["checked_on"], f"{indicator_path}.source_id", today, [])
                if verified_date is not None and source_date is not None and verified_date != source_date:
                    diagnostics.append(_diag("ERROR", "E_PROVENANCE_DATE", f"{indicator_path}.verified_on", "verified_on must match the cited source's checked_on date"))
            _validate_indicator_value(indicator, indicator_path, diagnostics)
            _validate_platform_surface(indicator, indicator_path, diagnostics)
            actionability = indicator["actionability"]
            confidence = indicator["confidence"]
            risk = indicator["false_positive_risk"]
            if actionability == "blocking_candidate":
                blocked = indicator["lifecycle"] != "current" or confidence != "high" or risk != "low" or "unknown" in indicator["platforms"] or "unknown" in indicator["surfaces"]
                if blocked:
                    diagnostics.append(_diag("ERROR", "E_BLOCKING_UNSAFE", indicator_path, "blocking candidates require current lifecycle, high evidence confidence, low false-positive risk, and known platform/surface scope"))
            if actionability == "unsafe_to_block" and risk != "high":
                diagnostics.append(_diag("ERROR", "E_FALSE_POSITIVE_ACTIONABILITY", indicator_path, "unsafe_to_block indicators must record high false-positive risk"))
            if confidence == "low" and actionability in {"advisory", "detection_candidate", "blocking_candidate"}:
                diagnostics.append(_diag("WARNING", "W_LOW_CONFIDENCE_ACTIONABILITY", indicator_path, "low-confidence indicators should remain research_only until reviewed"))
            if actionability == "detection_candidate" and risk in {"high", "unknown"}:
                diagnostics.append(_diag("WARNING", "W_DETECTION_FALSE_POSITIVE_RISK", indicator_path, "detection candidate has high or unknown false-positive risk"))
            if entry["status"] == "archived" and indicator["lifecycle"] == "current":
                diagnostics.append(_diag("WARNING", "W_ARCHIVED_PRODUCT_INDICATOR", indicator_path, "archived product has an indicator marked current"))

        relationships = entry["relationships"]
        if len(relationships) > 100:
            diagnostics.append(_diag("ERROR", "E_COLLECTION_LIMIT", f"{entry_path}.relationships", "entry exceeds the 100-relationship limit"))
        relationship_order = [(item["kind"], item["target_id"]) for item in relationships]
        if relationship_order != sorted(relationship_order):
            diagnostics.append(_diag("ERROR", "E_ORDER_RELATIONSHIPS", f"{entry_path}.relationships", "relationships must be sorted by kind and target ID"))
        seen_relationships: set[tuple[str, str]] = set()
        for relationship_index, relationship in enumerate(relationships):
            relationship_path = f"{entry_path}.relationships[{relationship_index}]"
            relationship_key = (relationship["kind"], relationship["target_id"])
            if relationship["target_id"] == entry_id:
                diagnostics.append(_diag("ERROR", "E_RELATIONSHIP_SELF", f"{relationship_path}.target_id", "self-relationships are not allowed"))
            if relationship_key in seen_relationships:
                diagnostics.append(_diag("ERROR", "E_DUPLICATE_RELATIONSHIP", relationship_path, "duplicate relationship in the same entry"))
            seen_relationships.add(relationship_key)

    if total_sources > 20_000:
        diagnostics.append(_diag("ERROR", "E_COLLECTION_LIMIT", "$.entries", "registry exceeds the 20000-source limit"))
    if total_indicators > 20_000:
        diagnostics.append(_diag("ERROR", "E_COLLECTION_LIMIT", "$.entries", "registry exceeds the 20000-indicator limit"))
    for entry_index, entry in enumerate(entries):
        for relationship_index, relationship in enumerate(entry["relationships"]):
            if relationship["target_id"] not in entry_ids:
                diagnostics.append(_diag("ERROR", "E_RELATIONSHIP_TARGET", f"$.entries[{entry_index}].relationships[{relationship_index}].target_id", f"relationship target {relationship['target_id']!r} does not resolve"))

    diagnostics.append(_diag("INFO", "I_RESEARCH_ONLY", "$", "registry values are research evidence and do not authorize enforcement"))
    severity_order = {"ERROR": 0, "WARNING": 1, "INFO": 2}
    return sorted(diagnostics, key=lambda item: (severity_order[item.severity], item.path, item.code, item.message))
