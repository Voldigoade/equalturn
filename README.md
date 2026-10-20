# Equalturn

Equalturn is a privacy-conscious integrity session system for people who mutually agree to compete without AI assistance. It gives participants pseudonymous session identities and a shared, signed record of events accepted by a relay.

**Status: Milestones 1, 1.5, and 1.6 are implemented; endpoint enforcement is not.** The Rust workspace contains a CLI client, PostgreSQL-backed relay, signed-event protocol, durable local event queue, verifiable reports, hardened participant-key storage, registry v3 validation, and explicit local/HTTPS TUF update flows with separately installed staging/production profiles. A public staging repository is verified by a pinned test root; production has no root or signed release and fails closed. Registry updates are explicit and never coupled to session or relay activity. The product remains non-enforcing: it accepts synthetic events and session/liveness claims only. It does not inspect workstation activity, block AI services or applications, or include a tray UI. Do not use its event log as proof that prohibited activity did or did not occur.

Participant signatures prove which session key made a claim and whether the included event chain was changed. Relay signatures attest to event receipt/order and to the final manifest. Neither proves that a client observed reality honestly, that an event was not omitted before signing, or that the relay did not withhold evidence. A workstation administrator, second device, virtual machine, remote host, or compromised client remains outside the guarantees of this milestone.

## Start here

- [Product brief](docs/product-brief.md)
- [Implemented and proposed architecture](docs/architecture.md)
- [Protocol v1 contract](docs/protocol-v1.md)
- [Threat model](docs/threat-model.md)
- [Windows and Linux enforcement research](docs/enforcement-matrix.md) — future work; no enforcement is implemented
- [AI ecosystem and registry design](docs/ai-registry.md)
- [Researched registry v3](registry/registry-v3.json) — 131 records and 84 sourced indicators; research data only, never enforcement policy
- [Indicator expansion review](docs/research/registry-indicator-expansion.md)
- [Registry v3 schema and migration](docs/research/registry-schema-v3.md)
- [Privacy and report model](docs/privacy.md)
- [Trusted registry distribution and current limits](docs/registry-trust.md)
- [Registry distribution incident recovery](docs/registry-disaster-recovery.md)
- [Enforcement test safety policy](docs/enforcement-test-safety.md)
- [Secret lifecycle and storage limits](docs/secret-lifecycle.md)
- [Prior art](docs/prior-art.md)
- [Stack decision](docs/stack-decision.md)
- [Name research](docs/naming.md)
- [Implementation roadmap](docs/implementation-roadmap.md)
- [Milestone 1 implementation plan and verification evidence](docs/milestone-one-implementation-plan.md)
- [Milestone 1.5 implementation plan and checkpoints](docs/superpowers/plans/2026-10-19-milestone-1-5-security-registry.md)
- [Research sources](docs/research/sources.md)

## What Milestone 1 implements

- Create a session and join it using a high-entropy invitation code and a conservative ASCII nickname.
- Generate a fresh Ed25519 signing identity for each participant/session.
- Sign deterministic, versioned event bytes; chain each participant's events by sequence and prior-event hash.
- Send events through the relay, which verifies membership/signatures/chains and returns signed receipts. Participants communicate through the relay and are not shown each other's network addresses.
- Authenticate persistent WebSocket reconnects with a one-use challenge; report heartbeat/liveness transitions as online, stale, disconnected, or ended.
- Persist signed outbox events and session keys locally; resend unacknowledged events idempotently after connectivity returns.
- Export a JSON evidence bundle and Markdown summary. Verify participant events, chains, receipts, relay notices, and final manifest offline.
- Submit an explicitly synthetic `ai-access-attempt` event to exercise the same event path; this is not an observed or blocked access attempt.

The protocol, limits, signatures, reconnect behavior, retention rules, and known limitations are specified in [protocol v1](docs/protocol-v1.md). The researched AI registry includes structured product, package, extension, executable, and runtime indicators. The session client and relay do not use it for policy; only the separate registry CLI reads a locally verified TUF target.

## Milestones 1.5 and 1.6 implementation boundary

- Participant session keys use current-user DPAPI on Windows and owner-only descriptor-relative storage on supported Unix systems. These protect data at rest within their documented limits; they do not resist the same user, administrator/root, or process-memory compromise.
- The registry CLI verifies bounded TUF metadata/targets from local repositories and an explicit HTTPS endpoint, checks registry schema/semantics, and installs a re-verifiable snapshot through an atomic two-slot store. Staging and production use separate local profiles; production remains unavailable until a root is embedded through an authenticated release.
- The registry remains a sourced research catalogue. No registry indicator drives enforcement, and no daemon/service, firewall rule, process observation/blocking, browser/IDE monitoring, or tray UI exists.
- Future enforcement testing must follow the [isolated-environment safety policy](docs/enforcement-test-safety.md). No experimental enforcement may run on the active development host.

## Offline registry commands

These commands accept a **local** TUF repository directory containing `metadata/` and `targets/`. Each command verifies the repository and the registry target before displaying entries. They do not contact a mirror, relay, or other network service. The root file and lowercase SHA-256 pin must come from an independently authenticated release channel; the repository itself cannot establish its own initial trust.

```sh
equalturn registry verify ./local-registry \
  --root ./trusted-root.json \
  --root-sha256 ROOT_SHA256

equalturn registry inspect ./local-registry openai-chatgpt \
  --root ./trusted-root.json \
  --root-sha256 ROOT_SHA256

equalturn registry search ./local-registry openai \
  --root ./trusted-root.json \
  --root-sha256 ROOT_SHA256
```

The verified metadata cache is persistent below the Equalturn data directory by default; set `--cache-dir` to select another location. Verification failures print a stable status such as `EXPIRED`, `ROLLBACK`, `INVALID_SIGNATURE`, `INVALID_ROOT`, `INVALID_TARGET`, or `INVALID_REGISTRY` and return a non-zero exit code. A valid TUF signature authenticates publication, not indicator truth or blocking safety. Production trust has no root or signed release.

## Explicit TUF profile commands

The HTTPS updater is opt-in and requires an out-of-band root plus an independent lowercase SHA-256 pin on every command. These commands use the public staging test root checked into `crates/registry/tests/fixtures/`; it is a compatibility fixture, **not production trust**:

```sh
equalturn registry update --profile staging \
  --url https://voldigoade.github.io/equalturn-registry-staging/ \
  --root crates/registry/tests/fixtures/staging-root.json \
  --root-sha256 719e372ecc71e8f509f5805889560382d3d599971b82947f9da825101f323144

equalturn registry status --profile staging \
  --root crates/registry/tests/fixtures/staging-root.json \
  --root-sha256 719e372ecc71e8f509f5805889560382d3d599971b82947f9da825101f323144

equalturn registry installed search --profile staging \
  --root crates/registry/tests/fixtures/staging-root.json \
  --root-sha256 719e372ecc71e8f509f5805889560382d3d599971b82947f9da825101f323144 \
  openai
```

Only `registry update --profile staging` contacts the selected HTTPS repository. Status and installed search reverify the local snapshot offline. The production profile intentionally returns `production registry trust not initialized`; do not copy the staging root into a release or use it to make that profile succeed. The current staging timestamp metadata expires 18 November 2026 and the fixed-content staging workflow does not renew TUF metadata; after expiry the client and smoke test fail closed until an explicit staging signing operation publishes fresh metadata.

## Local development

Requirements: the Rust toolchain declared in [`rust-toolchain.toml`](rust-toolchain.toml), Cargo, and Docker Compose (or a compatible local PostgreSQL 17 instance). The workspace can compile without a running database. Relay integration tests use `TEST_DATABASE_URL` when configured.

1. Copy the local environment example, replace the placeholder password with a random local development password, then start PostgreSQL:

   ```sh
   cp .env.example .env
   # Edit .env and replace the local placeholder password in all three values.
   docker compose up -d postgres
   set -a
   . ./.env
   set +a
   ```

2. In one terminal, run the relay in explicit loopback-only plaintext development mode. Migrations run on startup; production/non-loopback deployments require a TLS certificate and key.

   ```sh
   cargo run -p equalturn-relay -- serve --allow-insecure-loopback
   ```

3. In each client terminal, set the local relay URL and create/join the session. The creator shares the invitation code; the CLI prints the session UUID for lifecycle and report commands.

   ```sh
   export EQUALTURN_RELAY_URL=http://127.0.0.1:8787
   cargo run -p equalturn -- --allow-insecure-loopback session create --nickname voldigoade
   ```

   Copy the displayed invitation code and session UUID. In the second terminal:

   ```sh
   export EQUALTURN_RELAY_URL=http://127.0.0.1:8787
   cargo run -p equalturn -- --allow-insecure-loopback session join INVITATION_CODE --nickname opponent
   ```

   Replace `INVITATION_CODE` with the complete code printed by the creator. The invitation is valid for ten minutes, up to seven successful joins and eight participants total, and is unusable once the session starts.

4. The creator starts the session. Both clients may watch the session for authenticated reconnect, heartbeat, and live event delivery. Run `watch` in a foreground terminal; it exits when the session ends.

   ```sh
   cargo run -p equalturn -- --allow-insecure-loopback session start --session SESSION_UUID
   cargo run -p equalturn -- --allow-insecure-loopback session watch --session SESSION_UUID
   ```

5. Submit a synthetic event through the ordinary client, signature, local queue, relay verification, receipt, fan-out, and report path:

   ```sh
   cargo run -p equalturn -- --allow-insecure-loopback event simulate --session SESSION_UUID --kind ai-access-attempt --target chatgpt.com
   ```

   The event is explicitly synthetic; the CLI does not observe network or application use. To exercise offline queuing, stop the relay, submit the event while it is unavailable, restart the relay, then synchronize the signed local queue:

   ```sh
   cargo run -p equalturn -- --allow-insecure-loopback session sync --session SESSION_UUID
   ```

6. The creator ends the session, then export and verify the evidence bundle:

   ```sh
   cargo run -p equalturn -- --allow-insecure-loopback session end --session SESSION_UUID
   cargo run -p equalturn -- --allow-insecure-loopback report export --session SESSION_UUID --json equalturn-report.json --markdown equalturn-report.md
   cargo run -p equalturn -- report verify equalturn-report.json
   ```

   `report verify` works offline and accepts an optional `--relay-key` lowercase-hex public key to check against a fingerprint pinned out of band. The key printed during session creation/join is informational unless participants compare it through a trusted channel.

Useful verification commands:

```sh
cargo fmt --all -- --check
python3 tools/validate_registry.py
python3 tools/validate_registry_v3.py
python3 tools/build_registry_release.py --output dist/registry-v3.json
python3 tools/build_registry_release.py --output dist/registry-v3.json --check
python3 tools/build_registry_release.py --source-commit FULL_COMMIT_ID --output downloaded/registry-v3.json --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
```

The v3 validator and deterministic target builder require the pinned Python package in `requirements-registry.txt`. Install it in a virtual environment (`python3 -m venv .venv`, activate `.venv`, then `python -m pip install -r requirements-registry.txt`). The generated `dist/` directory is ignored build output, not a maintained source file.

The `--source-commit` comparison expects `downloaded/registry-v3.json` to already contain the published target obtained through the trusted TUF update path; it compares those bytes with the registry and schema from the exact Git commit.

## Privacy and trust boundary

Participant-visible status and reports contain session IDs, nicknames, session-scoped public keys/fingerprints, client version and coarse self-reported OS family, signed events, relay receipts/notices, and liveness/session state. They do not contain peer IP addresses. The relay necessarily sees source addresses at the transport layer; hosting, reverse-proxy, and infrastructure logs may retain connection metadata outside the application report, so relay operators must define and publish their own access and retention policy.

The client and relay do not collect keystrokes, clipboard contents, prompts, chat messages, passwords, tokens, cookies, screenshots, browsing history, command-line arguments, process inventories, filesystem inventories, or arbitrary URLs. Relay persistence has primary-database cleanup for unstarted sessions after 24 hours and ended/inactive sessions after seven days; deletion from external backups depends on the operator's infrastructure. Local report, event database, and key files remain on the participant's device until the participant deletes them.

## Project license

Apache-2.0. Its explicit contributor patent license is useful for a systems project that may later integrate OS-specific interfaces, while remaining permissive for use in tools and event environments. See [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Keep research claims sourced and distinguish verified facts, inference, proposed design, and unresolved risk. Do not claim complete AI detection or resistance to a workstation administrator. The AI registry remains research data; TUF authenticates publication but cannot prove indicator truth or make an indicator enforcement policy.
