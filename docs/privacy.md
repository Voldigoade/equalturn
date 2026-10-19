# Privacy and reporting model

**Status:** proposed defaults requiring review before implementation.

## Data minimization principle

Collect only the facts needed to show session membership, protection state, selected enforcement observations, connectivity gaps, and report integrity. A competition agreement should clearly state which policy mode is active and what that mode can observe. Privacy modes must describe actual differences in data collection, not imply stronger guarantees than the OS provides.

## Proposed modes

| Mode | Collection | Appropriate use |
| --- | --- | --- |
| **Observe-only** | Client/service version, platform family/version bucket, protection state, service/relay liveness, policy errors, selected process indicators, and optionally known destination categories. No destination detail is shared unless the session opts in. | Initial feasibility and compatibility sessions. Does not claim to block AI access. |
| **Standard** | Observe-only data plus known blocked-domain/IP rule hits, known blocked application launch indicators, and policy changes. Send only matched provider/product category and rule ID by default; avoid raw URL, path, process command line, and arbitrary destination strings. | Proposed default for a voluntary session after the block policy is reviewed. |
| **Destination detail opt-in** | Standard data plus the exact matched hostname or remote IP for selected network events, disclosed to all participants before the session starts. Never include path, query string, request body, or content. | Sessions where participants explicitly agree that more specific destination evidence is useful. |

An eventual organizer-selected mode must be visible to every participant before joining and remain fixed during an active session, except that the report records an explicit transition. A less invasive mode can be selected at any time; changing to a more detailed mode requires renewed participant consent.

## Collect

- Session-scoped random ID, participant public-key ID, and user-chosen nickname.
- Session start/end, join/leave, client version, broad OS/version bucket, and protection-state transitions.
- Signed heartbeat sequence and relay receipt time; connectivity stale/reconnect intervals.
- Policy version and explicitly selected enforcement mode.
- Rule ID and coarse provider/product category for blocked events; exact hostname/IP only in destination-detail mode.
- Known application identifier or registry indicator ID when a block rule matches; use a stable package/product ID where available rather than user-specific filesystem path.
- Protection failures, policy install/remove results, integrity check result categories, and termination reason.
- Versioned, signed event envelope and server receipt metadata needed to verify ordering and report gaps.

## Deliberately do not collect

- Keystrokes, clipboard, screenshots, microphone, webcam, or screen recordings.
- Prompt text, chat contents, API request/response bodies, source code, files, or full filesystem inventory.
- Passwords, access tokens, cookies, environment variables, or command-line arguments.
- Complete browser history, arbitrary page titles, full URLs, query strings, or unrelated DNS/network logs.
- User account name, real name, email address, machine hostname, MAC address, or peer-visible IP address.
- All running processes or installed software as a general inventory. Observe only selected known indicators required by the chosen policy.

Some OS/network APIs expose more information than this design needs. The implementation must discard it at collection time, not rely on downstream redaction. Any exception should be narrowly justified, documented, and separately consented.

## Who sees what

- The local participant sees their own detailed event timeline and report.
- Other participants see the nickname, session-scoped public-key ID, protection state, signed rule-hit category, state/liveness transitions, and events the selected mode allows.
- Exact hostname/IP is visible to peers only in Destination detail opt-in mode.
- The relay sees source IP and connection metadata because it terminates the connection. It should not expose those values to peers or include them in peer reports. Relay operators must document access, retention, and legal disclosure handling.
- The client keeps the private session key local and removes it at session expiry/explicit deletion. Key loss should be visible as an identity discontinuity.

## Retention proposal

Keep the relay’s active-session state only while active and a short reconnect grace period. Proposed deletion: seven days after normal end or last heartbeat for an abandoned session, with a separately documented maximum backup-expiry window. This is a product policy proposal, not an implemented retention guarantee. Until that policy is implemented and auditable, the relay should store no production session data. Participants can export a local report before expiry; local export retention is controlled by that participant.

## End-of-session report

Produce a portable JSON evidence bundle and readable HTML/PDF view from the same canonical data. Include:

- nickname and session ID; participant key fingerprint; client and registry versions;
- server-ordered session start/end, elapsed time from relay receipts, participant join/leave times, and explicit clock uncertainty;
- protection state timeline, enforcement mode, and platform coverage summary;
- blocked rule hits by provider/product category, with exact destination only if consented;
- policy failures, client/service liveness gaps, reconnects, queued/offline events, and termination reason;
- per-participant sequence/hash chain status, server receipt range, missing sequence ranges, and signed relay checkpoint/receipt verification status;
- explicit statement that signed events authenticate a key’s claim, not the truth or completeness of local observation;
- no automatic “cheated” label. The report presents observations, gaps, and limitations for participants to interpret.

Do not include a browsing-history appendix. Page-level tool history is not necessary to show the proposed controls and would create substantial privacy risk; it is out of scope unless a future, separately reviewed, explicit telemetry mode can establish necessity and consent.
