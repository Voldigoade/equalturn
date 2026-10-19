# Repository metadata and settings

| Property | Value |
| --- | --- |
| Project | Equalturn |
| Repository | [Voldigoade/equalturn](https://github.com/Voldigoade/equalturn) |
| Visibility | Public |
| Default branch | `main` |
| Description | A privacy-first integrity environment for mutually agreed no-AI technical competitions. |

**Topics:** `ctf`, `competition-integrity`, `endpoint-security`, `privacy`, `rust`, `windows`, `linux`, `programming-contest`

Repository metadata and security settings were checked on 19 October 2026. Issues are enabled and Wiki is disabled. GitHub secret scanning and push protection are enabled. Dependabot vulnerability alerts return HTTP 204 (enabled), and private vulnerability reporting reports `enabled: true`. The repository API reports Dependabot security updates disabled; automatic dependency update PRs are not configured.

## Contents and development

The repository contains a Rust Cargo workspace and lockfile, CLI and relay applications, protocol/client/relay/report/registry libraries, PostgreSQL migrations, a versioned research registry, documentation, a local Compose PostgreSQL service, and GitHub Actions CI. It has no workstation enforcement service, GUI, registry enforcement logic, automatic updater, production registry root/release, or production relay hosting configuration. The registry CLI verifies only an explicitly selected local TUF repository.

The root [README](../README.md) contains local development and verification commands. Copy `.env.example` to `.env` for local PostgreSQL and replace the placeholder. Never commit `.env`, credentials, local database files, relay private keys, or participant key files. Docker Compose binds PostgreSQL to localhost; do not treat this development setup as a deployment recipe.

## CI and future repository policy

`.github/workflows/ci.yml` runs formatting, v2/v3 registry validation, deterministic target comparison, dependency audit, Clippy, and PostgreSQL-backed workspace tests on Linux; it also runs the full workspace suite on Windows. Actions use read-only repository permissions and pinned revisions. [Run 39308964800](https://github.com/Voldigoade/equalturn/actions/runs/39308964800) passed both jobs, including DPAPI tests.

Branch protection is not configured for this single-maintainer repository. Add review and force-push protections when there are multiple maintainers, while retaining a recovery path for urgent fixes. Before distributing binaries, define artifact signing, key rotation/recovery, update trust, and incident response.
