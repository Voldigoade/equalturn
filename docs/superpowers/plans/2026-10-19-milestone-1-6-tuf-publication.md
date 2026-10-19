# Milestone 1.6 Trusted Registry Publication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish an operational, independently verifiable staging TUF publication and an explicit HTTPS registry update path, while keeping production trust fail-closed until its offline signer ceremony.

**Architecture:** Keep reviewed registry source and validation in Equalturn. Use a separately isolated TUF-on-CI staging repository for signed publication, with upstream workflows and no production keys. Extend the existing `equalturn-registry` crate with an HTTPS repository adapter and crash-safe installed-state boundary; route CLI commands through the selected trust profile and continue to use `sigstore-tuf` for all TUF verification.

**Tech Stack:** Existing Rust workspace and `sigstore-tuf` 0.15.0; TUF-on-CI current upstream template/actions; GitHub Pages; existing registry v3 generator/validators; Tokio/reqwest already present in the lockfile through `sigstore-tuf`.

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

- [ ] Initialize the actual repository with `tuf-on-ci-delegate sign/init` and non-production signers; do not handcraft repository metadata.
- [ ] Generate the target from an exact reviewed Equalturn commit and record source SHA, registry semantic version, target hash, and publication URL.
- [ ] Complete the signing event and publish through the template workflow; verify public root, timestamp, snapshot, targets metadata, target bytes, content types, and URLs independently.
- [ ] Verify the published target hash against a fresh deterministic local build and run the Rust verifier against the staging root.
- [ ] Record public key IDs/fingerprints only. Commit/push the ceremony evidence checkpoint.

### Task 5: HTTPS updater transport

**Files:**
- Modify: `Cargo.toml`, `Cargo.lock`, `crates/registry/Cargo.toml`, `crates/registry/src/trust.rs`, `crates/registry/src/lib.rs`
- Create: focused HTTP repository/client module and `crates/registry/tests/http_update.rs`

**Interfaces:**
- Consumes: `VerifiedRegistry`, bounded TUF verifier and `REGISTRY_TARGET_PATH`.
- Produces: `verify_remote_repository(base_url: &Url, bootstrap_root: &[u8], bootstrap_root_sha256: &[u8; 32], state_dir: &Path, now: Timestamp) -> Result<InstalledRegistry, RegistryError>` or a narrower equivalent using `sigstore_tuf::HttpRepository`.

- [ ] Add RED tests for non-HTTPS URLs, embedded credentials, query/fragment URLs, downgrade redirects, invalid target/hash, and bounded response handling.
- [ ] Use `sigstore-tuf`'s HTTP client with explicit connect/read/overall timeouts, redirects disabled, HTTPS-only validation, and current TUF per-role byte bounds.
- [ ] Run focused tests, then registry tests and Clippy; preserve local directory repository support for offline fixtures.
- [ ] Commit and push the transport checkpoint.

### Task 6: Atomic installed update state and cache recovery

**Files:**
- Create: focused registry update-store module.
- Modify: `crates/registry/src/trust.rs`, registry attack/integration tests.

**Interfaces:**
- Consumes: Task 5 updater and verified registry model.
- Produces: explicit update state with pinned-root digest, verified metadata, installed target bytes/digest, registry version, and last successful refresh.

- [ ] Add RED tests for cache rollback, corrupt metadata, missing/corrupt target, truncated/oversized files, symlink cache root/path, and failed interrupted replacement.
- [ ] Preserve the existing valid installed target if a refresh fails and its signed metadata remains currently valid.
- [ ] Use same-directory temporary files, fsync and atomic replacement; ensure TUF metadata-store best-effort writes cannot falsely report success.
- [ ] Verify restart behavior, cache tampering, and stable status mapping; commit/push the cache checkpoint.

### Task 7: Explicit staging/production trust profiles

**Files:**
- Modify: `apps/equalturn/src/cli.rs`, `apps/equalturn/src/commands/registry.rs`, `apps/equalturn/src/app.rs`.
- Create: profile/config module and CLI integration tests.

**Interfaces:**
- Consumes: Task 6 installed state and Task 2 production ceremony status.
- Produces: explicit `registry update`, `status`, `verify`, `inspect`, and `search` flows over verified installed state; production profile returns `production registry trust not initialized` until its pinned root exists.

- [ ] Add RED CLI tests for defaults, staging/production separation, no silent remote root discovery, offline search/status, and friendly error statuses.
- [ ] Implement explicit update URL/profile selection without coupling it to relay/session identity or background network activity.
- [ ] Report root/metadata versions, registry version, target digest, source URL, last successful refresh, and expiration state without raw TUF internals.
- [ ] Run CLI and full Rust tests; commit/push the CLI checkpoint.

### Task 8: Remote update, attack, and end-to-end tests

**Files:**
- Modify: registry and CLI integration tests; `apps/equalturn/tests/support/tuf.rs`.
- Create: a staging HTTPS smoke workflow with no signing credentials.

**Interfaces:**
- Consumes: Tasks 4–7 staging root, HTTP updater, store and CLI.
- Produces: repeatable local attack tests plus a live staging workflow that downloads and verifies published metadata and target bytes.

- [ ] Cover authenticated update, target tampering, signed rollback, expiry/freeze, unauthorized root, authorized root rotation, missing target, oversized metadata, cache corruption, and interrupted update.
- [ ] Add actual HTTPS end-to-end smoke against the published staging endpoint; mandatory PR CI remains deterministic and does not depend on external service availability.
- [ ] Record live root/targets rotation and short-expiry operational tests using TUF-on-CI signing events where staging tooling allows them.
- [ ] Verify source-to-target hash reproducibility and visible status on missing/stale service; commit/push the test checkpoint.

### Task 9: Release gates, recovery docs, and production preparation

**Files:**
- Create: `docs/registry-disaster-recovery.md`
- Modify: `README.md`, `CONTRIBUTING.md`, `docs/registry-trust.md`, `docs/registry-production-trust.md`, `docs/production-root-ceremony.md`, CI workflows.

**Interfaces:**
- Consumes: staging evidence and release interfaces from Tasks 4–8.
- Produces: release review gates, error/fallback semantics, source-to-target procedure, compromise/loss runbooks, and a production repository prepared without production private material.

- [ ] Require schema/semantic validation, warning review, deterministic target/hash, source revision, indicator actionability review, and signer threshold before signing.
- [ ] Document recovery for every TUF role, hosting/account compromise, erroneous indicators, expiry, and lost root quorum; correct errors by publishing a new semantic version.
- [ ] Ensure CI includes format, Clippy, Rust tests, registry validation, target reproducibility, TUF attacks, audit and Windows/Linux regression; keep permissions minimal.
- [ ] Add final explicit production fail-closed status and the exact offline signer handoff; commit/push the documentation/CI checkpoint.

### Task 10: Final branch review and integration

**Files:**
- Review full diff and repository settings for both Equalturn publication repositories.

**Interfaces:**
- Consumes: all task outputs.
- Produces: verified branch/PR, clean `main`, final Linux/PostgreSQL and Windows CI, and an evidence-based report.

- [ ] Run format, Clippy, full workspace tests, registry v2/v3 validation, deterministic generation, TUF attack tests, cargo-audit, Windows CI, Linux/PostgreSQL CI, and live staging smoke.
- [ ] Review trust boundaries, GitHub workflow permissions, staging/production separation, unsafe/panic/secret occurrences, and all TODO/FIXME/HACK occurrences.
- [ ] Open/update and merge the PR only after green branch CI; inspect final `main` CI and published staging after merge.
- [ ] Verify a clean final repository and report any external/environment limitation without overstating completion.
