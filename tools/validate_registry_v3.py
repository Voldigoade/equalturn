#!/usr/bin/env python3
"""Apply the registry v3 Draft 2020-12 schema to the checked-in snapshot."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REGISTRY = ROOT / "registry" / "registry-v3.json"
DEFAULT_SCHEMA = ROOT / "registry" / "schema-v3.schema.json"
MAX_REGISTRY_BYTES = 4 * 1024 * 1024


def _read_json(path: Path) -> object:
    if path.stat().st_size > MAX_REGISTRY_BYTES:
        raise ValueError(f"{path} exceeds the {MAX_REGISTRY_BYTES}-byte limit")
    return json.loads(path.read_text(encoding="utf-8"))


def _format_path(error: object) -> str:
    path = "$"
    for part in error.absolute_path:
        path += f"[{part}]" if isinstance(part, int) else f".{part}"
    return path


def validate(registry_path: Path = DEFAULT_REGISTRY, schema_path: Path = DEFAULT_SCHEMA) -> tuple[int, int]:
    registry = _read_json(registry_path)
    schema = _read_json(schema_path)
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema, format_checker=Draft202012Validator.FORMAT_CHECKER)
    errors = sorted(
        validator.iter_errors(registry),
        key=lambda error: (tuple(map(str, error.absolute_path)), error.message),
    )
    if errors:
        details = "\n".join(f"{_format_path(error)}: {error.message}" for error in errors[:50])
        if len(errors) > 50:
            details += f"\n... {len(errors) - 50} additional schema errors omitted"
        raise ValueError(details)
    if not isinstance(registry, dict) or not isinstance(registry.get("entries"), list):
        raise ValueError("registry root and entries must be objects and arrays")
    indicators = sum(len(entry["indicators"]) for entry in registry["entries"])
    return len(registry["entries"]), indicators


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry", type=Path, default=DEFAULT_REGISTRY)
    parser.add_argument("--schema", type=Path, default=DEFAULT_SCHEMA)
    args = parser.parse_args()
    try:
        entries, indicators = validate(args.registry, args.schema)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, SchemaError, ValueError) as error:
        print(f"registry v3 schema validation failed: {error}", file=sys.stderr)
        return 1
    print(f"Registry v3 schema valid: {entries} entries, {indicators} structured indicators.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
