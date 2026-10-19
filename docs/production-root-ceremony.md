# Production registry root ceremony

**Status:** not authorized to run in the development environment; no production key has been generated.  
This document is a future operational gate, not an invitation to create a key in this checkout.

## Preconditions

Do not begin until all are true:

1. Three independently accountable maintainers are named in project governance and each accepts offline root-custodian duty.
2. Each custodian has a separate supported hardware-backed signer and a clean, controlled signing environment. No signer key is copied into source control, CI, chat, a shared workstation account, or the other custodians' routine storage.
3. The production repository and exact endpoint are reviewed. The expected endpoint is `https://voldigoade.github.io/equalturn-registry/`; it must not be treated as initialized until published metadata passes independent verification.
4. Production branch protection, signing-event review, Pages configuration, and the minimal workflow permissions are inspected by maintainers. Online snapshot/timestamp identities are separately scoped; no static signing key is stored as a repository secret.
5. The application release channel that will carry the bootstrap root and its expected digest is documented. If GitHub is the incident under investigation, the digest must be verified through another channel.
6. `docs/registry-production-trust.md` has no unresolved policy decisions and current TUF-on-CI version/workflows are rechecked.

If any prerequisite is unavailable, defer publication. Do not reduce root or targets thresholds or import staging keys.

## Ceremony procedure

1. Freeze the Equalturn source commit and produce `registry-v3.json` with the deterministic release builder. Record the source revision, target SHA-256, registry semantic version, warnings, and review decision in a release record.
2. In the designated production publication repository, use the current upstream TUF-on-CI `tuf-on-ci-delegate sign/init` workflow. Do not handcraft TUF metadata. Confirm the configured root and top-level targets thresholds match the approved roster; create separate online snapshot/timestamp role identities only through a reviewed provider setup.
3. Each root custodian runs the upstream `tuf-on-ci-sign` client locally and inspects the signer tool's exact root change before signing. At least two of the three custodians independently verify the root version, key IDs, thresholds, expiry, repository identity, and role delegations. The root threshold must be met by distinct custodians.
4. Each target signer inspects the target diff and the source/hash/review record before signing. At least two distinct authorized signers must approve the target metadata. Review signatures are authorization, not a substitute for registry research review.
5. Complete the signing event using TUF-on-CI's normal pull-request workflow. Merge only after the required threshold and workflow verification pass. The publication workflow creates snapshot/timestamp metadata and publishes through the reviewed Pages environment.
6. Download the exact published version from an independent client. Verify root-to-timestamp/ snapshot/targets metadata and the target hash with the Rust TUF client. Compare the target bytes with a fresh deterministic local build. Independently compare the published initial root bytes and digest with the release candidate.
7. Pin the exact production root bytes and hash into the reviewed application release through the approved bootstrap channel. Confirm the production profile cannot select or fall back to staging. Announce the endpoint only after that release and the published repository have both been verified.
8. Record only public role/key fingerprints, metadata versions, expiration times, target hash, source commit, verification outputs, and signer roster/approval record. Never place private key material or seed exports in the evidence bundle.

## Rotation and recovery drills

Before initial release, test an authorized root rotation through the staged signing workflow, verify old and new threshold checks, and confirm a fresh client can advance version by version. Test target correction and online-role rotation in staging first. Exercise the recovery copy procedure without exposing the recovery material to CI. Preserve all versioned root metadata required for clients to rotate.

On root compromise or loss of threshold, stop the current repository. Do not accept a replacement root from its endpoint. Ship a new application trust anchor through an independently authenticated release and retire the prior repository identity. If that distribution path cannot be independently authenticated, pause the update service and communicate the unresolved risk rather than claiming recovery.

## Upstream runbooks

- [TUF-on-CI repository maintenance manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/REPOSITORY-MAINTENANCE.md)
- [TUF-on-CI signer manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-MANUAL.md)
- [TUF-on-CI signer setup](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-SETUP.md)
- [TUF-on-CI online-signing setup](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/ONLINE-SIGNING-SETUP.md)

