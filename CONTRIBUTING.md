# Contributing

Equalturn has a working non-enforcing Milestone 1 Rust CLI and relay. Contributions should preserve the documented trust boundary and privacy model; endpoint enforcement remains out of scope until a separate milestone is reviewed.

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

Use focused commits with clear messages. Avoid generated binaries, secrets, private machine data, and unrelated formatting changes. Run `cargo fmt --all -- --check`, `python3 tools/validate_registry.py`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, and `cargo test --workspace --locked` before submitting Rust changes. PostgreSQL integration tests use `TEST_DATABASE_URL` when configured. Keep future enforcement work separately scoped to the roadmap.
