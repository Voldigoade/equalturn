#!/usr/bin/env python3
"""Validate and emit the deterministic registry-v3 TUF target."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path

from validate_registry_v3 import (
    DEFAULT_REGISTRY,
    DEFAULT_SCHEMA,
    MAX_REGISTRY_BYTES,
    MAX_SCHEMA_BYTES,
    InputError,
    ValidationResult,
    parse_json_bytes,
    read_json_bounded,
    validate_data,
)


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "dist" / "registry-v3.json"
MAX_TARGET_BYTES = 4 * 1024 * 1024
GIT_OBJECT_ID = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})\Z")


class RegistryTargetError(ValueError):
    """The source registry cannot be emitted as a valid bounded target."""


def _normalize_text(value: object, path: str = "$") -> object:
    if isinstance(value, str):
        return unicodedata.normalize("NFC", value)
    if isinstance(value, list):
        return [_normalize_text(item, f"{path}[{index}]") for index, item in enumerate(value)]
    if isinstance(value, dict):
        normalized: dict[str, object] = {}
        for key, item in value.items():
            if not isinstance(key, str):
                raise RegistryTargetError(f"object key at {path} is not text")
            normalized_key = unicodedata.normalize("NFC", key)
            if normalized_key in normalized:
                raise RegistryTargetError(f"duplicate keys after NFC normalization at {path}")
            normalized[normalized_key] = _normalize_text(item, f"{path}.{normalized_key}")
        return normalized
    return value


def _validated_target(registry: object, schema: object) -> tuple[bytes, ValidationResult]:
    normalized = _normalize_text(registry)
    result = validate_data(normalized, schema)
    if result.errors:
        details = "\n".join(item.render() for item in result.errors)
        raise RegistryTargetError(details)
    try:
        serialized = json.dumps(
            normalized,
            ensure_ascii=False,
            allow_nan=False,
            indent=2,
            sort_keys=True,
        )
    except (TypeError, ValueError) as error:
        raise RegistryTargetError("registry cannot be serialized as deterministic JSON") from error
    target = (serialized + "\n").encode("utf-8", errors="strict")
    if len(target) > MAX_TARGET_BYTES:
        raise RegistryTargetError(f"target exceeds the {MAX_TARGET_BYTES}-byte limit")
    return target, result


def render_target(registry: object, schema: object) -> bytes:
    """Return canonical UTF-8 target bytes for a schema-valid registry value."""
    return _validated_target(registry, schema)[0]


def _write_atomically(output: Path, payload: bytes) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=output.parent, prefix=f".{output.name}.", delete=False) as temporary:
            temporary_path = Path(temporary.name)
            temporary.write(payload)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.chmod(temporary_path, 0o644)
        os.replace(temporary_path, output)
    except OSError:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)
        raise


def _read_bounded(path: Path) -> bytes:
    with path.open("rb") as source:
        contents = source.read(MAX_TARGET_BYTES + 1)
    if len(contents) > MAX_TARGET_BYTES:
        raise RegistryTargetError(f"existing target exceeds the {MAX_TARGET_BYTES}-byte limit")
    return contents


def _git_blob(repository_root: Path, commit: str, path: str, maximum: int) -> bytes:
    if GIT_OBJECT_ID.fullmatch(commit) is None:
        raise RegistryTargetError("source commit must be a full Git object ID")

    def git(*arguments: str) -> bytes:
        try:
            result = subprocess.run(
                ["git", "--no-replace-objects", "-C", str(repository_root), *arguments],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
            )
        except OSError as error:
            raise RegistryTargetError("Git is unavailable for source-commit verification") from error
        if result.returncode != 0:
            raise RegistryTargetError("source commit or registry file is unavailable in Git")
        return result.stdout

    resolved_commit = git("rev-parse", "--verify", "--end-of-options", f"{commit}^{{commit}}")
    if resolved_commit.decode("ascii", errors="replace").strip() != commit:
        raise RegistryTargetError("source commit must be a full Git commit ID")

    blob_id = git("rev-parse", "--verify", "--end-of-options", f"{commit}:{path}")
    blob_id_text = blob_id.decode("ascii", errors="replace").strip()
    if GIT_OBJECT_ID.fullmatch(blob_id_text) is None:
        raise RegistryTargetError("source registry file is not a Git blob")
    if git("cat-file", "-t", blob_id_text).strip() != b"blob":
        raise RegistryTargetError("source registry file is not a Git blob")
    size_bytes = git("cat-file", "-s", blob_id_text)
    try:
        size = int(size_bytes.strip())
    except ValueError as error:
        raise RegistryTargetError("source registry file size is invalid") from error
    if size < 0 or size > maximum:
        raise RegistryTargetError(f"source registry file exceeds the {maximum}-byte limit")

    contents = git("cat-file", "blob", blob_id_text)
    if len(contents) != size:
        raise RegistryTargetError("source registry file was truncated while reading Git object")
    return contents


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry", type=Path)
    parser.add_argument("--schema", type=Path)
    parser.add_argument(
        "--source-commit",
        help="rebuild from registry and schema files at an exact local Git commit ID",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the generated bytes with an existing target without writing it",
    )
    args = parser.parse_args(argv)

    registry_path = args.registry or DEFAULT_REGISTRY
    schema_path = args.schema or DEFAULT_SCHEMA
    protected_source_paths = {
        registry_path.resolve(),
        schema_path.resolve(),
        DEFAULT_REGISTRY.resolve(),
        DEFAULT_SCHEMA.resolve(),
    }
    if args.source_commit is not None and (args.registry is not None or args.schema is not None):
        print("ERROR: --source-commit selects both registry and schema from Git", file=sys.stderr)
        return 1
    if not args.check and args.output.resolve() in protected_source_paths:
        print("ERROR: output path must not replace a registry source or schema", file=sys.stderr)
        return 1

    try:
        if args.source_commit is not None:
            registry = parse_json_bytes(
                _git_blob(ROOT, args.source_commit, "registry/registry-v3.json", MAX_REGISTRY_BYTES),
                f"{args.source_commit}:registry/registry-v3.json",
                MAX_REGISTRY_BYTES,
            )
            schema = parse_json_bytes(
                _git_blob(ROOT, args.source_commit, "registry/schema-v3.schema.json", MAX_SCHEMA_BYTES),
                f"{args.source_commit}:registry/schema-v3.schema.json",
                MAX_SCHEMA_BYTES,
            )
        else:
            registry = read_json_bounded(registry_path, MAX_REGISTRY_BYTES)
            schema = read_json_bounded(schema_path, MAX_SCHEMA_BYTES)
        target, result = _validated_target(registry, schema)
    except (InputError, RegistryTargetError, OSError) as error:
        print(f"INVALID: {error}", file=sys.stderr)
        return 1

    for diagnostic in result.diagnostics:
        stream = sys.stderr if diagnostic.severity == "ERROR" else sys.stdout
        print(diagnostic.render(), file=stream)

    if args.check:
        try:
            matches = _read_bounded(args.output) == target
        except (OSError, RegistryTargetError) as error:
            print(f"MISMATCH: {error}", file=sys.stderr)
            return 1
        if not matches:
            print(f"MISMATCH: {args.output} differs from the deterministic registry target", file=sys.stderr)
            return 1
        source = (
            f" from source commit {args.source_commit}"
            if args.source_commit is not None
            else ""
        )
        print(f"REPRODUCIBLE: {args.output}{source}")
        return 0

    try:
        _write_atomically(args.output, target)
    except OSError as error:
        print(f"ERROR: cannot write {args.output}: {error.strerror or error}", file=sys.stderr)
        return 1
    print(f"WROTE: {args.output} ({len(target)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
