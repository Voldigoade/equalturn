# Local participant key storage

**Status:** implemented for the Milestone 1.5 participant-session identity file. Checked 19 October 2026.

Each client generates a fresh Ed25519 identity for one participant in one session. The key is stored separately from the SQLite event queue and is never sent to the relay. The local store identifier in the envelope binds the key file to the SQLite store that created it; loss of the database does not silently create a replacement identity.

## Version 2 envelope

The file is bounded to 16 KiB and has a fixed 59-byte header followed by one platform-protected payload. Integer fields are big-endian.

| Offset | Bytes | Meaning |
| --- | ---: | --- |
| 0 | 8 | `ETKEY\0\0\0` magic |
| 8 | 2 | Format version (`2`) |
| 10 | 1 | Protection scheme: `1` Unix owner-only; `2` Windows current-user DPAPI |
| 11 | 16 | Session UUID bytes |
| 27 | 16 | Participant UUID bytes |
| 43 | 16 | Local SQLite store binding |
| 59 | remaining | Protected seed payload; no trailing fields |

Unknown versions, unsupported platform schemes, mismatched session/participant/store bindings, truncated files, oversized files, and invalid protected data fail closed. Windows never falls back from DPAPI to plaintext. Unix stores the seed as a fixed 32-byte payload and relies on owner-only directory/file permissions; it does not encrypt the seed.

## Windows

The Windows implementation calls `CryptProtectData` and `CryptUnprotectData` through the maintained `windows` Rust binding (`windows` 0.62.2). It uses the default current-user scope, passes `CRYPTPROTECT_UI_FORBIDDEN`, supplies the envelope header as optional entropy with a domain string, and does not set `CRYPTPROTECT_LOCAL_MACHINE`. DPAPI output buffers are cleared and released with `LocalFree`.

The actual Windows CI tests run as a Windows user and cover save/reopen, absence of the raw 32-byte seed in the persisted v2 file, ciphertext mutation, truncation, session-header binding, and first-load migration of the recognized Milestone 1 v1 format. The v1 migration writes and flushes a protected temporary file before replacing the old file. Filesystem snapshots, backups, or remapped storage may retain old plaintext blocks; the client does not claim forensic secure deletion.

DPAPI current-user protection is an at-rest boundary against other ordinary accounts under normal Windows profile protection. It does not protect against code running as the same user, an administrator/root-equivalent attacker, process-memory disclosure while the client runs, or a broken/roaming profile. Microsoft documents that a password reset performed by an administrator may make protected data unrecoverable without recovery-agent or domain backup-key support.

## Unix

The client creates the data and identity directories with owner-only permissions and creates new key files with mode `0600`; it rejects group/world-accessible directories and files. The key payload remains plaintext for the owning account. This does not protect against root, same-user arbitrary code execution, backups, disk-level disclosure, or memory compromise while the client runs. Filesystem path race and ownership hardening is tracked separately in the Unix storage checkpoint.

## Compatibility

Only the exact 88-byte Milestone 1 v1 layout is accepted as legacy data. On Windows it is validated against the requested session, participant, and local store, checked against a stored profile when present, and upgraded to DPAPI before `load_identity` returns. On Unix it remains readable for existing sessions and is not rewritten. A v2 parse or DPAPI failure never falls back to v1 parsing.

## Authoritative references

- Microsoft, [CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata), accessed 19 October 2026: default user credentials, optional entropy, `CRYPTPROTECT_UI_FORBIDDEN`, `CRYPTPROTECT_LOCAL_MACHINE`, and `LocalFree` ownership.
- Microsoft, [CryptUnprotectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata), accessed 19 October 2026: integrity validation and the matching deallocation requirement.
- Microsoft-generated Rust docs for [`windows` 0.62.2 `CryptProtectData`](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Security/Cryptography/fn.CryptProtectData.html), [`CryptUnprotectData`](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Security/Cryptography/fn.CryptUnprotectData.html), and [`LocalFree`](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Foundation/fn.LocalFree.html), accessed 19 October 2026.
