# Product brief

**Status:** Equalturn's product direction remains a voluntary, best-effort integrity environment. Milestone 1 is a locally verified, non-enforcing Rust CLI/relay implementation; this brief separates what exists from the future product.

## Problem

Participants sometimes agree to compete without AI assistance, but existing event platforms mostly manage challenges and scores. Exam lockdown products control a narrow exam workflow; enterprise controls assume an organization owns the device or network. Informal technical competitions need a voluntary, cross-platform way to share integrity claims without pretending to prove honesty.

## Product definition

The intended product pairs a local agent on each participant's Windows or Linux workstation with a neutral session relay. Participants join under public nicknames, choose an agreed competition profile, and receive a signed, privacy-minimized integrity timeline and final report. Future local controls may block selected known network destinations or applications and report protection-state changes. Any such coverage will remain bounded by operating-system capability, registry quality, network configuration, and the participant's control of the machine.

## Milestone 1 implementation

The current CLI and relay implement pseudonymous session create/join, invitation codes, fresh per-session signing identities, signed event chains, relay receipts and notices, heartbeats/liveness, reconnect and replay, a durable local outbox, session start/end, and JSON/Markdown report export with offline verification. The only AI-related CLI event is a user-triggered **synthetic** event. Milestone 1 does not inspect, block, or detect workstation AI use and does not consume the AI research registry. See [architecture.md](architecture.md) and the normative [protocol-v1.md](protocol-v1.md).

## Target users and primary use case

- **Participants:** peers in CTFs, security challenges, programming competitions, and technical practice sessions.
- **Organizers:** small event hosts who want a clear, consent-based integrity procedure without enterprise device management or proctoring.
- **Primary use case:** two or more participants create or join an ephemeral pseudonymous session, keep a shared authenticated event record through a relay, and export/verify a report at the end. The future competition-mode client would add explicitly disclosed workstation controls.

## Intended future experience

Install the application, choose a nickname, create or join a session, review an agreed protection profile, start competition mode, then use the workstation normally. A future client should operate quietly while reporting selected protection-state changes and showing uncertainty when controls stop or connectivity is lost. Each participant should inspect and export the final record.

## Explicit non-goals

- Proving that no AI was used or making bypass impossible.
- Controlling phones, tablets, second computers, remote SSH/RDP hosts, or an administrator/root-controlled machine.
- Replacing CTFd, learning-management systems, enterprise endpoint management, or proctored exams.
- Webcam proctoring, screenshots, screen recording, keystrokes, clipboard contents, prompts, passwords, unrelated URLs, or filesystem inventory.
- Blanket application allowlisting that prevents normal CTF tools from running.
- A custom kernel driver, transparent TLS interception, or universal eBPF monitor.
- Discord or Telegram as the canonical session log.

## Assumptions and success criteria for a future v1

Participants opt in and accept that a peer can cheat outside the monitored workstation. A relay operator is required to avoid direct participant connections; that operator can see source addresses and connection timing. A nickname is a session pseudonym, not a verified real-world identity.

A future v1 should let supported Windows and Linux clients activate an explicit protection profile without repeatedly interrupting normal work, deliver authenticated ordered events without peer-to-peer networking, and provide reports that distinguish verified events from uncertain observations and gaps. Participants must understand collection before starting and be able to export/delete local records. An administrator/root user can still bypass local controls; the product should report that limitation clearly.
