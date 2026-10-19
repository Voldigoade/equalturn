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

The repository contains a Rust Cargo workspace and lockfile, two applications (CLI and relay), protocol/client/relay/report libraries, a PostgreSQL migration, a versioned research registry, protocol/architecture/privacy/threat-model documents, a local Compose PostgreSQL service, an environment example, and GitHub Actions CI. It has no workstation enforcement service, GUI, registry enforcement logic, release/update system, or production hosting configuration.

The root [README](../README.md) contains local development and verification commands. Copy `.env.example` to `.env` for local PostgreSQL and replace the placeholder. Never commit `.env`, credentials, local database files, relay private keys, or participant key files. Docker Compose binds PostgreSQL to localhost; do not treat this development setup as a deployment recipe.

## CI and future repository policy

`.github/workflows/ci.yml` runs formatting, registry validation, dependency audit, Clippy, and PostgreSQL-backed workspace tests on Linux; it also builds/tests on Windows. Actions use read-only repository permissions and pinned revisions. [Run 39270515978](https://github.com/Voldigoade/equalturn/actions/runs/39270515978) passed both jobs on implementation commit `89a2f1296783128654caab2e9ca1b7059f81efc9`. The workflow reruns on every push and pull request.

Branch protection is not configured for this single-maintainer repository. Add review and force-push protections when there are multiple maintainers, while retaining a recovery path for urgent fixes. Before distributing binaries, define artifact signing, key rotation/recovery, update trust, and incident response.
