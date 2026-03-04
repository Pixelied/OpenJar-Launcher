# Bugfix Polish Report (2026-03-03)

## Scope
- GitHub local mod identification
- GitHub discover pagination and result depth
- Discover/instance icon stability and scroll jitter mitigation
- Status banner severity polish
- Documentation alignment

## User-Reported Issues
1. Local `.jar` identification did not map to GitHub-hosted mods.
2. GitHub discover looked capped to one page / limited result depth.
3. Discover and instance pages could jitter when returning with active filters or broken images.
4. Problem banners were styled as success (green), which was misleading.
5. README lagged behind shipped GitHub/runtime features.

## Root Causes
- GitHub local resolver path was missing from local provider matching.
- GitHub repo search used single-page fetches and conservative candidate windows.
- Broken image URLs had no resilient fallback path in key list cards.
- Top status notices used a single success-style notice component.
- README still described older Modrinth+CurseForge-only behavior in several sections.

## Implemented Fixes
- Added GitHub local provider detection for local `.jar` imports/resolution:
  - Builds query hints from metadata + filename.
  - Scans policy-safe GitHub repos/releases.
  - Matches by exact release asset filename.
  - Uses checksum verification when digest metadata exists.
  - Writes provider candidates/source as `github` when matched.
- Expanded GitHub discover search depth:
  - Added true paged GitHub repo search requests (`page=`).
  - Multi-page aggregation per query with bounded caps.
  - Increased source-mode candidate ceiling for GitHub discover.
  - Added bounded release-fetch strategy and early-stop pagination to reduce per-search latency.
  - Added a fast-path “deferred release metadata” mode so GitHub search can return deeper paginated lists without blocking on per-repo release fetches.
  - Added provider icon fallback (GitHub owner avatar) plus best-effort Modrinth icon hint matching.
- Improved GitHub local identify coverage:
  - Increased GitHub local lookup query-hint breadth and repo candidate depth for popular repos that rely on filename-based matching.
- Added GitHub compatibility guards:
  - GitHub install/update selection now enforces loader + Minecraft version compatibility heuristics.
  - If no compatible release is found, install is blocked with a clear reason instead of installing an arbitrary release.
- Stabilized image rendering behavior:
  - Added resilient `RemoteImage` fallback handling.
  - Hardened installed icon component with error handling and failed-key suppression.
  - Added scroll-anchor guards on high-churn list/banner containers.
- Improved banner severity UX:
  - Added warning tone styles (`warningBox`).
  - Install notices now infer tone (success/warning/error) and style accordingly.
  - CurseForge blocked-download recovery prompt now renders as warning.
- Clarified filter behavior in Discover UI:
  - Added explicit provider capability messaging for GitHub/CurseForge filter limitations.
  - Disabled loader filtering in CurseForge-only mode to avoid misleading no-op behavior.
- Reduced instance filter jitter:
  - Scoped instance content filter state restore/persist to the active instance content route.
  - Debounced persistence writes to reduce high-frequency re-render churn while typing.
- Updated docs:
  - README now reflects GitHub discover/install/update/local-identify support.
  - README runtime section now clarifies canonical instance storage and isolated session behavior.
  - README now documents GitHub filter behavior and compatibility expectations.

## Validation Notes
- Added/updated Rust unit tests for:
  - GitHub digest matching behavior.
  - GitHub local release selector exact filename behavior.
- Manual QA targets:
  - Search `meteor client` and `trouser streak` with source `GitHub` and page forward.
  - Import local GitHub-release `.jar` and run **Identify local files**.
  - Trigger missing-provider notice and verify warning/error tone.
  - Navigate instance page with an active search, switch routes, and confirm reduced scroll jitter.
