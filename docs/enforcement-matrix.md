# Enforcement and detection matrix

**Status:** feasibility assessment, not measured product performance. Current-source review: 19 October 2026. Reliability labels are qualitative design estimates, not benchmarks.

## Reading the capability labels

- **BLOCKABLE** — a known target or action can be denied while the selected OS control and policy remain active.
- **DETECTABLE** — the specific event can be observed and recorded under the stated configuration.
- **PARTIALLY DETECTABLE** — some cases can be observed, with documented blind spots.
- **NOT RELIABLY DETECTABLE** — there is no dependable general mechanism for identifying the behavior.

“BLOCKABLE” does not mean impossible to bypass. Where a bypassing user controls the OS or has administrator/root privileges, all local controls are subject to tampering. A lack of observation is not evidence that an action did not occur.

## Windows

| Technique | Privilege required | Capability | Bypass and compatibility limits | Qualitative reliability |
| --- | --- | --- | --- | --- |
| Windows Filtering Platform (WFP), user-mode management | Install/manage filters requires elevated service/admin; no custom kernel driver is proposed | **BLOCKABLE** for selected IP/port/protocol/app conditions; **PARTIALLY DETECTABLE** for matching flows if audit/event collection is configured | Admin can remove/alter filters; IPs may be shared or change; path/app attribution has limits; does not understand encrypted request intent or HTTPS paths | High for the exact filter while active; low against admin tampering. |
| Windows Defender Firewall rules | Elevated service/admin | **BLOCKABLE** for known addresses, ports, protocols, profiles, and application paths | Admin can change rules; DNS/CDN address churn; direct IP and tunnels; not a URL/content filter | High for a matching flow while active; low against admin tampering. |
| App Control for Business / WDAC | Policy deployment is normally managed/elevated; stronger tamper resistance depends on signed policy, Secure Boot, and deployment configuration | **BLOCKABLE** for covered executables, scripts, drivers, packaged apps, or allowlist scope | Restrictive policy can break CTF tooling; BYOD lacks externally managed policy trust; relaxed policy is not comprehensive; administrator can alter unmanaged configuration | High under a properly managed restrictive deployment; low/medium on voluntary unmanaged devices. |
| AppLocker | Admin to configure; Application Identity service required | **BLOCKABLE** for matching executable, script, installer, DLL, or packaged-app rules; **DETECTABLE** in audit mode | Microsoft documents that AppLocker does not meet Windows security servicing criteria; service/configuration is subject to local admin; file rename/copy changes rules depending on rule type | Medium for known rules, weak as a tamper boundary. |
| Process enumeration / process-start observation | Standard user can enumerate some process data; service permissions and ETW session access vary | **PARTIALLY DETECTABLE** for known running processes and starts | Names/paths can be changed; portable/recompiled/short-lived clients evade; command lines may expose secrets and should not be collected | Medium for known installed tools while observer runs; low for semantic identification. |
| ETW | Session control permissions vary; often service/admin for system-wide sources | **PARTIALLY DETECTABLE** for events from enabled providers | Not a universal app-behavior sensor; events can be lost or disabled; admin can interfere; do not collect arbitrary high-volume traces | Medium for selected provider events; not suitable as sole evidence. |
| DNS/network policy and browser DNS-over-HTTPS settings | Local policy/admin; managed DNS policy can require enterprise configuration | **BLOCKABLE** for names queried through controlled DNS; **PARTIALLY DETECTABLE** for known destinations | Browser DoH, custom resolver, raw IP, VPN, proxy, shared CDN, encrypted DNS, and custom endpoints evade name controls | Medium when managed and all traffic uses the resolver; low on BYOD. |
| Packaged application identity | Policy/admin for enforcement | **PARTIALLY DETECTABLE**; package-aware application rules can **BLOCKABLE** covered packaged apps | Only helps packaged apps with recognized identity; desktop/portable apps and browser-based services remain | High for a package under explicit policy; narrow coverage. |
| WSL 2 and virtualized networking | Host policy/admin; WSL Hyper-V firewall availability depends on Windows/WSL version and configuration | **PARTIALLY DETECTABLE** at host/virtual network and known process boundaries; selected network paths can be **BLOCKABLE** | Guest Linux process visibility is limited; network mode and version matter; guest-local endpoints and tunnels create gaps | Medium for host traffic under supported configuration; low for guest activity attribution. |
| Service stop/config modification | Admin controls service management; signed WDAC policy may raise cost when managed externally | **DETECTABLE** if a separate observer/relay notices state loss; final local event is not guaranteed | The same local admin can suppress the observer or network report; relay sees loss, not cause | Low to medium; remote heartbeat only establishes liveness. |

## Linux

| Technique | Privilege required | Capability | Bypass and compatibility limits | Qualitative reliability |
| --- | --- | --- | --- | --- |
| nftables/netfilter host rules | `CAP_NET_ADMIN` or root to install rules; narrow system service recommended | **BLOCKABLE** for known IP/port/protocol and selected interface traffic; **PARTIALLY DETECTABLE** if counters/logging are configured | Root can flush rules; nftables does not identify encrypted application intent; shared/changing IP, tunnels, namespaces, and loopback complicate scope | High for the exact active rule; low against root tampering. |
| systemd service/watchdog | System manager configuration usually root/admin; user service is weaker | **DETECTABLE** when service crashes or disappears while manager/observer remains | An administrator can stop/alter units; systemd will not make a manager-requested stop proof of malicious action; sleep/network loss is ambiguous | Medium for service liveness, not proof of protection. |
| cgroup v2 for service-owned process tree | Service-manager/cgroup permissions; root can move/alter processes | **PARTIALLY DETECTABLE** for processes launched under the service’s own cgroup | It does not create universal system-wide AI process identity; independently launched/renamed clients evade | High for the service-owned process tree, narrow coverage. |
| procfs process observation | Process visibility depends on `/proc` mount options, namespaces, and permissions | **PARTIALLY DETECTABLE** for process IDs, executable links, and selected metadata | Names/paths can be changed; containers and namespaces hide or remap process views; command-line collection risks secrets | Medium for visible known processes while observer runs; low semantic confidence. |
| DNS/network controls | Local resolver/firewall privileges | **BLOCKABLE** for names routed through controlled resolver; **PARTIALLY DETECTABLE** for observed endpoints | DoH/DoT, direct IP, VPN/proxy, shared IP, local DNS, and custom endpoints bypass; encrypted request contents remain opaque | Medium in managed resolver-only mode; low on BYOD. |
| Network namespaces and containers | Typically user namespace/runtime permissions; host firewall changes require root | **PARTIALLY DETECTABLE** from host network/process context; selected paths can be **BLOCKABLE** | Isolated networks and rootless containers alter visibility; root can create alternate namespaces; image/process naming is not identity | Medium for host-visible flows; low for in-container attribution. |
| eBPF observation/enforcement | Kernel support, capabilities, verifier acceptance, and often root; exact requirements vary by program type | **PARTIALLY DETECTABLE** for narrowly instrumented kernel events; some hooks can enforce selected policies | Kernel/version/API complexity; verifier and lifecycle failure; does not decrypt HTTPS or reveal intent; root can disable/change programs | Potentially high for a narrowly proven hook, not justified for initial version. Defer. |
| Wayland/X11 window observation | User-session access; compositor/protocol and permissions vary | **NOT RELIABLY DETECTABLE** as a cross-desktop page/content monitor; Wayland restricts other-client surfaces, X11 offers broader but invasive access | Browser URL/title capture is inconsistent and privacy-invasive; display servers differ; screenshots/keystrokes are out of scope | Low and unsuitable as an integrity guarantee. |
| Daemon/rule tampering | Root controls systemd, firewall, files, and kernel interfaces | **PARTIALLY DETECTABLE** from external heartbeat or a second trusted observer | Root can suppress local signals; relay cannot distinguish kill from crash/network failure | Low to medium, limited to liveness. |

## Cross-platform surfaces

| Surface | Capability assessment | Main blind spot |
| --- | --- | --- |
| Known hosted AI domains and provider APIs | **BLOCKABLE** at selected DNS/IP/firewall layers when endpoint is known; **PARTIALLY DETECTABLE** as destination metadata | New/custom domains, direct IP, shared hosting, encrypted DNS, tunnels, and APIs routed through allowed services. |
| Browser extensions and IDE plugins | **PARTIALLY DETECTABLE** when installed package IDs are visible or application control is managed | Browser profile isolation, extension renaming, portable browsers, custom IDEs, remote agents, and BYOK/provider routing. |
| Local model runtimes and localhost servers | **PARTIALLY DETECTABLE** via known processes/ports; loopback policy can block selected known ports | Custom builds, ports, embedded inference libraries, private address endpoints, containers, and renamed binaries. |
| Custom/self-hosted provider or model endpoint | **NOT RELIABLY DETECTABLE** by a brand registry alone | No universal signature distinguishes model inference from arbitrary HTTP/TLS or local computation. |
| VPN, proxy, Tor, SSH tunnel, remote browser | Known client may be **PARTIALLY DETECTABLE**; traffic may be **BLOCKABLE** only if an explicit competition policy blocks the transport | Traffic intent and remote destination are hidden; blocking may disrupt legitimate CTF workflows. |
| VMs, WSL, containers, remote hosts, second devices | **PARTIALLY DETECTABLE** for some local hypervisors/runtimes; remote/second-device activity is **NOT RELIABLY DETECTABLE** | Software on one endpoint has no control or reliable visibility into separate machines or remote sessions. |

## Implication for policy modes

The first future implementation should use an explicit **observe-only** mode to validate event quality, then a narrowly scoped **standard** mode for selected domain/network blocks and known runtime indicators. A future organizer-managed high-enforcement mode may use system application-control policy, but it is not suitable as the default BYOD assumption. Each report must include the active policy version and platform coverage, not a binary “clean/cheated” verdict.

Authoritative OS references are collected in [research sources](research/sources.md#operating-system-and-network-controls).
