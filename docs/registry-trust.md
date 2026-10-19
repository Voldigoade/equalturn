# Trusted registry distribution: tooling decision

**Decision date:** 19 October 2026  
**Status:** architecture selected; repository publication and client integration are subsequent stages. No signed production registry is published by this change.

## Decision

Use **TUF-on-CI for repository publication and signing workflow**, with **`sigstore-tuf` 0.15.0 as the Rust verification client**. The registry remains an opaque TUF target: TUF authenticates the bytes and metadata chain; the existing v3 JSON Schema and semantic validator separately validate the target's content. There is no Equalturn-specific signature format or custom update protocol.

TUF is the right trust model for a public, infrequently updated catalogue mirrored through ordinary hosting. Its root, targets, snapshot, and timestamp roles limit the effect of a single online-key or hosting compromise; threshold signatures, metadata versions and expiry address distinct key-compromise, rollback, mix-and-match, and freeze risks. Clients must begin with a root obtained out of band and walk authorized root rotations. The [TUF specification](https://theupdateframework.io/spec/) is the protocol authority.

## Current implementation evidence checked

| Component | Observed state on 19 October 2026 | Relevance |
| --- | --- | --- |
| [TUF specification](https://theupdateframework.io/spec/) | Defines the four top-level roles, threshold verification, pinned initial root, versioned root rotation, and metadata expiry. | Use the standard role and client workflows; do not sign arbitrary serialized JSON. |
| [TUF-on-CI](https://github.com/theupdateframework/tuf-on-ci) | Active upstream repository; repository docs describe guided distributed signing, threshold delegations, hardware keys and online signing providers. The latest reviewed changelog entry was v0.15.0. | Provides a maintained publication/signing workflow instead of hand-written GitHub Actions cryptography. Its own docs recommend a repository created from its template, GitHub Pages using Actions, and the `publish` environment/branch arrangement. |
| [`sigstore-tuf` 0.15.0](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/) | Current docs describe a pure-Rust client, a transport-free `TrustedMetadataSet`, the `Updater`, offline `Repository` implementations, size bounds, root chaining, expiry/version/hash checks, and a re-verified metadata cache. | Fits the Rust application boundary and enables deterministic local/in-memory attack tests without network timing. |
| [`tough` 0.24.0](https://docs.rs/tough/0.24.0/tough/) / [`tuftool`](https://github.com/awslabs/tough) | Active Rust client and repository editor; documentation says it targets TUF 1.0.0 with documented exceptions for delegated roles and TAP 3/4. Upstream was pushed in September 2026. | Strong Rust-native alternative for local repository creation and verification, but it does not provide TUF-on-CI's distributed signing-event and GitHub publication workflow. Its key-ID handling also has a documented interoperability concern for some `tuf-on-ci` metadata extensions. |
| [`tuf` 0.2.0](https://docs.rs/crate/tuf/0.2.0) | Stable release dates from 2017; the current 0.3 line remains beta and the crate documentation warns that its API is unstable. | Rejected for a new client despite its historical connection to the TUF project. |

The repository versions and docs above were checked on the stated date. The `sigstore-rust` repository was active that day. Its TUF crate is a relatively new, focused implementation rather than a long-established general-purpose Rust library; keep it behind a small adapter, pin it in `Cargo.lock`, and exercise the complete attack suite before treating any update as usable. The crate documents interoperability details such as TUF canonical JSON and declared key IDs; those details are particularly relevant to metadata emitted by TUF-on-CI.

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

## Security boundary and constraints

- TUF proves that a target is authorized by the configured repository's metadata chain and has the expected length/hash. It does not prove an indicator is true, current, correctly scoped, or safe to block.
- The JSON Schema and semantic validator remain mandatory after TUF verification. A malicious but correctly signed registry target still requires trusted review/signing governance; TUF does not make signed content benign.
- The application must pin an initial root in the released application or another explicit out-of-band trust store. It must never accept a root merely because it came from the same untrusted mirror as the target.
- The client will refresh only on an explicit registry command/API call. Session startup, relay connection, and competition mode will not silently fetch a registry.
- Target paths are fixed logical names, not server-selected filesystem paths. Downloads have per-role and total byte limits and are verified before parsing or writing to the final cache path.
- No production root keys, signing keys, GitHub secrets, or repository credentials are created or committed during this tooling decision. A real threshold root ceremony and its signer set are required before publishing a production trusted repository.
- The existence of `Voldigoade/equalturn-registry` does not mean the TUF repository is initialized or a release is available. A client must fail closed until an out-of-band root is pinned and the complete metadata chain is valid.

## Sources

- [TUF specification](https://theupdateframework.io/spec/) — protocol roles, threshold signatures, initial trust, rotation, rollback, and expiration; checked 19 October 2026.
- [TUF-on-CI README](https://github.com/theupdateframework/tuf-on-ci) and [repository maintenance manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/REPOSITORY-MAINTENANCE.md) — signing workflow, publication setup, online signing choices, and GitHub permissions; checked 19 October 2026.
- [TUF-on-CI changelog](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/CHANGELOG.md) — latest reviewed release notes; checked 19 October 2026.
- [TUF-on-CI template](https://github.com/theupdateframework/tuf-on-ci-template) and its [publication workflow](https://github.com/theupdateframework/tuf-on-ci-template/blob/main/.github/workflows/publish.yml), [signing workflow](https://github.com/theupdateframework/tuf-on-ci-template/blob/main/.github/workflows/signing-event.yml), and [online-sign workflow](https://github.com/theupdateframework/tuf-on-ci-template/blob/main/.github/workflows/online-sign.yml) — checked 19 October 2026 for repository initialization and job-scoped permissions.
- [`sigstore-tuf` crate documentation](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/) and [Sigstore Rust repository](https://github.com/sigstore/sigstore-rust) — Rust client state machine, transport/cache boundaries, and repository interoperability notes; checked 19 October 2026.
- [`tough` 0.24.0 documentation](https://docs.rs/tough/0.24.0/tough/) and [upstream repository](https://github.com/awslabs/tough) — TUF 1.0.0 support statement and known feature limits; checked 19 October 2026.
- [`tuf` 0.2.0 crate record](https://docs.rs/crate/tuf/0.2.0) — release age and beta/API stability caveat; checked 19 October 2026.
