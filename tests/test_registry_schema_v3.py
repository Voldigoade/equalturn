import copy
import importlib.util
import json
import sys
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / "tools"
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))
SPEC = importlib.util.spec_from_file_location(
    "equalturn_registry_v3_validator", ROOT / "tools" / "validate_registry_v3.py"
)
VALIDATOR_MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = VALIDATOR_MODULE
SPEC.loader.exec_module(VALIDATOR_MODULE)


class RegistrySchemaV3Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schema = json.loads((ROOT / "registry" / "schema-v3.schema.json").read_text(encoding="utf-8"))
        cls.registry = json.loads((ROOT / "registry" / "registry-v3.json").read_text(encoding="utf-8"))
        cls.validator = Draft202012Validator(
            cls.schema,
            format_checker=Draft202012Validator.FORMAT_CHECKER,
        )

    def test_checked_in_v3_registry_passes_draft_2020_12_validation(self):
        result = VALIDATOR_MODULE.validate()

        self.assertEqual(result.errors, [])
        self.assertEqual(result.warnings, [])
        self.assertEqual(result.entry_count, len(self.registry["entries"]))
        self.assertEqual(
            result.indicator_count,
            sum(len(entry["indicators"]) for entry in self.registry["entries"]),
        )

    def test_researched_identifiers_cover_major_local_and_editor_surfaces(self):
        indicators = [
            indicator
            for entry in self.registry["entries"]
            for indicator in entry["indicators"]
        ]
        values = {(item["kind"], item["value"]) for item in indicators}

        self.assertIn(("vscode_extension_id", "anthropic.claude-code"), values)
        self.assertIn(("vscode_extension_id", "github.copilot"), values)
        self.assertIn(("package_name", "@moonshot-ai/kimi-code"), values)
        self.assertIn(("container_image", "ghcr.io/open-webui/open-webui"), values)
        self.assertIn(("executable_name", "vllm"), values)

    def test_research_indicators_do_not_authorize_blocking(self):
        self.assertFalse(
            any(
                indicator["actionability"] == "blocking_candidate"
                for entry in self.registry["entries"]
                for indicator in entry["indicators"]
            )
        )

    def test_local_listener_ports_are_explicitly_unsafe_to_block(self):
        listeners = [
            indicator
            for entry in self.registry["entries"]
            for indicator in entry["indicators"]
            if indicator["kind"] == "local_port"
        ]

        self.assertEqual(len(listeners), 3)
        self.assertTrue(
            all(
                indicator["actionability"] == "unsafe_to_block"
                and indicator["false_positive_risk"] == "high"
                for indicator in listeners
            )
        )

    def test_every_indicator_resolves_entry_local_provenance(self):
        for entry in self.registry["entries"]:
            source_ids = {source["id"] for source in entry["sources"]}
            for indicator in entry["indicators"]:
                self.assertIn(indicator["source_id"], source_ids, entry["id"])

    def test_archived_roo_code_identifier_is_not_currently_actionable(self):
        entry = next(
            item for item in self.registry["entries"]
            if item["id"] == "roo-code-vs-code-agent"
        )
        indicator = next(
            item for item in entry["indicators"]
            if item["kind"] == "vscode_extension_id"
        )

        self.assertEqual(entry["status"], "archived")
        self.assertEqual(indicator["lifecycle"], "archived")
        self.assertEqual(indicator["actionability"], "research_only")

    def test_schema_rejects_unknown_actionability(self):
        registry = copy.deepcopy(self.registry)
        indicator = next(item for entry in registry["entries"] for item in entry["indicators"])
        indicator["actionability"] = "block_everything"

        self.assertTrue(list(self.validator.iter_errors(registry)))

    def test_schema_requires_package_ecosystem(self):
        registry = copy.deepcopy(self.registry)
        indicator = next(
            item
            for entry in registry["entries"]
            for item in entry["indicators"]
            if item["kind"] == "package_name"
        )
        del indicator["ecosystem"]

        self.assertTrue(list(self.validator.iter_errors(registry)))

    def test_schema_requires_local_listener_bind_scope(self):
        registry = copy.deepcopy(self.registry)
        indicator = next(
            item
            for entry in registry["entries"]
            for item in entry["indicators"]
            if item["kind"] == "local_port"
        )
        del indicator["bind_scope"]

        self.assertTrue(list(self.validator.iter_errors(registry)))

    def test_schema_rejects_unrecognized_indicator_properties(self):
        registry = copy.deepcopy(self.registry)
        indicator = next(item for entry in registry["entries"] for item in entry["indicators"])
        indicator["policy"] = "deny"

        self.assertTrue(list(self.validator.iter_errors(registry)))


if __name__ == "__main__":
    unittest.main()
