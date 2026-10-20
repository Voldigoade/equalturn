# Production registry root ceremony

**Status:** production key generation is not authorized or started.<br>
This runbook describes the future owner-operated gate. It is not a request to create keys in this checkout.

The current staging repository is a fixed-content standard-key Pages publication used only for Rust-client interoperability. It does not sign or renew metadata. Its key material is disposable. The earlier TAP-18 Sigstore/OIDC root is incompatible with the pinned Rust verifier. Neither staging state is production trust.

## Before the owner gate

Complete the following without production secrets:

1. Recheck the current Equalturn and `equalturn-registry` source/default branches and current TUF-on-CI version.
2. Confirm production is initialized from the upstream TUF-on-CI template, but do not run `sign/init` yet.
3. Review the role thresholds, expiry/resign windows, PIV device/key type, role-key separation, encrypted recovery method, cloud KMS provider/costs, GitHub settings and OIDC claims with the owner.
4. In a disposable repository, exercise the exact TUF-on-CI signer setup and role configuration with standard TUF keys, then verify root bootstrap and repository metadata using Equalturn's Rust verifier. Test role renewal, root and targets rotation, target tamper, expiration, and recovery. Do not use production key material in the rehearsal.
5. Confirm the reviewed Equalturn application release channel that will carry the initial root bytes and digest. If GitHub is the incident under investigation, obtain the expected root fingerprint through another authenticated channel.
6. Confirm GitHub Actions defaults, action allowlist/full-SHA pinning, signing-event permissions, Pages source/environment, publication branch, ruleset/branch protection, and exact OIDC trust conditions. The repository API access used for this review could not read Pages, Actions, or branch-protection settings.

If the signer, recovery method, compatible online roles, or release channel cannot be demonstrated, defer production root creation. Never import or reuse staging keys.

## Owner-operated initialization and release

1. Freeze the reviewed Equalturn source commit. Build `registry-v3.json` with `tools/build_registry_release.py` and create the deterministic source-commit/digest/review record. Confirm schema and semantic validation and review every warning.
2. Using the owner-controlled PIV signer environment and the TUF-on-CI signer tool, run `tuf-on-ci-delegate sign/init` in a clean clone of `Voldigoade/equalturn-registry`. The owner must personally review the signing tool's proposed root, thresholds, roles, expiry, repository identity, and key IDs. Codex must not receive private key bytes or output that could contain them.
3. Keep root and targets under human PIV custody, using distinct standard TUF keys. Under the recommended initial single-custodian policy, thresholds remain 1-of-1; this is not independent multi-party approval. If the owner instead confirms an actual independent signer group, encode its real threshold and custody rather than synthesizing extra identities.
4. Configure the separately authorized OIDC-to-KMS path for snapshot and timestamp with separate role keys and least-privilege repository/workflow/branch/environment claims. Do not store static private role keys or cloud credentials in GitHub Secrets. If the provider has not been authorized/provisioned, do not publish a feed that nobody can renew.
5. Complete the TUF-on-CI signing event only after the target source commit, digest, size, semantic version, counts, warnings, and human review are recorded. The signing workflow must use only the TUF-on-CI implementation and pinned upstream actions.
6. From a fresh client, fetch only `https://voldigoade.github.io/equalturn-registry/` and verify root through timestamp, snapshot, targets, target digest, schema, and semantics with Equalturn's Rust client. Rebuild from the immutable source commit and compare target bytes exactly.
7. Pin the exact PUBLIC root bytes and their independently computed SHA-256 into Equalturn's production trust module. Add tests proving staging root, URL, cache, and metadata cannot replace production trust. Never include private material in the release.
8. Run the clean production update, offline reopen, expiry, rollback, and target-tamper smoke. Record public role IDs/fingerprints, metadata versions/expiries, root digest, target digest/size, source commit, publication commit, and workflow run.

## Key backup and recovery rehearsal

The public key IDs/fingerprints identify what recovery restored; they are not secrets. Keep the private recovery copy encrypted with a separately controlled passphrase and in a different physical/storage location where feasible. For a PIV-based key, do not assume a non-exportable token can be backed up: use only an upstream-supported import/recovery procedure verified in a disposable TUF-on-CI repository. Do not make an untracked backup in the development environment. Restore the disposable recovery copy into a clean test signer, sign a test role operation, and verify it with the rehearsal root. Destroy only rehearsal material after tests and confirm no secret entered logs, commits, or artifacts.

If the chosen signer has no tested recovery path, root loss requires shipping a new initial root through an independently authenticated application release; the existing TUF chain cannot recover itself. If the root is compromised, stop publication and use a new authenticated app trust anchor. Never accept a replacement root merely because it is served by the existing endpoint.

## Online renewal

TUF-on-CI's scheduled online signing renews snapshot/timestamp using the configured supported KMS identity. A separate credential-free production monitor should use the pinned root and Rust verifier to check role expiry well before the timestamp/targets/root thresholds; it must fail on missing, expired, tampered, or unverifiable metadata and remain separate from PR CI. Root renewal and all root-key changes remain explicit human signing events. Keep the last known-good client install intact after network or partial-update failure, but consider it valid only while the signed metadata remains unexpired and re-verifiable.

## Upstream runbooks

- [TUF-on-CI repository maintenance](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/REPOSITORY-MAINTENANCE.md)
- [TUF-on-CI signer setup](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-SETUP.md)
- [TUF-on-CI signer manual](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/SIGNER-MANUAL.md)
- [TUF-on-CI online signing](https://github.com/theupdateframework/tuf-on-ci/blob/main/docs/ONLINE-SIGNING-SETUP.md)
