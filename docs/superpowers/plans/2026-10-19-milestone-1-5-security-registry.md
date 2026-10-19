# Milestone 1.5 implementation plan

**Status:** executing on `work/milestone-1.5-registry-hardening` from reviewed Milestone 1 commit `226c531fd4724a9fc726868055f43d946df381ce`.

## Baseline capture

Observed before implementation on 19 October 2026:

- `main` and `origin/main` pointed to `226c531fd4724a9fc726868055f43d946df381ce`; the working tree was clean.
- Rust and Cargo were `1.99.1`; the repository pins the stable channel with `rustfmt` and Clippy.
- Workspace packages: `equalturn-protocol`, `equalturn-client-core`, `equalturn-relay-core`, `equalturn-report`, `equalturn`, and `equalturn-relay`.
- PostgreSQL migration: `crates/relay-core/migrations/0001_initial.sql`.
- CI has Linux/PostgreSQL and Windows jobs. The baseline GitHub Actions run `39271353939` completed successfully on this commit.
- Registry v2: 127 records, 130 source records, 59 structured indicators, and 18 categories; `python3 tools/validate_registry.py` passed.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed.
- `cargo test --workspace --locked` passed with a loopback-only PostgreSQL 17 container and `TEST_DATABASE_URL` configured: 49 tests, including both PostgreSQL relay integration tests. Running that command without sandbox network permission initially failed to connect to loopback; the same command passed with the explicitly authorized sandbox elevation. This was a sandbox restriction, not a repository failure.
- `cargo audit --file Cargo.lock` passed after the RustSec advisory database was initialized through the authorized elevated path; it scanned 302 locked dependencies and loaded 1,338 advisories.
- `gh auth status` reports the saved CLI token is invalid. The connected GitHub integration successfully read `Voldigoade/equalturn` and reports push/admin permissions; it will be used for branch commits and ref updates, without accessing browser credentials.

The native worktree tool placed its checkout outside the sandbox's writable roots. To keep edits within the authorized workspace, this task uses the named work branch in the existing checkout; `main` remains at the reviewed baseline until final integration.

## Review findings and invariants

The protocol and evidence model remain authoritative: participant signatures authenticate claims, not endpoint truth; relay signatures authenticate relay statements, not relay honesty or non-equivocation. Session privacy, bounded relay interfaces, and the non-enforcing milestone boundary remain unchanged.

The independent review found concrete hardening work allocated to later focused stages: Windows participant seeds are currently stored unprotected; Unix key-path checks have check/open race exposure and do not verify directory ownership; relay-key persistence does not validate parent-directory ownership/permissions or sync the parent; the registry validator checks hand-written constraints but does not actually apply its declared JSON Schema. These findings will be fixed and tested in their owning stages rather than folded into a broad refactor. No endpoint enforcement test will run on the development host.

## File and component map

- `crates/client-core/src/persistence.rs` and a small platform key-protection module: versioned participant-key envelope, DPAPI on Windows, strict owner-only Unix storage, and safe legacy upgrade behavior.
- `crates/relay-core/src/identity.rs`: relay signing-key path and atomic persistence hardening.
- `crates/registry/` (workspace crate): typed v3 model, bounds, semantic validation, deterministic release serialization, and read-only lookup/inspection APIs.
- `registry/registry-v3.json`, `registry/schema-v3.schema.json`, and `tools/validate_registry.py`: reviewed research snapshot, Draft 2020-12 validation, provenance checks, and deterministic diagnostics.
- `crates/registry` update module plus `apps/equalturn/src/commands/registry.rs`: mature TUF client integration and offline `verify`, `inspect`, and `search`; this code will not be connected to enforcement or silently fetch updates.
- `.github/workflows/ci.yml`, registry fixtures/tests, `docs/registry-trust.md`, `docs/enforcement-test-safety.md`, security review notes, and contributor/architecture/privacy documentation.

## Ordered stages and checkpoint gates

Every stage below has a focused verification, self-review, one or more coherent commits, a remote branch update, and observed CI when the stage changes CI behavior. Stages do not move forward on a known failure.

1. **Independent M1 security review.** Trace protocol crypto, event admission, reconnect, persistence, relay transactions, report verification, privacy, and resource limits. Record findings with severity, evidence, tests, and owner stage. Fix any issue that cannot safely be deferred. Commit/push the review record.
2. **Windows key protection.** Use the maintained Microsoft `windows` Rust bindings for current-user DPAPI with UI forbidden and no machine scope. Add a versioned envelope binding scheme, session, participant, local store, and bounded protected payload; bind DPAPI optional entropy to those identifiers. Migrate only the exact recognized v1 legacy envelope, atomically, and never treat a failed v2 unprotect as a legacy key. Add real Windows tests for persist/reopen, raw-seed absence, corruption, truncation, and bindings. Observe Windows CI before continuing.
3. **Unix key storage.** Add safe no-follow file opens, owner checks, strict directory/file modes, bounded reads, create-new writes, file and parent-directory sync, and failed-write cleanup. Test symlinks, non-regular files, wrong bindings, truncation, oversized files, and reopen. Document root/same-user/memory limitations.
4. **Relay identity storage.** Enforce a private owned parent directory, no-follow regular-file reads, exact file size, create-new behavior, zeroized temporary seeds, durable parent sync, and no replacement of an existing key. Test collision, symlink, permission, truncated, oversized, and repeated-start paths.
5. **Secret lifecycle.** Review all seed/challenge/invitation handling, reduce raw-seed API surface, use zeroizing containers for temporary copies, and verify no sensitive `Debug`, logs, panic output, or persistence fields expose secrets.
6. **Registry quality audit.** Audit every record/indicator and source for duplicates, stale/dead links, product/provider conflation, host scope, shared infrastructure, and unsupported claims. Fix or downgrade only evidence-backed entries; publish counts and questionable-record dispositions.
7. **Registry schema v3.** Define strict typed enums and migration from v2. Indicators carry kind, normalized value, platform/surface/scope, provenance, verification date, confidence, actionability, false-positive risk, lifecycle, and limitations. Encode contradictions as semantic errors; keep research separate from enforcement.
8. **Indicator research.** Expand only where first-party or upstream sources establish specific values. Add meaningful indicators for hosted/API, coding/IDE/extension/CLI, gateways, local runtimes/servers, and security workflows; every value gets provenance and scope caveats. Avoid generic process names, shared hosts, and universal/custom endpoint claims.
9. **Semantic validation.** Apply a real Draft 2020-12 validator and deterministic semantic checks, with `ERROR`, `WARNING`, and `INFO`. Cover enums, unique IDs/values, relationships, source resolution, dates, hostname/wildcard/CIDR/port/package syntax, confidence/actionability/risk consistency, bounds, and canonical ordering. Add the required negative fixture corpus and keep warnings visible.
10. **TUF implementation research and decision.** Compare current upstream `tuf-on-ci`, Amazon `tough`, the TUF project's Rust client, and a mature non-Rust reference client where relevant. Evaluate spec support, tuf-on-ci interoperability, root rotation, expiry/rollback state, offline repositories, maintenance, and dependencies. Do not use a client with a known interoperability flaw or replace it with custom TUF cryptography. Freeze the selected library and publication design in `docs/registry-trust.md` before implementation.
11. **Registry repository/publication layout.** Keep `Voldigoade/equalturn/registry/registry-v3.json` as reviewed source. Because TUF-on-CI signing and Pages workflows need write/deploy permissions that do not belong in application CI, a separate public publication repository is justified and was created from the upstream template: `Voldigoade/equalturn-registry` (public, `main`, initial template commit `50d8f3d`). Its README records that it has no Equalturn root, keys, metadata, target, Pages deployment, or signer configuration yet. Generated `registry-v3.json` will live at `targets/registry-v3.json`; role metadata lives in `metadata/`; the first public root must be pinned out of band in an Equalturn release. Pages and signing permissions remain deferred until the threshold root ceremony. Reviewed template workflows declare no workflow-wide permissions and pin Actions by commit; production readiness still requires review of `id-token:write` / contents write in the online signer job and protection of `main`.
12. **Trust model and key roles.** Proposed policy: root 2-of-3 offline; top-level targets 2-of-3 on distinct keys; snapshot and timestamp each on separate OIDC-restricted KMS keys; expiry 2 years / 180 days / 30 days / 7 days respectively. The actual custodians and KMS provider remain unconfigured pending a real signing ceremony; no key material is created or committed. Document compromise/loss/recovery, expiry, rollback/freeze, GitHub and mirror risks, and that TUF authenticates publication rather than indicator correctness.
13. **Deterministic target.** `tools/build_registry_release.py` emits bounded, UTF-8/NFC, canonical-key-order `dist/registry-v3.json` from validated source; the fixed publication name is `targets/registry-v3.json`. It preserves semantically canonical array order, adds no timestamps/signature, refuses to replace the source, and exposes `--check`. Two independent builds on the current 131-entry/84-indicator source matched at 152,438 bytes with SHA-256 `e6ccf57eca33951cd6b7a04e7e609f981c87c73298b7923b4d5a9a845caa02a0`; generated bytes passed v3 validation.
14. **Registry library and trusted update client.** Add the focused Rust crate. Parse bounded v3 data using the embedded Draft 2020-12 schema plus critical semantic checks; expose typed searches/inspection; verify repository metadata and target hashes through `sigstore-tuf`; and persist the version floor with bounded, atomic, synced cache writes. The signing ceremony has not produced an Equalturn root, so the adapter accepts only caller-supplied root bytes plus an independently obtained SHA-256 pin; it never fetches its initial root. The release must embed the resulting public root after the ceremony. Support local/offline repositories and reject unsafe target paths and oversized data.
15. **TUF attack tests.** Use ephemeral test keys and local repositories to prove valid update, target tampering, rollback, expiry/freeze, authorized root rotation, unauthorized root rejection, mix-and-match rejection where the library supports it, and malformed metadata failure. An attacker-supplied root plus metadata must not replace the compiled/pinned root.
16. **Offline registry CLI.** Add local `verify`, `inspect`, and `search` commands with stable user-facing statuses and bounded input; show provenance, actionability, confidence, false-positive risk, and limitations. No silent phone-home or enforcement decision.
17. **Adversarial registry tests.** Cover traversal/absolute targets, symlinks, size/depth/count limits, duplicate keys/IDs, malformed UTF-8/JSON/metadata/signatures, NUL/Unicode edge cases, stale metadata, and arbitrary mirror content.
18. **CI hardening.** Preserve Linux/PostgreSQL and Windows. Run format, Clippy, all tests, DPAPI tests, registry schema/semantic checks, deterministic target checks, TUF attacks, and cargo audit. Keep job/token permissions minimal, pin Actions, and ensure CI never modifies its network control plane.
19. **Documentation and contributor safety.** Update product/architecture/privacy/registry docs and exact workflow commands. The permanent [enforcement-test-safety.md](../../enforcement-test-safety.md) policy is linked from `AGENTS.md`, `CONTRIBUTING.md`, this plan, and the architecture/matrix docs. Future Linux enforcement tests require namespaces/VMs; Windows requires Sandbox/VM where the primitive is represented, otherwise a disposable VM. Isolation unavailable means skip and report; never fall back to host enforcement.
20. **Full review and integration.** Review all branch diff and sensitive-code searches; run every final local check including PostgreSQL and `cargo audit`; inspect Linux and Windows Actions for the branch; only then fast-forward `main` to the verified commit, inspect final main CI, and confirm clean branch/workspace state.

## Checkpoint notes

### Stage 15 — TUF attack tests

The tests build temporary repositories with ephemeral P-256 test keys and exercise the production `sigstore-tuf` verifier through its local-directory transport. They cover a valid update, target-byte tampering, cached rollback, expired timestamp metadata at an injected time, dual-authorized root rotation, unauthorized rotation, snapshot mix-and-match, malformed metadata, bootstrap-root substitution, semantically invalid signed target data, and recovery from corrupted cached metadata. No private test key is persisted outside the test process or committed.

Focused verification observed before the Stage 15 checkpoint:

- `CARGO_BUILD_JOBS=1 cargo test -p equalturn-registry --test tuf_attacks --offline` — 11 passed.
- `CARGO_BUILD_JOBS=1 cargo test -p equalturn-registry --offline` — 18 passed across unit, registry integration, and TUF attack tests.
- `CARGO_BUILD_JOBS=1 cargo clippy -p equalturn-registry --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed after the final test edits.

The bootstrap substitution test keeps the independently pinned bootstrap root and presents a repository fully signed by an attacker key; it is rejected by the actual TUF verifier. A separate earlier registry test covers a root whose supplied bytes fail the independent bootstrap digest pin.

### Stage 16 — offline registry CLI

Added `equalturn registry verify`, `inspect`, and `search`. Each command reads a local TUF directory repository and requires a caller-supplied root file plus independent lowercase SHA-256 pin. Each re-verifies the repository before exposing registry entries; there is no relay requirement, HTTP transport, or default production root. The persistent metadata cache defaults below the client data directory and can be overridden with `--cache-dir`. Inspection includes product/provider identity, categories, surfaces, status, indicator values, platforms, provenance, confidence, actionability, false-positive risk, verification date, and limitations. Stable statuses distinguish valid data, expiry, rollback, signature, root, target, registry, metadata, and repository failures.

Focused verification observed before the Stage 16 checkpoint:

- `CARGO_BUILD_JOBS=1 cargo test -p equalturn -p equalturn-registry --offline` — 22 passed across both packages (2 application unit tests, 2 CLI end-to-end tests, 1 registry unit test, 6 registry tests, and 11 TUF attack tests).
- `CARGO_BUILD_JOBS=1 cargo clippy -p equalturn-registry -p equalturn --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `CARGO_BUILD_JOBS=1 cargo run -p equalturn --offline -- registry --help` and `target/debug/equalturn registry verify --help` — command tree and required trust arguments observed.

The CLI end-to-end test generates a process-local ephemeral TUF repository and verifies successful `verify`, `inspect`, and `search` output without relay configuration. A separate invocation supplies a mismatched root pin and observes `INVALID_ROOT` plus a failing exit status. Test keys and repositories remain in temporary directories.

### Recovery and Stage 17 — adversarial registry inputs

After the development application was terminated, the checkout was inspected before changes. The working branch was `work/milestone-1.5-registry-hardening`, HEAD and the fetched remote branch both pointed to `3a08e86cd5eaae719a02dd5a2c1b55797312cd95`, and the worktree contained only the in-progress untracked adversarial registry test file. The local branch has no upstream tracking configuration, so the remote branch was compared by its explicit ref. Stage 3 itself was already committed as `350dc5a` and pushed; no Stage 3 edits were recreated or overwritten. Focused Unix persistence verification observed 28 passing tests. The `equalturn-key-protection` crate compiled and its test target ran, but it contains no crate-local tests; real DPAPI coverage remains on Windows CI.

Stage 17 adds adversarial parser, filesystem repository, and real TUF verifier coverage. Registry parsing now rejects C0/C1 control characters in every string and object key before schema decoding, preventing terminal control injection while allowing ordinary Unicode product labels. Tests cover path traversal and absolute target names, static repository-root/area/metadata/target symlinks on Unix, requested target-size limits, deep JSON, raw NUL, terminal controls, hostname homographs, Unicode product names, schema string/entry bounds, malformed UTF-8 and duplicate JSON keys, malformed and missing TUF metadata, malformed pinned roots and key material, invalid signatures, expired/rolled-back cached metadata, untrusted mirror content, cache corruption, and oversized signed targets. There is no compressed repository format, so decompression-bomb handling is not applicable.

Focused verification observed for Stage 17:

- `CARGO_BUILD_JOBS=1 cargo test -p equalturn-registry --all-targets --all-features --offline` — 34 passed across 6 unit tests, 6 adversarial parser tests, 6 registry tests, and 16 TUF attack tests.
- `CARGO_BUILD_JOBS=1 cargo clippy -p equalturn-registry --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.

### Stage 18 — CI hardening

The existing CI already uses full-SHA-pinned third-party actions and workflow-level `contents: read`; Linux runs dependency audit, v2/v3 registry validation, Python validator tests, Clippy, and the workspace/PostgreSQL suite, while Windows runs the full workspace suite including the real DPAPI identity tests. Added an explicit deterministic target check that writes one build under `$RUNNER_TEMP` and compares a second build against it. This avoids relying on `dist/`, which is intentionally ignored and absent in a fresh checkout. No workflow write permissions or host-network enforcement were added.

Verification observed on pushed commit `47a9ac1f1b556e68cc75cc6d8271bc89b52b77e3`:

- GitHub Actions run [39308964800](https://github.com/Voldigoade/equalturn/actions/runs/39308964800) — Linux/PostgreSQL passed, including the new deterministic target step; Windows workspace build/tests passed.
- `git diff --check` — passed before commit.
- The deterministic builder could not be run in the local environment because its Python 3.14 installation has neither `pip` nor `venv`, and the bundled Python lacks `jsonschema`. The exact two-build command ran successfully in the GitHub Linux job after installing the pinned `requirements-registry.txt` dependencies.

### Stage 19 — documentation and contributor safety

Updated the README, product brief, architecture, privacy, registry, repository, roadmap, and security-review docs to distinguish current M1/M1.5 behavior from future enforcement. The registry CLI is documented as explicit, directory-only, and rooted in caller-supplied trust; the session client/relay do not apply registry indicators. The publication repository still has no Equalturn root or signed release. Contributor guidance now covers source provenance, confidence/actionability/false-positive reasoning, validation, deterministic target generation, review, and the disabled signing ceremony.

Added [the permanent enforcement-test safety policy](../../enforcement-test-safety.md) and linked it from `AGENTS.md`, `CONTRIBUTING.md`, the implementation plan, architecture, roadmap, and enforcement matrix. It prohibits experimental enforcement on the active development host; requires Linux VM/network-namespace or appropriate Windows Sandbox/VM isolation; and requires dangerous tests to be skipped and reported when safe isolation is unavailable. It explicitly states these are developer/CI safeguards, not participant anti-bypass controls. Added `.venv/` to `.gitignore` for the documented isolated registry Python environment and corrected a stale key-storage documentation link.

Stage 19 review observed:

- A local Markdown-link audit over 29 files — passed after fixing the stale key-storage link.
- Search for machine-local absolute paths in Markdown, `git diff --check` — passed.
- No enforcement, network, or firewall tests were run.

## Completion evidence

No stage is complete because it compiles. Final acceptance requires the user's complete Milestone 1.5 checklist: baseline behavior preserved; key protections and storage invariants; registry v3/schema/semantic validation and provenance; deterministic target; tested mature TUF bootstrap/update/attacks; offline CLI; no enforcement coupling; no secrets; fmt/clippy/full PostgreSQL tests/registry validation/TUF tests/audit; observed Linux and Windows CI and final main CI; clean committed repository. Any unavailable external service or key-ceremony requirement must be identified precisely after all independent implementation work is complete.
