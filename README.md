# OpenJar Launcher

**OpenJar Launcher** is a **good-looking, Mac-first Minecraft launcher with a spec-driven Modpack Maker** built with **Tauri (Rust)** + **React (Vite + TypeScript)**.

It’s designed to feel clean and modern while still being powerful: manage instances, import from other launchers, browse Modrinth/CurseForge, install & update content with lockfiles, edit configs with a real UI, and launch safely — even running multiple copies at once.

---

## Table of contents

- [Screenshots](#screenshots)
- [Highlights](#highlights)
- [Features](#features)
  - [Instance Management](#instance-management)
  - [Import / Export](#import--export)
  - [Discover + Install (Multi-provider)](#discover--install-multi-provider)
  - [Updates + Update Availability (Multi-provider)](#updates--update-availability-multi-provider)
  - [Installed Mods (Per instance)](#installed-mods-per-instance)
  - [Snapshots + Rollback (installed content)](#snapshots--rollback-installed-content)
  - [World Backups + World Rollback (your saves)](#world-backups--world-rollback-your-saves)
  - [Launching](#launching)
  - [Multi-Launch Explained (Isolated Runtime Sessions)](#multi-launch-explained-isolated-runtime-sessions)
  - [Microsoft Account / Auth (Native Launch)](#microsoft-account--auth-native-launch)
  - [Logs + Crash Hints](#logs--crash-hints)
  - [Config Editor (UI-first, powerful)](#config-editor-ui-first-powerful)
  - [Modpack Maker (Spec / Resolve / Apply)](#modpack-maker-spec--resolve--apply)
- [Where your data lives](#where-your-data-lives)
- [Tech Stack](#tech-stack)
- [Platform support & testing](#platform-support--testing)
- [Dev Setup](#dev-setup)

---

## Screenshots

Screenshots live in `docs/screenshots/` — click any image to view full size.

<p align="center">
  <a href="docs/screenshots/modpack maker.png">
    <img src="docs/screenshots/modpack maker.png" width="49%" alt="Instances" />
  </a>
  <a href="docs/screenshots/instance%20content.png">
    <img src="docs/screenshots/instance%20content.png" width="49%" alt="Instance Content" />
  </a>
</p>

<p align="center">
  <a href="docs/screenshots/discover%20content.png">
    <img src="docs/screenshots/discover%20content.png" width="49%" alt="Discover" />
  </a>
  <a href="docs/screenshots/config%20editor.png">
    <img src="docs/screenshots/config%20editor.png" width="49%" alt="Config Editor" />
  </a>
</p>

<p align="center">
  <a href="docs/screenshots/mod%20updates.png">
    <img src="docs/screenshots/mod%20updates.png" width="49%" alt="Mod Update Availability" />
  </a>
  <a href="docs/screenshots/settings.png">
    <img src="docs/screenshots/settings.png" width="49%" alt="Settings" />
  </a>
</p>

---

## Highlights

- **Clean, modern UI** (macOS-friendly look & feel)
- **Instance management** + import from Vanilla / Prism
- **Multi-provider discovery**: Modrinth + CurseForge
- **Update availability** + **Update all** + **scheduled checks** across providers/content types
- **Dependency-aware installs** + per-instance **lockfile** tracking
- **Per-mod enable/disable** (rename to `.disabled`)
- **Config Editor** experience (file browser + editors + helpers)
- **Snapshots / rollback** tooling (for installed content)
- **Native launching** + Microsoft account login
- **Multi-launch support** using **isolated runtime sessions** (run many copies safely)

---

## Features

### Instance Management

Create and manage self-contained “instances” (your own Minecraft folders with their own mods, packs, saves, and settings).

What you can do:
- Create, list, rename, edit, delete instances
- Open/reveal instance folders and common paths
- Instance icons (store an icon path + load local images for display)

Per-instance launch settings (these affect the actual launch):
- Java executable path (or auto-detect a runtime)
- Memory limit (adds `-Xmx####M`)
- Extra JVM args
- “Keep launcher open” / “Close on game exit”

Note on settings:
- Some extra toggles exist in the UI/settings model (graphics preset, shader toggle, vsync, prefer releases, etc.)
- If something doesn’t change the game yet, it means it isn’t fully hooked up in the current build.

---

### Import / Export

Move your existing setup into OpenJar and back out again.

Create instance from a modpack archive (“From File” flow):
- Supports **Modrinth `.mrpack`** and **CurseForge** modpack zips
- Reads pack name / Minecraft version / loader from pack metadata
- Imports **override files** (configs/resources/scripts/etc.) into the instance

Important:
- It does **not** automatically download the modpack’s mods yet — it currently extracts overrides only.

Import instances from other launchers:
- **Vanilla Minecraft** (`.minecraft`)
- **Prism Launcher** instances (auto-detected)
- Copies common folders like:
  - `mods/`, `config/`, `resourcepacks/`, `shaderpacks/`, `saves/`
  - plus `options.txt` and `servers.dat`

Other import/export tools:
- Import a local mod **`.jar`** into an instance (“Add from file”)
- Export installed mods as a **ZIP**
  - Includes enabled `.jar` files and disabled `.disabled` files

---

### Discover + Install (Multi-provider)

Find content and install it straight into an instance.

Discover/search supports:
- **Modrinth**
- **CurseForge**

Filters include:
- Content type: mods / resourcepacks / shaderpacks / datapacks / modpacks
- Loader: Fabric / Forge / Quilt / NeoForge (and Vanilla where relevant)
- Minecraft version
- Sort: downloads / updated / newest / follows (depends on provider)

#### Modrinth (works now)

- Install Modrinth projects into an instance with progress events
- Install planning/preview (“here’s what will be installed before we do it”)
- Automatically installs **required dependencies**
- Writes installs to a per-instance lockfile (`lock.json`) so OpenJar can:
  - check for updates later
  - roll back installed content reliably

#### CurseForge

- Installs are supported through the same lockfile/update model as Modrinth.
- In local dev, key diagnostics are available in the hidden Dev section (`MPM_DEV_MODE=1`).
- Release builds are expected to use build-injected key configuration.

---

### Updates + Update Availability (Multi-provider)

This feature keeps installed content up to date across **Modrinth + CurseForge** and across supported content types tracked in `lock.json` (mods/resourcepacks/shaderpacks/datapacks).

What “Refresh / Check” actually does:
- Looks at tracked content entries currently installed in that instance
- For each entry, checks for a newer compatible provider version/file
- Shows you a clear per-mod result like:
  - `Sodium 0.5.11 → 0.5.13`
  - `Fabric API 0.97.0 → 0.98.1`
- Does not change anything until you choose to apply updates

Where you see updates:

Per instance (Maintenance card):
- **Refresh** checks tracked content in the selected instance
- You get a quick list of updates available (current → latest)
- **Update all** applies updates for tracked entries only

Global Updates page (Update availability dashboard):
- Shows which **instances** have mod updates available, and **how many**
- Shows **last checked** and **next scheduled check**
- Lets you jump into an instance (“Open instance”) or run a new check (“Recheck”)
- Adds a sidebar badge when any instance has mod updates waiting

Scheduled checks (so you don’t have to remember):
- Set a **Check cadence**:
  - Disabled, Every hour, Every 3 hours, Every 6 hours, Every 12 hours, Daily, Weekly
- The Updates page shows:
  - **Last run** (last scheduled/manual check)
  - **Next run** (next scheduled check)
- **Check now** triggers a full mod update check immediately

Update-all safety (so updates don’t feel risky):
- When you hit **Update all**, OpenJar creates a **snapshot first**
- If an updated mod breaks your game, you can roll back your *installed content* with one click
- Snapshots cover installed content (mods/packs/datapacks + lockfile), not full world saves (world saves are handled by World Backups)

Optional auto-apply (choose “notify me” vs “do it for me”):
- Choose what happens when a scheduled check finds mod updates:
  - **Check only (notify):** OpenJar will show the badge + update list, but you must click **Update all** to actually update the mods.
  - **Auto-apply updates:** OpenJar will check **and** automatically update the mods for you.
- Choose where auto-apply is allowed:
  - **Only opt-in instances** (instances you’ve explicitly marked as OK to auto-update)
  - **All instances**
- Choose when auto-apply can run:
  - **Scheduled runs only** (recommended)
  - **Scheduled + “Check now”** (manual checks can also auto-update)

In short: OpenJar tells you exactly which **mods** are outdated, lets you **update all** in one click, can **check on a schedule**, and gives you a snapshot so you can undo if something goes wrong.

---

### Installed Mods (Per instance)

Keep track of what’s installed, and quickly disable something that’s causing crashes.

- View installed content list (from `lock.json`)
- Enable/disable mods:
  - Disabling renames `SomeMod.jar` → `SomeMod.jar.disabled`
  - Enabling renames it back to `.jar`
  - (Currently enable/disable is supported for **mods** only.)
- Lockfile tracking (`lock.json` stored inside the instance folder)
  - Stores provider IDs + chosen version + filename + hashes + enabled/disabled state

---

### Snapshots + Rollback (installed content)

Snapshots are your “undo” button for **installed content** — not your entire world.

What a snapshot is:
- A stored copy of specific *content folders* + the lockfile at that moment,
  so you can revert after a bad install/update.

What gets snapshotted:
- `mods/`
- `resourcepacks/`
- `shaderpacks/`
- each world’s `saves/<world>/datapacks/`
- the instance `lock.json`

What does *not* get snapshotted:
- Your world data (region/playerdata/etc.) — that’s handled by **World Backups**
- Other world files outside `datapacks/`
- General config folders (for now)

When snapshots are created:
- Before installing content (when there are real actions to apply)
- Before applying presets (if enabled)
- Before “Update all” (when updates exist)

How rollback works:
- Snapshots are kept (up to 20) and listed in the UI
- Rolling back restores the snapshot’s content folders + the saved `lock.json`
- You must stop Minecraft before rolling back

---

### World Backups + World Rollback (your saves)

This is the “I don’t want to lose my world” safety net.

What it does:
- OpenJar can periodically back up each world in `saves/`
- Each backup is a **zip of the entire world folder**
  (region, playerdata, data, advancements, etc.)
- Backups are stored under `world_backups/` inside the instance folder

How you control it (per instance):
- Backup interval (minutes)
  - Example: every 10 minutes OpenJar zips your world and stores a backup
- Retention count (per world)
  - Example: keep the last 3 backups of each world, delete older ones automatically

World rollback (restore a backup):
- Choose a backup (most recent or a specific one)
- Restoring **replaces** `saves/<world>` with the backed-up copy
- You must stop Minecraft before restoring a world

Important nuance (multi-launch):
- When OpenJar launches additional copies using an **isolated runtime session**,
  it copies worlds/configs into a temporary folder so those extra sessions can’t corrupt
  your main world.
- In isolated mode, auto world backups are not run for that session.

---

### Launching

Two launch modes depending on how you prefer to run Minecraft.

Native launch mode (no Prism required):
- Loader support includes Vanilla / Fabric / Forge (auto resolution logic)
- Uses shared caches under app data (assets/libraries/versions caching)

Prism launch mode:
- Syncs instance content into a Prism instance folder
- Uses symlinks when possible, with copy fallback
- Launches through Prism’s workflow

Basic safety controls:
- Tracks running launches (per-launch IDs)
- Stop a running instance
- Cancel an in-progress launch
- Prevents unsafe duplicate native launch of the *same* instance folder

---

### Multi-Launch Explained (Isolated Runtime Sessions)

When you launch a second (or third…) copy of the same instance, OpenJar creates a **runtime session** folder.

How it behaves:
- First launch: the game uses the normal instance runtime folder
- Additional launches: OpenJar makes `runtime_sessions/<launch_id>/` and:
  - links mods/resourcepacks/shaderpacks (fast, shared)
  - copies config + saves into the session (so changes don’t touch your main instance)
  - copies `options.txt` + `servers.dat`
- When the game closes, that runtime session folder is deleted automatically

Why it exists:
- It avoids two Minecraft clients writing to the same world/config and corrupting things.

---

### Microsoft Account / Auth (Native Launch)

Sign in and stay signed in.

- Microsoft device-code login flow (begin + poll)
- List saved accounts
- Select active account
- Logout/disconnect
- Account diagnostics (helps when auth gets weird)
- Tokens are stored in the system keychain (with a safe fallback file inside app data)

---

### Logs + Crash Hints

OpenJar can read the latest instance logs and give you faster signals.

- Read instance logs
- Frontend log analyzer:
  - counts errors/warnings
  - tries to identify likely causes (“suspects”) based on common patterns
    (mod mentioned in a stack trace, missing dependency, incompatible loader/version, etc.)

---

### Config Editor (UI-first, powerful)

A full config editing experience inside the app.

Core workflow:
- Instance picker dropdown
- Optional world picker (lists worlds in `saves/`)
- Config file browser (lists config files)
- Reveal/open files in your file manager (Finder/Explorer/etc.)

Editing tools:
- Read and save files
- Create new config files (New File modal)
- Specialized editors:
  - JSON editor (parsing + friendly error display)
  - Text editor
  - `servers.dat` editor (edit server list)
- Advanced editor mode
- Inspector panel (context + suggestions)
- Helper features (formatting + suggestions)

---

### Modpack Maker (Spec / Resolve / Apply)

Creator Studio -> **Creator** is OpenJar’s built-in modpack builder.  
It helps you build a real modpack (not just a random list), preview what will happen for a specific instance, and apply safely with rollback support.

How to use it:
- Create or open a modpack in **Creator Studio -> Creator**
- Add content in the editor (quick add) or click **Open in Discover** for full browsing/filtering
- Choose the target instance and run **Preview + apply** to see exactly what will install
- Review results first: compatible installs, failures, conflicts, and confidence level
- Apply in **Linked** mode (track + re-align later) or **One-time** mode, with snapshot + rollback safety

What makes it unique:
- **Layered packs** so your pack stays organized:
  - `Template` = base pack
  - `User Additions` = your main add/remove list
  - `Overrides` = explicit conflict fixes or final wins
- **Open in Discover workflow** that adds search results straight into the selected modpack layer
- **Unified entries list + inspector** so you can quickly review and edit per-entry settings
- **Explain-first preview** with clear reasons when something fails (no silent changes)
- **Profiles** (like Lite / Recommended / Full) to toggle optional content cleanly
- **Linked mode + drift detection** to keep instances aligned over time
- **Reversible applies** with lock snapshots and one-click rollback

---

## Where your data lives

OpenJar keeps your data **on your computer** in your OS app data directory (Tauri app data). Each instance is just a normal folder structure you can open in your file manager.

Per instance you’ll typically see:

- `mods/`, `resourcepacks/`, `shaderpacks/`, `config/`, `saves/`
- `lock.json` (tracks what OpenJar installed so updates/rollback are reliable)
- `snapshots/` (snapshots of installed content for rollback)
- `world_backups/` (zipped backups of your worlds, based on your backup settings)
- `runtime/` and `runtime_sessions/` (temporary launch/runtime folders, especially for multi-launch)

Privacy note: OpenJar doesn’t upload your instances, worlds, or configs anywhere — it works directly with the files on your machine. Anything network-related is only for things you explicitly do (like browsing/installing from Modrinth/CurseForge or signing in to Microsoft for launching).

---

## Tech Stack

- **Tauri v1** + **Rust** backend
- **React + TypeScript** frontend (**Vite**)
- Multi-provider content flows (**Modrinth + CurseForge**)
- Clean separation between UI, commands, and instance filesystem operations

---

## Platform support & testing

OpenJar Launcher is **macOS-first** (highest-priority platform), with cross-platform builds for Linux and Windows.

Current build targets:
- macOS Intel (`x86_64-apple-darwin`)
- macOS Apple Silicon (`aarch64-apple-darwin`)
- Linux x64 (`x86_64-unknown-linux-gnu`)
- Windows x64 (`x86_64-pc-windows-msvc`, intended for Windows 11)

CI now runs a Tauri build matrix for all of the targets above and uploads artifacts per platform.

Known limitations:
- Windows CI runs on GitHub-hosted Windows Server images, not a full Windows 11 desktop session.
- Linux desktop runtime depends on WebKitGTK/libsoup2 packages available on the target distro.
- macOS receives the most day-to-day manual testing.

If you try Windows/Linux and run into issues, please open a GitHub Issue with:
- OS + version (and whether it’s Intel/AMD or ARM)
- steps to reproduce
- error messages
- relevant logs (and screenshots if helpful)

---

## Dev Setup

### Requirements

- Node.js **18+** recommended
- Rust toolchain (**stable**)
- Tauri prerequisites for your OS

### Install

```bash
npm install
````

### Run (dev)

Frontend only (Vite):

```bash
npm run dev
```

Full desktop app (Tauri + Vite):

```bash
npm run tauri:dev
```

### Build

Frontend build:

```bash
npm run build
```

Tauri desktop build:

```bash
npm run tauri:build
```

### Preview (frontend build)

```bash
npm run preview
```
