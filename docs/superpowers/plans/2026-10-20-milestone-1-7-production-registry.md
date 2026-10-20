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
