# Registry distribution incident and recovery guide

**Reviewed:** 20 October 2026<br>
**Production status:** no production TUF root or signed release exists. Do not use this staging repository or its test root as production trust.

This guide covers the TUF registry distribution path. TUF authenticates publisher authorization, metadata consistency, freshness, and target bytes under a client-pinned root. It does not establish that a signed indicator is true, current, complete, or safe to block. Registry research review and signing authorization remain separate responsibilities.

## Immediate response

1. Stop the affected publication workflow or mirror if doing so is needed to contain unauthorized writes. Do not disable client signature, expiry, version, root-rotation, or registry semantic checks.
2. Preserve the public repository ref, metadata bytes, target bytes, workflow run IDs, and their hashes. Avoid copying participant/session data into an incident record; it is not needed to investigate registry publication.
3. Determine whether the incident is availability-only, a metadata/signing-key incident, a mistaken but authorized target, or a loss/compromise of the root threshold.
4. Follow the role-specific response below. Verify any recovery using the pinned Rust client and compare target bytes with a fresh deterministic build before restoring publication.
5. Publish an incident notice through a channel independent of the suspected trust failure when the production identity or signing threshold may be affected.

Never recover by deleting a client's metadata floor and silently accepting a new root from the same endpoint. A client without a trustworthy root or version floor must be reinitialized through an authenticated application release or another independently authenticated channel.

## Role and infrastructure response

| Incident | Containment and recovery |
| --- | --- |
| Pages, mirror, or repository content is changed without signing authority | Treat this as delivery tampering or denial of service. TUF verification should reject altered metadata/targets; restore the reviewed public repository state and verify from the pinned root. Hosting compromise alone does not authorize new TUF metadata. |
| Timestamp role key is compromised | Revoke/rotate the timestamp role through metadata authorized by the root policy. Publish a fresh timestamp and check expiry/version behavior. A compromised timestamp can disrupt freshness and availability; it does not authorize changed target bytes. |
| Snapshot role key is compromised | Rotate the snapshot role through root-authorized metadata and inspect the metadata chain. Keep snapshot authority separate from root and targets. Reissue valid snapshot/timestamp metadata after correcting the workflow. |
| Targets role key is compromised | Stop target publication. Use the authorized root threshold to replace the targets key and publish reviewed metadata. Inspect all targets signed during the exposure window; publish corrected targets where needed. A targets signature authorizes content, so verify the root policy and release history carefully. |
| One root key is lost or suspected compromised, but the threshold remains available | Use the remaining authorized root quorum to rotate to new keys, following the version-by-version old-root and new-root thresholds. Publish each required root version and verify with a fresh client before resuming. |
| Root threshold is compromised or irrecoverably lost | The existing TUF chain cannot safely recover itself. Stop accepting the old repository identity. Produce a new root only through a separately authenticated application release/channel, announce retirement of the old identity, and require clients to adopt that trust anchor deliberately. If the independent channel is unavailable, pause trusted updates. |
| Signing workflow or GitHub account is compromised | Disable the affected workflow and credentials, preserve workflow/repository history, inspect permissions and signed metadata. If the actor could not access TUF private keys or signing quorum, treat it as delivery compromise; if it could, follow the affected role response. Restore from reviewed source and re-enable only after permission review. |
| Malicious registry change is correctly signed | TUF cannot detect factual or classification error. Stop the next release if possible, review source evidence and affected indicators, increment the registry semantic version, regenerate deterministic bytes, obtain required independent review/signatures, and publish a corrected target. Document the correction and potential false-positive impact. |
| Metadata is expired or the service is frozen | Clients must report expiry/unavailability and fail closed for current trust. Restore the authorized signer and publish fresh role metadata. Do not extend expiry locally or call stale metadata current. A previously installed registry is usable only while its metadata remains valid and its signatures reverify. |
| Client cache/pointer is corrupted or inconsistent | Preserve the state for diagnosis. The client must fail closed rather than silently resetting its rollback floor. Restore a verified backup only if its root binding, metadata chain, and expiry all verify. If no trustworthy state remains, re-bootstrap only from an independently authenticated root and disclose that local rollback state was lost. |

## Key roles and custody boundaries

Production policy is proposed as root 2-of-3 and targets 2-of-3, with separate online snapshot and timestamp identities. No production keys or signer roster currently exist. Do not lower thresholds to recover availability. The planned incident contacts and private-key recovery procedures must be confirmed by the actual custodians before production publication.

The staging repository is intentionally different: it has a one-signature standard-key root and static metadata created for compatibility testing. Its Pages deploy workflow has no signing keys and does not renew metadata. Current staging timestamp metadata expires 18 November 2026; targets and snapshot expire 17 January 2027; root expires 19 October 2027. The staging signing material is disposable and not a durable custody arrangement. Reinitializing staging requires updating the test root and digest intentionally and rerunning Rust verification. Never compile that root into production or reuse its keys for production.

## Client-side trust behavior

- Registry fetches are explicit CLI operations. Session creation, relay communication, and competition mode do not contact registry hosting.
- TUF requires the caller-provided root bytes and independent SHA-256 pin; a remote endpoint cannot bootstrap its own root.
- HTTPS redirects are not followed; origin, timeout, target name, response sizes, signatures, metadata versions, expiry, target hash, and registry semantics are checked before installation.
- Updates use a verified candidate snapshot and an atomic active pointer. An interrupted or invalid update must leave the prior selected snapshot intact.
- Installed snapshots are local files. They protect against ordinary accidental corruption, not a same-user attacker, administrator/root, full disk rollback, or a manipulated local clock.
- A valid registry signature proves authorized publication, not correct research or safe enforcement. The registry remains disconnected from enforcement.

## Pre-publication recovery readiness

Before production publication, maintainers must establish named root/targets custodians, key backups and recovery access, protected repository/workflow permissions, online role separation, expiration monitoring, an independent root-bootstrap channel, and a tested rotation/restore procedure. A disposable signer repository must exercise root rotation, target correction, and online-role rotation with the exact selected publisher and Equalturn Rust verifier. The current static staging endpoint cannot perform these drills.

For the production bootstrap and signing procedure, see [production root ceremony](production-root-ceremony.md). For current release profiles and client behavior, see [trusted registry distribution](registry-trust.md) and [production trust policy](registry-production-trust.md).
