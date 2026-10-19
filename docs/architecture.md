# Proposed architecture

**Status:** design proposal for review. Nothing here describes implemented behavior.

## Components and trust boundaries

```text
┌──────────────────────────── participant workstation ─────────────────────────────┐
│  unprivileged tray UI ── narrow local IPC ── endpoint service / daemon            │
│                                            ├─ policy + registry                  │
│                                            ├─ OS enforcement adapter             │
│                                            ├─ process/liveness observations      │
│                                            └─ signed local event queue           │
└──────────────────────────────────────────────┬───────────────────────────────────┘
                                               │ HTTPS setup + WSS event relay
                                  ┌────────────▼────────────┐
                                  │ session relay + database │
                                  └────────────┬────────────┘
                         signed event fan-out │ session state
                          ┌───────────────────┴───────────────────┐
                    Participant A                            Participant B
```

The workstation owner and local administrator/root are outside the client’s trust boundary. The endpoint service is trusted to report what it observes while it is running, but a user who controls the machine can stop or alter it. The relay is trusted for availability, membership, event receipt ordering, and privacy from other participants. Participant signatures make event alteration attributable, but do not make endpoint observations true. The relay can still omit, delay, fork, or selectively reveal events; signed receipts and participant comparison make some of those actions visible, not impossible.

The relay necessarily sees each connected client’s network address and connection timing. It must not disclose those addresses to other participants. A relay operator can still correlate a participant’s network address with session activity. This is a privacy property against peers, not network anonymity from the service operator. Do not use peer-to-peer transports, ICE/STUN, or direct client callbacks.

## Endpoint processes

- Keep the UI unprivileged. It displays participants, protection state, prompts, and reports; it cannot apply arbitrary firewall rules.
- Use one small OS service/daemon for privileged changes. Its IPC accepts typed operations such as enable/disable a named competition policy and return status. It must not expose shell execution, arbitrary rule input, or filesystem browsing.
- On Windows, prefer documented WFP user-mode management APIs for scoped network filters; avoid a kernel driver unless a demonstrated requirement survives a security review.
- On Linux, manage only an application-owned nftables table through a narrow systemd service. Prefer an explicit capability boundary to an always-root general-purpose process where practical. Test cleanup and recovery after crash/reboot.
- Keep policy evaluation, registry parsing, event encoding, signatures, and report generation in a shared Rust core. OS adapters define what evidence and controls are actually available on that platform.
- Defer the tray UI until the local service/protocol boundaries are proven. If used, Tauri v2 is a UI shell, never the enforcement boundary.

## Relay and session model

Use one modest HTTPS/WebSocket relay service rather than direct participant connections or speculative microservices. PostgreSQL may store active session metadata and event envelopes; the relay is not the authoritative source of local truth. Discord and Telegram may later receive optional notifications, but cannot be the integrity backend.

An organizer creates a short-lived session and receives a high-entropy invitation secret. Store only a verifier for the invitation secret, rate-limit attempts, and expire it after join or a short window. Joining creates an ephemeral Ed25519 key pair on the participant device. The public key is the session-scoped participant identity; the display nickname is the only profile field. There is no global user account in the proposed first version. A participant can rotate only by leaving and rejoining, with that change visible in the session log.

The session identifier should be random and non-guessable, but it is not a credential. The invitation secret authenticates joining; the session key authenticates subsequent events. Membership changes, start/stop, and control-plane actions are signed by the actor and acknowledged by the relay. Prevent one participant from editing another participant’s event stream by requiring each event signature to verify against the sender’s session public key.

## Event envelope and ordering

Each client creates a deterministic event envelope with:

- protocol and event-schema versions;
- opaque session and participant IDs, nickname snapshot, and client version;
- per-participant monotonic sequence number and previous-event hash;
- event type and allowlisted, data-minimized payload;
- local wall-clock timestamp plus monotonic elapsed offset when available;
- signature over canonical bytes (Ed25519 is the initial candidate).

The relay verifies membership and signatures, rejects duplicate sequence numbers, stores the event, and adds its own receipt sequence and UTC receive time. On reconnect the client resends idempotently from its last acknowledged sequence; gaps remain explicit. Local sequence and hash chains detect deletion/reordering inside a presented participant stream, but cannot prove a missing event was ever created. Relay receipt timestamps order server observations, while they do not prove the event occurred at that instant. Client clocks are untrusted and may be skewed or changed.

For a stronger later design, the relay can issue signed per-session checkpoints over a hash of the ordered event set. Each client stores checkpoints and includes their latest checkpoint in subsequent events. Participants can compare checkpoints and signed receipts in the final report. This makes inconsistent relay views easier to detect, but only if participants retain and later compare evidence; it does not stop a malicious relay from withholding an event from every participant. This needs protocol review before implementation. Do not add a blockchain.

## Heartbeats and reconnects

While Competition mode is active and online, send a signed heartbeat every 30 seconds containing only protection state, client/service version, local monotonic counter, and current sequence number. Three missed intervals can mark a participant **stale**; they do not prove whether the client was killed, the machine slept, or connectivity failed. Distinguish `connected`, `stale`, `disconnected`, and `protection-unknown` in the UI/report. Never translate network absence into a cheating verdict.

During a network outage, the client continues local policy where the platform allows and queues signed events. The report marks the unobserved relay interval and later receipt times. If protection itself cannot continue safely (for example, the service is gone or policy installation failed), fail to an explicit protection-loss state; do not silently claim protection. An organizer may set a session policy that pauses the competition clock on service loss, but that is a competition rule, not a technical fact.

## Retention and session expiration

Sessions expire after an explicit organizer-selected end time plus a short reconnect grace period. The initial proposal is to retain event data only while active and for seven days after normal end (or seven days after the last heartbeat for an abandoned session), then delete it from primary storage and backups according to a published backup window. This is a policy proposal requiring operator and participant review before launch. A participant should be able to export the signed report locally before deletion. No public directory of sessions or long-lived identity graph is needed.

## First protocol spike

Before enforcement implementation, build a local protocol spike only: create a session, join with two ephemeral keys, sign and verify canonical events, persist per-participant sequence/hash chains, deliver events through a relay, simulate reconnect/replay, and produce a report showing gaps and server receipts. The spike can use synthetic events and must not inspect workstation activity.
