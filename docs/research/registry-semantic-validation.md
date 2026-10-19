# Registry v3 semantic validation

**Status:** implemented. This validates a local research artifact; it does not authorize enforcement.

The v3 validator uses Python `jsonschema` with Draft 2020-12 and its format checker, then applies deterministic cross-field and cross-record checks. The JSON Schema defines the shape and closed vocabularies. Semantic checks enforce normalized values, references, evidence dates, canonical ordering, and consistency between confidence, actionability, lifecycle, and false-positive risk. CI fails on `ERROR`; `WARNING` and `INFO` diagnostics remain visible on successful runs.

Run it after installing the pinned dependency from `requirements-registry.txt`:

```sh
python3 -m pip install -r requirements-registry.txt
python3 tools/validate_registry_v3.py
```

The validator also accepts `--registry PATH` and `--schema PATH` for local review. Input is bounded to 4 MiB for a registry and 1 MiB for a schema. Invalid UTF-8, duplicate JSON object keys, malformed JSON, and oversized input fail before schema validation. Collection bounds are enforced by the schema and semantic checks. Diagnostics have stable severity, code, JSON path, and human-readable message fields; output order is deterministic.

## Semantic checks

- Canonical semantic version, ISO calendar dates, and no future verification dates.
- Unique entry IDs, provider aliases, normalized indicator kind/value pairs, entry-local source IDs, and source URL/check-date pairs.
- Resolved non-self relationships; indicator provenance resolves inside the same entry and its `verified_on` matches the cited source's `checked_on`.
- Normalized hostnames and wildcard placement; URL path, port, executable, package, extension, application identity, service, container image, and protocol endpoint syntax.
- Platform-kind compatibility and explicit use of `unknown` without mixing it with asserted scopes.
- Canonical ordering of entries, categories, surfaces, aliases, sources, indicators, and relationships.
- Safety consistency: a `blocking_candidate` requires current lifecycle, high confidence, low false-positive risk, and known platform/surface scope; `unsafe_to_block` requires high false-positive risk. Lower-confidence operational candidates remain visible warnings for review.

The validator rejects generic executable names such as `python`, `node`, `chrome`, `docker`, and `powershell`. There is no CIDR indicator kind in schema v3, so CIDR values cannot enter through an untyped escape hatch. Future indicator kinds require a reviewed schema change.

## Diagnostics and limits

`ERROR` means the document violates the schema or a semantic invariant and returns a nonzero exit status. `WARNING` highlights a classification that is structurally valid but deserves human review, such as low-confidence detection or high/unknown false-positive risk. `INFO I_RESEARCH_ONLY` reiterates that the catalogue is not policy. Diagnostics do not assess the truth of a source page, prove software execution, or prove whether a participant used AI.

The test corpus covers valid data, duplicate entry and indicator IDs, unresolved and self relationships, malformed hostnames and wildcard syntax, URL text in hostname fields, invalid ports, unsupported enums, missing/unresolved provenance, future dates, duplicate sources and aliases, actionability/risk contradictions, canonical order, visible warnings, malformed JSON, duplicate JSON keys, invalid UTF-8, and oversized input. The curated data review also consolidated six same-entry repeated URL/check-date pairs into single source records while retaining their separate assessments and updating references.

No validator branch applies network policy, reads workstation activity, fetches a source URL, or chooses an enforcement action. Human review remains necessary for whether a source substantiates an indicator and whether its classification is appropriate.
