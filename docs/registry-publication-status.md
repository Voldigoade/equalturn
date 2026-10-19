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

At baseline, `crates/registry` verifies a local TUF repository with an explicit bootstrap root and independent SHA-256 pin. TUF metadata/target sizes are bounded, the accepted registry target name is fixed, registry v3 parsing performs schema and semantic validation, and metadata cache writes are checked against the verified update state. The HTTP client feature is not enabled. There is no remote `registry update` command.

The public `Voldigoade/equalturn-registry` repository exists but has no initialized root, role metadata, signed target, or published repository. Production trust must remain unavailable until a separate offline signer ceremony establishes a root that is independently pinned in a released client. A distinct staging repository is being evaluated for exercising publication and remote verification without creating production trust material.

## Checkpoint updates

Milestone 1.6 stage commits, pushed branch heads, publication artifacts, CI runs, and live staging verification will be recorded here as each checkpoint completes. This file is not evidence of publication until the relevant observed values are added.

