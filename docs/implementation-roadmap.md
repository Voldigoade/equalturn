# Implementation roadmap

Each milestone needs a written scope, acceptance criteria, and OS-specific verification. The controls below are proposals; no milestone is implemented in this repository.

## 0. Research and architecture — current phase

Document trust boundaries, data flows, OS primitives, registry format, privacy modes, prior art, and implementation risks. Publish the architecture handoff before implementation starts.

## 1. Non-enforcing session vertical slice

Build a small Rust agent CLI and one relay service. Support nickname, create/join, invitation code, per-session signing key, heartbeat, signed protection-state events (initially `advisory/no enforcement`), reconnect queue, server receipts, and JSON/Markdown report. Run two clients through the relay. Do not add firewall changes, process blocking, tray UI, telemetry beyond the event model, or AI classification heuristics.

**Exit:** participants can verify event signatures and sequence gaps; dropped network and client termination produce `unknown/unreachable` rather than a clean result; peer IPs are not included in participant-visible payloads; reports clearly state the client is not enforcing controls.

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

## First implementation boundary

Start with Milestone 1 only: a non-enforcing, two-client session event path and useful export. This tests protocol, reconnects, pseudonymous identity, relay privacy, authenticated ordering, and report usefulness before privileged firewall code or UI obscures those questions. Keep OS enforcement, process blocking, auto-updates, registry automation, and browser-history telemetry out of that first slice.

## Ranked unresolved risks

1. **Endpoint trust ceiling:** a user with administrator/root access can stop, patch, or replace local enforcement and falsify local observations. The relay can expose loss of contact but cannot establish cause or truth.
2. **Coverage and false positives:** provider endpoints, BYOK routing, cloud marketplaces, local runtimes, custom models, tunnels, and shared hosting change continually; aggressive rules can break legitimate CTF tooling.
3. **Relay privacy and equivocation:** the relay sees source IPs and connection timing, and can omit/fork events or deny service. Signed events and checkpoint comparison reduce silent editing but do not remove operator trust.
4. **OS behavior and recovery:** WFP/firewall coexistence, DNS/DoH, WSL, nftables, systemd, containers, sleep, upgrades, and uninstall recovery need real Windows/Linux VM probes before selecting a v1 support matrix.
5. **Evidence semantics:** signed reports authenticate a key’s claim, not the completeness of a compromised endpoint sensor. Competition rules need a fair way to interpret unknown states and outages without automatic cheating verdicts.
6. **Registry and update trust:** a signed but stale or compromised registry can block legitimate work or miss new surfaces; key rotation, expiry policy, review ownership, and offline recovery are not yet defined.
7. **Retention and operator obligations:** seven-day deletion is only a proposal; backup expiration, relay deployment, abuse response, and participant deletion rights need an operating model before hosting sessions.
