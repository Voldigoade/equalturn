# Equalturn

Equalturn is a privacy-conscious integrity session system for people who mutually agree to compete without AI assistance. It gives participants pseudonymous session identities and a shared, signed record of events accepted by a relay.

**Status: Milestone 1 implemented; local verification and Linux/Windows GitHub Actions passed on 19 October 2026.** The Rust workspace contains a CLI client, a PostgreSQL-backed relay, a versioned signed-event protocol, a durable local event queue, and an offline report verifier. This milestone is non-enforcing: it accepts synthetic events and session/liveness claims only. It does not inspect workstation activity, block AI services or applications, or include a tray UI. Do not use its event log as proof that prohibited activity did or did not occur.

Participant signatures prove which session key made a claim and whether the included event chain was changed. Relay signatures attest to event receipt/order and to the final manifest. Neither proves that a client observed reality honestly, that an event was not omitted before signing, or that the relay did not withhold evidence. A workstation administrator, second device, virtual machine, remote host, or compromised client remains outside the guarantees of this milestone.

## Start here

- [Product brief](docs/product-brief.md)
- [Implemented and proposed architecture](docs/architecture.md)
- [Protocol v1 contract](docs/protocol-v1.md)
- [Threat model](docs/threat-model.md)
- [Windows and Linux enforcement research](docs/enforcement-matrix.md) — future work; no enforcement is implemented
- [AI ecosystem and registry design](docs/ai-registry.md)
- [Researched registry v2](registry/registry-v2.json) — research data only
- [Privacy and report model](docs/privacy.md)
- [Prior art](docs/prior-art.md)
- [Stack decision](docs/stack-decision.md)
- [Name research](docs/naming.md)
- [Implementation roadmap](docs/implementation-roadmap.md)
- [Milestone 1 implementation plan and verification evidence](docs/milestone-one-implementation-plan.md)
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

The protocol, limits, signatures, reconnect behavior, retention rules, and known limitations are specified in [protocol v1](docs/protocol-v1.md). The initial AI catalogue is separate from enforcement and is not loaded by the client or relay.

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
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
```

## Privacy and trust boundary

Participant-visible status and reports contain session IDs, nicknames, session-scoped public keys/fingerprints, client version and coarse self-reported OS family, signed events, relay receipts/notices, and liveness/session state. They do not contain peer IP addresses. The relay necessarily sees source addresses at the transport layer; hosting, reverse-proxy, and infrastructure logs may retain connection metadata outside the application report, so relay operators must define and publish their own access and retention policy.

The client and relay do not collect keystrokes, clipboard contents, prompts, chat messages, passwords, tokens, cookies, screenshots, browsing history, command-line arguments, process inventories, filesystem inventories, or arbitrary URLs. Relay persistence has primary-database cleanup for unstarted sessions after 24 hours and ended/inactive sessions after seven days; deletion from external backups depends on the operator's infrastructure. Local report, event database, and key files remain on the participant's device until the participant deletes them.

## Project license

Apache-2.0. Its explicit contributor patent license is useful for a systems project that may later integrate OS-specific interfaces, while remaining permissive for use in tools and event environments. See [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Keep research claims sourced and distinguish verified facts, inference, proposed design, and unresolved risk. Do not claim complete AI detection or resistance to a workstation administrator. The AI registry remains research data until separately reviewed; it is not enforcement policy.
