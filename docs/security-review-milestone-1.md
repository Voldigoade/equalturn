# Independent Milestone 1 security review

**Reviewed:** 19 October 2026 against `226c531fd4724a9fc726868055f43d946df381ce`.

This review examined the protocol, local persistence, relay transactions, report verifier, privacy boundaries, and attacker-controlled resource bounds as a security-sensitive implementation. Findings below describe code at the reviewed commit; remediations are tracked by the Milestone 1.5 stages. A cryptographically valid event authenticates a key's claim. It does not establish that a local client reported reality honestly or completely.

## Findings

| ID | Severity | Finding | Status / owner |
| --- | --- | --- | --- |
| M1-SEC-01 | Moderate | Windows participant identity files contain the raw 32-byte Ed25519 seed. Windows currently relies on inherited filesystem ACLs and does not validate them. Anyone who can read that file can impersonate the session participant. | Fix in the Windows DPAPI checkpoint. Preserve only an explicit, exact legacy-v1 migration path; a v2 protected-file failure must never fall back to plaintext. |
| M1-SEC-02 | Moderate | Unix participant-key loading checks `symlink_metadata` and permissions, then opens the path separately. The final-component check/open sequence is raceable; directory ownership is not checked; failed writes can leave a partial `create_new` file that prevents retry. | Fix in the Unix key-storage checkpoint with no-follow reads, ownership/mode validation, durability, and failure cleanup. Root and same-user code remain outside this protection. |
| M1-SEC-03 | Moderate | Relay identity persistence checks the final file but not the parent directory's ownership/mode, has a check/open race, does not sync the parent after creation, and a partial `read_exact` failure returns before explicitly zeroizing the stack seed buffer. | Fix in the relay identity checkpoint. Keep the local relay key file owner-only and document that this does not protect against root, account compromise, or storage rollback. |
| M1-SEC-04 | Low | `SigningIdentity::seed_bytes()` exposes a plain `[u8; 32]` copy. Current call sites manually zeroize mutable copies, but the type system does not enforce that lifecycle. The signing type itself has no `Debug` or serde implementation. | Reduce the raw-seed API surface and return a zeroizing wrapper in the secret-lifecycle checkpoint. This does not guarantee erasure of compiler/register/OS copies. |
| M1-SEC-05 | Moderate | `tools/validate_registry.py` confirms the schema's Draft 2020-12 declaration but does not run a JSON Schema implementation against the instance. Its hand-written checks can drift from the schema. | Replace with a real Draft 2020-12 validation path and semantic diagnostics in the registry validator checkpoint. |
| M1-SEC-06 | Moderate | `equalturn report verify` checked the pathname's length and then used an unbounded `fs::read`. A concurrent replacement/growth could exceed the report cap between those operations. | Fixed in this review checkpoint: open once, inspect the opened file, and read at most `limit + 1` bytes. Unit tests cover exact-limit acceptance and over-limit rejection. |

No other high-severity protocol, relay-membership, or report-verification defect was confirmed in the reviewed paths. Findings 01–05 remain explicit work items in the next focused stages, not accepted residual risks for Milestone 1.5 completion.

## Reviewed controls retained

- Participant events, reconnect challenges, heartbeats, relay receipts/notices, and session manifests use separate domain-separated fixed binary encodings. The wire JSON serialization is not treated as signable bytes. Golden vectors exist for the event encoding; Ed25519 verification uses the library's strict verifier.
- Participant keys are session-scoped and bound in the local key envelope to session, participant, and local-store identifiers. Unix key files use owner-only mode checks, although the path and ownership gaps above still require hardening.
- Event admission validates protocol version, IDs, payload bounds, participant membership, signature, sequence, previous hash, event uniqueness, and session capacity inside PostgreSQL transactions. Duplicate identical submissions return the existing receipt; conflicting reuse is rejected.
- Invitations have high entropy, are stored relay-side as a verifier, expire, are limited by participant/session lifecycle and join count, and have a bounded in-memory attempt limiter. Reconnect uses a short-lived random one-use challenge and connection generations to make replacement deterministic.
- HTTP bodies and WebSocket frames are capped at 32 KiB; per-session participants/events/evidence and live connection/fan-out resources have explicit limits. Reports have a 64 MiB parser cap and bounded item counts. The report CLI read cap was made race-resistant in this checkpoint.
- The report verifier checks participant keys, event signatures/chains, receipt/notice signatures and matching, manifest heads, ordering, duplicates, and gaps. It labels included-evidence consistency separately from relay identity anchoring and never emits a cheating verdict.
- Application evidence and structured logs do not include peer transport IPs, invitation codes, private seeds, request bodies, command lines, passwords, or arbitrary local inventories. The relay operator and infrastructure can still observe or retain transport metadata outside the evidence schema.
- PostgreSQL session/event/evidence ordering and participant heads are transactionally updated; event, receipt, session limits, and key metadata have database constraints. Relay restart/reconnect behavior is covered by live localhost integration tests.

## Verification evidence

- Baseline before the fix: `cargo test --workspace --locked` passed with a loopback-only PostgreSQL 17 test service: 49 tests, including both PostgreSQL integration scenarios.
- Regression test written first: `cargo test -p equalturn --bin equalturn commands::report::tests::bounded_reader_rejects_input_that_grows_past_the_limit --locked` failed to compile because the bounded reader did not exist.
- After implementation: `cargo test -p equalturn --bin equalturn commands::report::tests --locked` passed both bounded-reader tests. The full workspace, Clippy, and format checks are run again at this checkpoint before commit.
- The plan-only PR run `39276785506` passed Linux/PostgreSQL and Windows CI.
- After the report-read fix, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, `python3 tools/validate_registry.py`, and `TEST_DATABASE_URL=postgres://equalturn@127.0.0.1:55433/equalturn cargo test --workspace --locked` all passed. The workspace run reported 51 passing tests, including both live PostgreSQL integration tests.
