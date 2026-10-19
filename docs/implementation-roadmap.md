# Implementation roadmap

Each milestone has a written scope, acceptance criteria, and platform-specific verification. **Milestone 1 is implemented and locally verified.** Later OS enforcement milestones remain future work. OS enforcement milestones below remain future work.

## 0. Research and architecture — completed

Document trust boundaries, data flows, OS primitives, registry format, privacy model, prior art, and implementation risks. Phase 0 research is retained in `docs/` and the versioned research registry.

## 1. Non-enforcing session vertical slice — complete

The implementation now contains a Rust CLI client, Rust relay, versioned signed event protocol, session-scoped Ed25519 identities, high-entropy invitation flow, PostgreSQL relay state, SQLite outbox, authenticated WebSocket reconnect/heartbeat, signed relay evidence, and JSON/Markdown report export plus offline verification. The only AI-related event is synthetic. There is no firewall change, process blocking, tray UI, workstation telemetry, or AI classification logic. See the [current user flow](../README.md) and [protocol contract](protocol-v1.md).

**Verified:** A and B completed the relay flow; 49 tests pass, including two PostgreSQL-backed relay integration scenarios and adversarial report/protocol cases. Formatting, registry validation, Clippy with warnings denied, workspace build, and dependency audit pass. The synthetic event reached the peer; offline queue/reconnect, end-of-session export, and independent verification worked. A liveness gap remains unknown rather than a clean result; peer IPs are absent from participant-visible/report data; the client does not enforce controls. Linux/PostgreSQL and Windows CI passed in [run 39270515978](https://github.com/Voldigoade/equalturn/actions/runs/39270515978). That run verified the SQLite shutdown fix in commit `89a2f1296783128654caab2e9ca1b7059f81efc9`.

## 2. Registry validation and advisory local signals

Add a schema validator, signed/versioned registry packaging, local process-signature observations, and destination-only DNS/network observations behind a diagnostic profile. Measure false-positive and source freshness behavior. Keep advisory and blocked states distinct.

**Exit:** registry entries include source, freshness, confidence, scope, and false-positive notes; unknown destinations are not silently classified as non-AI.

## 3. Linux best-effort enforcement

Introduce a systemd service and narrowly owned nftables rules. Validate permissions, crash/restart cleanup, IPv4/IPv6, resolver paths, containers/namespaces, and protection-state reporting. Defer eBPF unless a measured requirement demonstrates value.

**Exit:** an ordinary user cannot change the service-owned rules; root bypass is detected only as state loss/heartbeat loss and is documented as possible.

## 4. Windows best-effort enforcement

Introduce a small Windows service and WFP user-mode filters for evidence-backed destinations and application identities. Validate service ACLs, WFP dynamic sessions, Windows Firewall coexistence, WSL networking, packaged apps, DNS/DoH, upgrades, uninstall, and administrative bypass.

**Exit:** installer/uninstaller are reversible; policy failures are reported; no custom kernel driver is needed for the supported baseline. Managed WDAC policy remains a separate organizer-controlled profile.

## 5. Minimal tray UX and privacy modes

Add the non-elevated Tauri tray UI, local IPC, session/consent screens, visible protection state, telemetry profile summary, and report export. Do not let the UI choose hidden privileged actions.

**Exit:** normal CTF workflow is not blocked except for selected policy matches; state and privacy choices are visible and can be stopped.

## 6. Beta hardening and distribution

Threat review, reproducible release builds, binary signing, staged update and rollback protections, registry release signing/TUF, uninstall recovery, packaging for Windows/Linux, localization/accessibility, and volunteer pilot with explicit known limitations.

**Exit:** every release states supported OS versions, tested network paths, protection capabilities, retention policy, known bypasses, and incident response path.

## 7. v1 and operations

Set support matrix, session server operating model, rate limits, retention enforcement, registry contributor review, release key rotation/recovery, security response ownership, and external review expectations. Optional Discord/Telegram notifications can be adapters only; the relay remains the canonical event log.

## Current implementation boundary

Milestone 1 is complete: a non-enforcing, two-client session event path and useful export are verified. This tests protocol, reconnects, pseudonymous identity, relay privacy, authenticated ordering, and report usefulness before privileged firewall code or UI obscures those questions. Keep OS enforcement, process blocking, auto-updates, registry automation, and browser-history telemetry out of that first slice.

## Ranked unresolved risks

1. **Endpoint trust ceiling:** a user with administrator/root access can stop, patch, or replace local enforcement and falsify local observations. The relay can expose loss of contact but cannot establish cause or truth.
2. **Coverage and false positives:** provider endpoints, BYOK routing, cloud marketplaces, local runtimes, custom models, tunnels, and shared hosting change continually; aggressive rules can break legitimate CTF tooling.
3. **Relay privacy and equivocation:** the relay sees source IPs and connection timing, and can omit/fork events or deny service. Signed events and checkpoint comparison reduce silent editing but do not remove operator trust.
4. **OS behavior and recovery:** WFP/firewall coexistence, DNS/DoH, WSL, nftables, systemd, containers, sleep, upgrades, and uninstall recovery need real Windows/Linux VM probes before selecting a v1 support matrix.
5. **Evidence semantics:** signed reports authenticate a key’s claim, not the completeness of a compromised endpoint sensor. Competition rules need a fair way to interpret unknown states and outages without automatic cheating verdicts.
6. **Registry and update trust:** a signed but stale or compromised registry can block legitimate work or miss new surfaces; key rotation, expiry policy, review ownership, and offline recovery are not yet defined.
7. **Retention and operator obligations:** relay cleanup now removes abandoned/ended session rows on a defined schedule, but backup expiry, hosting logs, abuse response, and participant deletion rights need an operating model before public hosting.
