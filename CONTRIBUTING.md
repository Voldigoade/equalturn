# Contributing

Equalturn is currently a research and architecture repository. The first useful contributions are sourced corrections, documented failure cases, schema review, and small feasibility probes that do not become product enforcement code without a scoped implementation milestone.

## Research contributions

- Prefer OS-vendor, standards-body, upstream-project, and official product documentation.
- Include a direct source URL and the date checked.
- Label claims as verified fact, inference, proposal, or unresolved risk.
- Describe false positives, privilege requirements, and bypass limits alongside detection ideas.
- Do not present a provider list as complete.

## Technical contributions

- Read the [architecture](docs/architecture.md), [threat model](docs/threat-model.md), and [privacy model](docs/privacy.md) before proposing behavior.
- Keep OS-specific code behind narrow interfaces and privileged services.
- Add no telemetry beyond the documented session modes without explaining purpose, local/shared scope, retention, and consent.
- Do not add kernel drivers, TLS interception, screen capture, or keystroke capture without a separately reviewed threat and privacy case.

## Changes

Use focused commits with clear messages. Avoid generated binaries, secrets, private machine data, and unrelated formatting changes. The repository is documentation-only in this phase; implementation work should follow the roadmap and be separately scoped.
