# Security policy

Equalturn has an implemented, non-enforcing Milestone 1 CLI and relay, but no released binary or endpoint enforcement service. The session protocol authenticates claims; it does not prove workstation truth or rule out administrator tampering.

Use GitHub's private vulnerability reporting rather than opening a public issue with exploitable details. Include the affected revision, impact, reproduction conditions, and any mitigation. Do not include real participant data, credentials, or session invitation codes.

Security reports will be acknowledged and triaged by maintainers. If the private reporting channel is unavailable, do not publish sensitive exploit details in an issue; contact a maintainer through a private channel linked from the repository profile.

Reports, session logs, and signed events are not proof that AI use did or did not occur. A participant with administrator/root control can tamper with local software, and remote or second-device use is outside the endpoint's trust boundary. See the [threat model](docs/threat-model.md).
