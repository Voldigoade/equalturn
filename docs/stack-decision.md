# Stack decision

**Decision:** use Rust as the primary language for the shared protocol, CLI client, report verifier, and relay. This is now the implemented Milestone 1 architecture. Windows/Linux enforcement adapters, a privileged service, and a GUI remain outside this milestone and are not yet validated.

## Why Rust wins for this project

The shared protocol, local queue, relay, and report verifier need consistent validation, deterministic byte encoding, async networking, and a small portable distribution. A single Rust workspace keeps these boundaries reviewable and avoids separate client/relay protocol implementations. Rust's ownership and thread-safety checks reduce some memory-lifetime and data-race errors; they do not remove unsafe FFI, protocol, configuration, dependency, or endpoint trust risks. The workspace forbids `unsafe` Rust.

Rust also has credible Windows and Linux system API access for a future OS-specific service, but no such bindings are used yet. WFP/nftables implementation and actual service/package behavior require their own platform research and VM validation. UI technology is deliberately unselected; any later tray process must remain unprivileged and separate from an elevated service.

## Implemented workspace and dependencies

The workspace targets Rust 2024, declares Rust 1.94 as its minimum version, and tracks the stable toolchain with `rustfmt` and `clippy` in `rust-toolchain.toml`. Applications are built with the committed `Cargo.lock`. Dependency versions below reflect the current manifests/lockfile and should be rechecked during upgrades.

| Concern | Implemented choice | Boundary |
| --- | --- | --- |
| CLI | `clap` derive, `tokio` async entry point | Commands cover session lifecycle, status/participants, watch/sync, synthetic events, report export and verification. No GUI or auto-update. |
| HTTP relay | Axum 0.8 + Tokio | Bounded JSON lifecycle/status/report endpoints and WebSocket upgrade. TLS uses `axum-server` with rustls. Plain HTTP is refused except explicit loopback development mode. |
| HTTP client / WebSocket | `reqwest` with rustls; `tokio-tungstenite` | HTTPS validation, redirects disabled, request timeouts, bounded response/frame sizes, signed challenge reconnect. |
| Durable relay storage | SQLx 0.9 + PostgreSQL; SQL migrations | Transactions serialize participant event-chain advancement and evidence ordering. Relay state is not required to compile the workspace. |
| Local queue | SQLx + bundled SQLite | WAL and full synchronous durability for signed events/outbox/receipts. Session keys are separate files, not rows in the event database. |
| Protocol serialization | Serde JSON for wire/report envelopes; hand-defined fixed binary bytes for signatures | Does not sign arbitrary `serde_json::to_vec` output. Protocol byte definitions and version compatibility are in [protocol-v1.md](protocol-v1.md). |
| Cryptography | `ed25519-dalek` 3.x, `getrandom`, `sha2`, `zeroize`, UUIDv4 | Ed25519 key per participant/session, domain separation, SHA-256 chain hashes/fingerprints, OS randomness. No custom cryptography. Standard algorithm reference: [RFC 8032](research/sources.md#stack-and-protocol). |
| Reports | Shared Rust report model and offline verifier | Checks included events, key bindings, chains, relay receipts/notices, manifest, and gaps; JSON and Markdown output. |
| Logging | `tracing` and `tracing-subscriber` | Structured relay JSON logging; secrets and source addresses are excluded from application evidence. Operators must still review infrastructure/proxy logs. |

## Candidate comparison

| Candidate | Strengths here | Costs / concerns | Decision |
| --- | --- | --- | --- |
| **Rust** | Memory-safe defaults; no managed runtime; strong async/network ecosystem; one shared protocol/client/relay implementation; credible system API access. | Unsafe FFI remains possible; Windows bindings and native service semantics need later validation; contributor learning curve. | **Selected and used.** Best balance for a shared session protocol plus future small OS-specific services. |
| **C++** | Direct WFP and `libnftnl` access; mature native service and Qt tray ecosystem. | Memory safety relies more on discipline/tooling; more ABI/build/packaging complexity; no advantage for shared wire semantics that offsets this here. | **Runner-up.** Revisit if Rust's native API boundaries prove unmaintainable in platform probes. |
| **Go** | Straightforward relay concurrency, broad contributors, easy cross-builds. | Would add a second implementation language or leave endpoint bindings outside the shared core; cgo/native system bindings complicate cross-compilation. | Credible relay-only alternative, but adds little to the selected all-Rust M1. |
| **Zig** | Explicit allocation and direct C interop. | Smaller systems/network ecosystem, contributor pool, and more toolchain/platform churn. | Too much ecosystem risk for the initial implementation. |

C, C#, Java/Kotlin, TypeScript/Tauri, and Swift were considered for narrower roles. C# could be attractive for Windows-only services but would still leave Linux native work separate. TypeScript/Tauri may fit a later unprivileged UI, but frontend code must not become the privileged boundary. Swift is not relevant to the Windows/Linux first-class targets. No frontend framework or tray UI has been selected.

## Future OS-specific boundary

**Proposed design, not implemented:** a small OS-specific service owns only explicitly named competition policy operations; a separate unprivileged UI communicates through typed IPC. Windows research points to user-mode WFP management first; Linux research points to a dedicated nftables table with narrow privilege and systemd lifecycle. Do not expose arbitrary shell/firewall commands. Do not add a kernel driver or eBPF without evidence of a requirement. Authoritative OS references and known limitations are in [research sources](research/sources.md#operating-system-and-network-controls) and the [enforcement matrix](enforcement-matrix.md).

## What remains to validate

- Supported Windows/Linux versions, system API crates, service/daemon packaging, and install/uninstall recovery.
- Whether Rust bindings provide a maintainable narrow WFP boundary; whether Linux nftables integration is safely transactional under expected permissions.
- Windows workspace tests pass in CI, while Linux/PostgreSQL integration tests pass on Linux. OS-specific enforcement, service packaging, install/uninstall recovery, and signing remain unvalidated.
- Dependency update/security review, relay deployment model, and key/backup operations before public hosting.
- A later unprivileged UI choice after protocol and local-service boundaries are stable.

Research and current-library documentation sources are indexed in [research/sources.md](research/sources.md). Context7 was used during Phase 0 for Tauri v2 tray and `windows-rs` guidance; those queries informed only later design candidates and did not validate integration.
