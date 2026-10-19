# Equalturn Milestone 1 implementation plan

**Status:** authorized for immediate execution by the project owner on 2026-10-19.

## Scope and invariants

Deliver a working, non-enforcing Rust client and relay for pseudonymous multi-participant sessions. No endpoint blocking, workstation inventory, browser or IDE monitoring, tray UI, privileged service, production hosting, or AI-use verdicts are in scope. Keep the Phase 0 trust boundary: participant keys authenticate claims, not workstation truth; the relay sees transport addresses but never places them in participant payloads or reports.

Lock Equalturn after the final preliminary name screen. Expand the provenance-backed registry in parallel with the protocol implementation; do not make registry data executable policy.

## Frozen protocol contract (v1)

The complete normative byte grammar, payload tags, field limits, event/receipt/liveness signatures, invite canonicalization and retry behavior, reconnect state machine, stable acceptance/replay rules, signed end manifest, offline-recovery semantics, and verifier outcomes are in [protocol-v1.md](protocol-v1.md). Signatures never cover ordinary JSON serialization. An independent protocol review is incorporated: reconnect responses are bound to socket-scoped one-use challenges; event persistence/order/receipt is atomic; reports detect missing suffixes relative to a relay-signed manifest but do not claim relay non-equivocation; missing local chain state is an explicit discontinuity.

## Architecture and interfaces

Workspace packages: `protocol` owns typed IDs, event/payload bounds, fixed binary encoders, Ed25519 verification, hash links, and receipts; `client-core` owns local key files, SQLite outbox, HTTP/WebSocket session operations and reconnect; `relay-core` owns Axum HTTP/WebSocket handlers, membership/lifecycle validation, PostgreSQL transactions, receipts and bounded fan-out; `report` owns JSON/Markdown materialization and offline verification. `apps/equalturn` and `apps/equalturn-relay` are thin CLI entry points. PostgreSQL is accessed through SQLx migrations and transactions. The local CLI defaults to explicit loopback-only HTTP; all non-loopback and production relay binds require TLS configuration.

No peer-to-peer transport or peer address field exists in protocol, client-facing types, or reports. The relay obtains transport peer addresses from the network stack only; they are neither logged nor persisted by application code. Use bounded request bodies, WebSocket frames, participant count, event payloads, session events, channels, and connection lifetimes.

## Execution order and test-first slices

1. Record final name evidence; freeze [protocol-v1.md](protocol-v1.md) and write golden-vector tests before implementation. Add a focused workspace/toolchain. Verify fixed bytes, version/size rejection, strict Ed25519 signatures, event hash chain, receipts and final-manifest verification.
2. Implement invitation/nickname/session pure logic and tests for entropy, normalization, expiry/consumption limits, uniqueness, and lifecycle authorization.
3. Add PostgreSQL migration and relay repository tests for transactional sequence advancement, idempotent resend, concurrent submissions, rollback, and membership checks. Add challenge/reconnect replay and replacement tests with an injectable clock.
4. Add client key-file handling and SQLite WAL outbox/received-event/receipt storage. Test reopen, atomic sequence allocation, ack removal, duplicate receipts, and corruption errors.
5. Implement real Axum HTTP and WebSocket relay paths and thin client CLI. Use real localhost TCP in integration coverage; no alternate simulated protocol path.
6. Implement report serialization, Markdown, and offline verification with tamper, gap, duplicate, malformed, and unsupported-version tests.
7. Add an automated two-client end-to-end test covering create, join, start, event fan-out, outage/offline queue, relay recovery/resend, receipts, end, export and verification. Run adversarial cases for malformed frames/messages, incorrect identities, boundary IDs/sequences, replay, and concurrent submissions.
8. Add CI with fmt, clippy warnings denied, workspace tests, PostgreSQL integration service, and Windows compilation/tests where the crate surface supports them. Document exact local commands and all behavior that tests establish.
9. Update Phase 0 docs to match implemented behavior, expand the registry only with traceable evidence, run final verification, commit focused changes, create/configure the public GitHub repository, and publish `main` through an available authorized GitHub path.

## Review focus

- Ensure client-facing and report schemas cannot expose network addresses or machine identifiers.
- Confirm no private signing key is stored in SQLite, logged, or transmitted.
- Ensure PostgreSQL event insert, per-participant sequence advancement, receipt allocation and session-state transition are atomic.
- Ensure duplicate event ID with identical content returns the original receipt, while conflicting content fails.
- Ensure missing client-side evidence is reported as a gap/unknown and never inferred clean.
- Inspect all dependency choices against current upstream documentation and lock versions through Cargo.

## Completion evidence

The milestone is complete only after actual `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, a real two-client flow through the relay, report verification and tamper rejection, `git status`, and observed GitHub repository/CI state where available. If a missing local PostgreSQL service or unavailable GitHub write path prevents a criterion, finish all independent work and report that exact blocker rather than simulating success.


## Observed completion evidence

Verification completed on 19 October 2026:

- `cargo fmt --all -- --check` passed.
- `python3 tools/validate_registry.py` passed: 127 entries, 130 provenance records, and 59 structured indicators.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed.
- `cargo test --workspace --locked` passed with PostgreSQL configured: 49 tests, including two real PostgreSQL-backed relay integration scenarios.
- `cargo build --workspace --locked` passed.
- `cargo audit --file Cargo.lock` passed after scanning 302 crate dependencies with no reported vulnerabilities.
- The documented CLI flow was exercised through create, join, start, synthetic event delivery, relay loss, durable local queue, relay recovery, resend, end, export, and offline verification.
- Initial GitHub Actions run 39269757339 passed Linux and identified a Windows SQLite file-lock assumption in the test. `ClientStore::close` now waits for SQLx pool shutdown; [follow-up run 39270515978](https://github.com/Voldigoade/equalturn/actions/runs/39270515978) passed both Windows tests and Linux/PostgreSQL checks on commit `89a2f1296783128654caab2e9ca1b7059f81efc9`.
- `gh repo create` created and pushed the public repository. The selected description, eight topics, `main` default branch, Issues/Wiki settings, secret scanning, push protection, Dependabot alerts, and private vulnerability reporting were read back successfully.
