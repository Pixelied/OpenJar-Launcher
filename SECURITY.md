# Security Model: OpenJar Launcher

This document describes the security hardening for Microsoft/Minecraft auth storage and FriendLink sync.

## Protected Assets
- Microsoft refresh tokens
- Minecraft/Xbox access tokens
- FriendLink shared session secrets
- Instance lock/config sync integrity and confidentiality

## Auth Storage Hardening
- Production builds persist refresh tokens in OS secure storage only (`keyring` backend on macOS/Windows/Linux).
- Plaintext fallback token persistence is removed from production runtime auth flows.
- Debug builds (`cfg(debug_assertions)`, for example `npm run tauri:dev`) keep a development recovery fallback at `launcher/tokens_debug_fallback.json` to avoid repeated account disconnects when local keychain access is unstable during development.
  - This file is never used in production builds.
  - On Unix it is written with restrictive permissions (`0600`).
- Legacy `launcher/tokens_fallback.json` is treated as migration-only:
  - read once at startup
  - migrate tokens into keyring (best effort)
  - delete fallback file
- Startup migration checks known legacy launcher data roots (including historical app identifiers) so old fallback files are migrated even after bundle-id changes.
- Refresh-token lookup also supports secure-storage alias recovery (legacy service names and selected-account alias) and rewrites recovered tokens into canonical keyring entries.
- No token values are logged.
- If keyring storage is unavailable, auth operations fail with an actionable secure-storage error instead of falling back to disk.

## FriendLink Secret Storage
- FriendLink shared secrets are stored in OS secure storage.
- `friend_link/store.v1.json` stores a key identifier only; raw shared secret material is not serialized.
- Legacy on-disk shared secrets are migrated to keyring on read.
- After migration, `store.v1.json` is rewritten immediately so legacy plaintext secret fields are removed from disk.
- On Unix, FriendLink store files are written with restrictive permissions (`0600`).
- If a stale session is missing its secure secret entry, host creation rotates session credentials instead of failing with an unrecoverable keyring error.

## FriendLink Filesystem Safety
- Added strict safe path handling for FriendLink disk operations:
  - `safe_join_under(root, rel)` with component validation
  - strict filename/world-name sanitization for lock entry binary paths
  - traversal/absolute/prefix rejection
- `lock_entry_paths` now validates and constrains all generated paths under the target instance directory.
- Config deletion from network-derived keys now resolves through safe instance-file path logic.

## FriendLink Transport Security
- Replaced unbounded raw reads with length-prefixed framing (`u32` big-endian).
- Enforced maximum encrypted frame size and plaintext size limits to prevent memory/DoS abuse.
- FriendLink frames are encrypted+authenticated with AEAD (`XChaCha20-Poly1305`).
- Per-session frame keys are derived from shared secret material with HKDF-SHA256.
- Tampered ciphertext fails auth/decrypt and is rejected.

## Endpoint and Trust Defaults
- Invite/bootstrap endpoints are validated.
- Loopback endpoints are blocked by default unless explicitly opted in.
- Loopback opt-in is restricted to Dev mode (`MPM_DEV_MODE=1`) and enforced in backend command handlers.
- Public internet endpoints are blocked by default unless explicitly opted in.
- Trust defaults are hardened:
  - no automatic trust-all behavior
  - bootstrap host can be default trusted in bootstrap scenarios; all other peers require explicit trust action.

## Session Rotation Safety
- FriendLink listeners are now bound to a session fingerprint (group id + local peer id + secret material).
- If host credentials rotate, the listener is restarted automatically so stale in-memory secrets do not cause persistent decrypt/auth failures.

## Runtime Behavior Notes
- FriendLink auto-sync execution is handled by app-level background scheduling and does not require the Friend Link modal to be open.
- `manual`, `ask`, `auto_metadata`, and `auto_all` mode behavior remains policy-driven; only the execution trigger location changed (from modal-only to app-level scheduler).

## Debug/Export Redaction
- FriendLink debug bundle export redacts secret-bearing fields (shared secret/key handles/bootstrap credentials).
- Support bundle redaction path remains token-aware for logs/config content.

## CSP Hardening
- Tauri CSP is now explicitly set and restrictive (`default-src 'self'`, no object/frame sources, scoped connect/img/style/font/script policies).

## Tests Added
- Auth storage tests:
  - keyring-only persist path (no plaintext fallback file creation)
  - keyring read path
  - legacy fallback migration + file deletion
  - actionable failure when secure storage is unavailable
- FriendLink path tests:
  - filename/world-name traversal rejection
  - `safe_join_under` traversal rejection
  - `lock_entry_paths` confinement under instance directory
- FriendLink protocol tests:
  - oversized frame rejection
  - tamper/decrypt-auth failure
  - relay-style deterministic encrypted message exchange harness
