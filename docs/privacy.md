# Privacy and reporting model

**Status:** Milestone 1's data model and collection boundaries are implemented in the client, relay, and report verifier. The telemetry modes below are future design options; none is selectable or implemented. The implementation collects no workstation activity telemetry. Verification results and deferred work are recorded in [implementation-roadmap.md](implementation-roadmap.md).

## Milestone 1 collection

The client currently collects or creates only the data needed to join and verify a session:

- Session UUIDv4, participant UUIDv4, ASCII nickname, and a fresh Ed25519 public/private identity for that participant/session.
- Signed session events, their per-participant sequence/hash chain, local client timestamp, and payload. Join payloads include client version and coarse OS family/bucket, reported by the participant client. The only AI-related payload supported by the CLI is explicitly synthetic and includes a user-supplied lowercase hostname.
- Relay receipts, liveness notices, session lifecycle state, and a signed final manifest.
- Local SQLite profile/outbox/evidence state and a separate key file so unacknowledged events can be retried. Signing secrets are not stored in SQLite or sent to the relay.

The report JSON contains protocol and report-schema versions, session ID, participant IDs/nicknames/public keys/fingerprints, participant-reported client and coarse OS metadata, participant events, relay receipts/notices, relay identity, final manifest when available, and local-only unreceipted events. Verification status is computed from the included evidence. Participant timestamps are untrusted; relay receive times/order are relay claims.

This milestone does not observe protection state, application/process launches, network requests, browser/IDE activity, or workstation integrity. It does not consume the AI registry. There is no tray UI, enforcement service, privacy-mode selector, or hidden telemetry collector.

## Deliberately not collected

The client, relay evidence model, and report format do not collect or include:

- Keystrokes, clipboard contents, prompts, chat messages, source code, screenshots, screen recordings, microphone/webcam data, or request/response bodies.
- Passwords, access tokens, cookies, or unrelated environment values. Required configuration such as the relay URL and relay database URL is read for operation and is not put into session evidence or application logs.
- Browsing history, page titles, full URLs, query strings, DNS history, or unrelated network logs.
- Real name, email, machine username, hostname, MAC address, hardware serial, filesystem inventory, or process/software inventory.
- Peer network addresses in participant-visible session objects or reports.

The invitation code and synthetic event hostname are explicit CLI inputs, not passively observed activity. The CLI necessarily parses its supplied arguments, but it does not record the general argument list as telemetry. Invitation input has OS-visible command-line lifetime limitations described in [secret lifecycle](secret-lifecycle.md). No participant is labeled as having cheated based on an event or liveness state.

## Who sees what

- A participant sees the shared pseudonymous participant roster, signed shared events, liveness state, receipts, and relay report. Local data also includes that participant's own pending signed events and their private key file.
- Other participants receive public keys, key fingerprints, nicknames, session-scoped participant IDs, signed events and relay evidence. They do not receive another participant's source IP from the API or report.
- The relay operator necessarily sees source IP addresses and connection timing at the transport layer. The application evidence schema does not persist or fan out these values. Deployment reverse proxies, firewalls, cloud providers, and operational logging may record transport metadata independently; the operator must document and control those systems.
- The relay signing private key stays in the relay's persistent identity file. It is not returned to participants; only its public key/fingerprint are sent. Session participant keys are stored locally in separate key files and are not global machine identities.

The relay key file is plaintext at rest. Unix relies on owner-only directory/file permissions; Windows inherits the parent directory's DACL, which operators must restrict to the relay account and administrators. See [relay key storage](relay-key-storage.md) for creation, recovery, backup, and platform limits.

Participant seed, invitation, and database credential handling is described in [secret lifecycle](secret-lifecycle.md). Current-user DPAPI does not defend against code running as the same Windows user; Unix owner-only permissions do not defend against the owner or `root`; relay keys remain plaintext at rest.

The CLI reports the relay key fingerprint during create/join and saves the relay public key with the local profile. This supports checking future relay responses against the same key for that locally stored profile. A self-contained exported report proves only that its receipts/manifests verify under the included relay key; to identify the expected relay, participants must compare or pin its fingerprint through a trusted out-of-band channel.

## Retention and deletion

**Implemented relay policy:** the relay periodically removes unstarted sessions 24 hours after creation, ended sessions seven days after their end timestamp, and active sessions after seven days without activity/heartbeat. The cleanup task runs hourly. Deletion cascades through session participants, event evidence, receipts, notices, and manifest in the primary PostgreSQL database.

This code cannot govern backups, database snapshots, hosting-provider logs, or reverse-proxy logs. Backup expiry and an operational deletion/incident policy remain operator responsibilities before public hosting. The participant's SQLite event store, key files, and exported reports remain local until the participant removes them. Losing the local database/key can create a visible identity or evidence discontinuity; the client refuses to silently recreate an existing identity. Platform-specific hardening/ACL assurance must be verified on supported Windows builds.

## Future telemetry modes

These are proposals only. Milestone 1 has no mechanism to select them or collect their data.

| Proposed mode | Potential collection | Required design review |
| --- | --- | --- |
| Observe-only | Version/platform, protection state, selected integrity checks, and liveness | Define exactly what each OS primitive reveals and how failures become visible. |
| Standard | Observe-only plus selected known blocked-rule/application matches | Prefer rule IDs and coarse categories; test false positives and consent before enabling. |
| Destination detail opt-in | Standard plus matched hostname or remote IP for selected events | Require explicit session-wide consent; exclude URL paths, queries, request bodies, and content. |

Any future mode must discard unnecessary data at collection time and describe real differences to participants before joining. A change to a more detailed mode requires renewed consent and a visible report transition. No mode can make device-owner telemetry trustworthy against a participant with administrator/root control.

## End-of-session report semantics

The current JSON bundle is canonical for evidence; a Markdown summary is rendered from the same bundle. The verifier returns `VALID`, `VALID_WITH_GAPS`, or `INVALID`. It checks participant signatures and chains, relay signatures, matching event receipts, evidence ordering, participant heads, and the final manifest. A report without a relay key pinned out of band is labeled as unanchored relay identity.

`VALID` means the included evidence is internally consistent and signatures verify under the included keys. `VALID_WITH_GAPS` means the signed evidence is internally valid but there are missing sequences/orders, local-only events, absent manifest/evidence, or other incompleteness findings. `INVALID` indicates malformed or conflicting evidence or a failed signature/integrity check. None of these statuses establishes that no AI was used, that the client observed the whole workstation, or that all relay evidence was disclosed. Reports use terms such as synthetic event, participant-reported metadata, liveness gap, and unreceipted local event; they do not infer intent or assign a cheating verdict.
