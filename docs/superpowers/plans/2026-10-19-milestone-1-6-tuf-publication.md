# Milestone 1.6 Trusted Registry Publication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish an operational, independently verifiable staging TUF publication and an explicit HTTPS registry update path, while keeping production trust fail-closed until its offline signer ceremony.

**Architecture:** Keep reviewed registry source and validation in Equalturn. Use a separately isolated staging repository with standard TUF keys and a fixed-content GitHub Pages deployment workflow; the deploy job does not sign metadata. Extend `equalturn-registry` with bounded HTTPS transport and crash-safe installed state, keep profile selection explicit, and use `sigstore-tuf` for all verification. Production remains fail-closed until its independent offline root ceremony.

**Tech Stack:** Existing Rust workspace and `sigstore-tuf` 0.15.0; `tuftool` 0.17.0 for one-time standard-key staging metadata; fixed-content GitHub Pages workflow; existing registry v3 generator/validators; Tokio/reqwest already present in the lockfile through `sigstore-tuf`. TUF-on-CI was evaluated but its staging signer path was not selected after interoperability/configuration failures.

**Spec:** User-supplied Milestone 1.6 mission brief; `docs/registry-trust.md`; `docs/enforcement-test-safety.md`.

## Global Constraints

- Never create or expose a production root private key in this agent/cloud environment.
- Do not modify host firewall, WFP, nftables, iptables, DNS, routing, proxy, VPN, WDAC, AppLocker, or login/session policy.
- Do not reduce test coverage to reduce memory use; run local Cargo builds with `CARGO_BUILD_JOBS=1`.
- Do not use staging keys as production keys or let a production build silently fall back to staging trust.
- Production registry update must fail closed until a separately authenticated production root is initialized.
- Keep registry updates explicit and independent from competition session identity or telemetry.
- Use TUF as the update protocol; do not add custom signing, root discovery, or HTTP downgrade behavior.
- Do not publish participant/session data, local paths, usernames, or machine identity in registry provenance.
- Treat any research registry target as data, never as enforcement policy.

## Review Focus

- Redirects from the configured HTTPS origin to HTTP or another host must be rejected without fetching the redirected target.
- Cache rollback, symlink replacement, interrupted writes, and stale metadata must not replace previously trusted registry state.
- A staging root or mirror must never initialize or replace production trust implicitly.
- A trusted but malformed/oversized registry target must fail semantic validation before installation or display.
- Offline, TLS, expiry, and target-fetch errors must preserve a still-valid installed registry and give stable user-facing status.

---

### Task 1: Baseline and upstream TUF review

**Files:**
- Create: `docs/research/tuf-publication-current-review.md`
- Modify: `docs/registry-trust.md`
- Record: Milestone 1.6 implementation notes in `docs/registry-publication-status.md`

**Interfaces:**
- Consumes: Milestone 1.5 local TUF verifier, registry v3 target builder, current TUF-on-CI publication repository.
- Produces: a dated upstream/version review and accepted publication/client boundaries for Tasks 2–10.

- [x] Record the clean baseline commit, workspace/toolchain, local test results, CI-required PostgreSQL limitation, and unavailable local `cargo-audit` network installation.
- [x] Verify current TUF spec, TUF-on-CI release/workflows, `sigstore-tuf` API/features/limits, and GitHub Pages setup against upstream sources.
- [x] Update trust design where current upstream behavior differs from Milestone 1.5 assumptions.
- [x] Run baseline registry validation and workspace verification again before implementation.
- [x] Commit and push the research/status checkpoint; inspect CI if CI files changed. (commit `3b122d2`, pushed; no CI trigger for documentation-only branch push.)

### Task 2: Production trust policy and manual ceremony gate

**Files:**
- Create: `docs/registry-production-trust.md`
- Create: `docs/production-root-ceremony.md`
- Modify: `docs/registry-trust.md`, `docs/registry-publication-status.md`

**Interfaces:**
- Consumes: Task 1 upstream review.
- Produces: explicit non-production/production role policy, key custody, expiration, recovery, and fail-closed bootstrap decisions.

- [x] Document an honest root/targets threshold based on actual independent maintainers; do not claim independent custody where it does not exist.
- [x] Choose separate staging-only signers and document online signer options and risks without provisioning paid infrastructure.
- [x] Specify production publication URL, initial-root out-of-band verification, role expiry/rotation, incident response, and the exact manual signer gate.
- [x] Verify no instruction or artifact includes private production material; review policy against the Mission's root-custody boundary.
- [x] Commit and push the policy checkpoint. (commit `3e775b5`, pushed.)

### Task 3: Isolated staging TUF repository

**Files:**
- GitHub repository: `Voldigoade/equalturn-registry-staging` if creation remains necessary and available.
- Modify staging repository only from the current upstream TUF-on-CI template.
- Modify: `docs/registry-publication-status.md`, `docs/registry-production-trust.md`.

**Interfaces:**
- Consumes: Task 2 role and staging policy.
- Produces: an isolated non-production publication origin, pinned workflow revisions, Pages `github-pages` environment restricted to `publish`, and inspectable repository settings.

- [x] Create the staging repository from the current TUF-on-CI template; retain clear NON-PRODUCTION labeling.
- [x] Configure Pages, environment branch policy, Actions token defaults, workflow permissions, and pinned actions according to current upstream guidance and least privilege.
- [x] Record settings that cannot be inspected through authorized GitHub tooling; do not add a long-lived PAT or production secret.
- [x] Commit/push the README-only staging adjustment (`a7d062d`). It matches no workflow trigger, so no workflow run occurred; initialization/publication remains Task 4.

### Task 4: Staging root ceremony and first target

**Files:**
- Staging repository: TUF-on-CI signing-event metadata and first target, generated only through supported tooling.
- Equalturn: `tools/build_registry_release.py`, `docs/registry-publication-status.md`.

**Interfaces:**
- Consumes: Task 3 isolated repository and Task 2 non-production signer policy.
- Produces: a staging root, signed role metadata, deterministic `registry-v3.json` target tied to an Equalturn source revision, and a public staging endpoint.

- [x] Initialize the staging repository with separate standard RSA-PSS-SHA256 role keys using `tuftool` 0.17.0. The previous OIDC state remains public history; it is not the current trust anchor.
- [x] Generate the target from the reviewed registry source at commit `5c46bc6d59a69b5417a3c571a45c8f15529e26ef`; record semantic version, SHA-256, and publication URL in the registry status docs.
- [x] Publish fixed metadata/target bytes through the restricted GitHub Pages workflow; independently fetch the public root and target and verify the HTTPS publication.
- [x] Rebuild the source target deterministically and run the Rust verifier using the pinned staging root; live HTTPS install and offline reopen pass.
- [x] Record public root/target digests only and document the non-production key-custody limitation.

**Deviation:** TUF-on-CI's standard-key publish action rejected its expiry configuration (`x-tuf-on-ci-expiry-period`, staging run `39331918162`), while its earlier OIDC/TAP-18 root was incompatible with the Rust verifier. Staging now serves a one-time standard-key repository from a fixed allowlist on a branch-restricted Pages workflow. This is a deploy path, not an automated signer; no cryptography was replaced or bypassed. Production publication still requires a separately reviewed signer architecture.

### Task 5: HTTPS updater transport

**Files:**
- Modify: `Cargo.toml`, `Cargo.lock`, `crates/registry/Cargo.toml`, `crates/registry/src/trust.rs`, `crates/registry/src/lib.rs`
- Create: focused HTTP repository/client module and `crates/registry/tests/http_update.rs`

**Interfaces:**
- Consumes: `VerifiedRegistry`, bounded TUF verifier and `REGISTRY_TARGET_PATH`.
- Produces: `verify_remote_repository(base_url: &Url, bootstrap_root: &[u8], bootstrap_root_sha256: &[u8; 32], state_dir: &Path, now: Timestamp) -> Result<InstalledRegistry, RegistryError>` or a narrower equivalent using `sigstore_tuf::HttpRepository`.

- [x] Add RED tests for non-HTTPS URLs, embedded credentials, query/fragment URLs, downgrade redirects, invalid target/hash, and bounded response handling. Redirect and both advertised/streamed size limits are covered by real loopback HTTP tests; existing TUF attack tests cover bad target hash.
- [x] Use `sigstore-tuf`'s HTTP client with explicit connect/read/overall timeouts, redirects disabled, HTTPS-only validation, and current TUF per-role byte bounds.
- [x] Run focused tests, registry tests and Clippy; preserve local directory repository support for offline fixtures.
- [x] Commit and push the transport checkpoint (`1d90053`).

### Task 6: Atomic installed update state and cache recovery

**Files:**
- Create: focused registry update-store module.
- Modify: `crates/registry/src/trust.rs`, registry attack/integration tests.

**Interfaces:**
- Consumes: Task 5 updater and verified registry model.
- Produces: explicit update state with pinned-root digest, verified metadata, installed target bytes/digest, registry version, and last successful refresh.

- [x] Add RED tests for installed rollback/target tampering, missing/corrupt pointer, symlink store/pointer/metadata paths, and failed candidate replacement.
- [x] Preserve the existing valid installed target if a refresh fails and its signed metadata remains currently valid.
- [x] Use temporary same-filesystem snapshots, fsync and atomic pointer replacement; re-open the installed candidate through TUF so metadata-store best-effort writes cannot falsely report success.
- [x] Verify restart behavior, consistent-snapshot names, authorized root rotation replay, cache-floor behavior, and tampering; focused tests and Clippy pass. (commit `9e4b117`, pushed)

### Task 7: Explicit staging/production trust profiles

**Files:**
- Modify: `apps/equalturn/src/cli.rs`, `apps/equalturn/src/commands/registry.rs`, `apps/equalturn/src/app.rs`.
- Create: profile/config module and CLI integration tests.

**Interfaces:**
- Consumes: Task 6 installed state and Task 2 production ceremony status.
- Produces: explicit `registry update`, `status`, `verify`, `inspect`, and `search` flows over verified installed state; production profile returns `production registry trust not initialized` until its pinned root exists.

- [x] Add RED CLI tests for staging/production separation, no silent remote root discovery, offline search/status, and friendly verification statuses. (4 CLI tests pass.)
- [x] Implement explicit update URL/profile selection without coupling it to relay/session identity or background network activity; production remains fail-closed.
- [x] Report root/metadata versions, registry version, target digest, locally recorded source and refresh time, and current expiry status without raw TUF internals. Signed metadata versions are checked against the installed TUF snapshot.
- [x] Run focused CLI/registry tests, Clippy and formatting. Registry tests: 49 passed, 1 live smoke test ignored pending compatible staging trust.
- [x] Review, commit and push the CLI checkpoint (`d3ef025`). No CI workflow was changed, and branch-only pushes do not trigger the existing workflow.

### Task 8: Remote update, attack, and end-to-end tests

**Files:**
- Modify: registry and CLI integration tests; `apps/equalturn/tests/support/tuf.rs`.
- Create: a staging HTTPS smoke workflow with no signing credentials.

**Interfaces:**
- Consumes: Tasks 4–7 staging root, HTTP updater, store and CLI.
- Produces: repeatable local attack tests plus a live staging workflow that downloads and verifies published metadata and target bytes.

- [x] Cover authenticated update, target tampering, signed rollback, expiry/freeze, unauthorized root, authorized root rotation, missing metadata, bounded/oversized target, mix-and-match, cache corruption, and malformed metadata in deterministic registry tests (23 TUF attack tests pass).
- [x] Add an actual HTTPS staging smoke test that installs the published repository and reopens the local state offline; mandatory PR CI remains deterministic and does not depend on Pages availability.
- [x] Live-signer root/targets rotation and short-expiry renewal were evaluated and intentionally deferred: staging metadata uses disposable local key material and the static deploy workflow has no signer. Deterministic Rust attack tests cover authorized/unauthorized root rotation and expiry without waiting; no live signer or production key was introduced.
- [x] Rebuild target bytes from source commit `5c46bc6d59a69b5417a3c571a45c8f15529e26ef` and compare the public target digest; CLI tests and the live test cover stable status/failure behavior. Registry crate: 50 tests pass with the live smoke ignored by default; explicit live smoke passes 1/1; focused Clippy is warning-free. The Windows fixture issue was fixed and follow-up branch CI passed Linux/PostgreSQL and Windows (run `39346243590`).

### Task 9: Release gates, recovery docs, and production preparation

**Files:**
- Create: `docs/registry-disaster-recovery.md`
- Modify: `README.md`, `CONTRIBUTING.md`, `docs/registry-trust.md`, `docs/registry-production-trust.md`, `docs/production-root-ceremony.md`, CI workflows.

**Interfaces:**
- Consumes: staging evidence and release interfaces from Tasks 4–8.
- Produces: release review gates, error/fallback semantics, source-to-target procedure, compromise/loss runbooks, and a production repository prepared without production private material.

- [x] Require schema/semantic validation, warning review, deterministic target/hash, source revision, indicator actionability review, and signer threshold before signing; documented in `CONTRIBUTING.md` and the TUF release gate.
- [x] Document recovery for every TUF role, hosting/account compromise, erroneous indicators, expiry, and lost root quorum; correct errors by publishing a new semantic version. See `docs/registry-disaster-recovery.md`.
- [x] Ensure CI includes format, Clippy, Rust tests, registry validation, target reproducibility, TUF attacks, audit and Windows/Linux regression; keep permissions minimal. The branch run `39346243590` passed.
- [x] Add final explicit production fail-closed status and signer handoff; commit/push the documentation checkpoint after focused review and verification.

### Task 10: Final branch review and integration

**Files:**
- Review full diff and repository settings for both Equalturn publication repositories.

**Interfaces:**
- Consumes: all task outputs.
- Produces: verified branch/PR, clean `main`, final Linux/PostgreSQL and Windows CI, and an evidence-based report.

- [x] Run format, Clippy, full workspace tests, registry v2/v3 validation, deterministic generation, TUF attack tests, cargo-audit, Windows CI, Linux/PostgreSQL CI, and live staging smoke. Local results and successful branch CI run `39348415799` are recorded in `docs/registry-publication-status.md`.
- [x] Review trust boundaries, GitHub workflow permissions, staging/production separation, unsafe/panic/secret occurrences, and all TODO/FIXME/HACK occurrences. Production request paths in the reviewed registry boundary do not use `unwrap`/`expect`; occurrences are in tests, and no unfinished TODO/FIXME/HACK implementation marker was found.
- [ ] Open/update and merge the PR only after green branch CI; inspect final `main` CI and published staging after merge.
- [ ] Verify a clean final repository and report any external/environment limitation without overstating completion.
