# Release Checklist

Use this before shipping a new OpenJar build.

## 1. Static checks

From the repo root:

```bash
npm run build
npm run verify:tauri-command-contract
npm run verify:platform-support
npm run verify:desktop-asset-paths
```

From `src-tauri/`:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected outcome:
- no TypeScript errors
- no failing Rust tests
- no clippy warnings
- no command-contract/platform/asset verification failures

## 2. Smoke tests

Run these manually in a local desktop build:

- Native launch: start one instance, confirm launch state, logs, and running-session UI update.
- Native stop: stop a running native instance and confirm state/history clear correctly.
- Prism launch: sync into Prism, start successfully, confirm settings hydration and success notice.
- Settings sync: with `options.txt` or `servers.dat` changed in one instance, launch and verify the configured sync target receives the latest file.
- World backup: launch an instance with auto backup enabled long enough to trigger a backup, then verify backup creation and restore UI.
- Update check: run tracked-content refresh on an instance with known provider-backed content and verify result list + snapshot-before-update flow.
- Local import: import a launcher instance or local mod file and verify lockfile/provider resolution still behaves normally.
- GitHub attach/update: attach a GitHub repo to a local mod and verify update checks still work without ambiguous-provider regressions.
- Account/auth: sign in, relaunch, and confirm the saved account still works without re-login.

## 3. Runtime and safety checks

Pay extra attention to these release blockers:

- Same-instance concurrent native launch must be blocked with a clear message.
- Runtime/session folders must not be treated as a source of truth over the canonical instance folder.
- Symlinked content under launcher-import or runtime-clone paths must be rejected rather than copied through.
- Snapshots should affect installed content only; world rollback should stay a separate flow.
- Stop/rollback actions must be blocked while the relevant instance is running.

## 4. Known limitations to keep honest in docs

Document these clearly in release notes and README if they still apply:

- Same-instance concurrent native launch is intentionally blocked until full reconciliation exists for world/config changes.
- Linux packaging still depends on target-machine WebKitGTK/libsoup availability.
- Windows CI validates builds, but it is not a full interactive Windows 11 desktop test.
- Production accounts/tokens should rely on secure storage, not plaintext fallbacks.

## 5. Dogfooding plan

Before calling the app “done,” use it like a real launcher for a few days:

- Daily play on one primary instance.
- Install/update/remove content through the app only.
- Edit configs from the built-in editor.
- Trigger at least one rollback and one restore path.
- Try one sign-out/sign-in recovery.
- Keep notes on any confusing wording, stale status, or “button looked like it worked but didn’t” moments.

These usually reveal the last bugs worth fixing before release.
