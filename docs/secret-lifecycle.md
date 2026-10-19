# Secret lifecycle

**Reviewed:** 19 October 2026 against the Milestone 1.5 Stage 5 implementation. This describes secret handling in the current non-enforcing client and relay; it is not a promise that process memory can be erased perfectly.

## Participant signing keys

`SigningSeed` owns a `zeroize::Zeroizing<[u8; 32]>`. It is intentionally not `Clone`, `Debug`, or serializable. `SigningIdentity` also has no `Debug` or serde implementation. Callers borrow a seed to construct an identity and receive another `SigningSeed` wrapper when exporting one for persistence. Client persistence keeps seed-bearing read/write buffers in zeroizing allocations. Windows protects the persisted participant seed with current-user DPAPI; Unix stores it in an owner-only key file. See [client key storage](key-storage.md) for file formats and OS boundaries.

The locked `ed25519-dalek` version is 3.0.0. Its default feature set includes `zeroize`, and its `SigningKey` drop implementation zeroizes its internal secret-key array. This reduces ordinary heap lifetime; it does not guarantee erasure from registers, compiler-generated copies, swap, hibernation, crash dumps, a debugger, or a compromised process. `SigningIdentity` does not expose the library's `Clone` implementation.

The [`ed25519-dalek` 3.0.0 feature documentation](https://docs.rs/crate/ed25519-dalek/3.0.0) and [`SigningKey` API](https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/struct.SigningKey.html) describe the default `zeroize` feature and its `ZeroizeOnDrop` implementation. The request buffer uses [`bytes` 1.12.1 `Bytes::from_owner`](https://docs.rs/bytes/1.12.1/bytes/struct.Bytes.html#method.from_owner), which retains the owner until the final `Bytes` clone is dropped. These upstream references were checked on 19 October 2026; the lockfile records the versions used here.

On Windows, DPAPI uses the current user scope, forbids UI, and does not request machine scope. The DPAPI-owned output allocation is overwritten before `LocalFree`; the returned plaintext is copied into a zeroizing seed wrapper. Current-user DPAPI protects a file copied to a context without the user's DPAPI material; it does not stop code already executing as that user from asking Windows to unprotect it. Administrators, account compromise, process-memory compromise, and backups remain outside that guarantee.

On Unix, mode and descriptor checks protect against other unprivileged local accounts when the filesystem honors them. They are access controls, not encryption. `root`, the owning user, same-user code execution, process-memory access, and backups can read the seed.

## Invitation codes

Invitation codes are 26 characters selected from a 31-symbol alphabet (about 129 bits of entropy). `InvitationCode` owns its string in `Zeroizing<String>`, is not cloneable or debuggable, validates on construction/deserialization, and retains the existing canonical JSON string representation. Request `Debug` output redacts the code. The relay computes a verifier and drops the typed code before database work; it persists only the verifier, not the raw code.

The client serializes HTTP JSON into a `Bytes` owner backed by `Zeroizing<Vec<u8>>`; that owner clears its allocation when the request body's final owner is dropped. The relay framework's raw incoming HTTP buffers and copies inside TLS, kernel networking, or allocator internals are not explicitly zeroized. Those copies are short-lived and the body is bounded, but this does not provide memory-erasure assurance.

The CLI consumes its owned code argument into `InvitationCode::parse_owned`, which zeroizes that Rust `String` after parsing. The operating system's original argument buffer, shell history, terminal scrollback, clipboard, and the intentionally displayed creator code are outside that wrapper. Until a safer input channel is designed, avoid putting a join code in a command line on shared or multi-user systems. Invitation possession is a bearer capability until the session starts or expires.

## Relay database credentials

`RelayConfig` takes ownership of the database URL in a non-cloneable `Zeroizing<String>` and its `Debug` output replaces the entire URL with `[REDACTED]`. The relay CLI accepts `EQUALTURN_DATABASE_URL` or `--database-url`; prefer the environment form over a command-line value to avoid shell history and process argument listings. Environment storage, the process startup argument area, SQLx parsing/connection state, and OS-managed memory are not all under this code's zeroization control. The application does not log or include the URL in evidence or user-facing errors.

## Relay signing key and reconnect challenges

The relay signing seed remains plaintext at rest in the relay identity file. Unix uses an owner-controlled directory/file and atomic no-replace publication. Windows relies on the inherited parent DACL and does not inspect or harden it. Local temporary/read buffers are zeroizing, but this does not protect against the service account, administrator/root, memory compromise, filesystem snapshots, or backups. Production operators should use a dedicated account and restrictive directory ACLs; KMS/HSM/secret-manager integration is future work.

Reconnect challenges are short-lived public nonces sent to the participant. They are not bearer secrets: the participant proves possession by signing the challenge with its session key. Challenge signatures and public protocol data are expected on the wire. No long-lived reconnect bearer token is stored.

## Logging and errors

- No private seed, invitation, database URL, request body, authentication signature, or transport address is written to application logs or reports.
- Sensitive protocol types intentionally omit `Debug`; request types with a required diagnostic representation redact invitation values.
- CLI errors use stable user-facing descriptions. SQLx, DPAPI, and filesystem internals are not formatted into ordinary command-line errors.
- Tests use deterministic public fixtures only. They do not snapshot or print generated private seeds.

These protections reduce accidental disclosure and exposure from storage copies. They do not defend against a participant controlling the host, same-user arbitrary code, administrator/root, a compromised relay process, or a compromised build/runtime environment.
