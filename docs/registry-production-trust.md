# Production registry trust policy

**Status:** policy proposal; production trust remains uninitialized.  
**Reviewed:** 19 October 2026

## Current maintainer and key status

The public Equalturn commit history currently shows one author account. That is not a complete collaborator or maintainer roster, and it does not prove only one person maintains the project. However, no independently accountable signer roster or custody arrangement has been established in the repository. It would be misleading to claim a multi-person threshold today. Accordingly, **no production root or production role keys are authorized or created**. Production registry updates remain unavailable until the conditions below are met.

## Proposed production roles

These are prerequisites for a future production ceremony, not current settings. Role values and expiration periods are project policy, not mandated by TUF.

| Role | Planned threshold | Separation and custody | Planned expiry |
| --- | --- | --- | --- |
| `root` | 2 of 3 | Three named, independently accountable maintainers; separate offline hardware-backed signing keys; root keys never available to CI or a cloud KMS. | 2 years; begin review/renewal at 1 year. |
| top-level `targets` | 2 of 3 | Three human signers review target bytes, source commit, validation, semantic diff, and release notes before signing. Use keys distinct from root keys; no broad delegation for v1. | 180 days; re-sign during an approved release or deliberate renewal. |
| `snapshot` | 1 of 1 | Dedicated OIDC-federated cloud KMS identity restricted to the publication repository, protected workflow, branch, and environment. Key is limited to snapshot metadata. | 30 days. |
| `timestamp` | 1 of 1 | A separate OIDC-federated identity/key from snapshot, limited to timestamp metadata. | 7 days. |

Do not perform the production root ceremony until three custodians have explicitly accepted the responsibilities and have tested their separate hardware signing setup. If that roster cannot be assembled, keep the production endpoint empty and do not weaken the threshold to make a release possible. A 1-of-1 production root would have no tolerance for key theft or maintainer compromise and is not an acceptable shortcut for this project.

The planned online roles are not provisioned. TUF-on-CI documents AWS, Google Cloud, and Azure KMS integrations; Sigstore online signing is documented as experimental. Select a provider only after reviewing its current trust conditions and ensuring the identity cannot sign root or targets. Avoid static GitHub repository secrets for long-lived private keys. If online roles are not configured, publication may be delayed; do not substitute a developer workstation secret.

## Staging-only signer policy

For a disposable staging repository used only to exercise tooling, a **1-of-1 test signer** may initialize root and targets with separate non-production key material. Use role expiry no longer than 90 days for `root` and `targets`, 30 days for `snapshot`, and 7 days for `timestamp`. The keys must be clearly labeled for staging, stay outside the repository and production build configuration, and be rotated or the repository reinitialized if signer material is lost. Staging data is public and non-authoritative. A staging signature must never be represented as independent review or as a production approval. This single-signer policy is deliberately not acceptable for production.

## Endpoints and bootstrap

- Intended production repository base URL: `https://voldigoade.github.io/equalturn-registry/`. It is currently uninitialized and must not be treated as a source of trust.
- Staging repository: a distinct repository and root, if created, at `https://voldigoade.github.io/equalturn-registry-staging/`. Staging is for non-production protocol smoke tests only.
- The initial production `root.json` and its SHA-256 digest must be embedded in an Equalturn release or provisioned through another authenticated, out-of-band release channel. A client must never download an initial root and accept its own signature from the same untrusted endpoint.
- The bootstrap release must disclose the root version, fingerprint/hash, repository URL, and date. Reviewers must verify the exact root bytes from the local signer ceremony against the copy used by the release. A GitHub-hosted checksum alone is not an independent channel if GitHub is the suspected compromise.
- Staging roots and test signer keys must never be compiled into production trust, accepted by a production profile, copied to the production repository, or reused for production roles.

## Rotation and expiration

TUF root rotation must follow the version-by-version authorized chain: each new root is signed by the threshold of the trusted previous root and by its own new root threshold. Keep each versioned root metadata file required by the TUF repository so outdated clients can advance. Role metadata updates must be authorized by the delegating role. Run and inspect a root-rotation smoke test before the first production release.

The client uses its current wall clock for expiry validation. A bad local clock can reject fresh metadata or make expiration semantics unreliable; reports must describe the observed clock/expiry state and must not silently accept expired metadata. On network failure, a previously installed registry can be shown only when its persisted root/metadata state re-verifies and is still within policy. It must be labeled with its last successful refresh and expiry; expired or unverifiable state is not current trusted data.

## Incident response

| Incident | Response |
| --- | --- |
| Mirror, Pages, or GitHub repository compromise | Treat it as availability/content-delivery compromise. TUF clients reject altered content; restore hosting and publish from the reviewed signer workflow. Investigate repository and workflow changes. |
| One offline root or targets signer lost/compromised | Use the remaining authorized root threshold to rotate the root and affected role keys; remove the old key, preserve verified evidence, and publish an out-of-band notice. |
| Root threshold compromised | Stop accepting the old repository identity. Release a new authenticated application trust anchor through an independent channel and announce the old root/repository as revoked. TUF cannot repair a compromised root by trusting a replacement root fetched from the compromised repository. |
| Snapshot/timestamp identity compromised | Stop the affected workflow identity, rotate the role through root-authorized metadata, and inspect version/expiry behavior. Do not let the online role acquire root/targets signing authority. |
| Registry contains a mistaken or harmful but correctly signed record | Review and correct the source registry; publish a new semantic version and target through the normal target threshold. TUF proves authorization and byte integrity, not truth or safe enforcement. |
| Production root quorum lost | There is no safe recovery inside the existing root chain. Establish a new repository identity only through a separately authenticated application release; explicitly retire the old endpoint. |
| Metadata expires before renewal | Fail closed for current trust. Restore signer availability and issue authorized fresh metadata. Do not extend acceptance locally or disable expiry checks. |

## Release gate

Before production publication, record (without secrets): the three custodians and role responsibilities; root and target public-key fingerprints; expiry values; hardware signer verification; protected repository/workflow settings; online identity scope; successful authorized rotation; source commit and deterministic target digest; two-person-or-more target review; published HTTPS smoke test; and the exact root bytes/hash included in the application trust anchor. The production root private keys must be generated and kept outside this development environment. The specific human/offline procedure is in [production-root-ceremony.md](production-root-ceremony.md).

## References

- [TUF v1.0.37 specification](https://theupdateframework.github.io/specification/v1.0.37/) — root rotation, thresholds, expiry, and consistent repository state; checked 19 October 2026.
- [TUF-on-CI repository maintenance manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/REPOSITORY-MAINTENANCE.md) — current initial setup, Pages, workflow authentication, and online roles; checked 19 October 2026.
- [TUF-on-CI signer manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-MANUAL.md) — local signer interactions and signing-event workflow; checked 19 October 2026.
- [TUF-on-CI online-signing setup](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/ONLINE-SIGNING-SETUP.md) — current KMS integrations and Sigstore status; checked 19 October 2026.

