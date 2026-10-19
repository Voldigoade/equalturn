import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests" / "fixtures" / "registry-v3"
VALID = json.loads((FIXTURES / "valid.json").read_text(encoding="utf-8"))
VALIDATOR = ROOT / "tools" / "validate_registry_v3.py"


def clone_registry():
    return copy.deepcopy(VALID)


def run_validator(registry_path):
    return subprocess.run(
        [sys.executable, str(VALIDATOR), "--registry", str(registry_path)],
        check=False,
        capture_output=True,
        text=True,
    )


class RegistrySemanticValidatorTests(unittest.TestCase):
    def validate_variant(self, registry, expected_code):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "registry.json"
            path.write_text(json.dumps(registry), encoding="utf-8")
            result = run_validator(path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(expected_code, result.stderr)

    def test_valid_registry_fixture_passes(self):
        result = run_validator(FIXTURES / "valid.json")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("VALID", result.stdout)

    def test_duplicate_entry_id_is_an_error(self):
        registry = clone_registry()
        duplicate = copy.deepcopy(registry["entries"][0])
        duplicate["product"] = "Conflicting Product"
        registry["entries"].append(duplicate)

        self.validate_variant(registry, "E_DUPLICATE_ENTRY_ID")

    def test_unresolved_relationship_is_an_error(self):
        registry = clone_registry()
        registry["entries"][0]["relationships"].append(
            {"kind": "integrates_with", "target_id": "missing-product", "notes": "fixture"}
        )

        self.validate_variant(registry, "E_RELATIONSHIP_TARGET")

    def test_hostname_with_empty_label_is_an_error(self):
        registry = clone_registry()
        registry["entries"][0]["indicators"][0]["value"] = "bad..example"

        self.validate_variant(registry, "E_HOSTNAME_INVALID")

    def test_unknown_platform_cannot_be_mixed_with_asserted_platforms(self):
        registry = clone_registry()
        registry["entries"][0]["indicators"][0]["platforms"] = ["linux", "unknown"]

        self.validate_variant(registry, "E_UNKNOWN_SCOPE_MIXED")

    def test_generic_executable_name_is_rejected(self):
        registry = clone_registry()
        indicator = registry["entries"][0]["indicators"][0]
        indicator.update(kind="executable_name", value="python")

        self.validate_variant(registry, "E_GENERIC_EXECUTABLE")

    def test_url_in_hostname_field_is_an_error(self):
        registry = clone_registry()
        registry["entries"][0]["indicators"][0]["value"] = "https://example.com/path"

        self.validate_variant(registry, "E_HOSTNAME_INVALID")

    def test_wildcard_must_be_a_single_leading_label(self):
        registry = clone_registry()
        registry["entries"][0]["indicators"][0]["value"] = "api.*.example.com"

        self.validate_variant(registry, "E_HOSTNAME_WILDCARD")

    def test_port_outside_tcp_range_is_an_error(self):
        registry = clone_registry()
        indicator = registry["entries"][0]["indicators"][0]
        indicator.update(kind="local_port", value="70000", bind_scope="loopback")

        self.validate_variant(registry, "E_PORT_RANGE")

    def test_unsupported_enum_is_reported_as_schema_error(self):
        registry = clone_registry()
        registry["entries"][0]["indicators"][0]["actionability"] = "block_everything"

        self.validate_variant(registry, "E_SCHEMA")

    def test_unresolved_indicator_source_is_an_error(self):
        registry = clone_registry()
        registry["entries"][0]["indicators"][0]["source_id"] = "source-9"

        self.validate_variant(registry, "E_SOURCE_UNRESOLVED")

    def test_indicator_without_provenance_is_a_schema_error(self):
        registry = clone_registry()
        del registry["entries"][0]["indicators"][0]["source_id"]

        self.validate_variant(registry, "E_SCHEMA")

    def test_noncanonical_semantic_version_is_an_error(self):
        registry = clone_registry()
        registry["registry_version"] = "01.2.3"

        self.validate_variant(registry, "E_SEMVER_INVALID")

    def test_future_verification_date_is_an_error(self):
        registry = clone_registry()
        registry["entries"][0]["indicators"][0]["verified_on"] = "2099-01-01"

        self.validate_variant(registry, "E_FUTURE_DATE")

    def test_duplicate_normalized_indicator_is_an_error(self):
        registry = clone_registry()
        duplicate = copy.deepcopy(registry["entries"][0]["indicators"][0])
        registry["entries"][0]["indicators"].append(duplicate)

        self.validate_variant(registry, "E_DUPLICATE_INDICATOR")

    def test_high_false_positive_risk_cannot_be_a_blocking_candidate(self):
        registry = clone_registry()
        indicator = registry["entries"][0]["indicators"][0]
        indicator.update(actionability="blocking_candidate", confidence="low", false_positive_risk="high")

        self.validate_variant(registry, "E_BLOCKING_UNSAFE")

    def test_duplicate_aliases_are_reported(self):
        registry = clone_registry()
        registry["entries"][0]["provider"]["aliases"] = ["Sample", "sample"]

        self.validate_variant(registry, "E_DUPLICATE_ALIAS")

    def test_duplicate_source_url_and_date_are_reported(self):
        registry = clone_registry()
        duplicate = copy.deepcopy(registry["entries"][0]["sources"][0])
        duplicate["id"] = "source-2"
        registry["entries"][0]["sources"].append(duplicate)

        self.validate_variant(registry, "E_DUPLICATE_SOURCE")

    def test_noncanonical_collection_order_is_reported(self):
        registry = clone_registry()
        registry["entries"][0]["categories"] = ["hosted_chat", "gateway"]

        self.validate_variant(registry, "E_ORDER_CATEGORIES")

    def test_duplicate_json_object_key_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "registry.json"
            path.write_text('{"schema_version":3,"schema_version":3}', encoding="utf-8")
            result = run_validator(path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("E_JSON_DUPLICATE_KEY", result.stderr)

    def test_invalid_utf8_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "registry.json"
            path.write_bytes(b"\xff")
            result = run_validator(path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("E_UTF8_INVALID", result.stderr)

    def test_oversized_registry_is_rejected_before_json_parse(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "oversized.json"
            path.write_bytes(b" " * (4 * 1024 * 1024 + 1))
            result = run_validator(path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("E_FILE_TOO_LARGE", result.stderr)

    def test_malformed_json_fixture_has_stable_error(self):
        result = run_validator(FIXTURES / "malformed-json.json")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("E_JSON_INVALID", result.stderr)

    def test_low_confidence_detection_candidate_emits_visible_warning(self):
        registry = clone_registry()
        indicator = registry["entries"][0]["indicators"][0]
        indicator.update(confidence="low", actionability="detection_candidate", false_positive_risk="unknown")

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "registry.json"
            path.write_text(json.dumps(registry), encoding="utf-8")
            result = run_validator(path)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("WARNING W_LOW_CONFIDENCE_ACTIONABILITY", result.stdout)


if __name__ == "__main__":
    unittest.main()
