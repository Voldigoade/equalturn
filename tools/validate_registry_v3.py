#!/usr/bin/env python3
"""Validate the researched registry v3 schema and semantic safety rules."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError

from registry_semantics import Diagnostic, semantic_diagnostics


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REGISTRY = ROOT / "registry" / "registry-v3.json"
DEFAULT_SCHEMA = ROOT / "registry" / "schema-v3.schema.json"
MAX_REGISTRY_BYTES = 4 * 1024 * 1024
MAX_SCHEMA_BYTES = 1024 * 1024


class InputError(ValueError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


class DuplicateJsonKeyError(ValueError):
    pass


@dataclass
class ValidationResult:
    entry_count: int = 0
    indicator_count: int = 0
    diagnostics: list[Diagnostic] = field(default_factory=list)

    @property
    def errors(self) -> list[Diagnostic]:
        return [item for item in self.diagnostics if item.severity == "ERROR"]

    @property
    def warnings(self) -> list[Diagnostic]:
        return [item for item in self.diagnostics if item.severity == "WARNING"]


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateJsonKeyError(f"duplicate JSON object key {key!r}")
        result[key] = value
    return result


def _read_json(path: Path, max_bytes: int) -> object:
    try:
        with path.open("rb") as source:
            contents = source.read(max_bytes + 1)
    except OSError as error:
        raise InputError("E_FILE_READ", f"cannot read {path}: {error.strerror or error}") from error
    if len(contents) > max_bytes:
        raise InputError("E_FILE_TOO_LARGE", f"{path} exceeds the {max_bytes}-byte limit")
    try:
        text = contents.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise InputError("E_UTF8_INVALID", f"{path} is not valid UTF-8 at byte {error.start}") from error
    try:
        return json.loads(text, object_pairs_hook=_unique_object)
    except DuplicateJsonKeyError as error:
        raise InputError("E_JSON_DUPLICATE_KEY", f"{path}: {error}") from error
    except (json.JSONDecodeError, RecursionError) as error:
        raise InputError("E_JSON_INVALID", f"{path} is malformed JSON") from error


def _format_path(parts: Any) -> str:
    path = "$"
    for part in parts:
        path += f"[{part}]" if isinstance(part, int) else f".{part}"
    return path


def _schema_diagnostics(registry: object, schema: object) -> list[Diagnostic]:
    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        return [Diagnostic("ERROR", "E_SCHEMA_DEFINITION", "$", error.message)]
    validator = Draft202012Validator(schema, format_checker=Draft202012Validator.FORMAT_CHECKER)
    errors = sorted(
        validator.iter_errors(registry),
        key=lambda error: (tuple(map(str, error.absolute_path)), error.message),
    )
    return [
        Diagnostic("ERROR", "E_SCHEMA", _format_path(error.absolute_path), error.message)
        for error in errors
    ]


def validate(
    registry_path: Path = DEFAULT_REGISTRY,
    schema_path: Path = DEFAULT_SCHEMA,
) -> ValidationResult:
    result = ValidationResult()
    try:
        registry = _read_json(registry_path, MAX_REGISTRY_BYTES)
        schema = _read_json(schema_path, MAX_SCHEMA_BYTES)
    except InputError as error:
        result.diagnostics.append(Diagnostic("ERROR", error.code, "$", str(error)))
        return result

    schema_errors = _schema_diagnostics(registry, schema)
    if schema_errors:
        result.diagnostics.extend(schema_errors)
        result.diagnostics.sort(key=lambda item: (item.path, item.code, item.message))
        return result

    if not isinstance(registry, dict) or not isinstance(registry.get("entries"), list):
        result.diagnostics.append(Diagnostic("ERROR", "E_SCHEMA", "$", "registry root must be an object with an entries array"))
        return result
    result.entry_count = len(registry["entries"])
    result.indicator_count = sum(len(entry["indicators"]) for entry in registry["entries"])
    result.diagnostics.extend(semantic_diagnostics(registry))
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry", type=Path, default=DEFAULT_REGISTRY)
    parser.add_argument("--schema", type=Path, default=DEFAULT_SCHEMA)
    args = parser.parse_args(argv)
    result = validate(args.registry, args.schema)
    for diagnostic in result.diagnostics:
        stream = sys.stderr if diagnostic.severity == "ERROR" else sys.stdout
        print(diagnostic.render(), file=stream)
    error_count = len(result.errors)
    warning_count = len(result.warnings)
    if error_count:
        print(
            f"INVALID: {error_count} error(s), {warning_count} warning(s); "
            f"{result.entry_count} entries, {result.indicator_count} indicators.",
            file=sys.stderr,
        )
        return 1
    label = "VALID WITH WARNINGS" if warning_count else "VALID"
    print(
        f"{label}: {result.entry_count} entries, {result.indicator_count} indicators; "
        f"{warning_count} warning(s)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
