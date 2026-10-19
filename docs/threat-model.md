# Threat model

**Status:** design threat model for voluntary, best-effort integrity. It is not a certification or a claim that cheating can be proven.

## Security objective

Make prohibited AI use difficult to perform on a monitored workstation and make protection loss, integrity failures, tampering, and suspicious attempts difficult to hide from the other participants. The software cannot establish what a person did on devices or systems outside its control.

## Assets and security properties

- Participant privacy: nickname-only identity; no peer IP disclosure, filesystem inventory, unrelated browsing history, prompts, keystrokes, credentials, or message contents.
- Competition integrity: explicit protection state, observations, blocked attempts, policy changes, and connectivity gaps are authenticated and reportable.
- Availability: participants can join, receive relay events, reconnect, and export a report.
- Endpoint safety: least-privileged UI and narrow privileged service with reversible, named OS policy.
- Evidence integrity: event signatures, per-participant sequence/hash chains, and relay receipts expose alteration and gaps in retained evidence.

## Actors and trust boundaries

| Actor | Capability | Treatment |
| --- | --- | --- |
| Honest participant | Uses agreed rules, may make mistakes | Normal user of the system. |
| Curious participant | Explores controls, alternate clients, and privacy | Assume they can inspect all local files and their own network traffic. |
| Bypassing participant, standard privileges | Can use portable apps, alternate browsers, DNS-over-HTTPS, proxies available to the user, local models, and configuration changes | Layered blocking/detection raises effort but is not a reliable guarantee. |
| Bypassing participant, administrator/root | Can stop services, edit rules, alter binaries, change boot/network setup, run VMs/containers, or replace the OS | Outside the endpoint trust boundary. Report loss and tampering signals when observable; do not claim prevention. |
| Malicious session participant | Can send malformed, false, replayed, delayed, or selectively omitted claims; can disclose own secrets | Authenticate each sender, validate schema/order, rate-limit, and preserve attributed evidence. Cannot be prevented from lying about their own workstation. |
| Relay operator or compromised relay | Sees connecting IPs and timing; can deny service, reorder/omit/fork fan-out, or expose retained data | Limit data and retention; verify signatures; give clients receipts/checkpoints; disclose operator trust. Signatures cannot guarantee relay availability or completeness. |
| Network attacker | Can observe or disrupt connections and attempt replay | TLS protects transport; signed events and sequence checks address replay/alteration. Availability remains vulnerable. |
| Second device / remote host | Phone, tablet, other computer, remote desktop, SSH host, or cloud browser runs outside the monitored endpoint | Not reliably detectable or controllable by this project. |

The local endpoint cannot attest that it is uncompromised merely by signing its own report. A session key proves which key signed an event, not that the OS observation was complete or truthful. The relay is not a trusted referee for local activity and peers should not receive endpoint network addresses.

## Threats and expected handling

| Threat | Best-effort response | Residual limit |
| --- | --- | --- |
| Kill client/service, stop watchdog, remove enforcement rules | Service-manager state, heartbeat loss, policy-state recheck, signed protection-loss event when possible | Root/admin can suppress final event; missing heartbeat is ambiguous between kill, crash, sleep, and network loss. |
| Edit local config, registry, or binary | Version/hash checks, signed registry metadata, policy revalidation, observable state transitions | A user controlling the endpoint can patch or replace the client and forge local observations. Secure boot/externally managed application control is outside normal BYOD assumptions. |
| DNS changes, raw IP, alternate resolver, browser DoH | Managed DNS/firewall where available; process and network observations; document unobserved paths | Encrypted DNS, direct IP, shared hosting, tunnels, and changing endpoints create coverage gaps. |
| VPN, proxy, Tor, SSH tunnel | Detect some known local applications/processes and report network-policy changes if observable | Tunnels can hide destination intent; disabling VPNs can disrupt legitimate CTF use and cannot be comprehensive. |
| Portable/renamed/recompiled AI clients or direct API requests | Block known destinations and observe selected process/network metadata | New binaries and custom clients have no universal identifying signature; server-side requests may be indistinguishable from permitted web traffic. |
| Local GGUF model, Ollama, llama.cpp, vLLM, LM Studio, custom endpoint | Detect known local ports/processes opportunistically; optionally block known local service ports | Custom ports, embedded libraries, compile-time changes, loopback, and private endpoints defeat broad detection. This can also collide with legitimate development tools. |
| Containers, namespaces, VMs, WSL | Host network controls, known runtime/process observations, explicit coverage status | Guest isolation and virtual network configuration make application attribution partial; participants with admin can create alternate environments. |
| Remote SSH/RDP/browser or second physical device | None beyond voluntary policy and session disclosure | Not reliably detectable. Workstation software cannot control another device or remote host. |
| Network interruption or machine sleep | Heartbeat/liveness state, locally queued signed events, gap and reconnect markers | Cannot identify cause from network loss alone; offline actions may not reach peers until reconnect. |
| Fake/spoofed telemetry | Sign sender identity, preserve raw event types and relay receipts, correlate independent participant observations | Malicious client can lie about its own sensor state; signatures attribute the claim rather than prove its truth. |
| Replay, sequence rollback, local clock manipulation | Per-participant monotonic sequence/hash chain, duplicate rejection, server receipt order/time, client version | Reinstall/rollback and key loss can create discontinuities; local wall time is not authoritative. |
| Older client/registry rollback | Signed releases/registry metadata and minimum-version policy controlled by session | Secure update enforcement is not available on unmanaged machines; user can run a patched build. |
| Relay compromise or equivocation | End-to-end event signatures, signed receipts/checkpoints, participant comparison, short retention | Relay can deny service or selectively withhold evidence; transparency requires checkpoint retention and comparison. |
| Malicious participant against another | No peer-to-peer connection; strict server authorization; nickname/key attribution; rate limits | Participants can still harass, spam, or make false claims; session moderation and appeal rules are needed. |

## Realistic guarantees

If the client and OS are functioning as assumed, the project can block a selected set of known destinations/processes, record selected control and connectivity state, authenticate events to ephemeral participant keys, and make gaps or inconsistencies visible in reports. Coverage differs by operating system and policy mode.

It cannot prove a participant did not use AI, prove intent from a blocked request, make bypass impossible, ensure an administrator cannot defeat local controls, attest that all relevant surfaces were observed, or infer cheating from a client disconnect. Reports must distinguish an observation from an inference and never automatically adjudicate a participant.
