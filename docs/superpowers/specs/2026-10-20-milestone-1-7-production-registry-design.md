# Milestone 1.7 production registry trust design

## Goal

Make Equalturn's production registry trust path operable and renewable without weakening the existing trust boundary or introducing endpoint enforcement. This design treats registry signing custody as a human-controlled security boundary: no production private key is generated, handled, stored, or logged by Codex.

## Current facts

- The reviewed baseline is `6c4e249b61b8fd7c6885347fe004ac6692e7bbc7`; production trust deliberately fails closed.
- `equalturn-registry` is an initialized TUF-on-CI repository template but contains no Equalturn root, signing material, TUF metadata, or target.
- The client pins `sigstore-tuf` 0.15.0 and implements ordinary TUF key verification. The staging TAP-18 `sigstore-oidc` root is not compatible with this verifier.
- The public staging repository now uses standard key metadata and passes the client's pinned-root live smoke. It is test evidence only.
- The earlier standard-key TUF-on-CI workflow failure was caused by metadata generated outside TUF-on-CI that lacked its `x-tuf-on-ci-expiry-period` configuration extension. Production initialization should use the upstream TUF-on-CI `sign/init` workflow rather than hand-editing metadata or writing a signer.
- TUF-on-CI v0.21.0 is currently pinned in the publication repository. Its online-sign workflow supports cloud OIDC integrations; it requires repository-scoped workflow permissions and a configured provider. No provider credentials or production signer have been provisioned.
- The active project has not established independent root/targets custodians. A multi-key threshold cannot honestly be claimed until independent custodians and custody are real.

## Architecture decision

Keep TUF as the only registry update trust protocol. Keep `sigstore-tuf` as the client verifier and TUF-on-CI as the publication workflow. Use only TUF-standard key formats accepted by the client. Do not use TAP-18/OIDC signatures for root or targets. Use the upstream initialization/signing process so TUF-on-CI's own lifecycle metadata is present and renewable.

Use the existing `Voldigoade/equalturn-registry` publication repository and its canonical Pages origin. `Voldigoade/equalturn` remains the only source of reviewed registry data and deterministic target generation. A target is eligible for signing only after validation, deterministic generation, a source-commit/digest review record, and explicit release authorization.

The production client remains disabled until an initial production root is created by an owner-operated or otherwise independently controlled signer and its public root and digest are pinned into a released Equalturn build. A URL, mirror, metadata set, or target from the same remote cannot bootstrap trust.

## Roles and custody proposal

Root and targets keys are human-controlled, offline standard TUF keys. They are not GitHub secrets and are never available to CI. One maintainer using a single signing key is not multi-party threshold security; the initial threshold and custody must be set honestly at the production ceremony after maintainers choose who can independently protect and recover the key. If only one accountable maintainer is available, a 1-of-1 root/targets key may be simpler and more truthful than creating several keys under one person or machine, but it carries single-custodian compromise and loss risk. This is a pending human security decision; existing 2-of-3 policy text must be updated only after that decision.

Snapshot and timestamp are separate narrowly scoped roles. The renewable path should use a supported TUF-on-CI OIDC-to-KMS integration with distinct role keys and GitHub repository/workflow/environment claims, short-lived identity tokens, and minimum signing permissions. Do not place long-lived online private keys in GitHub Secrets. Provider and account creation are not authorized by the staging-only approvals and require an explicit production operation decision.

## Expiry and freshness

Production expiry values and renewal windows are policy, not TUF defaults. Before production root creation, define bounded role expiries with an early warning and rehearsed renewal path. Root rotation is never automatic. Timestamp/snapshot renewal may be automated only after the OIDC/KMS identity and role scope are proven in an isolated rehearsal. If metadata expires, new refresh attempts fail closed. A previously installed, cryptographically valid registry may remain available for inspection with an explicit expired/freshness-unknown status; it must not be described as current. Corrupted or unverifiable installed state is unavailable and must fail closed. No AI detection or enforcement policy consumes the registry in this milestone.

## Release / recovery flow

1. Review the source change and semantic registry diff in Equalturn.
2. Run schema and semantic validation and generate the deterministic target from an immutable source commit.
3. Record source commit, semantic version, schema version, target digest/size, record counts, warnings, and release authorization.
4. Use TUF-on-CI signing events and actual human-controlled root/targets keys. CI never substitutes for content review.
5. Renew snapshot/timestamp through a separately scoped OIDC/KMS path and publish with the existing TUF-on-CI workflow.
6. Run the Rust client against the canonical HTTPS publication and verify the exact target hash, then reopen the installed state offline.

Bad content is corrected by publishing a higher-version target, never by rollback. Targets, snapshot, and timestamp compromise are recovered through authorized role rotation. Root compromise cannot be repaired by the compromised root; it requires an already trusted recovery path or a new client release distributed through an independently authenticated channel. Root loss requires a tested offline backup. A GitHub compromise alone must not replace the pinned root, but can deny service or publish role-authorized content if an online role is compromised.

## Privacy and scope

Registry retrieval is a common public download and must not include session IDs, nicknames, participant IDs, relay data, report data, CTF target, or registry search query. The local CLI is explicit; normal session operations do not phone home. There is no process, browser, IDE, filesystem, or network enforcement in this milestone.

## Human gate

Before creating the production root, the owner must choose and operate the actual root/targets custody workflow and authorize any OIDC/KMS account or provider setup. Do not generate a fake threshold or provision paid cloud infrastructure. Keep all public metadata separate from private signer material. At the gate, ask once with a recommended custody/provider plan and practical alternatives, then resume after the owner acts.

## Verification baseline

The continuation checkpoint is the clean branch `work/milestone-1.7-production-registry` at the same reviewed commit as `origin/main`. Observed baseline checks: formatting passed; workspace Clippy with warnings denied passed; workspace tests passed (128 passed, 1 ignored); legacy and v3 registry validators passed; deterministic release target matched; current staging live Rust smoke passed; local `cargo audit` used an advisory database snapshot dated 17 October 2026 and reported no vulnerable dependencies. PostgreSQL-backed coverage was not run locally because no supported local PostgreSQL server/container was available; the reviewed baseline's GitHub Actions run passed Linux/PostgreSQL and Windows. The missing local PostgreSQL service is recorded rather than represented as a local pass.

## Sources checked 20 October 2026

- [TUF specification v1.0.37](https://theupdateframework.github.io/specification/v1.0.37/)
- [TUF-on-CI repository](https://github.com/theupdateframework/tuf-on-ci), [v0.21.0 repository maintenance guide](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/REPOSITORY-MAINTENANCE.md), [signer manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-SETUP.md), and [online signing guide](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/ONLINE-SIGNING-SETUP.md)
- [TUF-on-CI template](https://github.com/theupdateframework/tuf-on-ci-template)
- [`sigstore-tuf` 0.15.0 Rust API](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/)
- [GitHub OIDC for cloud providers](https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments/about-security-hardening-with-openid-connect)

Context7 resolved TUF to the upstream Python-TUF docs but did not provide an exact TUF-on-CI or `sigstore-tuf` library entry. TUF-on-CI's own current manuals and the pinned Rust crate docs/source are therefore the primary implementation references.
