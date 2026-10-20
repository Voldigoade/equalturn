# Production registry trust policy

**Status:** recommended policy for owner decision; production trust remains uninitialized.<br>
**Reviewed:** 20 October 2026

## Current state

`Voldigoade/equalturn-registry` is the intended public TUF publication repository. Its default branch is `main`, and its current upstream-derived workflows are pinned to TUF-on-CI v0.21.0. It has no Equalturn root, keys, metadata, target, production Pages feed, or online signer. GitHub API access available to this review could read repository content but not Pages, Actions, or branch-protection settings; those settings must be checked through repository settings before initialization.

The public Equalturn commit history shows one author account, which is not a complete maintainer roster. No independently accountable signer roster or custody plan is documented. Do not claim multi-party security without named independent custodians and actual separate custody. No production key or cloud resource has been created or authorized.

## Recommended initial role policy

This is a proposed configuration, not metadata already in production. Role thresholds and expiries are project policy, not TUF requirements. Given the absence of a demonstrated independent signer roster, an honest initial policy is 1-of-1 for root and targets, with the single-custodian compromise limitation stated plainly. Do not create multiple keys under one person's control and describe them as independent threshold security. If maintainers later establish separate governance and custody, migrate through an authorized root update.

| Role | Recommended initial policy | Expiry and renewal | Authority and compromise impact |
| --- | --- | --- | --- |
| `root` | Threshold 1; one owner-operated PIV hardware signer using TUF-on-CI's supported standard TUF signature path. Never available to CI or KMS. Keep a tested, encrypted offline recovery copy through a documented supported key import/export process; if recoverability cannot be demonstrated, defer initialization. | 2 years; start review/renewal 180 days before expiry. Root renewal/rotation always requires an explicit human signing event. | Authorizes all role keys and thresholds. Root compromise permits complete repository takeover. Root loss cannot be recovered inside the TUF chain without a usable authorized recovery copy. |
| top-level `targets` | Threshold 1; a distinct PIV key from root, held by the maintainer. Require registry PR review, deterministic build, and source/digest review before the signing event. | 180 days; begin re-signing 60 days before expiry. | Authorizes registry target bytes. Compromise can publish false or harmful but valid research data; it cannot replace the root role key set. |
| `snapshot` | Threshold 1; a distinct OIDC-federated AWS KMS key/role with only snapshot signing permission. Restrict trust to the exact publication repository, branch, workflow, and protected environment. | 30 days; TUF-on-CI's scheduled workflow renews several times per day; alert if less than 14 days remain. | Authenticates repository metadata consistency. Compromise may disrupt freshness or metadata state but cannot authorize new targets or root keys. |
| `timestamp` | Threshold 1; a separate OIDC-federated AWS KMS key/role with only timestamp signing permission and the same narrow identity restrictions. | 7 days; scheduled renewal several times per day; alert if less than 72 hours remain. | Provides freshness. Compromise can cause freeze/availability issues within its authorization; it cannot authorize target bytes or replace root. |

TUF-on-CI supports AWS KMS, Google Cloud KMS, and Azure Key Vault OIDC flows. AWS KMS is the initial recommendation because the template exposes an AWS OIDC role input and can avoid long-lived cloud credentials. This requires an AWS account and may incur charges. The owner may choose GCP or Azure instead. Sigstore online signing is not selected: TUF-on-CI calls it experimental and warns that not all TUF clients support it; the staging TAP-18 root was rejected by Equalturn's current Rust verifier. If no supported standard-key compatible online signer is authorized and configured, keep the production repository uninitialized rather than storing static online keys in GitHub Secrets or claiming an unrenewable static feed is production-ready.

PIV is required by TUF-on-CI's documented local signer. The owner has not confirmed possession of compatible hardware. Confirm the exact PIV key type, role key separation, recovery process, tool version, and Rust-client compatibility in a disposable rehearsal before production signing. TUF-on-CI documents the local signing tool and hardware PIV; do not assume a generic encrypted software key file is a supported TUF-on-CI signer.

## Role and workflow boundaries

Separate four decisions:

1. **Content review:** source registry, authoritative provenance, semantic diff, validator output, deterministic target digest, and reviewer approval in Equalturn.
2. **Release authorization:** a TUF-on-CI `sign/**` event that contains only the reviewed target and metadata changes. Root/targets signatures are human decisions; GitHub Actions does not choose content.
3. **Online renewal:** a dedicated workflow uses GitHub OIDC to assume separate snapshot/timestamp KMS permissions. No root/targets material or long-lived cloud credentials enter CI.
4. **Client trust:** Equalturn embeds the exact public root bytes and digest and uses only `https://voldigoade.github.io/equalturn-registry/`. The URL or a remote root never bootstraps trust.

The current upstream workflow grants empty workflow-level permissions and narrowly scoped job permissions: online signing uses `id-token`, `contents`, and `actions`; signing events use `contents`, `pull-requests`, and `actions`; failure status updates use `issues` and `contents: read`. Keep those job boundaries and full-SHA action pins. TUF-on-CI requires GitHub Actions to create/approve pull requests when using its default token, or a configured GitHub App for stricter branch protection. Select and configure one explicitly. Restrict the Pages `github-pages` environment to the `publish` branch and verify the Actions write and OIDC trust settings before first initialization.

## Expiry, stale state, and privacy

The client rejects expired metadata for refresh and for installed-state verification. An expired or unavailable feed therefore cannot be shown as current trusted data; it is not silently accepted for advisory use, and future enforcement must never use it. A future product UX may preserve non-authoritative historical display only if it is explicitly marked expired and cannot enter a trust decision. This milestone does not change that behavior.

The client clock is part of expiry evaluation. Clock skew may reject current data or delay expiry detection; the CLI must show a stable expiry/error state and must not waive checks. A network failure must preserve the prior atomic install, but that install is only usable if the complete stored TUF chain remains valid and unexpired. Corrupt/unverifiable cache state fails closed. Losing the metadata version floor can weaken rollback protection; do not silently reset it.

Registry retrieval is an explicit public request. It must not include nickname, session/participant IDs, relay identity, event/report data, CTF target, or search query. TUF signatures authenticate publication authorization and bytes, not factual accuracy, indicator quality, or safe enforcement. Registry entries remain research data disconnected from enforcement.

## Expiry monitoring and renewal

After a trusted production root is embedded, run a credential-free scheduled check against the canonical public TUF repository using that pinned root and the Rust verifier. It should alert before the thresholds above, fail when metadata is missing/expired/unverifiable, and publish only public role versions/expiries and target digest. The check must be separate from mandatory deterministic PR CI and must never accept a root or metadata merely to inspect expiry. TUF-on-CI's online workflow runs several times per day; an automated timestamp/snapshot renewal is not a substitute for human root/targets review. Root rotation is never scheduled automatically.

## Key lifecycle and incident response

- Generate root/targets material only through the documented TUF-on-CI signer on the owner's controlled PIV environment. Private material never enters this checkout, chat, CI, GitHub Issues, PRs, Actions logs, build artifacts, or the publication repository.
- Maintain one encrypted offline recovery copy in a separate physical/storage location where the selected PIV method permits a supported recoverable import. Perform a disposable recovery rehearsal without exposing production bytes. Record only public key IDs/fingerprints and verification results.
- Recheck the exact signer and KMS trust policy at each workflow upgrade. Remove compromised online identities immediately, then rotate the affected delegated role through root-authorized metadata.
- For compromised targets, correct content with a higher semantic/metadata version; never roll back TUF version numbers. For bad but valid content, TUF cannot determine truth.
- For compromised/lost root without a valid offline recovery path, stop the repository and ship a new independently authenticated application trust anchor. TUF cannot repair a compromised root by fetching another root from the same repository.
- For GitHub/Pages compromise, treat it as denial/content-delivery risk unless an authorized signer or OIDC role was also compromised. Restore publication only after reviewing repo history, workflows, OIDC claims, and metadata through the pinned Rust client.
- For expired metadata, fail closed for updates and installed trusted use. Restore an authorized renewal; never patch expiry locally.

## Production initialization gate

Before `tuf-on-ci-delegate sign/init`, the owner must confirm root/targets threshold and physical custody, a compatible PIV signer and recoverability, online KMS provider/account and exact costs, GitHub workflow and Pages settings, role expiry/re-sign values, and the application channel that will distribute the root. Staging-only approvals do not authorize any of these production actions. Do not generate a production key or provision cloud resources until that explicit decision.

## Sources checked 20 October 2026

- [TUF v1.0.37 specification](https://theupdateframework.github.io/specification/v1.0.37/)
- [TUF-on-CI v0.21.0 repository maintenance](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/REPOSITORY-MAINTENANCE.md), [signer setup](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-SETUP.md), [signer manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-MANUAL.md), and [online signing](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/ONLINE-SIGNING-SETUP.md)
- [`sigstore-tuf` 0.15.0](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/)
- [GitHub OIDC security hardening](https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments/about-security-hardening-with-openid-connect)
