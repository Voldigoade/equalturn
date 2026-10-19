# Repository metadata and bootstrap

**Proposed repository:** `Voldigoade/equalturn`

**Default branch:** `main`

**Description:** A privacy-first, best-effort integrity environment for mutually agreed no-AI technical competitions.
**Topics:** `competition-integrity`, `ctf`, `programming-contest`, `endpoint-security`, `privacy`, `windows`, `linux`, `rust`, `open-source`

The local repository is prepared on `main`. GitHub creation, metadata, security settings, and remote publication could not be applied in Phase 0 because the local GitHub CLI credential is invalid, the browser session is signed out, and the connected GitHub integration does not expose repository-creation or repository-settings operations. No repository URL is claimed until it exists.

When write access is restored, create a **public** repository with the metadata above and this initial state. Keep Issues enabled; keep Discussions and Wiki off until needed because the handbook is in `docs/`.

## Security settings to apply when accessible

- Enable private vulnerability reporting and Dependabot alerts.
- Enable secret scanning and push protection if available for the account/repository.
- Keep `main` as the default branch. Require pull requests and block force pushes after more than one maintainer is available; avoid a review rule that makes a single-maintainer repository unable to publish urgent fixes.
- Use signed release artifacts and a documented key-rotation plan before binary distribution.
- Do not enable Actions workflows until there is a specific build or documentation check worth running.

## Files in this phase

The repository is documentation and data only: project brief, architecture, threat model, enforcement matrix, registry design/schema/sample data, privacy/reporting model, prior-art and naming research, stack recommendation, roadmap, source index, concise contribution/security guidance, license, and editor defaults. It contains no executable product or generated binaries.
