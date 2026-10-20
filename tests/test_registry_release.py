import copy
import contextlib
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))

import build_registry_release  # noqa: E402
from build_registry_release import RegistryTargetError, main, render_target  # noqa: E402


FIXTURES = ROOT / "tests" / "fixtures" / "registry-v3"
SCHEMA = json.loads((ROOT / "registry" / "schema-v3.schema.json").read_text(encoding="utf-8"))
SCHEMA_PATH = ROOT / "registry" / "schema-v3.schema.json"
VALID = json.loads((FIXTURES / "valid.json").read_text(encoding="utf-8"))


class RegistryReleaseTargetTests(unittest.TestCase):
    def test_target_is_deterministic_canonical_json_and_nfc_normalized(self):
        registry = copy.deepcopy(VALID)
        registry["entries"][0]["product"] = "Cafe\u0301 Assistant"

        first = render_target(registry, SCHEMA)
        second = render_target(registry, SCHEMA)

        self.assertEqual(first, second)
        self.assertTrue(first.endswith(b"\n"))
        self.assertIn("Caf\u00e9 Assistant".encode("utf-8"), first)
        self.assertNotIn("Cafe\u0301 Assistant".encode("utf-8"), first)
        decoded = json.loads(first)
        self.assertEqual(list(decoded), sorted(decoded))
        self.assertEqual(decoded["schema_version"], 3)
        self.assertEqual(decoded["registry_version"], VALID["registry_version"])

    def test_target_rejects_invalid_registry_semantics(self):
        registry = copy.deepcopy(VALID)
        registry["entries"][0]["indicators"][0]["value"] = "https://example.com/path"

        with self.assertRaisesRegex(RegistryTargetError, "E_HOSTNAME_INVALID"):
            render_target(registry, SCHEMA)

    def test_normalized_duplicate_object_keys_are_rejected(self):
        registry = copy.deepcopy(VALID)
        registry["é"] = "first"
        registry["e\u0301"] = "second"

        with self.assertRaisesRegex(RegistryTargetError, "duplicate keys after NFC normalization"):
            render_target(registry, SCHEMA)

    def test_output_size_is_bounded(self):
        with mock.patch("build_registry_release.MAX_TARGET_BYTES", 16):
            with self.assertRaisesRegex(RegistryTargetError, "target exceeds"):
                render_target(VALID, SCHEMA)

    def test_cli_writes_target_and_check_detects_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "registry-v3.json"
            arguments = [
                "--registry", str(FIXTURES / "valid.json"),
                "--schema", str(SCHEMA_PATH),
                "--output", str(output),
            ]
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(main(arguments), 0)
                self.assertEqual(main([*arguments, "--check"]), 0)

            original = output.read_bytes()
            output.write_bytes(original + b" ")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(main([*arguments, "--check"]), 1)

    def test_cli_refuses_to_replace_its_source(self):
        source = FIXTURES / "valid.json"
        original = source.read_bytes()
        with contextlib.redirect_stderr(io.StringIO()):
            result = main([
                "--registry", str(source),
                "--schema", str(SCHEMA_PATH),
                "--output", str(source),
            ])
        self.assertEqual(result, 1)
        self.assertEqual(source.read_bytes(), original)

    def test_cli_verifies_target_against_an_exact_git_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            registry_path = repository / "registry" / "registry-v3.json"
            schema_path = repository / "registry" / "schema-v3.schema.json"
            registry_path.parent.mkdir(parents=True)
            registry_path.write_text(json.dumps(VALID), encoding="utf-8")
            schema_path.write_bytes(SCHEMA_PATH.read_bytes())
            subprocess.run(["git", "init", "-q", str(repository)], check=True)
            subprocess.run(["git", "-C", str(repository), "add", "registry"], check=True)
            subprocess.run(
                [
                    "git", "-C", str(repository),
                    "-c", "user.name=Equalturn test",
                    "-c", "user.email=test@example.invalid",
                    "commit", "-qm", "fixture",
                ],
                check=True,
            )
            source_commit = subprocess.run(
                ["git", "-C", str(repository), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            target = repository / "published-registry-v3.json"
            target.write_bytes(render_target(VALID, SCHEMA))
            registry_path.write_text('{"uncommitted":true}', encoding="utf-8")

            with self.assertRaisesRegex(RegistryTargetError, "exceeds the 1-byte limit"):
                build_registry_release._git_blob(
                    repository,
                    source_commit,
                    "registry/registry-v3.json",
                    1,
                )

            with mock.patch.object(build_registry_release, "ROOT", repository):
                with contextlib.redirect_stdout(io.StringIO()):
                    result = main([
                        "--source-commit", source_commit,
                        "--output", str(target),
                        "--check",
                    ])
            self.assertEqual(result, 0)

            target.write_bytes(target.read_bytes() + b" ")
            with mock.patch.object(build_registry_release, "ROOT", repository):
                with contextlib.redirect_stderr(io.StringIO()):
                    result = main([
                        "--source-commit", source_commit,
                        "--output", str(target),
                        "--check",
                    ])
            self.assertEqual(result, 1)

    def test_source_commit_ignores_git_replacement_refs(self):
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            registry_path = repository / "registry" / "registry-v3.json"
            schema_path = repository / "registry" / "schema-v3.schema.json"
            registry_path.parent.mkdir(parents=True)
            registry_path.write_text(json.dumps(VALID), encoding="utf-8")
            schema_path.write_bytes(SCHEMA_PATH.read_bytes())
            subprocess.run(["git", "init", "-q", str(repository)], check=True)
            subprocess.run(["git", "-C", str(repository), "add", "registry"], check=True)
            commit_args = [
                "git", "-C", str(repository),
                "-c", "user.name=Equalturn test",
                "-c", "user.email=test@example.invalid",
                "commit", "-qm",
            ]
            subprocess.run([*commit_args, "original"], check=True)
            original_commit = subprocess.run(
                ["git", "-C", str(repository), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()

            replacement = copy.deepcopy(VALID)
            replacement["entries"][0]["product"] = "Replacement Assistant"
            registry_path.write_text(json.dumps(replacement), encoding="utf-8")
            subprocess.run(["git", "-C", str(repository), "add", "registry/registry-v3.json"], check=True)
            subprocess.run([*commit_args, "replacement"], check=True)
            replacement_commit = subprocess.run(
                ["git", "-C", str(repository), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            subprocess.run(
                ["git", "-C", str(repository), "replace", original_commit, replacement_commit],
                check=True,
            )

            target = repository / "target.json"
            target.write_bytes(render_target(VALID, SCHEMA))
            with mock.patch.object(build_registry_release, "ROOT", repository):
                with contextlib.redirect_stdout(io.StringIO()):
                    result = main([
                        "--source-commit", original_commit,
                        "--output", str(target),
                        "--check",
                    ])
            self.assertEqual(result, 0)

    def test_cli_rejects_non_commit_source_identifiers(self):
        for source_commit in ("HEAD", ""):
            output = io.StringIO()
            with contextlib.redirect_stderr(output):
                result = main([
                    "--source-commit", source_commit,
                    "--output", str(ROOT / "missing-target.json"),
                    "--check",
                ])
            self.assertEqual(result, 1)
            self.assertIn("full Git object ID", output.getvalue())

    def test_cli_does_not_allow_registry_overrides_with_source_commit(self):
        output = io.StringIO()
        with contextlib.redirect_stderr(output):
            result = main([
                "--source-commit", "0" * 40,
                "--registry", str(FIXTURES / "valid.json"),
                "--output", str(ROOT / "missing-target.json"),
                "--check",
            ])
        self.assertEqual(result, 1)
        self.assertIn("selects both registry and schema from Git", output.getvalue())


if __name__ == "__main__":
    unittest.main()
