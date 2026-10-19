import copy
import importlib.util
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "equalturn_registry_migration", ROOT / "tools" / "migrate_registry_v2_to_v3.py"
)
MIGRATION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MIGRATION)


class RegistryMigrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = json.loads((ROOT / "registry" / "registry-v2.json").read_text(encoding="utf-8"))

    def test_full_migration_is_deterministic_and_conservative(self):
        first = MIGRATION.migrate_registry(self.source)
        second = MIGRATION.migrate_registry(self.source)

        self.assertEqual(MIGRATION.serialize_registry(first), MIGRATION.serialize_registry(second))
        self.assertEqual(first["schema_version"], 3)
        self.assertEqual(first["registry_version"], "0.3.0")
        self.assertEqual(len(first["entries"]), len(self.source["entries"]))
        self.assertEqual([entry["id"] for entry in first["entries"]], sorted(entry["id"] for entry in first["entries"]))
        self.assertEqual(
            {indicator["kind"] for entry in first["entries"] for indicator in entry["indicators"]},
            {"web_hostname", "api_hostname", "url_path", "executable_name", "package_name", "local_port"},
        )

        for entry in first["entries"]:
            for indicator in entry["indicators"]:
                self.assertEqual(indicator["platforms"], ["unknown"])
                self.assertEqual(indicator["confidence"], "low")
                self.assertEqual(indicator["actionability"], "research_only")
                self.assertEqual(indicator["false_positive_risk"], "unknown")
                self.assertEqual(indicator["lifecycle"], "unknown")
                self.assertIn(indicator["source_id"], {source["id"] for source in entry["sources"]})

    def test_legacy_indicator_kinds_and_values_are_normalized(self):
        migrated = MIGRATION.migrate_registry(self.source)
        indicators = {
            (entry["id"], item["kind"]): item
            for entry in migrated["entries"]
            for item in entry["indicators"]
        }

        self.assertEqual(indicators[("openai-chatgpt", "web_hostname")]["value"], "chatgpt.com")
        self.assertEqual(indicators[("openai-api", "api_hostname")]["value"], "api.openai.com")
        self.assertEqual(
            indicators[("hugging-face-huggingchat", "url_path")]["value"],
            "https://huggingface.co/chat/",
        )
        self.assertEqual(indicators[("ollama", "local_port")]["value"], "11434")
        self.assertEqual(indicators[("ollama", "local_port")]["bind_scope"], "loopback")
        self.assertEqual(indicators[("kilo-code-cli", "package_name")]["ecosystem"], "npm")

    def test_migration_rejects_non_v2_input(self):
        source = copy.deepcopy(self.source)
        source["schema_version"] = 1

        with self.assertRaisesRegex(MIGRATION.MigrationError, "schema_version must be 2"):
            MIGRATION.migrate_registry(source)

    def test_migration_rejects_unrecognized_indicator_kind(self):
        source = copy.deepcopy(self.source)
        source["entries"][0]["indicators"].append(
            {
                "kind": "generic_process",
                "value": "python",
                "scope": "generic executable",
                "source_id": "source-1",
                "limitations": "not product-specific",
            }
        )

        with self.assertRaisesRegex(MIGRATION.MigrationError, "unsupported v2 indicator kind"):
            MIGRATION.migrate_registry(source)

    def test_migration_rejects_unrepresentable_listener(self):
        source = copy.deepcopy(self.source)
        entry = next(item for item in source["entries"] if item["id"] == "ollama")
        entry["indicators"][0]["value"] = "localhost:70000"

        with self.assertRaisesRegex(MIGRATION.MigrationError, "out of range"):
            MIGRATION.migrate_registry(source)

    def test_v3_schema_declares_classification_and_indicator_safety_fields(self):
        schema = json.loads((ROOT / "registry" / "schema-v3.schema.json").read_text(encoding="utf-8"))
        self.assertEqual(schema["$schema"], "https://json-schema.org/draft/2020-12/schema")
        indicator = schema["$defs"]["indicator"]
        self.assertTrue(indicator["additionalProperties"] is False)
        self.assertTrue(
            {
                "platforms", "surfaces", "verified_on", "confidence", "actionability",
                "false_positive_risk", "lifecycle", "limitations", "source_id"
            }.issubset(indicator["required"])
        )
        self.assertEqual(
            indicator["properties"]["actionability"]["enum"],
            ["research_only", "advisory", "detection_candidate", "blocking_candidate", "unsafe_to_block"],
        )


if __name__ == "__main__":
    unittest.main()
