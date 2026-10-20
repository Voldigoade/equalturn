# TUF publication and Rust client review

**Checked:** 19 October 2026<br>
**Scope:** TUF metadata specification, TUF-on-CI publication/signing, and the existing Rust TUF client boundary.

## Verified upstream facts

- The TUF specification site lists [v1.0.37](https://theupdateframework.github.io/specification/v1.0.37/) as its latest versioned specification. TUF's pinned-root bootstrap and root-version chain protect the client from accepting a replacement root solely because it arrived from the repository. Timestamp and snapshot metadata bind freshness and consistent repository state; expiry and version floors address freeze and rollback attempts, subject to the client's persisted state and usable clock.
- The [TUF-on-CI changelog](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/CHANGELOG.md) lists **v0.21.0** as the latest release and has an Unreleased section. The project describes a TUF repository/signing workflow with signing events, threshold delegations, hardware-backed signers, and online-signing integrations. Its [repository maintenance manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/REPOSITORY-MAINTENANCE.md) and [template](https://github.com/theupdateframework/tuf-on-ci-template) are the operational references; the template should be copied and reviewed rather than recreating its workflow from memory.
- [`sigstore-tuf` 0.15.0](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/) is the pinned workspace version. Its client module is feature-gated. `HttpRepository` accepts a caller-supplied `reqwest::Client`; its default transport configures 30-second connect, 60-second read, and 120-second overall timeouts. Every repository fetch receives a maximum byte length, and the transport enforces that bound while streaming as well as against a declared `Content-Length`.
- [`sigstore-tuf` transport documentation](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/transport/index.html) defines a pluggable repository boundary and per-fetch byte limits. Its client/cache documentation describes verification through the pinned root and re-verification of persisted metadata. The crate re-exports the `reqwest` used by its HTTP client when the client feature is enabled.
- The current crate source in the local Cargo registry confirms that the built-in `HttpRepository` uses the default `reqwest` redirect policy unless the caller injects a client. The current Equalturn workspace has `sigstore-tuf` client support disabled and only opens local `DirectoryRepository` objects. Enabling remote retrieval therefore requires an explicit feature change and an Equalturn-owned HTTP policy.

## Engineering interpretation

1. TUF remains the only metadata/target authentication protocol. Equalturn should not add a parallel registry signature format, custom root discovery, or trust-on-first-use.
2. The first root must be embedded or otherwise authenticated out of band. A remote root plus remote metadata fetched from the same origin is not a trust bootstrap.
3. The remote updater must set redirects to `none`; otherwise an HTTPS origin could redirect the client to a different host or to plaintext HTTP before TUF checks the downloaded bytes. Equalturn must also reject base URLs with user information, query, or fragment components and use a fixed registry target name.
4. `sigstore-tuf` already provides the relevant TUF state machine and bounded streaming. Equalturn's adapter should add origin policy, explicit timeouts, safe cache installation, registry semantic validation, and stable user-facing statuses without reimplementing TUF verification.
5. A correct TUF signature proves authorization under a configured key policy and byte integrity. It does not prove an indicator is factual, current, or safe to enforce.
6. A cached version floor only protects against rollback if it remains durable and bound to the same pinned root. The client must fail closed when cache state is unreadable or inconsistent; it must not delete a suspicious cache and silently restart from the old bootstrap root.

## Decision and limitations

Retain TUF-on-CI as the publication workflow and `sigstore-tuf` 0.15.0 as the Rust verifier, behind the existing `equalturn-registry` boundary. Use the upstream template at its current revision for an isolated staging repository. Do not grant the ordinary Equalturn build workflow write access to registry publication state.

Current implementation evidence is local and bounded: `crates/registry/src/trust.rs` accepts a local repository path, an explicit root byte slice and SHA-256 pin, and a metadata cache path. `sigstore-tuf`'s HTTP feature is disabled in `Cargo.toml`. There is not yet a remote update command or a successfully published staging root at this baseline.

Context7 was queried for the Rust crate but did not provide a Rust-specific `sigstore-tuf` library entry. The versioned docs.rs pages, the pinned crate source, and TUF/TUF-on-CI primary documentation were used instead. This is a documentation discovery limitation, not evidence that the library is unmaintained.

## Sources

- [TUF v1.0.37 specification](https://theupdateframework.github.io/specification/v1.0.37/) — current versioned specification page listed upstream on the check date.
- [TUF-on-CI repository](https://github.com/theupdateframework/tuf-on-ci), [changelog](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/CHANGELOG.md), [repository maintenance manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/REPOSITORY-MAINTENANCE.md), and [template](https://github.com/theupdateframework/tuf-on-ci-template) — publisher status and repository/signing operations.
- [`sigstore-tuf` 0.15.0 API docs](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/), [client docs](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/client/index.html), and [transport docs](https://docs.rs/sigstore-tuf/0.15.0/sigstore_tuf/transport/index.html) — Rust API and bounded transport behavior.
