#!/usr/bin/env python3
"""Validate the research registry structure and its provenance references."""

from __future__ import annotations

import json
import re
import sys
from datetime import date
from pathlib import Path
from urllib.parse import urlsplit


ROOT = Path(__file__).resolve().parents[1]
REGISTRY_PATH = ROOT / "registry" / "registry-v2.json"
SCHEMA_PATH = ROOT / "registry" / "schema-v2.schema.json"
ENTRY_ID = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
SOURCE_ID = re.compile(r"^source-[1-9][0-9]*$")
SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def exact_keys(value: object, required: set[str], optional: set[str], path: str) -> dict:
    require(isinstance(value, dict), f"{path} must be an object")
    keys = set(value)
    require(required <= keys, f"{path} is missing keys: {sorted(required - keys)}")
    require(keys <= required | optional, f"{path} has unknown keys: {sorted(keys - required - optional)}")
    return value


def check_date(value: object, path: str) -> None:
    require(isinstance(value, str), f"{path} must be a date string")
    try:
        date.fromisoformat(value)
    except (KeyError, TypeError, ValueError) as error:
        raise ValueError(f"{path} must be an ISO date") from error


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"could not read {path.relative_to(ROOT)}") from error


def validate() -> tuple[int, int, int]:
    registry = load_json(REGISTRY_PATH)
    schema = load_json(SCHEMA_PATH)
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", "schema must declare JSON Schema 2020-12")
    root = exact_keys(
        registry,
        {"schema_version", "registry_version", "last_updated", "purpose", "entries"},
        set(),
        "registry",
    )
    require(root["schema_version"] == 2, "registry schema_version must be 2")
    require(isinstance(root["registry_version"], str) and SEMVER.fullmatch(root["registry_version"]), "registry_version must be semantic version X.Y.Z")
    check_date(root["last_updated"], "registry.last_updated")
    require(root["purpose"] == "research_only", "registry purpose must remain research_only")
    require(isinstance(root["entries"], list) and root["entries"], "registry.entries must be a nonempty array")

    entry_schema = schema["$defs"]["entry"]["properties"]
    categories = set(entry_schema["categories"]["items"]["enum"])
    surfaces = set(entry_schema["surfaces"]["items"]["enum"])
    statuses = set(entry_schema["status"]["enum"])
    indicator_kinds = set(schema["$defs"]["indicator"]["properties"]["kind"]["enum"])
    relationship_kinds = set(schema["$defs"]["relationship"]["properties"]["kind"]["enum"])
    entry_ids: set[str] = set()
    source_count = 0
    indicator_count = 0

    for index, entry_value in enumerate(root["entries"]):
        path = f"registry.entries[{index}]"
        entry = exact_keys(
            entry_value,
            {"id", "provider", "product", "categories", "surfaces", "status", "sources", "indicators", "relationships"},
            set(),
            path,
        )
        entry_id = entry["id"]
        require(isinstance(entry_id, str) and ENTRY_ID.fullmatch(entry_id), f"{path}.id is malformed")
        require(entry_id not in entry_ids, f"{path}.id is duplicated: {entry_id}")
        entry_ids.add(entry_id)
        require(isinstance(entry["product"], str) and entry["product"].strip(), f"{path}.product is empty")
        require(isinstance(entry["categories"], list) and entry["categories"], f"{path}.categories must be nonempty")
        require(len(entry["categories"]) == len(set(entry["categories"])), f"{path}.categories contains duplicates")
        require(set(entry["categories"]) <= categories, f"{path}.categories contains unsupported values")
        require(isinstance(entry["surfaces"], list) and entry["surfaces"], f"{path}.surfaces must be nonempty")
        require(len(entry["surfaces"]) == len(set(entry["surfaces"])), f"{path}.surfaces contains duplicates")
        require(set(entry["surfaces"]) <= surfaces, f"{path}.surfaces contains unsupported values")
        require(entry["status"] in statuses, f"{path}.status is unsupported")

        provider = exact_keys(entry["provider"], {"id", "name"}, {"aliases"}, f"{path}.provider")
        require(isinstance(provider["id"], str) and ENTRY_ID.fullmatch(provider["id"]), f"{path}.provider.id is malformed")
        require(isinstance(provider["name"], str) and provider["name"].strip(), f"{path}.provider.name is empty")
        aliases = provider.get("aliases", [])
        require(isinstance(aliases, list) and all(isinstance(alias, str) and alias.strip() for alias in aliases), f"{path}.provider.aliases is malformed")
        require(len(aliases) == len(set(aliases)), f"{path}.provider.aliases contains duplicates")

        sources = entry["sources"]
        require(isinstance(sources, list) and sources, f"{path}.sources must be nonempty")
        source_ids: set[str] = set()
        for source_index, source_value in enumerate(sources):
            source_path = f"{path}.sources[{source_index}]"
            source = exact_keys(source_value, {"id", "url", "checked_on", "indicator_assessment"}, set(), source_path)
            source_id = source["id"]
            require(isinstance(source_id, str) and SOURCE_ID.fullmatch(source_id), f"{source_path}.id is malformed")
            require(source_id not in source_ids, f"{source_path}.id is duplicated within its entry")
            source_ids.add(source_id)
            parsed_url = urlsplit(source["url"] if isinstance(source["url"], str) else "")
            require(parsed_url.scheme in {"http", "https"} and bool(parsed_url.netloc), f"{source_path}.url must be an absolute HTTP(S) URL")
            check_date(source["checked_on"], f"{source_path}.checked_on")
            require(isinstance(source["indicator_assessment"], str) and source["indicator_assessment"].strip(), f"{source_path}.indicator_assessment is empty")
            source_count += 1

        indicators = entry["indicators"]
        require(isinstance(indicators, list), f"{path}.indicators must be an array")
        for indicator_index, indicator_value in enumerate(indicators):
            indicator_path = f"{path}.indicators[{indicator_index}]"
            indicator = exact_keys(indicator_value, {"kind", "value", "scope", "source_id", "limitations"}, set(), indicator_path)
            require(indicator["kind"] in indicator_kinds, f"{indicator_path}.kind is unsupported")
            require(isinstance(indicator["value"], str) and indicator["value"].strip(), f"{indicator_path}.value is empty")
            require(isinstance(indicator["scope"], str) and indicator["scope"].strip(), f"{indicator_path}.scope is empty")
            require(indicator["source_id"] in source_ids, f"{indicator_path}.source_id does not identify a source in this entry")
            require(isinstance(indicator["limitations"], str) and indicator["limitations"].strip(), f"{indicator_path}.limitations is empty")
            indicator_count += 1

        relationships = entry["relationships"]
        require(isinstance(relationships, list), f"{path}.relationships must be an array")
        for relationship_index, relationship_value in enumerate(relationships):
            relationship_path = f"{path}.relationships[{relationship_index}]"
            relationship = exact_keys(relationship_value, {"kind", "target_id", "notes"}, set(), relationship_path)
            require(relationship["kind"] in relationship_kinds, f"{relationship_path}.kind is unsupported")
            require(isinstance(relationship["target_id"], str) and ENTRY_ID.fullmatch(relationship["target_id"]), f"{relationship_path}.target_id is malformed")
            require(isinstance(relationship["notes"], str), f"{relationship_path}.notes must be a string")

    for index, entry in enumerate(root["entries"]):
        for relationship in entry["relationships"]:
            require(relationship["target_id"] in entry_ids, f"registry.entries[{index}] has an unresolved relationship target")

    return len(entry_ids), source_count, indicator_count


def main() -> int:
    try:
        entries, sources, indicators = validate()
    except ValueError as error:
        print(f"registry validation failed: {error}", file=sys.stderr)
        return 1
    print(f"Registry valid: {entries} entries, {sources} provenance records, {indicators} structured indicators.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
