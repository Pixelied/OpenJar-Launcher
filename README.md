# OpenJar Launcher

OpenJar Launcher is a macOS-first Minecraft launcher and modding toolkit built with Tauri, Rust, React, and TypeScript.

It focuses on the parts of modded Minecraft that usually go wrong: keeping instances organized, installing content safely, tracking what changed, recovering from bad updates, and making multiplayer pack parity less painful.

Security notes live in [SECURITY.md](SECURITY.md). License terms live in [License.md](License.md).

## Screenshots

Screenshots live in `readme-assets/images/`.

<p align="center">
  <a href="readme-assets/images/discover content.png">
    <img src="readme-assets/images/discover content.png" width="49%" alt="Discover content" />
  </a>
  <a href="readme-assets/images/instance content.png">
    <img src="readme-assets/images/instance content.png" width="49%" alt="Instance content" />
  </a>
</p>

<p align="center">
  <a href="readme-assets/images/modpack maker.png">
    <img src="readme-assets/images/modpack maker.png" width="49%" alt="Modpack Maker" />
  </a>
  <a href="readme-assets/images/config editor.png">
    <img src="readme-assets/images/config editor.png" width="49%" alt="Config editor" />
  </a>
</p>
<p align="center">
  <a href="readme-assets/images/instance library.png">
    <img src="readme-assets/images/instance library.png" width="49%" alt="Instance Library" />
  </a>
  <a href="readme-assets/images/updates available.png">
    <img src="readme-assets/images/updates available.png" width="49%" alt="Updates Available" />
  </a>
</p>

## Highlights

- Multi-provider discover, install, and update flows for Modrinth, CurseForge, and GitHub
- Per-instance lockfile tracking so installs, updates, rollback, and provider switching stay explainable
- Snapshot and rollback tooling for installed content, plus world backup / restore flows
- Native Minecraft launching with Microsoft device-code sign-in
- Config Editor for instance and world files with scoped writes and backup history
- Modpack creation and apply flows with preview, drift detection, and safer reconciliation
- Friend Link for keeping shared instances aligned across a small group
- Stronger local safety defaults around filesystem writes, launch state, and provider activation

## What OpenJar Does

### Instance management

OpenJar manages self-contained Minecraft instances with their own mods, packs, saves, configs, launch settings, and metadata. Instances stay visible on disk as normal folders instead of opaque app-only blobs.

### Discover, install, and update

OpenJar can browse provider content, install it into an instance, write the result into `lock.json`, and later use that lockfile for update checks, update-all flows, rollback, and local source identification.

Provider notes:

- Modrinth has the most complete support path.
- CurseForge works through the same lockfile model, but some files still block third-party direct downloads. When that happens, OpenJar surfaces the limitation and points users toward local import.
- GitHub support is intentionally conservative. Installability depends on release assets and compatibility hints, filter quality is best-effort, and local mods can store a GitHub repo hint so later update checks have something trustworthy to build from.

### Config editing

The Config Editor works directly against instance and world files, with scoped writes, backups before save, and helper views for common formats. It is designed to reduce “open the file manager, find the right config, hope you edit the right thing” friction.

### Modpack creation

OpenJar includes a built-in modpack workflow for assembling layered packs, previewing changes before apply, and re-aligning instances later. It is meant to be safer than maintaining a loose spreadsheet or folder of manual mod notes.

### Friend Link

Friend Link helps small groups keep content and selected config files aligned before launch. It is a direct peer model, not a hosted relay service, and it prioritizes explicit trust and review over silent background mutation.

### Recovery and rollback

OpenJar takes pre-change snapshots around risky content operations and supports rollback of installed content. World backups are separate and handle full save restoration.

## Local-First and Safety Model

OpenJar is local-first:

- instances, worlds, configs, and lockfiles stay on the user's machine
- network activity is limited to explicit provider, authentication, update, and sync flows
- production auth and GitHub token storage use OS secure storage
- install and update flows snapshot content before risky changes
- same-instance concurrent native launch is intentionally blocked to avoid silent data loss

For the detailed security posture, disclosure process, and current security boundaries, see [SECURITY.md](SECURITY.md).

## Where Data Lives

OpenJar stores app data in the OS app-data location used by Tauri. Instance folders typically contain:

- `mods/`, `resourcepacks/`, `shaderpacks/`, `config/`, `saves/`
- `lock.json`
- `snapshots/`
- `world_backups/`

Some users may also have legacy runtime/session directories from older builds. Current native launches use the canonical instance folder directly.

## Platform Support

OpenJar is macOS-first, but the repo maintains cross-platform build targets.

Current targets:

- macOS Intel: `x86_64-apple-darwin`
- macOS Apple Silicon: `aarch64-apple-darwin`
- Linux x64: `x86_64-unknown-linux-gnu`
- Windows x64: `x86_64-pc-windows-msvc`
- Windows ARM64: `aarch64-pc-windows-msvc`

Notes:

- macOS gets the most day-to-day manual testing.
- Linux desktop runtime depends on distro WebKitGTK/libsoup compatibility.
- Windows CI runs on GitHub-hosted Windows images, not a full consumer desktop environment.

## Repository Docs

- [SECURITY.md](SECURITY.md): security reporting process and current security model
- [License.md](License.md): source-available project license
- [docs/release-checklist.md](docs/release-checklist.md): release and smoke-test checklist

## Development

### Requirements

- Node.js 20+
- Rust stable
- Tauri prerequisites for your OS

### Install

```bash
npm install
```

### Run

Frontend only:

```bash
npm run dev
```

Desktop app:

```bash
npm run tauri:dev
```

### Build

Frontend bundle:

```bash
npm run build
```

Desktop bundle:

```bash
npm run tauri:build
```

Artifact notes:

- Local macOS release builds produced by `npm run tauri:build` create the `.app` bundle and then package it into a `.dmg`.
- CI and tag-based release workflows build the same target matrix, but ship zipped `.app` bundles on macOS plus updater artifacts instead of publishing a `.dmg`.

### Recommended verification

Run these before opening a PR that changes commands, platform targets, or risky behavior:

```bash
npm run verify:platform-support
npm run verify:tauri-command-contract
npm run verify:desktop-asset-paths
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

There is currently no dedicated `npm test` script for frontend unit tests, so repo validation is centered on build checks, Rust tests, contract verification, and release smoke tests.

### macOS bundle verification

After building a macOS app bundle, you can run:

```bash
./scripts/verify-macos-bundle.sh
```

Or target a specific bundle:

```bash
./scripts/verify-macos-bundle.sh "/path/to/OpenJar Launcher.app"
```

### Release workflow

Use [docs/release-checklist.md](docs/release-checklist.md) before shipping a release. It covers launch smoke tests, provider flows, updater assets, and packaging sanity checks.

## Contributing

Issues and pull requests are welcome. For security-sensitive issues, do not open a public issue with exploit details; use the reporting guidance in [SECURITY.md](SECURITY.md).
