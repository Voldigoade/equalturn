# Registry publication status

**Checked:** 19 October 2026  
**Milestone:** 1.6 implementation in progress  
**Production status:** uninitialized; fail closed

## Baseline capture

| Item | Observed value |
| --- | --- |
| Baseline commit | `e07bdd4a36f524784cb96290d7662917861d5c59` |
| Development branch | `work/milestone-1.6-tuf-publication` |
| Default branch | `main` at the same baseline commit |
| Rust toolchain | `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)` |
| Workspace packages | 8 Rust packages: protocol, key protection, client core, relay core, report, registry, client CLI, relay CLI |
| Relay migrations | `crates/relay-core/migrations/0001_initial.sql` |
| GitHub Actions | One CI workflow with a Linux/PostgreSQL job and a Windows workspace test job; Linux runs `cargo audit`, formatting, both registry validators and Python fixtures, deterministic registry generation, Clippy, and workspace tests |
| v2 registry | 129 entries, 132 provenance records, 58 structured indicators |
| v3 registry | 131 entries, 84 indicators; semantic validator reported zero warnings |
| Rust tests | 116 passed, 0 failed across workspace test binaries; database-backed relay tests returned early because `TEST_DATABASE_URL` is absent locally |
| PostgreSQL/Docker | No local PostgreSQL server and no Docker executable; CI is the available real PostgreSQL integration environment |
| `cargo-audit` | Not installed. `cargo audit --file Cargo.lock` returned “no such command”; the prior installation attempt could not resolve crates.io. Existing CI installs `cargo-audit` 0.22.2 and runs it. |

Observed local baseline commands:

```text
cargo fmt --all -- --check                                         PASS
python3 tools/validate_registry.py                                  PASS
uv run --offline --with-requirements requirements-registry.txt \
  python tools/validate_registry_v3.py                              PASS
CARGO_BUILD_JOBS=1 cargo clippy --workspace --all-targets \
  --all-features --locked -- -D warnings                            PASS
CARGO_BUILD_JOBS=1 cargo test --workspace --locked                  PASS: 116 tests
cargo audit --file Cargo.lock                                       NOT RUN: cargo-audit unavailable
```

The unavailable local database and audit executable are environment limitations, not test passes. CI must exercise both in its configured environment.

## Current trust path

The registry library verifies local TUF repositories with an explicit bootstrap root and independent SHA-256 pin, and now also offers explicit HTTPS verification and installation APIs. The HTTP adapter rejects insecure or credential-bearing origins, disables redirects, sets bounded connect/read/overall timeouts, and relies on the TUF client for per-role streamed byte limits. HTTP transport errors are distinguished from malformed/missing repository metadata. The accepted target name remains fixed, registry v3 parsing performs schema and semantic validation, and metadata cache writes are checked against the verified update state. Installation uses a separately re-verifiable two-slot snapshot and atomic active pointer. No CLI update command or automatic/background registry contact exists yet.

The public `Voldigoade/equalturn-registry` repository exists but has no initialized root, role metadata, signed target, or published repository. Production trust must remain unavailable until a separate offline signer ceremony establishes a root that is independently pinned in a released client. The distinct public staging repository exists at `Voldigoade/equalturn-registry-staging`, created from the TUF-on-CI v0.21.0 template. Its approved GitHub/Sigstore OIDC signing ceremony generated a signed root and targets metadata in branch `sign/init` (commit `bc87d0550addb9aedb4e29bb7b2e1d21163854d9`); the first target was merged and published to GitHub Pages. This is a successful publication by TUF-on-CI's Python client, but the published metadata uses TAP-18 `sigstore-oidc` keys that the current Rust `sigstore-tuf` client cannot parse or verify. Therefore the live staging publication is not yet consumable by Equalturn's Rust verifier, and the publication/tooling choice remains open. Production trust remains uninitialized and fail-closed.

## Checkpoint updates

- Stage 1 baseline/research checkpoint: commit `3b122d2` (`docs: record milestone 1.6 TUF publication baseline`) was pushed to `work/milestone-1.6-tuf-publication`. The CI workflow only runs on pushes to `main` or pull requests targeting `main`, so this docs-only branch push did not start CI.
- Stage 2 production policy checkpoint: commit `3e775b5` (`docs: gate production registry trust on offline custody`) was pushed to `work/milestone-1.6-tuf-publication`. No production or staging private key has been generated, and no root is trusted by the client.
- Stage 3 staging repository checkpoint: `Voldigoade/equalturn-registry-staging` is public with default branch `main`; README commit `a7d062d` labels it explicitly non-production. GitHub Pages source is Actions and the default-domain HTTPS requirement is enabled. The `github-pages` environment deploys from `publish`. Actions allow only the TUF-on-CI actions, `actions/create-github-app-token`, and `actions/deploy-pages`, require full-length commit SHA pins, and retain read-only default `GITHUB_TOKEN` permissions. The upstream signing workflow's required pull-request creation permission is enabled only on this staging repository; no production setting was changed. No long-lived token, GitHub App, cloud KMS credential, or production secret was added.
- Stage 3 workflow status: the README-only push did not match any workflow trigger. The later staging signing/publication workflows ran as expected after the exact upstream action repositories were allowed while retaining full-SHA pinning.
- Stage 4 signing and publication: after explicit approval for staging-only GitHub/Sigstore signing and a separate approval to include the GitHub noreply identity in Sigstore's immutable public transparency log, TUF-on-CI generated root metadata on `sign/init` (`bc87d0550addb9aedb4e29bb7b2e1d21163854d9`) and a registry target on `sign/add-registry-v3`; the target branch was merged to staging `main` at `6ec076a42ae6313461fde99bf1b7418add02724e`. TUF-on-CI online signing run [39324623632](https://github.com/Voldigoade/equalturn-registry-staging/actions/runs/39324623632) succeeded. Publication run [39324676740](https://github.com/Voldigoade/equalturn-registry-staging/actions/runs/39324676740) succeeded for build, Pages deployment, Python TUF smoke-test and issue update. GitHub Pages serves root metadata with SHA-256 `d1e08a932e9a07c959a896dd818c5d16fcb84ce0824b9f5a155b025a89c7b048` and target `registry-v3.json` with SHA-256 `e6ccf57eca33951cd6b7a04e7e609f981c87c73298b7923b4d5a9a845caa02a0` (152,438 bytes; 131 entries). The root uses TAP-18 `sigstore-oidc` keys; the pinned Rust verifier rejects it because its TUF key model requires ordinary public-key material and does not implement TAP-18. This makes the staging publication evidence useful but not an accepted Rust trust path. No private signing key was committed.
- Stage 5 HTTPS transport: commit `1d90053` (`feat: add bounded HTTPS TUF repository transport`) is on `work/milestone-1.6-tuf-publication` and pushed to origin. `equalturn-registry` exposes explicit HTTPS repository verification through the pinned `sigstore-tuf` client, with redirects disabled, 10/30/60-second timeouts, URL-origin validation, bounded downloads and transport errors mapped to repository unavailability. The adapter maps the configured repository root to TUF-on-CI's `/metadata/` and `/targets/` paths. Loopback tests cover path mapping, refused redirects and both Content-Length and chunked response bounds. Focused test result: 41 passed across the registry crate; `cargo clippy -p equalturn-registry --all-targets --all-features --locked -- -D warnings` passed. This code commit did not change CI configuration; the branch workflow runs only on main pushes or pull requests targeting main.
- Stage 6 local installed state: implementation and focused review are complete locally; checkpoint commit/push is pending. Explicit local/HTTPS update APIs serialize refreshes, build a bounded candidate repository snapshot in a temporary directory, replay TUF from the pinned bootstrap root, and atomically select one of two installed slots only after offline re-verification. The pointer binds the root pin, target digest, registry semantic version, and final root version. Installed reads recheck the signature chain, expiry, target hash, and registry schema/semantics. Tests cover reopen/offline verification, failed semantic update preserving the previous install, installed target tampering, pointer/metadata symlinks, consistent-snapshot filenames, root rotation replay, and store-root symlink rejection. Full `equalturn-registry` tests: 49 passed, 0 failed, 1 staging smoke test ignored because the live OIDC root is not Rust-compatible; focused Clippy with warnings denied and `cargo fmt --all -- --check` passed. Initial sandboxed loopback test run failed because socket binding is denied; rerunning the same suite with narrowly scoped permission for ephemeral loopback sockets passed. No external network, firewall, DNS, route, or proxy settings were changed.

Milestone 1.6 stage commits, pushed branch heads, publication artifacts, CI runs, and live staging verification are recorded here as each checkpoint completes. The milestone remains in progress until the remaining local update-store, CLI, attack-test, CI, and compatible staging trust-path criteria pass.

