import copy
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))

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


if __name__ == "__main__":
    unittest.main()
