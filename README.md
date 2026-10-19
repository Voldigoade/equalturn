# Equalturn

Equalturn is a research-stage open-source project for a privacy-conscious, best-effort integrity environment for mutually agreed no-AI technical competitions.

**Status: research / architecture phase.** This repository contains Phase 0 research and a versioned sample detection registry. It does not contain a client, relay service, blocker, or enforcement code.

The security objective is to make prohibited AI use difficult on a monitored workstation and make protection loss, tampering, and suspicious attempts difficult to hide from other session participants. Software running on a workstation cannot guarantee this against its administrator, root user, a second device, or a remote machine.

## Start here

- [Product brief](docs/product-brief.md)
- [Architecture and event protocol](docs/architecture.md)
- [Threat model](docs/threat-model.md)
- [Windows and Linux enforcement matrix](docs/enforcement-matrix.md)
- [AI ecosystem and registry design](docs/ai-registry.md)
- [Initial registry data](registry/registry-v1.json)
- [Privacy and reporting model](docs/privacy.md)
- [Prior art](docs/prior-art.md)
- [Stack decision](docs/stack-decision.md)
- [Name research](docs/naming.md)
- [Implementation roadmap](docs/implementation-roadmap.md)
- [Research sources](docs/research/sources.md)

## Intended users

People running or joining CTFs, security challenges, programming competitions, study groups, and other technical exercises where participants choose to compete without AI assistance. The first target is a small session of peers using Windows or Linux workstations they control.

## Principles

- State evidence and coverage limits; never claim that bypass is impossible.
- Keep participant identity pseudonymous and avoid exposing peer IP addresses.
- Use a neutral relay for canonical shared events; do not connect participants directly.
- Keep privileged code small and separate from the user interface.
- Record integrity signals, not private work: no keystrokes, prompts, passwords, complete browsing history, or filesystem inventory.
- Treat network and process matches as evidence with confidence and scope, not proof that a model was used.

## License

Apache-2.0. Its explicit contributor patent license is useful for a systems project that may integrate OS-specific interfaces, while remaining permissive for use in tools and event environments. See [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Research claims should cite primary sources where available and be labeled as verified facts, inference, proposal, or unresolved risk.
