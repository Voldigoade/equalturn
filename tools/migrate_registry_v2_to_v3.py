#!/usr/bin/env python3
"""Migrate the research-only registry v2 snapshot to the stricter v3 model."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from urllib.parse import urlsplit, urlunsplit


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INPUT = ROOT / "registry" / "registry-v2.json"
DEFAULT_OUTPUT = ROOT / "registry" / "registry-v3.json"
SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
KIND_MAP = {
    "domain_exact": "web_hostname",
    "api_host": "api_hostname",
    "url_path": "url_path",
    "process_name": "executable_name",
    "package_name": "package_name",
    "tcp_listener": "local_port",
}
SURFACE_MAP = {
    "web_hostname": ["web"],
    "api_hostname": ["api"],
    "url_path": ["web"],
    "local_port": ["local_server"],
}
LOCAL_SURFACES = {"browser_extension", "cli", "desktop", "ide", "local_runtime", "local_server", "python_library", "server"}


class MigrationError(ValueError):
    """Raised when a source registry cannot be migrated without guessing."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise MigrationError(message)


def _object(value: object, required: set[str], optional: set[str], path: str) -> dict:
    _require(isinstance(value, dict), f"{path} must be an object")
    missing = required - value.keys()
    extra = value.keys() - required - optional
    _require(not missing, f"{path} is missing keys: {sorted(missing)}")
    _require(not extra, f"{path} has unknown keys: {sorted(extra)}")
    return value


def _normalize_value(kind: str, value: object, entry_id: str) -> tuple[str, str | None]:
    _require(isinstance(value, str) and value.strip(), f"{entry_id}: indicator value must be a nonempty string")
    raw = value.strip()
    if kind in {"web_hostname", "api_hostname"}:
        _require("/" not in raw and "://" not in raw and ":" not in raw, f"{entry_id}: hostname indicator is not a hostname")
        return raw.rstrip(".").lower(), None
    if kind == "url_path":
        parsed = urlsplit(raw)
        _require(parsed.scheme == "https" and parsed.hostname, f"{entry_id}: v2 URL path must be an absolute HTTPS URL")
        _require(not parsed.query and not parsed.fragment, f"{entry_id}: query and fragment cannot be represented as a path indicator")
        _require(parsed.username is None and parsed.password is None, f"{entry_id}: URL credentials cannot be represented as an indicator")
        try:
            port = parsed.port
        except ValueError as error:
            raise MigrationError(f"{entry_id}: URL port is malformed") from error
        _require(port in {None, 443}, f"{entry_id}: non-default HTTPS ports cannot be represented as a product path")
        hostname = parsed.hostname.lower()
        authority = f"[{hostname}]" if ":" in hostname else hostname
        normalized = urlunsplit(("https", authority, parsed.path or "/", "", ""))
        return normalized, None
    if kind == "local_port":
        host, separator, port_text = raw.rpartition(":")
        _require(bool(separator) and port_text.isascii() and port_text.isdecimal(), f"{entry_id}: listener must include a decimal port")
        port = int(port_text)
        _require(1 <= port <= 65535, f"{entry_id}: listener port is out of range")
        _require(host.lower() in {"localhost", "127.0.0.1", "[::1]", "::1"}, f"{entry_id}: listener is not a recognized loopback address")
        return str(port), "loopback"
    return raw, None


def migrate_registry(source: object) -> dict:
    """Return a deterministic v3 object without inventing enforcement policy."""
    source = _object(
        source,
        {"schema_version", "registry_version", "last_updated", "purpose", "entries"},
        set(),
        "registry",
    )
    _require(source.get("schema_version") == 2, "input schema_version must be 2")
    _require(isinstance(source.get("registry_version"), str) and SEMVER.fullmatch(source["registry_version"]), "input registry_version must be semantic version X.Y.Z")
    _require(source.get("purpose") == "research_only", "input purpose must be research_only")
    _require(isinstance(source.get("last_updated"), str), "input last_updated must be a date string")
    _require(isinstance(source.get("entries"), list) and source["entries"], "input entries must be a nonempty array")

    migrated_entries = []
    for index, entry_value in enumerate(source["entries"]):
        entry = _object(
            entry_value,
            {"id", "provider", "product", "categories", "surfaces", "status", "sources", "indicators", "relationships"},
            set(),
            f"entries[{index}]",
        )
        entry_id = entry.get("id")
        _require(isinstance(entry_id, str) and entry_id, "entry id must be a nonempty string")
        provider = _object(entry["provider"], {"id", "name"}, {"aliases"}, f"{entry_id}.provider")
        _require(isinstance(entry["categories"], list) and all(isinstance(item, str) for item in entry["categories"]), f"{entry_id}: categories must be strings")
        _require(isinstance(entry["surfaces"], list) and all(isinstance(item, str) for item in entry["surfaces"]), f"{entry_id}: surfaces must be strings")
        _require(isinstance(entry["sources"], list), f"{entry_id}: sources must be an array")
        _require(isinstance(entry["indicators"], list), f"{entry_id}: indicators must be an array")
        _require(isinstance(entry["relationships"], list), f"{entry_id}: relationships must be an array")
        sources = [
            _object(item, {"id", "url", "checked_on", "indicator_assessment"}, set(), f"{entry_id}.sources[{source_index}]")
            for source_index, item in enumerate(entry["sources"])
        ]
        source_map = {item["id"]: item for item in sources}
        _require(len(source_map) == len(sources), f"{entry_id}: source IDs must be unique")
        migrated_indicators = []
        for indicator_index, indicator_value in enumerate(entry["indicators"]):
            indicator = _object(
                indicator_value,
                {"kind", "value", "scope", "source_id", "limitations"},
                set(),
                f"{entry_id}.indicators[{indicator_index}]",
            )
            old_kind = indicator.get("kind")
            _require(old_kind in KIND_MAP, f"{entry_id}: unsupported v2 indicator kind {old_kind!r}")
            kind = KIND_MAP[old_kind]
            source_id = indicator.get("source_id")
            provenance = source_map.get(source_id)
            _require(isinstance(provenance, dict), f"{entry_id}: indicator source_id does not resolve")
            verified_on = provenance.get("checked_on")
            _require(isinstance(verified_on, str), f"{entry_id}: source checked_on must be a date string")
            value, bind_scope = _normalize_value(kind, indicator.get("value"), entry_id)
            migrated = {
                "kind": kind,
                "value": value,
                "platforms": ["unknown"],
                "surfaces": SURFACE_MAP.get(kind, ["unknown"]),
                "scope": indicator["scope"],
                "source_id": source_id,
                "verified_on": verified_on,
                "confidence": "low",
                "actionability": "research_only",
                "false_positive_risk": "unknown",
                "lifecycle": "unknown",
                "limitations": indicator["limitations"],
            }
            if kind == "local_port":
                migrated["bind_scope"] = bind_scope
            if kind == "package_name":
                migrated["ecosystem"] = "npm" if value.startswith("@") else "unknown"
            if kind in {"executable_name", "package_name"}:
                local_surfaces = sorted(set(entry["surfaces"]) & LOCAL_SURFACES)
                migrated["surfaces"] = local_surfaces if len(local_surfaces) == 1 else ["unknown"]
            migrated_indicators.append(migrated)

        migrated_entry = {
            "id": entry["id"],
            "provider": provider,
            "product": entry["product"],
            "categories": sorted(entry["categories"]),
            "surfaces": sorted(entry["surfaces"]),
            "status": entry["status"],
            "sources": sorted(sources, key=lambda item: int(item["id"].removeprefix("source-"))),
            "indicators": sorted(migrated_indicators, key=lambda item: (item["kind"], item["value"])),
            "relationships": sorted(entry["relationships"], key=lambda item: (item["kind"], item["target_id"])),
        }
        if "aliases" in provider:
            _require(isinstance(provider["aliases"], list) and all(isinstance(alias, str) for alias in provider["aliases"]), f"{entry_id}: provider aliases must be strings")
            migrated_entry["provider"] = {**provider, "aliases": sorted(provider["aliases"], key=str.casefold)}
        migrated_entries.append(migrated_entry)

    migrated_entries.sort(key=lambda item: item["id"])
    return {
        "schema_version": 3,
        "registry_version": "0.3.0",
        "last_updated": source["last_updated"],
        "purpose": "research_only",
        "entries": migrated_entries,
    }


def serialize_registry(registry: dict) -> bytes:
    return (json.dumps(registry, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true", help="compare generated bytes with an existing output")
    parser.add_argument("--force", action="store_true", help="replace an existing output file")
    args = parser.parse_args(argv)
    if args.check and args.force:
        parser.error("--check and --force cannot be combined")
    try:
        source = json.loads(args.input.read_text(encoding="utf-8"))
        migrated = migrate_registry(source)
        generated = serialize_registry(migrated)
        if args.check:
            _require(args.output.is_file(), f"output does not exist: {args.output}")
            _require(args.output.read_bytes() == generated, f"output is not reproducible: {args.output}")
        else:
            _require(args.force or not args.output.exists(), f"refusing to overwrite {args.output}; pass --force")
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(generated)
    except (OSError, json.JSONDecodeError, MigrationError, KeyError, TypeError, ValueError) as error:
        print(f"registry migration failed: {error}", file=sys.stderr)
        return 1
    print(f"Migrated {len(migrated['entries'])} entries to schema v3 at {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
