# Threat model

**Status:** this document combines the product threat model with the implemented Milestone 1 evidence boundary. It is not a certification or a claim that cheating can be proven. Network and OS capability research is indexed in [research/sources.md](research/sources.md).

## Security objective

The eventual product objective is to make prohibited AI use difficult on a monitored workstation and make protection loss, integrity failures, tampering, and suspicious attempts difficult to hide from other participants. Milestone 1 implements the session/evidence substrate only. It does not monitor or protect a workstation, so it cannot currently make AI use difficult.

## Assets and properties

- **Participant privacy:** pseudonymous nickname and session-scoped identity; no peer-visible network address, machine username, filesystem inventory, browsing history, prompts, keystrokes, credentials, or message contents.
- **Evidence integrity:** Ed25519-signed participant claims, per-participant sequence/hash chains, signed relay receipts/notices, and a signed final manifest make edits to included evidence detectable.
- **Availability:** participants can create/join sessions, reconnect, replay retained evidence, and export a report while the relay and network are available.
- **Endpoint integrity:** no endpoint-control asset exists in Milestone 1. Future OS enforcement is a separate trust boundary and will require a narrow privileged component.

An event signature establishes which key signed a claim and detects modification to that event. It does not show that the local client observed reality honestly, that a relevant event was created, or that the relay supplied every event. A relay signature attests to what the relay key says it received/ordered; the self-contained report does not prove who operated that key.

## Actors and trust boundaries

| Actor | Capability | Treatment |
| --- | --- | --- |
| Honest participant | Uses agreed rules; may make mistakes | Normal user. Their key-authenticated claims become shared evidence. |
| Curious participant | Inspects controls, files, and their own traffic | Assume access to all local client files and own traffic; do not rely on secrecy of local implementation. |
| Participant attempting protocol abuse | Sends malformed, false, replayed, delayed, oversized, or conflicting claims | Relay applies bounded parsing, membership/signature/chain validation, idempotency checks, rate/resource limits, and stable errors. A participant can still lie about their own behavior. |
| Standard-privilege workstation user | Can use alternate browsers, portable applications, user-configured proxies/VPNs, local models, and remote services | Milestone 1 makes no workstation-level claim or detection. Future layered controls will have incomplete coverage. |
| Administrator/root or compromised endpoint | Can stop/patch the client, edit local files, change boot/network setup, use VMs/containers, or replace the OS | Outside the endpoint trust boundary. Local signing keys and sensors cannot prove an administrator did not alter the source of a claim. |
| Relay operator or compromised relay | Sees connection IP/timing; can deny service, withhold, reorder, fork, or expose stored data | Participants verify signatures and compare reports. Signed receipts/manifest detect edits to included material but do not prevent withholding, equivocation, or denial of service. |
| Network attacker | Can observe, disrupt, or replay transport traffic | TLS is required for non-loopback production transport; signed protocol and sequence checks protect evidence semantics. Local plaintext is permitted only with explicit loopback development mode. Availability remains vulnerable. |
| Second device or remote host | Phone/tablet/other computer, SSH/RDP system, cloud browser, or remote model endpoint | Not controlled or reliably detected by workstation software; outside scope. |

Participants connect only to the relay. The relay receives source addresses at the transport layer, but participant API objects and report fields exclude peer addresses. Hosting proxies or infrastructure may keep separate transport logs.

## Threat handling

| Threat | Milestone 1 behavior | Residual limit / future work |
| --- | --- | --- |
| Invalid signature, payload mutation, unsupported version, malformed IDs/payloads, oversized request | Protocol validation and strict signature checks reject invalid evidence; report verification returns `INVALID` for malformed/contradictory included evidence. | Does not prove an endpoint signed a truthful observation. |
| Duplicate/replayed event, sequence rollback/gap, wrong prior hash, wrong session/member key | Relay validates membership and chain head transactionally; exact retry is idempotent; conflicting duplicate or invalid order is rejected. Reports expose missing sequence/evidence-order gaps. | Missing-before-signing events are unknowable; a relay may omit evidence from the report. |
| Replayed/stale reconnect authentication | One-use challenge is bound to socket/session/participant and consumed; completed reconnect replaces the prior active socket. | Key theft permits impersonation until the identity is visibly replaced/revoked; local user/admin can access local key storage. |
| Participant disconnect, killed client, sleep, or network interruption | Signed heartbeats and relay liveness transitions produce online/stale/disconnected states; reconnect replays persisted relay evidence; signed local outbox is retried. | Cause of missed liveness is unknown; if client process and local evidence store are destroyed, unsubmitted events cannot be recovered. `STALE` is not cheating. |
| Malicious client claims false observations | Signatures attribute claims to a session key; report labels synthetic and participant-reported data. | No sensor truth or attestation in M1; client owner can lie. |
| Clock manipulation | Client timestamp is treated as untrusted; relay ordering and its receive times are separately recorded. | Relay wall clock may be wrong; monotonic liveness time is process-local and resets on relay restart. |
| Relay restart | Persisted event/order state and relay signing identity survive; relay recovers online/stale participant liveness as disconnected. | Disconnect notices may be absent if the process crashes before committing them; availability is lost during restart. |
| Relay compromise/equivocation | Participant event keys, signed receipts, liveness notices, and final manifest let verifiers check included evidence; report may be checked against a pinned relay key. | No transparency log, quorum, regular checkpoints, or guarantee against withholding/forks. Operators remain trusted for confidentiality/availability. |
| Old client or registry rollback | The protocol version and report schema are explicit; unsupported protocol/report versions fail closed. | No secure auto-update, minimum-client enforcement, or registry use in M1; a participant can run a modified client. |
| DNS changes, raw IP, DoH, VPN/proxy/Tor, portable/recompiled clients, API routing, local GGUF/Ollama/vLLM, containers/VMs/WSL | No detection or blocking is attempted in M1. The registry is research data only. | These are open coverage questions for future OS-specific work; no universal model-use signature is known. |
| Remote SSH/RDP/browser or second physical device | No detection or control. | Not reliably detectable or controllable from the monitored workstation. |

## Realistic guarantees

The current implementation can authenticate included event claims to session keys; detect mutation, invalid signatures, chain conflicts, duplicate identifiers, and gaps in presented evidence; record relay acceptance/order receipts; expose liveness loss; and produce an offline-verifiable report. It prevents participant-facing APIs and reports from exposing peer IP fields.

It cannot prove that a participant did not use AI, prove intent from a synthetic or reported event, make bypass impossible, ensure an administrator cannot defeat local controls, attest that all relevant surfaces were observed, prevent a compromised relay from denying service or withholding/forking evidence, or infer cheating from disconnection. Milestone 1 does not block or detect AI access. Reports distinguish protocol verification from endpoint truth and never assign a cheating verdict.
