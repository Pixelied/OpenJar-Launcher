# Security Policy

OpenJar Launcher handles local Minecraft instances, account sign-in, provider APIs, and peer-to-peer sync. This document explains how to report a vulnerability and summarizes the current security model in the codebase.

## Reporting a Vulnerability

Please do not open a public GitHub issue with exploit details.

Preferred disclosure path:

1. Use GitHub Security Advisories or another private GitHub disclosure channel if one is available for the repository.
2. If no private security channel is available, contact a maintainer privately through GitHub and share details there.
3. If you cannot reach a maintainer privately, open a minimal public issue that only asks for a private contact path. Do not include exploit steps, secrets, payloads, or proof-of-concept details.

Include as much of the following as you can:

- affected version, branch, or commit
- target platform and OS version
- impact and attack preconditions
- reproduction steps
- logs, screenshots, or traces with secrets removed
- whether the issue depends on local access, network adjacency, or malicious provider/peer input

## Supported Fix Targets

This project moves quickly. Security fixes are expected to land on:

- the current `main` branch
- the most recent release, when a backport is practical

Older builds may not receive security fixes.

## Security Model

### Protected assets

OpenJar treats the following as sensitive:

- Microsoft refresh tokens and related launcher auth state
- GitHub personal access tokens used for higher API limits
- Friend Link shared secrets and signing keys
- local instance content, config files, snapshots, and world backups
- peer identity bindings and last-good sync metadata

### Secret storage

- Production auth flows store refresh tokens in OS secure storage through `keyring`.
- Production builds do not intentionally fall back to plaintext token storage.
- Dev builds may keep a debug-only recovery fallback to avoid repeated local sign-ins during development.
- GitHub token pools can be loaded from secure storage and/or environment variables; OpenJar does not ship embedded GitHub tokens.
- Friend Link secrets and signing keys are stored through OS secure storage, while on-disk store files persist only metadata and handles.

### Network trust boundaries

- Provider data from Modrinth, CurseForge, and GitHub is treated as untrusted input until validated and normalized.
- GitHub support is intentionally conservative and uses installability checks, release-asset matching, and repo-hint verification to reduce unsafe provider activation.
- Friend Link is a direct peer model. Internet mode, UPnP, and public endpoint exposure are opt-in.
- Loopback and public endpoint behavior is gated by policy, not silently enabled.

### Filesystem safety

- OpenJar operates on user-owned instance folders, but higher-risk flows are scoped and validated before writes.
- Friend Link file operations are confined under instance roots.
- Path traversal, absolute-path escapes, and unsafe symlink targets are rejected for Friend Link content/config flows.
- Config editing is scoped to allowlisted locations rather than arbitrary disk writes.

### Tauri command surface

- UI-to-Rust calls go through explicit Tauri commands rather than open-ended script bridges.
- The repo includes `scripts/verify-tauri-command-contract.mjs` to catch frontend/backend command drift and shared payload shape drift.
- Commands that mutate instance state now use tighter sequencing and per-instance coordination to reduce stale writes and rollback hazards.

### Launch, install, and rollback safety

- Risky content changes create snapshots before mutation so rollback stays available.
- Same-instance concurrent native launch is intentionally blocked because silent reconciliation was not safe enough.
- World backups and installed-content snapshots are separate mechanisms so recovery stays understandable.

### Friend Link transport

- Transport uses encrypted frames with authenticated payloads.
- Peer key identity and nonce replay protections are enforced.
- Trust state and guardrails are explicit and can block auto-sync behavior when risk is too high.

## Known Boundaries

- OpenJar is local-first, but any provider lookup, sign-in, update check, or peer sync increases attack surface compared with fully offline use.
- Friend Link is peer-to-peer and does not provide a hosted relay trust boundary.
- GitHub, CurseForge, and Modrinth metadata quality varies; OpenJar reduces risk with validation, but provider-side ambiguity still exists.
- Local machine compromise, malicious browser extensions, or compromised OS credential storage are outside what the app alone can fully defend against.

## Security-Relevant Verification in This Repo

Examples of security-relevant checks already in the repository:

- Rust tests under `src-tauri/src/friend_link/tests.rs`
- Rust tests under `src-tauri/src/modpack/tests.rs`
- Rust regression tests under `src-tauri/src/tests/` for token storage, runtime/playtime, storage usage, update-check resilience, provider matching, and related safety-sensitive paths
- contract verification in `scripts/verify-tauri-command-contract.mjs`
- platform and packaging checks in `scripts/verify-platform-support.mjs` and `scripts/verify-desktop-asset-paths.mjs`

If you report a vulnerability, pointing to the nearest affected command, file path, or flow will make triage much faster.
