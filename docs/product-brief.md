# Product brief

**Status:** proposed product definition, 19 October 2026.

## Problem

Participants sometimes agree to compete without AI assistance, but current event platforms mostly manage challenges and scores. Exam lockdown products control a narrow exam workflow; enterprise controls assume an organization owns the device or network. Informal technical competitions need a voluntary, cross-platform workstation signal and a shared record that is useful without pretending to prove honesty.

## Product definition

Equalturn is intended to pair a small local agent on each participant's Windows or Linux workstation with a neutral session relay. Participants join under public nicknames, enable an agreed competition profile, and receive a signed, privacy-minimized integrity timeline and final report. The local agent can enforce selected known network and application policies and report protection-state changes. Coverage is best effort and visibly bounded by OS support and the participant's control of the machine.

## Target users and first use case

- **Participants:** peers in CTFs, security challenges, programming competitions, and technical practice sessions.
- **Organizers:** small event hosts who want a clear, consent-based integrity procedure without adopting enterprise device management or proctoring.
- **First use case:** two or more participants create/join an ephemeral session, agree on a telemetry profile, start competition mode, exchange signed state and integrity events through a relay, and export a report at the end.

## Intended experience

Install the application, choose a nickname, create or join a session, review the agreed protection profile, start competition mode, then use the workstation normally. The client remains in the tray, reports relevant state changes, and marks uncertain coverage when controls stop or connectivity is lost. After the session, each participant can inspect and export the record.

## Explicit non-goals

- Proving that no AI was used or making bypass impossible.
- Controlling phones, tablets, second computers, remote SSH/RDP hosts, or an administrator/root-controlled machine.
- Replacing CTFd, learning-management systems, enterprise endpoint management, or proctored exams.
- Webcam proctoring, screenshots, screen recording, keystrokes, clipboard contents, prompts, passwords, unrelated URLs, or filesystem inventory.
- Blanket application allowlisting that prevents normal CTF tools from running.
- A custom kernel driver, transparent TLS interception, or an eBPF-based universal monitor in the first release.
- Discord or Telegram as the canonical session log.

## Assumptions

- Participants opt in and accept that a peer can cheat outside the monitored workstation.
- A relay operator is needed to avoid peer-to-peer address disclosure; the operator can observe network source addresses at the transport layer unless participants use a privacy network.
- A nickname is a session pseudonym, not a verified real-world identity.
- An event can be meaningful even when it is not conclusive: protection state unknown, a known destination matched, or an agent heartbeat missed.

## Success criteria for a future v1

- Windows and Linux clients can start, pause, stop, and report an explicit protection profile without repeatedly interrupting normal technical work.
- The session relay delivers authenticated, ordered events without direct participant connections.
- The report distinguishes verified events from uncertain observations and shows coverage gaps.
- Participants can understand what the tool records before starting, and can locally export and delete their report.
- A local administrator/root can still bypass controls; the product reports that boundary clearly.
