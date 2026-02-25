# Security Model: OpenJar Launcher

This document describes the current security posture for authentication storage, Friend Link sync, and release/update integrity.

## Protected Assets
- Microsoft refresh tokens and session identifiers
- Minecraft/Xbox access-token chain
- Friend Link session secrets
- Friend Link peer identity bindings
- Synced instance content and config integrity

## Auth Storage (Microsoft/Minecraft)
- Production builds persist refresh tokens in OS secure storage only (`keyring` backend).
- Production auth flows do not use plaintext token fallback persistence.
- Legacy fallback files are migration-only and are removed after successful migration.
- Refresh-token lookup supports legacy alias/service recovery and rewrites to canonical secure-storage entries.
- Sensitive token values are not logged.
- If secure storage is unavailable, auth fails with actionable secure-storage errors instead of unsafe disk fallback.

## Friend Link Secret Storage
- Friend Link shared secrets are stored in OS secure storage.
- `friend_link/store.v1.json` persists secret handles/metadata, not raw secret bytes.
- Legacy plaintext Friend Link secrets are migrated and scrubbed from store files.
- On Unix, store writes use restrictive permissions (`0600`).

## Friend Link Transport Security
- Length-prefixed frame transport with strict max-size limits.
- Payload confidentiality/integrity via `XChaCha20-Poly1305` (AEAD).
- Frame keys derived from session secret material with HKDF-SHA256.
- Per-frame signatures (Ed25519) are verified before processing payloads.
- Peer key fingerprint trust is enforced; mismatched key identity is rejected.
- Nonce replay protection is enforced with bounded nonce tracking windows.

## Friend Link Endpoint and Exposure Controls
- Loopback endpoints are blocked in internet mode unless explicitly allowed in Dev mode.
- Internet endpoints are opt-in (`allow_internet_endpoints`).
- UPnP is separate opt-in (`allow_upnp_endpoints`) and is not implied by internet mode.
- UPnP mappings are cleaned up on listener stop/restart/leave (best-effort with warning logs on cleanup failure).
- Public-IP discovery is disabled by default and gated by `OPENJAR_FRIENDLINK_DISCOVER_PUBLIC_IP=1`.
- Internet mode join guidance requires reachable host endpoint override or UPnP mapping.

## Invite and Session Controls
- Invite payload V2 adds policy metadata (`invite_version`, `invite_id`, `max_uses`, expiry).
- Invite usage is enforced host-side.
- Default invite policy:
  - Internet mode: one-time, short-lived
  - LAN mode: limited multi-use, longer-lived
- Legacy invite parsing remains supported for backward compatibility.
- Listener session fingerprinting forces listener refresh when host credentials rotate.

## Trust Model
- No global trust-all behavior.
- Bootstrap host may be default-trusted in bootstrap scenarios.
- Other peers require explicit trust.
- Untrusted-peer changes can block or downgrade sync actions based on policy.

## Filesystem Confinement and Symlink Safety
- Friend Link file operations are confined under instance roots.
- Path traversal/absolute/prefix escapes are rejected.
- Symlinked target/parent paths are rejected for Friend Link config/content operations.
- Lock/config path resolution uses constrained, validated path helpers.

## Diagnostics and Redaction
- Friend Link debug bundle export redacts secret-bearing fields.
- Support bundle redaction remains token-aware for logs/config payloads.

## CSP and App Surface
- Tauri CSP is explicitly restrictive (`default-src 'self'` with scoped allowances).
- Network-derived inputs are validated before endpoint/path use.

## Release and Update Integrity
- Release CI verifies platform support declarations and desktop asset path constraints.
- Updater artifact generation is enforced per-platform in build scripts/workflow.

## Known Boundaries
- Friend Link is P2P-first (no relay introduced in this pass).
- Internet mode still increases attack surface versus LAN-only use.
- Users should keep OS keychain/credential vault unlocked and healthy for stable auth/session persistence.

## Test Coverage Highlights
- Auth storage migration, alias recovery, and secure-storage error handling.
- Friend Link frame tamper/replay/oversize protection.
- Invite policy enforcement and backward-compat parsing.
- Endpoint policy behavior (internet/loopback/UPnP toggles).
- Filesystem traversal/symlink confinement checks.
