# Architecture

**Status:** Milestones 1 and 1.5 are implemented on the current branch and verified in [GitHub Actions run 39308964800](https://github.com/Voldigoade/equalturn/actions/runs/39308964800), which passed Linux/PostgreSQL and Windows jobs. The production TUF root/signing ceremony remains uninitialized. Sections labeled *future design* describe later enforcement work, not current behavior. The protocol contract in [protocol-v1.md](protocol-v1.md) is normative for implemented wire and evidence formats.

## Milestone 1 implementation snapshot

**Verified repository fact:** the workspace contains `equalturn-protocol`, `equalturn-client-core`, `equalturn-relay-core`, `equalturn-report`, and `equalturn-registry` libraries plus the `equalturn` CLI and `equalturn-relay` server. The session client uses a per-session SQLite store and a separate session-key file; the relay stores session, participant, event, receipt, notice, and manifest state in PostgreSQL. It serves bounded JSON lifecycle/report endpoints and an authenticated WebSocket stream. A separate explicit registry CLI verifies local TUF directory repositories and displays typed registry entries; session and relay behavior does not consume registry indicators as policy.

The implemented event protocol uses Ed25519 signatures over explicit domain-separated binary encodings, SHA-256 event-chain hashes, per-participant sequence numbers, relay-signed receipts/notices, and a relay-signed final session manifest. The wire representation is JSON, but JSON serialization is not signed. An invitation code can be used by several participants, is stored at the relay only as a verifier, and is limited by time, participant count, and lifecycle. Each participant key is generated for a single session. Reconnect uses a socket-bound, one-use challenge signed by that key.

Liveness notices distinguish online, stale, disconnected, and ended. The relay uses a monotonic timer for liveness; heartbeat age is not a misconduct signal. Events created while the relay is unavailable remain signed in the local outbox and are retried in order. A receipt is persisted locally before the event is marked acknowledged. The report verifier can check included keys, signatures, chains, receipts, notices, and manifest and reports missing evidence as a gap. Cryptographic validity proves the integrity of included claims, not completeness or workstation truth.

The relay signing seed is stored separately from PostgreSQL. On Unix, the key file lives in an owner-controlled `0700` directory, is `0600`, and is published atomically without replacement. On Windows, the key is published with a no-replace hard link, inherits the parent directory's DACL, and the relay rejects reparse-point key paths; operators must protect the parent ACL. The seed is plaintext at rest and filesystem access does not protect against the service account, administrator/root, memory compromise, or backups. See [relay key storage](relay-key-storage.md).

Participant seed wrappers, DPAPI and Unix storage limits, invitation-code handling, and relay database credential handling are documented in [secret lifecycle](secret-lifecycle.md).

Participant key files use a versioned envelope bound to session, participant, and local-store IDs. Windows protects the seed with current-user DPAPI; Unix uses owner-only permissions and descriptor-relative no-follow file operations. These controls are at-rest protections with documented same-user, administrator/root, backup, and memory-compromise limits, not tamper-proof workstation controls. See [key storage](key-storage.md).

The M1.5 registry boundary validates the v3 source and generated target, then verifies an explicitly selected local TUF repository against caller-supplied root bytes and an independently supplied SHA-256 pin. TUF metadata and targets are bounded and the initial root is never fetched from a repository. No production Equalturn root, signed release, remote transport, or automatic update path exists. See [registry trust](registry-trust.md).

The relay does not expose peer addresses in its participant API or reports, and no P2P transport is used. **Privacy limitation:** the relay transport necessarily receives the connecting source address; relay hosting or proxy infrastructure may retain its own connection logs. The application does not persist source addresses as session evidence.

Milestone 1.5 still does not include an endpoint service/daemon, firewall or application control, process or browser observation, a tray UI, registry-driven policy, production registry publication, automatic updates, or production hosting. The only AI-related session event is explicitly synthetic and user-triggered. Experimental future enforcement must follow [the isolated test policy](enforcement-test-safety.md).

## Future endpoint architecture

The following diagram is the *proposed later enforcement boundary*, not the current executable architecture.

### Proposed components and trust boundaries

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

### Proposed endpoint processes

- Keep the UI unprivileged. It displays participants, protection state, prompts, and reports; it cannot apply arbitrary firewall rules.
- Use one small OS service/daemon for privileged changes. Its IPC accepts typed operations such as enable/disable a named competition policy and return status. It must not expose shell execution, arbitrary rule input, or filesystem browsing.
- On Windows, prefer documented WFP user-mode management APIs for scoped network filters; avoid a kernel driver unless a demonstrated requirement survives a security review.
- On Linux, manage only an application-owned nftables table through a narrow systemd service. Prefer an explicit capability boundary to an always-root general-purpose process where practical. Test cleanup and recovery after crash/reboot.
- Keep policy evaluation, registry parsing, event encoding, signatures, and report generation in a shared Rust core. OS adapters define what evidence and controls are actually available on that platform.
- Defer the tray UI until the local service/protocol boundaries are proven. If used, Tauri v2 is a UI shell, never the enforcement boundary.

## Relay and session model

Use one modest HTTPS/WebSocket relay service rather than direct participant connections or speculative microservices. The implemented relay uses PostgreSQL for session lifecycle, participant heads, event envelopes, receipts, signed liveness notices, and final manifests; the relay is not the authoritative source of local truth. Discord and Telegram may later receive optional notifications, but cannot be the integrity backend.

Milestone 1 implements an invitation code with 26 uniformly selected symbols, a ten-minute expiry, a maximum of seven successful joins/eight participants, and invalidation when the session starts. The relay stores a verifier rather than the raw code. Joining binds a fresh session-scoped Ed25519 key to the participant ID and ASCII nickname. Nicknames are case-insensitively unique within a session. There is no global account, verified real-world identity, or peer address in the session object. See [protocol-v1.md](protocol-v1.md) for precise limits and compatibility behavior.

The session identifier should be random and non-guessable, but it is not a credential. The invitation secret authenticates joining; the session key authenticates subsequent events. Membership changes, start/stop, and control-plane actions are signed by the actor and acknowledged by the relay. Prevent one participant from editing another participant’s event stream by requiring each event signature to verify against the sender’s session public key.

## Event envelope and ordering

Protocol v1 defines the following signed event envelope:

- protocol version;
- UUIDv4 session, participant, and event IDs;
- per-participant monotonic sequence number and previous-event hash;
- allowlisted, data-minimized payload and untrusted client timestamp;
- Ed25519 signature over a fixed, domain-separated binary encoding.

The signed `ParticipantJoined` payload binds the nickname, public key, client version, and coarse self-reported OS data to sequence 1. JSON is used for transport only. The exact encoding and payload variants are defined in [protocol-v1.md](protocol-v1.md); the standard Ed25519 scheme is specified by [RFC 8032](research/sources.md#stack-and-protocol).

The relay verifies membership and signatures, rejects duplicate sequence numbers, stores the event, and adds its own receipt order and Unix-millisecond receive time. The local outbox resends unacknowledged events idempotently in participant sequence order; the authenticated stream can replay retained relay evidence after a cursor. Gaps remain explicit. Local sequence and hash chains detect deletion/reordering inside a presented participant stream, but cannot prove a missing event was ever created. Relay receipt timestamps order server observations, while they do not prove the event occurred at that instant. Client clocks are untrusted and may be skewed or changed.

Milestone 1 implements a signed receipt for every accepted participant event, signed liveness notices, and a signed final manifest containing participant chain heads and the highest evidence order. Those signatures make alteration of included evidence detectable and let a verifier report gaps. Participants can still receive different views, and a relay can omit evidence before issuing a receipt or manifest. Out-of-band comparison of reports can reveal some inconsistencies; the protocol does not provide global transparency or non-equivocation. Do not add a blockchain.

## Heartbeats and reconnects

The current client sends a signed heartbeat about every 30 seconds containing its session/participant identity, connection generation, counter, client timestamp, and last event-chain head. The relay uses monotonic time: `STALE` after 90 seconds without a heartbeat and `DISCONNECTED` after five minutes or an observed close; recovery produces `ONLINE`. These states mean liveness is unknown, not that a participant cheated. The client retries authenticated connections with backoff and replays relay evidence from its durable cursor.

During a network outage, Milestone 1 can queue locally generated signed session events; there is no local enforcement policy to continue, and no process/network observation is made. The report preserves unreceipted local events and liveness gaps. A future protection service will need explicit protection-loss states and a separate session policy for outages; neither exists in this milestone.

## Retention and session expiration

The relay currently deletes unstarted sessions 24 hours after creation, ended sessions seven days after the end time, and inactive active sessions after seven days without accepted activity/heartbeat. The relay runs this cleanup hourly. This is primary-database cleanup; the application cannot specify deletion from operator backups or infrastructure logs. Local key files, SQLite data, and participant exports remain until removed on the participant device. A published operator backup-expiry and incident policy remains unresolved before public hosting.

## Implementation boundary and remaining design work

Milestone 1 provides the non-enforcing session protocol, CLI, relay, outbox, liveness stream, and verifiable report. Milestone 1.5 adds participant/relay key-storage hardening, secret-lifecycle fixes, a typed registry v3 client boundary, local TUF verification, an offline registry CLI, and adversarial coverage. The latest branch CI run [39308964800](https://github.com/Voldigoade/equalturn/actions/runs/39308964800) passed Linux/PostgreSQL and Windows jobs, including the DPAPI tests and deterministic target check. The production registry root/release ceremony remains uninitialized. Future endpoint work should proceed through the [roadmap](implementation-roadmap.md) and [test safety policy](enforcement-test-safety.md), with isolated OS-specific testing, consent/policy decisions, and review of the limitations in [threat-model.md](threat-model.md) and [enforcement-matrix.md](enforcement-matrix.md).
