# Trusted registry distribution: tooling decision

**Decision date:** 19 October 2026
**Status:** local Rust verification, explicit HTTPS transport, and offline CLI are implemented. No production root or production release exists. The non-production TUF-on-CI publication has been exercised, but its Sigstore/OIDC TAP-18 keys are not supported by the selected Rust client, so it is not currently an accepted client trust path.

## Decision

Use TUF for registry distribution, with `sigstore-tuf` 0.15.0 as the Rust verification client for standard TUF keys. TUF-on-CI v0.21.0 has been exercised in a non-production staging repository using its Sigstore/OIDC signer, but this produces TAP-18 metadata the Rust client does not support. The publication tool and signer mode therefore remain a conditional decision until the full staging-to-Rust verification path is compatible. Do not add a custom signature format, bypass TUF verification, or treat the current live staging target as trusted by Equalturn.

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

The client now exposes an explicit `verify_https_repository` path. It requires an HTTPS URL with a host and rejects user information, query strings, and fragments. Its `sigstore-tuf` HTTP adapter has redirects disabled and 10-second connect, 30-second read, and 60-second total timeouts; it also enforces the TUF fetch-size limits while streaming. A transport failure is reported as repository unavailable, while missing required metadata remains invalid metadata. The pre-existing local directory repository path remains available for offline fixtures. Calling this API is explicit; it does not cause session creation or normal CLI use to contact the registry.

The live TUF-on-CI staging repository currently has the TAP-18 `sigstore-oidc` key type. TUF-on-CI's upstream signer setup guide labels Sigstore signing experimental and warns it may not interoperate with all TUF clients. `sigstore-tuf` 0.15.0 requires `keyval.public` and only resolves standard cryptographic schemes; an explicit Rust smoke test against the pinned public staging root fails parsing with a missing `public` field. Current upstream `sigstore-rust` source retains the same key model. TAP-18 is still marked Draft. See [the dated compatibility review](research/tuf-client-oidc-compatibility.md). This mismatch is a real integration gap; TUF signature checks are not weakened to make staging pass.

## Alternatives considered

### A. TUF-on-CI publication + maintained Rust verifier — conditional

This separates the signing ceremony and release workflow from registry semantics. Reviewers can approve a normal data change while authorized signers participate in the TUF signing event; GitHub Pages or a mirror carries only public metadata and target bytes. However, the currently tested TUF-on-CI Sigstore mode emits TAP-18 signatures the Rust verifier cannot process. This option is not accepted for Equalturn until a supported signer/client combination is demonstrated end to end.

### B. Rust-native publication and verification with `tuftool`/`tough`

This keeps metadata generation in Rust and has active upstream maintenance. It would require Equalturn to maintain the signer invitation, threshold collection, role rotation, CI permissions, and publish workflow around the CLI. That is avoidable operational security code. The client can also have interoperability friction with repositories whose key IDs include TUF-on-CI-specific key fields; the `sigstore-tuf` documentation describes why it accepts declared IDs for interoperability.

### C. Reference Python TUF publication/client

Python-TUF is an important reference implementation and TUF-on-CI itself uses it, but a Python runtime in the Equalturn client would add a second production language and packaging surface. Use the Python implementation as a publisher dependency inside TUF-on-CI, not as a runtime dependency of the Rust CLI.

### D. Older `tuf` Rust crate

The published stable version is stale for a new security boundary; a beta release with an unstable API is not a better fit than the active alternatives above.

## Publication repository layout

**Decision:** keep reviewed source data in `Voldigoade/equalturn`. Reserve [`Voldigoade/equalturn-registry`](https://github.com/Voldigoade/equalturn-registry) as the future production publication endpoint; it remains uninitialized. Use the distinct [`Voldigoade/equalturn-registry-staging`](https://github.com/Voldigoade/equalturn-registry-staging) as a non-production publication experiment, created from the upstream [`tuf-on-ci-template`](https://github.com/theupdateframework/tuf-on-ci-template). This avoids giving application build and test workflows metadata publication permissions. The staging repository is not an accepted production release surface while its signer format is incompatible with the Rust client.

- **Source of truth and review:** `registry/registry-v3.json`, its schema, semantic validator, and research sources in Equalturn. Registry changes are reviewed and validated there first.
- **Generated target:** deterministic output `registry-v3.json`, published as a TUF target. A release change copies the generated bytes into a TUF signing event; nobody edits that target by hand.
- **TUF metadata:** role metadata under `metadata/` in the publication repository. TUF-on-CI creates signing events and updates metadata through its standard workflow; its current staging Sigstore signer format is not supported by the Rust client.
- **Published location:** the template's Pages workflow builds from a ref, deploys to GitHub Pages, and smoke-tests the deployed repository. The upstream maintenance instructions call for Pages source `GitHub Actions` and a `github-pages` environment deployment branch of `publish`.
- **Initial trust anchor:** after a root ceremony, its public `root.json` will be pinned in the Equalturn release/application trust store. The application must not fetch the initial root from Pages. Root private keys remain offline and are never stored in either repository.

The production repository `Voldigoade/equalturn-registry` is public and remains uninitialized: no root, signer roles, TUF metadata, target, or release workflow has been established there. The separate staging repository was created from the template on 19 October 2026, has default branch `main`, and has an initialized TUF-on-CI root and published target. Its Pages deployment, GitHub Actions signing/publication workflow, and staging-only pull-request permission were exercised. The TUF-on-CI workflows declare empty workflow-level permissions, grant needed permissions at job scope, and pin third-party actions to commit SHAs. The staging online-sign job uses `id-token: write`, `contents: write`, and `actions: write`; production must not copy those permissions without a separate signer design and protected workflow review.

The split adds a second repository and a release-copy step, but creates a useful permission boundary: ordinary Rust CI and research PRs cannot publish trusted metadata. Generated target bytes can be traced to an approved Equalturn source revision. Publication automation must record that source commit in the signing-event PR or release notes.

## Production role policy and key custody

Production trust is **not initialized**. The public Git history currently attributes its commits to one account; it does not establish a roster of independent custodians. No production `root`, `targets`, `snapshot`, or `timestamp` key has been created or configured. Do not create a production root until three independently accountable maintainers accept distinct roles and custody arrangements. The proposed steady-state policy, which is conditional and not a current capability, is documented in [registry-production-trust.md](registry-production-trust.md).

Thresholds and lifetimes are Equalturn design choices, not TUF requirements. TUF verifies role signature thresholds, freshness, version progression, root rotation, and target hashes according to the currently trusted root. See the [TUF v1.0.37 specification](https://theupdateframework.github.io/specification/v1.0.37/) checked 19 October 2026.

The intended production endpoint is `https://voldigoade.github.io/equalturn-registry/`; it is not yet a usable signed repository. The exact bootstrap and custody ceremony is in [production-root-ceremony.md](production-root-ceremony.md). An application build must fail closed until an out-of-band authenticated root is embedded or explicitly provisioned by a trusted release. A root fetched from that same endpoint cannot bootstrap its own trust.

The isolated staging repository currently uses one approved Sigstore OIDC identity and a staging-only root. That root and its signatures are not production trust and must not be embedded in production builds. TUF-on-CI is the repository workflow; its [signer manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-MANUAL.md) describes signing events and local signer prompts. The Rust verifier compatibility gap is documented above; staging must remain untrusted by Equalturn until resolved.

### Threats and response

| Threat | What TUF / policy contains | Residual risk and response |
| --- | --- | --- |
| GitHub or mirror compromise | A mirror cannot change signed metadata or target bytes without a trusted signing threshold; the eventual root pin is distributed out of band. | Hosting can deny service or replay metadata only until expiry. Restore hosting and publish fresh metadata; compare target hashes to reviewed source. |
| Malicious registry PR or compromised source maintainer | CI validation and registry review precede an independent multi-signer target event once the required production custodian roster exists. | A malicious but correctly signed entry remains possible; correct it in a reviewed release. TUF does not evaluate indicator truth or block safety. |
| Signer compromise | A future 2-of-3 policy keeps one stolen offline signer below threshold. | Threshold compromise authorizes a takeover or malicious target; revoke through a verified root rotation and communicate out of band. Staging's single signer is deliberately not suitable for production. |
| Snapshot or timestamp key compromise | Separate online roles cannot sign top-level targets under the proposed role separation. | They can disrupt metadata freshness or cause denial of service; revoke/rotate the affected role through authorized metadata. |
| Freeze / rollback / mix-and-match | Expiry, monotonically increasing metadata versions, snapshot hashes/lengths, consistent snapshots, and persistent client metadata state reject stale or inconsistent state. | A client that never checks for updates cannot learn a new release exists; show the last check and expiry. If rollback state is lost or unreadable, fail closed rather than silently resetting the floor. |
| Root key loss or threshold compromise | Future independent offline custody, expiry monitoring, recovery copies, and a documented root rotation. | Loss of the root quorum is unrecoverable inside the existing trust chain. Root compromise requires a new authenticated application trust anchor and retirement of the old endpoint. |
| Malicious valid entry or wrong classification | Provenance, semantic validation, review, and explicit indicator limitations make decisions inspectable. | Signatures establish authorization, not factual accuracy, recency, or safe enforcement. No registry indicator is executable policy. |

Production release gates are: three independently accountable custodians; successful dry-run root rotation; protected repository and publication environment; narrowly scoped online-role identities; reviewed source commit and deterministic target hash; target signature threshold; published repository smoke test; and a release that distributes the independently verified initial root. These gates are unmet. A future publication implementation or a staging test does not satisfy them.

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

`crates/registry` provides a typed, read-only v3 registry model and the TUF verification boundary. It embeds `registry/schema-v3.schema.json`, applies JSON Schema Draft 2020-12 with format checking, rejects duplicate JSON object keys and unknown fields, enforces a 4 MiB registry/target limit, and checks provenance resolution, dates, duplicate IDs/indicators, hostname normalization, local port syntax, and the blocking-candidate safety constraints. The Python validator remains the canonical full registry authoring check; the Rust parser repeats the constraints needed before a client displays or searches a verified target.

The local update API remains explicit and offline: callers provide a local repository directory, bootstrap root bytes, an independent SHA-256 root pin, a persistent metadata cache, and a time value. A separate explicit `verify_https_repository` API accepts only clean HTTPS origins and applies redirect, timeout, and size limits. The only accepted target is `registry-v3.json`. TUF metadata and targets have bounded sizes; repository and cache path components reject traversal and symlinks. The cache uses atomic replacement, syncs files before replacement and the containing directory on Unix, and verifies the persisted root, metadata versions, and target bytes before returning an accepted registry. This check is necessary because `sigstore-tuf` treats metadata-store write failures as best effort. The explicit HTTPS API does not cause session creation, normal CLI use, or background work to contact a registry.

The SHA-256 pin is a trust input, not a self-authenticating convenience: an operator must obtain both the root and its digest through an independently authenticated Equalturn release channel. No production Equalturn root exists yet, so the CLI must not offer a default or silently accept an arbitrary remote root. Once the custodians complete the root ceremony, the public root and its fingerprint can be embedded in an application release. The current adapter is trust plumbing for local repositories and tests; it does not establish that the unconfigured publication repository is usable.

The persistent cache protects rollback floors from an ordinary mirror replay when the local cache remains intact. It does not protect against a local administrator, same-user code able to rewrite the cache, disk rollback, or a client whose clock is manipulated. Cache contents are public metadata, not secrets. Registry verification remains separate from enforcement.

The registry crate also exposes explicit local and HTTPS update APIs that install a separately re-verifiable snapshot under a caller-selected state directory. A snapshot is assembled in a temporary directory, re-opened through the same pinned-root TUF verifier, and switched into a two-slot store only after target/schema validation succeeds. The small versioned `current.json` pointer binds the bootstrap-root digest, target digest, registry version, final root version, and last successful refresh time; it is replaced atomically after file and directory synchronization. The previously selected slot remains available until a later successful update reuses it. Installed-state loads repeat TUF signature, expiry, root-rotation, target-hash, and registry semantic checks offline. A failed or interrupted candidate build does not select that candidate, and transport/cache metadata churn does not overwrite the installed snapshot. Update attempts sharing this API are serialized with a filesystem lock; readers lock the pointer and selected slot while verifying.

The installed snapshot and pointer are local public data, not an anti-tamper boundary against the workstation owner. A same-user attacker can modify or restore local state; TUF detects modifications, but ordinary files cannot provide hardware-backed monotonic rollback protection. If installed files are missing, malformed, expired, or fail verification, loading fails closed. The old slot is only a recovery copy, not a claim that its metadata remains fresh.

## Offline CLI

`equalturn registry verify`, `inspect`, and `search` accept a local TUF repository directory plus `--root PATH` and `--root-sha256 HEX`. They call the same local verifier above; inspection and search never parse an arbitrary registry file or skip TUF verification. The CLI remains directory-only; remote registry update commands are not implemented. `--cache-dir PATH` selects persistent metadata state; otherwise the client uses `registry/trusted` below its configured data directory. The initial root bytes and lowercase 32-byte digest are required independently supplied trust inputs. No default root or remote root-discovery path exists.

The CLI emits stable failure statuses and a non-zero exit code. Expired metadata maps to `EXPIRED`; lower metadata versions map to `ROLLBACK`; role threshold or cryptographic verification failures map to `INVALID_SIGNATURE` (or `INVALID_ROOT` for the root role); root parsing/bootstrap failures map to `INVALID_ROOT`; target retrieval/integrity failures map to `INVALID_TARGET`; schema/semantic failures map to `INVALID_REGISTRY`; other TUF metadata failures map to `INVALID_METADATA`; local repository or persistent-cache failures map to `INVALID_REPOSITORY`. Unknown future TUF error variants fail closed as `INVALID_METADATA` rather than exposing dependency diagnostics as a stable interface. Successful operations print `VALID`, the registry version, entry/indicator counts, trusted root version, and target hash. Entry displays include provider, product, categories, surfaces, lifecycle, indicator provenance, confidence, actionability, false-positive risk, and limitations.

Until a release embeds a root and its independently authenticated fingerprint, the CLI is intended for local test repositories and operator-supplied local trust material. The production publication repository remains unconfigured; there is no production root to distribute or use by default. A separate public staging repository at `Voldigoade/equalturn-registry-staging` has a published root and target, but the root's TAP-18 signer format is incompatible with the pinned Rust verifier. It must never be accepted as production trust.

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
