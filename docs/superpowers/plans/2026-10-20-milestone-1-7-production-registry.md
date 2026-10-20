# Milestone 1.7 implementation plan

## Goal

Complete the safe, reproducible production-registry trust path while keeping production signing custody with the human owner and keeping all endpoint enforcement out of scope.

## Architecture

Continue using the existing Equalturn registry client (`sigstore-tuf`), embedded production bootstrap root and fixed canonical production origin once human custody is complete, deterministic registry v3 target, and the existing `Voldigoade/equalturn-registry` TUF-on-CI publisher. Use only standard TUF keys compatible with the Rust client. Keep root/targets offline and human controlled; use separate OIDC-to-KMS snapshot/timestamp roles only after explicit authorization and isolated rehearsal.

## Tech stack

Existing Rust workspace and pinned dependencies; existing Python registry validator/target generator; TUF-on-CI v0.21.0 for publication; existing Rust TUF client and staging fixtures for compatibility tests. No bespoke signing implementation, no enforcement changes, no new production repository.

## Spec

See [the design record](../specs/2026-10-20-milestone-1-7-production-registry-design.md). The production root, role thresholds, and online signing provider remain unset until the human custody gate. Production commands continue to fail closed before that point.

## Global constraints

- Preserve baseline and staging trust separation.
- Never generate or expose production private signing material in Codex, CI, logs, commits, artifacts, or GitHub Issues.
- Never provision cloud/KMS resources without explicit production authorization.
- Do not touch the active host's networking, firewall, DNS, routing, AI access, or system enforcement.
- Keep updates explicit, bounded, HTTPS-only, atomic, and separately verifiable offline.
- Use sequential heavyweight validation with one Cargo job and disk-backed target/temp paths.
- Checkpoint each finished stage with focused tests, review, commit, push, and relevant CI before moving on.

## Review focus

TUF role and key boundaries, initial root pinning, root/key lifecycle, fixed production/staging separation, expiration semantics, package/repository updates, privacy, workflow permissions, deterministic targets, interrupted update safety, and honesty of end-to-end claims. Any production signing decision that requires the owner to create/store a private key is the single human gate.

## Execution tasks

1. **Baseline and tooling review** — preserve baseline test results, compare upstream TUF-on-CI configuration and current published staging behavior; update current research findings and source links. No production secrets or root creation. Focused checks: existing validators and staging smoke where network permits. Commit: `docs: record milestone 1.7 production trust baseline`.
2. **Trust policy and publisher workflow** — reconcile the existing threshold proposal with actual maintainer/custody facts; define role expiry/resign policy, offline roots/targets, OIDC role constraints, scheduled expiry monitoring, publication and recovery. Update Equalturn trust/disaster-recovery docs and the existing publication repository only where the action is safe without credentials. Validate workflow permissions and upstream pins. Commit: focused docs/workflow change.
3. **Production client boundary** — implement a fixed production trust profile that embeds a public root/digest only when the owner supplies public material; do not check in an empty/fake root. Keep production unavailable in default builds until bootstrap material exists. Add regression tests proving staging root/URL/cache cannot substitute for production. Commit: `feat: add pinned production registry profile` only if the profile can remain honestly fail-closed without a root.
4. **Rehearsals and release proof** — use disposable test TUF repositories/keys to exercise production-equivalent root and targets rotation, expiration, interrupted refresh, rollback, tamper, cache corruption, and renewal workflow. Add deterministic source-commit-to-target review tooling and a no-secret live-smoke workflow that remains disabled until production root exists. Commit focused tooling and test changes.
5. **Owner custody gate** — stop before first production root key is generated or online cloud resources are provisioned. Ask one bundled decision about root/targets custody and online role provider. After owner response, use a signer process where private bytes never enter Codex output, add only public root metadata/pin, and continue through production publication and client proof.
6. **Final verification and integration** — run resource-safe full checks, branch Linux/PostgreSQL and Windows CI, audit all changes and secrets, open/update PR, review full diff, merge to `main`, verify main CI and production live smoke, and leave a clean tree. Do not claim full milestone completion if the owner custody gate remains unanswered.

## Stage completion records

Each task is complete only after its relevant focused tests pass and its code/docs have been reviewed. Commits and push/CI URLs will be appended here after each checkpoint; external custody-dependent work is explicitly marked blocked until the owner operates the approved workflow.

### Task 1 — baseline and current publisher review

- Reviewed the clean reviewed-main baseline, production and staging trust boundary, registry client/CLI, CI, production publisher repository README and workflows, and current TUF-on-CI v0.21.0 manuals/workflow.
- Recorded the baseline and safe scope in the design record. Baseline observed: `cargo fmt --all -- --check` and workspace Clippy passed; workspace tests passed with 128 passed and 1 ignored; v2/v3 registry validators and deterministic target check passed; pinned-root staging HTTPS smoke passed 1/1; local advisory audit snapshot from 17 October 2026 reported no findings. PostgreSQL was unavailable locally, so the DB-backed subcase was skipped; reviewed baseline main CI had passed Linux/PostgreSQL and Windows.
- Current publisher review confirms `sign/init` is the supported initialization path and standard TUF keys are required for the pinned Equalturn client. The staging failure came from importing externally generated metadata without TUF-on-CI's `x-tuf-on-ci-expiry-period`; Sigstore OIDC remains incompatible with the current Rust client. The production repo has no root, metadata, target, Pages feed, or configured cloud role.
- Context7 lookup did not return an exact TUF-on-CI or `sigstore-tuf` library entry; upstream manuals, current workflow, pinned Rust crate docs/source, and staging run evidence were used.
- Focused verification for this documentation-only task: `git diff --check` (record result after commit). No code/CI behavior changed. The repository's mandatory CI triggers on `main` push, PR to `main`, or manual dispatch; this branch push alone is not expected to start CI.
- Plan checkpoint: `3deeac7e243c0a6ed466563b6ae18a459620653a` (`docs: plan milestone 1.7 production registry trust`), pushed to `work/milestone-1.7-production-registry`.

### Task 2a — production trust policy and recovery documentation

- Corrected the earlier proposal that required three custodians even though no independent maintainer roster was established. Current recommendation is 1-of-1 root and targets under one actual custodian, with explicit single-custodian compromise/loss limitations; the owner must confirm it before any key operation. No artificial keys or threshold claims.
- Selected TUF-on-CI v0.21.0 `sign/init` and its PIV human signer path for production standard TUF signatures; retained `sigstore-tuf` 0.15.0 and rejected experimental TAP-18 for compatibility reasons. Recommended but unprovisioned AWS KMS through GitHub OIDC for distinct snapshot/timestamp roles; costs and account authorization remain for the owner gate.
- Defined proposed root/targets/snapshot/timestamp expiry and alert windows, explicit root renewal, fail-closed expiry semantics for installed data, role compromise impact, key backup/recovery rehearsal, GitHub/PIV/KMS boundaries, and privacy requirements.
- Updated `registry-production-trust.md`, `production-root-ceremony.md`, `registry-disaster-recovery.md`, and contradicted sections of `registry-trust.md`. No production key/resource was created or configured.
- Focused verification: `git diff --check` (record result after commit); manual review of trust claims against implementation (`load_installed_registry` expiry verification) and current upstream TUF-on-CI signer/maintenance manuals.

#### GitHub repository settings inspection

- Read-only authenticated browser inspection was performed for both repositories on 20 October 2026.
- `equalturn-registry`: `main`; Pages disabled; no environments; no branch rulesets; default workflow token read-only and Actions PR creation disabled; all Actions allowed and full-SHA enforcement disabled; secret scanning and push protection enabled; dependency graph/Dependabot alerts disabled.
- `equalturn`: `main`; no branch rulesets; default workflow token read-only and Actions PR creation disabled; all Actions allowed and full-SHA enforcement disabled; secret scanning, push protection, private vulnerability reporting, dependency graph, and Dependabot alerts enabled.
- No setting was changed. The production policy now records the actions to take before signing. The owner gate must authorize enabling Actions PR creation or choosing a GitHub App, because this changes write access; the earlier staging-only approval does not cover production.

### Task 3 — production and staging client boundary regression

- Reviewed `apps/equalturn/src/commands/registry.rs`: production operations fail before root parsing, state-directory creation, or repository/network access; staging requires an explicitly supplied root plus an independent SHA-256 pin. No implementation refactor was justified while the production anchor is intentionally absent.
- Strengthened the CLI integration regression to install a genuinely signed staging snapshot, then attempt production update and status with the staging root, repository, URL, and shared data directory. Both production operations remain fail-closed, and no production-profile state is created. Invalid staging input also leaves existing staging state intact.
- Focused verification: `cargo fmt --all -- --check`; `CARGO_BUILD_JOBS=1 cargo clippy -p equalturn --all-targets --all-features --locked -- -D warnings`; `CARGO_BUILD_JOBS=1 cargo test -p equalturn --locked --test registry_cli` (4 passed); the production/staging isolation case was rerun after adding the URL assertion and passed. An initial test invocation used a sandbox-read-only home cache temp path; rerun with disk-backed `target/tmp` succeeded. No production root or private key was generated.
- This is an integration-test-only hardening checkpoint; production remains deliberately unavailable pending the separate root/custody gate.

### Task 4 — client-side signing-role rotation rehearsal

- Added a TUF verifier rehearsal that rotates the targets role from the bootstrap key to a distinct target key under a dual-authorized root v2. The client accepts target metadata signed by the new authorized key, then rejects a later target manifest signed only by the retired key. The fixture uses `sigstore-tuf`'s canonical JSON and signing primitives; no project-owned TUF signature implementation was added.
- Focused verification: `cargo fmt --all -- --check`; `CARGO_BUILD_JOBS=1 cargo test -p equalturn-registry --locked --test tuf_attacks` (24 passed); `CARGO_BUILD_JOBS=1 cargo clippy -p equalturn-registry --all-targets --all-features --locked -- -D warnings`.
- Scope limit: this proves the Equalturn client's TUF role transition behavior, not an operational TUF-on-CI renewal ceremony. The disposable staging repository remains fixed-content Pages publication; production signer, root, online provider, and renewal path remain gated on owner custody and account-level decisions.

### Task 4b — source-commit target reproducibility

- Extended the deterministic target builder with `--source-commit FULL_COMMIT_ID`. It reads the registry and Draft 2020-12 schema from that exact commit, ignores Git replacement refs, rejects short refs and user-overridden source paths, verifies each Git object is a bounded blob before parsing, and compares only deterministic target bytes. This demonstrates source-to-target reproducibility without treating reproducibility as release authorization.
- Added temporary-Git-repository tests proving uncommitted working-tree edits and replacement refs cannot change the selected commit's output, target mutation is rejected, oversized Git blobs are rejected, malformed/empty identifiers fail closed, and commit-source mode cannot be combined with alternate source paths.
- Focused verification: offline pinned Python environment; release-tool tests (10 passed); full `test_registry_*.py` suite (50 passed); v2 validator (129 entries, 132 provenance records, 58 indicators); v3 validator (131 entries, 84 indicators, 0 warnings); deterministic target build and exact-commit comparison (152,438 bytes, source `0eb74c67fe9ec2517d400b3f8817435ba22b8108`); `cargo fmt --all -- --check`; `git diff --check`.
- Added contributor-facing command and scope note to README and `docs/registry-trust.md`. No production metadata, signer, credential, or repository setting changed.

#### Staging-only GitHub workflow permission state

- Read-only UI inspection on 20 October 2026 confirmed `equalturn-registry-staging` has GitHub Actions PR creation/approval enabled, while its default `GITHUB_TOKEN` permission remains read-only and its publication workflow grants only the documented job-level permissions. This is consistent with the user's staging-only approval.
- The production `equalturn-registry` repository remains unchanged: Actions PR creation/approval is disabled, the default token is read-only, and the repository currently allows all Actions without requiring full-SHA pins at the repository setting level. Production settings have not been changed; these remain a production setup gate.
- The staging repository's existing merged PRs #5 and #10–#12 continue to establish standard-key fixture replacement and fixed-content Pages publishing. They do not establish a renewable signing path or production trust.
