# Trusted registry distribution: tooling decision

**Decision date:** 19 October 2026
**Status:** local Rust verification and offline CLI are implemented; repository publication, production root ceremony, and signed release remain uninitialized. No signed production registry is available.

## Decision

Use **TUF-on-CI for repository publication and signing workflow**, with **`sigstore-tuf` 0.15.0 as the Rust verification client**. The registry remains an opaque TUF target: TUF authenticates the bytes and metadata chain; the existing v3 JSON Schema and semantic validator separately validate the target's content. There is no Equalturn-specific signature format or custom update protocol.

TUF is the right trust model for a public, infrequently updated catalogue mirrored through ordinary hosting. Its root, targets, snapshot, and timestamp roles limit the effect of a single online-key or hosting compromise; threshold signatures, metadata versions and expiry address distinct key-compromise, rollback, mix-and-match, and freeze risks. Clients must begin with a root obtained out of band and walk authorized root rotations. The [TUF specification](https://theupdateframework.io/spec/) is the protocol authority.

## Current implementation evidence checked

| Component | Observed state on 19 October 2026 | Relevance |
| --- | --- | --- |
| [TUF specification v1.0.37](https://theupdateframework.github.io/specification/v1.0.37/) | Latest versioned specification listed by the upstream site on 19 October 2026. It defines threshold verification, pinned initial roots, versioned root rotation, consistent snapshots, and metadata expiry. | Use standard roles and client workflows; do not sign arbitrary serialized JSON. |
| [TUF-on-CI](https://github.com/theupdateframework/tuf-on-ci) | The upstream changelog lists v0.21.0 as the latest release, with an Unreleased section. The project describes guided distributed signing, threshold delegations, hardware signers, and online-signing integrations. | Provides publication and signing workflows instead of hand-written GitHub Actions cryptography. Follow the current template and repository maintenance documentation, including its GitHub Pages and `publish` branch setup. |
| [`sigstore-tuf` 0.15.0](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/) | Current crate documentation describes a transport-free verifier state machine, pluggable bounded repository transport, an HTTP repository behind its `client` feature, root chaining, expiry/version/hash checks, and re-verified metadata caching. | Fits the Rust boundary and deterministic local tests. Its HTTP adapter must explicitly disable redirects and enforce Equalturn's HTTPS origin policy. |
| [`tough` 0.24.0](https://docs.rs/tough/0.24.0/tough/) / [`tuftool`](https://github.com/awslabs/tough) | Active Rust client and repository editor; documentation says it targets TUF 1.0.0 with documented exceptions for delegated roles and TAP 3/4. Upstream was pushed in September 2026. | Strong Rust-native alternative for local repository creation and verification, but it does not provide TUF-on-CI's distributed signing-event and GitHub publication workflow. Its key-ID handling also has a documented interoperability concern for some `tuf-on-ci` metadata extensions. |
| [`tuf` 0.2.0](https://docs.rs/crate/tuf/0.2.0) | Stable release dates from 2017; the current 0.3 line remains beta and the crate documentation warns that its API is unstable. | Rejected for a new client despite its historical connection to the TUF project. |

The repository versions and docs above were checked on the stated date. The `sigstore-rust` repository was active that day. Its TUF crate is a relatively new, focused implementation rather than a long-established general-purpose Rust library; keep it behind a small adapter, pin it in `Cargo.lock`, and exercise the complete attack suite before treating any update as usable. The crate documents interoperability details such as TUF canonical JSON and declared key IDs; those details are particularly relevant to metadata emitted by TUF-on-CI. Context7 did not return a Rust-specific `sigstore-tuf` library entry during this review; the crate's versioned docs.rs pages, pinned source, and upstream repository were used instead.

## Alternatives considered

### A. TUF-on-CI publication + maintained Rust verifier — selected

This separates the signing ceremony and release workflow from registry semantics. Reviewers can approve a normal data change while authorized signers participate in the TUF signing event; GitHub Pages or a mirror carries only public metadata and target bytes. The Rust client still verifies the standard TUF chain. The cost is a dedicated publication repository/workflow and a separate signer/key ceremony; the repository decision is recorded in the next architecture stage.

### B. Rust-native publication and verification with `tuftool`/`tough`

This keeps metadata generation in Rust and has active upstream maintenance. It would require Equalturn to maintain the signer invitation, threshold collection, role rotation, CI permissions, and publish workflow around the CLI. That is avoidable operational security code. The client can also have interoperability friction with repositories whose key IDs include TUF-on-CI-specific key fields; the `sigstore-tuf` documentation describes why it accepts declared IDs for interoperability.

### C. Reference Python TUF publication/client

Python-TUF is an important reference implementation and TUF-on-CI itself uses it, but a Python runtime in the Equalturn client would add a second production language and packaging surface. Use the Python implementation as a publisher dependency inside TUF-on-CI, not as a runtime dependency of the Rust CLI.

### D. Older `tuf` Rust crate

The published stable version is stale for a new security boundary; a beta release with an unstable API is not a better fit than the active alternatives above.

## Publication repository layout

**Decision:** keep reviewed source data in `Voldigoade/equalturn` and publish TUF metadata and generated targets from a separate public repository, [`Voldigoade/equalturn-registry`](https://github.com/Voldigoade/equalturn-registry), created from the upstream [`tuf-on-ci-template`](https://github.com/theupdateframework/tuf-on-ci-template). This avoids giving application build and test workflows the metadata publication permissions required by TUF-on-CI. The publication repository is a release surface, not a second editable source of registry facts.

- **Source of truth and review:** `registry/registry-v3.json`, its schema, semantic validator, and research sources in Equalturn. Registry changes are reviewed and validated there first.
- **Generated target:** deterministic output `registry-v3.json`, published under TUF's `targets/` directory. A release change copies the generated bytes into a TUF signing event; nobody edits that target by hand.
- **TUF metadata:** role metadata under `metadata/` in the publication repository. TUF-on-CI creates signing events and updates metadata through its standard workflow.
- **Published location:** the template's Pages workflow builds from a ref, deploys to GitHub Pages, and smoke-tests the deployed repository. The upstream maintenance instructions call for Pages source `GitHub Actions` and a `github-pages` environment deployment branch of `publish`.
- **Initial trust anchor:** after a root ceremony, its public `root.json` will be pinned in the Equalturn release/application trust store. The application must not fetch the initial root from Pages. Root private keys remain offline and are never stored in either repository.

The publication repository was created on 19 October 2026 from the template, with public visibility and default branch `main`. Its README records the incomplete state. It currently contains no Equalturn root, signer roles, TUF metadata, or target. Pages, signer roles, online signing, and the workflow permission that allows `GITHUB_TOKEN` to open pull requests remain unconfigured until the independent root/signing ceremony is scheduled. Enabling release permissions before credentials and signers exist would add capability without enabling a trusted release. The template workflows declare empty workflow-level permissions, grant needed permissions at job scope, and pin TUF-on-CI and deployment actions to commit SHAs. The online-sign job still has `id-token: write`, `contents: write`, and `actions: write`; those permissions require protected `main` review and a tightly scoped online-signing identity before that job is enabled for production.

The split adds a second repository and a release-copy step, but creates a useful permission boundary: ordinary Rust CI and research PRs cannot publish trusted metadata. Generated target bytes can be traced to an approved Equalturn source revision. Publication automation must record that source commit in the signing-event PR or release notes.

## Proposed role policy and key custody

This is the operational policy to implement before the first production signing event. It is not configured in the new repository yet. Thresholds and lifetimes below are Equalturn design choices, not TUF requirements. TUF requires role signatures to meet the threshold in the currently trusted root, rejects expired metadata, and requires a rotated root to advance one version at a time under the old and new root trust. See the [TUF 1.0.37 specification](https://theupdateframework.github.io/specification/v1.0.37/) (checked 19 October 2026).

| Role | Threshold and key separation | Custody / permitted operation | Proposed expiry |
| --- | --- | --- | --- |
| `root` | 2 of 3 distinct keys, all independent of other roles | Offline, held by three named maintainers on separate signer devices. Never present in GitHub Actions or cloud KMS. Root changes require two custodians to verify the exact versioned root diff and sign the rotation. | 2 years; plan a reviewed renewal/rotation after 1 year |
| top-level `targets` | 2 of 3 distinct keys, separate from root keys | Offline, held by the same three maintainers but with different role keys. Each signer reviews the generated target, source commit, validator results, semantic diff, and release notes before signing. No broad delegations in this milestone. | 180 days; renew with an approved registry release |
| `snapshot` | 1 of 1 separate online key | OIDC-federated cloud KMS identity restricted to the publication repository, protected `main`, and the exact signing workflow/environment. The key can sign only snapshot metadata. | 30 days |
| `timestamp` | 1 of 1 separate online key | A different OIDC-federated KMS identity and key with only timestamp-role permission. TUF-on-CI's template schedules online signing four times daily; keep automatic refresh enabled after deployment. | 7 days |

Root and target keys must be generated through the TUF-on-CI-supported signer workflow, with key algorithms and storage support verified against the chosen signer version during the ceremony. Keep one encrypted recovery copy for each offline key under custody separate from the active device; do not place multiple threshold shares in one person's normal workstation account. Record key fingerprints, role, custodian, creation date, expiration, backup location class, and recovery procedure, but not secret material. Rotate a single lost/compromised key by producing the next root version with the old-root threshold and the new-root threshold; distribute the new public root through the repository only after verification. If the 2-of-3 root quorum is lost, there is no safe self-service recovery: establish a new repository identity through a separately authenticated Equalturn release and retire the old endpoint. If the root threshold is compromised, revoke the repository trust anchor in a signed application release and announce the incident out of band.

The online snapshot/timestamp keys must not be static GitHub secrets. Use a supported TUF-on-CI OIDC-to-KMS provider, with trust conditions pinning repository, branch, workflow identity, and deployment environment. Provider selection is intentionally pending because no production cloud account or service is part of this milestone. Until configured, online-sign jobs must not receive credentials or run for production releases. Never copy signing keys into this project checkout or its CI cache.

The 7-day timestamp lifetime bounds how long an online mirror can keep serving apparently fresh metadata after updates stop, assuming the client has a trustworthy clock. Expired metadata is rejected; it does not imply that the registry itself is false. The short timestamp and weekly-or-better publication cadence trade outage tolerance for freeze detection. The 180-day targets lifetime requires a reviewed renewal even when content is unchanged, so a forgotten registry cannot remain trusted indefinitely. The two-year root lifetime leaves time for offline custodians to rotate it, while the one-year renewal target avoids operating close to expiry.

### Threats and response

| Threat | What TUF / policy contains | Residual risk and response |
| --- | --- | --- |
| GitHub or mirror compromise | A mirror cannot change signed metadata/target bytes; application root and targets thresholds remain independent of hosting. | Hosting can deny service or replay metadata only until expiry. Restore hosting and publish fresh metadata; compare target hashes to the reviewed source. |
| Malicious registry PR or compromised source maintainer | CI semantic validation and source review precede an independent 2-of-3 target signing event. | A malicious but correctly signed entry remains possible; remove it in a reviewed release and explain the correction. TUF does not evaluate indicator truth or block safety. |
| One compromised root or targets signer | A single key is below its 2-of-3 threshold. | Two compromised custodians can authorize a takeover or malicious target; revoke the role through a verified root rotation and publish an incident notice. |
| Snapshot or timestamp key compromise | Separate 1-of-1 online keys cannot sign the top-level targets role. | They can disrupt, roll forward metadata versions, or produce denial of service; revoke/rotate the affected role via root-authorized metadata and invalidate cached metadata where needed. |
| Freeze / rollback / mix-and-match | Expiry, monotonically increasing metadata versions, snapshot hashes/lengths, consistent snapshots, and persistent client metadata state reject stale or inconsistent state. | A client that never checks for updates cannot learn that a new release exists; the CLI must report last-checked time and expired/frozen state. A compromised local metadata cache can erase rollback history unless stored in protected persistent state. |
| Root key loss or threshold compromise | Independent offline custody, expiry monitoring, recovery copies, and two-threshold root rotation. | Loss of the 2-of-3 quorum is unrecoverable within the existing root chain. Threshold compromise destroys repository trust; ship a new pinned root through an authenticated application release and clearly retire the old repository. |
| Malicious valid entry or wrong classification | Separate two-person source review, explicit provenance, semantic validation, and visible limitations. | Signatures establish authorization, not factual accuracy, recency, or safe enforcement. Correct the data and retain a review trail; no indicator is executable policy. |

Production release gates are: three independently accountable root/targets custodians; successful dry-run root rotation; protected `main` and publication environment; limited OIDC/KMS role tests; a reviewed source commit and deterministic target hash; two target signatures; online role smoke test; published repository smoke test; and an Equalturn release that embeds the exact initial root. These gates are not met merely by creating `equalturn-registry`.

## Deterministic target bytes

`tools/build_registry_release.py` validates the v3 source with the checked-in Draft 2020-12 schema and semantic validator, then emits `dist/registry-v3.json`. The ignored `dist/` output is a build artifact; it is not a second maintained source. The publication copy goes to the fixed TUF target name `targets/registry-v3.json`.

The emitted bytes use UTF-8, NFC-normalized text, lexicographically sorted JSON object keys, two-space indentation, and one final newline. The validator requires the semantically ordered arrays (entries, categories, surfaces, sources, indicators, and relationships) to already be canonical; the builder will reject invalid ordering instead of silently rearranging research. NFC key collisions are rejected. The builder enforces the 4 MiB maximum, writes through a same-directory temporary file and refuses to replace the source registry. It adds no build timestamp, host information, or signature. TUF metadata supplies publication versions, expiry, and target hashes.

Reproduce and compare a target with:

```sh
python3 tools/build_registry_release.py --output dist/registry-v3.json
python3 tools/build_registry_release.py --output dist/registry-v3.json --check
```

The checked-in snapshot currently emits 152,438 bytes with SHA-256 `e6ccf57eca33951cd6b7a04e7e609f981c87c73298b7923b4d5a9a845caa02a0`. On 19 October 2026, two independent builds matched byte-for-byte and the generated target passed the v3 schema/semantic validator. CI repeats the generation-and-compare check in its dedicated hardening step.

## Security boundary and constraints

- TUF proves that a target is authorized by the configured repository's metadata chain and has the expected length/hash. It does not prove an indicator is true, current, correctly scoped, or safe to block.
- The JSON Schema and semantic validator remain mandatory after TUF verification. A malicious but correctly signed registry target still requires trusted review/signing governance; TUF does not make signed content benign.
- The application must pin an initial root in the released application or another explicit out-of-band trust store. It must never accept a root merely because it came from the same untrusted mirror as the target.
- The client will refresh only on an explicit registry command/API call. Session startup, relay connection, and competition mode will not silently fetch a registry.
- Target paths are fixed logical names, not server-selected filesystem paths. Downloads have per-role and total byte limits and are verified before parsing or writing to the final cache path.
- No production root keys, signing keys, GitHub secrets, or repository credentials are created or committed during this tooling decision. A real threshold root ceremony and its signer set are required before publishing a production trusted repository.
- The existence of `Voldigoade/equalturn-registry` does not mean the TUF repository is initialized or a release is available. A client must fail closed until an out-of-band root is pinned and the complete metadata chain is valid.

## Rust client boundary (Milestone 1.5)

`crates/registry` now provides a typed, read-only v3 registry model and the TUF verification boundary. It embeds `registry/schema-v3.schema.json`, applies JSON Schema Draft 2020-12 with format checking, rejects duplicate JSON object keys and unknown fields, enforces a 4 MiB registry/target limit, and checks provenance resolution, dates, duplicate IDs/indicators, hostname normalization, local port syntax, and the blocking-candidate safety constraints. The Python validator remains the canonical full registry authoring check; the Rust parser repeats the constraints needed before a client displays or searches a verified target.

The update API is explicit and offline: callers provide a local repository directory, bootstrap root bytes, the SHA-256 digest of those bytes from an independent trust source, a persistent cache path, and a time value. The only accepted target is `registry-v3.json`. HTTP support is disabled in the dependency features, and this API performs no network access. TUF metadata and targets have bounded sizes; repository and cache path components reject traversal and symlinks. The cache uses atomic replacement, syncs files before replacement and the containing directory on Unix, and verifies the persisted root, metadata versions, and target bytes before returning an accepted registry. This check is necessary because `sigstore-tuf` treats metadata-store write failures as best effort.

The SHA-256 pin is a trust input, not a self-authenticating convenience: an operator must obtain both the root and its digest through an independently authenticated Equalturn release channel. No production Equalturn root exists yet, so the CLI must not offer a default or silently accept an arbitrary remote root. Once the custodians complete the root ceremony, the public root and its fingerprint can be embedded in an application release. The current adapter is trust plumbing for local repositories and tests; it does not establish that the unconfigured publication repository is usable.

The persistent cache protects rollback floors from an ordinary mirror replay when the local cache remains intact. It does not protect against a local administrator, same-user code able to rewrite the cache, disk rollback, or a client whose clock is manipulated. Cache contents are public metadata, not secrets. Registry verification remains separate from enforcement.

## Offline CLI

`equalturn registry verify`, `inspect`, and `search` all accept a local TUF repository directory plus `--root PATH` and `--root-sha256 HEX`. They call the same verifier above on each invocation; inspection and search never parse an arbitrary registry file or skip TUF verification. The repository transport is directory-only and the `sigstore-tuf` HTTP feature is disabled. `--cache-dir PATH` selects persistent metadata state; otherwise the client uses `registry/trusted` below its configured data directory. The initial root bytes and lowercase 32-byte digest are required independently supplied trust inputs. No default root or remote root-discovery path exists.

The CLI emits stable failure statuses and a non-zero exit code. Expired metadata maps to `EXPIRED`; lower metadata versions map to `ROLLBACK`; role threshold or cryptographic verification failures map to `INVALID_SIGNATURE` (or `INVALID_ROOT` for the root role); root parsing/bootstrap failures map to `INVALID_ROOT`; target retrieval/integrity failures map to `INVALID_TARGET`; schema/semantic failures map to `INVALID_REGISTRY`; other TUF metadata failures map to `INVALID_METADATA`; local repository or persistent-cache failures map to `INVALID_REPOSITORY`. Unknown future TUF error variants fail closed as `INVALID_METADATA` rather than exposing dependency diagnostics as a stable interface. Successful operations print `VALID`, the registry version, entry/indicator counts, trusted root version, and target hash. Entry displays include provider, product, categories, surfaces, lifecycle, indicator provenance, confidence, actionability, false-positive risk, and limitations.

Until a release embeds a root and its independently authenticated fingerprint, the CLI is intended for local test repositories and operator-supplied local trust material. The separate publication repository remains unconfigured; there is no production root to distribute or use by default.

## Sources

- [TUF specification](https://theupdateframework.io/spec/) — protocol roles, threshold signatures, initial trust, rotation, rollback, and expiration; checked 19 October 2026.
- [TUF 1.0.37 specification](https://theupdateframework.github.io/specification/v1.0.37/) — currently reviewed versioned protocol text for root rotation, key thresholds, version rollback, and expiration; checked 19 October 2026.
- [TUF-on-CI README](https://github.com/theupdateframework/tuf-on-ci) and [repository maintenance manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/REPOSITORY-MAINTENANCE.md) — signing workflow, publication setup, online signing choices, and GitHub permissions; checked 19 October 2026.
- [TUF-on-CI changelog](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/CHANGELOG.md) — latest reviewed release notes; checked 19 October 2026.
- [TUF-on-CI template](https://github.com/theupdateframework/tuf-on-ci-template) and its [publication workflow](https://github.com/theupdateframework/tuf-on-ci-template/blob/main/.github/workflows/publish.yml), [signing workflow](https://github.com/theupdateframework/tuf-on-ci-template/blob/main/.github/workflows/signing-event.yml), and [online-sign workflow](https://github.com/theupdateframework/tuf-on-ci-template/blob/main/.github/workflows/online-sign.yml) — checked 19 October 2026 for repository initialization and job-scoped permissions.
- [`sigstore-tuf` 0.15.0 crate documentation](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/), [error taxonomy](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/error/enum.Error.html), and [Sigstore Rust repository](https://github.com/sigstore/sigstore-rust) — Rust client state machine, transport/cache boundaries, error categories, and repository interoperability notes; checked 19 October 2026.
- [`jsonschema` 0.59.0 documentation](https://docs.rs/jsonschema/0.59.0/jsonschema/) — Draft 2020-12 validator and opt-in format validation; checked 19 October 2026.
- [`tough` 0.24.0 documentation](https://docs.rs/tough/0.24.0/tough/) and [upstream repository](https://github.com/awslabs/tough) — TUF 1.0.0 support statement and known feature limits; checked 19 October 2026.
- [`tuf` 0.2.0 crate record](https://docs.rs/crate/tuf/0.2.0) — release age and beta/API stability caveat; checked 19 October 2026.
