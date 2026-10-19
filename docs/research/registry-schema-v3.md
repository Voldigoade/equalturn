# Registry schema v3 and v2 migration

**Status:** implemented research format; no application enforcement consumer.

**Schema:** [`registry/schema-v3.schema.json`](../../registry/schema-v3.schema.json)

**Current target:** [`registry/registry-v3.json`](../../registry/registry-v3.json)

**Migration:** [`tools/migrate_registry_v2_to_v3.py`](../../tools/migrate_registry_v2_to_v3.py)

## Classification semantics

These fields classify the registry claim. They do not predict whether a participant used AI, and they are not executable policy.

| Field | Values | Meaning |
| --- | --- | --- |
| `confidence` | `high`, `medium`, `low` | Strength of evidence that the indicator value and stated scope are correct. It is not a probability. `high` requires direct and specific first-party/upstream evidence; `medium` is credible but indirect or constrained; `low` is ambiguous, weak, or not yet individually reviewed. |
| `actionability` | `research_only`, `advisory`, `detection_candidate`, `blocking_candidate`, `unsafe_to_block` | What kind of later human review an indicator may inform. `research_only` has no proposed operational use; `advisory` may help explain a possible signal; `detection_candidate` could support a carefully scoped observation; `blocking_candidate` is eligible for separate policy review but is not a block rule; `unsafe_to_block` must not be used as a block key because its scope or collateral impact is unacceptable. |
| `false_positive_risk` | `low`, `medium`, `high`, `unknown` | Risk that unrelated or permitted activity matches the indicator. `unknown` is explicitly not equivalent to low risk. |
| `lifecycle` | `current`, `deprecated`, `archived`, `disputed`, `unknown` | Lifecycle of the indicator claim, independently of the product record status. `current` requires recent evidence for the indicator itself; a product-level observation alone does not establish it. |
| `platforms` | OS/platform enum, including `unknown` | Platforms for which the indicator is evidenced. A value of `any` is intentionally absent: broad applicability must be enumerated or left unknown, not assumed. |
| `surfaces` | Product/surface enum, including indicator-level `unknown` | The surface directly associated with the indicator. It is distinct from all surfaces a product may offer. |

## Indicator kinds

`kind` has a closed enum. Current kinds cover normalized web/API hostnames and URL paths, executable/package names, OS package or application identifiers, editor/browser extension IDs, local ports, service names, container images, and protocol endpoints. A generic `other` escape hatch is not provided: if a future value cannot fit the vocabulary, the schema should be reviewed and versioned first.

Kind-specific structured fields are explicit. `package_name` requires an `ecosystem`; `local_port` requires `bind_scope` and a numeric port value. Ports are observations, not identities: a port can be configurable, shared, or used by an unrelated service. A future semantic validator must enforce port range and format, host normalization, identifier syntax, and the safety consistency rules listed below.

## Safety consistency rules

The schema is strict about shape and enums. Cross-field security semantics belong in the semantic validator and human review:

- `unsafe_to_block` must never also be a blocking candidate; the single-valued enum makes this unrepresentable.
- `blocking_candidate` with `low` confidence or `high`/`unknown` false-positive risk is an error, not a warning.
- `blocking_candidate` requires specific provenance, a concrete platform and surface, and limitations that identify bypass and collateral-use boundaries.
- shared infrastructure, generic executable names, and generic local ports must not be promoted to `blocking_candidate` without a specific product-scoped discriminator; otherwise classify `unsafe_to_block` or `research_only`.
- every indicator's `source_id` resolves within its own entry; `verified_on` must match an evidence check or be supported by explicit indicator-level verification.
- product lifecycle does not flow automatically into indicator lifecycle.

These rules are review criteria, not claims that the registry can prove an indicator factually correct. Registry contents remain research and never authorize endpoint enforcement.

## One-time v2 import

The v2 migration is a deterministic, conservative import utility. V2 is retained as historical input; v3 is independently curated after import. Do not run `--force` against the checked-in `registry/registry-v3.json` after curation because that would erase v3-only research and classifications. Determinism is covered by the migration unit tests. To inspect a fresh migration without touching the maintained target, write it to a separate path:

```sh
python3 tools/migrate_registry_v2_to_v3.py --output /tmp/registry-v3-import.json
```

Only during an intentional re-import should an existing temporary output be replaced:

```sh
python3 tools/migrate_registry_v2_to_v3.py --output /tmp/registry-v3-import.json --force
```

The migration rejects unsupported schema versions, unknown v2 indicator kinds, unresolved source references, unrepresentable URL paths, and malformed/out-of-range loopback listeners. It sorts entries, categories, product surfaces, numeric local source IDs, indicators, relationships, and aliases, then serializes UTF-8 JSON with fixed indentation and one trailing newline. Existing outputs are not overwritten unless `--force` is passed. The importer is not a generator for the curated v3 dataset.

V2 kinds map to v3 as follows: `domain_exact` → `web_hostname`; `api_host` → `api_hostname`; `url_path` → `url_path`; `process_name` → `executable_name`; `package_name` → `package_name`; `tcp_listener` → `local_port`. Hostnames are lowercased and stripped of a terminal dot; path URLs are normalized to HTTPS origin plus path and reject query/fragment data; loopback listener values become a numeric port plus `bind_scope: loopback`. The package value `@kilocode/cli` is classified as npm from its scoped package form; other package ecosystems remain `unknown`.

V2 did not contain indicator-level platform, confidence, actionability, false-positive risk, or lifecycle. The migration therefore sets `platforms: ["unknown"]`, `confidence: "low"`, `actionability: "research_only"`, `false_positive_risk: "unknown"`, and `lifecycle: "unknown"`. These are deliberate safe defaults rather than inferred judgments. Indicator `verified_on` is copied from its resolved source's `checked_on`. Web/API host and URL-path kinds map to `web`/`api` surfaces and local ports map to `local_server`; executable and package indicators inherit a single local product surface only when the v2 record has exactly one, otherwise they use `unknown`. Individual classifications and platform claims can change only after source review.

## Provenance and review limits

Source IDs remain entry-local. Indicator claims cite one source from the same product record and keep scope/limitations alongside the claim. The checked date records when a source was reviewed; it does not certify that the vendor page remains live, prove the page supports every interpretation, or imply a refresh schedule. The Stage 6 [quality audit](registry-quality-audit.md) is selective and static, not a guarantee that every URL was fetched successfully on the review date.

The current curated snapshot has 131 entries, 158 source records, and 84 indicators. Its 58 imported v2 indicators retain the v2 source history but have been reviewed and assigned conservative, evidence-specific classifications. The additional structured identifiers are documented in the [indicator expansion review](registry-indicator-expansion.md). The v3 schema is the data contract; semantic cross-record diagnostics and the negative fixture corpus are implemented in the following validator stage.
