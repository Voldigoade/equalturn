# Relay signing-key storage

The relay keeps one Ed25519 signing seed in the file selected by `--identity-file` (default: `data/relay-ed25519.key`). It uses this key to sign relay receipts, notices, and session manifests. This local server state is separate from PostgreSQL and participant keys. The seed is plaintext at rest; filesystem access controls are the protection for this milestone.

## Unix behavior

The relay creates missing parent directories with mode `0700`, then opens the final parent directory without following a symlink and retains that directory handle for key-file operations. The opened directory must be owned by the effective user, have no group or other access, and give that user full directory access. The key is opened relative to that handle. Existing keys must be regular files owned by the effective user, have no group or other access, be readable by the owner, and contain exactly 32 bytes. Symlinks, FIFOs, directories, unsafe permissions, malformed lengths, and read errors fail closed.

New keys are written to a unique `0600` temporary file in that directory. The relay flushes the file, publishes it with an atomic no-replace hard link, removes the temporary name, and synchronizes the directory. A concurrent relay start cannot replace the winning key or observe a partly written final key. A failed pre-publication write removes the temporary file; cleanup errors are returned. The key file remains plaintext and is not protected from the account owner, `root`, memory inspection, disk snapshots, or backups.

## Windows behavior

The relay writes a unique temporary file with create-new semantics, flushes it, then publishes it with a same-directory hard link that cannot replace an existing name. It removes the temporary name after publication. This requires a filesystem that supports hard links within the relay directory; if that operation is unavailable, identity creation fails closed. When loading, it opens the final path with `FILE_FLAG_OPEN_REPARSE_POINT` and rejects reparse points based on metadata from the opened handle. The file inherits its default DACL from the parent directory; the relay does not inspect or rewrite that ACL, and it does not validate the DACL or reparse status of parent path components. Configure a service-controlled parent directory path so only the dedicated relay service account and the required administrators can access it. Windows does not provide the Unix directory `fsync` behavior through this implementation, so it flushes the key file but does not claim power-loss durability for the directory entry.

Microsoft documents that a file created without an explicit security descriptor inherits ACLs from its parent and that `FILE_FLAG_OPEN_REPARSE_POINT` opens a reparse point itself instead of following it. Those behaviors support the Windows choices above; they do not establish that a particular deployment's inherited ACL is suitably restrictive. Sources checked 19 October 2026: [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew), [Reparse Points and File Operations](https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points-and-file-operations), and [Security Descriptors for New Objects](https://learn.microsoft.com/en-us/windows/win32/secauthz/security-descriptors-for-new-objects).

## Operations and limitations

- Run the relay under a dedicated operating-system account and keep its data directory private. Do not place the identity file in a shared writable directory.
- Back up the relay seed only through an encrypted, access-controlled backup that is available to the same authorized operators as the relay database backup. A restored relay key preserves verification continuity; a lost key cannot be reconstructed from participant reports or PostgreSQL metadata.
- Do not silently delete a malformed key file to make the relay start. Investigate and restore it from a trusted backup, or deliberately rotate to a new relay identity and communicate the new fingerprint out of band. Existing reports remain verifiable under the public key they include, but a new key is a distinct relay identity.
- Filesystem protections do not defend against `root`/administrators, compromise of the service account, process-memory compromise, a malicious filesystem, or a compromised backup system. Unix owner-only permissions are access control, not encryption.
- Production deployments may later replace local files with a secret manager, KMS, or HSM, but this repository does not implement or require those services.
