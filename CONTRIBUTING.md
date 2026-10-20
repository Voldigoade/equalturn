# Contributing

Equalturn has a working non-enforcing Milestone 1 Rust CLI and relay, hardened session-key storage, registry v3 semantic validation, and Milestone 1.6 TUF trust plumbing with an opt-in HTTPS staging path. The production registry root/release and all endpoint enforcement remain uninitialized or out of scope. Contributions must preserve the documented trust boundary and privacy model.

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
- Never exercise experimental enforcement on the host running development or CI. Follow [the enforcement test safety policy](docs/enforcement-test-safety.md); if the required VM, Sandbox, or namespace is unavailable, skip that dangerous test and report it.

## Registry contributions

1. Research the product or provider using first-party documentation, an upstream repository, or another independently credible source. Record a direct URL, the date checked, and what the source establishes.
2. Add or update the product record and source assessment in `registry/registry-v3.json`. Keep provider identity separate from product/surface identity and describe configurable or provider-routed behavior.
3. Add an indicator only when its exact value and scope are supported. Select confidence as evidence strength, then choose actionability and false-positive risk independently. Explain shared infrastructure, ambiguity, platform/surface limits, and why a proposed blocking candidate is narrow enough to review.
4. Run the v3 schema and semantic checks and inspect every diagnostic:

   ```sh
   python3 tools/validate_registry_v3.py
   python3 tools/validate_registry.py
   ```

5. Build and compare the deterministic target. Install the pinned Python dependency from `requirements-registry.txt` first if needed; `dist/` is ignored build output.

   ```sh
   python3 tools/build_registry_release.py --output dist/registry-v3.json
   python3 tools/build_registry_release.py --output dist/registry-v3.json --check
   ```

6. In the pull request, explain why each indicator exists, how its source supports the value, and why its actionability/false-positive classification is justified. Request an independent review for source claims and safety semantics.
7. Do not create or publish TUF metadata or signing keys as part of a registry-content pull request. The public staging endpoint is a non-production compatibility fixture with disposable signer material; its static deploy workflow cannot create a new signed release. Production publication is not initialized. Any future release requires an approved signer/custody design and the separate release gates in [registry trust](docs/registry-trust.md).

Registry contents remain research data. They do not trigger network policy or workstation enforcement.

## Registry trust and release gates

TUF establishes that metadata/target bytes were authorized by the configured key policy and have not been altered. It does not prove that an indicator is true, current, or safe to block. Research review and signing authorization are distinct checks.

Before an authorized maintainer requests or performs a TUF target release, the release record must include:

- the exact Equalturn source commit, registry semantic version, deterministic target SHA-256 and byte length;
- schema and semantic validator output, including reviewer disposition for every warning;
- a semantic diff of changed products, sources, relationships, and indicators;
- an evidence-based reason for every new/changed indicator and its confidence, actionability, false-positive risk, scope, and limitations;
- the expected repository identity, metadata versions/expiries, trusted root digest, and signer threshold;
- independent review approval and post-publication target/hash verification with the Rust client.

Do not sign directly from an unreviewed working tree. The current staging deploy job only copies fixed static files; it holds no signing key and cannot renew metadata. Current staging root/targets signer material is not durable, and the earliest published metadata expiry is 18 November 2026. Staging renewals require a deliberate separate non-production signing operation; the production root ceremony remains blocked until its maintainer-custody prerequisites are satisfied. See [registry disaster recovery](docs/registry-disaster-recovery.md), [registry trust](docs/registry-trust.md), and [production root ceremony](docs/production-root-ceremony.md).

## Changes

Use focused commits with clear messages. Avoid generated binaries, secrets, private machine data, and unrelated formatting changes. Before submitting Rust changes, run `cargo fmt --all -- --check`, `python3 tools/validate_registry_v3.py`, `python3 tools/validate_registry.py`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, and `cargo test --workspace --locked`. Run `cargo audit --file Cargo.lock` when `cargo-audit` is installed. PostgreSQL integration tests use `TEST_DATABASE_URL` when configured; see the README for the local Compose service. Keep future enforcement work separately scoped to the roadmap and follow the safety policy linked above.
