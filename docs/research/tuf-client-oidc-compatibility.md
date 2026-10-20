# TUF-on-CI Sigstore/OIDC compatibility with the Rust client

**Checked:** 19 October 2026
**Scope:** non-production `Voldigoade/equalturn-registry-staging` publication and Equalturn's pinned `sigstore-tuf` 0.15.0 verifier.

## Observed result

The staging publication succeeded through TUF-on-CI v0.21.0 and its Python TUF smoke test. The Rust live smoke test fails before metadata retrieval: loading the published `metadata/1.root.json` with `sigstore_tuf::TrustedMetadataSet::from_root` returns a JSON error for missing field `public`.

The published root uses the TAP-18 key shape:

```json
{
  "keytype": "sigstore-oidc",
  "scheme": "Fulcio",
  "keyval": {
    "identity": "…",
    "issuer": "…"
  }
}
```

TAP-18 extends TUF signatures with a Sigstore bundle. `sigstore-tuf` 0.15.0 models `keyval.public` as required and resolves only standard public-key signing schemes. Current upstream `sigstore-rust` main has the same required `public` field and standard scheme resolution. No supported TUF key-verification extension is exposed by this crate. This is a format and verifier capability mismatch, not a root pin, TLS, URL layout, or target-hash failure.

## Primary sources checked

- [TUF-on-CI signer setup](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-SETUP.md), accessed 19 October 2026: its Sigstore signer is experimental and may be incompatible with TUF client implementations.
- [TUF-on-CI online signing setup](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/ONLINE-SIGNING-SETUP.md), accessed 19 October 2026: automatic snapshot/timestamp signing includes Sigstore as an experimental method, alongside provider-specific KMS options.
- [TAP 18](https://github.com/theupdateframework/taps/blob/master/tap18.md), accessed 19 October 2026: defines `sigstore-oidc` keys and bundle verification; the proposal is still marked Draft.
- [`sigstore-tuf` 0.15.0 key model](https://github.com/sigstore/sigstore-rust/blob/main/crates/sigstore-tuf/src/key.rs), accessed 19 October 2026: `KeyVal.public` is required and only standard signing schemes are mapped.
- [TUF client conformance results](https://theupdateframework.github.io/tuf-conformance/), accessed 19 October 2026: the current Rust client passes the standard TUF client corpus; that corpus does not establish TAP-18 interoperability.

## Engineering conclusion

Do not claim the staging repository is verifiable by Equalturn. Do not parse and ignore the OIDC fields, skip signature verification, substitute a custom signature, shell out to another runtime, or add ad hoc certificate/transparency-log cryptography. Keep the Rust verifier fail-closed.

To close the gap, choose one path and prove it end to end before accepting the publication architecture:

1. use a signer/publication configuration that emits standard TUF key types supported by the pinned Rust verifier, including for root, targets, snapshot, and timestamp roles; or
2. adopt an upstream-supported Rust implementation of TAP-18, if one becomes available and passes focused verification tests; or
3. revise the client-language boundary only after a deliberate architecture review, including packaging and runtime costs.

The first option changes only non-production staging trust metadata but requires a compatible signing capability and explicit key custody. This document does not authorize creation or publication of an alternate signer identity. Production remains uninitialized.

## Trust distinction

Even after the technical compatibility issue is resolved, TUF only authenticates that a target was authorized by the pinned repository root and has the verified content hash. It does not prove that an AI product or indicator is correctly classified, safe to block, or complete.
