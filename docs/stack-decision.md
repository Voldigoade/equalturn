# Stack decision

**Recommendation:** Rust for the shared client/event core, per-OS user-mode enforcement adapters, and initially the relay service; Tauri v2 with a TypeScript interface for the later unprivileged tray UI. Keep the UI process separate from the elevated Windows service or Linux system daemon. This is an architecture recommendation, not a prototype result.

## Why Rust

The endpoint service handles local IPC, event signing, registry parsing, process signals, and firewall configuration. Rust's checked ownership and thread-safety traits reduce common memory-lifetime and data-race errors in that privileged code. It can call both OS APIs through reviewed FFI boundaries. The framework already provides Windows service bindings and current Tauri tray support; the UI can remain low privilege.

Rust does not remove unsafe FFI risks, policy mistakes, administrator bypass, or API-version churn. Microsoft `windows-rs` documentation recommends focused bindings where possible, and notes that generated broad bindings create dependency weight and version churn. Pin the selected Windows bindings and isolate them behind a small adapter. The WFP user-mode management API should be tried before considering a kernel callout driver.

## Candidate comparison

| Candidate | Strengths here | Costs / concerns | Decision |
| --- | --- | --- | --- |
| **Rust** | Memory-safe defaults; no managed runtime; strong async/networking; credible Windows and Linux bindings; cross-build targets; Tauri tray option. | FFI remains unsafe; Windows binding metadata has churn; contributors need Rust; Tauri adds a webview and frontend dependency surface. | **Selected.** Best balance for a privileged endpoint component plus shared protocol. |
| **C++** | Most direct WFP and `libnftnl` access; strong Qt tray tooling; familiar native service ecosystem. | Memory safety is primarily discipline/tooling; CMake, ABI, and runtime packaging add work; UI framework is comparatively large. | **Runner-up.** Select if Windows binding probes show Rust cannot maintain the required API cleanly. |
| **Go** | Simple service/server concurrency, broad contributor pool, easy pure-Go cross-builds, official Windows service and syscall packages. | GC/runtime included; cgo complicates cross-compilation; no maintained WFP-specific wrapper found, leaving hand-maintained ABI glue in a critical subsystem. | Good alternative for a relay or command-driven monitor, but not the best shared privileged core. |
| **Zig** | Explicit allocation, direct C interop, strong cross-target build design, small-runtime potential. | Uneven platform/library support, rapidly evolving toolchain, smaller systems/UI ecosystem and contributor pool. | Too much ecosystem/toolchain risk for initial v1. |

| Other options | Role considered | Reason not selected as the primary stack |
| --- | --- | --- |
| --- | --- | --- |
| C# / .NET | Could make a Windows-only service and UI straightforward. | Does not provide the same shared Linux service core; a separate Linux implementation would be needed. |
| Java/Kotlin | Mature concurrency and UI libraries. | Runtime and packaging footprint, plus less direct OS firewall integration; does not reduce native per-OS work. |
| TypeScript/Tauri | Useful for a cross-platform settings/tray UI. | Must not own privileged policy or become the enforcement boundary; frontend code is a larger attack surface than a small native UI. |
| Swift | Good macOS integration. | macOS is not a first-class platform for this phase. |

## Proposed process boundaries

- **Unprivileged UI:** Tauri tray/settings/report viewer. It never runs elevated.
- **Local service:** Rust service with a small typed IPC surface. Windows uses Service Control Manager plus WFP user-mode management; Linux uses a systemd unit plus a dedicated nftables table. The service exposes no generic shell or arbitrary firewall commands.
- **Shared core:** registry parser, event schema, signing, local queue, report generator, and session protocol.
- **Relay:** one Rust service using HTTPS for session setup and WebSocket for live events, initially backed by PostgreSQL. No speculative microservices, cache, or blockchain.
- **Platform UI:** Windows and Linux tray behaviors are tested separately; Tauri does not make OS service semantics or UI behavior identical.

## Dependencies / primitives to evaluate in the next phase

- Rust stable toolchain and Cargo lockfile; `windows-rs`/focused Windows crates and `windows-service` for Windows.
- Tauri v2 tray-icon capability for the UI after the protocol prototype.
- `tokio` plus a small HTTP/WebSocket stack for the relay and network client; use standard TLS validation.
- SQLite for the local signed-event queue; PostgreSQL for relay session/event retention.
- Ed25519 (RFC 8032) for participant signatures; RFC 8785 JSON Canonicalization Scheme or a deterministic binary encoding before signing.
- `nft`/nftables interface on Linux and WFP user-mode APIs on Windows. Avoid custom kernel drivers and eBPF initially.
- TUF metadata for a later independently updated registry, rather than inventing a custom auto-update trust scheme.

These are candidates to pin and validate before source-level commitment. No benchmark or binary-size claim was made.

## Feasibility probes before enforcement implementation

1. Windows VM: open a WFP management session, add/remove one narrowly scoped test filter, and start/stop the service under an administrator account.
2. Linux VM: create/remove an Equalturn-owned nftables table/set atomically; confirm recovery after service crash and reboot.
3. Exercise local IPC permissions and ensure UI requests cannot become arbitrary privileged commands.
4. Build on Windows and at least Ubuntu and Fedora in CI; test with IPv4, IPv6, DNS, browser DoH, WSL, and a container network.

## Sources

Language and primitive references are listed in [research sources](research/sources.md). Context7 was used on 19 October 2026 to query current Tauri v2 tray guidance and Microsoft's `windows-rs` binding guidance; those queries confirmed the tray feature and the need to keep generated/unsafe OS bindings narrow. They do not establish successful product integration.
