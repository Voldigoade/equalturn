# Equalturn protocol v1

**Status:** implementation contract for Milestone 1. **Checked:** 19 October 2026.

This protocol authenticates claims made by a session key and records what a relay accepted. It does not prove that a participant's workstation observed reality honestly, that no event was omitted before signing, or that a relay did not withhold or equivocate. A self-contained report verifies the included relay key cryptographically; it does not identify the key operator. Use an out-of-band pinned fingerprint when relay identity matters.

## Common representation and limits

- JSON is the transport and report representation only. Signatures cover the fixed binary encodings below, never JSON bytes.
- JSON UUIDs are lowercase canonical UUID strings. IDs are UUIDv4 and the all-zero UUID is invalid. Public keys and hashes are lowercase hexadecimal strings of exactly 64 characters; signatures are lowercase hexadecimal strings of exactly 128 characters. Hex decoders reject uppercase, wrong lengths, and non-hex characters.
- Protocol version is unsigned 16-bit big-endian and v1 is `1`. Unsupported versions are rejected with `unsupported_protocol_version` (HTTP 426 for HTTP requests). A future version uses a new domain separator and explicitly defined compatibility policy; v1 implementations never guess.
- All integers in signed encodings are unsigned big-endian unless a field is explicitly a signed timestamp. Client timestamps are Unix milliseconds in `u64`, nonzero and untrusted. Relay timestamps are Unix milliseconds in `u64`; relay monotonic time, not wall time, drives liveness.
- Text in signed v1 fields is ASCII with no normalization step. Nickname is 1–32 bytes matching `[A-Za-z0-9][A-Za-z0-9_-]{0,31}`. Nickname uniqueness is ASCII case-insensitive within a session. Event targets are empty or a lowercase DNS hostname of at most 253 bytes, with DNS-label lengths 1–63, no trailing dot, no URL/path/port/IP literal, and no Unicode.
- A signed event's encoded payload is at most 2,048 bytes; event envelope and signature are at most 16 KiB in JSON. HTTP JSON bodies are capped at 32 KiB. WebSocket frames/messages are capped at 32 KiB. Sessions have at most eight participants and at most 10,000 accepted participant events. Per-connection fan-out queues are bounded to 128 messages; a slow peer is disconnected and must resume from the durable relay cursor.

The relay reserves the final participant-event slot and two evidence-order positions for the creator's terminal event and its `session_ended` notice. Ordinary event and liveness admission also leaves eight evidence positions available for restart reconciliation of the maximum participant count. A restart can therefore record each currently online/stale participant as disconnected without consuming the terminal reserve. At exhaustion, regular event submission and new authenticated connections return `resource_limit`; a creator can still submit the signed end event over the HTTP event API without first reconnecting. Reports retain the high-water mark and verification exposes any missing evidence orders as gaps. These fixed v1 bounds limit resource use; they do not guarantee availability if evidence is already inconsistent or the relay is unavailable.

## Participant event envelope and signing

`ParticipantEvent` contains protocol version, session ID, participant ID, event ID, participant sequence, previous-event hash, client timestamp, payload, and 64-byte Ed25519 signature. Event IDs are unique within the session. The payload variant is the event type; there is no separately mutable event-type field.

The event's signable bytes are exactly:

```text
ASCII("Equalturn\0event\0v1\0")
u16 protocol_version
16-byte session UUID
16-byte participant UUID
16-byte event UUID
u64 participant_sequence
32-byte previous_event_hash
u64 client_timestamp_ms
u32 payload_byte_length
payload bytes
```

The domain string includes each shown NUL byte. The first event has sequence `1` and a zero previous hash. Later events increment by exactly one and use the exact prior event hash. Sequence increment uses checked arithmetic; wraparound is invalid.

Payload bytes have these v1 tags and layouts. Lengths are byte lengths; no trailing bytes or unknown tag is accepted.

| Tag | Variant | Payload bytes after tag |
| ---: | --- | --- |
| `1` | `ParticipantJoined` | `u8 nickname_len`, nickname ASCII; 32-byte Ed25519 public key; `u8 client_version_len`, 0–64 printable ASCII bytes; `u8 os_family` (`0` unknown, `1` Windows, `2` Linux, `3` other); `u8 os_bucket_len`, 0–32 ASCII bytes. OS and version are participant-reported. |
| `2` | `SessionStarted` | No fields. Only the session creator may sign it. |
| `3` | `SyntheticAiAccessAttempt` | `u8 target_len`, 0–253 lowercase hostname bytes. This event is explicitly synthetic and is not a workstation observation. |
| `4` | `SessionEnded` | `u8 reason` (`1` participant ended). Only the creator may sign it. |

Signing uses Ed25519 over the complete domain-separated bytes above. Verification uses strict Ed25519 verification. The signature is 64 raw bytes in the cryptographic operation. Compute the event hash as `SHA-256(ASCII("Equalturn\0event-hash\0v1\0") || signable_bytes || signature_bytes)`. Fingerprints are lowercase hex of `SHA-256(ASCII("Equalturn\0fingerprint\0v1\0") || public_key_bytes)`.

The participant key is fresh per participant/session. The signed sequence-1 join event binds that key to the participant ID, nickname, reported client version, and reported coarse OS. The relay compares the signed public key to the join request before registering membership.

## Invitation and lifecycle

An invitation is 26 uniformly random symbols from `23456789ABCDEFGHJKMNPQRSTUVWXYZ` (about 128.6 bits), displayed in groups of `5-5-5-5-6` characters. Each symbol is selected uniformly with rejection sampling from OS randomness; the alphabet has 31 symbols and avoids common ambiguous characters. Input is uppercased and hyphens are removed before strict alphabet/length validation. The relay stores only SHA-256 of the normalized uppercase ASCII code. It is multi-use until the earliest of 10-minute expiry, seven successful joins, eight total participants, or session start. Join request ID is the signed sequence-1 event ID; retrying the same request with the same key and exact event returns the original membership/receipt. Reuse with conflicting bytes fails. Check verifier, expiry, use limit, participant cap, nickname uniqueness, and insert membership atomically. Rate-limit failed invite lookups in memory; never persist their source addresses.

The creator's signed `SessionStarted` event transitions `CREATED` to `ACTIVE`; the same creator's signed `SessionEnded` transitions `ACTIVE` to `ENDED`. The relay accepts start/end and chain-head changes in one transaction. Membership freezes at start. Events submitted after end are rejected. Reconnect after end is read-only for report retrieval. Pending client events not committed before end remain local, are retained in the local outbox, and appear as unreceipted local evidence if exported locally.

## Event acceptance and receipts

For a new event, the relay requires: supported protocol; active membership; valid non-nil IDs; bounded/valid payload; strict signature under the member key; exact next sequence and previous hash; an unended session; and a fresh event ID. In one PostgreSQL transaction, lock the participant head, check idempotency, insert the event, advance the participant head, allocate the next per-session evidence order, create/sign/store the receipt, update lifecycle if applicable, and commit. Publish only after commit.

An exact retry (same event ID and same signed bytes/hash) returns the originally stored receipt. Same ID with different bytes, occupied sequence with different data, rollback, gap, wrong previous hash, non-member key, wrong session, or late event fails with a stable protocol error. A gap is not buffered by the relay; the response includes the expected sequence and head hash. Concurrent submissions serialize on the participant head. Reconnecting peers resume from a relay cursor and receive persisted accepted evidence, not just future in-memory fan-out.

Receipt bytes are exactly `ASCII("Equalturn\0receipt\0v1\0") || u16 version || 16-byte session ID || 16-byte participant ID || 16-byte event ID || 32-byte event hash || u64 per-session evidence order || u64 relay_receive_time_ms`. The relay signs these bytes with a persistent Ed25519 key and includes the relay key in exported report material. A receipt proves that this key attested to acceptance/order/time; absent an out-of-band key fingerprint, it does not prove who controlled the relay or prevent equivocation.

## Reconnect and live delivery

HTTP lifecycle/event writes are authenticated by the signed request/event. A WebSocket connection is authenticated with a one-use challenge:

1. The server creates a random 16-byte challenge ID and 32-byte nonce, records a 15-second monotonic expiry, and binds the challenge to that exact socket instance, session, and participant.
2. The client signs `ASCII("Equalturn\0reconnect\0v1\0") || u16 version || challenge_id[16] || nonce[32] || session_id[16] || participant_id[16]` using the session key.
3. The server atomically consumes the socket-bound challenge before taking the participant's active-connection slot. A stale, cross-socket, wrong-key, or replayed response fails. The latest successfully completed handshake replaces and closes the prior socket; every later frame is fenced by its connection generation.
4. On success the relay emits a signed `ONLINE` liveness notice, then replays persisted events/receipts after the supplied cursor before sending live fan-out.

Duplicate active connection behavior is therefore deterministic by completed handshake order. The old connection cannot send events or revive liveness after replacement. TLS is mandatory off loopback; the relay refuses non-loopback plaintext. Local insecure HTTP requires an explicit development-only flag and a loopback bind.

## Heartbeats and liveness

Heartbeat is a signed WebSocket control message, not a participant event and not an offline-queued item. Its signature bytes are `ASCII("Equalturn\0heartbeat\0v1\0") || u16 version || session_id[16] || participant_id[16] || u64 connection_generation || u64 heartbeat_counter || u64 client_timestamp_ms || u64 last_event_sequence || last_event_hash[32]`. The relay verifies identity/generation, strictly increasing counter, and active session. The client-provided clock/head are advisory and cannot change server ordering.

The relay uses monotonic elapsed time: `ONLINE` while authenticated heartbeats are less than 90 seconds old; `STALE` at 90 seconds without one; `DISCONNECTED` on an observed close or five minutes without a heartbeat; `ENDED` when the session ends. Recovery from stale/disconnected emits `ONLINE`. `STALE`/`DISCONNECTED` mean liveness is unknown; neither is a misconduct finding. Transitions are stored and relay-signed in the same per-session evidence ordering as event receipts. Wall-clock timestamps are included for human reports, with clock uncertainty stated.

## Relay notices and final manifest

Each relay notice contains protocol version, session ID, notice ID, evidence order, relay timestamp, notice kind, optional participant ID, and transition data. Kinds are `participant_online`, `participant_stale`, `participant_disconnected`, and `session_ended`. Sign exactly `ASCII("Equalturn\0relay-notice\0v1\0") || u16 version || session_id[16] || notice_id[16] || u64 evidence_order || u64 relay_timestamp_ms || u8 kind || participant_present[u8] || participant_id[16 if present] || u64 connection_generation || u64 heartbeat_counter`. For session end, participant-present is zero and the final two integers are zero. For participant liveness, participant-present is one and the transition's current generation/counter are included. All IDs are non-nil. The relay signs with its relay key; no transport address is included.

After committing the creator's end event, the relay creates and signs a final manifest. Bytes are `ASCII("Equalturn\0session-manifest\0v1\0") || u16 version || session_id[16] || end_event_id[16] || end_event_hash[32] || u64 highest_evidence_order || u64 start_relay_timestamp_ms || u64 end_relay_timestamp_ms || u8 participant_count || participant_heads[]`. Heads are sorted by participant UUID bytes and each is `participant_id[16] || u64 final_sequence || final_hash[32]`; a participant with no participant events has sequence zero and zero hash. Manifest JSON also includes the relay public key/fingerprint. It is stored atomically with the end transition. A verifier compares included events/notices/receipts to the manifest and reports missing sequence/order suffixes as gaps. The manifest is the relay's signed completeness assertion only; a malicious relay can omit an event before signing, sign conflicting manifests, or withhold the whole report. Comparing participant reports/checkpoints may reveal some forks; this protocol does not provide global transparency or non-equivocation.

## Offline queue and local key state

SQLite stores signed event envelopes, received relay evidence, and outbox/ack state. Event sequence allocation, previous hash, event signing input, and outbox insertion occur in one SQLite transaction; use WAL and full synchronous durability. A relay acknowledgement is removed only after the exact signed receipt is validated and durably stored. Resend in participant sequence order. Duplicate delivery is idempotent.

The session signing key is stored outside the event database in a separate file. On Unix, the client creates/checks owner-only directory and key-file permissions. On Windows, the file inherits the default DACL of the user's profile/application-data directory; this client does not inspect or guarantee that DACL. Secure Windows key storage is an explicit v1 prerequisite before relying on the client against other local accounts. The key is never sent to the relay or written to SQLite/logs. Same-user malware and administrators can still read or alter local state. If the outbox database is lost/corrupt while the key survives, the client refuses to restart sequence 1; it reports identity/evidence discontinuity and requires a new visible participant identity. If the key is lost, queued events remain unverifiable locally and the participant must rejoin with a new key. Neither condition is silently repaired.

## Reports and compatibility

Report schema version 1 contains the versioned session/participant envelopes, join events, participant events, signed receipts, signed relay notices, relay public key/fingerprint, final manifest, client versions and self-reported coarse OS fields, local-only unreceipted events, and verifier findings. It contains no peer address, machine username, hostname, full URL, prompt, command line, process inventory, or unrelated local data. JSON field ordering is not used for any signature; verification reconstructs the specified bytes.

Verification outcomes are `VALID`, `VALID_WITH_GAPS`, or `INVALID`. Invalid structure, unsupported version, malformed IDs/payloads, duplicate/conflicting IDs, invalid signatures, chain/hash mismatch, non-monotonic receipt order for a participant's event sequence, or receipt/manifest mismatch is `INVALID`. Missing interior/trailing participant sequence, missing evidence order, missing receipts, or local-only unreceipted events are `VALID_WITH_GAPS` when the included signed items are otherwise valid. A cryptographically valid self-contained report without a pinned relay key is labeled `relay_identity_unanchored`; none of these outcomes means no cheating occurred.
