import copy
import importlib.util
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "equalturn_registry_v3_validator", ROOT / "tools" / "validate_registry_v3.py"
)
VALIDATOR_MODULE = importlib.util.module_from_spec(SPEC)
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
        self.assertEqual(VALIDATOR_MODULE.validate(), (129, 58))

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
