import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog, save as saveDialog } from "@tauri-apps/api/dialog";
import { open as shellOpen } from "@tauri-apps/api/shell";
import { convertFileSrc } from "@tauri-apps/api/tauri";
import { getVersion } from "@tauri-apps/api/app";
import { checkUpdate, installUpdate } from "@tauri-apps/api/updater";
import { relaunch } from "@tauri-apps/api/process";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import rehypeSanitize from "rehype-sanitize";
import type {
  AccountDiagnostics,
  CreateInstanceFromModpackFileResult,
  CreatorPreset,
  CreatorPresetEntry,
  CreatorPresetSettings,
  CurseforgeApiStatus,
  GithubTokenPoolStatus,
  CurseforgeProjectDetail,
  GithubProjectDetail,
  GithubInstallState,
  DiscoverContentType,
  DiscoverSearchHit,
  DiscoverSource,
  InstanceLastRunMetadata,
  InstancePlaytimeSummary,
  InstanceRunReport,
  InstanceHistoryEvent,
  InstanceWorld,
  LaunchMethod,
  LauncherAccount,
  LauncherImportSource,
  LauncherSettings,
  PresetApplyPreview,
  PresetApplyResult,
  ReadInstanceLogsLine,
  ReadInstanceLogsResult,
  RollbackResult,
  WorldRollbackResult,
  ImportInstanceFromLauncherResult,
  InstanceLogSourceApi,
  RunningInstance,
  BeginMicrosoftLoginResult,
  ContentUpdateCheckResult,
  ContentUpdateInfo,
  AutoProfileRecommendation,
  FriendLinkReconcileResult,
  FriendLinkDriftPreview,
  FriendLinkStatus,
  InstanceHealthScore,
  LaunchCompatibilityReport,
  LaunchPermissionChecklistItem,
  LaunchFixAction,
  LaunchFixApplyResult,
  LaunchFixPlan,
  MicrosoftLoginState,
  InstallPlanPreview,
  Instance,
  InstanceSettings,
  InstallProgressEvent,
  InstalledMod,
  JavaRuntimeCandidate,
  LaunchResult,
  Loader,
  ProviderCandidate,
  QuickPlayServerEntry,
  ModpackSpec,
  SnapshotMeta,
  StorageBucketTotal,
  StorageCleanupRecommendation,
  StorageCleanupResult,
  StorageInstanceSummary,
  StorageUsageEntry,
  StorageUsageOverview,
  SupportPerfAction,
} from "./types";
import {
  applySelectedAccountAppearance,
  beginMicrosoftLogin,
  cancelInstanceLaunch,
  clearDevCurseforgeApiKey,
  checkInstanceContentUpdates,
  createInstance,
  createInstanceFromModpackFile,
  deleteInstance,
  exportPresetsJson,
  exportInstanceModsZip,
  exportInstanceSupportBundle,
  getDevModeState,
  getCurseforgeApiStatus,
  getGithubTokenPoolStatus,
  getCurseforgeProjectDetail,
  getGithubProjectDetail,
  getStorageUsageEntries,
  getStorageUsageOverview,
  getSelectedAccountDiagnostics,
  getInstanceDiskUsage,
  getInstanceLastRunMetadata,
  getInstancePlaytime,
  getInstanceLastRunReport,
  getLauncherSettings,
  importPresetsJson,
  importLocalModFile,
  importInstanceFromLauncher,
  installCurseforgeMod,
  installDiscoverContent,
  installModrinthMod,
  importProviderModpackTemplate,
  previewPresetApply,
  applyPresetToInstance,
  launchInstance,
  openMicrophoneSystemSettings as openMicrophoneSystemSettingsNative,
  listInstanceWorlds,
  listInstanceSnapshots,
  listLauncherAccounts,
  listLauncherImportSources,
  listRunningInstances,
  listInstalledMods,
  listInstances,
  logoutMicrosoftAccount,
  rollbackInstance,
  rollbackInstanceWorldBackup,
  reconcileFriendLink,
  resolveFriendLinkConflicts,
  getFriendLinkStatus,
  previewFriendLinkDrift,
  pollMicrosoftLogin,
  previewModrinthInstall,
  preflightLaunchCompatibility,
  pruneMissingInstalledEntries,
  triggerInstanceMicrophonePermissionPrompt,
  readInstanceLogs,
  readLocalImageDataUrl,
  revealConfigEditorFile,
  removeInstalledMod,
  listModpackSpecs,
  getModpackSpec,
  openInstancePath,
  searchDiscoverContent,
  selectLauncherAccount,
  setDevCurseforgeApiKey,
  setGithubTokenPool,
  setLauncherSettings,
  setInstanceIcon,
  setInstalledModEnabled,
  setInstalledModPin,
  setInstalledModProvider,
  resetInstanceConfigFilesWithBackup,
  syncFriendLinkSelected,
  stopRunningInstance,
  resolveLocalModSources,
  revealStorageUsagePath,
  attachInstalledModGithubRepo,
  runStorageCleanup,
  upsertModpackSpec,
  detectJavaRuntimes,
  updateAllInstanceContent,
  updateInstance,
  listInstanceHistoryEvents,
  listQuickPlayServers,
  upsertQuickPlayServer,
  removeQuickPlayServer,
  launchQuickPlayServer,
  clearGithubTokenPool,
} from "./tauri";
import {
  getProject,
  getProjectMembers,
  getProjectVersions,
  type ModrinthIndex,
  type Project,
  type ProjectMember,
  type ProjectVersion,
} from "./modrinth";
import { IdleAnimation, NameTagObject, SkinViewer } from "skinview3d";
import ModpacksConfigEditor from "./pages/ModpacksConfigEditor";
import ModpackMaker from "./pages/ModpackMaker";
import InstanceModpackCard from "./components/InstanceModpackCard";
import DependencyBadge from "./components/DependencyBadge";
import CommandPalette, { type CommandPaletteItem } from "./components/CommandPalette";
import Icon, { type IconName } from "./components/app-shell/Icon";
import NavButton from "./components/app-shell/NavButton";
import Modal from "./components/app-shell/Modal";
import GlobalTooltipLayer from "./components/app-shell/GlobalTooltipLayer";
import Dropdown from "./components/app-shell/controls/Dropdown";
import MultiSelectDropdown from "./components/app-shell/controls/MultiSelectDropdown";
import MenuSelect from "./components/app-shell/controls/MenuSelect";
import SegmentedControl from "./components/app-shell/controls/SegmentedControl";
import ActivityFeed from "./components/activity/ActivityFeed";
import FullHistoryView from "./components/activity/FullHistoryView";
import type { RecentActivityFeedEntry, RecentActivityFilter } from "./components/activity/types";
import {
  analyzeLogLines,
  analyzeLogText,
  detectCrashSuspectsFromMessages,
  extractLogTimestamp,
  inferLogSeverity,
  type CrashSuspect,
  type LogAnalyzeResult,
  type LogSeverity,
} from "./lib/logAnalysis";
import {
  formatBytes,
  formatCompact,
  formatDate,
  formatDateTime,
  formatDurationMs,
  formatEtaSeconds,
  formatFileSize,
  parseDateLike,
  formatPercent,
  formatPerfActionLabel,
  humanizeToken,
} from "./app/utils/format";
import {
  APP_LANGUAGE_OPTIONS,
  getAppLanguageOption,
  normalizeAppLanguage,
  translateAppText,
  type AppLanguage,
  type AppTranslationKey,
} from "./lib/i18n";
import {
  ACCOUNT_DIAGNOSTICS_CACHE_KEY,
  ACCOUNT_DIAGNOSTICS_TIMEOUT_MS,
  APP_MENU_CHECK_FOR_UPDATES_EVENT,
  APP_UPDATE_BANNER_ANIMATION_MS,
  APP_UPDATE_BANNER_AUTO_HIDE_MS,
  APP_UPDATER_AUTOCHECK_KEY,
  AUTOPROFILE_APPLIED_KEY,
  AUTOPROFILE_DISMISSED_KEY,
  DISCOVER_ADD_TRAY_STICKY_KEY,
  HOME_LAYOUT_KEY,
  INSTANCE_HEALTH_PANEL_PREFS_KEY,
  INSTANCE_CONTENT_FILTERS_KEY,
  INSTANCE_SETTINGS_MODE_KEY,
  INSTALL_NOTICE_AUTO_HIDE_MS,
  INSTALLED_ICON_CACHE_KEY,
  LAUNCH_OUTCOMES_KEY,
  PERF_ACTION_LOG_KEY,
  PREFLIGHT_IGNORE_KEY,
  SCHEDULED_UPDATE_WORKERS_MAX_KEY,
  SETTINGS_MODE_KEY,
  SKIN_HEAD_CACHE_MAX,
  SKIN_IMAGE_FETCH_TIMEOUT_MS,
  SKIN_THUMB_3D_SIZE,
  SKIN_THUMB_FRAMING_VERSION,
  SKIN_VIEWER_LOAD_TIMEOUT_MS,
  SUPPORT_BUNDLE_RAW_DEFAULT_KEY,
  TOP_ERROR_AUTO_HIDE_MS,
} from "./app/constants";

type Route = "home" | "discover" | "modpacks" | "library" | "updates" | "skins" | "instance" | "account" | "settings" | "dev";
type AccentPreset = "neutral" | "blue" | "emerald" | "amber" | "rose" | "violet" | "teal";
type AccentStrength = "subtle" | "normal" | "vivid" | "max";
type MotionPreset = "calm" | "standard" | "expressive";
type DensityPreset = "comfortable" | "compact";
type ProjectDetailTab = "overview" | "versions" | "changelog";
type CurseforgeDetailTab = "overview" | "files" | "changelog";
type GithubDetailTab = "overview" | "releases" | "readme";
type DiscoverProviderSource = Exclude<DiscoverSource, "all">;
type SchedulerCadence =
  | "off"
  | "hourly"
  | "every_3_hours"
  | "every_6_hours"
  | "every_12_hours"
  | "daily"
  | "weekly";
type SchedulerAutoApplyMode = "never" | "opt_in_instances" | "all_instances";
type SchedulerApplyScope = "scheduled_only" | "scheduled_and_manual";
type UpdatableContentType = "mods" | "resourcepacks" | "datapacks" | "shaderpacks";
type SettingsMode = "basic" | "advanced";
type FriendSyncPolicy = "manual" | "ask" | "auto_metadata" | "auto_all";
type FriendSyncPrefs = {
  policy: FriendSyncPolicy;
  snoozed_until: number;
};

type VersionItem = {
  id: string;
  type: "release" | "snapshot" | "old_beta" | "old_alpha" | string;
  release_time?: string;
};

type InstallTarget = {
  source: DiscoverSource;
  projectId: string;
  title: string;
  contentType: DiscoverContentType;
  slug?: string | null;
  targetWorlds?: string[];
  iconUrl?: string | null;
  description?: string | null;
  installSupported?: boolean;
  installNote?: string | null;
};

type CurseforgeBlockedRecoveryPrompt = {
  instanceId: string;
  instanceName: string;
  contentView: "mods" | "resourcepacks" | "datapacks" | "shaders";
  target: InstallTarget;
  projectUrl: string;
};

type DiscoverAddContext = {
  modpackId: string;
  modpackName: string;
  layerId?: string | null;
  layerName?: string | null;
};

type DiscoverAddTrayItem = {
  id: string;
  title: string;
  projectId: string;
  source: DiscoverSource;
  contentType: DiscoverContentType;
  modpackName: string;
  layerName: string;
  addedAt: string;
};

type GithubAttachModalTarget = {
  instanceId: string;
  instanceName: string;
  mod: InstalledMod;
};

type InstanceLaunchStateEvent = {
  instance_id: string;
  launch_id?: string | null;
  method?: string | null;
  status?: string | null;
  message?: string | null;
};

type LaunchHealthChecks = {
  auth: boolean;
  assets: boolean;
  libraries: boolean;
  starting_java: boolean;
};

type LaunchHealthRecord = {
  first_success_at: string;
  checks: LaunchHealthChecks;
};

type LaunchFailureRecord = {
  status: string;
  method: string;
  message: string;
  updated_at: number;
};

type InstanceActivityEntry = {
  id: string;
  message: string;
  at: number;
  tone: "info" | "success" | "warn" | "error";
};

type ScheduledUpdateCheckEntry = {
  instance_id: string;
  instance_name: string;
  checked_at: string;
  checked_entries: number;
  update_count: number;
  updates: ContentUpdateInfo[];
  error?: string | null;
};

type ScheduledAppliedUpdateEntry = {
  instance_id: string;
  instance_name: string;
  applied_at: string;
  updated_entries: number;
  updates: ContentUpdateInfo[];
  warnings: string[];
};

type AppUpdaterState = {
  checked_at: string;
  current_version: string;
  available: boolean;
  latest_version?: string | null;
  release_notes?: string | null;
  pub_date?: string | null;
};

type PerfActionStatus = "ok" | "error";

type PerfActionEntry = {
  id: string;
  name: string;
  detail?: string | null;
  status: PerfActionStatus;
  duration_ms: number;
  finished_at: number;
};

type HomeWidgetId =
  | "action_required"
  | "launchpad"
  | "recent_activity"
  | "performance_pulse"
  | "friend_link"
  | "maintenance"
  | "running_sessions"
  | "recent_instances";

type HomeWidgetLayoutItem = {
  id: HomeWidgetId;
  visible: boolean;
  pinned: boolean;
  column: "main" | "side";
  order: number;
};

type InstanceContentFilters = {
  query: string;
  state: "all" | "enabled" | "disabled";
  source: "all" | "modrinth" | "curseforge" | "github" | "local" | "other";
  missing: "all" | "missing" | "present";
  warningsOnly: boolean;
};

type InstanceContentSort =
  | "recently_added"
  | "name_asc"
  | "name_desc"
  | "source"
  | "enabled_first"
  | "disabled_first";
type InstanceContentBulkAction = "__menu" | "add_local" | "identify_local" | "clean_missing";

type LaunchOutcomeEntry = {
  at: number;
  ok: boolean;
  message?: string | null;
};

type LaunchFixActionDraft = LaunchFixAction & {
  selected: boolean;
  dryRun?: string;
  reversible?: boolean;
};

type LaunchOutcomesByInstance = Record<string, LaunchOutcomeEntry[]>;
type LaunchPreflightIgnoreEntry = {
  fingerprint: string;
  expires_at: number;
};
type InstanceHealthPanelPrefs = Record<
  string,
  {
    hidden?: boolean;
    collapsed?: boolean;
    permissions_expanded?: boolean;
  }
>;

const CRITICAL_UPDATE_TOKENS = [
  "fabric-api",
  "fabric_api",
  "architectury",
  "architectury-api",
  "cloth-config",
  "forge-config-api-port",
  "forge_config_api_port",
  "neoforge",
  "forge",
];

const CRASH_OUTCOME_RE = /\b(crash|exception|fatal|sigsegv|exit code -1)\b/i;

function isLikelyCriticalUpdate(update: ContentUpdateInfo) {
  const haystack = `${update.project_id} ${update.name}`.toLowerCase();
  return CRITICAL_UPDATE_TOKENS.some((token) => haystack.includes(token));
}

function pickAppliedUpdatesFromCheck(
  check: ContentUpdateCheckResult,
  updatedEntries: number
): ContentUpdateInfo[] {
  const applied = Math.max(0, Number(updatedEntries) || 0);
  if (applied === 0) return [];
  return check.updates.slice(0, Math.min(check.updates.length, applied));
}

function healthScoreGrade(score: number): InstanceHealthScore["grade"] {
  if (score >= 90) return "A";
  if (score >= 75) return "B";
  if (score >= 60) return "C";
  if (score >= 45) return "D";
  return "F";
}

function computeInstanceHealthScore(args: {
  instanceId: string;
  launchOutcomesByInstance: LaunchOutcomesByInstance;
  friendStatus?: FriendLinkStatus | null;
  scheduledUpdatesByInstance: Record<string, ScheduledUpdateCheckEntry>;
}): InstanceHealthScore {
  const now = Date.now();
  let score = 100;
  const reasons: string[] = [];
  const outcomes = (args.launchOutcomesByInstance[args.instanceId] ?? []).slice(0, 24);
  const recent72h = outcomes.filter((item) => now - Number(item.at || 0) <= 72 * 60 * 60 * 1000);
  const hasRecentFailedLaunch = recent72h.some((item) => !item.ok);
  if (hasRecentFailedLaunch) {
    score -= 40;
    reasons.push("Recent launch failure");
  }
  if ((args.friendStatus?.pending_conflicts_count ?? 0) > 0) {
    score -= 25;
    reasons.push("Friend Link conflicts pending");
  }
  const crashLikeCount = outcomes.filter((item) => !item.ok && CRASH_OUTCOME_RE.test(String(item.message ?? ""))).length;
  if (crashLikeCount > 0) {
    const penalty = Math.min(20, crashLikeCount * 5);
    score -= penalty;
    reasons.push(`${crashLikeCount} crash-like outcome${crashLikeCount === 1 ? "" : "s"}`);
  }
  const scheduled = args.scheduledUpdatesByInstance[args.instanceId];
  const criticalUpdates = (scheduled?.updates ?? []).filter(isLikelyCriticalUpdate).length;
  if (criticalUpdates > 0) {
    const penalty = Math.min(15, criticalUpdates * 5);
    score -= penalty;
    reasons.push(`${criticalUpdates} critical update${criticalUpdates === 1 ? "" : "s"} pending`);
  }
  const recentStreak = outcomes.slice(0, 3);
  if (recentStreak.length >= 2 && recentStreak.every((item) => item.ok)) {
    score += 5;
    reasons.push("Recent launch streak");
  }
  score = Math.max(0, Math.min(100, score));
  return {
    score,
    grade: healthScoreGrade(score),
    reasons,
  };
}

function computeAutoProfileRecommendation(args: {
  instance: Instance;
  enabledModCount: number;
  launchOutcomesByInstance: LaunchOutcomesByInstance;
}): AutoProfileRecommendation {
  const outcomes = (args.launchOutcomesByInstance[args.instance.id] ?? []).slice(0, 16);
  const outcomeText = outcomes.map((item) => String(item.message ?? "").toLowerCase()).join("\n");
  const oomLike = /\boutofmemoryerror|java heap space|gc overhead\b/i.test(outcomeText);
  const recentFailureRate = outcomes.length
    ? outcomes.filter((item) => !item.ok).length / outcomes.length
    : 0;
  let memoryMb = 4096;
  const reasons: string[] = [];
  if (args.enabledModCount >= 220) {
    memoryMb = 8192;
    reasons.push("Very high enabled mod count");
  } else if (args.enabledModCount >= 140) {
    memoryMb = 6144;
    reasons.push("High enabled mod count");
  } else if (args.enabledModCount <= 45) {
    memoryMb = 3072;
    reasons.push("Light mod footprint");
  } else {
    reasons.push("Balanced mod footprint");
  }
  if (oomLike) {
    memoryMb = Math.max(memoryMb, 8192);
    reasons.push("OOM signature detected in launch history");
  }
  const loader = String(args.instance.loader ?? "").toLowerCase();
  const jvmArgs = loader === "vanilla"
    ? "-XX:+UseG1GC -XX:MaxGCPauseMillis=75"
    : "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=60 -XX:+UnlockExperimentalVMOptions";
  let graphicsPreset: AutoProfileRecommendation["graphics_preset"] = "Balanced";
  if (args.enabledModCount >= 180 || recentFailureRate >= 0.35) {
    graphicsPreset = "Performance";
  } else if (args.enabledModCount <= 60 && recentFailureRate < 0.15) {
    graphicsPreset = "Quality";
  }
  const confidence: AutoProfileRecommendation["confidence"] =
    outcomes.length >= 4 ? (recentFailureRate < 0.2 ? "high" : "medium") : "low";
  return {
    memory_mb: memoryMb,
    jvm_args: jvmArgs,
    graphics_preset: graphicsPreset,
    confidence,
    reasons,
  };
}

function autoProfileSignature(recommendation: AutoProfileRecommendation): string {
  const reasons = [...(recommendation.reasons ?? [])].map((item) => String(item).trim()).filter(Boolean);
  return [
    recommendation.memory_mb,
    recommendation.jvm_args.trim(),
    recommendation.graphics_preset,
    recommendation.confidence,
    reasons.join("|"),
  ].join("::");
}

function normalizeVersionLabel(value: string | null | undefined) {
  return String(value ?? "").trim().replace(/^v/i, "");
}

function emptyLaunchHealthChecks(): LaunchHealthChecks {
  return {
    auth: false,
    assets: false,
    libraries: false,
    starting_java: false,
  };
}

function mergeLaunchChecksFromMessage(
  prev: LaunchHealthChecks,
  message?: string | null
): LaunchHealthChecks {
  const text = String(message ?? "").toLowerCase();
  return {
    auth: prev.auth || text.includes("refreshing microsoft"),
    assets: prev.assets || text.includes("installing assets"),
    libraries: prev.libraries || text.includes("installing libraries"),
    starting_java: prev.starting_java || text.includes("starting java process"),
  };
}

function launchStageBadgeLabel(status?: string | null, message?: string | null) {
  const state = String(status ?? "").toLowerCase();
  const text = String(message ?? "").toLowerCase();
  if (state === "running") return "Running";
  if (state === "stopped") return "Stopped";
  if (state === "exited") return "Exited";
  if (text.includes("isolated runtime session")) return "Disposable Runtime";
  if (text.includes("refreshing microsoft")) return "Auth";
  if (text.includes("installing game version")) return "Version";
  if (text.includes("installing assets")) return "Assets";
  if (text.includes("installing libraries")) return "Libraries";
  if (text.includes("starting java process")) return "Starting Java";
  if (text.includes("preparing runtime")) return "Runtime";
  if (text.includes("preparing native launch")) return "Preparing";
  if (text.includes("preparing prism")) return "Prism Sync";
  if (state === "starting") return "Launching";
  return "";
}

function inferActivityTone(message: string): InstanceActivityEntry["tone"] {
  const lower = message.toLowerCase();
  if (/\b(fail|failed|error|exception|fatal|crash)\b/.test(lower)) return "error";
  if (/\b(warn|warning|retry)\b/.test(lower)) return "warn";
  if (/\b(success|ready|launched|started|complete|completed|saved|updated|refreshed|connected)\b/.test(lower)) {
    return "success";
  }
  return "info";
}

const RECENT_ACTIVITY_WINDOW_HOURS = 24;
const RECENT_ACTIVITY_LIMIT = 12;
const RECENT_ACTIVITY_COALESCE_WINDOW_MS = 20_000;
const FULL_HISTORY_PAGE_SIZE = 60;
const INSTANCE_HISTORY_STORE_LIMIT = 300;

function hasDependencyWarnings(mod: InstalledMod): boolean {
  return Array.isArray(mod.local_analysis?.warnings) && (mod.local_analysis?.warnings.length ?? 0) > 0;
}

function extractDependencyHints(mod: InstalledMod): string[] {
  const hints = new Set<string>();
  const fromAnalyzer = Array.isArray(mod.local_analysis?.required_dependencies)
    ? mod.local_analysis?.required_dependencies ?? []
    : [];
  for (const value of fromAnalyzer) {
    const normalized = String(value ?? "").trim().toLowerCase();
    if (/^[a-z0-9._-]{2,64}$/.test(normalized)) hints.add(normalized);
  }
  const warnings = Array.isArray(mod.local_analysis?.warnings) ? mod.local_analysis?.warnings ?? [] : [];
  const patterns = [
    /\bmissing mandatory dependency\s+([a-z0-9._-]{2,64})/gi,
    /\brequires\s+([a-z0-9._-]{2,64})/gi,
    /\bdepends on\s+([a-z0-9._-]{2,64})/gi,
  ];
  for (const warning of warnings) {
    const text = String(warning ?? "").toLowerCase();
    for (const pattern of patterns) {
      for (const match of text.matchAll(pattern)) {
        const candidate = String(match?.[1] ?? "").trim();
        if (/^[a-z0-9._-]{2,64}$/.test(candidate)) hints.add(candidate);
      }
    }
  }
  const blocked = new Set(["minecraft", "fabric", "forge", "quilt", "neoforge", "java"]);
  for (const id of mod.local_analysis?.mod_ids ?? []) {
    blocked.add(String(id ?? "").trim().toLowerCase());
  }
  return Array.from(hints).filter((id) => id && !blocked.has(id));
}

function relativeTimeFromMs(atMs: number): string {
  if (!Number.isFinite(atMs) || atMs <= 0) return "Unknown";
  const deltaSeconds = Math.round((atMs - Date.now()) / 1000);
  const absSeconds = Math.abs(deltaSeconds);
  if (absSeconds < 10) return "Just now";
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  if (absSeconds < 60) return formatter.format(deltaSeconds, "second");
  const deltaMinutes = Math.round(deltaSeconds / 60);
  if (Math.abs(deltaMinutes) < 60) return formatter.format(deltaMinutes, "minute");
  const deltaHours = Math.round(deltaMinutes / 60);
  if (Math.abs(deltaHours) < 24) return formatter.format(deltaHours, "hour");
  const deltaDays = Math.round(deltaHours / 24);
  return formatter.format(deltaDays, "day");
}

function inferRecentActivityTarget(summary: string): string {
  const text = String(summary ?? "").trim();
  if (!text) return "Instance";
  const singleQuoted = text.match(/'([^']+)'/);
  if (singleQuoted?.[1]) return singleQuoted[1];
  const doubleQuoted = text.match(/"([^"]+)"/);
  if (doubleQuoted?.[1]) return doubleQuoted[1];
  const fileLike = text.match(/([a-z0-9._-]+\.(?:jar|zip|json|toml|yml|yaml|txt))/i);
  if (fileLike?.[1]) return fileLike[1];
  const forTarget = text.match(/\bfor\s+([a-z0-9._:-]{3,})/i);
  if (forTarget?.[1]) return forTarget[1];
  return "Instance";
}

function inferRecentActivityCategory(
  rawKind: string,
  summary: string,
  tone: InstanceActivityEntry["tone"]
): RecentActivityFilter {
  const kind = String(rawKind ?? "").toLowerCase();
  const text = String(summary ?? "").toLowerCase();
  if (tone === "error" || tone === "warn") return "warnings";
  if (kind.includes("import") || text.includes("imported local")) return "imports";
  if (
    kind.includes("pin") ||
    /\b(set pin|clear(?:ed)? pin|pinned|unpinned)\b/.test(text) ||
    text.includes(" pin ")
  ) {
    return "pins";
  }
  if (
    kind.includes("update") ||
    kind.includes("upgrade") ||
    kind.includes("refresh") ||
    kind.includes("snapshot") ||
    kind.includes("rollback")
  ) {
    return "updates";
  }
  if (
    kind.includes("install") ||
    kind.includes("add") ||
    kind.includes("resolve") ||
    text.includes("installed") ||
    text.includes("resolved")
  ) {
    return "installs";
  }
  return "all";
}

function inferRecentActivityVisual(
  rawKind: string,
  summary: string,
  tone: InstanceActivityEntry["tone"]
): Pick<RecentActivityFeedEntry, "icon" | "accent"> {
  const kind = String(rawKind ?? "").toLowerCase();
  const text = String(summary ?? "").toLowerCase();
  if (tone === "error" || tone === "warn" || /\b(error|warn|failed|missing)\b/.test(text)) {
    return { icon: "slash_circle", accent: "amber" };
  }
  if (kind.includes("import") || text.includes("imported local")) {
    return { icon: "upload", accent: "blue" };
  }
  if (
    kind.includes("pin") ||
    /\b(set pin|clear(?:ed)? pin|pinned|unpinned)\b/.test(text) ||
    text.includes(" pin ")
  ) {
    return { icon: "layers", accent: "purple" };
  }
  if (kind.includes("resolve") || text.includes("resolved")) {
    return { icon: "check_circle", accent: "green" };
  }
  if (kind.includes("update") || kind.includes("install") || text.includes("installed")) {
    return { icon: "download", accent: "neutral" };
  }
  return { icon: "sparkles", accent: "neutral" };
}

function toRecentActivityEntry(input: {
  id: string;
  atMs: number;
  message: string;
  tone: InstanceActivityEntry["tone"];
  rawKind: string;
  sourceLabel: string;
}): RecentActivityFeedEntry {
  const category = inferRecentActivityCategory(input.rawKind, input.message, input.tone);
  const visual = inferRecentActivityVisual(input.rawKind, input.message, input.tone);
  return {
    id: input.id,
    atMs: input.atMs,
    tone: input.tone,
    message: input.message,
    target: inferRecentActivityTarget(input.message),
    sourceLabel: input.sourceLabel,
    rawKind: input.rawKind,
    category,
    icon: visual.icon,
    accent: visual.accent,
    exactTime: formatDateTime(new Date(input.atMs).toISOString(), "Unknown time"),
    relativeTime: relativeTimeFromMs(input.atMs),
  };
}

function summarizeWarnings(warnings: string[], maxItems = 3): string {
  const cleaned = warnings
    .map((item) => String(item ?? "").trim())
    .filter((item) => item.length > 0);
  if (cleaned.length === 0) return "";
  const preview = cleaned.slice(0, Math.max(1, maxItems));
  const extra = cleaned.length - preview.length;
  return extra > 0 ? `${preview.join(" | ")} | +${extra} more` : preview.join(" | ");
}

function hasGithubRateLimitWarning(warnings: string[]): boolean {
  return warnings.some((warning) => {
    const text = String(warning ?? "").toLowerCase();
    return text.includes("github") && text.includes("rate limit");
  });
}

type StorageManagerSelection = "overview" | "app" | "cache" | `instance:${string}`;
type StorageManagerScope = "overview" | "app" | "cache" | "instance";
type StorageDetailMode = "folders" | "files";

function storageSelectionForInstance(instanceId: string): StorageManagerSelection {
  return `instance:${instanceId}`;
}

function parseStorageSelection(selection: StorageManagerSelection): {
  scope: StorageManagerScope;
  instanceId?: string;
} {
  if (selection === "overview" || selection === "app" || selection === "cache") {
    return { scope: selection };
  }
  if (selection.startsWith("instance:")) {
    return { scope: "instance", instanceId: selection.slice("instance:".length) };
  }
  return { scope: "overview" };
}

function storageRequestKey(
  selection: StorageManagerSelection,
  mode: StorageDetailMode,
  relativePath: string
): string {
  return `${selection}:${mode}:${relativePath.trim()}`;
}

function storageInstanceBreakdown(summary: StorageInstanceSummary): StorageBucketTotal[] {
  return [
    { key: "mods", label: "Mods", bytes: Number(summary.mods ?? 0) },
    { key: "resourcepacks", label: "Resource packs", bytes: Number(summary.resourcepacks ?? 0) },
    { key: "shaderpacks", label: "Shaderpacks", bytes: Number(summary.shaderpacks ?? 0) },
    { key: "saves", label: "Saves", bytes: Number(summary.saves ?? 0) },
    { key: "config", label: "Config", bytes: Number(summary.config ?? 0) },
    { key: "snapshots", label: "Snapshots", bytes: Number(summary.snapshots ?? 0) },
    { key: "world_backups", label: "World backups", bytes: Number(summary.world_backups ?? 0) },
    { key: "logs", label: "Logs", bytes: Number(summary.logs ?? 0) },
    { key: "crash_reports", label: "Crash reports", bytes: Number(summary.crash_reports ?? 0) },
    { key: "runtime_sessions", label: "Runtime sessions", bytes: Number(summary.runtime_sessions ?? 0) },
    { key: "other", label: "Other", bytes: Number(summary.other ?? 0) },
  ];
}

function storageAppBreakdownForScope(
  overview: StorageUsageOverview | null,
  scope: "app" | "cache"
): StorageBucketTotal[] {
  const rows = overview?.app_breakdown ?? [];
  return rows.filter((row) =>
    scope === "cache" ? row.key.startsWith("shared_cache_") : !row.key.startsWith("shared_cache_")
  );
}

function storageCleanupRecommendationLabel(recommendation: StorageCleanupRecommendation): string {
  const bytes = Number(recommendation.reclaimable_bytes ?? 0);
  return bytes > 0 ? `${recommendation.title} · ${formatBytes(bytes)}` : recommendation.title;
}

function storageRevealActionLabel() {
  return isMacDesktopPlatform() ? "Reveal in Finder" : "Reveal in file manager";
}

type MicrosoftCodePrompt = {
  code: string;
  verificationUrl: string;
};

type LibraryGroupBy = "none" | "loader" | "version";

type LibraryContextMenuState = {
  instanceId: string;
  x: number;
  y: number;
};

type UserPresetEntry = CreatorPresetEntry;
type UserPreset = CreatorPreset;

type PresetExportPayload = {
  format: "mpm-presets/v2";
  exported_at: string;
  presets: UserPreset[];
};

type AccountSkinOption = {
  id: string;
  label: string;
  skin_url: string;
  apply_source?: string | null;
  variant?: string | null;
  preview_url?: string | null;
  group: "saved" | "default";
  origin: "profile" | "custom" | "default";
};

type AccountSkinThumbSet = {
  front: string;
  back: string;
  mode: "3d" | "fallback";
};

type SavedCustomSkin = {
  id: string;
  label: string;
  skin_path: string;
  preview_data_url?: string | null;
};

type InstanceLaunchHooksDraft = {
  enabled: boolean;
  pre_launch: string;
  wrapper: string;
  post_exit: string;
};

function defaultPresetSettings(): CreatorPresetSettings {
  return {
    dependency_policy: "required",
    conflict_strategy: "replace",
    provider_priority: ["modrinth", "curseforge"],
    snapshot_before_apply: true,
    apply_order: ["mods", "resourcepacks", "shaderpacks", "datapacks"],
    datapack_target_policy: "choose_worlds",
  };
}

function defaultInstanceSettings(): InstanceSettings {
  return {
    keep_launcher_open_while_playing: true,
    close_launcher_on_game_exit: false,
    notes: "",
    sync_minecraft_settings: true,
    sync_minecraft_settings_target: "none",
    auto_update_installed_content: false,
    prefer_release_builds: true,
    java_path: "",
    memory_mb: 4096,
    jvm_args: "",
    graphics_preset: "Balanced",
    enable_shaders: false,
    force_vsync: false,
    world_backup_interval_minutes: 10,
    world_backup_retention_count: 1,
    snapshot_retention_count: 5,
    snapshot_max_age_days: 14,
  };
}

function defaultLaunchHooksDraft(): InstanceLaunchHooksDraft {
  return {
    enabled: false,
    pre_launch: "",
    wrapper: "",
    post_exit: "",
  };
}

function normalizeInstanceSettings(input?: Partial<InstanceSettings> | null): InstanceSettings {
  const merged = {
    ...defaultInstanceSettings(),
    ...(input ?? {}),
  };
  const normalizedMemory = Number.isFinite(Number(merged.memory_mb))
    ? Math.max(512, Math.min(65536, Math.round(Number(merged.memory_mb))))
    : 4096;
  const preset = String(merged.graphics_preset ?? "Balanced");
  const graphicsPreset = ["Performance", "Balanced", "Quality"].includes(preset) ? preset : "Balanced";
  const backupInterval = Number.isFinite(Number(merged.world_backup_interval_minutes))
    ? Math.max(5, Math.min(15, Math.round(Number(merged.world_backup_interval_minutes))))
    : 10;
  const backupRetention = Number.isFinite(Number(merged.world_backup_retention_count))
    ? Math.max(1, Math.min(2, Math.round(Number(merged.world_backup_retention_count))))
    : 1;
  const snapshotRetention = Number.isFinite(Number(merged.snapshot_retention_count))
    ? Math.max(1, Math.min(20, Math.round(Number(merged.snapshot_retention_count))))
    : 5;
  const snapshotMaxAgeDays = Number.isFinite(Number(merged.snapshot_max_age_days))
    ? Math.max(1, Math.min(90, Math.round(Number(merged.snapshot_max_age_days))))
    : 14;
  const syncTargetRaw = String(merged.sync_minecraft_settings_target ?? "none").trim();
  const syncTarget = syncTargetRaw.length > 0 ? syncTargetRaw : "none";
  return {
    ...merged,
    notes: String(merged.notes ?? ""),
    sync_minecraft_settings: Boolean(merged.sync_minecraft_settings),
    sync_minecraft_settings_target: syncTarget,
    java_path: String(merged.java_path ?? "").trim(),
    jvm_args: String(merged.jvm_args ?? "").trim(),
    graphics_preset: graphicsPreset,
    memory_mb: normalizedMemory,
    world_backup_interval_minutes: backupInterval,
    world_backup_retention_count: backupRetention,
    snapshot_retention_count: snapshotRetention,
    snapshot_max_age_days: snapshotMaxAgeDays,
  };
}

function requiredJavaMajorForMcVersion(mcVersion: string): number {
  const parts = parseReleaseParts(mcVersion);
  if (!parts) return 17;
  const major = parts[0] ?? 0;
  const minor = parts[1] ?? 0;
  if (major > 1 || (major === 1 && minor >= 20 && (parts[2] ?? 0) >= 5)) return 21;
  if (major > 1 || (major === 1 && minor >= 18)) return 17;
  return 8;
}

function javaRuntimeDisplayLabel(runtime: JavaRuntimeCandidate): string {
  const majorSuffix = Number.isFinite(runtime.major) && runtime.major > 0 ? ` ${runtime.major}` : "";
  const haystack = `${runtime.version_line} ${runtime.path}`.toLowerCase();
  if (haystack.includes("temurin")) return `Temurin${majorSuffix}`;
  if (
    haystack.includes("/opt/homebrew/") ||
    haystack.includes("/homebrew/") ||
    haystack.includes("homebrew")
  ) {
    return `Homebrew OpenJDK${majorSuffix}`;
  }
  if (
    haystack.includes("/library/java/javavirtualmachines/") ||
    haystack.includes("/usr/bin/java") ||
    haystack.includes("system")
  ) {
    return `System Java${majorSuffix}`;
  }
  if (haystack.includes("openjdk")) return `OpenJDK${majorSuffix}`;
  if (haystack.includes("oracle")) return `Oracle Java${majorSuffix}`;
  return `Java${majorSuffix}`;
}

function normalizeCreatorEntryType(input?: string) {
  const raw = String(input ?? "").trim().toLowerCase();
  if (!raw) return "mods";
  const value = raw.replace(/[\s_-]+/g, "");
  if (value === "resourcepack" || value === "resourcepacks") return "resourcepacks";
  if (value === "shaderpack" || value === "shaderpacks" || value === "shaders") return "shaderpacks";
  if (value === "datapack" || value === "datapacks") return "datapacks";
  if (value === "modpack" || value === "modpacks") return "modpacks";
  if (value === "mod" || value === "mods") return "mods";
  return "mods";
}

function normalizeInstanceContentType(input?: string): "mods" | "resourcepacks" | "datapacks" | "shaders" {
  const value = normalizeCreatorEntryType(input);
  if (value === "shaderpacks") return "shaders";
  if (value === "resourcepacks") return "resourcepacks";
  if (value === "datapacks") return "datapacks";
  return "mods";
}

function instanceContentTypeToBackend(input: "mods" | "resourcepacks" | "datapacks" | "shaders") {
  if (input === "shaders") return "shaderpacks";
  return input;
}

function localImportExtensionsForInstanceType(input: "mods" | "resourcepacks" | "datapacks" | "shaders") {
  if (input === "mods") return ["jar"];
  if (input === "resourcepacks") return ["zip"];
  if (input === "datapacks") return ["zip"];
  return ["zip", "jar"];
}

function localImportTypeLabel(input: "mods" | "resourcepacks" | "datapacks" | "shaders") {
  if (input === "mods") return "mod";
  if (input === "resourcepacks") return "resourcepack";
  if (input === "datapacks") return "datapack";
  return "shaderpack";
}

function normalizeUpdatableContentType(input?: string): UpdatableContentType | null {
  const value = String(input ?? "").trim().toLowerCase();
  if (value === "mods" || value === "mod") return "mods";
  if (value === "resourcepacks" || value === "resourcepack") return "resourcepacks";
  if (value === "datapacks" || value === "datapack") return "datapacks";
  if (value === "shaderpacks" || value === "shaderpack" || value === "shaders") return "shaderpacks";
  return null;
}

function updateContentTypeLabel(value: UpdatableContentType): string {
  if (value === "mods") return "mods";
  if (value === "resourcepacks") return "resourcepacks";
  if (value === "datapacks") return "datapacks";
  return "shaders";
}

function summarizeUpdateContentTypeSelection(values: UpdatableContentType[]): string {
  if (values.length === 0 || values.length === ALL_UPDATABLE_CONTENT_TYPES.length) {
    return "all content";
  }
  return values.map((value) => updateContentTypeLabel(value)).join(", ");
}

function instanceContentSectionLabel(input: "mods" | "resourcepacks" | "datapacks" | "shaders") {
  if (input === "mods") return "mods";
  if (input === "resourcepacks") return "resource packs";
  if (input === "datapacks") return "datapacks";
  return "shaderpacks";
}

function discoverContentTypeToInstanceView(
  input?: DiscoverContentType
): "mods" | "resourcepacks" | "datapacks" | "shaders" {
  const normalized = normalizeCreatorEntryType(input);
  if (normalized === "resourcepacks") return "resourcepacks";
  if (normalized === "datapacks") return "datapacks";
  if (normalized === "shaderpacks") return "shaders";
  return "mods";
}

function curseforgeCategoryPathForContentType(contentType?: DiscoverContentType) {
  const normalized = normalizeCreatorEntryType(contentType);
  if (normalized === "resourcepacks") return "texture-packs";
  if (normalized === "shaderpacks") return "shaders";
  if (normalized === "datapacks") return "data-packs";
  return "mc-mods";
}

function buildCurseforgeProjectUrl(target: InstallTarget) {
  const slug = String(target.slug ?? "").trim();
  if (slug) {
    const category = curseforgeCategoryPathForContentType(target.contentType);
    return `https://www.curseforge.com/minecraft/${category}/${slug}`;
  }
  return `https://www.curseforge.com/projects/${encodeURIComponent(target.projectId)}`;
}

function isCurseforgeBlockedDownloadUrlError(message?: string | null) {
  const value = String(message ?? "").toLowerCase();
  if (!value) return false;
  return (
    value.includes("curseforge blocked automated download url access") ||
    (value.includes("curseforge download-url lookup failed with status 403") &&
      value.includes("403"))
  );
}

function installedContentTypeLabel(contentType?: string) {
  const normalized = normalizeCreatorEntryType(contentType);
  if (normalized === "resourcepacks") return "resourcepack";
  if (normalized === "shaderpacks") return "shaderpack";
  if (normalized === "datapacks") return "datapack";
  return "mod";
}

function installedContentTypeToDiscover(contentType?: string): DiscoverContentType {
  const normalized = normalizeCreatorEntryType(contentType);
  if (normalized === "resourcepacks") return "resourcepacks";
  if (normalized === "shaderpacks") return "shaderpacks";
  if (normalized === "datapacks") return "datapacks";
  if (normalized === "modpacks") return "modpacks";
  return "mods";
}

function normalizeProviderSource(
  value?: string | null
): "modrinth" | "curseforge" | "github" | "local" | "other" {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (normalized === "modrinth") return "modrinth";
  if (normalized === "curseforge") return "curseforge";
  if (normalized === "github") return "github";
  if (normalized === "local") return "local";
  return "other";
}

function providerSourceLabel(value?: string | null): string {
  const normalized = normalizeProviderSource(value);
  if (normalized === "modrinth") return "Modrinth";
  if (normalized === "curseforge") return "CurseForge";
  if (normalized === "github") return "GitHub";
  if (normalized === "local") return "Local";
  return String(value ?? "Unknown").trim() || "Unknown";
}

function normalizeGithubVerificationStatus(value?: string | null) {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (
    normalized === "verified" ||
    normalized === "deferred" ||
    normalized === "manual_unverified" ||
    normalized === "unavailable"
  ) {
    return normalized;
  }
  return "unknown";
}

function normalizeGithubInstallState(value?: string | null): GithubInstallState {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (normalized === "ready" || normalized === "checking" || normalized === "unsupported") {
    return normalized;
  }
  return "checking";
}

function githubVerificationStatusLabel(value?: string | null): string | null {
  const normalized = normalizeGithubVerificationStatus(value);
  if (normalized === "verified") return "verified release";
  if (normalized === "deferred") return "release check on open";
  if (normalized === "manual_unverified") return "manual verification";
  if (normalized === "unavailable") return "GitHub check unavailable";
  return null;
}

function githubInstallStateChipLabel(value?: string | null): string | null {
  const normalized = normalizeGithubInstallState(value);
  if (normalized === "unsupported") return "no compatible release";
  return null;
}

function providerCandidateExplain(candidate: ProviderCandidate): string | null {
  const statusLabel =
    normalizeProviderSource(candidate.source) === "github"
      ? githubVerificationStatusLabel(candidate.verification_status)
      : null;
  const confidence = String(candidate.confidence ?? "").trim();
  const reason = String(candidate.reason ?? "").trim();
  if (!statusLabel && !confidence && !reason) return null;
  const parts = [providerSourceLabel(candidate.source)];
  if (statusLabel) parts.push(statusLabel);
  if (confidence) parts.push(`${confidence} confidence`);
  if (reason) parts.push(reason);
  return parts.join(" • ");
}

function githubStatusChipClass(kind: "verification" | "installability", value?: string | null) {
  if (kind === "verification") {
    const normalized = normalizeGithubVerificationStatus(value);
    if (normalized === "verified") return "chip subtle";
    if (normalized === "unavailable") return "chip danger";
    return "chip";
  }
  const normalized = normalizeGithubInstallState(value);
  if (normalized === "ready") return "chip subtle";
  if (normalized === "unsupported") return "chip danger";
  return "chip";
}

function githubInstallSummary(
  hit: DiscoverSearchHit | null | undefined,
  detail?: GithubProjectDetail | null
): string | null {
  const detailSummary = String(detail?.install_summary ?? "").trim();
  if (detailSummary) return detailSummary;
  const hitSummary = String(hit?.install_summary ?? "").trim();
  return hitSummary || null;
}

function githubInstallState(
  hit: DiscoverSearchHit | null | undefined,
  detail?: GithubProjectDetail | null
): GithubInstallState {
  return normalizeGithubInstallState(detail?.install_state ?? hit?.install_state);
}

function githubResultInstallSupported(
  hit: DiscoverSearchHit | null | undefined,
  detail?: GithubProjectDetail | null
): boolean {
  return githubInstallState(hit, detail) !== "unsupported";
}

function githubResultInstallNote(
  hit: DiscoverSearchHit | null | undefined,
  detail?: GithubProjectDetail | null
): string | null {
  return githubInstallSummary(hit, detail);
}


function normalizeDiscoverSource(value?: string | null): DiscoverSource {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (normalized === "modrinth" || normalized === "curseforge" || normalized === "github") {
    return normalized;
  }
  return "modrinth";
}

function parseDiscoverSource(value?: string | null): DiscoverSource | null {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (normalized === "modrinth" || normalized === "curseforge" || normalized === "github") {
    return normalized;
  }
  return null;
}

function inferNoticeTone(message?: string | null): "success" | "warning" | "error" {
  const lower = String(message ?? "").trim().toLowerCase();
  if (!lower) return "success";
  if (
    lower.includes("error") ||
    lower.includes("failed") ||
    lower.includes("fatal") ||
    lower.includes("crash")
  ) {
    return "error";
  }
  if (
    lower.includes("warning") ||
    lower.includes("blocked") ||
    lower.includes("missing") ||
    lower.includes("reverted") ||
    lower.includes("not available") ||
    lower.includes("no provider") ||
    lower.includes("resolve local") ||
    lower.includes("could not") ||
    lower.includes("pending verification") ||
    lower.includes("unverified")
  ) {
    return "warning";
  }
  return "success";
}

function parseCurseforgeProjectId(raw?: string | null): string | null {
  const value = String(raw ?? "").trim();
  if (!value) return null;
  if (/^\d+$/.test(value)) return value;
  const prefixed = value.match(/^cf:(\d+)$/i);
  if (prefixed?.[1]) return prefixed[1];
  const trailing = value.match(/(\d+)$/);
  if (trailing?.[1]) return trailing[1];
  return null;
}

function parseGithubProjectId(raw?: string | null): string | null {
  const value = String(raw ?? "").trim();
  if (!value) return null;
  let normalized = value.replace(/^gh:/i, "").replace(/^github:/i, "").trim();
  let parsedFromGithubUrl = false;
  if (/^https?:\/\//i.test(normalized)) {
    let parsedUrl: URL;
    try {
      parsedUrl = new URL(normalized);
    } catch {
      return null;
    }
    const host = parsedUrl.hostname.toLowerCase();
    if (host !== "github.com" && host !== "www.github.com") {
      return null;
    }
    parsedFromGithubUrl = true;
    normalized = parsedUrl.pathname ?? "";
  } else {
    const hostPrefixed = normalized.match(/^(?:www\.)?github\.com\/(.+)$/i);
    if (hostPrefixed?.[1]) {
      parsedFromGithubUrl = true;
      normalized = hostPrefixed[1];
    } else if (normalized.includes("://")) {
      return null;
    }
  }

  normalized = normalized
    .split(/[?#]/, 1)[0]
    ?.replace(/^\/+|\/+$/g, "")
    .trim();
  if (!normalized) return null;
  const parts = normalized.split("/").filter(Boolean);
  if (parts.length < 2 || (!parsedFromGithubUrl && parts.length !== 2)) {
    return null;
  }
  const owner = parts[0]?.trim() ?? "";
  const repo = (parts[1]?.trim() ?? "").replace(/\.git$/i, "");
  if (!/^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$/.test(owner)) return null;
  if (!/^[A-Za-z0-9._-]{1,100}$/.test(repo) || repo === "." || repo === "..") return null;
  return `${owner}/${repo}`;
}

function providerSourceRank(value?: string | null): number {
  const normalized = normalizeProviderSource(value);
  if (normalized === "modrinth") return 3;
  if (normalized === "curseforge") return 2;
  if (normalized === "github") return 1;
  return 0;
}

function providerCandidateConfidenceRank(candidate: ProviderCandidate): number {
  const confidence = String(candidate.confidence ?? "").trim().toLowerCase();
  if (confidence === "deterministic") return 5;
  if (confidence === "high") return 4;
  if (confidence === "medium") return 3;
  if (confidence === "manual") return 2;
  if (confidence === "low") return 1;
  return 0;
}

function githubManualUnverifiedHintIsPromotableCandidate(candidate: ProviderCandidate): boolean {
  const confidence = String(candidate.confidence ?? "").trim().toLowerCase();
  if (confidence !== "manual") return false;
  if (!String(candidate.version_id ?? "").trim().toLowerCase().startsWith("gh_repo_unverified")) {
    return false;
  }
  const reason = String(candidate.reason ?? "").trim().toLowerCase();
  if (!reason.includes("direct metadata repo hint")) return false;
  return (
    reason.includes("verification is unavailable") ||
    reason.includes("verification unavailable") ||
    reason.includes("temporarily unavailable") ||
    reason.includes("currently unverifiable") ||
    reason.includes("rate limit")
  );
}

function providerCandidateIsAutoActivatable(candidate: ProviderCandidate): boolean {
  const source = normalizeProviderSource(candidate.source);
  if (source !== "github") return true;
  if (!parseGithubProjectId(candidate.project_id)) return false;
  const confidence = String(candidate.confidence ?? "").trim().toLowerCase();
  return (
    confidence === "deterministic" ||
    confidence === "high" ||
    githubManualUnverifiedHintIsPromotableCandidate(candidate)
  );
}

function eventTargetsInteractiveControl(
  event: ReactMouseEvent<HTMLElement> | ReactKeyboardEvent<HTMLElement>
): boolean {
  const target = event.target as HTMLElement | null;
  if (!target) return false;
  return Boolean(
    target.closest(
      "button, input, select, textarea, a, label, [data-row-action='true']"
    )
  );
}

function installedEntryUiKey(entry: InstalledMod): string {
  const contentType = normalizeCreatorEntryType(entry.content_type);
  const source = normalizeProviderSource(entry.source);
  const projectId = String(entry.project_id ?? "").trim();
  const versionId = String(entry.version_id ?? "").trim();
  const filename = String(entry.filename ?? "").trim().toLowerCase();
  const targetScope = String(entry.target_scope ?? "instance").trim().toLowerCase();
  const targetWorlds = Array.isArray(entry.target_worlds)
    ? entry.target_worlds.map((value) => String(value ?? "").trim().toLowerCase()).filter(Boolean).sort().join(",")
    : "";
  return [contentType, source, projectId, versionId, filename, targetScope, targetWorlds].join("::");
}

function resolveGithubReadmeUrl(raw?: string | null, base?: string | null): string {
  const href = String(raw ?? "").trim();
  if (!href) return "";
  if (href.startsWith("#")) return href;
  if (/^(https?:|data:|mailto:|tel:)/i.test(href)) return href;
  if (href.startsWith("//")) return `https:${href}`;
  const baseHref = String(base ?? "").trim();
  if (!baseHref) return href;
  try {
    if (href.startsWith("/")) {
      const parsed = new URL(baseHref);
      if (parsed.hostname.toLowerCase() === "raw.githubusercontent.com") {
        const parts = parsed.pathname.split("/").filter(Boolean);
        if (parts.length >= 3) {
          const prefix = parts.slice(0, 3).join("/");
          return `https://raw.githubusercontent.com/${prefix}/${href.replace(/^\/+/, "")}`;
        }
      }
      return new URL(href, `${parsed.protocol}//${parsed.host}`).toString();
    }
    return new URL(href, baseHref).toString();
  } catch {
    return href;
  }
}

function GithubReadmeMarkdown({
  text,
  className,
  readmeHtmlUrl,
  readmeSourceUrl,
}: {
  text: string;
  className?: string;
  readmeHtmlUrl?: string | null;
  readmeSourceUrl?: string | null;
}) {
  return (
    <div className={className}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw, rehypeSanitize]}
        components={{
          a: ({ node: _node, href, ...props }) => (
            <a
              {...props}
              href={resolveGithubReadmeUrl(href, readmeHtmlUrl)}
              target="_blank"
              rel="noreferrer"
            />
          ),
          img: ({ node: _node, src, alt, ...props }) => (
            <img
              {...props}
              src={resolveGithubReadmeUrl(src, readmeSourceUrl ?? readmeHtmlUrl)}
              alt={alt ?? ""}
              loading="lazy"
              style={{ maxWidth: "100%", height: "auto", borderRadius: 8 }}
            />
          ),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}

function installedProviderCandidates(mod: InstalledMod): ProviderCandidate[] {
  const raw = Array.isArray(mod.provider_candidates) ? mod.provider_candidates : [];
  const byKey = new Map<string, ProviderCandidate>();
  const isBetter = (candidate: ProviderCandidate, existing: ProviderCandidate) => {
    const candidateActivatable = Number(providerCandidateIsAutoActivatable(candidate));
    const existingActivatable = Number(providerCandidateIsAutoActivatable(existing));
    if (candidateActivatable !== existingActivatable) {
      return candidateActivatable > existingActivatable;
    }
    const candidateConfidence = providerCandidateConfidenceRank(candidate);
    const existingConfidence = providerCandidateConfidenceRank(existing);
    if (candidateConfidence !== existingConfidence) {
      return candidateConfidence > existingConfidence;
    }
    const candidateVersion = String(candidate.version_id ?? "").trim().toLowerCase();
    const existingVersion = String(existing.version_id ?? "").trim().toLowerCase();
    const candidateReleaseVersion = Number(
      candidateVersion.startsWith("gh_release:") || candidateVersion.startsWith("cf_file:")
    );
    const existingReleaseVersion = Number(
      existingVersion.startsWith("gh_release:") || existingVersion.startsWith("cf_file:")
    );
    if (candidateReleaseVersion !== existingReleaseVersion) {
      return candidateReleaseVersion > existingReleaseVersion;
    }
    return candidateVersion.length > existingVersion.length;
  };
  const push = (candidate: ProviderCandidate) => {
    const source = String(candidate.source ?? "").trim().toLowerCase();
    const projectId = String(candidate.project_id ?? "").trim();
    if (!source || !projectId) return;
    const key = `${source}:${projectId.toLowerCase()}`;
    const existing = byKey.get(key);
    if (!existing || isBetter(candidate, existing)) {
      byKey.set(key, candidate);
    }
  };
  if (mod.project_id) {
    push({
      source: mod.source,
      project_id: mod.project_id,
      version_id: mod.version_id,
      name: mod.name,
      version_number: mod.version_number,
    });
  }
  for (const candidate of raw) push(candidate);
  const out = Array.from(byKey.values());
  out.sort((a, b) => {
    const aPriority = normalizeProviderSource(a.source) === normalizeProviderSource(mod.source) ? 0 : 1;
    const bPriority = normalizeProviderSource(b.source) === normalizeProviderSource(mod.source) ? 0 : 1;
    if (aPriority !== bPriority) return aPriority - bPriority;
    const confidenceCmp = providerCandidateConfidenceRank(b) - providerCandidateConfidenceRank(a);
    if (confidenceCmp !== 0) return confidenceCmp;
    return providerSourceLabel(a.source).localeCompare(providerSourceLabel(b.source));
  });
  return out;
}

function installedProviderBadgeCandidates(mod: InstalledMod): ProviderCandidate[] {
  const canonical = installedProviderCandidates(mod);
  const out: ProviderCandidate[] = [];
  const seenSources = new Set<string>();
  for (const candidate of canonical) {
    const source = normalizeProviderSource(candidate.source);
    if (!source || seenSources.has(source)) continue;
    seenSources.add(source);
    out.push(candidate);
  }
  return out;
}

function preferredProjectIdForProvider(mod: InstalledMod, source: string): string {
  const normalized = normalizeProviderSource(source);
  const fromCandidates = installedProviderCandidates(mod).find(
    (candidate) => normalizeProviderSource(candidate.source) === normalized
  );
  if (fromCandidates?.project_id) return fromCandidates.project_id;
  return mod.project_id;
}

function effectiveInstalledProviderSource(mod: InstalledMod): ReturnType<typeof normalizeProviderSource> {
  const active = normalizeProviderSource(mod.source);
  if (active === "modrinth" || active === "curseforge" || active === "github") return active;
  if (active !== "local") return active;
  const candidates = installedProviderCandidates(mod).filter((candidate) => {
    const source = normalizeProviderSource(candidate.source);
    if (source !== "modrinth" && source !== "curseforge" && source !== "github") {
      return false;
    }
    if (source === "github") {
      return providerCandidateIsAutoActivatable(candidate);
    }
    return true;
  });
  if (candidates.length === 0) return active;
  candidates.sort((a, b) => {
    const confidenceCmp = providerCandidateConfidenceRank(b) - providerCandidateConfidenceRank(a);
    if (confidenceCmp !== 0) return confidenceCmp;
    const sourceCmp = providerSourceRank(b.source) - providerSourceRank(a.source);
    if (sourceCmp !== 0) return sourceCmp;
    return String(a.project_id ?? "").localeCompare(String(b.project_id ?? ""));
  });
  return normalizeProviderSource(candidates[0]?.source);
}

function installedIconCacheKey(mod: InstalledMod): string {
  const source = effectiveInstalledProviderSource(mod);
  const projectId = preferredProjectIdForProvider(mod, source);
  return `${source}:${String(projectId ?? "").trim().toLowerCase()}`;
}

function creatorEntryTypeLabel(input?: string) {
  const normalized = normalizeCreatorEntryType(input);
  if (normalized === "resourcepacks") return "Resourcepacks";
  if (normalized === "shaderpacks") return "Shaderpacks";
  if (normalized === "datapacks") return "Datapacks";
  if (normalized === "modpacks") return "Modpacks";
  return "Mods";
}

function toLocalIconSrc(path?: string | null) {
  const value = String(path ?? "").trim();
  if (!value) return null;
  if (/^http:\/\//i.test(value)) return value.replace(/^http:\/\//i, "https://");
  if (/^(https?:|data:|blob:|asset:|tauri:)/i.test(value)) return value;
  try {
    return convertFileSrc(value);
  } catch {
    return value;
  }
}

const LOCAL_IMAGE_DATA_URL_CACHE = new Map<string, string>();
const LOCAL_IMAGE_DATA_URL_PENDING = new Map<string, Promise<string | null>>();

function isDirectImageSrc(value: string) {
  return /^(https?:|data:|blob:|asset:|tauri:)/i.test(value);
}

async function resolveLocalImageDataUrl(path: string): Promise<string | null> {
  const value = String(path ?? "").trim();
  if (!value) return null;
  if (isDirectImageSrc(value)) return value;
  const cached = LOCAL_IMAGE_DATA_URL_CACHE.get(value);
  if (cached) return cached;
  const inFlight = LOCAL_IMAGE_DATA_URL_PENDING.get(value);
  if (inFlight) return inFlight;
  const task = readLocalImageDataUrl({ path: value })
    .then((data) => {
      const normalized = String(data ?? "").trim();
      if (!normalized) return null;
      LOCAL_IMAGE_DATA_URL_CACHE.set(value, normalized);
      return normalized;
    })
    .catch(() => null)
    .finally(() => {
      LOCAL_IMAGE_DATA_URL_PENDING.delete(value);
    });
  LOCAL_IMAGE_DATA_URL_PENDING.set(value, task);
  return task;
}

function LocalImage({
  path,
  alt,
  fallback = null,
}: {
  path?: string | null;
  alt: string;
  fallback?: ReactNode;
}) {
  const normalizedPath = String(path ?? "").trim();
  const [src, setSrc] = useState<string | null>(() => {
    if (!normalizedPath) return null;
    if (isDirectImageSrc(normalizedPath)) return normalizedPath;
    return LOCAL_IMAGE_DATA_URL_CACHE.get(normalizedPath) ?? null;
  });

  useEffect(() => {
    let cancelled = false;
    if (!normalizedPath) {
      setSrc(null);
      return () => {
        cancelled = true;
      };
    }
    if (isDirectImageSrc(normalizedPath)) {
      setSrc(normalizedPath);
      return () => {
        cancelled = true;
      };
    }
    const cached = LOCAL_IMAGE_DATA_URL_CACHE.get(normalizedPath);
    if (cached) {
      setSrc(cached);
      return () => {
        cancelled = true;
      };
    }
    setSrc(null);
    void resolveLocalImageDataUrl(normalizedPath).then((resolved) => {
      if (!cancelled) setSrc(resolved);
    });
    return () => {
      cancelled = true;
    };
  }, [normalizedPath]);

  if (!src) return <>{fallback}</>;
  return <img src={src} alt={alt} loading="lazy" decoding="async" />;
}

function RemoteImage({
  src,
  alt,
  fallback = null,
}: {
  src?: string | null;
  alt: string;
  fallback?: ReactNode;
}) {
  const normalized = String(src ?? "").trim();
  const [loadFailed, setLoadFailed] = useState(false);
  useEffect(() => {
    setLoadFailed(false);
  }, [normalized]);
  if (!normalized || loadFailed) return <>{fallback}</>;
  return (
    <img
      src={normalized}
      alt={alt}
      loading="lazy"
      decoding="async"
      onError={() => setLoadFailed(true)}
    />
  );
}

async function openExternalLink(url: string) {
  try {
    await shellOpen(url);
    return;
  } catch {
    // Fallback for environments where shell.open is unavailable.
  }
  try {
    window.open(url, "_blank", "noopener,noreferrer");
  } catch {
    // no-op
  }
}

const FALLBACK_VERSIONS: VersionItem[] = [
  { id: "1.21.1", type: "release" },
  { id: "1.21", type: "release" },
  { id: "1.20.6", type: "release" },
  { id: "1.20.4", type: "release" },
  { id: "1.20.1", type: "release" },
  { id: "1.19.4", type: "release" },
  { id: "1.18.2", type: "release" },
  { id: "1.16.5", type: "release" },
  { id: "1.12.2", type: "release" },
  { id: "1.7.10", type: "release" },
];

function majorMinorGroup(id: string) {
  const m = id.match(/^(\d+)\.(\d+)/);
  if (!m) return "Other";
  return `${m[1]}.${m[2]}`;
}

function parseReleaseParts(id: string) {
  if (!/^\d+(?:\.\d+){1,3}$/.test(id)) return null;
  return id.split(".").map((n) => parseInt(n, 10));
}

function sameRunningInstances(a: RunningInstance[], b: RunningInstance[]) {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    const left = a[i];
    const right = b[i];
    if (
      left.launch_id !== right.launch_id ||
      left.instance_id !== right.instance_id ||
      left.instance_name !== right.instance_name ||
      left.method !== right.method ||
      Boolean(left.isolated) !== Boolean(right.isolated) ||
      left.pid !== right.pid ||
      left.started_at !== right.started_at
    ) {
      return false;
    }
  }
  return true;
}

function normalizeRunningInstancesPayload(input: unknown): RunningInstance[] {
  if (!Array.isArray(input)) return [];
  return input.filter((row): row is RunningInstance => {
    if (!row || typeof row !== "object") return false;
    const item = row as Record<string, unknown>;
    return (
      typeof item.launch_id === "string" &&
      typeof item.instance_id === "string" &&
      typeof item.instance_name === "string" &&
      typeof item.method === "string" &&
      typeof item.started_at === "string" &&
      (typeof item.pid === "number" || typeof item.pid === "string")
    );
  });
}

function compareReleaseIdDesc(a: string, b: string) {
  const pa = parseReleaseParts(a);
  const pb = parseReleaseParts(b);
  if (pa && pb) {
    const len = Math.max(pa.length, pb.length);
    for (let i = 0; i < len; i += 1) {
      const da = pa[i] ?? 0;
      const db = pb[i] ?? 0;
      if (da !== db) return db - da;
    }
    return 0;
  }
  return b.localeCompare(a, undefined, { numeric: true, sensitivity: "base" });
}

function toTimestamp(input?: string) {
  if (!input) return Number.NaN;
  const ts = Date.parse(input);
  return Number.isFinite(ts) ? ts : Number.NaN;
}

function normalizeUpdateCheckCadence(input?: string | null): SchedulerCadence {
  const value = String(input ?? "").trim().toLowerCase();
  if (value === "off" || value === "disabled") return "off";
  if (value === "hourly" || value === "1h") return "hourly";
  if (value === "every_3_hours" || value === "3h") return "every_3_hours";
  if (value === "every_6_hours" || value === "6h") return "every_6_hours";
  if (value === "every_12_hours" || value === "12h") return "every_12_hours";
  if (value === "weekly") return "weekly";
  return "daily";
}

function normalizeUpdateAutoApplyMode(input?: string | null): SchedulerAutoApplyMode {
  const value = String(input ?? "").trim().toLowerCase();
  if (value === "opt_in_instances" || value === "opt-in" || value === "instance_opt_in") {
    return "opt_in_instances";
  }
  if (value === "all_instances" || value === "all") return "all_instances";
  return "never";
}

function normalizeUpdateApplyScope(input?: string | null): SchedulerApplyScope {
  const value = String(input ?? "").trim().toLowerCase();
  if (value === "scheduled_and_manual" || value === "scheduled+manual" || value === "scheduled_and_check_now") {
    return "scheduled_and_manual";
  }
  return "scheduled_only";
}

function updateCadenceLabel(cadence: SchedulerCadence): string {
  switch (cadence) {
    case "off":
      return "Disabled";
    case "hourly":
      return "Every hour";
    case "every_3_hours":
      return "Every 3 hours";
    case "every_6_hours":
      return "Every 6 hours";
    case "every_12_hours":
      return "Every 12 hours";
    case "weekly":
      return "Weekly";
    default:
      return "Daily";
  }
}

function updateAutoApplyModeLabel(mode: SchedulerAutoApplyMode): string {
  switch (mode) {
    case "opt_in_instances":
      return "Only chosen instances";
    case "all_instances":
      return "All instances";
    default:
      return "Do not auto-install";
  }
}

function updateApplyScopeLabel(scope: SchedulerApplyScope): string {
  return scope === "scheduled_and_manual" ? "Scheduled runs and Run check now" : "Scheduled runs only";
}

function updateCadenceIntervalMs(cadence: SchedulerCadence): number {
  switch (cadence) {
    case "hourly":
      return 60 * 60 * 1000;
    case "every_3_hours":
      return 3 * 60 * 60 * 1000;
    case "every_6_hours":
      return 6 * 60 * 60 * 1000;
    case "every_12_hours":
      return 12 * 60 * 60 * 1000;
    case "weekly":
      return 7 * 24 * 60 * 60 * 1000;
    default:
      return 24 * 60 * 60 * 1000;
  }
}

function computeNextUpdateRunAt(lastRunAtIso: string | null, cadence: SchedulerCadence): string | null {
  if (cadence === "off") return null;
  const lastMs = toTimestamp(lastRunAtIso ?? undefined);
  if (!Number.isFinite(lastMs)) return null;
  return new Date(lastMs + updateCadenceIntervalMs(cadence)).toISOString();
}

function compareVersionItems(a: VersionItem, b: VersionItem) {
  const ta = toTimestamp(a.release_time);
  const tb = toTimestamp(b.release_time);
  const aHas = Number.isFinite(ta);
  const bHas = Number.isFinite(tb);
  if (aHas && bHas && ta !== tb) return tb - ta;
  if (aHas !== bHas) return aHas ? -1 : 1;
  return compareReleaseIdDesc(a.id, b.id);
}

function groupVersions(items: VersionItem[]) {
  const map = new Map<string, VersionItem[]>();
  for (const v of items) {
    const g = majorMinorGroup(v.id);
    const arr = map.get(g) ?? [];
    arr.push(v);
    map.set(g, arr);
  }

  const keys = Array.from(map.keys()).sort((a, b) => {
    const pa = a.split(".").map((n) => parseInt(n, 10));
    const pb = b.split(".").map((n) => parseInt(n, 10));
    if ((pa[0] ?? 0) !== (pb[0] ?? 0)) return (pb[0] ?? 0) - (pa[0] ?? 0);
    return (pb[1] ?? 0) - (pa[1] ?? 0);
  });

  return keys.map((k) => ({
    group: k,
    items: [...(map.get(k) ?? [])].sort((a, b) => compareReleaseIdDesc(a.id, b.id)),
  }));
}

function groupAllVersions(items: VersionItem[]) {
  const unique = new Map<string, VersionItem>();
  for (const item of items) {
    const id = String(item.id ?? "").trim();
    if (!id || unique.has(id)) continue;
    unique.set(id, item);
  }
  const sorted = Array.from(unique.values()).sort(compareVersionItems);
  const releases = sorted.filter((v) => v.type === "release");
  const releaseGroups = groupVersions(releases);

  const releaseCandidates = sorted.filter((v) => /-rc\d+$/i.test(v.id));
  const preReleases = sorted.filter((v) => /-pre\d+$/i.test(v.id));
  const weeklySnapshots = sorted.filter((v) => /^\d{2}w\d{2}[a-z]$/i.test(v.id));
  const oldBeta = sorted.filter((v) => v.type === "old_beta");
  const oldAlpha = sorted.filter((v) => v.type === "old_alpha");

  const releaseLike = new Set([
    ...releaseCandidates.map((v) => v.id),
    ...preReleases.map((v) => v.id),
    ...weeklySnapshots.map((v) => v.id),
  ]);
  const extraSnapshots = sorted.filter(
    (v) => v.type === "snapshot" && !releaseLike.has(v.id)
  );

  const seriesKey = (id: string) => {
    const match = id.trim().match(/^(\d+\.\d+)/);
    return match?.[1] ?? "Other";
  };
  const compareSeriesDesc = (a: string, b: string) => {
    const parse = (value: string) => value.split(".").map((n) => Number.parseInt(n, 10) || 0);
    const pa = parse(a);
    const pb = parse(b);
    if ((pa[0] ?? 0) !== (pb[0] ?? 0)) return (pb[0] ?? 0) - (pa[0] ?? 0);
    return (pb[1] ?? 0) - (pa[1] ?? 0);
  };
  const groupByKey = (arr: VersionItem[], keyFn: (id: string) => string) => {
    const map = new Map<string, VersionItem[]>();
    for (const item of arr) {
      const key = keyFn(item.id);
      const current = map.get(key) ?? [];
      current.push(item);
      map.set(key, current);
    }
    return map;
  };

  const rcBySeries = groupByKey(releaseCandidates, (id) => seriesKey(id));
  const preBySeries = groupByKey(preReleases, (id) => seriesKey(id));
  const snapshotsByYear = groupByKey(weeklySnapshots, (id) => {
    const m = id.match(/^(\d{2})w\d{2}[a-z]$/i);
    if (!m) return "Other";
    const yy = Number.parseInt(m[1], 10);
    if (!Number.isFinite(yy)) return "Other";
    return String(yy >= 70 ? 1900 + yy : 2000 + yy);
  });

  const out: { group: string; items: VersionItem[] }[] = [];
  out.push(
    ...releaseGroups.map((g) => ({
      group: `Stable releases • ${g.group}`,
      items: g.items,
    }))
  );
  for (const key of Array.from(rcBySeries.keys()).sort(compareSeriesDesc)) {
    const values = (rcBySeries.get(key) ?? []).sort(compareVersionItems);
    if (!values.length) continue;
    out.push({ group: `Release candidates • ${key}`, items: values });
  }
  for (const key of Array.from(preBySeries.keys()).sort(compareSeriesDesc)) {
    const values = (preBySeries.get(key) ?? []).sort(compareVersionItems);
    if (!values.length) continue;
    out.push({ group: `Pre-releases • ${key}`, items: values });
  }
  for (const key of Array.from(snapshotsByYear.keys()).sort((a, b) => Number(b) - Number(a))) {
    const values = (snapshotsByYear.get(key) ?? []).sort(compareVersionItems);
    if (!values.length) continue;
    out.push({ group: `Snapshots • ${key}`, items: values });
  }
  if (extraSnapshots.length) out.push({ group: "Experimental / dev builds", items: extraSnapshots });
  if (oldBeta.length) out.push({ group: "Old Beta", items: oldBeta });
  if (oldAlpha.length) out.push({ group: "Old Alpha", items: oldAlpha });
  return out;
}

async function fetchOfficialManifest(): Promise<VersionItem[]> {
  const parseMcVersionsHtml = (html: string): VersionItem[] => {
    const found = new Set<string>();
    const out: VersionItem[] = [];
    const re = /data-version="([^"]+)"/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(html)) !== null) {
      const id = m[1]?.trim();
      if (!id || found.has(id)) continue;
      found.add(id);

      let type: VersionItem["type"] = "snapshot";
      if (/^b\d/i.test(id)) type = "old_beta";
      else if (/^a\d/i.test(id)) type = "old_alpha";
      else if (/^\d+(?:\.\d+){1,3}$/.test(id)) type = "release";

      out.push({ id, type });
    }
    return out;
  };

  try {
    const url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    const res = await fetch(url);
    if (!res.ok) throw new Error(`Failed to fetch versions (${res.status})`);
    const data = (await res.json()) as {
      versions: { id: string; type: VersionItem["type"]; releaseTime?: string }[];
    };
    if (!Array.isArray(data.versions) || data.versions.length < 50) {
      throw new Error("Version manifest response was unexpectedly small");
    }
    return data.versions.map((v) => ({
      id: v.id,
      type: v.type,
      release_time: v.releaseTime,
    }));
  } catch {
    const backup = await fetch("https://mcversions.net/");
    if (!backup.ok) throw new Error(`Failed to fetch backup versions (${backup.status})`);
    const html = await backup.text();
    const parsed = parseMcVersionsHtml(html);
    if (!parsed.length) throw new Error("Backup version parse returned 0 versions");
    return parsed;
  }
}


function prefersReducedMotion() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}


function formatSnapshotReason(
  reason?: string | null,
  resolveProjectLabel?: (rawId: string, sourceHint?: "modrinth" | "curseforge" | null) => string | null
) {
  const raw = String(reason ?? "").trim();
  if (!raw) return "Snapshot";
  const installPrefix = raw.match(/^before-install-(discover|modrinth|curseforge):(.*)$/i);
  if (installPrefix) {
    const sourceRaw = String(installPrefix[1] ?? "").trim().toLowerCase();
    const sourceHint =
      sourceRaw === "curseforge" ? "curseforge" : sourceRaw === "modrinth" || sourceRaw === "discover" ? "modrinth" : null;
    const subjectRaw = String(installPrefix[2] ?? "").trim();
    const subject =
      resolveProjectLabel?.(subjectRaw, sourceHint) ??
      subjectRaw;
    const pretty = subject
      .replace(/[_-]+/g, " ")
      .replace(/\s+/g, " ")
      .trim();
    return `Before installing ${pretty || "content"}`;
  }
  const parsed = raw
    .replace(/^before-install-discover:/i, "Before installing ")
    .replace(/^before-install-modrinth:/i, "Before installing ")
    .replace(/^before-install-curseforge:/i, "Before installing ")
    .replace(/^before-update-all$/i, "Before update all")
    .replace(/^before-/i, "Before ")
    .replace(/[:_/-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  return parsed ? parsed.charAt(0).toUpperCase() + parsed.slice(1) : "Snapshot";
}

function formatSnapshotOptionLabel(
  snapshot: SnapshotMeta,
  resolveProjectLabel?: (rawId: string, sourceHint?: "modrinth" | "curseforge" | null) => string | null
) {
  const reason = formatSnapshotReason(snapshot.reason, resolveProjectLabel);
  const created = formatDateTime(snapshot.created_at, "Unknown time");
  if (created === "Unknown time") {
    return `${reason} • ${snapshot.id}`;
  }
  return `${reason} • ${created}`;
}

function launchCompatibilityFingerprint(report: LaunchCompatibilityReport) {
  const parts = [...(report.items ?? [])]
    .map((item) => {
      const code = String(item.code ?? "").trim();
      const severity = String(item.severity ?? "").trim().toLowerCase();
      const blocking = item.blocking ? "1" : "0";
      const message = String(item.message ?? "").trim().toLowerCase();
      return `${code}|${severity}|${blocking}|${message}`;
    })
    .filter(Boolean)
    .sort();
  return `${report.status}|${report.blocking_count}|${report.warning_count}|${parts.join("||")}`;
}

function isMacDesktopPlatform() {
  const ua = `${navigator.userAgent || ""} ${navigator.platform || ""}`.toLowerCase();
  return ua.includes("mac");
}

function micPermissionNeedsAction(item?: LaunchPermissionChecklistItem | null) {
  if (!item?.required) return false;
  const status = String(item.status ?? "").trim().toLowerCase();
  return ["denied", "not_determined"].includes(status);
}

function micPermissionCheckUnavailable(item?: LaunchPermissionChecklistItem | null) {
  if (!item?.required) return false;
  const status = String(item.status ?? "").trim().toLowerCase();
  return ["unavailable", "unknown"].includes(status);
}

function permissionStatusLabel(status: string) {
  switch ((status || "").trim().toLowerCase()) {
    case "granted":
      return "Allowed";
    case "denied":
      return "Denied";
    case "not_determined":
      return "Needs setup";
    case "not_required":
      return "Not required";
    case "unavailable":
      return "Check manually";
    default:
      return "Unknown";
  }
}

function permissionStatusChipClass(status: string) {
  switch ((status || "").trim().toLowerCase()) {
    case "granted":
      return "subtle";
    case "denied":
      return "danger";
    case "not_determined":
      return "danger";
    case "unavailable":
      return "";
    default:
      return "";
  }
}

function normalizeMinecraftUuid(uuid?: string | null) {
  if (!uuid) return null;
  return uuid.replace(/-/g, "").trim() || null;
}

function friendLinkDriftBadge(preview?: FriendLinkDriftPreview | null): string | null {
  if (!preview || preview.total_changes <= 0 || preview.status !== "unsynced") return null;
  const modItems = (preview.items ?? []).filter((item) => item.kind === "lock_entry");
  if (modItems.length === 0) return null;
  const added = modItems.filter((item) => item.change === "added").length;
  const removed = modItems.filter((item) => item.change === "removed").length;
  const changed = modItems.filter((item) => item.change === "changed").length;
  return `+${added} / -${removed} / ~${changed}`;
}

function friendLinkDriftBadgeTooltip(preview?: FriendLinkDriftPreview | null): string {
  if (!preview) {
    return "Unsynced counters compare friends to your instance: + items friends have that you do not, - items you have that friends do not, ~ same item changed version/settings.";
  }
  const modItems = (preview.items ?? []).filter((item) => item.kind === "lock_entry");
  const cfgItems = (preview.items ?? []).filter((item) => item.kind !== "lock_entry");
  const modAdded = modItems.filter((item) => item.change === "added").length;
  const modRemoved = modItems.filter((item) => item.change === "removed").length;
  const modChanged = modItems.filter((item) => item.change === "changed").length;
  if (modItems.length === 0) {
    return `Mods are currently aligned. There are ${cfgItems.length} config drift item${cfgItems.length === 1 ? "" : "s"}.`;
  }
  return `Unsynced mod drift vs your instance: +${modAdded} mods friends have that you do not, -${modRemoved} mods you have that friends do not, ~${modChanged} mods changed version/settings.${cfgItems.length > 0 ? ` (${cfgItems.length} config drift item${cfgItems.length === 1 ? "" : "s"} also present.)` : ""}`;
}

function friendLinkDriftSignature(preview?: FriendLinkDriftPreview | null): string {
  if (!preview) return "";
  const rows = (preview.items ?? [])
    .map((item) => `${item.kind}|${item.key}|${item.change}|${item.peer_id}`)
    .sort();
  return `${preview.status}|${preview.added}|${preview.removed}|${preview.changed}|${rows.join("||")}`;
}

const FRIEND_SYNC_PREFS_KEY = "mpm.friend_link.sync_prefs.v1";
const DEFAULT_FRIEND_SYNC_PREFS: FriendSyncPrefs = {
  policy: "ask",
  snoozed_until: 0,
};
const FRIEND_LINK_AUTOSYNC_INTERVAL_MS = 12000;

function normalizeFriendSyncPolicy(raw: unknown): FriendSyncPolicy {
  const policy = String(raw ?? "").trim();
  if (policy === "manual" || policy === "ask" || policy === "auto_metadata" || policy === "auto_all") {
    return policy;
  }
  return "ask";
}

function readFriendSyncPrefs(instanceId: string): FriendSyncPrefs {
  if (typeof window === "undefined") return DEFAULT_FRIEND_SYNC_PREFS;
  try {
    const raw = localStorage.getItem(FRIEND_SYNC_PREFS_KEY);
    if (!raw) return DEFAULT_FRIEND_SYNC_PREFS;
    const parsed = JSON.parse(raw) as Record<string, any>;
    const row = parsed?.[instanceId] as Record<string, any> | undefined;
    return {
      policy: normalizeFriendSyncPolicy(row?.policy),
      snoozed_until: Number(row?.snoozed_until ?? 0) || 0,
    };
  } catch {
    return DEFAULT_FRIEND_SYNC_PREFS;
  }
}

const skinHeadRenderCache = new Map<string, string>();
const skinHeadRenderPending = new Map<string, Promise<string | null>>();
const skin3dThumbCache = new Map<string, string>();
const skin3dThumbPending = new Map<string, Promise<string | null>>();

const DEFAULT_HOME_LAYOUT: HomeWidgetLayoutItem[] = [
  { id: "action_required", visible: true, pinned: true, column: "main", order: 0 },
  { id: "launchpad", visible: true, pinned: true, column: "main", order: 1 },
  { id: "recent_activity", visible: true, pinned: false, column: "main", order: 2 },
  { id: "performance_pulse", visible: true, pinned: false, column: "main", order: 3 },
  { id: "friend_link", visible: true, pinned: false, column: "side", order: 0 },
  { id: "maintenance", visible: true, pinned: true, column: "side", order: 1 },
  { id: "running_sessions", visible: true, pinned: false, column: "side", order: 2 },
  { id: "recent_instances", visible: true, pinned: false, column: "side", order: 3 },
];

function defaultInstanceContentFilters(): InstanceContentFilters {
  return {
    query: "",
    state: "all",
    source: "all",
    missing: "all",
    warningsOnly: false,
  };
}

function sameInstanceContentFilters(a: InstanceContentFilters, b: InstanceContentFilters) {
  return (
    a.query === b.query &&
    a.state === b.state &&
    a.source === b.source &&
    a.missing === b.missing &&
    a.warningsOnly === b.warningsOnly
  );
}

function isSourceFilterValue(value: string): value is InstanceContentFilters["source"] {
  return (
    value === "all" ||
    value === "modrinth" ||
    value === "curseforge" ||
    value === "github" ||
    value === "local" ||
    value === "other"
  );
}

function normalizeDiscoverProviderSources(values: readonly string[]): DiscoverProviderSource[] {
  const seen = new Set<DiscoverProviderSource>();
  const next: DiscoverProviderSource[] = [];
  for (const raw of values) {
    const value = String(raw ?? "").trim().toLowerCase();
    if (value !== "modrinth" && value !== "curseforge" && value !== "github") continue;
    const source = value as DiscoverProviderSource;
    if (seen.has(source)) continue;
    seen.add(source);
    next.push(source);
  }
  return next;
}

function discoverRequestSources(source: DiscoverSource): DiscoverProviderSource[] {
  if (source === "all") return [...DISCOVER_PROVIDER_SOURCES];
  return normalizeDiscoverProviderSources([source]);
}

function readSettingsMode(): SettingsMode {
  if (typeof window === "undefined") return "basic";
  try {
    const raw = localStorage.getItem(SETTINGS_MODE_KEY);
    return raw === "advanced" ? "advanced" : "basic";
  } catch {
    return "basic";
  }
}

function readInstanceSettingsMode(): SettingsMode {
  if (typeof window === "undefined") return "basic";
  try {
    const raw = localStorage.getItem(INSTANCE_SETTINGS_MODE_KEY);
    return raw === "advanced" ? "advanced" : "basic";
  } catch {
    return "basic";
  }
}

function readSupportBundleRawDefault(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return localStorage.getItem(SUPPORT_BUNDLE_RAW_DEFAULT_KEY) === "1";
  } catch {
    return false;
  }
}

function readInstalledIconCache(): Record<string, string> {
  if (typeof window === "undefined") return {};
  try {
    const raw = localStorage.getItem(INSTALLED_ICON_CACHE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return {};
    const out: Record<string, string> = {};
    for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
      const cacheKey = String(key ?? "").trim();
      const cacheValue = String(value ?? "").trim();
      if (!cacheKey || !cacheValue) continue;
      out[cacheKey] = cacheValue;
    }
    return out;
  } catch {
    return {};
  }
}

function readHomeLayout(): HomeWidgetLayoutItem[] {
  if (typeof window === "undefined") return DEFAULT_HOME_LAYOUT;
  try {
    const raw = localStorage.getItem(HOME_LAYOUT_KEY);
    if (!raw) return DEFAULT_HOME_LAYOUT;
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return DEFAULT_HOME_LAYOUT;
    const byId = new Map<HomeWidgetId, HomeWidgetLayoutItem>();
    for (const row of parsed) {
      if (!row || typeof row !== "object") continue;
      const id = String((row as any).id ?? "") as HomeWidgetId;
      const base = DEFAULT_HOME_LAYOUT.find((item) => item.id === id);
      if (!base) continue;
      byId.set(id, {
        id,
        visible: (row as any).visible !== false,
        pinned: Boolean((row as any).pinned),
        column: (row as any).column === "side" ? "side" : "main",
        order: Number.isFinite(Number((row as any).order)) ? Number((row as any).order) : base.order,
      });
    }
    return DEFAULT_HOME_LAYOUT.map((item) => byId.get(item.id) ?? item);
  } catch {
    return DEFAULT_HOME_LAYOUT;
  }
}

function readInstanceContentFiltersState(): Record<string, InstanceContentFilters> {
  if (typeof window === "undefined") return {};
  try {
    const raw = localStorage.getItem(INSTANCE_CONTENT_FILTERS_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return {};
    const out: Record<string, InstanceContentFilters> = {};
    for (const [key, value] of Object.entries(parsed as Record<string, any>)) {
      if (!value || typeof value !== "object") continue;
      const source = String(value.source ?? "all").trim().toLowerCase();
      out[key] = {
        query: String(value.query ?? ""),
        state: value.state === "enabled" || value.state === "disabled" ? value.state : "all",
        source: isSourceFilterValue(source) ? source : "all",
        missing: value.missing === "missing" || value.missing === "present" ? value.missing : "all",
        warningsOnly: Boolean(value.warningsOnly),
      };
    }
    return out;
  } catch {
    return {};
  }
}

function scheduledUpdateWorkerLimit(instanceCount: number): number {
  if (instanceCount <= 1) return Math.max(1, instanceCount);
  const cpuRaw =
    typeof navigator !== "undefined" && Number.isFinite(Number(navigator.hardwareConcurrency))
      ? Number(navigator.hardwareConcurrency)
      : 8;
  const cpuCap = Math.max(2, Math.min(16, Math.floor(cpuRaw)));
  const byWorkload =
    instanceCount >= 24
      ? 10
      : instanceCount >= 12
        ? 9
        : instanceCount >= 6
          ? 8
          : 6;
  let cap = Math.min(byWorkload, cpuCap, instanceCount);
  if (typeof window !== "undefined") {
    try {
      const raw = Number.parseInt(localStorage.getItem(SCHEDULED_UPDATE_WORKERS_MAX_KEY) ?? "", 10);
      if (Number.isFinite(raw)) {
        cap = Math.max(1, Math.min(instanceCount, Math.min(16, raw)));
      }
    } catch {
      // ignore override read failures
    }
  }
  return Math.max(1, cap);
}

function skinThumbSourceCandidates(input?: string | null): string[] {
  const src = String(input ?? "").trim();
  if (!src) return [];
  const out = [src];
  const minotar = src.match(/minotar\.net\/skin\/([^/?#]+)/i);
  if (minotar?.[1]) out.push(`https://mc-heads.net/skin/${encodeURIComponent(minotar[1])}`);
  const mcHeads = src.match(/mc-heads\.net\/skin\/([^/?#]+)/i);
  if (mcHeads?.[1]) out.push(`https://minotar.net/skin/${encodeURIComponent(mcHeads[1])}`);
  return [...new Set(out)];
}

function readCachedAccountDiagnostics(): AccountDiagnostics | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = localStorage.getItem(ACCOUNT_DIAGNOSTICS_CACHE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return null;
    const candidate = parsed as Partial<AccountDiagnostics>;
    if (
      typeof candidate.status !== "string" ||
      typeof candidate.last_refreshed_at !== "string" ||
      typeof candidate.token_exchange_status !== "string" ||
      typeof candidate.client_id_source !== "string" ||
      !Array.isArray(candidate.skins) ||
      !Array.isArray(candidate.capes)
    ) {
      return null;
    }
    return candidate as AccountDiagnostics;
  } catch {
    return null;
  }
}

async function renderMinecraftHeadFromSkin(
  skinUrl?: string | null,
  size = 128
): Promise<string | null> {
  const src = skinUrl?.trim();
  if (!src || typeof window === "undefined") return null;
  const cacheKey = `${size}:${src}`;
  const cached = skinHeadRenderCache.get(cacheKey);
  if (cached) return cached;
  const pending = skinHeadRenderPending.get(cacheKey);
  if (pending) return pending;

  const task = new Promise<string | null>((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.referrerPolicy = "no-referrer";
    img.decoding = "async";
    let done = false;
    const finish = (value: string | null) => {
      if (done) return;
      done = true;
      window.clearTimeout(timeoutHandle);
      resolve(value);
    };
    const timeoutHandle = window.setTimeout(() => {
      finish(null);
    }, SKIN_IMAGE_FETCH_TIMEOUT_MS);
    img.onload = () => {
      try {
        if (img.naturalWidth < 64 || img.naturalHeight < 32) {
          finish(null);
          return;
        }
        const headCanvas = document.createElement("canvas");
        headCanvas.width = 8;
        headCanvas.height = 8;
        const headCtx = headCanvas.getContext("2d");
        if (!headCtx) {
          finish(null);
          return;
        }
        headCtx.imageSmoothingEnabled = false;
        // Base face + hat overlay from standard skin layout.
        headCtx.drawImage(img, 8, 8, 8, 8, 0, 0, 8, 8);
        headCtx.drawImage(img, 40, 8, 8, 8, 0, 0, 8, 8);

        const out = document.createElement("canvas");
        out.width = size;
        out.height = size;
        const outCtx = out.getContext("2d");
        if (!outCtx) {
          finish(null);
          return;
        }
        outCtx.imageSmoothingEnabled = false;
        outCtx.drawImage(headCanvas, 0, 0, size, size);
        const dataUrl = out.toDataURL("image/png");
        skinHeadRenderCache.set(cacheKey, dataUrl);
        if (skinHeadRenderCache.size > SKIN_HEAD_CACHE_MAX) {
          const oldest = skinHeadRenderCache.keys().next().value as string | undefined;
          if (oldest) skinHeadRenderCache.delete(oldest);
        }
        finish(dataUrl);
      } catch {
        finish(null);
      }
    };
    img.onerror = () => finish(null);
    img.src = src;
  });

  skinHeadRenderPending.set(cacheKey, task);
  return task.finally(() => {
    skinHeadRenderPending.delete(cacheKey);
  });
}

async function withTimeout<T>(task: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    let settled = false;
    const timer = window.setTimeout(() => {
      if (settled) return;
      settled = true;
      reject(new Error("timeout"));
    }, ms);
    task
      .then((value) => {
        if (settled) return;
        settled = true;
        window.clearTimeout(timer);
        resolve(value);
      })
      .catch((error) => {
        if (settled) return;
        settled = true;
        window.clearTimeout(timer);
        reject(error);
      });
  });
}

async function renderMinecraftSkinThumb3d(args: {
  skinUrl?: string | null;
  view: "front" | "back";
  size?: number;
  capeUrl?: string | null;
}): Promise<string | null> {
  const src = String(args.skinUrl ?? "").trim();
  if (!src || typeof window === "undefined") return null;
  const size = Math.max(96, Math.round(args.size ?? SKIN_THUMB_3D_SIZE));
  const view = args.view;
  const cape = String(args.capeUrl ?? "").trim();
  const cacheKey = `${SKIN_THUMB_FRAMING_VERSION}:${size}:${view}:${src}:${cape}`;
  const cached = skin3dThumbCache.get(cacheKey);
  if (cached) return cached;
  const pending = skin3dThumbPending.get(cacheKey);
  if (pending) return pending;

  const task = (async () => {
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    let viewer: SkinViewer | null = null;
    try {
      viewer = new SkinViewer({
        canvas,
        width: size,
        height: size,
        zoom: 0.82,
        fov: 38,
      });
      const renderer = (viewer as unknown as {
        renderer?: { setPixelRatio?: (ratio: number) => void };
      }).renderer;
      renderer?.setPixelRatio?.(1);
      viewer.background = null;
      viewer.globalLight.intensity = 1.18;
      viewer.cameraLight.intensity = 1.06;
      viewer.controls.enabled = false;
      viewer.playerWrapper.position.y = 0.18;
      viewer.playerWrapper.rotation.y = view === "back" ? Math.PI + 0.42 : -0.42;
      viewer.controls.target.set(0, 9.8, 0);
      viewer.controls.update();
      await withTimeout(viewer.loadSkin(src, { model: "auto-detect" }), SKIN_VIEWER_LOAD_TIMEOUT_MS);
      if (cape) {
        await withTimeout(
          viewer.loadCape(cape, { backEquipment: "cape" }),
          SKIN_VIEWER_LOAD_TIMEOUT_MS
        ).catch(() => null);
      }
      await new Promise<void>((resolve) =>
        window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve()))
      );
      viewer.render();
      const dataUrl = canvas.toDataURL("image/png");
      skin3dThumbCache.set(cacheKey, dataUrl);
      if (skin3dThumbCache.size > 240) {
        const oldest = skin3dThumbCache.keys().next().value as string | undefined;
        if (oldest) skin3dThumbCache.delete(oldest);
      }
      return dataUrl;
    } catch {
      return null;
    } finally {
      viewer?.dispose();
    }
  })();

  skin3dThumbPending.set(cacheKey, task);
  return task.finally(() => {
    skin3dThumbPending.delete(cacheKey);
  });
}

function clampNumber(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

function normalizeTimeOfDay(input: number) {
  if (!Number.isFinite(input)) return 14;
  let value = input % 24;
  if (value < 0) value += 24;
  return value;
}

function minecraftAvatarSources(uuid?: string | null) {
  const id = normalizeMinecraftUuid(uuid);
  const out: string[] = [];
  if (id) {
    out.push(`https://mc-heads.net/avatar/${id}/128`);
    out.push(`https://crafatar.com/avatars/${id}?size=128&overlay=true&default=MHF_Steve`);
    out.push(`https://visage.surgeplay.com/face/128/${id}`);
    out.push(`https://minotar.net/avatar/${id}/128`);
  }
  return out;
}

const DEFAULT_SKIN_LIBRARY: AccountSkinOption[] = [
  {
    id: "default:steve",
    label: "Steve",
    skin_url: "https://mc-heads.net/skin/Steve",
    variant: "classic",
    preview_url: "https://mc-heads.net/body/Steve/right",
    group: "default",
    origin: "default",
  },
  {
    id: "default:alex",
    label: "Alex",
    skin_url: "https://mc-heads.net/skin/Alex",
    variant: "slim",
    preview_url: "https://mc-heads.net/body/Alex/right",
    group: "default",
    origin: "default",
  },
  {
    id: "default:ari",
    label: "Ari",
    skin_url: "https://mc-heads.net/skin/Ari",
    preview_url: "https://mc-heads.net/body/Ari/right",
    group: "default",
    origin: "default",
  },
  {
    id: "default:efe",
    label: "Efe",
    skin_url: "https://mc-heads.net/skin/Efe",
    preview_url: "https://mc-heads.net/body/Efe/right",
    group: "default",
    origin: "default",
  },
  {
    id: "default:kai",
    label: "Kai",
    skin_url: "https://mc-heads.net/skin/Kai",
    preview_url: "https://mc-heads.net/body/Kai/right",
    group: "default",
    origin: "default",
  },
  {
    id: "default:noor",
    label: "Noor",
    skin_url: "https://mc-heads.net/skin/Noor",
    preview_url: "https://mc-heads.net/body/Noor/right",
    group: "default",
    origin: "default",
  },
  {
    id: "default:makena",
    label: "Makena",
    skin_url: "https://mc-heads.net/skin/Makena",
    preview_url: "https://mc-heads.net/body/Makena/right",
    group: "default",
    origin: "default",
  },
  {
    id: "default:sunny",
    label: "Sunny",
    skin_url: "https://mc-heads.net/skin/Sunny",
    preview_url: "https://mc-heads.net/body/Sunny/right",
    group: "default",
    origin: "default",
  },
  {
    id: "default:zuri",
    label: "Zuri",
    skin_url: "https://mc-heads.net/skin/Zuri",
    preview_url: "https://mc-heads.net/body/Zuri/right",
    group: "default",
    origin: "default",
  },
];

function basenameWithoutExt(input: string) {
  const trimmed = String(input ?? "").trim();
  if (!trimmed) return "Custom skin";
  const parts = trimmed.split(/[\\/]/).filter(Boolean);
  const file = parts[parts.length - 1] ?? trimmed;
  return file.replace(/\.[^.]+$/, "") || "Custom skin";
}

function toReadableBody(markdown?: string | null) {
  if (!markdown) return "";
  return markdown
    .replace(/\r\n/g, "\n")
    .replace(/^#{1,6}\s*/gm, "")
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1 ($2)")
    .replace(/`{1,3}/g, "")
    .trim();
}

function MarkdownBlock({ text, className }: { text: string; className?: string }) {
  return (
    <div className={className}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ node: _node, ...props }) => <a {...props} target="_blank" rel="noreferrer" />,
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}

function RichTextBlock({ text, className }: { text: string; className?: string }) {
  return (
    <div className={className}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw, rehypeSanitize]}
        components={{
          a: ({ node: _node, ...props }) => <a {...props} target="_blank" rel="noreferrer" />,
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}

const DISCOVER_SORT_OPTIONS: { value: ModrinthIndex; label: string }[] = [
  { value: "relevance", label: "Relevance" },
  { value: "downloads", label: "Downloads" },
  { value: "follows", label: "Followers" },
  { value: "newest", label: "Newest" },
  { value: "updated", label: "Recently updated" },
];

const DISCOVER_VIEW_OPTIONS: { value: string; label: string }[] = [
  { value: "10", label: "10" },
  { value: "20", label: "20" },
  { value: "30", label: "30" },
  { value: "50", label: "50" },
];

const DISCOVER_PROVIDER_SOURCES: DiscoverProviderSource[] = ["modrinth", "curseforge", "github"];

const DISCOVER_SOURCE_OPTIONS: { value: DiscoverProviderSource; label: string }[] = [
  { value: "modrinth", label: "Modrinth" },
  { value: "curseforge", label: "CurseForge" },
  { value: "github", label: "GitHub" },
];

const DISCOVER_SOURCE_GROUPS: CatGroup[] = [
  {
    group: "Sources",
    items: DISCOVER_SOURCE_OPTIONS.map((option) => ({ id: option.value, label: option.label })),
  },
];

const DISCOVER_CONTENT_OPTIONS: { value: DiscoverContentType; label: string }[] = [
  { value: "mods", label: "Mods" },
  { value: "shaderpacks", label: "Shaderpacks" },
  { value: "resourcepacks", label: "Resourcepacks" },
  { value: "datapacks", label: "Datapacks" },
  { value: "modpacks", label: "Modpacks" },
];

const DISCOVER_LOADER_GROUPS: CatGroup[] = [
  {
    group: "",
    items: [
      { id: "fabric", label: "Fabric" },
      { id: "forge", label: "Forge" },
      { id: "quilt", label: "Quilt" },
      { id: "neoforge", label: "NeoForge" },
    ],
  },
];

const ACCENT_OPTIONS: { value: AccentPreset; label: string }[] = [
  { value: "neutral", label: "Neutral" },
  { value: "blue", label: "Blue" },
  { value: "emerald", label: "Emerald" },
  { value: "amber", label: "Amber" },
  { value: "rose", label: "Rose" },
  { value: "violet", label: "Violet" },
  { value: "teal", label: "Teal" },
];

const ACCENT_STRENGTH_OPTIONS: { value: AccentStrength; label: string }[] = [
  { value: "subtle", label: "Subtle" },
  { value: "normal", label: "Normal" },
  { value: "vivid", label: "Vivid" },
  { value: "max", label: "Max" },
];

const MOTION_OPTIONS: { value: MotionPreset; label: string }[] = [
  { value: "calm", label: "Calm" },
  { value: "standard", label: "Standard" },
  { value: "expressive", label: "Expressive" },
];

const MOTION_PROFILE_DETAILS: Record<
  MotionPreset,
  { label: string; summary: string; traits: string[] }
> = {
  calm: {
    label: "Quiet workspace",
    summary: "Gentler hover states and softer panel movement.",
    traits: ["Soft lift", "Slow icons", "Low contrast motion"],
  },
  standard: {
    label: "Balanced motion",
    summary: "A restrained default with clear feedback and quick transitions.",
    traits: ["Balanced lift", "Clear panel motion", "Responsive nav"],
  },
  expressive: {
    label: "Sharper feedback",
    summary: "More pronounced lift, panel reveals, and icon motion without going noisy.",
    traits: ["Stronger lift", "Livelier panels", "Bolder nav motion"],
  },
};

const DENSITY_OPTIONS: { value: DensityPreset; label: string }[] = [
  { value: "comfortable", label: "Comfortable" },
  { value: "compact", label: "Compact" },
];

const UPDATE_CADENCE_OPTIONS: { value: SchedulerCadence; label: string }[] = [
  { value: "off", label: "Disabled" },
  { value: "hourly", label: "Every hour" },
  { value: "every_3_hours", label: "Every 3 hours" },
  { value: "every_6_hours", label: "Every 6 hours" },
  { value: "every_12_hours", label: "Every 12 hours" },
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
];

const UPDATE_AUTO_APPLY_MODE_OPTIONS: { value: SchedulerAutoApplyMode; label: string }[] = [
  { value: "never", label: "Do not install automatically" },
  { value: "opt_in_instances", label: "Only instances you marked for auto-install" },
  { value: "all_instances", label: "Install updates for every instance" },
];

const UPDATE_APPLY_SCOPE_OPTIONS: { value: SchedulerApplyScope; label: string }[] = [
  { value: "scheduled_only", label: "Only during scheduled runs" },
  { value: "scheduled_and_manual", label: "During scheduled runs and Run check now" },
];

const UPDATE_CONTENT_TYPE_OPTIONS: { value: UpdatableContentType; label: string }[] = [
  { value: "mods", label: "Mods" },
  { value: "resourcepacks", label: "Resourcepacks" },
  { value: "datapacks", label: "Datapacks" },
  { value: "shaderpacks", label: "Shaders" },
];

const ALL_UPDATABLE_CONTENT_TYPES: UpdatableContentType[] = UPDATE_CONTENT_TYPE_OPTIONS.map(
  (item) => item.value
);

const UPDATE_CONTENT_TYPE_GROUPS: { group: string; items: { id: string; label: string }[] }[] = [
  {
    group: "Update content types",
    items: UPDATE_CONTENT_TYPE_OPTIONS.map((item) => ({
      id: item.value,
      label: item.label,
    })),
  },
];

const WORLD_BACKUP_INTERVAL_OPTIONS: { value: string; label: string }[] = [
  { value: "5", label: "Every 5 minutes" },
  { value: "10", label: "Every 10 minutes" },
  { value: "15", label: "Every 15 minutes" },
];

const WORLD_BACKUP_RETENTION_OPTIONS: { value: string; label: string }[] = [
  { value: "1", label: "Keep 1 backup per world" },
  { value: "2", label: "Keep 2 backups per world" },
];

const SNAPSHOT_RETENTION_OPTIONS: { value: string; label: string }[] = [
  { value: "3", label: "Keep 3 snapshots" },
  { value: "5", label: "Keep 5 snapshots" },
  { value: "10", label: "Keep 10 snapshots" },
  { value: "20", label: "Keep 20 snapshots" },
];

const SNAPSHOT_MAX_AGE_OPTIONS: { value: string; label: string }[] = [
  { value: "7", label: "Delete after 7 days" },
  { value: "14", label: "Delete after 14 days" },
  { value: "30", label: "Delete after 30 days" },
  { value: "60", label: "Delete after 60 days" },
  { value: "90", label: "Delete after 90 days" },
];

const PROJECT_DETAIL_TABS: { value: string; label: string }[] = [
  { value: "overview", label: "Overview" },
  { value: "versions", label: "Versions" },
  { value: "changelog", label: "Changelog" },
];

const CURSEFORGE_DETAIL_TABS: { value: string; label: string }[] = [
  { value: "overview", label: "Overview" },
  { value: "files", label: "Files" },
  { value: "changelog", label: "Changelog" },
];

const GITHUB_DETAIL_TABS: { value: string; label: string }[] = [
  { value: "overview", label: "Info" },
  { value: "releases", label: "Releases" },
  { value: "readme", label: "README" },
];

function isAccentPreset(value: string | null): value is AccentPreset {
  return (
    value === "neutral" ||
    value === "blue" ||
    value === "emerald" ||
    value === "amber" ||
    value === "rose" ||
    value === "violet" ||
    value === "teal"
  );
}

function isAccentStrength(value: string | null): value is AccentStrength {
  return value === "subtle" || value === "normal" || value === "vivid" || value === "max";
}

function isMotionPreset(value: string | null): value is MotionPreset {
  return value === "calm" || value === "standard" || value === "expressive";
}

function isDensityPreset(value: string | null): value is DensityPreset {
  return value === "comfortable" || value === "compact";
}

type UiSettingsSnapshot = {
  theme: "dark" | "light";
  accentPreset: AccentPreset;
  accentStrength: AccentStrength;
  motionPreset: MotionPreset;
  densityPreset: DensityPreset;
};

const UI_SETTINGS_STORAGE_KEY = "mpm.ui.settings.v2";

function defaultUiTheme(): "dark" | "light" {
  if (typeof window === "undefined") return "dark";
  try {
    return window.matchMedia?.("(prefers-color-scheme: light)")?.matches ? "light" : "dark";
  } catch {
    return "dark";
  }
}

function defaultUiSettingsSnapshot(): UiSettingsSnapshot {
  return {
    theme: defaultUiTheme(),
    accentPreset: "neutral",
    accentStrength: "normal",
    motionPreset: "standard",
    densityPreset: "comfortable",
  };
}

function readUiSettingsSnapshot(): UiSettingsSnapshot {
  const fallback = defaultUiSettingsSnapshot();
  if (typeof window === "undefined") return fallback;
  try {
    const raw = localStorage.getItem(UI_SETTINGS_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      const theme =
        parsed?.theme === "dark" || parsed?.theme === "light" ? parsed.theme : fallback.theme;
      const accentPreset = isAccentPreset(String(parsed?.accentPreset ?? ""))
        ? parsed.accentPreset
        : fallback.accentPreset;
      const accentStrength = isAccentStrength(String(parsed?.accentStrength ?? ""))
        ? parsed.accentStrength
        : fallback.accentStrength;
      const motionPreset = isMotionPreset(String(parsed?.motionPreset ?? ""))
        ? parsed.motionPreset
        : fallback.motionPreset;
      const densityPreset = isDensityPreset(String(parsed?.densityPreset ?? ""))
        ? parsed.densityPreset
        : fallback.densityPreset;
      return { theme, accentPreset, accentStrength, motionPreset, densityPreset };
    }

    const legacyTheme = localStorage.getItem("mpm.theme");
    const legacyAccent = localStorage.getItem("mpm.accent");
    const legacyAccentStrength = localStorage.getItem("mpm.accentStrength");
    const legacyMotion = localStorage.getItem("mpm.motionPreset");
    const legacyDensity = localStorage.getItem("mpm.densityPreset");
    return {
      theme:
        legacyTheme === "dark" || legacyTheme === "light" ? legacyTheme : fallback.theme,
      accentPreset: isAccentPreset(legacyAccent) ? legacyAccent : fallback.accentPreset,
      accentStrength: isAccentStrength(legacyAccentStrength)
        ? legacyAccentStrength
        : fallback.accentStrength,
      motionPreset: isMotionPreset(legacyMotion) ? legacyMotion : fallback.motionPreset,
      densityPreset: isDensityPreset(legacyDensity) ? legacyDensity : fallback.densityPreset,
    };
  } catch {
    return fallback;
  }
}

function persistUiSettingsSnapshot(next: UiSettingsSnapshot) {
  if (typeof window === "undefined") return;
  try {
    localStorage.setItem(UI_SETTINGS_STORAGE_KEY, JSON.stringify(next));
    // Keep legacy keys for backwards compatibility with older builds.
    localStorage.setItem("mpm.theme", next.theme);
    localStorage.setItem("mpm.accent", next.accentPreset);
    localStorage.setItem("mpm.accentStrength", next.accentStrength);
    localStorage.setItem("mpm.motionPreset", next.motionPreset);
    localStorage.setItem("mpm.densityPreset", next.densityPreset);
  } catch {
    // ignore storage failures
  }
}

function clearUiSettingsStorage() {
  if (typeof window === "undefined") return;
  try {
    localStorage.removeItem(UI_SETTINGS_STORAGE_KEY);
    localStorage.removeItem("mpm.theme");
    localStorage.removeItem("mpm.accent");
    localStorage.removeItem("mpm.accentStrength");
    localStorage.removeItem("mpm.motionPreset");
    localStorage.removeItem("mpm.densityPreset");
  } catch {
    // ignore storage failures
  }
}

type InstanceLogSeverity = LogSeverity;
type InstanceLogSource = InstanceLogSourceApi;
type LogViewMode = "live" | "analyze";
type QuickLogFilter = "errors" | "warnings" | "suspects" | "crashes";

type InstanceLogLine = {
  id: string;
  source: InstanceLogSource;
  severity: InstanceLogSeverity;
  timestamp: string;
  message: string;
  lineNo: number | null;
};

const LOG_MAX_LINES_OPTIONS: { value: string; label: string }[] = [
  { value: "400", label: "400" },
  { value: "1200", label: "1,200" },
  { value: "2500", label: "2,500" },
  { value: "5000", label: "5,000" },
  { value: "8000", label: "8,000" },
  { value: "12000", label: "12,000" },
];

const LOG_SEVERITY_OPTIONS: { value: "all" | InstanceLogSeverity; label: string }[] = [
  { value: "all", label: "All" },
  { value: "error", label: "Error" },
  { value: "warn", label: "Warn" },
  { value: "info", label: "Info" },
  { value: "debug", label: "Debug" },
  { value: "trace", label: "Trace" },
];

const LOG_SOURCE_OPTIONS: { value: InstanceLogSource; label: string }[] = [
  { value: "live", label: "Live log" },
  { value: "latest_launch", label: "Latest launch" },
  { value: "latest_crash", label: "Latest crash" },
];

const QUICK_LOG_FILTER_OPTIONS: { id: QuickLogFilter; label: string }[] = [
  { id: "errors", label: "Errors" },
  { id: "warnings", label: "Warnings" },
  { id: "suspects", label: "Suspects" },
  { id: "crashes", label: "Crashes" },
];

function severityLabel(level: InstanceLogSeverity) {
  if (level === "error") return "Error";
  if (level === "warn") return "Warn";
  if (level === "info") return "Info";
  if (level === "debug") return "Debug";
  return "Trace";
}

function severityShort(level: InstanceLogSeverity) {
  if (level === "error") return "ERR";
  if (level === "warn") return "WRN";
  if (level === "info") return "INF";
  if (level === "debug") return "DBG";
  return "TRC";
}

function sourceLabel(source: InstanceLogSource) {
  if (source === "live") return "Live log";
  if (source === "latest_launch") return "Latest launch";
  return "Latest crash";
}

function formatLogTimestamp(iso: string) {
  const raw = String(iso ?? "").trim();
  if (!raw) return "";
  const d = new Date(raw);
  if (Number.isFinite(d.getTime())) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  }
  return raw;
}

function toInstanceLogLine(args: {
  raw: string;
  source: InstanceLogSource;
  index: number;
  updatedAt: number;
  severity?: string | null;
  timestamp?: string | null;
  lineNo?: number | null;
}): InstanceLogLine {
  const message = String(args.raw ?? "")
    .replace(/\u0000/g, "")
    .trimEnd();
  const severityRaw = String(args.severity ?? "").trim().toLowerCase();
  const severity: InstanceLogSeverity =
    severityRaw === "error" ||
    severityRaw === "warn" ||
    severityRaw === "info" ||
    severityRaw === "debug" ||
    severityRaw === "trace"
      ? severityRaw
      : inferLogSeverity(message);
  const ts =
    String(args.timestamp ?? "").trim() ||
    extractLogTimestamp(message) ||
    new Date(args.updatedAt + args.index).toISOString();
  const numericLineNo = Number(args.lineNo);
  const lineNo = Number.isFinite(numericLineNo) && numericLineNo > 0 ? Math.floor(numericLineNo) : null;
  const idStem = message.slice(0, 80).replace(/\s+/g, " ");
  return {
    id:
      lineNo != null
        ? `${args.source}:${lineNo}`
        : `${args.source}:${args.updatedAt}:${args.index}:${idStem}`,
    source: args.source,
    severity,
    message,
    timestamp: ts,
    lineNo,
  };
}

function fallbackInstanceLogLines(args: {
  source: InstanceLogSource;
  instanceId: string;
  hasRunning: boolean;
  message?: string | null;
}): InstanceLogLine[] {
  const now = Date.now();
  const seedMessage =
    String(args.message ?? "").trim() ||
    (args.source === "live"
      ? args.hasRunning
        ? "Streaming live logs…"
        : "No live game process detected yet."
      : "No log file found for this source yet.");
  return [
    {
      id: `${args.source}:${args.instanceId}:fallback`,
      source: args.source,
      severity: "info",
      timestamp: new Date(now).toISOString(),
      message: seedMessage,
      lineNo: null,
    },
  ];
}

type LogWindowState = {
  nextBeforeLine: number | null;
  loadingOlder: boolean;
  fullyLoaded: boolean;
};

function normalizeLogLineNo(value: number | null | undefined) {
  const numeric = Number(value ?? NaN);
  if (!Number.isFinite(numeric) || numeric <= 0) return null;
  return Math.floor(numeric);
}

function logLineIdentity(line: ReadInstanceLogsLine, source: string, index: number) {
  const lineNo = normalizeLogLineNo(line.line_no);
  if (lineNo != null) return `${source}:line:${lineNo}`;
  const raw = String(line.raw ?? "").replace(/\s+/g, " ").trim().slice(0, 240);
  const timestamp = String(line.timestamp ?? "").trim();
  return `${source}:raw:${timestamp}:${raw}:${index}`;
}

function mergeReadInstanceLogPayload(args: {
  existing: ReadInstanceLogsResult | null;
  incoming: ReadInstanceLogsResult;
  mode: "replace_tail" | "prepend_older";
}): ReadInstanceLogsResult {
  const { existing, incoming, mode } = args;
  if (!existing || existing.source !== incoming.source) {
    return incoming;
  }
  if (!incoming.available) {
    return existing.available ? existing : incoming;
  }
  if (!existing.available) {
    return incoming;
  }
  if (existing.path && incoming.path && existing.path !== incoming.path) {
    return incoming;
  }
  const existingStart = normalizeLogLineNo(existing.start_line_no);
  const incomingStart = normalizeLogLineNo(incoming.start_line_no);
  // If the incoming window starts later, this is usually a narrower/newer tail request
  // (for example, changing line depth from 2500 -> 400). Replace outright so depth changes apply.
  if (mode === "replace_tail" && existingStart != null && incomingStart != null && incomingStart > existingStart) {
    return incoming;
  }
  if (
    mode === "replace_tail" &&
    normalizeLogLineNo(existing.end_line_no) != null &&
    normalizeLogLineNo(incoming.end_line_no) != null &&
    normalizeLogLineNo(incoming.end_line_no)! < normalizeLogLineNo(existing.end_line_no)!
  ) {
    return incoming;
  }

  const dedupe = new Set<string>();
  const mergedLines: ReadInstanceLogsLine[] = [];
  const push = (line: ReadInstanceLogsLine, index: number) => {
    const key = logLineIdentity(line, incoming.source, index);
    if (dedupe.has(key)) return;
    dedupe.add(key);
    mergedLines.push(line);
  };
  if (mode === "replace_tail" && incomingStart != null) {
    existing.lines.forEach((line, index) => {
      const lineNo = normalizeLogLineNo(line.line_no);
      if (lineNo != null && lineNo >= incomingStart) return;
      push(line, index);
    });
    incoming.lines.forEach((line, index) => push(line, index + existing.lines.length));
  } else if (mode === "prepend_older") {
    incoming.lines.forEach((line, index) => push(line, index));
    existing.lines.forEach((line, index) => push(line, index + incoming.lines.length));
  } else {
    existing.lines.forEach((line, index) => push(line, index));
    incoming.lines.forEach((line, index) => push(line, index + existing.lines.length));
  }

  mergedLines.sort((a, b) => {
    const lineA = normalizeLogLineNo(a.line_no);
    const lineB = normalizeLogLineNo(b.line_no);
    if (lineA != null && lineB != null) return lineA - lineB;
    if (lineA != null) return -1;
    if (lineB != null) return 1;
    return 0;
  });

  const firstLineNo = mergedLines.length > 0 ? normalizeLogLineNo(mergedLines[0].line_no) : null;
  const lastLineNo =
    mergedLines.length > 0 ? normalizeLogLineNo(mergedLines[mergedLines.length - 1].line_no) : null;
  const preservedNext =
    mode === "replace_tail" && existing.next_before_line != null
      ? normalizeLogLineNo(existing.next_before_line)
      : null;
  const nextBeforeLine = normalizeLogLineNo(incoming.next_before_line) ?? preservedNext;

  return {
    ...incoming,
    lines: mergedLines,
    returned_lines: mergedLines.length,
    total_lines: Math.max(incoming.total_lines, existing.total_lines),
    truncated: nextBeforeLine != null,
    start_line_no: firstLineNo,
    end_line_no: lastLineNo,
    next_before_line: nextBeforeLine,
    updated_at: Math.max(existing.updated_at, incoming.updated_at),
  };
}

type Cat = { id: string; label: string };
type CatGroup = { group: string; items: Cat[] };

const MOD_CATEGORY_GROUPS: CatGroup[] = [
  {
    group: "Gameplay",
    items: [
      { id: "adventure", label: "Adventure" },
      { id: "combat", label: "Combat" },
      { id: "mobs", label: "Mobs" },
      { id: "magic", label: "Magic" },
      { id: "quests", label: "Quests" },
      { id: "minigame", label: "Minigame" },
      { id: "game-mechanics", label: "Game mechanics" },
      { id: "cursed", label: "Horror / Cursed" },
    ],
  },
  {
    group: "Performance",
    items: [
      { id: "optimization", label: "Optimization" },
      { id: "utility", label: "Utility" },
      { id: "management", label: "Management" },
    ],
  },
  {
    group: "World & Content",
    items: [
      { id: "worldgen", label: "Worldgen" },
      { id: "decoration", label: "Decoration" },
      { id: "food", label: "Food" },
      { id: "economy", label: "Economy" },
      { id: "technology", label: "Technology" },
      { id: "transportation", label: "Transportation" },
      { id: "social", label: "Social" },
      { id: "multiplayer", label: "Multiplayer" },
    ],
  },
];





function SegTabs({
  tabs,
  active,
  onChange,
}: {
  tabs: { id: string; label: string; disabled?: boolean }[];
  active: string;
  onChange: (id: string) => void;
}) {
  return (
    <div className="pillRow" style={{ gap: 12 }}>
      {tabs.map((t) => {
        const cls = `pill ${t.disabled ? "disabled" : ""} ${
          active === t.id ? "active" : ""
        }`;
        return (
          <div
            key={t.id}
            className={cls}
            onClick={() => (t.disabled ? null : onChange(t.id))}
          >
            {active === t.id ? "✓ " : ""}
            {t.label}
          </div>
        );
      })}
    </div>
  );
}






function LoaderChip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button className={`btn ${active ? "primary" : ""}`} onClick={onClick}>
      {label}
    </button>
  );
}

function LazyInstalledModIcon({
  alt,
  src,
  onVisible,
  onError,
}: {
  alt: string;
  src?: string | null;
  onVisible: () => void;
  onError?: () => void;
}) {
  const holderRef = useRef<HTMLDivElement | null>(null);
  const [inView, setInView] = useState(Boolean(src));
  const [loadFailed, setLoadFailed] = useState(false);

  useEffect(() => {
    if (src) {
      setInView(true);
      return;
    }
    if (inView) return;
    const node = holderRef.current;
    if (!node) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          setInView(true);
          observer.disconnect();
          break;
        }
      },
      {
        rootMargin: "160px 0px",
      }
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [inView, src]);

  useEffect(() => {
    if (!inView || src) return;
    onVisible();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [inView, src]);
  useEffect(() => {
    setLoadFailed(false);
  }, [src]);

  return (
    <div className="instanceModIcon" ref={holderRef}>
      {src && !loadFailed ? (
        <img
          src={src}
          alt={alt}
          loading="lazy"
          decoding="async"
          onError={() => {
            setLoadFailed(true);
            onError?.();
          }}
        />
      ) : (
        <Icon name="layers" size={16} />
      )}
    </div>
  );
}

export default function App() {
  // theme
  const [uiSettingsSeed] = useState<UiSettingsSnapshot>(() => readUiSettingsSnapshot());
  const [theme, setTheme] = useState<"dark" | "light">(uiSettingsSeed.theme);
  const [accentPreset, setAccentPreset] = useState<AccentPreset>(uiSettingsSeed.accentPreset);
  const [accentStrength, setAccentStrength] = useState<AccentStrength>(
    uiSettingsSeed.accentStrength
  );
  const [motionPreset, setMotionPreset] = useState<MotionPreset>(uiSettingsSeed.motionPreset);
  const [densityPreset, setDensityPreset] = useState<DensityPreset>(uiSettingsSeed.densityPreset);
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    document.documentElement.style.colorScheme = theme;
  }, [theme]);
  useEffect(() => {
    document.documentElement.setAttribute("data-accent", accentPreset);
  }, [accentPreset]);
  useEffect(() => {
    document.documentElement.setAttribute("data-accent-strength", accentStrength);
  }, [accentStrength]);
  useEffect(() => {
    document.documentElement.setAttribute("data-motion", motionPreset);
  }, [motionPreset]);
  useEffect(() => {
    document.documentElement.setAttribute("data-density", densityPreset);
  }, [densityPreset]);
  useEffect(() => {
    persistUiSettingsSnapshot({
      theme,
      accentPreset,
      accentStrength,
      motionPreset,
      densityPreset,
    });
  }, [theme, accentPreset, accentStrength, motionPreset, densityPreset]);
  useEffect(() => {
    const isDevMode = Boolean((import.meta as { env?: { DEV?: boolean } }).env?.DEV);
    if (isDevMode) {
      return;
    }
    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
    };
    window.addEventListener("contextmenu", handleContextMenu);
    return () => window.removeEventListener("contextmenu", handleContextMenu);
  }, []);

  const [route, setRoute] = useState<Route>("home");
  const [settingsMode, setSettingsMode] = useState<SettingsMode>(() => readSettingsMode());
  const [instanceSettingsMode, setInstanceSettingsMode] = useState<SettingsMode>(() =>
    readInstanceSettingsMode()
  );
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [pendingSettingAnchor, setPendingSettingAnchor] = useState<string | null>(null);
  const [homeCustomizeOpen, setHomeCustomizeOpen] = useState(false);
  const [homeLayout, setHomeLayout] = useState<HomeWidgetLayoutItem[]>(() => readHomeLayout());
  const [draggedHomeWidgetId, setDraggedHomeWidgetId] = useState<HomeWidgetId | null>(null);

  // instances
  const [instances, setInstances] = useState<Instance[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = useMemo(
    () => instances.find((x) => x.id === selectedId) ?? null,
    [instances, selectedId]
  );

  // library UI state (frontend only)
  const [libraryScope, setLibraryScope] = useState<"all" | "downloaded" | "custom">("all");
  const [libraryQuery, setLibraryQuery] = useState("");
  const [librarySort, setLibrarySort] = useState<"recent" | "name">("recent");
  const [libraryGroupBy, setLibraryGroupBy] = useState<LibraryGroupBy>("none");
  const [libraryContextMenu, setLibraryContextMenu] =
    useState<LibraryContextMenuState | null>(null);
  const libraryContextMenuRef = useRef<HTMLDivElement | null>(null);

  // instance page UI state (frontend only)
  const [instanceTab, setInstanceTab] = useState<"content" | "worlds" | "logs" | "settings">("content");
  const [instanceContentType, setInstanceContentType] = useState<"mods" | "resourcepacks" | "datapacks" | "shaders">("mods");
  const [instanceQuery, setInstanceQuery] = useState("");
  const [instanceFilterState, setInstanceFilterState] = useState<InstanceContentFilters["state"]>("all");
  const [instanceFilterSource, setInstanceFilterSource] = useState<InstanceContentFilters["source"]>("all");
  const [instanceFilterMissing, setInstanceFilterMissing] = useState<InstanceContentFilters["missing"]>("all");
  const [instanceFilterWarningsOnly, setInstanceFilterWarningsOnly] = useState(false);
  const [instanceSort, setInstanceSort] = useState<InstanceContentSort>("name_asc");
  const [instanceActivityPanelOpenByInstance, setInstanceActivityPanelOpenByInstance] = useState<
    Record<string, boolean>
  >({});
  const [recentActivityFilterByInstance, setRecentActivityFilterByInstance] = useState<
    Record<string, RecentActivityFilter>
  >({});
  const [instanceContentFiltersByScope, setInstanceContentFiltersByScope] = useState<
    Record<string, InstanceContentFilters>
  >(() => readInstanceContentFiltersState());
  useEffect(() => {
    try {
      localStorage.setItem(SETTINGS_MODE_KEY, settingsMode);
    } catch {
      // ignore persistence failures
    }
  }, [settingsMode]);
  useEffect(() => {
    try {
      localStorage.setItem(INSTANCE_SETTINGS_MODE_KEY, instanceSettingsMode);
    } catch {
      // ignore persistence failures
    }
  }, [instanceSettingsMode]);
  useEffect(() => {
    try {
      localStorage.setItem(HOME_LAYOUT_KEY, JSON.stringify(homeLayout));
    } catch {
      // ignore persistence failures
    }
  }, [homeLayout]);
  useEffect(() => {
    const timer = window.setTimeout(() => {
      try {
        localStorage.setItem(INSTANCE_CONTENT_FILTERS_KEY, JSON.stringify(instanceContentFiltersByScope));
      } catch {
        // ignore persistence failures
      }
    }, 160);
    return () => window.clearTimeout(timer);
  }, [instanceContentFiltersByScope]);
  const [logFilterQuery, setLogFilterQuery] = useState("");
  const [logSeverityFilter, setLogSeverityFilter] = useState<"all" | InstanceLogSeverity>("all");
  const [logSourceFilter, setLogSourceFilter] = useState<InstanceLogSource>("live");
  const [logViewMode, setLogViewMode] = useState<LogViewMode>("live");
  const [logQuickFilters, setLogQuickFilters] = useState<Record<QuickLogFilter, boolean>>({
    errors: false,
    warnings: false,
    suspects: false,
    crashes: false,
  });
  const [logAnalyzeInput, setLogAnalyzeInput] = useState("");
  const [logAnalyzeResult, setLogAnalyzeResult] = useState<LogAnalyzeResult | null>(null);
  const [logAnalyzeBusy, setLogAnalyzeBusy] = useState(false);
  const [logAnalyzeSourcesUsed, setLogAnalyzeSourcesUsed] = useState<InstanceLogSource[]>([]);
  const [logAnalyzeMissingCrash, setLogAnalyzeMissingCrash] = useState(false);
  const [selectedCrashSuspect, setSelectedCrashSuspect] = useState<string | null>(null);
  const [logMaxLines, setLogMaxLines] = useState<number>(() => {
    if (typeof window === "undefined") return 2500;
    const raw = Number.parseInt(localStorage.getItem("mpm.logs.max_lines.v1") ?? "2500", 10);
    if (!Number.isFinite(raw)) return 2500;
    return Math.max(200, Math.min(12000, raw));
  });
  const [logLoadBusy, setLogLoadBusy] = useState(false);
  const [logLoadErr, setLogLoadErr] = useState<string | null>(null);
  const [rawLogLinesBySource, setRawLogLinesBySource] = useState<Record<string, ReadInstanceLogsResult>>({});
  const [logWindowBySource, setLogWindowBySource] = useState<Record<string, LogWindowState>>({});
  const [logAutoFollow, setLogAutoFollow] = useState(true);
  const [logJumpVisible, setLogJumpVisible] = useState(false);
  const logViewerRef = useRef<HTMLDivElement | null>(null);
  const logJumpAnimationFrameRef = useRef<number | null>(null);
  const logLoadRequestSeqRef = useRef(0);
  const [instanceSettingsOpen, setInstanceSettingsOpen] = useState(false);
  const [instanceLinksOpen, setInstanceLinksOpen] = useState(false);
  const [instanceSettingsSection, setInstanceSettingsSection] = useState<
    "general" | "installation" | "java" | "graphics" | "content"
  >("general");
  const [instanceSettingsBusy, setInstanceSettingsBusy] = useState(false);
  const [instanceNameDraft, setInstanceNameDraft] = useState("");
  const [instanceNotesDraft, setInstanceNotesDraft] = useState("");
  const [instanceJavaPathDraft, setInstanceJavaPathDraft] = useState("");
  const [instanceMemoryDraft, setInstanceMemoryDraft] = useState("4096");
  const [instanceJvmArgsDraft, setInstanceJvmArgsDraft] = useState("");
  const [javaRuntimeCandidates, setJavaRuntimeCandidates] = useState<JavaRuntimeCandidate[]>([]);
  const [javaRuntimeBusy, setJavaRuntimeBusy] = useState(false);

  function openInstance(id: string) {
    setLibraryContextMenu(null);
    setInstanceLinksOpen(false);
    setSelectedId(id);
    setRoute("instance");
  }

  function resetHomeLayout() {
    setHomeLayout(DEFAULT_HOME_LAYOUT);
  }

  function patchHomeLayout(id: HomeWidgetId, patch: Partial<HomeWidgetLayoutItem>) {
    setHomeLayout((prev) =>
      prev.map((item) => (item.id === id ? { ...item, ...patch } : item))
    );
  }

  function moveHomeWidgetToColumn(column: "main" | "side") {
    if (!draggedHomeWidgetId) return;
    setHomeLayout((prev) => {
      const source = prev.find((item) => item.id === draggedHomeWidgetId);
      if (!source) return prev;
      const next = prev.map((item) =>
        item.id === draggedHomeWidgetId ? { ...item, column } : item
      );
      const main = next
        .filter((item) => item.column === "main")
        .sort((a, b) => a.order - b.order)
        .map((item, idx) => ({ ...item, order: idx }));
      const side = next
        .filter((item) => item.column === "side")
        .sort((a, b) => a.order - b.order)
        .map((item, idx) => ({ ...item, order: idx }));
      return [...main, ...side];
    });
  }

  function nudgeHomeWidget(id: HomeWidgetId, direction: -1 | 1) {
    setHomeLayout((prev) => {
      const target = prev.find((item) => item.id === id);
      if (!target) return prev;
      const siblings = prev
        .filter((item) => item.column === target.column)
        .sort((a, b) => a.order - b.order);
      const index = siblings.findIndex((item) => item.id === id);
      if (index < 0) return prev;
      const nextIndex = Math.max(0, Math.min(siblings.length - 1, index + direction));
      if (nextIndex === index) return prev;
      const reordered = [...siblings];
      const [moved] = reordered.splice(index, 1);
      reordered.splice(nextIndex, 0, moved);
      const orderById = new Map<HomeWidgetId, number>();
      reordered.forEach((item, idx) => orderById.set(item.id, idx));
      return prev.map((item) =>
        item.column === target.column
          ? { ...item, order: orderById.get(item.id) ?? item.order }
          : item
      );
    });
  }

  function reorderHomeWidget(targetId: HomeWidgetId) {
    if (!draggedHomeWidgetId || draggedHomeWidgetId === targetId) return;
    setHomeLayout((prev) => {
      const source = prev.find((item) => item.id === draggedHomeWidgetId);
      const target = prev.find((item) => item.id === targetId);
      if (!source || !target) return prev;
      const moving = prev
        .filter((item) => item.id !== draggedHomeWidgetId)
        .map((item) =>
          item.id === targetId
            ? { ...item, order: item.order + 0.5 }
            : item
        );
      moving.push({ ...source, column: target.column, order: target.order });
      const main = moving
        .filter((item) => item.column === "main")
        .sort((a, b) => a.order - b.order)
        .map((item, idx) => ({ ...item, order: idx }));
      const side = moving
        .filter((item) => item.column === "side")
        .sort((a, b) => a.order - b.order)
        .map((item, idx) => ({ ...item, order: idx }));
      return [...main, ...side];
    });
  }

  function openSettingAnchor(anchorId: string, options?: { advanced?: boolean; target?: "global" | "instance" }) {
    if (options?.target === "instance") {
      if (options?.advanced && instanceSettingsMode !== "advanced") {
        setInstanceSettingsMode("advanced");
        setInstallNotice("Switched Instance settings to Advanced mode.");
      }
      const targetInstanceId = selectedId ?? instances[0]?.id ?? null;
      if (!targetInstanceId) {
        setInstallNotice("Create an instance first to open instance settings.");
        return;
      }
      setSelectedId(targetInstanceId);
      setRoute("instance");
      setInstanceSettingsOpen(true);
      if (anchorId.includes("installation")) setInstanceSettingsSection("installation");
      else if (anchorId.includes("java")) setInstanceSettingsSection("java");
      else if (anchorId.includes("graphics")) setInstanceSettingsSection("graphics");
      else if (anchorId.includes("hooks")) setInstanceSettingsSection("content");
      else setInstanceSettingsSection("general");
    } else {
      if (options?.advanced && settingsMode !== "advanced") {
        setSettingsMode("advanced");
        setInstallNotice("Switched to Advanced mode to open this setting.");
      }
      setRoute("settings");
    }
    setPendingSettingAnchor(anchorId);
  }

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== "k") return;
      if (!event.metaKey && !event.ctrlKey) return;
      if (commandPaletteOpen) {
        event.preventDefault();
        setCommandPaletteOpen(false);
        return;
      }
      const target = event.target as HTMLElement | null;
      const tag = target?.tagName?.toLowerCase();
      const isInputLike = Boolean(
        target?.isContentEditable || tag === "input" || tag === "textarea" || tag === "select"
      );
      if (isInputLike) return;
      event.preventDefault();
      setCommandPaletteOpen(true);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [commandPaletteOpen]);

  useEffect(() => {
    if (!pendingSettingAnchor) return;
    const targetId = `setting-anchor-${pendingSettingAnchor}`;
    const focus = () => {
      const element = document.getElementById(targetId);
      if (!element) return false;
      element.scrollIntoView({ behavior: "smooth", block: "center" });
      element.classList.add("settingAnchorFlash");
      window.setTimeout(() => element.classList.remove("settingAnchorFlash"), 1400);
      setPendingSettingAnchor(null);
      return true;
    };
    if (focus()) return;
    const timer = window.setTimeout(() => {
      focus();
    }, 220);
    return () => window.clearTimeout(timer);
  }, [pendingSettingAnchor, route, instanceSettingsOpen]);

  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function refreshInstances() {
    const list = await listInstances();
    setInstances(list);
    if (selectedId && !list.some((x) => x.id === selectedId)) {
      setSelectedId(null);
    }
  }

  async function refreshLauncherData() {
    const [settings, accounts, running, devMode] = await Promise.all([
      getLauncherSettings(),
      listLauncherAccounts(),
      listRunningInstances(),
      getDevModeState().catch(() => false),
    ]);
    setLauncherSettingsState(settings);
    setLauncherAccounts(accounts);
    const runningSafe = normalizeRunningInstancesPayload(running);
    setRunningInstances((prev) => (sameRunningInstances(prev, runningSafe) ? prev : runningSafe));
    setIsDevMode(Boolean(devMode));
    setJavaPathDraft(settings.java_path ?? "");
    setOauthClientIdDraft(settings.oauth_client_id ?? "");
    setLaunchMethodPick(settings.default_launch_method ?? "native");
    setUpdateCheckCadence(normalizeUpdateCheckCadence(settings.update_check_cadence));
    setUpdateAutoApplyMode(normalizeUpdateAutoApplyMode(settings.update_auto_apply_mode));
    setUpdateApplyScope(normalizeUpdateApplyScope(settings.update_apply_scope));
  }

  async function refreshAccountDiagnostics() {
    const startedAt = performance.now();
    setAccountDiagnosticsBusy(true);
    setAccountDiagnosticsErr(null);
    try {
      const info = await withTimeout(
        getSelectedAccountDiagnostics(),
        ACCOUNT_DIAGNOSTICS_TIMEOUT_MS
      );
      setAccountDiagnostics(info);
      return info;
    } catch (e: any) {
      const raw = e?.toString?.() ?? String(e);
      const msg = String(raw).toLowerCase().includes("timeout")
        ? "Account check timed out. Your network may be blocking Microsoft/Xbox services (common on school/work Wi-Fi). Try another network or reconnect your account."
        : raw;
      setAccountDiagnosticsErr(msg);
      return null;
    } finally {
      const duration = Math.round(performance.now() - startedAt);
      if (duration > 900) {
        console.info(`[perf] account diagnostics took ${duration}ms`);
      }
      setAccountDiagnosticsBusy(false);
    }
  }

  useEffect(() => {
    Promise.all([refreshInstances(), refreshLauncherData()]).catch((e) =>
      setError(String(e))
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (route !== "instance") {
      setInstanceLinksOpen(false);
    }
  }, [route]);
  useEffect(() => {
    if (instanceSettingsMode === "advanced") return;
    if (instanceSettingsSection === "content") {
      setInstanceSettingsSection("general");
    }
  }, [instanceSettingsMode, instanceSettingsSection]);

  useEffect(() => {
    const shouldDetect =
      route === "settings" || (instanceSettingsOpen && instanceSettingsSection === "java");
    if (!shouldDetect || javaRuntimeCandidates.length > 0 || javaRuntimeBusy) return;
    refreshJavaRuntimeCandidates().catch(() => null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route, instanceSettingsOpen, instanceSettingsSection, javaRuntimeCandidates.length, javaRuntimeBusy]);

  // Deprecated legacy Creator bridge state (kept for compatibility/migration only).
  const [presets, setPresets] = useState<UserPreset[]>([]);
  const [presetNameDraft, setPresetNameDraft] = useState("");
  const [presetBusy, setPresetBusy] = useState(false);
  const [modpacksStudioTab, setModpacksStudioTab] = useState<"creator" | "templates" | "saved" | "config">("creator");
  const [creatorDraft, setCreatorDraft] = useState<UserPreset | null>(null);
  const [instanceWorlds, setInstanceWorlds] = useState<InstanceWorld[]>([]);
  const [instanceDiskUsageById, setInstanceDiskUsageById] = useState<Record<string, number>>({});
  const [storageOverview, setStorageOverview] = useState<StorageUsageOverview | null>(null);
  const [storageOverviewBusy, setStorageOverviewBusy] = useState(false);
  const [storageOverviewError, setStorageOverviewError] = useState<string | null>(null);
  const storageOverviewRef = useRef<StorageUsageOverview | null>(null);
  const storageOverviewBusyRef = useRef(false);
  const [storageManagerSelection, setStorageManagerSelection] =
    useState<StorageManagerSelection | null>(null);
  const [storageManagerPathBySelection, setStorageManagerPathBySelection] = useState<
    Record<string, string>
  >({});
  const [storageDetailMode, setStorageDetailMode] = useState<StorageDetailMode>("folders");
  const [storageEntriesByKey, setStorageEntriesByKey] = useState<Record<string, StorageUsageEntry[]>>(
    {}
  );
  const [storageEntriesBusyKey, setStorageEntriesBusyKey] = useState<string | null>(null);
  const [storageEntriesErrorByKey, setStorageEntriesErrorByKey] = useState<Record<string, string>>(
    {}
  );
  const [storageCleanupBusy, setStorageCleanupBusy] = useState(false);
  const [storageActionBusyId, setStorageActionBusyId] = useState<string | null>(null);
  const [storageManagerNotice, setStorageManagerNotice] = useState<string | null>(null);
  const [instanceLastRunMetadataById, setInstanceLastRunMetadataById] = useState<
    Record<string, InstanceLastRunMetadata>
  >({});
  const [instancePlaytimeById, setInstancePlaytimeById] = useState<
    Record<string, InstancePlaytimeSummary | null>
  >({});
  const [instanceRunReportById, setInstanceRunReportById] = useState<
    Record<string, InstanceRunReport | null>
  >({});
  const [presetPreview, setPresetPreview] = useState<PresetApplyPreview | null>(null);
  const [presetPreviewBusy, setPresetPreviewBusy] = useState(false);
  const [templateQuery, setTemplateQuery] = useState("");
  const [templateQueryDebounced, setTemplateQueryDebounced] = useState("");
  const [templateSource, setTemplateSource] = useState<DiscoverSource>("all");
  const [templateType, setTemplateType] = useState<"modpacks" | "datapacks">("modpacks");
  const [templateHits, setTemplateHits] = useState<DiscoverSearchHit[]>([]);
  const [templateTotalHits, setTemplateTotalHits] = useState(0);
  const [templateOffset, setTemplateOffset] = useState(0);
  const [templateBusy, setTemplateBusy] = useState(false);
  const [templateErr, setTemplateErr] = useState<string | null>(null);

  useEffect(() => {
    storageOverviewRef.current = storageOverview;
  }, [storageOverview]);

  useEffect(() => {
    storageOverviewBusyRef.current = storageOverviewBusy;
  }, [storageOverviewBusy]);

  const refreshStorageOverview = useCallback(
    async (options?: { force?: boolean; clearEntries?: boolean }) => {
      if (storageOverviewBusyRef.current && !options?.force) {
        return storageOverviewRef.current;
      }
      setStorageOverviewBusy(true);
      setStorageOverviewError(null);
      try {
        const overview = await getStorageUsageOverview();
        setStorageOverview(overview);
        setInstanceDiskUsageById((prev) => {
          const next = { ...prev };
          for (const summary of overview.instance_summaries ?? []) {
            next[summary.instance_id] = Number(summary.total_bytes ?? 0);
          }
          return next;
        });
        if (options?.clearEntries) {
          setStorageEntriesByKey({});
          setStorageEntriesErrorByKey({});
        }
        return overview;
      } catch (e: any) {
        const message = e?.toString?.() ?? String(e);
        setStorageOverviewError(message);
        return null;
      } finally {
        setStorageOverviewBusy(false);
      }
    },
    []
  );

  const openStorageManager = useCallback(
    (selection: StorageManagerSelection = "overview") => {
      setStorageManagerSelection(selection);
      setStorageManagerNotice(null);
      void refreshStorageOverview({ force: true });
    },
    [refreshStorageOverview]
  );

  const loadStorageEntries = useCallback(
    async (
      selection: StorageManagerSelection,
      mode: StorageDetailMode,
      relativePath: string,
      options?: { force?: boolean }
    ) => {
      const requestKey = storageRequestKey(selection, mode, relativePath);
      if (!options?.force && storageEntriesByKey[requestKey]) {
        return storageEntriesByKey[requestKey];
      }
      const parsed = parseStorageSelection(selection);
      if (parsed.scope === "overview") return [];
      setStorageEntriesBusyKey(requestKey);
      setStorageEntriesErrorByKey((prev) => {
        if (!prev[requestKey]) return prev;
        const next = { ...prev };
        delete next[requestKey];
        return next;
      });
      try {
        const rows = await getStorageUsageEntries({
          scope: parsed.scope,
          instanceId: parsed.instanceId,
          relativePath: relativePath || undefined,
          mode,
          limit: mode === "files" ? 18 : 16,
        });
        setStorageEntriesByKey((prev) => ({ ...prev, [requestKey]: rows }));
        return rows;
      } catch (e: any) {
        const message = e?.toString?.() ?? String(e);
        setStorageEntriesErrorByKey((prev) => ({ ...prev, [requestKey]: message }));
        return [];
      } finally {
        setStorageEntriesBusyKey((prev) => (prev === requestKey ? null : prev));
      }
    },
    [storageEntriesByKey]
  );

  const revealStoragePath = useCallback(
    async (selection: StorageManagerSelection, relativePath?: string) => {
      const parsed = parseStorageSelection(selection);
      if (parsed.scope === "overview") return;
      try {
        const result = await revealStorageUsagePath({
          scope: parsed.scope,
          instanceId: parsed.instanceId,
          relativePath,
        });
        setStorageManagerNotice(result.message);
      } catch (e: any) {
        setStorageManagerNotice(e?.toString?.() ?? String(e));
      }
    },
    []
  );

  const performStorageCleanup = useCallback(
    async (
      actionIds: string[],
      options?: {
        instanceIds?: string[];
        description?: string;
        buttonId?: string;
      }
    ): Promise<StorageCleanupResult | null> => {
      const cleanActionIds = actionIds
        .map((value) => String(value ?? "").trim())
        .filter((value) => value.length > 0);
      if (cleanActionIds.length === 0) return null;
      const recommendationMap = new Map(
        (storageOverview?.cleanup_recommendations ?? []).map((item) => [item.action_id, item])
      );
      const reclaimableBytes = cleanActionIds.reduce((sum, actionId) => {
        return sum + Number(recommendationMap.get(actionId)?.reclaimable_bytes ?? 0);
      }, 0);
      const confirmMessage =
        options?.description ??
        `Run safe cleanup${reclaimableBytes > 0 ? ` and reclaim about ${formatBytes(reclaimableBytes)}` : ""}?`;
      if (!window.confirm(confirmMessage)) return null;
      setStorageCleanupBusy(true);
      setStorageActionBusyId(options?.buttonId ?? "all");
      setStorageManagerNotice(null);
      try {
        const result = await runStorageCleanup({
          actionIds: cleanActionIds,
          instanceIds: options?.instanceIds,
        });
        setStorageManagerNotice(
          result.messages?.length
            ? result.messages.join(" ")
            : result.reclaimed_bytes > 0
              ? `Reclaimed ${formatBytes(result.reclaimed_bytes)}.`
              : "No cleanup was needed."
        );
        await refreshStorageOverview({ force: true, clearEntries: true });
        if (storageManagerSelection && storageManagerSelection !== "overview") {
          const path = storageManagerPathBySelection[storageManagerSelection] ?? "";
          await loadStorageEntries(storageManagerSelection, storageDetailMode, path, { force: true });
        }
        return result;
      } catch (e: any) {
        setStorageManagerNotice(e?.toString?.() ?? String(e));
        return null;
      } finally {
        setStorageCleanupBusy(false);
        setStorageActionBusyId(null);
      }
    },
    [
      loadStorageEntries,
      refreshStorageOverview,
      storageDetailMode,
      storageManagerPathBySelection,
      storageManagerSelection,
      storageOverview?.cleanup_recommendations,
    ]
  );

  useEffect(() => {
    if (creatorDraft) return;
    const baseName = selected ? `${selected.name} custom preset` : "Custom preset";
    setCreatorDraft({
      id: `preset_${Date.now()}`,
      name: baseName,
      created_at: new Date().toISOString(),
      source_instance_id: selected?.id ?? "custom",
      source_instance_name: selected?.name ?? "Custom",
      entries: [],
      settings: defaultPresetSettings(),
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected?.id, creatorDraft]);

  useEffect(() => {
    try {
      const raw = localStorage.getItem("mpm.presets.v2") ?? localStorage.getItem("mpm.presets.v1");
      if (!raw) return;
      const parsed = JSON.parse(raw) as UserPreset[];
      if (Array.isArray(parsed)) {
        setPresets(parsed);
      }
    } catch {
      // ignore invalid local preset data
    }
  }, []);

  useEffect(() => {
    localStorage.setItem("mpm.presets.v2", JSON.stringify(presets));
  }, [presets]);

  useEffect(() => {
    const t = window.setTimeout(() => setTemplateQueryDebounced(templateQuery), 240);
    return () => window.clearTimeout(t);
  }, [templateQuery]);

  useEffect(() => {
    let cancelled = false;
    const poll = () => {
      listRunningInstances()
        .then((items) => {
          if (cancelled) return;
          const next = normalizeRunningInstancesPayload(items);
          setRunningInstances((prev) => (sameRunningInstances(prev, next) ? prev : next));
        })
        .catch(() => null);
    };
    const intervalMs = route === "library" || route === "instance" ? 3000 : 9000;
    poll();
    const t = window.setInterval(() => {
      if (document.hidden) return;
      poll();
    }, intervalMs);
    const onVisibility = () => {
      if (!document.hidden) poll();
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      cancelled = true;
      document.removeEventListener("visibilitychange", onVisibility);
      window.clearInterval(t);
    };
  }, [route]);

  // create modal state
  const [showCreate, setShowCreate] = useState(false);
  const [createMode, setCreateMode] = useState<"custom" | "file" | "launcher">("custom");
  const [name, setName] = useState("");
  const [loader, setLoader] = useState<Loader>("fabric");
  const [createIconPath, setCreateIconPath] = useState<string | null>(null);
  const [createPackFilePath, setCreatePackFilePath] = useState<string | null>(null);
  const [launcherImportSources, setLauncherImportSources] = useState<LauncherImportSource[]>([]);
  const [launcherImportBusy, setLauncherImportBusy] = useState(false);
  const [selectedLauncherImportSourceId, setSelectedLauncherImportSourceId] = useState<string | null>(null);

  // versions list
  const [discoverAllVersions, setDiscoverAllVersions] = useState(false);
  const [createAllVersions, setCreateAllVersions] = useState(false);
  const [manifest, setManifest] = useState<VersionItem[]>(FALLBACK_VERSIONS);
  const [mcVersion, setMcVersion] = useState<string | null>(null);
  const [manifestError, setManifestError] = useState<string | null>(null);

  useEffect(() => {
    fetchOfficialManifest()
      .then((list) => setManifest(list.length ? list : FALLBACK_VERSIONS))
      .catch((e) => setManifestError(String(e)));
  }, []);

  useEffect(() => {
    if (!showCreate || createMode !== "launcher") return;
    refreshLauncherImportSources().catch(() => null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showCreate, createMode]);

  const visibleCreateVersions = useMemo(() => {
    return createAllVersions ? manifest : manifest.filter((v) => v.type === "release");
  }, [manifest, createAllVersions]);

  const visibleDiscoverVersions = useMemo(() => {
    return discoverAllVersions ? manifest : manifest.filter((v) => v.type === "release");
  }, [manifest, discoverAllVersions]);

  const groupedCreateVersions = useMemo(
    () =>
      createAllVersions
        ? groupAllVersions(visibleCreateVersions)
        : groupVersions(visibleCreateVersions),
    [visibleCreateVersions, createAllVersions]
  );

  const groupedDiscoverVersions = useMemo(
    () =>
      discoverAllVersions
        ? groupAllVersions(visibleDiscoverVersions)
        : groupVersions(visibleDiscoverVersions),
    [visibleDiscoverVersions, discoverAllVersions]
  );
  const instanceVersionOptions = useMemo(() => {
    const values = new Set<string>();
    for (const item of manifest) {
      if (item.type === "release") values.add(item.id);
    }
    if (selected?.mc_version) values.add(selected.mc_version);
    return Array.from(values)
      .sort(compareReleaseIdDesc)
      .slice(0, 80)
      .map((value) => ({ value, label: value }));
  }, [manifest, selected?.mc_version]);

  useEffect(() => {
    if (!instanceSettingsOpen || !selected) return;
    const normalized = normalizeInstanceSettings(selected.settings);
    setInstanceNameDraft(selected.name);
    setInstanceNotesDraft(normalized.notes);
    setInstanceJavaPathDraft(normalized.java_path);
    setInstanceMemoryDraft(String(normalized.memory_mb));
    setInstanceJvmArgsDraft(normalized.jvm_args);
  }, [instanceSettingsOpen, selected]);

  async function onPickCreateIcon() {
    setError(null);
    try {
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }],
      });
      if (!picked || Array.isArray(picked)) return;
      setCreateIconPath(picked);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    }
  }

  async function onPickCreateModpackFile() {
    setError(null);
    try {
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "Modpack archive", extensions: ["mrpack", "zip"] }],
      });
      if (!picked || Array.isArray(picked)) return;
      setCreatePackFilePath(picked);
      if (!name.trim()) {
        setName(basenameWithoutExt(picked));
      }
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    }
  }

  async function refreshLauncherImportSources() {
    setLauncherImportBusy(true);
    setError(null);
    try {
      const list = await listLauncherImportSources();
      setLauncherImportSources(list);
      setSelectedLauncherImportSourceId((prev) => {
        if (prev && list.some((item) => item.id === prev)) return prev;
        return list[0]?.id ?? null;
      });
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setLauncherImportBusy(false);
    }
  }

  async function onAddCustomSkin() {
    setError(null);
    try {
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "Minecraft skin", extensions: ["png"] }],
      });
      if (!picked || Array.isArray(picked)) return;
      const previewDataUrl = await resolveLocalImageDataUrl(picked);
      const entry: SavedCustomSkin = {
        id: `custom_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
        label: basenameWithoutExt(picked),
        skin_path: picked,
        preview_data_url: previewDataUrl,
      };
      setCustomSkins((prev) => [entry, ...prev.filter((row) => row.skin_path !== entry.skin_path)]);
      setSelectedAccountSkinId(`custom:${entry.id}`);
      setInstallNotice(`Added custom skin "${entry.label}".`);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    }
  }

  function onCycleAccountCape() {
    if (capeOptions.length <= 1) return;
    setSelectedAccountCapeId((prev) => {
      const currentIdx = Math.max(
        0,
        capeOptions.findIndex((cape) => cape.id === prev)
      );
      const nextIdx = (currentIdx + 1) % capeOptions.length;
      return capeOptions[nextIdx]?.id ?? "none";
    });
  }

  function onPlaySkinViewerEmote() {
    skinViewerEmoteTriggerRef.current?.("play");
  }

  function onRenameSelectedCustomSkin() {
    if (!selectedAccountSkin || selectedAccountSkin.origin !== "custom") return;
    const nextLabel = skinRenameDraft.trim();
    if (!nextLabel) return;
    const token = selectedAccountSkin.id.replace(/^custom:/, "");
    setCustomSkins((prev) =>
      prev.map((row) => (row.id === token ? { ...row, label: nextLabel } : row))
    );
    setInstallNotice(`Renamed skin to "${nextLabel}".`);
  }

  async function onApplySelectedAppearance() {
    if (!selectedLauncherAccountId) {
      setSkinViewerErr("Connect and select a Microsoft account first.");
      return;
    }
    const skinSource = String(
      selectedAccountSkin?.apply_source ?? selectedAccountSkin?.skin_url ?? ""
    ).trim();
    if (!skinSource) {
      setSkinViewerErr("Select a skin first.");
      return;
    }
    setSkinViewerErr(null);
    setLauncherErr(null);
    setAccountAppearanceBusy(true);
    try {
      const diag = await applySelectedAccountAppearance({
        applySkin: true,
        skinSource,
        skinVariant: selectedAccountSkin?.variant ?? null,
        applyCape: true,
        capeId: selectedAccountCape?.id === "none" ? null : selectedAccountCape?.id ?? null,
      });
      setAccountDiagnostics(diag);
      setInstallNotice(
        selectedAccountCape?.id === "none"
          ? "Applied skin and cleared active cape."
          : "Applied skin and cape to your Minecraft profile."
      );
      skinViewerEmoteTriggerRef.current?.("celebrate");
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setSkinViewerErr(msg);
    } finally {
      setAccountAppearanceBusy(false);
    }
  }

  function onRemoveSelectedCustomSkin() {
    if (!selectedAccountSkin || selectedAccountSkin.origin !== "custom") return;
    const token = selectedAccountSkin.id.replace(/^custom:/, "");
    setCustomSkins((prev) => prev.filter((row) => row.id !== token));
    setSelectedAccountSkinId(null);
    setInstallNotice(`Removed custom skin "${selectedAccountSkin.label}".`);
  }

  async function onSelectInstanceIcon(inst: Instance) {
    setError(null);
    setBusy("instance-icon");
    try {
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }],
      });
      if (!picked || Array.isArray(picked)) return;
      await setInstanceIcon({ instanceId: inst.id, iconPath: picked });
      await refreshInstances();
      setInstallNotice(`Updated icon for ${inst.name}.`);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setBusy(null);
    }
  }

  async function onRemoveInstanceIcon(inst: Instance) {
    setError(null);
    setBusy("instance-icon");
    try {
      await setInstanceIcon({ instanceId: inst.id, iconPath: null });
      await refreshInstances();
      setInstallNotice(`Removed icon for ${inst.name}.`);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setBusy(null);
    }
  }

  async function refreshJavaRuntimeCandidates() {
    setJavaRuntimeBusy(true);
    setLauncherErr(null);
    setError(null);
    try {
      const runtimes = await detectJavaRuntimes();
      const deduped = Array.from(new Map(runtimes.map((rt) => [rt.path, rt])).values()).sort(
        (a, b) => b.major - a.major || a.path.localeCompare(b.path)
      );
      setJavaRuntimeCandidates(deduped);
      if (deduped.length === 0) {
        setInstallNotice("No Java runtimes detected automatically. You can still choose one manually.");
      } else {
        setInstallNotice(
          `Detected ${deduped.length} Java runtime${deduped.length === 1 ? "" : "s"}.`
        );
      }
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setError(msg);
    } finally {
      setJavaRuntimeBusy(false);
    }
  }

  async function onPickLauncherJavaPath() {
    setLauncherErr(null);
    setError(null);
    try {
      const picked = await openDialog({
        multiple: false,
      });
      if (!picked || Array.isArray(picked)) return;
      setJavaPathDraft(picked);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setError(msg);
    }
  }

  async function onPickInstanceJavaPath(inst: Instance) {
    setError(null);
    try {
      const picked = await openDialog({
        multiple: false,
      });
      if (!picked || Array.isArray(picked)) return;
      setInstanceJavaPathDraft(picked);
      await persistInstanceChanges(inst, {
        settings: {
          java_path: picked,
        },
      });
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    }
  }

  async function persistInstanceChanges(
    inst: Instance,
    patch: {
      name?: string;
      mcVersion?: string;
      loader?: Loader;
      settings?: Partial<InstanceSettings>;
    },
    successMessage?: string
  ) {
    setInstanceSettingsBusy(true);
    setError(null);
    try {
      const live = instances.find((x) => x.id === inst.id) ?? inst;
      const payload: {
        instanceId: string;
        name?: string;
        mcVersion?: string;
        loader?: Loader;
        settings?: InstanceSettings;
      } = { instanceId: live.id };
      if (typeof patch.name === "string") payload.name = patch.name;
      if (typeof patch.mcVersion === "string") payload.mcVersion = patch.mcVersion;
      if (typeof patch.loader === "string") payload.loader = patch.loader;
      if (patch.settings) {
        payload.settings = normalizeInstanceSettings({
          ...normalizeInstanceSettings(live.settings),
          ...patch.settings,
        });
      }
      const updated = await updateInstance(payload);
      setInstances((prev) => prev.map((row) => (row.id === updated.id ? updated : row)));
      const normalized = normalizeInstanceSettings(updated.settings);
      setInstanceNameDraft(updated.name);
      setInstanceNotesDraft(normalized.notes);
      setInstanceJavaPathDraft(normalized.java_path);
      setInstanceMemoryDraft(String(normalized.memory_mb));
      setInstanceJvmArgsDraft(normalized.jvm_args);
      if (successMessage) setInstallNotice(successMessage);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setInstanceSettingsBusy(false);
    }
  }

  async function onCommitInstanceName(inst: Instance) {
    const trimmed = instanceNameDraft.trim();
    if (!trimmed) {
      setError("Instance name cannot be empty.");
      setInstanceNameDraft(inst.name);
      return;
    }
    if (trimmed === inst.name) return;
    await persistInstanceChanges(inst, { name: trimmed }, "Instance name saved.");
  }

  async function onCommitInstanceNotes(inst: Instance) {
    const current = normalizeInstanceSettings(inst.settings).notes;
    if (instanceNotesDraft === current) return;
    await persistInstanceChanges(
      inst,
      {
        settings: {
          notes: instanceNotesDraft,
        },
      },
      "Notes saved."
    );
  }

  async function onCommitInstanceJavaPath(inst: Instance) {
    const next = instanceJavaPathDraft.trim();
    const current = normalizeInstanceSettings(inst.settings).java_path;
    if (next === current) return;
    await persistInstanceChanges(
      inst,
      {
        settings: {
          java_path: next,
        },
      },
      next ? "Instance Java path updated." : "Instance Java override cleared."
    );
  }

  async function onCommitInstanceMemory(inst: Instance) {
    const parsed = Number(instanceMemoryDraft);
    if (!Number.isFinite(parsed)) {
      setError("Memory must be a number in MB.");
      setInstanceMemoryDraft(String(normalizeInstanceSettings(inst.settings).memory_mb));
      return;
    }
    const clamped = Math.max(512, Math.min(65536, Math.round(parsed)));
    setInstanceMemoryDraft(String(clamped));
    const current = normalizeInstanceSettings(inst.settings).memory_mb;
    if (clamped === current) return;
    await persistInstanceChanges(
      inst,
      {
        settings: {
          memory_mb: clamped,
        },
      },
      "Instance memory saved."
    );
  }

  async function onCommitInstanceJvmArgs(inst: Instance) {
    const next = instanceJvmArgsDraft.trim();
    const current = normalizeInstanceSettings(inst.settings).jvm_args;
    if (next === current) return;
    await persistInstanceChanges(
      inst,
      {
        settings: {
          jvm_args: next,
        },
      },
      "JVM args saved."
    );
  }

  async function onCreate() {
    setError(null);
    setBusy("create");
    try {
      let inst: Instance;
      if (createMode === "custom") {
        if (!name.trim()) throw new Error("Name is required");
        if (!mcVersion) throw new Error("Pick a game version");
        inst = await createInstance({ name, mcVersion, loader, iconPath: createIconPath });
      } else if (createMode === "file") {
        if (!createPackFilePath) throw new Error("Pick a modpack archive first.");
        const result: CreateInstanceFromModpackFileResult = await createInstanceFromModpackFile({
          filePath: createPackFilePath,
          name: name.trim() || undefined,
          iconPath: createIconPath,
        });
        inst = result.instance;
        if (result.warnings.length > 0) {
          setInstallNotice(
            `Imported ${result.imported_files} override file${result.imported_files === 1 ? "" : "s"} with warnings: ${result.warnings.join(" | ")}`
          );
        } else {
          setInstallNotice(
            `Imported modpack archive with ${result.imported_files} override file${result.imported_files === 1 ? "" : "s"}.`
          );
        }
      } else {
        if (!selectedLauncherImportSourceId) throw new Error("Select a launcher source first.");
        const result: ImportInstanceFromLauncherResult = await importInstanceFromLauncher({
          sourceId: selectedLauncherImportSourceId,
          name: name.trim() || undefined,
          iconPath: createIconPath,
        });
        inst = result.instance;
        setInstallNotice(
          `Imported ${result.imported_files} file${result.imported_files === 1 ? "" : "s"} from launcher source.`
        );
      }
      await refreshInstances();

      setSelectedId(inst.id);
      setShowCreate(false);
      setRoute("library");

      // reset
      setCreateMode("custom");
      setName("");
      setLoader("fabric");
      setCreateIconPath(null);
      setCreatePackFilePath(null);
      setSelectedLauncherImportSourceId(null);
      setCreateAllVersions(false);
      setMcVersion(null);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setBusy(null);
    }
  }

  function requestDelete(inst: Instance) {
    setLibraryContextMenu(null);
    setDeleteTarget(inst);
  }

  async function onDelete() {
    if (!deleteTarget) return;
    setError(null);
    setBusy("delete");
    try {
      await deleteInstance(deleteTarget.id);
      await refreshInstances();
      if (selectedId === deleteTarget.id) {
        setSelectedId(null);
        setRoute("library");
        setInstanceSettingsOpen(false);
      }
      setDeleteTarget(null);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setBusy(null);
    }
  }

  // Discover (Step 2)
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<DiscoverSearchHit[]>([]);
  const [totalHits, setTotalHits] = useState(0);
  const [offset, setOffset] = useState(0);
  const [limit, setLimit] = useState(20);
  const [index, setIndex] = useState<ModrinthIndex>("relevance");
  const [discoverSources, setDiscoverSources] = useState<DiscoverProviderSource[]>([
    ...DISCOVER_PROVIDER_SOURCES,
  ]);
  const [discoverContentType, setDiscoverContentType] = useState<DiscoverContentType>("mods");
  const [filterLoaders, setFilterLoaders] = useState<string[]>([]);
  const [filterVersion, setFilterVersion] = useState<string | null>(null);
  const [filterCategories, setFilterCategories] = useState<string[]>([]);
  const [discoverErr, setDiscoverErr] = useState<string | null>(null);
  const [discoverBusy, setDiscoverBusy] = useState(false);
  const effectiveDiscoverSources = useMemo(
    () => {
      const normalized = normalizeDiscoverProviderSources(discoverSources);
      return normalized.length > 0 ? normalized : [...DISCOVER_PROVIDER_SOURCES];
    },
    [discoverSources]
  );
  const discoverSourceValue = useMemo<DiscoverSource>(
    () => (effectiveDiscoverSources.length === 1 ? effectiveDiscoverSources[0] : "all"),
    [effectiveDiscoverSources]
  );

  const page = useMemo(() => Math.floor(offset / limit) + 1, [offset, limit]);
  const pages = useMemo(() => Math.max(1, Math.ceil(totalHits / limit)), [totalHits, limit]);

  const [projectOpen, setProjectOpen] = useState<Project | null>(null);
  const [projectVersions, setProjectVersions] = useState<ProjectVersion[]>([]);
  const [projectMembers, setProjectMembers] = useState<ProjectMember[]>([]);
  const [projectDetailTab, setProjectDetailTab] = useState<ProjectDetailTab>("overview");
  const [projectCopyNotice, setProjectCopyNotice] = useState<string | null>(null);
  const [curseforgeOpen, setCurseforgeOpen] = useState<CurseforgeProjectDetail | null>(null);
  const [githubOpen, setGithubOpen] = useState<DiscoverSearchHit | null>(null);
  const [githubDetail, setGithubDetail] = useState<GithubProjectDetail | null>(null);
  const githubDetailRequestIdRef = useRef(0);
  const [githubBusy, setGithubBusy] = useState(false);
  const [githubErr, setGithubErr] = useState<string | null>(null);
  const [githubDetailTab, setGithubDetailTab] = useState<GithubDetailTab>("overview");
  const [curseforgeDetailTab, setCurseforgeDetailTab] = useState<CurseforgeDetailTab>("overview");
  const [curseforgeBusy, setCurseforgeBusy] = useState(false);
  const [curseforgeErr, setCurseforgeErr] = useState<string | null>(null);
  const [projectOpenContentType, setProjectOpenContentType] = useState<DiscoverContentType>("mods");
  const [curseforgeOpenContentType, setCurseforgeOpenContentType] = useState<DiscoverContentType>("mods");

  const [installTarget, setInstallTarget] = useState<InstallTarget | null>(null);
  const [installInstanceQuery, setInstallInstanceQuery] = useState("");
  const [modpackAddTarget, setModpackAddTarget] = useState<InstallTarget | null>(null);
  const [modpackAddSpecs, setModpackAddSpecs] = useState<ModpackSpec[]>([]);
  const [modpackAddSpecId, setModpackAddSpecId] = useState("");
  const [modpackAddLayerId, setModpackAddLayerId] = useState("");
  const [modpackAddRequired, setModpackAddRequired] = useState(true);
  const [modpackAddEnabledByDefault, setModpackAddEnabledByDefault] = useState(true);
  const [modpackAddChannelPolicy, setModpackAddChannelPolicy] = useState<"stable" | "beta" | "alpha">("stable");
  const [modpackAddFallbackPolicy, setModpackAddFallbackPolicy] = useState<"inherit" | "strict" | "smart" | "loose">("inherit");
  const [modpackAddPinnedVersion, setModpackAddPinnedVersion] = useState("");
  const [modpackAddNotes, setModpackAddNotes] = useState("");
  const [modpackAddSpecsBusy, setModpackAddSpecsBusy] = useState(false);
  const [modpackAddBusy, setModpackAddBusy] = useState(false);
  const [modpackAddErr, setModpackAddErr] = useState<string | null>(null);
  const [discoverAddContext, setDiscoverAddContext] = useState<DiscoverAddContext | null>(null);
  const [discoverAddTrayItems, setDiscoverAddTrayItems] = useState<DiscoverAddTrayItem[]>([]);
  const [discoverAddTrayExpanded, setDiscoverAddTrayExpanded] = useState(true);
  const [discoverAddTraySticky, setDiscoverAddTraySticky] = useState<boolean>(() => {
    try {
      return localStorage.getItem(DISCOVER_ADD_TRAY_STICKY_KEY) === "1";
    } catch {
      return false;
    }
  });
  const discoverAddContextKeyRef = useRef<string | null>(null);
  const selectedModpackAddSpec = useMemo(
    () => modpackAddSpecs.find((spec) => spec.id === modpackAddSpecId) ?? null,
    [modpackAddSpecs, modpackAddSpecId]
  );
  const [projectBusy, setProjectBusy] = useState(false);
  const [projectErr, setProjectErr] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Instance | null>(null);
  const [installedMods, setInstalledMods] = useState<InstalledMod[]>([]);
  const [installedModsInstanceId, setInstalledModsInstanceId] = useState<string | null>(null);
  const [installedIconCache, setInstalledIconCache] = useState<Record<string, string>>(() =>
    readInstalledIconCache()
  );
  const [installedIconFailedByKey, setInstalledIconFailedByKey] = useState<Record<string, boolean>>(
    {}
  );
  const installedIconFetchesRef = useRef<Map<string, Promise<string | null>>>(new Map());
  const installedModsLoadSeqRef = useRef(0);
  const selectedInstanceIdRef = useRef<string | null>(selectedId);
  const routeRef = useRef(route);
  const [selectedModVersionIds, setSelectedModVersionIds] = useState<string[]>([]);
  const [modsBusy, setModsBusy] = useState(false);
  const [modsErr, setModsErr] = useState<string | null>(null);
  const [cleanMissingBusyInstanceId, setCleanMissingBusyInstanceId] = useState<string | null>(null);
  const [toggleBusyVersion, setToggleBusyVersion] = useState<string | null>(null);
  const [providerSwitchBusyKey, setProviderSwitchBusyKey] = useState<string | null>(null);
  const [pinBusyVersion, setPinBusyVersion] = useState<string | null>(null);
  const [dependencyInstallBusyVersion, setDependencyInstallBusyVersion] = useState<string | null>(null);
  const [githubAttachBusyVersion, setGithubAttachBusyVersion] = useState<string | null>(null);
  const [githubAttachTarget, setGithubAttachTarget] = useState<GithubAttachModalTarget | null>(null);
  const [githubAttachInput, setGithubAttachInput] = useState("");
  const [githubAttachErr, setGithubAttachErr] = useState<string | null>(null);
  const [installProgress, setInstallProgress] = useState<InstallProgressEvent | null>(null);
  const [installingKey, setInstallingKey] = useState<string | null>(null);
  const [installNotice, setInstallNotice] = useState<string | null>(null);
  const [curseforgeBlockedRecoveryPrompt, setCurseforgeBlockedRecoveryPrompt] =
    useState<CurseforgeBlockedRecoveryPrompt | null>(null);
  const [installProgressEtaSeconds, setInstallProgressEtaSeconds] = useState<number | null>(null);
  const [installProgressElapsedSeconds, setInstallProgressElapsedSeconds] = useState<number | null>(null);
  const [installProgressBytesPerSecond, setInstallProgressBytesPerSecond] = useState<number | null>(null);
  const installProgressTimingRef = useRef<Record<string, {
    started_at: number;
    last_at: number;
    last_percent: number;
    rate_percent_per_sec: number;
    last_downloaded: number;
    rate_bytes_per_sec: number;
  }>>({});
  const [scheduledUpdateRunStartedAt, setScheduledUpdateRunStartedAt] = useState<number | null>(null);
  const [scheduledUpdateRunCompleted, setScheduledUpdateRunCompleted] = useState(0);
  const [scheduledUpdateRunTotal, setScheduledUpdateRunTotal] = useState(0);
  const [scheduledUpdateRunEtaSeconds, setScheduledUpdateRunEtaSeconds] = useState<number | null>(null);
  const [scheduledUpdateRunElapsedSeconds, setScheduledUpdateRunElapsedSeconds] = useState<number | null>(null);

  useLayoutEffect(() => {
    selectedInstanceIdRef.current = selectedId;
  }, [selectedId]);

  useLayoutEffect(() => {
    routeRef.current = route;
  }, [route]);

  const canMutateVisibleInstalledMods = useCallback(
    (instanceId: string) =>
      routeRef.current === "instance" && selectedInstanceIdRef.current === instanceId,
    []
  );

  const applyInstalledModsForInstance = useCallback(
    (
      instanceId: string,
      updater: InstalledMod[] | ((prev: InstalledMod[]) => InstalledMod[])
    ) => {
      if (!canMutateVisibleInstalledMods(instanceId)) return;
      setInstalledMods((prev) => (typeof updater === "function" ? (updater as (prev: InstalledMod[]) => InstalledMod[])(prev) : updater));
      setInstalledModsInstanceId(instanceId);
    },
    [canMutateVisibleInstalledMods]
  );
  const [perfActions, setPerfActions] = useState<PerfActionEntry[]>(() => {
    try {
      const raw = localStorage.getItem(PERF_ACTION_LOG_KEY);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .filter((entry) => entry && typeof entry === "object")
        .slice(0, 120) as PerfActionEntry[];
    } catch {
      return [];
    }
  });
  function recordPerfAction(
    name: string,
    status: PerfActionStatus,
    startedAtMs: number,
    detail?: string | null
  ) {
    const duration = Math.max(0, performance.now() - startedAtMs);
    const entry: PerfActionEntry = {
      id: `${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      name,
      detail: detail ?? null,
      status,
      duration_ms: duration,
      finished_at: Date.now(),
    };
    setPerfActions((prev) => [entry, ...prev].slice(0, 120));
    if (duration >= 1500) {
      console.info(`[perf] ${name} took ${Math.round(duration)}ms${detail ? ` (${detail})` : ""}`);
    }
  }

  useEffect(() => {
    try {
      localStorage.setItem(PERF_ACTION_LOG_KEY, JSON.stringify(perfActions.slice(0, 120)));
    } catch {
      // ignore telemetry persistence failures
    }
  }, [perfActions]);

  const [instanceActivityById, setInstanceActivityById] = useState<
    Record<string, InstanceActivityEntry[]>
  >({});
  function appendInstanceActivity(
    instanceId: string,
    messages: string[],
    tone?: InstanceActivityEntry["tone"]
  ) {
    const cleaned = messages
      .map((msg) => String(msg ?? "").trim())
      .filter((msg) => msg.length > 0);
    if (!instanceId || cleaned.length === 0) return;
    setInstanceActivityById((prev) => {
      const existing = prev[instanceId] ?? [];
      const now = Date.now();
      const latest = existing[0];
      if (
        latest &&
        cleaned.length === 1 &&
        latest.message === cleaned[0] &&
        (tone == null || latest.tone === tone) &&
        now - latest.at < 60_000
      ) {
        return prev;
      }
      const newEntries = cleaned.map((message) => ({
        id: `activity_${now}_${Math.random().toString(36).slice(2, 8)}`,
        message,
        at: now,
        tone: tone ?? inferActivityTone(message),
      }));
      const nextItems = [...newEntries, ...existing].slice(0, 20);
      return {
        ...prev,
        [instanceId]: nextItems,
      };
    });
  }
  const lastInstallNoticeRef = useRef<string | null>(null);
  const [importingInstanceId, setImportingInstanceId] = useState<string | null>(null);
  const [launchBusyInstanceIds, setLaunchBusyInstanceIds] = useState<string[]>([]);
  const [launchCancelBusyInstanceId, setLaunchCancelBusyInstanceId] = useState<string | null>(null);
  const userRequestedStopLaunchIdsRef = useRef<Set<string>>(new Set());
  const [launchStageByInstance, setLaunchStageByInstance] = useState<
    Record<string, { status: string; label: string; message: string; updated_at: number }>
  >({});
  const [launchProgressChecksByInstance, setLaunchProgressChecksByInstance] = useState<
    Record<string, LaunchHealthChecks>
  >({});
  const [launchHealthByInstance, setLaunchHealthByInstance] = useState<Record<string, LaunchHealthRecord>>(() => {
    try {
      const raw = localStorage.getItem("mpm.launchHealth.v1");
      if (!raw) return {};
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object") return {};
      return parsed as Record<string, LaunchHealthRecord>;
    } catch {
      return {};
    }
  });
  const [launchHealthDismissedByInstance, setLaunchHealthDismissedByInstance] = useState<
    Record<string, boolean>
  >(() => {
    try {
      const raw = localStorage.getItem("mpm.launchHealth.dismissed.v1");
      if (!raw) return {};
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object") return {};
      return parsed as Record<string, boolean>;
    } catch {
      return {};
    }
  });
  const [launchFailureByInstance, setLaunchFailureByInstance] = useState<
    Record<string, LaunchFailureRecord>
  >({});
  const [launchOutcomesByInstance, setLaunchOutcomesByInstance] = useState<LaunchOutcomesByInstance>(() => {
    try {
      const raw = localStorage.getItem(LAUNCH_OUTCOMES_KEY);
      if (!raw) return {};
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object") return {};
      const next: LaunchOutcomesByInstance = {};
      for (const [instanceId, rows] of Object.entries(parsed as Record<string, any>)) {
        if (!Array.isArray(rows)) continue;
        next[instanceId] = rows
          .map((item) => ({
            at: Number(item?.at ?? Date.now()),
            ok: Boolean(item?.ok),
            message: item?.message ? String(item.message) : null,
          }))
          .filter((item) => Number.isFinite(item.at))
          .slice(0, 30);
      }
      return next;
    } catch {
      return {};
    }
  });
  const [instanceHealthPanelPrefsByInstance, setInstanceHealthPanelPrefsByInstance] =
    useState<InstanceHealthPanelPrefs>(() => {
      try {
        const raw = localStorage.getItem(INSTANCE_HEALTH_PANEL_PREFS_KEY);
        if (!raw) return {};
        const parsed = JSON.parse(raw);
        if (!parsed || typeof parsed !== "object") return {};
        const next: InstanceHealthPanelPrefs = {};
        for (const [instanceId, value] of Object.entries(parsed as Record<string, any>)) {
          if (!instanceId || !value || typeof value !== "object") continue;
          next[instanceId] = {
            hidden: Boolean((value as any).hidden),
            collapsed: Boolean((value as any).collapsed),
            permissions_expanded: Boolean((value as any).permissions_expanded),
          };
        }
        return next;
      } catch {
        return {};
      }
    });
  const [autoProfileAppliedHintsByInstance, setAutoProfileAppliedHintsByInstance] = useState<Record<string, string>>(
    () => {
      try {
        const raw = localStorage.getItem(AUTOPROFILE_APPLIED_KEY);
        if (!raw) return {};
        const parsed = JSON.parse(raw);
        if (!parsed || typeof parsed !== "object") return {};
        return parsed as Record<string, string>;
      } catch {
        return {};
      }
    }
  );
  const [autoProfileDismissedByInstance, setAutoProfileDismissedByInstance] = useState<Record<string, string>>(
    () => {
      try {
        const raw = localStorage.getItem(AUTOPROFILE_DISMISSED_KEY);
        if (!raw) return {};
        const parsed = JSON.parse(raw);
        if (!parsed || typeof parsed !== "object") return {};
        const out: Record<string, string> = {};
        for (const [instanceId, signature] of Object.entries(parsed as Record<string, unknown>)) {
          if (!instanceId) continue;
          const normalized = String(signature ?? "").trim();
          if (!normalized) continue;
          out[instanceId] = normalized;
        }
        return out;
      } catch {
        return {};
      }
    }
  );
  const [instanceFriendLinkStatus, setInstanceFriendLinkStatus] = useState<FriendLinkStatus | null>(null);
  const [friendLinkStatusByInstance, setFriendLinkStatusByInstance] = useState<Record<string, FriendLinkStatus>>({});
  const [friendLinkDriftByInstance, setFriendLinkDriftByInstance] = useState<Record<string, FriendLinkDriftPreview>>({});
  const friendLinkDriftAnnounceRef = useRef<Record<string, string>>({});
  const friendLinkAutoSyncBusyRef = useRef(false);
  const friendLinkAutoSyncInFlightRef = useRef<Record<string, boolean>>({});
  const friendLinkAutoSyncLastSignatureRef = useRef<Record<string, string>>({});
  const [friendLinkSyncBusyInstanceId, setFriendLinkSyncBusyInstanceId] = useState<string | null>(null);
  const [friendConflictInstanceId, setFriendConflictInstanceId] = useState<string | null>(null);
  const [friendConflictResult, setFriendConflictResult] = useState<FriendLinkReconcileResult | null>(null);
  const [friendConflictResolveBusy, setFriendConflictResolveBusy] = useState(false);
  const [preflightReportModal, setPreflightReportModal] = useState<{
    instanceId: string;
    method: LaunchMethod;
    report: LaunchCompatibilityReport;
  } | null>(null);
  const [preflightReportByInstance, setPreflightReportByInstance] = useState<
    Record<string, LaunchCompatibilityReport>
  >({});
  const [permissionChecklistBusyByInstance, setPermissionChecklistBusyByInstance] = useState<
    Record<string, boolean>
  >({});
  const [autoMicPromptSettingBusy, setAutoMicPromptSettingBusy] = useState(false);
  const permissionChecklistRefreshInFlightRef = useRef<Record<string, boolean>>({});
  const autoMicPromptAttemptRef = useRef<Record<string, { fingerprint: string; at: number }>>({});
  const [preflightIgnoreByInstance, setPreflightIgnoreByInstance] = useState<
    Record<string, LaunchPreflightIgnoreEntry>
  >(() => {
    try {
      const raw = localStorage.getItem(PREFLIGHT_IGNORE_KEY);
      if (!raw) return {};
      const parsed = JSON.parse(raw) as Record<string, any>;
      if (!parsed || typeof parsed !== "object") return {};
      const now = Date.now();
      const out: Record<string, LaunchPreflightIgnoreEntry> = {};
      for (const [instanceId, row] of Object.entries(parsed)) {
        const fingerprint = String((row as any)?.fingerprint ?? "").trim();
        const expiresAt = Number((row as any)?.expires_at ?? 0);
        if (!instanceId || !fingerprint || !Number.isFinite(expiresAt) || expiresAt <= now) continue;
        out[instanceId] = { fingerprint, expires_at: expiresAt };
      }
      return out;
    } catch {
      return {};
    }
  });
  const [launchFixPlanByInstance, setLaunchFixPlanByInstance] = useState<Record<string, LaunchFixPlan>>({});
  const [launchFixPlanDraftByInstance, setLaunchFixPlanDraftByInstance] = useState<
    Record<string, LaunchFixActionDraft[]>
  >({});
  const [launchFixApplyResultByInstance, setLaunchFixApplyResultByInstance] = useState<
    Record<string, LaunchFixApplyResult>
  >({});
  const [launchFixBusyInstanceId, setLaunchFixBusyInstanceId] = useState<string | null>(null);
  const [launchFixApplyBusyInstanceId, setLaunchFixApplyBusyInstanceId] = useState<string | null>(null);
  const [launchFixModalInstanceId, setLaunchFixModalInstanceId] = useState<string | null>(null);
  const [launchFixDryRunByActionId, setLaunchFixDryRunByActionId] = useState<Record<string, string>>({});
  const localResolverBackfillAtRef = useRef<Record<string, number>>({});
  const localResolverBusyRef = useRef<Record<string, boolean>>({});
  const [supportBundleModalInstanceId, setSupportBundleModalInstanceId] = useState<string | null>(null);
  const [supportBundleIncludeRawLogs, setSupportBundleIncludeRawLogs] = useState<boolean>(() =>
    readSupportBundleRawDefault()
  );
  const [supportBundleBusy, setSupportBundleBusy] = useState(false);
  const [launchMethodPick, setLaunchMethodPick] = useState<LaunchMethod>("native");
  const [updateCheckCadence, setUpdateCheckCadence] = useState<SchedulerCadence>("daily");
  const [updateAutoApplyMode, setUpdateAutoApplyMode] = useState<SchedulerAutoApplyMode>("never");
  const [updateApplyScope, setUpdateApplyScope] = useState<SchedulerApplyScope>("scheduled_only");
  const [updatesPageContentTypes, setUpdatesPageContentTypes] = useState<UpdatableContentType[]>(
    ALL_UPDATABLE_CONTENT_TYPES
  );
  const [launcherSettings, setLauncherSettingsState] = useState<LauncherSettings | null>(null);
  const [appLanguageBusy, setAppLanguageBusy] = useState(false);
  const [autoIdentifyLocalJarsBusy, setAutoIdentifyLocalJarsBusy] = useState(false);
  const [launcherAccounts, setLauncherAccounts] = useState<LauncherAccount[]>([]);
  const [settingsAccountManageId, setSettingsAccountManageId] = useState<string | null>(null);
  const [runningInstances, setRunningInstances] = useState<RunningInstance[]>([]);
  const [launcherErr, setLauncherErr] = useState<string | null>(null);
  const [launcherBusy, setLauncherBusy] = useState(false);
  const [appVersion, setAppVersion] = useState("unknown");
  const [appUpdaterState, setAppUpdaterState] = useState<AppUpdaterState | null>(null);
  const [appUpdaterBusy, setAppUpdaterBusy] = useState(false);
  const [appUpdaterInstallBusy, setAppUpdaterInstallBusy] = useState(false);
  const [appUpdaterLastError, setAppUpdaterLastError] = useState<string | null>(null);
  const [appUpdateBannerDismissedKey, setAppUpdateBannerDismissedKey] = useState<string | null>(null);
  const [appUpdateBannerMounted, setAppUpdateBannerMounted] = useState(false);
  const [appUpdateBannerExiting, setAppUpdateBannerExiting] = useState(false);
  const [appUpdaterAutoCheck, setAppUpdaterAutoCheck] = useState<boolean>(() => {
    try {
      const raw = localStorage.getItem(APP_UPDATER_AUTOCHECK_KEY);
      return raw == null ? true : raw === "1";
    } catch {
      return true;
    }
  });
  const appUpdaterAutoCheckStartedRef = useRef(false);
  const [msLoginSessionId, setMsLoginSessionId] = useState<string | null>(null);
  const [msLoginState, setMsLoginState] = useState<MicrosoftLoginState | null>(null);
  const [msCodePrompt, setMsCodePrompt] = useState<MicrosoftCodePrompt | null>(null);
  const [msCodePromptVisible, setMsCodePromptVisible] = useState(false);
  const [msCodeCopied, setMsCodeCopied] = useState(false);
  const [javaPathDraft, setJavaPathDraft] = useState("");
  const [isDevMode, setIsDevMode] = useState(false);
  const [devCurseforgeKeyDraft, setDevCurseforgeKeyDraft] = useState("");
  const [devCurseforgeKeyBusy, setDevCurseforgeKeyBusy] = useState(false);
  const [devCurseforgeNotice, setDevCurseforgeNotice] = useState<string | null>(null);
  const [devCurseforgeNoticeIsError, setDevCurseforgeNoticeIsError] = useState(false);
  const appLanguage = useMemo(
    () => normalizeAppLanguage(launcherSettings?.app_language ?? null),
    [launcherSettings?.app_language]
  );
  const t = useCallback(
    (key: AppTranslationKey, vars?: Record<string, string | number>) =>
      translateAppText(appLanguage, key, vars),
    [appLanguage]
  );
  useEffect(() => {
    document.documentElement.lang = appLanguage;
  }, [appLanguage]);
  const appLanguageMenuOptions = useMemo(
    () =>
      APP_LANGUAGE_OPTIONS.map((option) => ({
        value: option.value,
        label:
          option.value === "en-US"
            ? option.nativeLabel
            : `${option.nativeLabel} · ${option.englishLabel}`,
      })),
    []
  );
  const settingsRailItems = useMemo<
    Array<{ id: string; label: string; icon: IconName; advanced?: boolean }>
  >(
    () => [
      { id: "global:appearance", label: t("settings.appearance.section_title"), icon: "sparkles" },
      { id: "global:language", label: t("settings.language.section_title"), icon: "books" },
      { id: "global:launch-method", label: t("settings.launch.section_title"), icon: "cpu" },
      { id: "global:account", label: t("settings.account.section_title"), icon: "user" },
      { id: "global:app-updates", label: t("settings.updates.section_title"), icon: "download" },
      { id: "global:content-visuals", label: t("settings.content.section_title"), icon: "skin" },
      ...(settingsMode === "advanced"
        ? [
            { id: "global:permissions", label: "Launch permissions", icon: "sliders" as IconName, advanced: true },
            { id: "global:github-api", label: "GitHub API auth", icon: "layers" as IconName, advanced: true },
          ]
        : []),
    ],
    [settingsMode, t]
  );
  const [activeSettingsRail, setActiveSettingsRail] = useState("global:appearance");
  useEffect(() => {
    if (route !== "settings") return;
    const elements = settingsRailItems
      .map((item) => document.getElementById(`setting-anchor-${item.id}`))
      .filter((element): element is HTMLElement => Boolean(element));
    if (elements.length === 0) return;
    const findScrollContainer = (start: HTMLElement) => {
      let current: HTMLElement | null = start.parentElement;
      while (current) {
        const style = window.getComputedStyle(current);
        const overflowY = style.overflowY;
        if ((overflowY === "auto" || overflowY === "scroll") && current.scrollHeight > current.clientHeight + 4) {
          return current;
        }
        current = current.parentElement;
      }
      return document.querySelector(".content") as HTMLElement | null;
    };
    const scrollContainer = findScrollContainer(elements[0]);
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((entry) => entry.isIntersecting)
          .sort((a, b) => b.intersectionRatio - a.intersectionRatio);
        const nextId = visible[0]?.target?.id?.replace(/^setting-anchor-/, "");
        if (nextId) setActiveSettingsRail(nextId);
      },
      {
        root: scrollContainer,
        rootMargin: "-16% 0px -58% 0px",
        threshold: [0.18, 0.35, 0.6],
      }
    );
    elements.forEach((element) => observer.observe(element));
    return () => observer.disconnect();
  }, [route, settingsRailItems]);
  useEffect(() => {
    if (route !== "settings") return;
    if (!pendingSettingAnchor?.startsWith("global:")) return;
    setActiveSettingsRail(pendingSettingAnchor);
  }, [pendingSettingAnchor, route]);
  const [curseforgeApiStatus, setCurseforgeApiStatus] = useState<CurseforgeApiStatus | null>(null);
  const [curseforgeApiBusy, setCurseforgeApiBusy] = useState(false);
  const [githubTokenPoolStatus, setGithubTokenPoolStatus] = useState<GithubTokenPoolStatus | null>(null);
  const [githubTokenPoolDraft, setGithubTokenPoolDraft] = useState("");
  const [githubTokenPoolBusy, setGithubTokenPoolBusy] = useState(false);
  const [githubTokenPoolNotice, setGithubTokenPoolNotice] = useState<string | null>(null);
  const [githubTokenPoolNoticeIsError, setGithubTokenPoolNoticeIsError] = useState(false);
  const [discordPresenceBusy, setDiscordPresenceBusy] = useState(false);
  const [quickPlayServers, setQuickPlayServers] = useState<QuickPlayServerEntry[]>([]);
  const [quickPlayBusy, setQuickPlayBusy] = useState(false);
  const [quickPlayErr, setQuickPlayErr] = useState<string | null>(null);
  const [quickPlayDraftName, setQuickPlayDraftName] = useState("");
  const [quickPlayDraftHost, setQuickPlayDraftHost] = useState("");
  const [quickPlayDraftPort, setQuickPlayDraftPort] = useState("25565");
  const [quickPlayDraftBoundInstanceId, setQuickPlayDraftBoundInstanceId] = useState("none");
  const [instanceHistoryById, setInstanceHistoryById] = useState<Record<string, InstanceHistoryEvent[]>>({});
  const [instanceHistoryBusyById, setInstanceHistoryBusyById] = useState<Record<string, boolean>>({});
  const [timelineClearedAtByInstance, setTimelineClearedAtByInstance] = useState<Record<string, number>>({});
  const [fullHistoryModalInstanceId, setFullHistoryModalInstanceId] = useState<string | null>(null);
  const [fullHistoryByInstance, setFullHistoryByInstance] = useState<
    Record<string, InstanceHistoryEvent[]>
  >({});
  const [fullHistoryBeforeAtByInstance, setFullHistoryBeforeAtByInstance] = useState<
    Record<string, string | null>
  >({});
  const [fullHistoryHasMoreByInstance, setFullHistoryHasMoreByInstance] = useState<Record<string, boolean>>({});
  const [fullHistoryBusyByInstance, setFullHistoryBusyByInstance] = useState<Record<string, boolean>>({});
  const [fullHistoryFilterByInstance, setFullHistoryFilterByInstance] = useState<
    Record<string, RecentActivityFilter>
  >({});
  const [fullHistorySearchByInstance, setFullHistorySearchByInstance] = useState<Record<string, string>>({});
  const [instanceModCountById, setInstanceModCountById] = useState<Record<string, number>>({});
  const instanceHistoryRefreshInFlightRef = useRef<Record<string, boolean>>({});
  const [oauthClientIdDraft, setOauthClientIdDraft] = useState("");
  const [accountDiagnostics, setAccountDiagnostics] = useState<AccountDiagnostics | null>(() =>
    readCachedAccountDiagnostics()
  );
  const [accountDiagnosticsBusy, setAccountDiagnosticsBusy] = useState(false);
  const [accountDiagnosticsErr, setAccountDiagnosticsErr] = useState<string | null>(null);
  const [accountAvatarFromSkin, setAccountAvatarFromSkin] = useState<string | null>(null);
  const [accountAvatarSourceIdx, setAccountAvatarSourceIdx] = useState(0);
  const [customSkins, setCustomSkins] = useState<SavedCustomSkin[]>(() => {
    try {
      const raw = localStorage.getItem("mpm.account.custom_skins.v1");
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .map((item) => ({
          id: String(item?.id ?? "").trim() || `custom:${Math.random().toString(36).slice(2)}`,
          label: String(item?.label ?? "").trim() || "Custom skin",
          skin_path: String(item?.skin_path ?? "").trim(),
          preview_data_url: String(item?.preview_data_url ?? "").trim() || null,
        }))
        .filter((item) => item.skin_path);
    } catch {
      return [];
    }
  });
  const [instanceLaunchHooksById, setInstanceLaunchHooksById] = useState<
    Record<string, InstanceLaunchHooksDraft>
  >(() => {
    try {
      const raw = localStorage.getItem("mpm.instance.launch_hooks.v1");
      if (!raw) return {};
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object") return {};
      const normalized: Record<string, InstanceLaunchHooksDraft> = {};
      for (const [id, value] of Object.entries(parsed as Record<string, any>)) {
        if (!id) continue;
        normalized[id] = {
          enabled: Boolean(value?.enabled),
          pre_launch: String(value?.pre_launch ?? ""),
          wrapper: String(value?.wrapper ?? ""),
          post_exit: String(value?.post_exit ?? ""),
        };
      }
      return normalized;
    } catch {
      return {};
    }
  });
  const [selectedAccountSkinId, setSelectedAccountSkinId] = useState<string | null>(null);
  const [selectedAccountCapeId, setSelectedAccountCapeId] = useState<string>("none");
  const [accountSkinThumbs, setAccountSkinThumbs] = useState<Record<string, AccountSkinThumbSet>>(
    {}
  );
  const [previewTimeOfDay] = useState<number>(() => {
    try {
      const raw = localStorage.getItem("mpm.skinPreview.time_of_day.v1");
      if (!raw) return 14;
      return normalizeTimeOfDay(Number(raw));
    } catch {
      return 14;
    }
  });
  const [skinPreviewEnabled, setSkinPreviewEnabled] = useState<boolean>(() => {
    try {
      const raw = localStorage.getItem("mpm.skinPreview3d.enabled.v1");
      if (raw === null) {
        const nav = navigator as Navigator & { deviceMemory?: number };
        const cores = nav.hardwareConcurrency ?? 8;
        const memory = typeof nav.deviceMemory === "number" ? nav.deviceMemory : 8;
        return !(cores <= 4 || memory <= 4);
      }
      return raw === "1" || raw === "true";
    } catch {
      return true;
    }
  });
  const [skinViewerErr, setSkinViewerErr] = useState<string | null>(null);
  const [skinViewerPreparing, setSkinViewerPreparing] = useState(false);
  const [skinViewerBusy, setSkinViewerBusy] = useState(false);
  const [accountAppearanceBusy, setAccountAppearanceBusy] = useState(false);
  const [skinRenameDraft, setSkinRenameDraft] = useState("");
  const [skinViewerEpoch, setSkinViewerEpoch] = useState(0);
  const accountSkinViewerStageRef = useRef<HTMLDivElement | null>(null);
  const accountSkinViewerCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const accountSkinViewerRef = useRef<SkinViewer | null>(null);
  const accountSkinViewerResizeRef = useRef<ResizeObserver | null>(null);
  const skinViewerInputCleanupRef = useRef<(() => void) | null>(null);
  const skinViewerEmoteTriggerRef = useRef<((mode?: "play" | "celebrate") => void) | null>(null);
  const skinTextureCacheRef = useRef<Map<string, string>>(new Map());
  const capeTextureCacheRef = useRef<Map<string, string>>(new Map());
  const skinViewerNameTagTextRef = useRef<string | null>(null);
  const lastLoadedSkinSrcRef = useRef<string | null>(null);
  const lastLoadedCapeSrcRef = useRef<string | null>(null);
  const [updateCheck, setUpdateCheck] = useState<ContentUpdateCheckResult | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [updateAllBusy, setUpdateAllBusy] = useState(false);
  const [updateErr, setUpdateErr] = useState<string | null>(null);
  const [scheduledUpdateEntriesByInstance, setScheduledUpdateEntriesByInstance] = useState<
    Record<string, ScheduledUpdateCheckEntry>
  >(() => {
    try {
      const raw = localStorage.getItem("mpm.scheduledUpdates.v1");
      if (!raw) return {};
      const parsed = JSON.parse(raw) as Record<string, any>;
      if (!parsed || typeof parsed !== "object") return {};
      const next: Record<string, ScheduledUpdateCheckEntry> = {};
      for (const [instanceId, row] of Object.entries(parsed)) {
        if (!row || typeof row !== "object") continue;
        const updatesRaw = Array.isArray((row as any).updates) ? (row as any).updates : [];
        const updates: ContentUpdateInfo[] = updatesRaw
          .filter((item: any) => item && typeof item === "object")
          .map((item: any) => ({
            source: String(item.source ?? "modrinth"),
            content_type: String(item.content_type ?? "mods"),
            project_id: String(item.project_id ?? ""),
            name: String(item.name ?? ""),
            current_version_id: String(item.current_version_id ?? ""),
            current_version_number: String(item.current_version_number ?? ""),
            latest_version_id: String(item.latest_version_id ?? ""),
            latest_version_number: String(item.latest_version_number ?? ""),
            enabled: item.enabled !== false,
            target_worlds: Array.isArray(item.target_worlds)
              ? item.target_worlds.map((w: any) => String(w ?? "")).filter(Boolean)
              : [],
            compatibility_status: item.compatibility_status
              ? String(item.compatibility_status)
              : undefined,
            compatibility_notes: Array.isArray(item.compatibility_notes)
              ? item.compatibility_notes.map((note: any) => String(note ?? "")).filter(Boolean)
              : [],
          }))
          .filter((item) => Boolean(item.project_id));
        next[instanceId] = {
          instance_id: String((row as any).instance_id ?? instanceId),
          instance_name: String((row as any).instance_name ?? "Instance"),
          checked_at: String((row as any).checked_at ?? new Date(0).toISOString()),
          checked_entries: Number((row as any).checked_entries ?? (row as any).checked_mods ?? 0),
          update_count: Number((row as any).update_count ?? updates.length),
          updates,
          error: (row as any).error ? String((row as any).error) : null,
        };
      }
      return next;
    } catch {
      return {};
    }
  });
  const [scheduledAppliedUpdatesByInstance, setScheduledAppliedUpdatesByInstance] = useState<
    Record<string, ScheduledAppliedUpdateEntry>
  >(() => {
    try {
      const raw = localStorage.getItem("mpm.scheduledUpdates.applied.v1");
      if (!raw) return {};
      const parsed = JSON.parse(raw) as Record<string, any>;
      if (!parsed || typeof parsed !== "object") return {};
      const next: Record<string, ScheduledAppliedUpdateEntry> = {};
      for (const [instanceId, row] of Object.entries(parsed)) {
        if (!row || typeof row !== "object") continue;
        const updatesRaw = Array.isArray((row as any).updates) ? (row as any).updates : [];
        const updates: ContentUpdateInfo[] = updatesRaw
          .filter((item: any) => item && typeof item === "object")
          .map((item: any) => ({
            source: String(item.source ?? "modrinth"),
            content_type: String(item.content_type ?? "mods"),
            project_id: String(item.project_id ?? ""),
            name: String(item.name ?? ""),
            current_version_id: String(item.current_version_id ?? ""),
            current_version_number: String(item.current_version_number ?? ""),
            latest_version_id: String(item.latest_version_id ?? ""),
            latest_version_number: String(item.latest_version_number ?? ""),
            enabled: item.enabled !== false,
            target_worlds: Array.isArray(item.target_worlds)
              ? item.target_worlds.map((w: any) => String(w ?? "")).filter(Boolean)
              : [],
            compatibility_status: item.compatibility_status
              ? String(item.compatibility_status)
              : undefined,
            compatibility_notes: Array.isArray(item.compatibility_notes)
              ? item.compatibility_notes.map((note: any) => String(note ?? "")).filter(Boolean)
              : [],
          }))
          .filter((item) => Boolean(item.project_id));
        next[instanceId] = {
          instance_id: String((row as any).instance_id ?? instanceId),
          instance_name: String((row as any).instance_name ?? "Instance"),
          applied_at: String((row as any).applied_at ?? new Date(0).toISOString()),
          updated_entries: Number((row as any).updated_entries ?? updates.length),
          updates,
          warnings: Array.isArray((row as any).warnings)
            ? (row as any).warnings.map((warning: any) => String(warning ?? "")).filter(Boolean)
            : [],
        };
      }
      return next;
    } catch {
      return {};
    }
  });
  const [scheduledUpdateLastRunAt, setScheduledUpdateLastRunAt] = useState<string | null>(() => {
    try {
      const raw = localStorage.getItem("mpm.scheduledUpdates.lastRunAt");
      if (!raw) return null;
      const iso = String(raw);
      return Number.isFinite(toTimestamp(iso)) ? iso : null;
    } catch {
      return null;
    }
  });
  const [scheduledUpdateBusy, setScheduledUpdateBusy] = useState(false);
  const [scheduledUpdateErr, setScheduledUpdateErr] = useState<string | null>(null);
  const [updatePrefsBusy, setUpdatePrefsBusy] = useState(false);
  const [updatePrefsSavedFlash, setUpdatePrefsSavedFlash] = useState(false);
  const scheduledUpdateRunningRef = useRef(false);
  const [installPlanPreview, setInstallPlanPreview] = useState<
    Record<string, InstallPlanPreview>
  >({});
  const [installPlanPreviewBusy, setInstallPlanPreviewBusy] = useState<Record<string, boolean>>({});
  const [installPlanPreviewErr, setInstallPlanPreviewErr] = useState<Record<string, string>>({});
  const [snapshots, setSnapshots] = useState<SnapshotMeta[]>([]);
  const [snapshotsBusy, setSnapshotsBusy] = useState(false);
  const snapshotsLoadSeqRef = useRef(0);
  const [rollbackBusy, setRollbackBusy] = useState(false);
  const [rollbackSnapshotId, setRollbackSnapshotId] = useState<string | null>(null);
  const [worldRollbackBusyById, setWorldRollbackBusyById] = useState<Record<string, boolean>>({});
  const [presetIoBusy, setPresetIoBusy] = useState(false);
  const scopedInstalledMods = useMemo(
    () => (selectedId && installedModsInstanceId === selectedId ? installedMods : []),
    [installedMods, installedModsInstanceId, selectedId]
  );
  const instanceFilterScopeKey = useMemo(
    () => (selectedId ? `${selectedId}::${instanceContentType}` : ""),
    [selectedId, instanceContentType]
  );
  useLayoutEffect(() => {
    if (route !== "instance" || instanceTab !== "content" || !instanceFilterScopeKey) return;
    const saved = instanceContentFiltersByScope[instanceFilterScopeKey] ?? defaultInstanceContentFilters();
    const nextQuery = saved.query ?? "";
    const nextState = saved.state ?? "all";
    const nextSource = saved.source ?? "all";
    const nextMissing = saved.missing ?? "all";
    const nextWarningsOnly = Boolean(saved.warningsOnly);
    setInstanceQuery((prev) => (prev === nextQuery ? prev : nextQuery));
    setInstanceFilterState((prev) => (prev === nextState ? prev : nextState));
    setInstanceFilterSource((prev) => (prev === nextSource ? prev : nextSource));
    setInstanceFilterMissing((prev) => (prev === nextMissing ? prev : nextMissing));
    setInstanceFilterWarningsOnly((prev) => (prev === nextWarningsOnly ? prev : nextWarningsOnly));
    // Keep restore-to-scope deterministic without rebinding on every keystroke.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route, instanceTab, instanceFilterScopeKey]);
  useEffect(() => {
    if (route !== "instance" || instanceTab !== "content" || !instanceFilterScopeKey) return;
    const timer = window.setTimeout(() => {
      setInstanceContentFiltersByScope((prev) => {
        const nextEntry: InstanceContentFilters = {
          query: instanceQuery,
          state: instanceFilterState,
          source: instanceFilterSource,
          missing: instanceFilterMissing,
          warningsOnly: instanceFilterWarningsOnly,
        };
        const current = prev[instanceFilterScopeKey];
        if (current && sameInstanceContentFilters(current, nextEntry)) return prev;
        return {
          ...prev,
          [instanceFilterScopeKey]: nextEntry,
        };
      });
    }, 90);
    return () => window.clearTimeout(timer);
  }, [
    route,
    instanceTab,
    instanceFilterScopeKey,
    instanceQuery,
    instanceFilterState,
    instanceFilterSource,
    instanceFilterMissing,
    instanceFilterWarningsOnly,
  ]);
  const normalizedInstanceQuery = useMemo(
    () => instanceQuery.trim().toLowerCase(),
    [instanceQuery]
  );
  const selectedModVersionIdSet = useMemo(
    () => new Set(selectedModVersionIds),
    [selectedModVersionIds]
  );
  const runningByInstanceId = useMemo(() => {
    const map = new Map<string, RunningInstance[]>();
    for (const item of runningInstances) {
      const key = item.instance_id;
      const prev = map.get(key);
      if (prev) {
        prev.push(item);
      } else {
        map.set(key, [item]);
      }
    }
    return map;
  }, [runningInstances]);
  const scheduledUpdateEntries = useMemo(
    () =>
      Object.values(scheduledUpdateEntriesByInstance).sort(
        (a, b) =>
          toTimestamp(b.checked_at) - toTimestamp(a.checked_at) ||
          a.instance_name.localeCompare(b.instance_name)
      ),
    [scheduledUpdateEntriesByInstance]
  );
  const scheduledUpdatesAvailableTotal = useMemo(
    () => scheduledUpdateEntries.reduce((sum, row) => sum + Math.max(0, row.update_count || 0), 0),
    [scheduledUpdateEntries]
  );
  const scheduledInstancesWithUpdatesCount = useMemo(
    () => scheduledUpdateEntries.filter((row) => (row.update_count ?? 0) > 0).length,
    [scheduledUpdateEntries]
  );
  const updatesPageContentTypesNormalized = useMemo(() => {
    const seen = new Set<UpdatableContentType>();
    const ordered: UpdatableContentType[] = [];
    for (const candidate of updatesPageContentTypes) {
      const normalized = normalizeUpdatableContentType(candidate);
      if (!normalized || seen.has(normalized)) continue;
      seen.add(normalized);
      ordered.push(normalized);
    }
    if (ordered.length === 0) {
      return [...ALL_UPDATABLE_CONTENT_TYPES];
    }
    return ALL_UPDATABLE_CONTENT_TYPES.filter((value) => seen.has(value));
  }, [updatesPageContentTypes]);
  const updatesPageUseAllContentTypes = useMemo(
    () => updatesPageContentTypesNormalized.length === ALL_UPDATABLE_CONTENT_TYPES.length,
    [updatesPageContentTypesNormalized]
  );
  const updatesPageBackendContentTypes = useMemo(
    () => (updatesPageUseAllContentTypes ? [] : [...updatesPageContentTypesNormalized]),
    [updatesPageUseAllContentTypes, updatesPageContentTypesNormalized]
  );
  const updatesPageVisibleEntries = useMemo(() => {
    if (updatesPageUseAllContentTypes) return scheduledUpdateEntries;
    const filterSet = new Set<UpdatableContentType>(updatesPageContentTypesNormalized);
    return scheduledUpdateEntries.map((row) => {
      const filteredUpdates = row.updates.filter((item) => {
        const normalized = normalizeUpdatableContentType(item.content_type);
        return normalized ? filterSet.has(normalized) : false;
      });
      return {
        ...row,
        update_count: filteredUpdates.length,
        updates: filteredUpdates,
      };
    });
  }, [scheduledUpdateEntries, updatesPageUseAllContentTypes, updatesPageContentTypesNormalized]);
  const updatesPageUpdatesAvailableTotal = useMemo(
    () => updatesPageVisibleEntries.reduce((sum, row) => sum + Math.max(0, row.update_count || 0), 0),
    [updatesPageVisibleEntries]
  );
  const updatesPageInstancesWithUpdatesCount = useMemo(
    () => updatesPageVisibleEntries.filter((row) => (row.update_count ?? 0) > 0).length,
    [updatesPageVisibleEntries]
  );
  const scheduledAppliedUpdates = useMemo(
    () =>
      Object.values(scheduledAppliedUpdatesByInstance).sort(
        (a, b) =>
          toTimestamp(b.applied_at) - toTimestamp(a.applied_at) ||
          a.instance_name.localeCompare(b.instance_name)
      ),
    [scheduledAppliedUpdatesByInstance]
  );
  const scheduledAppliedUpdatesRecent = useMemo(
    () => scheduledAppliedUpdates.slice(0, 6),
    [scheduledAppliedUpdates]
  );
  const perfActionMetrics = useMemo(() => {
    if (perfActions.length === 0) return null;
    const durations = perfActions
      .map((entry) => Math.max(0, Number(entry.duration_ms) || 0))
      .sort((a, b) => a - b);
    const count = durations.length;
    const total = durations.reduce((sum, value) => sum + value, 0);
    const avgMs = total / Math.max(1, count);
    const p95Index = Math.min(count - 1, Math.floor(count * 0.95));
    const p95Ms = durations[p95Index] ?? durations[count - 1] ?? 0;
    const slowestMs = durations[count - 1] ?? 0;
    return {
      count,
      avg_ms: avgMs,
      p95_ms: p95Ms,
      slowest_ms: slowestMs,
    };
  }, [perfActions]);
  const nextScheduledUpdateRunAt = useMemo(
    () => computeNextUpdateRunAt(scheduledUpdateLastRunAt, updateCheckCadence),
    [scheduledUpdateLastRunAt, updateCheckCadence]
  );
  const installedContentSummary = useMemo(() => {
    const modEntries: InstalledMod[] = [];
    const resourcepackEntries: InstalledMod[] = [];
    const shaderpackEntries: InstalledMod[] = [];
    const datapackEntries: InstalledMod[] = [];
    const visibleInstalledMods: InstalledMod[] = [];
    const selectableVisibleEntries: InstalledMod[] = [];
    let selectedVisibleEntryCount = 0;
    let selectedInstalledEntryCount = 0;

    for (const entry of scopedInstalledMods) {
      const normalized = normalizeCreatorEntryType(entry.content_type);
      const entryKey = installedEntryUiKey(entry);
      if (normalized === "mods") modEntries.push(entry);
      else if (normalized === "resourcepacks") resourcepackEntries.push(entry);
      else if (normalized === "shaderpacks") shaderpackEntries.push(entry);
      else if (normalized === "datapacks") datapackEntries.push(entry);

      if (selectedModVersionIdSet.has(entryKey)) {
        selectedInstalledEntryCount += 1;
      }

      if (normalizeInstanceContentType(entry.content_type) !== instanceContentType) continue;
      if (instanceFilterState === "enabled" && !entry.enabled) continue;
      if (instanceFilterState === "disabled" && entry.enabled) continue;
      if (instanceFilterMissing === "missing" && entry.file_exists) continue;
      if (instanceFilterMissing === "present" && !entry.file_exists) continue;
      if (instanceFilterWarningsOnly && !hasDependencyWarnings(entry)) continue;
      if (instanceFilterSource !== "all") {
        const source = effectiveInstalledProviderSource(entry);
        if (instanceFilterSource === "other") {
          if (
            source === "modrinth" ||
            source === "curseforge" ||
            source === "github" ||
            source === "local"
          ) {
            continue;
          }
        } else if (source !== instanceFilterSource) {
          continue;
        }
      }
      if (
        normalizedInstanceQuery &&
        !entry.name.toLowerCase().includes(normalizedInstanceQuery) &&
        !entry.version_number.toLowerCase().includes(normalizedInstanceQuery) &&
        !entry.filename.toLowerCase().includes(normalizedInstanceQuery)
      ) {
        continue;
      }

      visibleInstalledMods.push(entry);
      if (entry.file_exists) {
        selectableVisibleEntries.push(entry);
        if (selectedModVersionIdSet.has(entryKey)) {
          selectedVisibleEntryCount += 1;
        }
      }
    }

    visibleInstalledMods.sort((a, b) => {
      const byName = a.name.localeCompare(b.name);
      if (instanceSort === "recently_added") {
        const byAddedAt = Number(b.added_at ?? 0) - Number(a.added_at ?? 0);
        if (byAddedAt !== 0) return byAddedAt;
        return byName;
      }
      if (instanceSort === "name_asc") return byName;
      if (instanceSort === "name_desc") return b.name.localeCompare(a.name);
      if (instanceSort === "source") {
        const sourceCmp = providerSourceLabel(effectiveInstalledProviderSource(a)).localeCompare(
          providerSourceLabel(effectiveInstalledProviderSource(b))
        );
        if (sourceCmp !== 0) return sourceCmp;
        return byName;
      }
      if (instanceSort === "enabled_first") {
        const stateCmp = Number(a.enabled === b.enabled ? 0 : a.enabled ? -1 : 1);
        if (stateCmp !== 0) return stateCmp;
        return byName;
      }
      if (instanceSort === "disabled_first") {
        const stateCmp = Number(a.enabled === b.enabled ? 0 : a.enabled ? 1 : -1);
        if (stateCmp !== 0) return stateCmp;
        return byName;
      }
      return byName;
    });

    return {
      modEntries,
      resourcepackEntries,
      shaderpackEntries,
      datapackEntries,
      visibleInstalledMods,
      selectableVisibleEntries,
      selectedVisibleEntryCount,
      selectedInstalledEntryCount,
    };
  }, [
    scopedInstalledMods,
    instanceContentType,
    normalizedInstanceQuery,
    selectedModVersionIdSet,
    instanceFilterState,
    instanceFilterSource,
    instanceFilterMissing,
    instanceFilterWarningsOnly,
    instanceSort,
  ]);
  const hasDependencyWarningsInScope = useMemo(
    () =>
      scopedInstalledMods.some(
        (entry) =>
          normalizeInstanceContentType(entry.content_type) === instanceContentType &&
          hasDependencyWarnings(entry)
      ),
    [scopedInstalledMods, instanceContentType]
  );
  const instanceHealthById = useMemo<Record<string, InstanceHealthScore>>(() => {
    const out: Record<string, InstanceHealthScore> = {};
    for (const inst of instances) {
      out[inst.id] = computeInstanceHealthScore({
        instanceId: inst.id,
        launchOutcomesByInstance,
        friendStatus: friendLinkStatusByInstance[inst.id] ?? null,
        scheduledUpdatesByInstance: scheduledUpdateEntriesByInstance,
      });
    }
    return out;
  }, [instances, launchOutcomesByInstance, friendLinkStatusByInstance, scheduledUpdateEntriesByInstance]);
  const selectedInstanceAutoProfileRecommendation = useMemo<AutoProfileRecommendation | null>(() => {
    if (!selected) return null;
    const enabledModCount = installedContentSummary.modEntries.filter((mod) => mod.enabled).length;
    return computeAutoProfileRecommendation({
      instance: selected,
      enabledModCount,
      launchOutcomesByInstance,
    });
  }, [selected, installedContentSummary.modEntries, launchOutcomesByInstance]);

  useEffect(() => {
    localStorage.setItem("mpm.launchHealth.v1", JSON.stringify(launchHealthByInstance));
  }, [launchHealthByInstance]);

  useEffect(() => {
    localStorage.setItem(
      "mpm.launchHealth.dismissed.v1",
      JSON.stringify(launchHealthDismissedByInstance)
    );
  }, [launchHealthDismissedByInstance]);

  useEffect(() => {
    localStorage.setItem("mpm.account.custom_skins.v1", JSON.stringify(customSkins));
  }, [customSkins]);

  useEffect(() => {
    let cancelled = false;
    const missing = customSkins.filter(
      (item) => item.skin_path && !String(item.preview_data_url ?? "").trim()
    );
    if (missing.length === 0) return;
    (async () => {
      const updates: Record<string, string> = {};
      for (const item of missing) {
        const dataUrl = await resolveLocalImageDataUrl(item.skin_path);
        if (!dataUrl) continue;
        updates[item.id] = dataUrl;
      }
      if (cancelled || Object.keys(updates).length === 0) return;
      setCustomSkins((prev) =>
        prev.map((item) =>
          updates[item.id]
            ? { ...item, preview_data_url: updates[item.id] }
            : item
        )
      );
    })().catch(() => null);
    return () => {
      cancelled = true;
    };
  }, [customSkins]);

  useEffect(() => {
    if (accountDiagnostics) {
      localStorage.setItem(ACCOUNT_DIAGNOSTICS_CACHE_KEY, JSON.stringify(accountDiagnostics));
    } else {
      localStorage.removeItem(ACCOUNT_DIAGNOSTICS_CACHE_KEY);
    }
  }, [accountDiagnostics]);

  useEffect(() => {
    localStorage.setItem("mpm.skinPreview3d.enabled.v1", skinPreviewEnabled ? "1" : "0");
  }, [skinPreviewEnabled]);

  useEffect(() => {
    localStorage.setItem("mpm.logs.max_lines.v1", String(Math.max(200, Math.min(12000, logMaxLines))));
  }, [logMaxLines]);

  useEffect(() => {
    localStorage.setItem(INSTALLED_ICON_CACHE_KEY, JSON.stringify(installedIconCache));
  }, [installedIconCache]);

  useEffect(() => {
    localStorage.setItem(
      "mpm.skinPreview.time_of_day.v1",
      String(normalizeTimeOfDay(previewTimeOfDay))
    );
  }, [previewTimeOfDay]);

  useEffect(() => {
    localStorage.setItem("mpm.instance.launch_hooks.v1", JSON.stringify(instanceLaunchHooksById));
  }, [instanceLaunchHooksById]);

  useEffect(() => {
    localStorage.setItem("mpm.scheduledUpdates.v1", JSON.stringify(scheduledUpdateEntriesByInstance));
  }, [scheduledUpdateEntriesByInstance]);

  useEffect(() => {
    localStorage.setItem(
      "mpm.scheduledUpdates.applied.v1",
      JSON.stringify(scheduledAppliedUpdatesByInstance)
    );
  }, [scheduledAppliedUpdatesByInstance]);

  useEffect(() => {
    localStorage.setItem(LAUNCH_OUTCOMES_KEY, JSON.stringify(launchOutcomesByInstance));
  }, [launchOutcomesByInstance]);

  useEffect(() => {
    const now = Date.now();
    const normalized: Record<string, LaunchPreflightIgnoreEntry> = {};
    for (const [instanceId, row] of Object.entries(preflightIgnoreByInstance)) {
      if (!instanceId) continue;
      const fingerprint = String(row?.fingerprint ?? "").trim();
      const expiresAt = Number(row?.expires_at ?? 0);
      if (!fingerprint || !Number.isFinite(expiresAt) || expiresAt <= now) continue;
      normalized[instanceId] = { fingerprint, expires_at: expiresAt };
    }
    localStorage.setItem(PREFLIGHT_IGNORE_KEY, JSON.stringify(normalized));
    if (Object.keys(normalized).length !== Object.keys(preflightIgnoreByInstance).length) {
      setPreflightIgnoreByInstance(normalized);
    }
  }, [preflightIgnoreByInstance]);

  useEffect(() => {
    if (route !== "instance" || !selectedId) return;
    void refreshInstancePermissionChecklist(selectedId, launchMethodPick, {
      silent: true,
      skipAutoPrompt: true,
    });
  }, [route, selectedId, launchMethodPick]);

  useEffect(() => {
    localStorage.setItem(
      INSTANCE_HEALTH_PANEL_PREFS_KEY,
      JSON.stringify(instanceHealthPanelPrefsByInstance)
    );
  }, [instanceHealthPanelPrefsByInstance]);

  useEffect(() => {
    localStorage.setItem(AUTOPROFILE_APPLIED_KEY, JSON.stringify(autoProfileAppliedHintsByInstance));
  }, [autoProfileAppliedHintsByInstance]);

  useEffect(() => {
    localStorage.setItem(AUTOPROFILE_DISMISSED_KEY, JSON.stringify(autoProfileDismissedByInstance));
  }, [autoProfileDismissedByInstance]);

  useEffect(() => {
    localStorage.setItem(DISCOVER_ADD_TRAY_STICKY_KEY, discoverAddTraySticky ? "1" : "0");
  }, [discoverAddTraySticky]);

  useEffect(() => {
    localStorage.setItem(APP_UPDATER_AUTOCHECK_KEY, appUpdaterAutoCheck ? "1" : "0");
  }, [appUpdaterAutoCheck]);
  useEffect(() => {
    localStorage.setItem(
      SUPPORT_BUNDLE_RAW_DEFAULT_KEY,
      supportBundleIncludeRawLogs ? "1" : "0"
    );
  }, [supportBundleIncludeRawLogs]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const version = await getVersion();
        if (!cancelled && version) {
          setAppVersion(String(version));
        }
      } catch {
        // ignore
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!installNotice) {
      lastInstallNoticeRef.current = null;
      return;
    }
    if (route !== "instance" || !selectedId) return;
    const message = installNotice.trim();
    if (!message) return;
    if (lastInstallNoticeRef.current === message) return;
    lastInstallNoticeRef.current = message;
    appendInstanceActivity(selectedId, [message]);
  }, [installNotice, route, selectedId]);

  useEffect(() => {
    if (!installNotice) return;
    const notice = installNotice;
    const timer = window.setTimeout(() => {
      setInstallNotice((current) => (current === notice ? null : current));
    }, INSTALL_NOTICE_AUTO_HIDE_MS);
    return () => window.clearTimeout(timer);
  }, [installNotice]);

  useEffect(() => {
    if (!error) return;
    const currentError = error;
    const timer = window.setTimeout(() => {
      setError((current) => (current === currentError ? null : current));
    }, TOP_ERROR_AUTO_HIDE_MS);
    return () => window.clearTimeout(timer);
  }, [error]);

  useEffect(() => {
    if (instances.length === 0) return;
    const nameById = new Map(instances.map((inst) => [inst.id, inst.name]));
    setScheduledUpdateEntriesByInstance((prev) => {
      let changed = false;
      const next: Record<string, ScheduledUpdateCheckEntry> = {};
      for (const [instanceId, entry] of Object.entries(prev)) {
        if (!nameById.has(instanceId)) {
          changed = true;
          continue;
        }
        const name = nameById.get(instanceId) ?? entry.instance_name;
        if (name !== entry.instance_name) {
          next[instanceId] = { ...entry, instance_name: name };
          changed = true;
        } else {
          next[instanceId] = entry;
        }
      }
      return changed ? next : prev;
    });
    setScheduledAppliedUpdatesByInstance((prev) => {
      let changed = false;
      const next: Record<string, ScheduledAppliedUpdateEntry> = {};
      for (const [instanceId, entry] of Object.entries(prev)) {
        if (!nameById.has(instanceId)) {
          changed = true;
          continue;
        }
        const name = nameById.get(instanceId) ?? entry.instance_name;
        if (name !== entry.instance_name) {
          next[instanceId] = { ...entry, instance_name: name };
          changed = true;
        } else {
          next[instanceId] = entry;
        }
      }
      return changed ? next : prev;
    });
  }, [instances]);

  useEffect(() => {
    if (scheduledUpdateLastRunAt) {
      localStorage.setItem("mpm.scheduledUpdates.lastRunAt", scheduledUpdateLastRunAt);
    } else {
      localStorage.removeItem("mpm.scheduledUpdates.lastRunAt");
    }
  }, [scheduledUpdateLastRunAt]);

  useEffect(() => {
    if (!updatePrefsSavedFlash) return;
    const timer = window.setTimeout(() => {
      setUpdatePrefsSavedFlash(false);
    }, 1600);
    return () => window.clearTimeout(timer);
  }, [updatePrefsSavedFlash]);

  useEffect(() => {
    if (!launcherSettings) return;
    if (instances.length === 0) return;
    const cadence = normalizeUpdateCheckCadence(updateCheckCadence);
    if (cadence === "off") return;
    const dueNow = () => {
      if (scheduledUpdateRunningRef.current || document.hidden) return;
      const lastMs = toTimestamp(scheduledUpdateLastRunAt ?? undefined);
      const intervalMs = updateCadenceIntervalMs(cadence);
      if (!Number.isFinite(lastMs) || Date.now() - lastMs >= intervalMs) {
        runScheduledUpdateChecks("scheduled").catch(() => null);
      }
    };
    dueNow();
    const timer = window.setInterval(dueNow, 60_000);
    const onVisibility = () => {
      if (!document.hidden) dueNow();
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibility);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    launcherSettings,
    instances,
    updateCheckCadence,
    scheduledUpdateLastRunAt,
    updateAutoApplyMode,
    updateApplyScope,
  ]);

  useEffect(() => {
    if (!scheduledUpdateBusy || !scheduledUpdateRunStartedAt) return;
    const tick = () => {
      const elapsedSeconds = Math.max(0, (Date.now() - scheduledUpdateRunStartedAt) / 1000);
      setScheduledUpdateRunElapsedSeconds(elapsedSeconds);
      if (
        scheduledUpdateRunCompleted > 0 &&
        scheduledUpdateRunCompleted < scheduledUpdateRunTotal
      ) {
        const remaining = scheduledUpdateRunTotal - scheduledUpdateRunCompleted;
        const avgSecondsPerInstance = elapsedSeconds / scheduledUpdateRunCompleted;
        setScheduledUpdateRunEtaSeconds(Math.max(0, avgSecondsPerInstance * remaining));
      }
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, [
    scheduledUpdateBusy,
    scheduledUpdateRunStartedAt,
    scheduledUpdateRunCompleted,
    scheduledUpdateRunTotal,
  ]);

  async function refreshCurseforgeApiStatus() {
    setCurseforgeApiBusy(true);
    setLauncherErr(null);
    try {
      const status = await getCurseforgeApiStatus();
      setCurseforgeApiStatus(status);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
    } finally {
      setCurseforgeApiBusy(false);
    }
  }

  async function refreshGithubTokenPoolStatus(): Promise<GithubTokenPoolStatus | null> {
    setGithubTokenPoolBusy(true);
    setLauncherErr(null);
    try {
      const status = await getGithubTokenPoolStatus();
      setGithubTokenPoolStatus(status);
      return status;
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setGithubTokenPoolStatus(null);
      return null;
    } finally {
      setGithubTokenPoolBusy(false);
    }
  }

  async function onSaveDevCurseforgeKey() {
    const key = devCurseforgeKeyDraft.trim();
    if (!key) {
      setLauncherErr("Enter a CurseForge API key first.");
      setDevCurseforgeNotice("Enter a CurseForge API key first.");
      setDevCurseforgeNoticeIsError(true);
      return;
    }
    setDevCurseforgeKeyBusy(true);
    setLauncherErr(null);
    setDevCurseforgeNotice(null);
    try {
      const message = await setDevCurseforgeApiKey({ key });
      setDevCurseforgeKeyDraft("");
      await refreshCurseforgeApiStatus();
      setDevCurseforgeNotice(message);
      setDevCurseforgeNoticeIsError(false);
      setInstallNotice(message);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setDevCurseforgeNotice(msg);
      setDevCurseforgeNoticeIsError(true);
    } finally {
      setDevCurseforgeKeyBusy(false);
    }
  }

  async function onClearDevCurseforgeKey() {
    setDevCurseforgeKeyBusy(true);
    setLauncherErr(null);
    setDevCurseforgeNotice(null);
    try {
      const message = await clearDevCurseforgeApiKey();
      setDevCurseforgeKeyDraft("");
      await refreshCurseforgeApiStatus();
      setDevCurseforgeNotice(message);
      setDevCurseforgeNoticeIsError(false);
      setInstallNotice(message);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setDevCurseforgeNotice(msg);
      setDevCurseforgeNoticeIsError(true);
    } finally {
      setDevCurseforgeKeyBusy(false);
    }
  }

  async function onSaveGithubTokenPool() {
    const tokens = githubTokenPoolDraft.trim();
    if (!tokens) {
      setLauncherErr("Paste one or more GitHub tokens first.");
      setGithubTokenPoolNotice("Paste one or more GitHub tokens first.");
      setGithubTokenPoolNoticeIsError(true);
      return;
    }
    setGithubTokenPoolBusy(true);
    setLauncherErr(null);
    setGithubTokenPoolNotice(null);
    try {
      const status = await setGithubTokenPool({ tokens });
      setGithubTokenPoolStatus(status);
      setGithubTokenPoolDraft("");
      setGithubTokenPoolNotice(
        `Saved ${status.total_tokens} GitHub token${status.total_tokens === 1 ? "" : "s"} in secure keychain storage.`
      );
      setGithubTokenPoolNoticeIsError(false);
      setInstallNotice(status.message);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setGithubTokenPoolNotice(msg);
      setGithubTokenPoolNoticeIsError(true);
    } finally {
      setGithubTokenPoolBusy(false);
    }
  }

  async function onValidateGithubTokenPool() {
    if (githubTokenPoolBusy) return;
    const draft = githubTokenPoolDraft.trim();
    if (draft) {
      await onSaveGithubTokenPool();
      return;
    }
    const status = await refreshGithubTokenPoolStatus();
    if (!status) return;
    if (status.configured) {
      setGithubTokenPoolNotice("Validated saved GitHub tokens from Keychain.");
      setGithubTokenPoolNoticeIsError(false);
    } else {
      setGithubTokenPoolNotice("No saved tokens yet. Paste one or more tokens, then click Validate.");
      setGithubTokenPoolNoticeIsError(false);
    }
  }

  async function onClearGithubTokenPool() {
    setGithubTokenPoolBusy(true);
    setLauncherErr(null);
    setGithubTokenPoolNotice(null);
    try {
      const status = await clearGithubTokenPool();
      setGithubTokenPoolStatus(status);
      setGithubTokenPoolDraft("");
      setGithubTokenPoolNotice("Cleared GitHub token pool from secure keychain storage.");
      setGithubTokenPoolNoticeIsError(false);
      setInstallNotice(status.message);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setGithubTokenPoolNotice(msg);
      setGithubTokenPoolNoticeIsError(true);
    } finally {
      setGithubTokenPoolBusy(false);
    }
  }

  useEffect(() => {
    if (route !== "dev") return;
    if (curseforgeApiStatus || curseforgeApiBusy) return;
    refreshCurseforgeApiStatus().catch(() => null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route]);

  useEffect(() => {
    if (route !== "settings" && route !== "dev") return;
    if (githubTokenPoolStatus || githubTokenPoolBusy) return;
    refreshGithubTokenPoolStatus().catch(() => null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route]);

  const sortedProjectVersions = useMemo(
    () =>
      [...projectVersions].sort(
        (a, b) =>
          new Date(b.date_published).getTime() - new Date(a.date_published).getTime()
      ),
    [projectVersions]
  );

  const latestProjectVersion = sortedProjectVersions[0] ?? null;
  const selectedLauncherAccountId = launcherSettings?.selected_account_id ?? null;
  const selectedLauncherAccount = useMemo(
    () => launcherAccounts.find((acct) => acct.id === selectedLauncherAccountId) ?? null,
    [launcherAccounts, selectedLauncherAccountId]
  );
  useEffect(() => {
    const selectedId = selectedLauncherAccountId ?? null;
    setAccountDiagnostics((prev) => {
      if (!prev) return prev;
      const cachedId = prev.selected_account_id ?? null;
      if (cachedId === selectedId) return prev;
      return null;
    });
  }, [selectedLauncherAccountId]);

  const accountDisplayName =
    accountDiagnostics?.minecraft_username ??
    accountDiagnostics?.account?.username ??
    selectedLauncherAccount?.username ??
    "Player";
  const accountSkinOptions = useMemo<AccountSkinOption[]>(() => {
    const out: AccountSkinOption[] = [];
    const seen = new Set<string>();
    const activeProfileSkin = (accountDiagnostics?.skins ?? []).find((skin) =>
      String(skin.state ?? "").trim().toLowerCase() === "active"
    );

    const pushOption = (next: AccountSkinOption) => {
      const key = String(next.apply_source ?? next.skin_url).trim().toLowerCase();
      if (!key || seen.has(key)) return;
      seen.add(key);
      out.push(next);
    };

    const primarySkin = String(accountDiagnostics?.skin_url ?? "").trim();
    if (primarySkin) {
      pushOption({
        id: "saved:primary",
        label: accountDiagnostics?.minecraft_username?.trim() || "Current skin",
        skin_url: primarySkin,
        apply_source: primarySkin,
        variant: activeProfileSkin?.variant ?? null,
        group: "saved",
        origin: "profile",
      });
    }

    for (const skin of accountDiagnostics?.skins ?? []) {
      const skinUrl = String(skin.url ?? "").trim();
      if (!skinUrl) continue;
      const variant = String(skin.variant ?? "").trim();
      const label = variant || "Saved skin";
      pushOption({
        id: `saved:${String(skin.id ?? skinUrl)}`,
        label,
        skin_url: skinUrl,
        apply_source: skinUrl,
        variant: variant || null,
        group: "saved",
        origin: "profile",
      });
    }

    for (const skin of customSkins) {
      const raw = String(skin.skin_path ?? "").trim();
      if (!raw) continue;
      const preview = String(skin.preview_data_url ?? "").trim();
      pushOption({
        id: `custom:${skin.id}`,
        label: skin.label || "Custom skin",
        skin_url: preview || toLocalIconSrc(raw) || raw,
        apply_source: raw,
        variant: null,
        preview_url: preview || null,
        group: "saved",
        origin: "custom",
      });
    }

    for (const preset of DEFAULT_SKIN_LIBRARY) {
      pushOption(preset);
    }

    return out;
  }, [accountDiagnostics?.minecraft_username, accountDiagnostics?.skin_url, accountDiagnostics?.skins, customSkins]);
  const savedSkinOptions = useMemo(
    () => accountSkinOptions.filter((skin) => skin.group === "saved"),
    [accountSkinOptions]
  );
  const defaultSkinOptions = useMemo(
    () => accountSkinOptions.filter((skin) => skin.group === "default"),
    [accountSkinOptions]
  );
  const selectedAccountSkin = useMemo(
    () =>
      accountSkinOptions.find((skin) => skin.id === selectedAccountSkinId) ??
      accountSkinOptions[0] ??
      null,
    [accountSkinOptions, selectedAccountSkinId]
  );
  const capeOptions = useMemo(
    () => [
      { id: "none", label: "No cape", url: null as string | null },
      ...(accountDiagnostics?.capes ?? []).map((cape, idx) => ({
        id: String(cape.id ?? `cape-${idx}`),
        label: String(cape.alias ?? "").trim() || `Cape ${idx + 1}`,
        url: String(cape.url ?? "").trim() || null,
      })),
    ],
    [accountDiagnostics?.capes]
  );
  const selectedAccountCape = useMemo(
    () => capeOptions.find((cape) => cape.id === selectedAccountCapeId) ?? capeOptions[0],
    [capeOptions, selectedAccountCapeId]
  );

  useEffect(() => {
    if (!accountSkinOptions.length) {
      setSelectedAccountSkinId(null);
      return;
    }
    if (!selectedAccountSkinId || !accountSkinOptions.some((skin) => skin.id === selectedAccountSkinId)) {
      setSelectedAccountSkinId(accountSkinOptions[0].id);
    }
  }, [accountSkinOptions, selectedAccountSkinId]);

  useEffect(() => {
    if (!capeOptions.length) {
      setSelectedAccountCapeId("none");
      return;
    }
    if (!capeOptions.some((cape) => cape.id === selectedAccountCapeId)) {
      setSelectedAccountCapeId(capeOptions[0].id);
    }
  }, [capeOptions, selectedAccountCapeId]);

  useEffect(() => {
    if (selectedAccountSkin?.origin === "custom") {
      setSkinRenameDraft(selectedAccountSkin.label ?? "");
    } else {
      setSkinRenameDraft("");
    }
  }, [selectedAccountSkin?.id, selectedAccountSkin?.label, selectedAccountSkin?.origin]);

  const libraryContextTarget = useMemo(
    () =>
      libraryContextMenu
        ? instances.find((inst) => inst.id === libraryContextMenu.instanceId) ?? null
        : null,
    [instances, libraryContextMenu]
  );
  const libraryContextMenuStyle = useMemo(() => {
    if (!libraryContextMenu || typeof window === "undefined") return null;
    const EDGE = 10;
    const MENU_WIDTH = 236;
    const MENU_HEIGHT = 326;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let left = libraryContextMenu.x;
    let top = libraryContextMenu.y;
    left = Math.min(left, Math.max(EDGE, vw - MENU_WIDTH - EDGE));
    left = Math.max(EDGE, left);
    if (top + MENU_HEIGHT > vh - EDGE) {
      top = Math.max(EDGE, vh - MENU_HEIGHT - EDGE);
    }
    return {
      left,
      top,
      width: MENU_WIDTH,
    };
  }, [libraryContextMenu]);

  const projectLoaderFacets = useMemo(() => {
    const set = new Set<string>();
    for (const v of sortedProjectVersions) {
      for (const loaderName of v.loaders) {
        if (loaderName) set.add(loaderName);
      }
      if (set.size >= 9) break;
    }
    return Array.from(set);
  }, [sortedProjectVersions]);

  const projectGameVersionFacets = useMemo(() => {
    const set = new Set<string>();
    for (const v of sortedProjectVersions) {
      for (const gameVersion of v.game_versions) {
        if (gameVersion) set.add(gameVersion);
      }
      if (set.size >= 10) break;
    }
    return Array.from(set);
  }, [sortedProjectVersions]);

  const latestPrimaryFile = useMemo(() => {
    if (!latestProjectVersion) return null;
    return (
      latestProjectVersion.files.find((f) => f.primary) ??
      latestProjectVersion.files[0] ??
      null
    );
  }, [latestProjectVersion]);

  const projectPageUrl = projectOpen
    ? `https://modrinth.com/mod/${projectOpen.slug || projectOpen.id}`
    : null;

  const changelogVersions = useMemo(
    () =>
      sortedProjectVersions
        .filter((v) => Boolean(toReadableBody(v.changelog).trim()))
        .slice(0, 10),
    [sortedProjectVersions]
  );

  function closeProjectOverlays() {
    githubDetailRequestIdRef.current += 1;
    setProjectBusy(false);
    setProjectOpen(null);
    setProjectVersions([]);
    setProjectMembers([]);
    setProjectDetailTab("overview");
    setProjectCopyNotice(null);
    setProjectErr(null);
    setCurseforgeBusy(false);
    setCurseforgeOpen(null);
    setGithubBusy(false);
    setGithubOpen(null);
    setGithubDetail(null);
    setGithubErr(null);
    setGithubDetailTab("overview");
    setProjectOpenContentType("mods");
    setCurseforgeOpenContentType("mods");
    setCurseforgeDetailTab("overview");
    setCurseforgeErr(null);
  }

  function normalizeImportedPreset(raw: any): UserPreset | null {
    if (!raw || typeof raw !== "object") return null;
    const name = typeof raw.name === "string" && raw.name.trim() ? raw.name.trim() : "Imported preset";
    const entriesRaw = Array.isArray(raw.entries) ? raw.entries : [];
    const entries: UserPresetEntry[] = entriesRaw
      .filter((entry: any) => entry && typeof entry === "object")
      .map((entry: any) => {
        const normalizedSource = parseDiscoverSource(entry.source);
        const project_id = String(entry.project_id ?? "").trim();
        const title = String(entry.title ?? project_id ?? "").trim();
        const content_type = String(entry.content_type ?? "mods").trim().toLowerCase();
        const normalizedContentType =
          content_type === "resourcepacks" || content_type === "resourcepack"
            ? "resourcepacks"
            : content_type === "shaderpacks" || content_type === "shaderpack" || content_type === "shaders"
              ? "shaderpacks"
              : content_type === "datapacks" || content_type === "datapack"
                ? "datapacks"
                : content_type === "modpacks" || content_type === "modpack"
                  ? "modpacks"
                  : "mods";
        const targetScope = normalizedContentType === "datapacks" ? "world" : "instance";
        const targetWorlds = Array.isArray(entry.target_worlds)
          ? entry.target_worlds
            .map((w: any) => String(w ?? "").trim())
            .filter((w: string) => Boolean(w))
          : [];
        if (!normalizedSource || !project_id) return null;
        return {
          source: normalizedSource,
          project_id,
          title: title || project_id,
          content_type: normalizedContentType,
          pinned_version: typeof entry.pinned_version === "string" ? entry.pinned_version : null,
          target_scope: targetScope,
          target_worlds: targetWorlds,
          enabled: entry.enabled !== false,
        } as UserPresetEntry;
      })
      .filter((entry): entry is UserPresetEntry => Boolean(entry));

    if (entries.length === 0) return null;

    const baseId = typeof raw.id === "string" && raw.id.trim() ? raw.id.trim() : `preset_${Date.now()}`;
    return {
      id: baseId,
      name,
      created_at:
        typeof raw.created_at === "string" && raw.created_at.trim()
          ? raw.created_at
          : new Date().toISOString(),
      source_instance_id:
        typeof raw.source_instance_id === "string" ? raw.source_instance_id : "imported",
      source_instance_name:
        typeof raw.source_instance_name === "string" && raw.source_instance_name.trim()
          ? raw.source_instance_name
          : "Imported",
      entries,
      settings: {
        ...defaultPresetSettings(),
        ...(raw.settings && typeof raw.settings === "object" ? raw.settings : {}),
      },
    };
  }

  async function runSearch(newOffset: number) {
    const query = q.trim();
    setDiscoverErr(null);
    setDiscoverBusy(true);
    try {
      const res = await searchDiscoverContent({
        query,
        loaders: discoverContentType === "mods" ? filterLoaders : [],
        gameVersion: filterVersion,
        categories: filterCategories,
        index,
        limit,
        offset: newOffset,
        sources: effectiveDiscoverSources,
        contentType: discoverContentType,
      });
      setHits(res.hits);
      setTotalHits(res.total_hits);
      setOffset(res.offset);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      const lower = msg.toLowerCase();
      if (
        discoverSourceValue === "curseforge" &&
        (lower.includes("curseforge is not configured for this build") ||
          lower.includes("curseforge api key is not configured for this build"))
      ) {
        setDiscoverErr(
          `${msg} For local dev, export MPM_CURSEFORGE_API_KEY and restart tauri:dev. Release builds should include the injected key.`
        );
      } else {
        setDiscoverErr(msg);
      }
    } finally {
      setDiscoverBusy(false);
    }
  }

  useEffect(() => {
    if (route !== "discover") return;
    runSearch(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route, index, limit, filterLoaders, filterVersion, filterCategories, discoverSourceValue, effectiveDiscoverSources, discoverContentType]);

  useEffect(() => {
    if (!discoverAddContext) {
      discoverAddContextKeyRef.current = null;
      setDiscoverAddTrayItems([]);
      setDiscoverAddTrayExpanded(true);
      return;
    }
    const nextKey = `${discoverAddContext.modpackId}:${discoverAddContext.layerId ?? ""}`;
    if (discoverAddContextKeyRef.current !== nextKey) {
      setDiscoverAddTrayItems([]);
      setDiscoverAddTrayExpanded(true);
    }
    discoverAddContextKeyRef.current = nextKey;
  }, [discoverAddContext]);

  async function runTemplateSearch(newOffset: number, queryOverride?: string) {
    setTemplateErr(null);
    setTemplateBusy(true);
    try {
      const res = await searchDiscoverContent({
        query: queryOverride ?? templateQueryDebounced,
        loaders: [],
        gameVersion: filterVersion,
        categories: filterCategories,
        index,
        limit,
        offset: newOffset,
        sources: discoverRequestSources(templateSource),
        contentType: templateType as DiscoverContentType,
      });
      setTemplateHits(res.hits);
      setTemplateTotalHits(res.total_hits);
      setTemplateOffset(res.offset);
    } catch (e: any) {
      setTemplateErr(e?.toString?.() ?? String(e));
      setTemplateHits([]);
      setTemplateTotalHits(0);
    } finally {
      setTemplateBusy(false);
    }
  }

  useEffect(() => {
    if (route !== "modpacks" || modpacksStudioTab !== "templates") return;
    runTemplateSearch(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route, modpacksStudioTab, templateQueryDebounced, templateSource, templateType, filterVersion, filterCategories, index, limit]);

  // Deprecated legacy Creator bridge helpers (kept until compatibility window ends).
  function ensureCreatorDraft(inst: Instance | null): UserPreset {
    if (creatorDraft) return creatorDraft;
    const draft: UserPreset = {
      id: `preset_${Date.now()}`,
      name: inst ? `${inst.name} custom preset` : "Custom preset",
      created_at: new Date().toISOString(),
      source_instance_id: inst?.id ?? "custom",
      source_instance_name: inst?.name ?? "Custom",
      entries: [],
      settings: defaultPresetSettings(),
    };
    setCreatorDraft(draft);
    return draft;
  }

  function addEntryToCreator(entry: UserPresetEntry, inst: Instance | null) {
    const base = ensureCreatorDraft(inst);
    const existingKey = `${entry.source}:${entry.project_id}:${entry.content_type}:${(entry.target_worlds ?? []).join("|")}`;
    const mergedEntries = [
      ...base.entries.filter((e) => {
        const k = `${e.source}:${e.project_id}:${e.content_type}:${(e.target_worlds ?? []).join("|")}`;
        return k !== existingKey;
      }),
      {
        ...entry,
        enabled: entry.enabled !== false,
        target_scope: entry.content_type === "datapacks" ? "world" : "instance",
      },
    ];
    const next = {
      ...base,
      // Preserve creator intent/order; users can manually reorder entries in the studio.
      entries: mergedEntries,
    };
    setCreatorDraft(next);
  }

  function addHitToCreator(hit: DiscoverSearchHit, inst: Instance | null) {
    const contentType =
      hit.content_type === "modpacks"
        ? "modpacks"
        : (hit.content_type as DiscoverContentType) || "mods";
    addEntryToCreator(
      {
        source: normalizeDiscoverSource(hit.source),
        project_id: hit.project_id,
        title: hit.title,
        content_type: contentType,
        pinned_version: null,
        target_scope: contentType === "datapacks" ? "world" : "instance",
        target_worlds:
          contentType === "datapacks"
            ? (instanceWorlds.length ? [instanceWorlds[0].id] : [])
            : [],
        enabled: contentType !== "modpacks",
      },
      inst
    );
    setInstallNotice(`Added "${hit.title}" to creator draft.`);
  }

  async function importTemplateFromHit(hit: DiscoverSearchHit, inst: Instance | null) {
    setPresetBusy(true);
    setError(null);
    try {
      if (hit.content_type === "modpacks") {
        const templateSource = normalizeDiscoverSource(hit.source);
        if (templateSource === "github" || templateSource === "all") {
          throw new Error("GitHub modpacks are not supported for template import yet.");
        }
        const preset = await importProviderModpackTemplate({
          source: templateSource,
          projectId: hit.project_id,
          projectTitle: hit.title,
        });
        setCreatorDraft(preset);
        setInstallNotice(`Imported template "${preset.name}" with ${preset.entries.length} entries.`);
      } else {
        addHitToCreator(hit, inst);
      }
      setModpacksStudioTab("creator");
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setPresetBusy(false);
    }
  }

  function updateCreatorDraft(mutator: (current: UserPreset) => UserPreset) {
    const inst = instances.find((i) => i.id === selectedId) ?? null;
    const current = creatorDraft ?? ensureCreatorDraft(inst);
    const next = mutator(current);
    setCreatorDraft({
      ...next,
      settings: {
        ...defaultPresetSettings(),
        ...(next.settings ?? {}),
      },
    });
  }

  function onAddCreatorBlankEntry(inst: Instance | null) {
    const base = ensureCreatorDraft(inst);
    const next = {
      ...base,
      entries: [
        ...base.entries,
        {
          source: "modrinth",
          project_id: "",
          title: "Untitled entry",
          content_type: "mods",
          pinned_version: null,
          target_scope: "instance",
          target_worlds: [],
          enabled: true,
        },
      ],
    };
    setCreatorDraft(next);
  }

  function moveCreatorEntry(index: number, direction: -1 | 1) {
    if (!creatorDraft) return;
    const nextIndex = index + direction;
    if (nextIndex < 0 || nextIndex >= creatorDraft.entries.length) return;
    updateCreatorDraft((current) => {
      const entries = [...current.entries];
      const [item] = entries.splice(index, 1);
      entries.splice(nextIndex, 0, item);
      return { ...current, entries };
    });
  }

  async function onSaveCreatorToPresets() {
    if (!creatorDraft) return;
    const cleanName = creatorDraft.name.trim() || "Custom preset";
    const next: UserPreset = {
      ...creatorDraft,
      id: creatorDraft.id?.trim() ? creatorDraft.id : `preset_${Date.now()}`,
      name: cleanName,
      created_at: creatorDraft.created_at || new Date().toISOString(),
      source_instance_id: creatorDraft.source_instance_id || "custom",
      source_instance_name: creatorDraft.source_instance_name || "Custom",
      settings: {
        ...defaultPresetSettings(),
        ...(creatorDraft.settings ?? {}),
      },
      entries: (creatorDraft.entries ?? []).filter((e) => Boolean(e.project_id?.trim())),
    };
    if (!next.entries.length) {
      setError("Creator draft has no valid entries yet.");
      return;
    }
    setPresets((prev) => {
      const without = prev.filter((p) => p.id !== next.id);
      return [next, ...without];
    });
    setInstallNotice(`Saved "${next.name}" (${next.entries.length} entries).`);
  }

  async function openProject(id: string, contentType?: DiscoverContentType) {
    closeProjectOverlays();
    setProjectErr(null);
    setProjectBusy(true);
    setProjectVersions([]);
    setProjectMembers([]);
    setProjectDetailTab("overview");
    setProjectCopyNotice(null);
    setProjectOpenContentType(contentType ?? "mods");
    try {
      const [p, versionsRes, membersRes] = await Promise.all([
        getProject(id),
        getProjectVersions(id).catch(() => [] as ProjectVersion[]),
        getProjectMembers(id).catch(() => [] as ProjectMember[]),
      ]);
      setProjectOpen(p);
      setProjectVersions(versionsRes);
      setProjectMembers(membersRes);
    } catch (e: any) {
      setProjectErr(e?.toString?.() ?? String(e));
    } finally {
      setProjectBusy(false);
    }
  }

  async function openCurseforgeProject(projectId: string, contentType?: DiscoverContentType) {
    closeProjectOverlays();
    setCurseforgeErr(null);
    setCurseforgeBusy(true);
    setCurseforgeDetailTab("overview");
    setCurseforgeOpenContentType(contentType ?? "mods");
    try {
      const detail = await getCurseforgeProjectDetail({
        projectId,
        contentType: contentType ?? "mods",
      });
      setCurseforgeOpen(detail);
    } catch (e: any) {
      setCurseforgeErr(e?.toString?.() ?? String(e));
    } finally {
      setCurseforgeBusy(false);
    }
  }

  async function openGithubProject(hit: DiscoverSearchHit, contentType?: DiscoverContentType) {
    closeProjectOverlays();
    const requestId = githubDetailRequestIdRef.current + 1;
    githubDetailRequestIdRef.current = requestId;
    setProjectOpenContentType(contentType ?? "mods");
    setGithubBusy(true);
    setGithubErr(null);
    setGithubDetail(null);
    setGithubDetailTab("overview");
    setGithubOpen(hit);
    try {
      const parsedProjectId =
        parseGithubProjectId(hit.project_id) ??
        parseGithubProjectId(hit.external_url) ??
        parseGithubProjectId(hit.slug ? `${hit.author}/${hit.slug}` : "");
      if (!parsedProjectId) {
        if (githubDetailRequestIdRef.current === requestId) {
          setGithubErr("GitHub repository id is missing for this entry.");
        }
        return;
      }
      const detail = await getGithubProjectDetail({ projectId: parsedProjectId });
      if (githubDetailRequestIdRef.current === requestId) {
        setGithubDetail(detail);
      }
    } catch (e: any) {
      if (githubDetailRequestIdRef.current === requestId) {
        setGithubErr(e?.toString?.() ?? String(e));
      }
    } finally {
      if (githubDetailRequestIdRef.current === requestId) {
        setGithubBusy(false);
      }
    }
  }

  async function fetchInstalledIconFromProvider(
    source: string,
    projectId: string,
    contentType: DiscoverContentType
  ): Promise<string | null> {
    const normalizedSource = normalizeProviderSource(source);
    if (normalizedSource === "modrinth") {
      const cleanProjectId = String(projectId ?? "").trim();
      if (!cleanProjectId) return null;
      const project = await getProject(cleanProjectId);
      const icon = String(project?.icon_url ?? "").trim();
      return icon || null;
    }
    if (normalizedSource === "curseforge") {
      const parsedId = parseCurseforgeProjectId(projectId);
      if (!parsedId) return null;
      const detail = await getCurseforgeProjectDetail({ projectId: parsedId, contentType });
      const icon = String(detail?.icon_url ?? "").trim();
      return icon || null;
    }
    if (normalizedSource === "github") {
      const parsed = parseGithubProjectId(projectId);
      if (!parsed) return null;
      const owner = parsed.split("/")[0]?.trim();
      if (!owner) return null;
      return `https://github.com/${encodeURIComponent(owner)}.png?size=96`;
    }
    return null;
  }

  async function resolveInstalledModIcon(mod: InstalledMod): Promise<string | null> {
    const cacheKey = installedIconCacheKey(mod);
    if (installedIconFailedByKey[cacheKey]) return null;
    const cached = String(installedIconCache[cacheKey] ?? "").trim();
    if (cached) return cached;

    const inFlight = installedIconFetchesRef.current.get(cacheKey);
    if (inFlight) return inFlight;

    const task = (async () => {
      const detailType = installedContentTypeToDiscover(mod.content_type);
      const candidates = installedProviderCandidates(mod);
      const orderedTargets: Array<{ source: string; projectId: string }> = [];
      const seen = new Set<string>();
      const push = (source: string, projectId: string) => {
        const normalizedSource = normalizeProviderSource(source);
        if (
          normalizedSource !== "modrinth" &&
          normalizedSource !== "curseforge" &&
          normalizedSource !== "github"
        ) {
          return;
        }
        const cleanProjectId = String(projectId ?? "").trim();
        if (!cleanProjectId) return;
        const key = `${normalizedSource}:${cleanProjectId}`;
        if (seen.has(key)) return;
        seen.add(key);
        orderedTargets.push({ source: normalizedSource, projectId: cleanProjectId });
      };

      push(mod.source, preferredProjectIdForProvider(mod, mod.source));
      for (const candidate of candidates) {
        push(candidate.source, candidate.project_id);
      }

      for (const target of orderedTargets) {
        try {
          const icon = await fetchInstalledIconFromProvider(
            target.source,
            target.projectId,
            detailType
          );
          if (!icon) continue;
          setInstalledIconCache((prev) => {
            if (prev[cacheKey] === icon) return prev;
            return {
              ...prev,
              [cacheKey]: icon,
            };
          });
          return icon;
        } catch {
          // try the next provider candidate
        }
      }

      return null;
    })().finally(() => {
      installedIconFetchesRef.current.delete(cacheKey);
    });

    installedIconFetchesRef.current.set(cacheKey, task);
    return task;
  }

  function requestInstalledModIcon(mod: InstalledMod) {
    const cacheKey = installedIconCacheKey(mod);
    if (installedIconFailedByKey[cacheKey]) return;
    if (installedIconCache[cacheKey]) return;
    void resolveInstalledModIcon(mod);
  }

  function markInstalledModIconFailed(mod: InstalledMod) {
    const cacheKey = installedIconCacheKey(mod);
    setInstalledIconFailedByKey((prev) => {
      if (prev[cacheKey]) return prev;
      return {
        ...prev,
        [cacheKey]: true,
      };
    });
    setInstalledIconCache((prev) => {
      if (!prev[cacheKey]) return prev;
      const next = { ...prev };
      delete next[cacheKey];
      return next;
    });
  }

  async function openInstalledModDetails(
    mod: InstalledMod,
    options?: { autoResolveAttempted?: boolean }
  ) {
    const candidates = installedProviderCandidates(mod);
    const detailType = installedContentTypeToDiscover(mod.content_type);
    const currentSource = normalizeProviderSource(mod.source);
    const order: Array<"modrinth" | "curseforge"> =
      currentSource === "modrinth"
        ? ["modrinth", "curseforge"]
        : currentSource === "curseforge"
          ? ["curseforge", "modrinth"]
          : ["modrinth", "curseforge"];

    for (const source of order) {
      const candidate =
        candidates.find((item) => normalizeProviderSource(item.source) === source) ?? null;
      const projectId = String(
        candidate?.project_id ??
          (normalizeProviderSource(mod.source) === source ? mod.project_id : "")
      ).trim();
      if (!projectId) continue;
      if (source === "modrinth") {
        await openProject(projectId, detailType);
        return;
      }
      const parsedCfId = parseCurseforgeProjectId(projectId);
      if (!parsedCfId) continue;
      await openCurseforgeProject(parsedCfId, detailType);
      return;
    }

    const githubCandidate =
      candidates.find((item) => normalizeProviderSource(item.source) === "github") ?? null;
    const githubProjectId = parseGithubProjectId(
      githubCandidate?.project_id ??
        (normalizeProviderSource(mod.source) === "github" ? mod.project_id : "")
    );
    if (githubProjectId) {
      await openGithubProject(
        {
          source: "github",
          project_id: `gh:${githubProjectId}`,
          title: mod.name,
          description: mod.filename,
          author: githubProjectId.split("/")[0] ?? "Unknown",
          downloads: 0,
          follows: 0,
          icon_url: `https://github.com/${encodeURIComponent(githubProjectId.split("/")[0] ?? "")}.png?size=96`,
          categories: [],
          versions: [mod.version_number ?? ""].filter(Boolean),
          date_modified: "",
          content_type: installedContentTypeToDiscover(mod.content_type),
          slug: githubProjectId.split("/")[1] ?? null,
          external_url: `https://github.com/${githubProjectId}`,
          confidence: null,
          reason: "Installed from GitHub release metadata.",
          install_state: String(githubCandidate?.version_id ?? "")
            .trim()
            .toLowerCase()
            .startsWith("gh_release:")
            ? "ready"
            : "checking",
          install_summary: String(githubCandidate?.version_id ?? "")
            .trim()
            .toLowerCase()
            .startsWith("gh_release:")
            ? null
            : "GitHub install compatibility will be checked when you open this project.",
        },
        detailType
      );
      return;
    }

    const canAttemptAutoResolve =
      !options?.autoResolveAttempted &&
      Boolean(selectedId) &&
      normalizeProviderSource(mod.source) === "local";
    if (canAttemptAutoResolve && selectedId) {
      await runLocalResolverBackfill(selectedId, "all", {
        silent: true,
        refreshListAfterResolve: true,
        contentTypes: [normalizeCreatorEntryType(mod.content_type)],
      });
      const refreshed = await listInstalledMods(selectedId).catch(() => null);
      if (refreshed) {
        applyInstalledModsForInstance(selectedId, refreshed);
        const updated = refreshed.find((entry) => entry.version_id === mod.version_id) ?? null;
        if (updated) {
          const hasProviderNow =
            normalizeProviderSource(updated.source) !== "local" ||
            installedProviderCandidates(updated).some((candidate) => {
              const source = normalizeProviderSource(candidate.source);
              return source === "modrinth" || source === "curseforge" || source === "github";
            });
          if (hasProviderNow) {
            await openInstalledModDetails(updated, { autoResolveAttempted: true });
            return;
          }
        }
      }
    }

    setInstallNotice(
      `No provider details are available for ${mod.name} yet. Resolve local sources first or attach a GitHub repository manually.`
    );
  }

  function beginAttachInstalledModGithubRepo(inst: Instance, mod: InstalledMod) {
    if (normalizeCreatorEntryType(mod.content_type) !== "mods") {
      setModsErr("Manual GitHub repo attach is currently supported for mods only.");
      return;
    }
    const activeSource = normalizeProviderSource(mod.source);
    if (activeSource === "modrinth" || activeSource === "curseforge") {
      setModsErr(
        "Manual GitHub repo attach is available only for local or existing GitHub entries."
      );
      return;
    }
    const existingGithubCandidate =
      installedProviderCandidates(mod).find(
        (candidate) => normalizeProviderSource(candidate.source) === "github"
      ) ?? null;
    const suggestedRepo =
      parseGithubProjectId(existingGithubCandidate?.project_id ?? mod.project_id) ?? "";
    setGithubAttachTarget({
      instanceId: inst.id,
      instanceName: inst.name,
      mod,
    });
    setGithubAttachInput(suggestedRepo);
    setGithubAttachErr(null);
  }

  async function submitAttachInstalledModGithubRepo() {
    const target = githubAttachTarget;
    if (!target) return;
    const targetEntryKey = installedEntryUiKey(target.mod);
    const githubRepo = parseGithubProjectId(githubAttachInput);
    if (!githubRepo) {
      setGithubAttachErr("Invalid GitHub repository. Use owner/repo or a GitHub repository URL.");
      return;
    }

    setGithubAttachBusyVersion(targetEntryKey);
    setModsErr(null);
    setGithubAttachErr(null);
    try {
      const updated = await attachInstalledModGithubRepo({
        instanceId: target.instanceId,
        versionId: target.mod.version_id,
        contentType: target.mod.content_type,
        filename: target.mod.filename,
        githubRepo,
        activate: true,
      });
      applyInstalledModsForInstance(target.instanceId, (prev) =>
        prev.map((entry) => (installedEntryUiKey(entry) === targetEntryKey ? updated : entry))
      );
      requestInstalledModIcon(updated);
      if (normalizeProviderSource(updated.source) === "github") {
        setInstallNotice(`Attached GitHub repo ${githubRepo} to ${updated.name}.`);
      } else {
        setInstallNotice(
          `Saved GitHub repo ${githubRepo} for ${updated.name}. Provider activation is pending verification.`
        );
      }
      setGithubAttachTarget(null);
      setGithubAttachInput("");
      setGithubAttachErr(null);
    } catch (e: any) {
      const message = e?.toString?.() ?? String(e);
      setModsErr(message);
      setGithubAttachErr(message);
    } finally {
      setGithubAttachBusyVersion(null);
    }
  }

  async function onSetInstalledModProvider(inst: Instance, mod: InstalledMod, source: string) {
    const normalizedSource = normalizeProviderSource(source);
    if (
      normalizedSource !== "modrinth" &&
      normalizedSource !== "curseforge" &&
      normalizedSource !== "github"
    ) {
      return;
    }
    if (normalizeProviderSource(mod.source) === normalizedSource) return;
    const modEntryKey = installedEntryUiKey(mod);
    const busyKey = `${modEntryKey}:${normalizedSource}`;
    setProviderSwitchBusyKey(busyKey);
    setModsErr(null);
    try {
      const updated = await setInstalledModProvider({
        instanceId: inst.id,
        versionId: mod.version_id,
        contentType: mod.content_type,
        filename: mod.filename,
        source: normalizedSource,
      });
      applyInstalledModsForInstance(inst.id, (prev) =>
        prev.map((entry) => (installedEntryUiKey(entry) === modEntryKey ? updated : entry))
      );
      requestInstalledModIcon(updated);
      setInstallNotice(`Set ${mod.name} provider to ${providerSourceLabel(normalizedSource)}.`);
    } catch (e: any) {
      setModsErr(e?.toString?.() ?? String(e));
    } finally {
      setProviderSwitchBusyKey(null);
    }
  }

  async function onInstallMissingDependencies(inst: Instance, mod: InstalledMod) {
    if (dependencyInstallBusyVersion) return;
    const isModsEntry = normalizeCreatorEntryType(mod.content_type) === "mods";
    if (!isModsEntry) return;
    setDependencyInstallBusyVersion(installedEntryUiKey(mod));
    setModsErr(null);
    try {
      const dependencyHints = extractDependencyHints(mod);
      const activeSource = effectiveInstalledProviderSource(mod);
      const activeProjectId = String(preferredProjectIdForProvider(mod, activeSource) ?? "").trim();
      if (
        dependencyHints.length === 0 &&
        activeProjectId &&
        (activeSource === "modrinth" || activeSource === "curseforge" || activeSource === "github")
      ) {
        if (activeSource === "curseforge") {
          await installCurseforgeMod({
            instanceId: inst.id,
            projectId: activeProjectId,
            projectTitle: mod.name,
          });
        } else if (activeSource === "modrinth") {
          await installModrinthMod({
            instanceId: inst.id,
            projectId: activeProjectId,
            projectTitle: mod.name,
          });
        } else {
          await installDiscoverContent({
            instanceId: inst.id,
            source: activeSource,
            projectId: activeProjectId,
            projectTitle: mod.name,
            contentType: "mods",
          });
        }
        await refreshInstalledMods(inst.id);
        await refreshSnapshots(inst.id).catch(() => null);
        setInstallNotice(`Dependency sync complete for ${mod.name}. Required dependencies were installed when available.`);
        return;
      }

      if (dependencyHints.length === 0) {
        setInstallNotice(`No installable dependency ids were found for ${mod.name}.`);
        return;
      }
      const baselineEntries =
        installedModsInstanceId === inst.id
          ? installedMods
          : await listInstalledMods(inst.id).catch(() => [] as InstalledMod[]);
      const installedIds = new Set<string>();
      for (const entry of baselineEntries) {
        const projectId = String(entry.project_id ?? "").trim().toLowerCase();
        if (projectId) installedIds.add(projectId);
        for (const id of entry.local_analysis?.mod_ids ?? []) {
          const normalized = String(id ?? "").trim().toLowerCase();
          if (normalized) installedIds.add(normalized);
        }
      }
      const missingHints = dependencyHints.filter((id) => !installedIds.has(id)).slice(0, 8);
      if (missingHints.length === 0) {
        setInstallNotice(`Dependencies for ${mod.name} already appear installed.`);
        return;
      }
      let installedCount = 0;
      const failed: string[] = [];
      for (const depId of missingHints) {
        try {
          await installModrinthMod({
            instanceId: inst.id,
            projectId: depId,
            projectTitle: depId,
          });
          installedCount += 1;
        } catch {
          failed.push(depId);
        }
      }
      await refreshInstalledMods(inst.id);
      await refreshSnapshots(inst.id).catch(() => null);
      if (installedCount > 0) {
        const failSuffix =
          failed.length > 0
            ? ` ${failed.length} hint${failed.length === 1 ? "" : "s"} could not be resolved automatically.`
            : "";
        setInstallNotice(
          `Installed ${installedCount} dependency mod${installedCount === 1 ? "" : "s"} for ${mod.name}.${failSuffix}`
        );
      } else {
        setModsErr(`Could not auto-install dependencies for ${mod.name}. Try switching provider metadata first.`);
      }
    } catch (e: any) {
      const message = e?.toString?.() ?? String(e);
      setModsErr(message);
      setError(message);
    } finally {
      setDependencyInstallBusyVersion(null);
    }
  }

  async function onToggleInstalledModPin(inst: Instance, mod: InstalledMod) {
    if (pinBusyVersion) return;
    const modEntryKey = installedEntryUiKey(mod);
    setPinBusyVersion(modEntryKey);
    setModsErr(null);
    try {
      const updated = await setInstalledModPin({
        instanceId: inst.id,
        versionId: mod.version_id,
        contentType: mod.content_type,
        filename: mod.filename,
        pin: mod.pinned_version ? "" : undefined,
      });
      applyInstalledModsForInstance(inst.id, (prev) =>
        prev.map((entry) => (installedEntryUiKey(entry) === modEntryKey ? updated : entry))
      );
      setInstallNotice(
        updated.pinned_version
          ? `Pinned ${updated.name} to ${updated.pinned_version}.`
          : `Unpinned ${updated.name}.`
      );
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setModsErr(msg);
      setError(msg);
    } finally {
      setPinBusyVersion(null);
    }
  }

  async function refreshInstanceHistory(instanceId: string, options?: { silent?: boolean }) {
    if (!instanceId) return;
    if (instanceHistoryRefreshInFlightRef.current[instanceId]) return;
    instanceHistoryRefreshInFlightRef.current[instanceId] = true;
    setInstanceHistoryBusyById((prev) => ({ ...prev, [instanceId]: true }));
    if (!options?.silent) {
      setLauncherErr(null);
    }
    try {
      const events = await listInstanceHistoryEvents({
        instanceId,
        limit: 80,
      });
      setInstanceHistoryById((prev) => ({ ...prev, [instanceId]: events ?? [] }));
    } catch (e: any) {
      if (!options?.silent) {
        setLauncherErr(e?.toString?.() ?? String(e));
      }
    } finally {
      setInstanceHistoryBusyById((prev) => ({ ...prev, [instanceId]: false }));
      delete instanceHistoryRefreshInFlightRef.current[instanceId];
    }
  }

  async function loadFullHistoryPage(instanceId: string, options?: { reset?: boolean }) {
    if (!instanceId) return;
    if (fullHistoryBusyByInstance[instanceId]) return;
    const reset = options?.reset === true;
    const beforeAt = reset ? null : fullHistoryBeforeAtByInstance[instanceId] ?? null;
    setFullHistoryBusyByInstance((prev) => ({ ...prev, [instanceId]: true }));
    try {
      const page = await listInstanceHistoryEvents({
        instanceId,
        limit: FULL_HISTORY_PAGE_SIZE,
        beforeAt: beforeAt || undefined,
      });
      const rows = Array.isArray(page) ? page : [];
      setFullHistoryByInstance((prev) => {
        const base = reset ? [] : prev[instanceId] ?? [];
        const merged = [...base, ...rows];
        const deduped = new Map<string, InstanceHistoryEvent>();
        for (const item of merged) {
          const key = `${item.id}:${item.at}:${item.kind}:${item.summary}`;
          if (!deduped.has(key)) deduped.set(key, item);
        }
        return {
          ...prev,
          [instanceId]: Array.from(deduped.values()).sort((a, b) => b.at.localeCompare(a.at)),
        };
      });
      const oldest = rows[rows.length - 1];
      setFullHistoryBeforeAtByInstance((prev) => ({
        ...prev,
        [instanceId]: oldest?.at ?? prev[instanceId] ?? null,
      }));
      setFullHistoryHasMoreByInstance((prev) => ({
        ...prev,
        [instanceId]: rows.length >= FULL_HISTORY_PAGE_SIZE,
      }));
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setFullHistoryBusyByInstance((prev) => ({ ...prev, [instanceId]: false }));
    }
  }

  function openFullHistory(instanceId: string) {
    if (!instanceId) return;
    setFullHistoryModalInstanceId(instanceId);
    void loadFullHistoryPage(instanceId, { reset: true });
  }

  async function refreshQuickPlayServers(options?: { silent?: boolean }) {
    setQuickPlayBusy(true);
    if (!options?.silent) {
      setQuickPlayErr(null);
    }
    try {
      const rows = await listQuickPlayServers();
      setQuickPlayServers(Array.isArray(rows) ? rows : []);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setQuickPlayErr(msg);
      if (!options?.silent) setError(msg);
    } finally {
      setQuickPlayBusy(false);
    }
  }

  async function onSaveQuickPlayServer(currentInstance?: Instance | null) {
    const name = quickPlayDraftName.trim();
    const host = quickPlayDraftHost.trim();
    if (!name || !host) {
      setQuickPlayErr("Quick Play needs a server name and host.");
      return;
    }
    const parsedPort = Number.parseInt(quickPlayDraftPort.trim() || "25565", 10);
    if (!Number.isFinite(parsedPort) || parsedPort < 1 || parsedPort > 65535) {
      setQuickPlayErr("Quick Play port must be between 1 and 65535.");
      return;
    }
    setQuickPlayBusy(true);
    setQuickPlayErr(null);
    try {
      const boundInstanceId =
        quickPlayDraftBoundInstanceId === "none"
          ? currentInstance?.id ?? null
          : quickPlayDraftBoundInstanceId;
      const rows = await upsertQuickPlayServer({
        name,
        host,
        port: parsedPort,
        boundInstanceId: boundInstanceId || null,
      });
      setQuickPlayServers(Array.isArray(rows) ? rows : []);
      setQuickPlayDraftName("");
      setQuickPlayDraftHost("");
      setQuickPlayDraftPort("25565");
      setInstallNotice("Quick Play server saved.");
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setQuickPlayErr(msg);
      setError(msg);
    } finally {
      setQuickPlayBusy(false);
    }
  }

  async function onRemoveQuickPlayServer(serverId: string) {
    setQuickPlayBusy(true);
    setQuickPlayErr(null);
    try {
      const rows = await removeQuickPlayServer({ id: serverId });
      setQuickPlayServers(Array.isArray(rows) ? rows : []);
      setInstallNotice("Quick Play server removed.");
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setQuickPlayErr(msg);
      setError(msg);
    } finally {
      setQuickPlayBusy(false);
    }
  }

  async function onLaunchQuickPlayServer(server: QuickPlayServerEntry, currentInstance?: Instance | null) {
    setLauncherErr(null);
    setQuickPlayErr(null);
    const targetInstanceId = currentInstance?.id ?? server.bound_instance_id ?? undefined;
    if (!targetInstanceId) {
      setQuickPlayErr("Bind this server to an instance first.");
      return;
    }
    try {
      const result = await launchQuickPlayServer({
        serverId: server.id,
        instanceId: targetInstanceId,
        method: launchMethodPick,
      });
      setInstallNotice(result.message);
      const running = await listRunningInstances();
      const runningSafe = normalizeRunningInstancesPayload(running);
      setRunningInstances((prev) => (sameRunningInstances(prev, runningSafe) ? prev : runningSafe));
      void refreshQuickPlayServers({ silent: true });
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setQuickPlayErr(msg);
      setError(msg);
    }
  }

  function defaultModpackLayerId(spec: ModpackSpec): string {
    if (!spec.layers.length) return "";
    const byId = spec.layers.find((layer) => layer.id === "layer_user");
    if (byId) return byId.id;
    const byName = spec.layers.find((layer) => layer.name.trim().toLowerCase().includes("user"));
    if (byName) return byName.id;
    return spec.layers[0]?.id ?? "";
  }

  async function openAddToModpack(
    target: InstallTarget,
    preferred?: { modpackId?: string | null; layerId?: string | null }
  ) {
    closeProjectOverlays();
    setModpackAddTarget(target);
    setModpackAddErr(null);
    setModpackAddRequired(true);
    setModpackAddEnabledByDefault(true);
    setModpackAddChannelPolicy("stable");
    setModpackAddFallbackPolicy("inherit");
    setModpackAddPinnedVersion("");
    setModpackAddNotes(target.title ?? "");

    setModpackAddSpecsBusy(true);
    try {
      const specs = await listModpackSpecs();
      setModpackAddSpecs(specs);
      if (!specs.length) {
        setModpackAddSpecId("");
        setModpackAddLayerId("");
        return;
      }
      const preferredSpecId = preferred?.modpackId ?? discoverAddContext?.modpackId ?? null;
      const preferredLayerId = preferred?.layerId ?? discoverAddContext?.layerId ?? null;
      const preferredSpec = preferredSpecId ? specs.find((spec) => spec.id === preferredSpecId) : null;
      const existingSelected = specs.find((spec) => spec.id === modpackAddSpecId);
      const chosenSpec = preferredSpec ?? existingSelected ?? specs[0];
      const chosenLayerId =
        preferredLayerId && chosenSpec.layers.some((layer) => layer.id === preferredLayerId)
          ? preferredLayerId
          : defaultModpackLayerId(chosenSpec);
      setModpackAddSpecId(chosenSpec.id);
      setModpackAddLayerId(chosenLayerId);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setModpackAddErr(msg);
      setModpackAddSpecs([]);
      setModpackAddSpecId("");
      setModpackAddLayerId("");
    } finally {
      setModpackAddSpecsBusy(false);
    }
  }

  async function onAddDiscoverTargetToModpack() {
    const target = modpackAddTarget;
    if (!target) return;
    if (!modpackAddSpecId) {
      setModpackAddErr("Select a modpack first.");
      return;
    }
    if (target.contentType === "modpacks") {
      setModpackAddErr("Modpack templates should be imported as layers, not added as a single entry.");
      return;
    }

    setModpackAddBusy(true);
    setModpackAddErr(null);
    try {
      const spec = await getModpackSpec({ modpackId: modpackAddSpecId });
      const layerId = modpackAddLayerId || defaultModpackLayerId(spec);
      const layerIndex = spec.layers.findIndex((layer) => layer.id === layerId);
      if (layerIndex < 0) {
        throw new Error("Selected layer was not found on this modpack.");
      }
      const layer = spec.layers[layerIndex];
      if (layer.is_frozen) {
        throw new Error(`Layer "${layer.name}" is frozen. Unfreeze it before adding new entries.`);
      }

      spec.layers[layerIndex].entries_delta.add.push({
        provider: normalizeDiscoverSource(target.source),
        project_id: target.projectId,
        slug: target.slug ?? null,
        content_type: target.contentType,
        required: modpackAddRequired,
        pin: modpackAddPinnedVersion.trim() ? modpackAddPinnedVersion.trim() : null,
        channel_policy: modpackAddChannelPolicy,
        fallback_policy: modpackAddFallbackPolicy,
        replacement_group: null,
        notes: modpackAddNotes.trim() || target.title || target.projectId,
        disabled_by_default: !modpackAddEnabledByDefault,
        optional: !modpackAddRequired,
        target_scope: "instance",
        target_worlds: [],
      });
      spec.updated_at = new Date().toISOString();

      await upsertModpackSpec({ spec });
      setDiscoverAddContext({
        modpackId: spec.id,
        modpackName: spec.name,
        layerId: layer.id,
        layerName: layer.name,
      });
      setDiscoverAddTrayItems((prev) =>
        [
          {
            id: `${target.source}:${target.projectId}:${Date.now()}`,
            title: target.title || target.projectId,
            projectId: target.projectId,
            source: target.source,
            contentType: target.contentType,
            modpackName: spec.name,
            layerName: layer.name,
            addedAt: new Date().toISOString(),
          },
          ...prev,
        ].slice(0, 24)
      );
      setInstallNotice(`Added "${target.title}" to "${spec.name}" (${layer.name}).`);
      setModpackAddTarget(null);
    } catch (e: any) {
      setModpackAddErr(e?.toString?.() ?? String(e));
    } finally {
      setModpackAddBusy(false);
    }
  }

  function openInstall(target: InstallTarget) {
    // Close any open project modal so we only ever have one overlay active.
    closeProjectOverlays();

    setInstallTarget(target);
    setInstallInstanceQuery("");
  }

  useEffect(() => {
    if (!installTarget) {
      setInstallPlanPreview({});
      setInstallPlanPreviewBusy({});
      setInstallPlanPreviewErr({});
      return;
    }

    if (installTarget.contentType !== "mods") {
      const nextBusy: Record<string, boolean> = {};
      const nextPreview: Record<string, InstallPlanPreview> = {};
      for (const inst of instances) {
        nextBusy[inst.id] = false;
        nextPreview[inst.id] = {
          total_mods: 1,
          dependency_mods: 0,
          will_install_mods: 1,
        };
      }
      setInstallPlanPreview(nextPreview);
      setInstallPlanPreviewErr({});
      setInstallPlanPreviewBusy(nextBusy);
      return;
    }

    if (installTarget.source === "curseforge" || installTarget.source === "github") {
      const nextBusy: Record<string, boolean> = {};
      const nextPreview: Record<string, InstallPlanPreview> = {};
      for (const inst of instances) {
        nextBusy[inst.id] = false;
        nextPreview[inst.id] = {
          total_mods: 1,
          dependency_mods: 0,
          will_install_mods: 1,
        };
      }
      setInstallPlanPreview(nextPreview);
      setInstallPlanPreviewErr({});
      setInstallPlanPreviewBusy(nextBusy);
      return;
    }

    let cancelled = false;
    const nextBusy: Record<string, boolean> = {};
    for (const inst of instances) {
      nextBusy[inst.id] = true;
    }

    setInstallPlanPreview({});
    setInstallPlanPreviewErr({});
    setInstallPlanPreviewBusy(nextBusy);

    for (const inst of instances) {
      previewModrinthInstall({
        instanceId: inst.id,
        projectId: installTarget.projectId,
        projectTitle: installTarget.title,
      })
        .then((preview) => {
          if (cancelled) return;
          setInstallPlanPreview((prev) => ({ ...prev, [inst.id]: preview }));
          setInstallPlanPreviewErr((prev) => {
            const { [inst.id]: _ignored, ...rest } = prev;
            return rest;
          });
        })
        .catch((e: any) => {
          if (cancelled) return;
          setInstallPlanPreviewErr((prev) => ({
            ...prev,
            [inst.id]: e?.toString?.() ?? String(e),
          }));
        })
        .finally(() => {
          if (cancelled) return;
          setInstallPlanPreviewBusy((prev) => ({ ...prev, [inst.id]: false }));
        });
    }

    return () => {
      cancelled = true;
    };
  }, [installTarget?.projectId, installTarget?.title, installTarget?.source, installTarget?.contentType, instances]);

  async function copyProjectText(label: string, value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setProjectCopyNotice(`${label} copied`);
    } catch {
      setProjectCopyNotice("Copy failed");
    }
    window.setTimeout(() => {
      setProjectCopyNotice((current) => (current ? null : current));
    }, 1400);
  }

  async function copyMicrosoftCode() {
    if (!msCodePrompt?.code) return;
    try {
      await navigator.clipboard.writeText(msCodePrompt.code);
      setMsCodeCopied(true);
    } catch {
      setLauncherErr("Couldn't copy code. Please copy it manually.");
    }
    window.setTimeout(() => {
      setMsCodeCopied(false);
    }, 1200);
  }

  function recordLaunchOutcome(instanceId: string, ok: boolean, message?: string | null) {
    const entry: LaunchOutcomeEntry = {
      at: Date.now(),
      ok,
      message: message ?? null,
    };
    setLaunchOutcomesByInstance((prev) => ({
      ...prev,
      [instanceId]: [entry, ...(prev[instanceId] ?? [])].slice(0, 30),
    }));
  }

  async function runLocalResolverBackfill(
    instanceId: string,
    mode: "missing_only" | "all" = "missing_only",
    options?: {
      silent?: boolean;
      refreshListAfterResolve?: boolean;
      contentTypes?: Array<"mods" | "resourcepacks" | "shaderpacks" | "datapacks" | string>;
    }
  ) {
    const now = Date.now();
    const cooldownMs = mode === "all" ? 1000 : 5 * 60_000;
    if ((localResolverBusyRef.current[instanceId] ?? false) && mode !== "all") {
      return;
    }
    if (
      mode !== "all" &&
      now - Number(localResolverBackfillAtRef.current[instanceId] ?? 0) < cooldownMs
    ) {
      return;
    }
    localResolverBusyRef.current[instanceId] = true;
    localResolverBackfillAtRef.current[instanceId] = now;
    try {
      const result = await resolveLocalModSources({
        instanceId,
        mode,
        contentTypes: options?.contentTypes,
      });
      if (
        options?.refreshListAfterResolve !== false &&
        (result.resolved_entries > 0 || mode === "all")
      ) {
        const refreshed = await listInstalledMods(instanceId).catch(() => null);
        if (refreshed && canMutateVisibleInstalledMods(instanceId)) {
          applyInstalledModsForInstance(instanceId, refreshed);
        }
      }
      if (result.resolved_entries > 0) {
        if (!options?.silent) {
          const githubReverts = (result.matches ?? []).filter(
            (item) =>
              normalizeProviderSource(item.from_source) === "github" &&
              normalizeProviderSource(item.to_source) === "local"
          ).length;
          const githubActivations = (result.matches ?? []).filter(
            (item) => normalizeProviderSource(item.to_source) === "github"
          ).length;
          const reasons = (result.matches ?? [])
            .map((item) => String(item.reason ?? "").trim())
            .filter(Boolean);
          const githubVerificationPending = reasons.filter((reason) =>
            /verification is unavailable|temporarily unavailable|rate limit/i.test(reason)
          ).length;
          const githubAssetMatchPending = reasons.filter((reason) =>
            /no verified release asset matched/i.test(reason)
          ).length;
          const summaryBits: string[] = [];
          summaryBits.push(
            `Resolved ${result.resolved_entries} entr${result.resolved_entries === 1 ? "y" : "ies"}`
          );
          if (githubActivations > 0) summaryBits.push(`GitHub activated ${githubActivations}`);
          if (githubReverts > 0) summaryBits.push(`GitHub reverted ${githubReverts}`);
          if (githubVerificationPending > 0) {
            summaryBits.push(
              `GitHub verification pending ${githubVerificationPending} entr${
                githubVerificationPending === 1 ? "y" : "ies"
              } (API unavailable/rate-limited)`
            );
          }
          if (githubAssetMatchPending > 0) {
            summaryBits.push(
              `GitHub release asset match pending ${githubAssetMatchPending} entr${
                githubAssetMatchPending === 1 ? "y" : "ies"
              }`
            );
          }
          setInstallNotice(summaryBits.join(" • "));
        }
      }
      if (!options?.silent && result.resolved_entries === 0) {
        const warningSuffix =
          result.warnings.length > 0
            ? ` ${result.warnings.length} warning${result.warnings.length === 1 ? "" : "s"}.`
            : "";
        setInstallNotice(
          `Identify local files scanned ${result.scanned_entries} entr${result.scanned_entries === 1 ? "y" : "ies"} and found no new provider matches.${warningSuffix}`
        );
      }
      if (!options?.silent && result.warnings.length > 0 && result.resolved_entries === 0) {
        setModsErr(result.warnings[0] ?? "Some local files could not be identified.");
      }
    } catch (e: any) {
      if (!options?.silent) {
        setModsErr(e?.toString?.() ?? String(e));
      }
    } finally {
      localResolverBusyRef.current[instanceId] = false;
    }
  }

  function estimatePostApplyUpdateCheck(
    check: ContentUpdateCheckResult,
    updatedEntries: number
  ): ContentUpdateCheckResult {
    const applied = Math.max(0, Number(updatedEntries) || 0);
    const remaining = Math.max(0, (check.update_count ?? 0) - applied);
    const nextUpdates = remaining === 0
      ? []
      : check.updates.slice(Math.min(check.updates.length, applied));
    return {
      ...check,
      update_count: remaining,
      updates: nextUpdates,
      warnings: check.warnings ?? [],
    };
  }

  function shouldAutoApplyManualChecksForInstance(inst: Instance) {
    if (updateAutoApplyMode === "never") return false;
    if (updateApplyScope !== "scheduled_and_manual") return false;
    return (
      updateAutoApplyMode === "all_instances" ||
      (updateAutoApplyMode === "opt_in_instances" &&
        Boolean(inst.settings?.auto_update_installed_content))
    );
  }

  function applyAutoProfileRecommendation(inst: Instance, recommendation: AutoProfileRecommendation) {
    const memory = recommendation.memory_mb;
    const jvmArgs = recommendation.jvm_args;
    const graphics = recommendation.graphics_preset;
    const signature = autoProfileSignature(recommendation);
    setInstanceMemoryDraft(String(memory));
    setInstanceJvmArgsDraft(jvmArgs);
    void persistInstanceChanges(
      inst,
      {
        settings: {
          memory_mb: memory,
          jvm_args: jvmArgs,
          graphics_preset: graphics,
        },
      },
      "Applied smart auto-profile recommendation."
    );
    setAutoProfileAppliedHintsByInstance((prev) => ({
      ...prev,
      [inst.id]: new Date().toISOString(),
    }));
    setAutoProfileDismissedByInstance((prev) => ({
      ...prev,
      [inst.id]: signature,
    }));
  }

  function launchFixActionDryRunSummary(action: LaunchFixActionDraft): string {
    const explicit = String(action.dryRun ?? "").trim();
    if (explicit) return explicit;
    if (action.kind === "toggle_mod") {
      const targetEnabled = Boolean(action.payload?.target_enabled);
      return `Would ${targetEnabled ? "enable" : "disable"} the selected mod entry.`;
    }
    if (action.kind === "disable_suspect_mods") {
      const count = Array.isArray(action.payload?.versionIds)
        ? (action.payload?.versionIds as unknown[]).length
        : 0;
      return `Would disable ${count} suspect mod${count === 1 ? "" : "s"}.`;
    }
    if (action.kind === "rollback_snapshot") {
      return "Would restore the selected snapshot content and lockfile.";
    }
    if (action.kind === "reset_config_files") {
      const count = Array.isArray(action.payload?.paths) ? (action.payload?.paths as unknown[]).length : 0;
      return `Would back up and reset ${count} config file${count === 1 ? "" : "s"}.`;
    }
    if (action.kind === "open_java_settings") {
      return "Would open Java settings with the recommended runtime highlighted.";
    }
    if (action.kind === "open_logs") {
      return "Would open latest launch/crash logs in Finder/Explorer.";
    }
    if (action.kind === "export_support_bundle") {
      return "Would open support-bundle export flow with redaction enabled.";
    }
    if (action.kind === "install_dependency") {
      return "Would install the required dependency mod.";
    }
    if (action.kind === "open_config") {
      return "Would open the referenced config file or Java settings.";
    }
    if (action.kind === "rerun_preflight") {
      return "Would rerun compatibility checks and report blockers.";
    }
    return "Would apply this action without deleting instance data.";
  }

  function buildLaunchFixPlanFromRunReport(inst: Instance, report: InstanceRunReport): LaunchFixPlan {
    const causes =
      report.topCauses.length > 0
        ? report.topCauses.slice(0, 6)
        : report.findings.slice(0, 6).map((finding) => finding.title);
    const actions: LaunchFixAction[] = report.suggestedActions.map((action) => ({
      id: action.id,
      kind: action.kind,
      title: action.title,
      detail: action.detail,
      selected: true,
      payload: (action.payload ?? undefined) as Record<string, unknown> | undefined,
    }));
    return {
      instance_id: inst.id,
      generated_at: report.createdAt || new Date().toISOString(),
      source: "run_report",
      causes,
      actions,
    };
  }

  function buildLaunchFixPlanFromAnalysis(
    inst: Instance,
    analysis: LogAnalyzeResult,
    mods: InstalledMod[]
  ): LaunchFixPlan {
    const actions: LaunchFixAction[] = [];
    const seen = new Set<string>();
    const addAction = (action: LaunchFixAction) => {
      if (seen.has(action.id)) return;
      seen.add(action.id);
      actions.push(action);
    };
    const lowerErrors = (analysis.keyErrors ?? []).map((line) => line.toLowerCase());
    const enabledMods = mods.filter((mod) => mod.enabled && mod.file_exists);
    const suspectIds = new Set((analysis.suspects ?? []).map((item) => item.id.toLowerCase()));
    const suspectedMods = enabledMods.filter((mod) => {
      const key = `${mod.project_id} ${mod.name} ${mod.filename}`.toLowerCase();
      return Array.from(suspectIds).some((token) => token && key.includes(token));
    });
    for (const mod of suspectedMods.slice(0, 3)) {
      addAction({
        id: `toggle:${mod.version_id}`,
        kind: "toggle_mod",
        title: `Disable ${mod.name}`,
        detail: "Likely crash suspect from log analysis.",
        selected: true,
        payload: { version_id: mod.version_id, target_enabled: false },
      });
    }
    const depPattern = /\b(?:requires|missing mandatory dependency)\s+([a-z0-9._-]{3,})/i;
    for (const line of lowerErrors.slice(0, 16)) {
      const match = line.match(depPattern);
      if (!match?.[1]) continue;
      const projectId = match[1].trim();
      addAction({
        id: `dep:${projectId}`,
        kind: "install_dependency",
        title: `Install dependency: ${projectId}`,
        detail: "Detected from launch errors.",
        selected: true,
        payload: { project_id: projectId, source: "modrinth" },
      });
      if (actions.length >= 5) break;
    }
    const configLine = lowerErrors.find((line) => line.includes("config/") || line.includes("options.txt"));
    if (configLine || analysis.likelyCauses.some((cause) => cause.id === "config_parse_error")) {
      const configMatch = configLine?.match(/(config\/[a-z0-9_./-]+\.(?:json|toml|properties)|options\.txt)/i);
      addAction({
        id: `cfg:${configMatch?.[1] ?? "config"}`,
        kind: "open_config",
        title: "Open config editor",
        detail: configMatch?.[1]
          ? `Inspect and fix ${configMatch[1]}.`
          : "Inspect config files referenced in the crash logs.",
        selected: true,
        payload: { path: configMatch?.[1] ?? null },
      });
    }
    if (analysis.likelyCauses.some((cause) => cause.id === "java_mismatch")) {
      addAction({
        id: "cfg:java-runtime",
        kind: "open_config",
        title: "Review Java runtime settings",
        detail: "Logs suggest Java version/runtime mismatch.",
        selected: true,
        payload: { open_java_settings: true },
      });
    }
    addAction({
      id: "rerun:preflight",
      kind: "rerun_preflight",
      title: "Re-run compatibility checks",
      detail: "Validate blockers after applying fixes.",
      selected: true,
    });
    const causes = analysis.likelyCauses.slice(0, 6).map((cause) => cause.title);
    return {
      instance_id: inst.id,
      generated_at: new Date().toISOString(),
      source: "log_analysis",
      causes,
      actions,
    };
  }

  async function prepareLaunchFixPlan(inst: Instance) {
    setLaunchFixBusyInstanceId(inst.id);
    setError(null);
    try {
      const [runReport, mods] = await Promise.all([
        getInstanceLastRunReport({ instanceId: inst.id }).catch(() => null),
        listInstalledMods(inst.id).catch(() => [] as InstalledMod[]),
      ]);
      setInstanceRunReportById((prev) => ({
        ...prev,
        [inst.id]: runReport && typeof runReport === "object" ? runReport : null,
      }));
      if (
        runReport &&
        ((runReport.findings?.length ?? 0) > 0 || (runReport.suggestedActions?.length ?? 0) > 0)
      ) {
        const plan = buildLaunchFixPlanFromRunReport(inst, runReport);
        const drafts: LaunchFixActionDraft[] = (runReport.suggestedActions ?? [])
          .slice(0, 10)
          .map((action) => ({
            id: action.id,
            kind: action.kind,
            title: action.title,
            detail: action.detail,
            selected: true,
            payload: (action.payload ?? undefined) as Record<string, unknown> | undefined,
            dryRun: action.dryRun,
            reversible: action.reversible,
          }));
        setLaunchFixPlanByInstance((prev) => ({ ...prev, [inst.id]: plan }));
        setLaunchFixPlanDraftByInstance((prev) => ({
          ...prev,
          [inst.id]:
            drafts.length > 0
              ? drafts
              : plan.actions.map((action) => ({
                  ...action,
                  selected: action.selected !== false,
                  dryRun: launchFixActionDryRunSummary({ ...action, selected: true }),
                })),
        }));
        setLaunchFixApplyResultByInstance((prev) => {
          const next = { ...prev };
          delete next[inst.id];
          return next;
        });
        setLaunchFixDryRunByActionId({});
        setLaunchFixModalInstanceId(inst.id);
        setInstallNotice(
          `Built instance fix plan from last run report (${plan.causes.length} cause${plan.causes.length === 1 ? "" : "s"}).`
        );
        return;
      }

      const [launchLog, crashLog] = await Promise.all([
        readInstanceLogs({ instanceId: inst.id, source: "latest_launch", maxLines: 4000 }).catch(() => null),
        readInstanceLogs({ instanceId: inst.id, source: "latest_crash", maxLines: 4000 }).catch(() => null),
      ]);
      const lines: Array<{
        message: string;
        severity: LogSeverity;
        source: string;
        lineNo?: number | null;
        timestamp?: string | null;
      }> = [];
      const append = (payload: ReadInstanceLogsResult | null, source: InstanceLogSourceApi) => {
        if (!payload?.available || !Array.isArray(payload.lines)) return;
        for (const line of payload.lines) {
          lines.push({
            message: String(line.raw ?? ""),
            severity: inferLogSeverity(String(line.raw ?? "")),
            source,
            lineNo: line.line_no ?? null,
            timestamp: line.timestamp ?? null,
          });
        }
      };
      append(launchLog, "latest_launch");
      append(crashLog, "latest_crash");
      if (lines.length === 0) {
        throw new Error("No launch logs found yet. Launch once, then retry Fix My Instance.");
      }
      const analysis = analyzeLogLines(lines);
      const plan = buildLaunchFixPlanFromAnalysis(inst, analysis, mods);
      if (plan.actions.length === 0) {
        throw new Error("No actionable fixes were detected from current logs.");
      }
      const drafts = plan.actions.map((action) => ({
        ...action,
        selected: action.selected !== false,
        dryRun: launchFixActionDryRunSummary({ ...action, selected: action.selected !== false }),
      }));
      setLaunchFixPlanByInstance((prev) => ({ ...prev, [inst.id]: plan }));
      setLaunchFixPlanDraftByInstance((prev) => ({
        ...prev,
        [inst.id]: drafts,
      }));
      setLaunchFixApplyResultByInstance((prev) => {
        const next = { ...prev };
        delete next[inst.id];
        return next;
      });
      setLaunchFixDryRunByActionId({});
      setLaunchFixModalInstanceId(inst.id);
      setInstallNotice(
        `Built launch fix plan with ${plan.actions.length} action${plan.actions.length === 1 ? "" : "s"}.`
      );
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setError(msg);
    } finally {
      setLaunchFixBusyInstanceId(null);
    }
  }

  async function previewLaunchFixAction(inst: Instance, action: LaunchFixActionDraft) {
    const fallback = launchFixActionDryRunSummary(action);
    if (action.kind !== "reset_config_files") {
      setLaunchFixDryRunByActionId((prev) => ({ ...prev, [action.id]: fallback }));
      return;
    }
    const paths = Array.isArray(action.payload?.paths)
      ? (action.payload?.paths as unknown[]).map((value) => String(value)).filter(Boolean)
      : [];
    if (paths.length === 0) {
      setLaunchFixDryRunByActionId((prev) => ({ ...prev, [action.id]: fallback }));
      return;
    }
    try {
      const out = await resetInstanceConfigFilesWithBackup({
        instanceId: inst.id,
        paths,
        dryRun: true,
      });
      setLaunchFixDryRunByActionId((prev) => ({
        ...prev,
        [action.id]:
          out.items.length > 0
            ? `${out.message} ${out.items
                .slice(0, 3)
                .map((item) => item.path)
                .join(", ")}`
            : out.message,
      }));
    } catch (err: any) {
      setLaunchFixDryRunByActionId((prev) => ({
        ...prev,
        [action.id]: `Dry run failed: ${err?.toString?.() ?? String(err)}`,
      }));
    }
  }

  async function applyLaunchFixPlan(inst: Instance) {
    const draft = launchFixPlanDraftByInstance[inst.id] ?? [];
    const selectedActions = draft.filter((action) => action.selected);
    if (selectedActions.length === 0) {
      setInstallNotice("Select at least one fix action to apply.");
      return;
    }
    setLaunchFixApplyBusyInstanceId(inst.id);
    const result: LaunchFixApplyResult = {
      applied: 0,
      failed: 0,
      skipped: 0,
      messages: [],
    };
    try {
      for (const action of selectedActions) {
        try {
          if (action.kind === "toggle_mod") {
            const versionId = String(action.payload?.version_id ?? "");
            const targetEnabled = Boolean(action.payload?.target_enabled);
            if (!versionId) {
              result.skipped += 1;
              result.messages.push(`${action.title}: missing version id`);
              continue;
            }
            await setInstalledModEnabled({ instanceId: inst.id, versionId, enabled: targetEnabled });
            result.applied += 1;
            result.messages.push(`${action.title}: applied`);
          } else if (action.kind === "disable_suspect_mods") {
            const versionIds = Array.isArray(action.payload?.versionIds)
              ? (action.payload?.versionIds as unknown[]).map((value) => String(value)).filter(Boolean)
              : [];
            if (versionIds.length === 0) {
              result.skipped += 1;
              result.messages.push(`${action.title}: no suspect version ids were provided`);
              continue;
            }
            for (const versionId of versionIds) {
              await setInstalledModEnabled({ instanceId: inst.id, versionId, enabled: false });
            }
            result.applied += 1;
            result.messages.push(`${action.title}: disabled ${versionIds.length} mod(s)`);
          } else if (action.kind === "rollback_snapshot") {
            const snapshotId = String(action.payload?.snapshotId ?? "").trim();
            const out = await rollbackInstance({
              instanceId: inst.id,
              snapshotId: snapshotId || undefined,
            });
            result.applied += 1;
            result.messages.push(`${action.title}: restored ${out.snapshot_id}`);
          } else if (action.kind === "open_java_settings") {
            setInstanceSettingsSection("java");
            setInstanceSettingsOpen(true);
            setRoute("instance");
            result.applied += 1;
            result.messages.push(`${action.title}: opened`);
          } else if (action.kind === "reset_config_files") {
            const paths = Array.isArray(action.payload?.paths)
              ? (action.payload?.paths as unknown[]).map((value) => String(value)).filter(Boolean)
              : [];
            if (paths.length === 0) {
              result.skipped += 1;
              result.messages.push(`${action.title}: no config files detected`);
              continue;
            }
            const out = await resetInstanceConfigFilesWithBackup({
              instanceId: inst.id,
              paths,
              dryRun: false,
            });
            if (out.resetCount > 0) {
              result.applied += 1;
            } else {
              result.skipped += 1;
            }
            result.messages.push(
              `${action.title}: reset ${out.resetCount}, skipped ${out.skippedCount}`
            );
          } else if (action.kind === "open_logs") {
            await onOpenLaunchLog(inst);
            result.applied += 1;
            result.messages.push(`${action.title}: opened`);
          } else if (action.kind === "export_support_bundle") {
            setSupportBundleModalInstanceId(inst.id);
            result.applied += 1;
            result.messages.push(`${action.title}: opened`);
          } else if (action.kind === "install_dependency") {
            const projectId = String(action.payload?.project_id ?? "").trim();
            const source = String(action.payload?.source ?? "modrinth");
            if (!projectId) {
              result.skipped += 1;
              result.messages.push(`${action.title}: missing project id`);
              continue;
            }
            const normalizedSource = normalizeDiscoverSource(source);
            if (normalizedSource === "curseforge") {
              await installCurseforgeMod({ instanceId: inst.id, projectId, projectTitle: projectId });
            } else if (normalizedSource === "modrinth") {
              await installModrinthMod({ instanceId: inst.id, projectId, projectTitle: projectId });
            } else {
              await installDiscoverContent({
                instanceId: inst.id,
                source: normalizedSource,
                projectId,
                projectTitle: projectId,
                contentType: "mods",
              });
            }
            result.applied += 1;
            result.messages.push(`${action.title}: installed`);
          } else if (action.kind === "open_config") {
            const path = String(action.payload?.path ?? "").trim();
            if (Boolean(action.payload?.open_java_settings)) {
              setInstanceSettingsSection("java");
              setInstanceSettingsOpen(true);
              setRoute("instance");
            } else if (path) {
              await revealConfigEditorFile({
                instanceId: inst.id,
                scope: "instance",
                path,
              }).catch(() => null);
              setRoute("modpacks");
              setModpacksStudioTab("config");
            }
            result.applied += 1;
            result.messages.push(`${action.title}: opened`);
          } else if (action.kind === "rerun_preflight") {
            const report = await preflightLaunchCompatibility({
              instanceId: inst.id,
              method: launchMethodPick,
            });
            setPreflightReportByInstance((prev) => ({ ...prev, [inst.id]: report }));
            if (report.status === "blocked") {
              result.failed += 1;
              result.messages.push("Preflight still has blockers.");
            } else {
              result.applied += 1;
              result.messages.push("Preflight passed.");
            }
          } else {
            result.skipped += 1;
            result.messages.push(`${action.title}: unsupported action kind`);
          }
        } catch (e: any) {
          result.failed += 1;
          result.messages.push(`${action.title}: ${e?.toString?.() ?? String(e)}`);
        }
      }
      await refreshInstalledMods(inst.id);
      await refreshInstanceHealthPanelData(inst.id);
      await refreshSnapshots(inst.id).catch(() => null);
      setLaunchFixApplyResultByInstance((prev) => ({ ...prev, [inst.id]: result }));
      setInstallNotice(
        `Fix plan complete: ${result.applied} applied, ${result.failed} failed, ${result.skipped} skipped.`
      );
    } finally {
      setLaunchFixApplyBusyInstanceId(null);
    }
  }

  async function onExportSupportBundle(inst: Instance, includeRawLogs = supportBundleIncludeRawLogs) {
    setSupportBundleBusy(true);
    setError(null);
    setLauncherErr(null);
    try {
      const suggested = `${inst.name.replace(/\s+/g, "-") || "instance"}-support-bundle.zip`;
      const savePath = await saveDialog({
        defaultPath: suggested,
        filters: [{ name: "Zip archive", extensions: ["zip"] }],
      });
      if (!savePath || Array.isArray(savePath)) return;
      const perfPayload: SupportPerfAction[] = perfActions.slice(0, 150).map((entry) => ({
        id: entry.id,
        name: entry.name,
        detail: entry.detail ?? null,
        status: entry.status,
        duration_ms: entry.duration_ms,
        finished_at: entry.finished_at,
      }));
      const out = await exportInstanceSupportBundle({
        instanceId: inst.id,
        outputPath: savePath,
        includeRawLogs,
        perfActions: perfPayload,
      });
      setInstallNotice(`${out.message} ${out.files_count} files exported (${out.redactions_applied} redactions).`);
      await shellOpen(savePath);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setError(msg);
    } finally {
      setSupportBundleBusy(false);
      setSupportBundleModalInstanceId(null);
    }
  }

  async function onManualFriendLinkSync(instanceId: string) {
    setFriendLinkSyncBusyInstanceId(instanceId);
    try {
      const out = await reconcileFriendLink({ instanceId, mode: "manual" });
      if (out.status === "conflicted") {
        setFriendConflictInstanceId(instanceId);
        setFriendConflictResult(out);
        setInstallNotice("Friend Link found conflicts. Resolve before launching.");
      } else {
        const warningSuffix =
          out.warnings.length > 0 ? ` ${out.warnings.length} warning${out.warnings.length === 1 ? "" : "s"}.` : "";
        setInstallNotice(`Friend Link sync: ${out.status}. Applied ${out.actions_applied} changes.${warningSuffix}`);
      }
      if (selectedId === instanceId && route === "instance" && out.actions_applied > 0) {
        await refreshInstalledMods(instanceId);
      }
      const status = await getFriendLinkStatus({ instanceId }).catch(() => null);
      if (status) {
        setFriendLinkStatusByInstance((prev) => ({ ...prev, [instanceId]: status }));
        if (selectedId === instanceId) setInstanceFriendLinkStatus(status);
      }
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setFriendLinkSyncBusyInstanceId(null);
    }
  }

  async function refreshInstalledMods(
    instanceId: string,
    options?: { autoIdentifyAfterRefresh?: boolean }
  ) {
    const loadSeq = ++installedModsLoadSeqRef.current;
    setModsBusy(true);
    setModsErr(null);
    try {
      const mods = await listInstalledMods(instanceId);
      if (loadSeq !== installedModsLoadSeqRef.current) return;
      setInstanceModCountById((prev) => ({
        ...prev,
        [instanceId]: mods.filter((entry) => normalizeCreatorEntryType(entry.content_type) === "mods").length,
      }));
      const isActiveInstanceView =
        routeRef.current === "instance" && selectedInstanceIdRef.current === instanceId;
      if (!isActiveInstanceView) return;
      setInstalledMods(mods);
      setInstalledModsInstanceId(instanceId);
      const shouldAutoIdentify =
        options?.autoIdentifyAfterRefresh === true &&
        Boolean(launcherSettings?.auto_identify_local_jars);
      if (shouldAutoIdentify) {
        void runLocalResolverBackfill(instanceId, "missing_only", {
          silent: true,
          refreshListAfterResolve: true,
        });
      }
      if (routeRef.current === "instance" && selectedInstanceIdRef.current === instanceId) {
        void refreshInstanceHistory(instanceId, { silent: true });
      }
    } catch (e: any) {
      if (loadSeq !== installedModsLoadSeqRef.current) return;
      setModsErr(e?.toString?.() ?? String(e));
      if (routeRef.current === "instance" && selectedInstanceIdRef.current === instanceId) {
        setInstalledMods([]);
        setInstalledModsInstanceId(instanceId);
      }
    } finally {
      if (loadSeq === installedModsLoadSeqRef.current) {
        setModsBusy(false);
      }
    }
  }

  async function refreshSnapshots(instanceId: string) {
    const loadSeq = ++snapshotsLoadSeqRef.current;
    setSnapshotsBusy(true);
    try {
      const list = await listInstanceSnapshots({ instanceId });
      const isActiveInstanceView =
        routeRef.current === "instance" && selectedInstanceIdRef.current === instanceId;
      if (loadSeq !== snapshotsLoadSeqRef.current || !isActiveInstanceView) return;
      setSnapshots(list);
      setRollbackSnapshotId((prev) =>
        prev && list.some((s) => s.id === prev) ? prev : list[0]?.id ?? null
      );
    } catch {
      const isActiveInstanceView =
        routeRef.current === "instance" && selectedInstanceIdRef.current === instanceId;
      if (loadSeq !== snapshotsLoadSeqRef.current || !isActiveInstanceView) return;
      setSnapshots([]);
      setRollbackSnapshotId(null);
    } finally {
      if (loadSeq === snapshotsLoadSeqRef.current) {
        setSnapshotsBusy(false);
      }
    }
  }

  async function refreshInstanceHealthPanelData(instanceId: string) {
    try {
      const [diskUsage, lastRun, runReport, playtime] = await Promise.all([
        getInstanceDiskUsage({ instanceId }).catch(() => null),
        getInstanceLastRunMetadata({ instanceId }).catch(() => null),
        getInstanceLastRunReport({ instanceId }).catch(() => null),
        getInstancePlaytime({ instanceId }).catch(() => null),
      ]);
      if (typeof diskUsage === "number" && Number.isFinite(diskUsage) && diskUsage >= 0) {
        setInstanceDiskUsageById((prev) => ({ ...prev, [instanceId]: diskUsage }));
      }
      if (lastRun && typeof lastRun === "object") {
        setInstanceLastRunMetadataById((prev) => ({ ...prev, [instanceId]: lastRun }));
      }
      setInstancePlaytimeById((prev) => ({
        ...prev,
        [instanceId]: playtime && typeof playtime === "object" ? playtime : null,
      }));
      setInstanceRunReportById((prev) => ({
        ...prev,
        [instanceId]: runReport && typeof runReport === "object" ? runReport : null,
      }));
    } catch {
      // Keep this best-effort and non-blocking for the instance page.
    }
  }

  async function onInstallToInstance(inst: Instance) {
    const target = installTarget;
    if (!target) return;
    if (target.installSupported === false) {
      setInstallNotice(
        target.installNote ||
          "This result cannot be installed directly yet. Open the provider page for manual download/import."
      );
      return;
    }
    const key = `${inst.id}:${target.source}:${target.contentType}:${target.projectId}`;
    const timingKey = `${inst.id}:${target.projectId}`;
    const nowPerf = performance.now();
    installProgressTimingRef.current[timingKey] = {
      started_at: nowPerf,
      last_at: nowPerf,
      last_percent: 0,
      rate_percent_per_sec: 0,
      last_downloaded: 0,
      rate_bytes_per_sec: 0,
    };
    setInstallProgressEtaSeconds(null);
    setInstallProgressElapsedSeconds(0);
    setInstallProgressBytesPerSecond(null);
    setInstallingKey(key);
    setInstallNotice(null);
    setError(null);
    setCurseforgeBlockedRecoveryPrompt(null);
    setInstallProgress({
      instance_id: inst.id,
      project_id: target.projectId,
      stage: "resolving",
      downloaded: 0,
      total: null,
      percent: null,
      message:
        target.source === "github"
          ? "Resolving compatible GitHub release…"
          : "Resolving compatible version…",
    });
    let installSucceeded = false;

    try {
      const directDatapackWorlds =
        target.contentType === "datapacks"
          ? (
              target.targetWorlds?.length
                ? target.targetWorlds
                : (await listInstanceWorlds({ instanceId: inst.id })).map((w) => w.id)
            )
          : [];
      if (target.contentType === "datapacks" && directDatapackWorlds.length === 0) {
        throw new Error("No worlds found in this instance. Create a world first, or add this datapack to the creator and choose targets.");
      }
      const mod =
        target.contentType === "mods"
          ? target.source === "curseforge"
            ? await installCurseforgeMod({
                instanceId: inst.id,
                projectId: target.projectId,
                projectTitle: target.title,
              })
            : target.source === "modrinth"
              ? await installModrinthMod({
                  instanceId: inst.id,
                  projectId: target.projectId,
                  projectTitle: target.title,
                })
              : await installDiscoverContent({
                  instanceId: inst.id,
                  source: normalizeDiscoverSource(target.source),
                  projectId: target.projectId,
                  projectTitle: target.title,
                  contentType: target.contentType,
                  targetWorlds: directDatapackWorlds,
                })
          : await installDiscoverContent({
              instanceId: inst.id,
              source: normalizeDiscoverSource(target.source),
              projectId: target.projectId,
              projectTitle: target.title,
              contentType: target.contentType,
              targetWorlds: directDatapackWorlds,
            });
      await refreshInstalledMods(inst.id);
      await refreshSnapshots(inst.id);
      await refreshInstances();
      setCurseforgeBlockedRecoveryPrompt(null);
      setInstallNotice(`Installed ${mod.name} ${mod.version_number} in ${inst.name}.`);
      setInstallProgress({
        instance_id: inst.id,
        project_id: target.projectId,
        stage: "completed",
        downloaded: 1,
        total: 1,
        percent: 100,
        message: "Install complete",
      });
      setInstallProgressEtaSeconds(0);
      installSucceeded = true;
    } catch (e: any) {
      const errText = e?.toString?.() ?? String(e);
      setError(errText);
      if (target.source === "curseforge" && isCurseforgeBlockedDownloadUrlError(errText)) {
        const contentView = discoverContentTypeToInstanceView(target.contentType);
        let projectUrl = buildCurseforgeProjectUrl(target);
        try {
          const detail = await getCurseforgeProjectDetail({
            projectId: target.projectId,
            contentType: target.contentType,
          });
          if (detail.external_url?.trim()) {
            projectUrl = detail.external_url.trim();
          }
        } catch {
          // Keep fallback URL if detail lookup is unavailable.
        }
        setCurseforgeBlockedRecoveryPrompt({
          instanceId: inst.id,
          instanceName: inst.name,
          contentView,
          target,
          projectUrl,
        });
      } else {
        setCurseforgeBlockedRecoveryPrompt(null);
      }
      setInstallProgress((prev) => ({
        instance_id: inst.id,
        project_id: target.projectId,
        stage: "error",
        downloaded: prev?.downloaded ?? 0,
        total: prev?.total ?? null,
        percent: prev?.percent ?? null,
        message: e?.toString?.() ?? String(e),
      }));
      setInstallProgressEtaSeconds(null);
    } finally {
      const startedAt = installProgressTimingRef.current[timingKey]?.started_at ?? nowPerf;
      const elapsedSeconds = Math.max(0, (performance.now() - startedAt) / 1000);
      setInstallProgressElapsedSeconds(elapsedSeconds);
      recordPerfAction(
        "install_content",
        installSucceeded ? "ok" : "error",
        startedAt,
        `${target.source}:${target.contentType}:${target.projectId}`
      );
      delete installProgressTimingRef.current[timingKey];
      setInstallProgressBytesPerSecond(null);
      setInstallingKey(null);
      window.setTimeout(() => {
        setInstallProgress((prev) => (prev?.stage === "completed" ? null : prev));
      }, 900);
    }
  }

  async function onRollbackToSnapshot(inst: Instance, snapshotId?: string | null) {
    setRollbackBusy(true);
    setError(null);
    try {
      const out: RollbackResult = await rollbackInstance({
        instanceId: inst.id,
        snapshotId: snapshotId ?? undefined,
      });
      await refreshInstalledMods(inst.id);
      await refreshSnapshots(inst.id);
      setUpdateCheck(null);
      setInstallNotice(
        `${out.message} Restored ${out.restored_files} file(s) from snapshot ${out.snapshot_id}.`
      );
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setRollbackBusy(false);
    }
  }

  async function onRollbackWorldBackup(inst: Instance, world: InstanceWorld) {
    const worldId = String(world.id ?? "").trim();
    if (!worldId) return;
    if (runningInstances.some((run) => run.instance_id === inst.id)) {
      setInstallNotice("Stop all running sessions for this instance before rolling back a world backup.");
      return;
    }
    setWorldRollbackBusyById((prev) => ({ ...prev, [worldId]: true }));
    setError(null);
    setInstallNotice(`Rolling back "${world.name}" to latest backup…`);
    try {
      const out: WorldRollbackResult = await rollbackInstanceWorldBackup({
        instanceId: inst.id,
        worldId,
        backupId: world.latest_backup_id ?? undefined,
      });
      const worlds = await listInstanceWorlds({ instanceId: inst.id }).catch(() => [] as InstanceWorld[]);
      setInstanceWorlds(worlds);
      setInstallNotice(
        `${out.message} Restored ${out.restored_files} file(s) in "${world.name}" from ${formatDateTime(out.created_at)}.`
      );
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setWorldRollbackBusyById((prev) => {
        const next = { ...prev };
        delete next[worldId];
        return next;
      });
    }
  }

  async function onCreatePresetFromInstance(inst: Instance) {
    setPresetBusy(true);
    setError(null);
    try {
      const mods = await listInstalledMods(inst.id);
      const entries: UserPresetEntry[] = mods
        .filter(
          (m) =>
            m.source === "modrinth" || m.source === "curseforge" || m.source === "github"
        )
        .map((m) => ({
          source: m.source,
          project_id: m.project_id,
          title: m.name,
          content_type: (m.content_type as any) ?? "mods",
          pinned_version: m.pinned_version ?? null,
          target_scope: (m.target_scope as any) ?? ((m.content_type ?? "mods") === "datapacks" ? "world" : "instance"),
          target_worlds: m.target_worlds ?? [],
          enabled: true,
        }));
      if (entries.length === 0) {
        throw new Error(
          "This instance has no Modrinth/CurseForge/GitHub entries to save as a preset."
        );
      }
      const next: UserPreset = {
        id: `preset_${Date.now()}`,
        name: presetNameDraft.trim() || `${inst.name} preset`,
        created_at: new Date().toISOString(),
        source_instance_id: inst.id,
        source_instance_name: inst.name,
        entries,
        settings: defaultPresetSettings(),
      };
      setPresets((prev) => [next, ...prev]);
      setPresetNameDraft("");
      setCreatorDraft(next);
      setInstallNotice(`Created preset "${next.name}" with ${entries.length} entries.`);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setPresetBusy(false);
    }
  }

  async function onApplyPresetToInstance(preset: UserPreset, inst: Instance) {
    setPresetBusy(true);
    setError(null);
    try {
      const applyResult: PresetApplyResult = await applyPresetToInstance({
        instanceId: inst.id,
        preset,
      });
      await refreshInstalledMods(inst.id);
      await refreshSnapshots(inst.id);
      const byTypeText = Object.entries(applyResult.by_content_type)
        .map(([k, v]) => `${k}:${v}`)
        .join(", ");
      setInstallNotice(
        `${applyResult.message} Installed ${applyResult.installed_entries}, skipped ${applyResult.skipped_entries}, failed ${applyResult.failed_entries}.${byTypeText ? ` (${byTypeText})` : ""}`
      );
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setPresetBusy(false);
    }
  }

  async function onPreviewPresetApply(preset: UserPreset, inst: Instance) {
    setPresetPreviewBusy(true);
    setError(null);
    try {
      const preview = await previewPresetApply({
        instanceId: inst.id,
        preset,
      });
      setPresetPreview(preview);
      if (!preview.valid) {
        const msg = [
          ...preview.provider_warnings,
          preview.missing_world_targets.length
            ? `Missing datapack targets: ${preview.missing_world_targets.join(", ")}`
            : "",
        ]
          .filter(Boolean)
          .join(" | ");
        setError(msg || "Preset preview found issues.");
      } else {
        setInstallNotice(
          `Preview OK: ${preview.installable_entries} installable, ${preview.skipped_disabled_entries} disabled, ${preview.duplicate_entries} duplicates.`
        );
      }
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
      setPresetPreview(null);
    } finally {
      setPresetPreviewBusy(false);
    }
  }

  async function onExportPresets() {
    setPresetIoBusy(true);
    setError(null);
    try {
      if (presets.length === 0) {
        throw new Error("No presets to export.");
      }
      const savePath = await saveDialog({
        defaultPath: "openjar-launcher-presets.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!savePath || Array.isArray(savePath)) return;

      const payload: PresetExportPayload = {
        format: "mpm-presets/v2",
        exported_at: new Date().toISOString(),
        presets,
      };
      const out = await exportPresetsJson({
        outputPath: savePath,
        payload,
      });
      setInstallNotice(`Exported ${out.items} preset(s) to ${out.path}`);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setPresetIoBusy(false);
    }
  }

  async function onImportPresets() {
    setPresetIoBusy(true);
    setError(null);
    try {
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!picked || Array.isArray(picked)) return;

      const imported = await importPresetsJson({ inputPath: picked });
      const values = Array.isArray(imported)
        ? imported
        : Array.isArray((imported as any)?.presets)
          ? (imported as any).presets
          : [];

      const normalized = values
        .map((item) => normalizeImportedPreset(item))
        .filter((item): item is UserPreset => Boolean(item));
      if (normalized.length === 0) {
        throw new Error("No valid presets found in the selected file.");
      }

      setPresets((prev) => {
        const map = new Map<string, UserPreset>();
        for (const item of prev) map.set(item.id, item);
        for (const item of normalized) {
          const id = map.has(item.id) ? `${item.id}_${Date.now()}` : item.id;
          map.set(id, { ...item, id });
        }
        return Array.from(map.values()).sort((a, b) => b.created_at.localeCompare(a.created_at));
      });
      setInstallNotice(`Imported ${normalized.length} preset(s).`);
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setPresetIoBusy(false);
    }
  }

  async function onToggleInstalledMod(inst: Instance, mod: InstalledMod, enabled: boolean) {
    setToggleBusyVersion(installedEntryUiKey(mod));
    setModsErr(null);
    try {
      await setInstalledModEnabled({
        instanceId: inst.id,
        versionId: mod.version_id,
        contentType: mod.content_type,
        filename: mod.filename,
        enabled,
      });
      await refreshInstalledMods(inst.id);
    } catch (e: any) {
      setModsErr(e?.toString?.() ?? String(e));
    } finally {
      setToggleBusyVersion(null);
    }
  }

  async function onDeleteInstalledMod(inst: Instance, mod: InstalledMod) {
    const contentLabel = installedContentTypeLabel(mod.content_type);
    const confirmed = window.confirm(
      `Delete "${mod.name}" from this instance?\n\nThis removes ${contentLabel} file(s) from disk for this instance.`
    );
    if (!confirmed) return;
    setToggleBusyVersion(installedEntryUiKey(mod));
    setModsErr(null);
    try {
      await removeInstalledMod({
        instanceId: inst.id,
        versionId: mod.version_id,
        contentType: mod.content_type,
        filename: mod.filename,
      });
      await refreshInstalledMods(inst.id);
      setInstallNotice(`Deleted ${mod.name} from ${inst.name}.`);
    } catch (e: any) {
      setModsErr(e?.toString?.() ?? String(e));
    } finally {
      setToggleBusyVersion(null);
    }
  }

  async function onCleanMissingInstalledEntries(
    inst: Instance,
    contentView: "mods" | "resourcepacks" | "datapacks" | "shaders"
  ) {
    setCleanMissingBusyInstanceId(inst.id);
    setModsErr(null);
    try {
      const backendContentType = instanceContentTypeToBackend(contentView);
      const out = await pruneMissingInstalledEntries({
        instanceId: inst.id,
        contentTypes: [backendContentType],
      });
      await refreshInstalledMods(inst.id);
      const sectionLabel = instanceContentSectionLabel(contentView);
      if (out.removed_count > 0) {
        const removedPreview = out.removed_names.slice(0, 3).join(", ");
        const extraCount = Math.max(0, out.removed_count - 3);
        const previewSuffix = removedPreview
          ? ` Removed: ${removedPreview}${extraCount > 0 ? ` (+${extraCount} more)` : ""}.`
          : "";
        setInstallNotice(
          `Cleaned ${out.removed_count} missing ${sectionLabel} entr${
            out.removed_count === 1 ? "y" : "ies"
          } from lock metadata.${previewSuffix}`
        );
      } else {
        setInstallNotice(`No missing ${sectionLabel} entries needed cleanup.`);
      }
    } catch (e: any) {
      setModsErr(e?.toString?.() ?? String(e));
    } finally {
      setCleanMissingBusyInstanceId(null);
    }
  }

  function onToggleModSelection(entryKey: string, checked: boolean) {
    setSelectedModVersionIds((prev) => {
      if (checked) {
        if (prev.includes(entryKey)) return prev;
        return [...prev, entryKey];
      }
      return prev.filter((id) => id !== entryKey);
    });
  }

  function onToggleAllVisibleModSelection(mods: InstalledMod[], checked: boolean) {
    const ids = mods
      .filter((m) => m.file_exists)
      .map((m) => installedEntryUiKey(m));
    if (ids.length === 0) return;
    setSelectedModVersionIds((prev) => {
      if (checked) {
        const merged = new Set([...prev, ...ids]);
        return Array.from(merged);
      }
      const remove = new Set(ids);
      return prev.filter((id) => !remove.has(id));
    });
  }

  async function onBulkToggleSelectedMods(inst: Instance, enabled: boolean) {
    const candidates = installedContentSummary.visibleInstalledMods.filter(
      (m) =>
        selectedModVersionIdSet.has(installedEntryUiKey(m)) &&
        m.file_exists &&
        m.enabled !== enabled
    );
    if (candidates.length === 0) {
      setInstallNotice(
        selectedModVersionIds.length === 0
          ? "Select one or more entries first."
          : "No selected entries need changes."
      );
      return;
    }
    setToggleBusyVersion("__bulk__");
    setModsErr(null);
    const succeeded = new Set<string>();
    const failedNames: string[] = [];
    try {
      for (const mod of candidates) {
        try {
          await setInstalledModEnabled({
            instanceId: inst.id,
            versionId: mod.version_id,
            contentType: mod.content_type,
            filename: mod.filename,
            enabled,
          });
          succeeded.add(installedEntryUiKey(mod));
        } catch {
          failedNames.push(mod.name);
        }
      }
      await refreshInstalledMods(inst.id);
      if (succeeded.size > 0) {
        const sectionLabel = instanceContentSectionLabel(instanceContentType);
        setInstallNotice(
          `${enabled ? "Enabled" : "Disabled"} ${succeeded.size} selected ${sectionLabel} entr${
            succeeded.size === 1 ? "" : "s"
          }.`
        );
      }
      if (failedNames.length > 0) {
        setModsErr(
          `Could not update ${failedNames.length} entr${
            failedNames.length === 1 ? "" : "s"
          }: ${failedNames.slice(0, 3).join(", ")}${
            failedNames.length > 3 ? ` (+${failedNames.length - 3} more)` : ""
          }`
        );
      }
      if (succeeded.size > 0) {
        setSelectedModVersionIds((prev) => prev.filter((id) => !succeeded.has(id)));
      }
    } finally {
      setToggleBusyVersion(null);
    }
  }

  async function onAddContentFromFile(
    inst: Instance,
    contentView: "mods" | "resourcepacks" | "datapacks" | "shaders"
  ) {
    setError(null);
    setModsErr(null);
    setInstallNotice(null);
    try {
      const backendContentType = instanceContentTypeToBackend(contentView);
      const picked = await openDialog({
        multiple: true,
        filters: [
          {
            name: `Minecraft ${localImportTypeLabel(contentView)}`,
            extensions: localImportExtensionsForInstanceType(contentView),
          },
        ],
      });
      if (!picked) return;
      const filePaths = Array.isArray(picked) ? picked : [picked];
      if (filePaths.length === 0) return;

      const datapackWorlds =
        backendContentType === "datapacks"
          ? (await listInstanceWorlds({ instanceId: inst.id })).map((world) => world.id)
          : [];
      if (backendContentType === "datapacks" && datapackWorlds.length === 0) {
        setModsErr(
          "No worlds found in this instance. Create a world first, then add local datapacks."
        );
        return;
      }

      setImportingInstanceId(inst.id);
      let successCount = 0;
      const failedPaths: string[] = [];
      for (const filePath of filePaths) {
        try {
          await importLocalModFile({
            instanceId: inst.id,
            filePath,
            contentType: backendContentType,
            targetWorlds: backendContentType === "datapacks" ? datapackWorlds : undefined,
          });
          successCount += 1;
        } catch {
          failedPaths.push(filePath);
        }
      }
      await refreshInstalledMods(inst.id, { autoIdentifyAfterRefresh: successCount > 0 });
      if (successCount > 0) {
        const typeLabel = localImportTypeLabel(contentView);
        setInstallNotice(
          `Added ${successCount} local ${typeLabel} file${successCount === 1 ? "" : "s"} from your computer.`
        );
      }
      if (failedPaths.length > 0) {
        const short = failedPaths
          .slice(0, 3)
          .map((path) => basenameWithoutExt(path))
          .join(", ");
        setModsErr(
          `Could not import ${failedPaths.length} file${failedPaths.length === 1 ? "" : "s"}: ${short}${
            failedPaths.length > 3 ? ` (+${failedPaths.length - 3} more)` : ""
          }`
        );
      }
    } catch (e: any) {
      setError(e?.toString?.() ?? String(e));
    } finally {
      setImportingInstanceId(null);
    }
  }

  async function onCancelPendingLaunch(inst: Instance) {
    setError(null);
    setLauncherErr(null);
    setLaunchCancelBusyInstanceId(inst.id);
    setLaunchStageByInstance((prev) => ({
      ...prev,
      [inst.id]: {
        status: "starting",
        label: "Cancelling",
        message: "Launch cancellation requested…",
        updated_at: Date.now(),
      },
    }));
    try {
      const message = await cancelInstanceLaunch({ instanceId: inst.id });
      setInstallNotice(message || `Launch cancellation requested for ${inst.name}.`);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setError(msg);
      setLauncherErr(msg);
    } finally {
      setLaunchCancelBusyInstanceId(null);
    }
  }

  async function maybeAutoTriggerMicPermissionPrompt(
    instanceId: string,
    report: LaunchCompatibilityReport | null | undefined,
    method: LaunchMethod,
    opts?: { silent?: boolean }
  ) {
    const autoEnabled = launcherSettings?.auto_trigger_mic_permission_prompt ?? true;
    if (!autoEnabled || !isMacDesktopPlatform() || method !== "native" || !report) return;
    const micPermission =
      (report.permissions ?? []).find((item) => item.key === "microphone") ?? null;
    if (!micPermissionNeedsAction(micPermission)) return;
    const fingerprint = `${String(micPermission.status ?? "").toLowerCase()}|${String(
      micPermission.detail ?? ""
    ).slice(0, 120)}`;
    const now = Date.now();
    const prevAttempt = autoMicPromptAttemptRef.current[instanceId];
    if (
      prevAttempt &&
      prevAttempt.fingerprint === fingerprint &&
      now - prevAttempt.at < 3 * 60 * 1000
    ) {
      return;
    }
    autoMicPromptAttemptRef.current[instanceId] = { fingerprint, at: now };
    try {
      const message = await triggerInstanceMicrophonePermissionPrompt({
        instanceId,
        method: "native",
      });
      if (!opts?.silent) {
        setInstallNotice(`Automatic microphone setup: ${message}`);
      }
    } catch (e: any) {
      if (!opts?.silent) {
        const msg = e?.toString?.() ?? String(e);
        setInstallNotice(`Automatic microphone setup failed: ${msg}`);
      }
    }
  }

  async function refreshInstancePermissionChecklist(
    instanceId: string,
    method: LaunchMethod = launchMethodPick,
    opts?: { silent?: boolean; skipAutoPrompt?: boolean }
  ) {
    if (permissionChecklistRefreshInFlightRef.current[instanceId]) return;
    permissionChecklistRefreshInFlightRef.current[instanceId] = true;
    setPermissionChecklistBusyByInstance((prev) => ({ ...prev, [instanceId]: true }));
    try {
      const report = await preflightLaunchCompatibility({
        instanceId,
        method,
      });
      setPreflightReportByInstance((prev) => ({ ...prev, [instanceId]: report }));
      if (!opts?.skipAutoPrompt) {
        await maybeAutoTriggerMicPermissionPrompt(instanceId, report, method, {
          silent: true,
        });
      }
      return report;
    } catch (e: any) {
      if (!opts?.silent) {
        const msg = e?.toString?.() ?? String(e);
        setInstallNotice(`Permission check failed: ${msg}`);
      }
      return null;
    } finally {
      delete permissionChecklistRefreshInFlightRef.current[instanceId];
      setPermissionChecklistBusyByInstance((prev) => ({ ...prev, [instanceId]: false }));
    }
  }

  async function openMicrophoneSystemSettings() {
    if (!isMacDesktopPlatform()) {
      setInstallNotice(
        "Open your OS privacy settings and allow microphone access for Java/Minecraft, then click Re-check."
      );
      return;
    }
    try {
      const message = await openMicrophoneSystemSettingsNative();
      setInstallNotice(message);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setInstallNotice(`Could not open System Settings automatically: ${msg}`);
    }
  }

  async function triggerInstanceMicrophonePrompt(instanceId: string) {
    if (!isMacDesktopPlatform()) {
      setInstallNotice(
        "Microphone permission prompt helper is available on macOS only. Open your OS privacy settings and allow Java/Minecraft microphone access."
      );
      return;
    }
    setPermissionChecklistBusyByInstance((prev) => ({ ...prev, [instanceId]: true }));
    try {
      const message = await triggerInstanceMicrophonePermissionPrompt({
        instanceId,
        method: "native",
      });
      setInstallNotice(message);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setInstallNotice(`Could not trigger Java microphone prompt: ${msg}`);
    } finally {
      setPermissionChecklistBusyByInstance((prev) => ({ ...prev, [instanceId]: false }));
      await refreshInstancePermissionChecklist(instanceId, "native", { silent: true, skipAutoPrompt: true });
    }
  }

  async function onPlayInstance(inst: Instance, method?: LaunchMethod) {
    const requestedMethod = method ?? launchMethodPick;
    if (launchBusyInstanceIds.includes(inst.id)) {
      await onCancelPendingLaunch(inst);
      return;
    }
    const runningForInstance = runningByInstanceId.get(inst.id) ?? [];
    const hasNativeRunningForInstance = runningForInstance.some(
      (run) => String(run.method ?? "").toLowerCase() === "native"
    );
    if (requestedMethod === "native" && hasNativeRunningForInstance) {
      setInstallNotice(
        "Starting another native run in disposable session mode. This extra run gets a temporary copy of the instance; only Minecraft settings sync back when it closes."
      );
    }
    setError(null);
    setLauncherErr(null);
    if (!(requestedMethod === "native" && hasNativeRunningForInstance)) {
      setInstallNotice(null);
    }
    setLaunchFailureByInstance((prev) => {
      if (!prev[inst.id]) return prev;
      const next = { ...prev };
      delete next[inst.id];
      return next;
    });
    setLaunchBusyInstanceIds((prev) => (prev.includes(inst.id) ? prev : [...prev, inst.id]));
    setLaunchStageByInstance((prev) => ({
      ...prev,
      [inst.id]: {
        status: "starting",
        label: "Preparing",
        message: "Preparing launch…",
        updated_at: Date.now(),
      },
    }));
    try {
      const preflight = await preflightLaunchCompatibility({
        instanceId: inst.id,
        method: requestedMethod,
      });
      setPreflightReportByInstance((prev) => ({ ...prev, [inst.id]: preflight }));
      await maybeAutoTriggerMicPermissionPrompt(inst.id, preflight, requestedMethod, { silent: false });
      const preflightFingerprint = launchCompatibilityFingerprint(preflight);
      const now = Date.now();
      const ignoreEntry = preflightIgnoreByInstance[inst.id];
      const canIgnoreBlockedPreflight = Boolean(
        ignoreEntry &&
          ignoreEntry.expires_at > now &&
          ignoreEntry.fingerprint === preflightFingerprint
      );
      if (ignoreEntry && ignoreEntry.expires_at <= now) {
        setPreflightIgnoreByInstance((prev) => {
          if (!prev[inst.id]) return prev;
          const next = { ...prev };
          delete next[inst.id];
          return next;
        });
      }
      if (preflight.status === "blocked") {
        if (canIgnoreBlockedPreflight) {
          setInstallNotice(
            `Compatibility blockers were ignored for this instance until ${formatDateTime(
              new Date(ignoreEntry!.expires_at).toISOString()
            )}.`
          );
        } else {
        const reason =
          preflight.items.find((item) => item.blocking)?.message ??
          "Launch is blocked by compatibility checks.";
        setPreflightReportModal({
          instanceId: inst.id,
          method: requestedMethod,
          report: preflight,
        });
        setError(reason);
        setLauncherErr(reason);
        setLaunchStageByInstance((prev) => ({
          ...prev,
          [inst.id]: {
            status: "error",
            label: "Blocked",
            message: reason,
            updated_at: Date.now(),
          },
        }));
        return;
        }
      }
      if (preflight.warning_count > 0 || preflight.status === "warning") {
        const warn = preflight.items.find((item) => !item.blocking)?.message;
        setInstallNotice(
          warn
            ? `Preflight warning: ${warn}`
            : "Preflight found non-blocking warnings. Launching anyway."
        );
      }

      const friendLinkStatus = await getFriendLinkStatus({ instanceId: inst.id }).catch(() => null);
      if (friendLinkStatus) {
        setFriendLinkStatusByInstance((prev) => ({ ...prev, [inst.id]: friendLinkStatus }));
      }
      if (friendLinkStatus?.linked) {
        const friendReconcile = await reconcileFriendLink({ instanceId: inst.id, mode: "prelaunch" });
        if (friendReconcile.status === "conflicted") {
          setFriendConflictInstanceId(inst.id);
          setFriendConflictResult(friendReconcile);
          setInstallNotice("Friend Link has unresolved conflicts. Resolve them before launching.");
          setLaunchStageByInstance((prev) => {
            const next = { ...prev };
            delete next[inst.id];
            return next;
          });
          recordLaunchOutcome(inst.id, false, "Friend Link conflicts blocked launch.");
          return;
        }
        if (
          friendReconcile.status === "blocked_offline_stale" ||
          friendReconcile.status === "blocked_untrusted" ||
          friendReconcile.status === "error"
        ) {
          const reason = friendReconcile.blocked_reason ?? "Friend Link state is not safe to launch.";
          setError(reason);
          setLauncherErr(reason);
          setLaunchStageByInstance((prev) => ({
            ...prev,
            [inst.id]: {
              status: "error",
              label: "Blocked",
              message: reason,
              updated_at: Date.now(),
            },
          }));
          recordLaunchOutcome(inst.id, false, reason);
          return;
        }
        if (friendReconcile.status === "degraded_offline_last_good") {
          setInstallNotice("Friend Link: peers offline, launching with last-good synced snapshot.");
        }
      }

      const res: LaunchResult = await launchInstance({
        instanceId: inst.id,
        method: requestedMethod,
      });
      if (res.method === "prism" && res.prism_instance_id) {
        setInstallNotice(`${res.message} (Prism instance: ${res.prism_instance_id})`);
      } else {
        setInstallNotice(res.message);
      }
      recordLaunchOutcome(inst.id, true, res.message);
      setLaunchStageByInstance((prev) => ({
        ...prev,
        [inst.id]: {
          status: "running",
          label: "Running",
          message: res.message,
          updated_at: Date.now(),
        },
      }));
      const running = await listRunningInstances();
      const runningSafe = normalizeRunningInstancesPayload(running);
      setRunningInstances((prev) => (sameRunningInstances(prev, runningSafe) ? prev : runningSafe));
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      if (/cancelled by user|launch cancelled/i.test(msg)) {
        setInstallNotice("Launch cancelled.");
        setLaunchStageByInstance((prev) => {
          const next = { ...prev };
          delete next[inst.id];
          return next;
        });
      } else {
        setError(msg);
        setLauncherErr(msg);
        const launchMethod = String(requestedMethod ?? "native").toLowerCase();
        setLaunchFailureByInstance((prev) => ({
          ...prev,
          [inst.id]: {
            status: "error",
            method: launchMethod,
            message: msg,
            updated_at: Date.now(),
          },
        }));
        recordLaunchOutcome(inst.id, false, msg);
        setLaunchStageByInstance((prev) => ({
          ...prev,
          [inst.id]: {
            status: "error",
            label: "Error",
            message: msg,
            updated_at: Date.now(),
          },
        }));
      }
    } finally {
      setLaunchBusyInstanceIds((prev) => prev.filter((id) => id !== inst.id));
    }
  }

  async function onStopRunning(launchId: string) {
    setLauncherErr(null);
    userRequestedStopLaunchIdsRef.current.add(launchId);
    const runningTarget = runningInstances.find((item) => item.launch_id === launchId);
    if (runningTarget?.instance_id) {
      setLaunchFailureByInstance((prev) => {
        if (!prev[runningTarget.instance_id]) return prev;
        const next = { ...prev };
        delete next[runningTarget.instance_id];
        return next;
      });
    }
    try {
      await stopRunningInstance({ launchId });
      const running = await listRunningInstances();
      const runningSafe = normalizeRunningInstancesPayload(running);
      setRunningInstances((prev) => (sameRunningInstances(prev, runningSafe) ? prev : runningSafe));
      setInstallNotice("Stop signal sent.");
    } catch (e: any) {
      userRequestedStopLaunchIdsRef.current.delete(launchId);
      setLauncherErr(e?.toString?.() ?? String(e));
    }
  }

  async function onExportModsZip(inst: Instance) {
    setLauncherErr(null);
    setInstallNotice(null);
    try {
      const suggested = `${inst.name.replace(/\s+/g, "-") || "instance"}-mods.zip`;
      const savePath = await saveDialog({
        defaultPath: suggested,
        filters: [{ name: "Zip archive", extensions: ["zip"] }],
      });
      if (!savePath || Array.isArray(savePath)) return;
      const out = await exportInstanceModsZip({ instanceId: inst.id, outputPath: savePath });
      setInstallNotice(`Exported ${out.files_count} file(s) to ${out.output_path}`);
    } catch (e: any) {
      setLauncherErr(e?.toString?.() ?? String(e));
    }
  }

  async function onOpenInstancePath(
    inst: Instance,
    target: "instance" | "mods" | "resourcepacks" | "shaderpacks" | "saves" | "launch-log" | "crash-log"
  ) {
    setLauncherErr(null);
    setInstallNotice(null);
    try {
      const out = await openInstancePath({ instanceId: inst.id, target });
      setInstallNotice(
        out.target === "launch-log"
          ? `Opened launch log: ${out.path}`
          : out.target === "crash-log"
            ? `Opened latest crash report: ${out.path}`
          : `Opened ${out.target} folder: ${out.path}`
      );
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setError(msg);
    }
  }

  async function onOpenLaunchLog(inst: Instance) {
    await onOpenInstancePath(inst, "launch-log");
  }

  async function persistUpdateSchedulerPrefs(next: {
    cadence?: SchedulerCadence;
    autoApplyMode?: SchedulerAutoApplyMode;
    applyScope?: SchedulerApplyScope;
  }) {
    setUpdatePrefsBusy(true);
    setUpdatePrefsSavedFlash(false);
    setScheduledUpdateErr(null);
    try {
      const settings = await setLauncherSettings({
        updateCheckCadence: next.cadence ?? updateCheckCadence,
        updateAutoApplyMode: next.autoApplyMode ?? updateAutoApplyMode,
        updateApplyScope: next.applyScope ?? updateApplyScope,
      });
      setLauncherSettingsState(settings);
      setUpdateCheckCadence(normalizeUpdateCheckCadence(settings.update_check_cadence));
      setUpdateAutoApplyMode(normalizeUpdateAutoApplyMode(settings.update_auto_apply_mode));
      setUpdateApplyScope(normalizeUpdateApplyScope(settings.update_apply_scope));
      setUpdatePrefsSavedFlash(true);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setScheduledUpdateErr(msg);
      setError(msg);
    } finally {
      setUpdatePrefsBusy(false);
    }
  }

  async function onSaveLauncherPrefs() {
    setLauncherBusy(true);
    setLauncherErr(null);
    try {
      const next = await setLauncherSettings({
        defaultLaunchMethod: launchMethodPick,
        javaPath: javaPathDraft,
        oauthClientId: oauthClientIdDraft,
      });
      setLauncherSettingsState(next);
      setUpdateCheckCadence(normalizeUpdateCheckCadence(next.update_check_cadence));
      setUpdateAutoApplyMode(normalizeUpdateAutoApplyMode(next.update_auto_apply_mode));
      setUpdateApplyScope(normalizeUpdateApplyScope(next.update_apply_scope));
      setInstallNotice(t("settings.launch.saved_notice"));
    } catch (e: any) {
      setLauncherErr(e?.toString?.() ?? String(e));
    } finally {
      setLauncherBusy(false);
    }
  }

  async function onSetAppLanguage(nextLanguage: AppLanguage) {
    if (nextLanguage === appLanguage) return;
    setAppLanguageBusy(true);
    setLauncherErr(null);
    try {
      const next = await setLauncherSettings({
        appLanguage: nextLanguage,
      });
      setLauncherSettingsState(next);
      const savedLanguage = normalizeAppLanguage(next.app_language ?? nextLanguage);
      setInstallNotice(
        translateAppText(savedLanguage, "settings.language.saved_notice", {
          language: getAppLanguageOption(savedLanguage).nativeLabel,
        })
      );
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setError(msg);
    } finally {
      setAppLanguageBusy(false);
    }
  }

  async function onToggleAutoIdentifyLocalJars() {
    const nextEnabled = !Boolean(launcherSettings?.auto_identify_local_jars);
    setAutoIdentifyLocalJarsBusy(true);
    setLauncherErr(null);
    try {
      const next = await setLauncherSettings({
        autoIdentifyLocalJars: nextEnabled,
      });
      setLauncherSettingsState(next);
      setInstallNotice(`Automatic identify local files ${nextEnabled ? "enabled" : "disabled"}.`);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setError(msg);
    } finally {
      setAutoIdentifyLocalJarsBusy(false);
    }
  }

  async function onToggleAutoMicPermissionPrompt() {
    const nextEnabled = !(launcherSettings?.auto_trigger_mic_permission_prompt ?? true);
    setAutoMicPromptSettingBusy(true);
    setLauncherErr(null);
    try {
      const next = await setLauncherSettings({
        autoTriggerMicPermissionPrompt: nextEnabled,
      });
      setLauncherSettingsState(next);
      setInstallNotice(
        `Automatic microphone permission prompt ${nextEnabled ? "enabled" : "disabled"}.`
      );
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setError(msg);
    } finally {
      setAutoMicPromptSettingBusy(false);
    }
  }

  async function onToggleDiscordPresenceEnabled() {
    const nextEnabled = !(launcherSettings?.discord_presence_enabled ?? true);
    setDiscordPresenceBusy(true);
    setLauncherErr(null);
    try {
      const next = await setLauncherSettings({
        discordPresenceEnabled: nextEnabled,
      });
      setLauncherSettingsState(next);
      setInstallNotice(`Discord presence ${nextEnabled ? "enabled" : "disabled"}.`);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setError(msg);
    } finally {
      setDiscordPresenceBusy(false);
    }
  }

  async function onSetDiscordPresenceDetailLevel(level: "minimal" | "expanded") {
    setDiscordPresenceBusy(true);
    setLauncherErr(null);
    try {
      const next = await setLauncherSettings({
        discordPresenceDetailLevel: level,
      });
      setLauncherSettingsState(next);
      setInstallNotice(`Discord presence detail updated to ${level}.`);
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setError(msg);
    } finally {
      setDiscordPresenceBusy(false);
    }
  }

  async function onCheckAppUpdate(options?: { silent?: boolean }) {
    const silent = options?.silent === true;
    setAppUpdaterBusy(true);
    if (!silent) {
      setLauncherErr(null);
    }
    setAppUpdaterLastError(null);
    try {
      const currentVersion = await getVersion().catch(() => appVersion || "unknown");
      if (currentVersion && currentVersion !== appVersion) {
        setAppVersion(currentVersion);
      }
      const result = await checkUpdate();
      const manifest = (result as any)?.manifest ?? {};
      const latestVersionRaw = String(manifest?.version ?? "").trim();
      const latestVersion = latestVersionRaw || null;
      const releaseNotesRaw = String(manifest?.body ?? manifest?.notes ?? "").trim();
      const releaseNotes = releaseNotesRaw || null;
      const pubDateRaw = String(manifest?.date ?? manifest?.pub_date ?? "").trim();
      const pubDate = pubDateRaw || null;

      const nextState: AppUpdaterState = {
        checked_at: new Date().toISOString(),
        current_version: String(currentVersion || "unknown"),
        available:
          Boolean(result?.shouldUpdate) &&
          normalizeVersionLabel(latestVersion) !== normalizeVersionLabel(String(currentVersion || "")),
        latest_version: latestVersion,
        release_notes: releaseNotes,
        pub_date: pubDate,
      };
      setAppUpdaterState(nextState);
      setAppUpdaterLastError(null);

      if (!silent) {
        // Updater status is surfaced in the dedicated app update banner.
        // Clear legacy updater notices so we don't render duplicate banners.
        setInstallNotice((current) => {
          const text = String(current ?? "").toLowerCase();
          if (
            text.startsWith("app update available") ||
            text.startsWith("openjar launcher is up to date")
          ) {
            return null;
          }
          return current;
        });
      }
    } catch (e: any) {
      const raw = e?.toString?.() ?? String(e);
      const lower = raw.toLowerCase();
      const guidance =
        lower.includes("updater is disabled") ||
        lower.includes("updater disabled") ||
        lower.includes("pubkey") ||
        lower.includes("signature") ||
        lower.includes("endpoints") ||
        lower.includes("404") ||
        lower.includes("latest.json")
          ? "App updater is not fully configured for this build yet. Configure updater pubkey/endpoints and publish signed release metadata."
          : raw;
      setAppUpdaterLastError(guidance);
      if (!silent) {
        setLauncherErr(guidance);
      }
    } finally {
      setAppUpdaterBusy(false);
    }
  }

  async function onInstallAppUpdate() {
    if (!appUpdaterState?.available) return;
    const latest = appUpdaterState.latest_version ? `v${appUpdaterState.latest_version}` : "the latest version";
    const confirmed = window.confirm(
      `Install ${latest} now? OpenJar Launcher will restart after update.`
    );
    if (!confirmed) return;
    setAppUpdaterInstallBusy(true);
    setLauncherErr(null);
    try {
      await installUpdate();
      setInstallNotice("App update installed. Restarting now…");
      await relaunch();
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setLauncherErr(msg);
      setAppUpdaterLastError(msg);
    } finally {
      setAppUpdaterInstallBusy(false);
    }
  }

  useEffect(() => {
    if (!appUpdaterAutoCheck || appUpdaterAutoCheckStartedRef.current) return;
    appUpdaterAutoCheckStartedRef.current = true;
    void onCheckAppUpdate({ silent: true });
  }, [appUpdaterAutoCheck]);

  const appUpdateBannerStateKey = useMemo(
    () =>
      [
        appUpdaterBusy ? "checking" : "idle",
        appUpdaterInstallBusy ? "installing" : "not-installing",
        appUpdaterLastError ? `error:${appUpdaterLastError}` : "no-error",
        appUpdaterState
          ? [
              appUpdaterState.checked_at,
              appUpdaterState.current_version,
              appUpdaterState.available ? "available" : "not-available",
              appUpdaterState.latest_version ?? "",
              appUpdaterState.pub_date ?? "",
            ].join("|")
          : "no-state",
      ].join("|"),
    [appUpdaterBusy, appUpdaterInstallBusy, appUpdaterLastError, appUpdaterState]
  );

  useEffect(() => {
    setAppUpdateBannerDismissedKey(null);
  }, [appUpdateBannerStateKey]);

  useEffect(() => {
    if (appUpdaterBusy || appUpdaterInstallBusy) return;
    const timer = window.setTimeout(() => {
      setAppUpdateBannerDismissedKey(appUpdateBannerStateKey);
    }, APP_UPDATE_BANNER_AUTO_HIDE_MS);
    return () => window.clearTimeout(timer);
  }, [appUpdateBannerStateKey, appUpdaterBusy, appUpdaterInstallBusy]);

  function onResetUiSettings() {
    const next = defaultUiSettingsSnapshot();
    clearUiSettingsStorage();
    setTheme(next.theme);
    setAccentPreset(next.accentPreset);
    setAccentStrength(next.accentStrength);
    setMotionPreset(next.motionPreset);
    setDensityPreset(next.densityPreset);
    setInstallNotice("UI settings reset to defaults.");
  }

  async function onBeginMicrosoftLogin() {
    setLauncherBusy(true);
    setLauncherErr(null);
    setMsLoginState(null);
    setMsCodePrompt(null);
    setMsCodePromptVisible(false);
    setMsCodeCopied(false);
    try {
      const start: BeginMicrosoftLoginResult = await beginMicrosoftLogin();
      setMsLoginSessionId(start.session_id);
      const verifyUrl = start.verification_uri ?? start.auth_url;
      if (start.user_code) {
        setMsCodePrompt({
          code: start.user_code,
          verificationUrl: verifyUrl,
        });
        setMsCodePromptVisible(true);
        setInstallNotice(
          `Microsoft sign-in started. Open ${verifyUrl} and enter code ${start.user_code}. If the browser says "Prism Launcher", that's expected when using the bundled client ID.`
        );
      } else {
        setInstallNotice("Microsoft sign-in started in your browser.");
      }
    } catch (e: any) {
      setLauncherErr(e?.toString?.() ?? String(e));
    } finally {
      setLauncherBusy(false);
    }
  }

  async function onSelectAccount(accountId: string) {
    setLauncherBusy(true);
    setLauncherErr(null);
    try {
      const settings = await selectLauncherAccount({ accountId });
      setLauncherSettingsState(settings);
      setUpdateCheckCadence(normalizeUpdateCheckCadence(settings.update_check_cadence));
      setUpdateAutoApplyMode(normalizeUpdateAutoApplyMode(settings.update_auto_apply_mode));
      setUpdateApplyScope(normalizeUpdateApplyScope(settings.update_apply_scope));
      await refreshAccountDiagnostics();
      setInstallNotice("Launcher account selected.");
    } catch (e: any) {
      setLauncherErr(e?.toString?.() ?? String(e));
    } finally {
      setLauncherBusy(false);
    }
  }

  async function onLogoutAccount(accountId: string) {
    setLauncherBusy(true);
    setLauncherErr(null);
    try {
      const accounts = await logoutMicrosoftAccount({ accountId });
      setLauncherAccounts(accounts);
      setSettingsAccountManageId((prev) => (prev === accountId ? null : prev));
      const settings = await getLauncherSettings();
      setLauncherSettingsState(settings);
      setUpdateCheckCadence(normalizeUpdateCheckCadence(settings.update_check_cadence));
      setUpdateAutoApplyMode(normalizeUpdateAutoApplyMode(settings.update_auto_apply_mode));
      setUpdateApplyScope(normalizeUpdateApplyScope(settings.update_apply_scope));
      await refreshAccountDiagnostics();
      setInstallNotice("Microsoft account disconnected.");
    } catch (e: any) {
      setLauncherErr(e?.toString?.() ?? String(e));
    } finally {
      setLauncherBusy(false);
    }
  }

  function storeScheduledUpdateResult(
    inst: Instance,
    result: ContentUpdateCheckResult | null,
    checkedAtIso?: string,
    errorMessage?: string | null
  ) {
    const checkedAt = checkedAtIso ?? new Date().toISOString();
    setScheduledUpdateEntriesByInstance((prev) => ({
      ...prev,
      [inst.id]: {
        instance_id: inst.id,
        instance_name: inst.name,
        checked_at: checkedAt,
        checked_entries: result?.checked_entries ?? 0,
        update_count: result?.update_count ?? 0,
        updates: result?.updates ?? [],
        error: errorMessage ? String(errorMessage) : null,
      },
    }));
  }

  function storeScheduledAppliedUpdateResult(
    inst: Instance,
    updates: ContentUpdateInfo[],
    appliedAtIso?: string,
    warnings?: string[] | null
  ) {
    if (updates.length === 0) return;
    const appliedAt = appliedAtIso ?? new Date().toISOString();
    setScheduledAppliedUpdatesByInstance((prev) => ({
      ...prev,
      [inst.id]: {
        instance_id: inst.id,
        instance_name: inst.name,
        applied_at: appliedAt,
        updated_entries: updates.length,
        updates,
        warnings: (warnings ?? []).map((warning) => String(warning ?? "")).filter(Boolean),
      },
    }));
  }

  async function runScheduledUpdateChecks(
    reason: "manual" | "scheduled" = "manual",
    options?: { contentTypes?: string[] }
  ) {
    if (scheduledUpdateRunningRef.current) return;
    if (instances.length === 0) return;
    const contentTypes = (options?.contentTypes ?? [])
      .map((value) => String(value ?? "").trim())
      .filter(Boolean);
    const hasContentTypeFilter = contentTypes.length > 0;
    const contentScopeLabel = summarizeUpdateContentTypeSelection(
      contentTypes
        .map((value) => normalizeUpdatableContentType(value))
        .filter((value): value is UpdatableContentType => Boolean(value))
    );
    const runStartedPerf = performance.now();
    const runStartedAt = Date.now();
    let runSucceeded = false;
    scheduledUpdateRunningRef.current = true;
    setScheduledUpdateBusy(true);
    setScheduledUpdateRunStartedAt(runStartedAt);
    setScheduledUpdateRunCompleted(0);
    setScheduledUpdateRunTotal(instances.length);
    setScheduledUpdateRunElapsedSeconds(0);
    setScheduledUpdateRunEtaSeconds(null);
    if (reason === "manual") setScheduledUpdateErr(null);
    const checkedAt = new Date().toISOString();
    let completed = 0;
    let autoAppliedInstances = 0;
    let autoAppliedEntries = 0;
    const workerLimit = scheduledUpdateWorkerLimit(instances.length);
    const canAutoApplyInRun = updateAutoApplyMode !== "never" && (
      reason === "scheduled" || updateApplyScope === "scheduled_and_manual"
    );

    const processInstance = async (inst: Instance) => {
      const instanceStartedAt = performance.now();
      let instanceSuccess = false;
      try {
        const result = await checkInstanceContentUpdates({
          instanceId: inst.id,
          ...(hasContentTypeFilter ? { contentTypes } : {}),
        });
        const shouldAutoApplyForInstance =
          canAutoApplyInRun &&
          result.update_count > 0 &&
          (updateAutoApplyMode === "all_instances" ||
            (updateAutoApplyMode === "opt_in_instances" &&
              Boolean(inst.settings?.auto_update_installed_content)));
        if (shouldAutoApplyForInstance) {
          try {
            const applyResult = await updateAllInstanceContent({
              instanceId: inst.id,
              ...(hasContentTypeFilter ? { contentTypes } : {}),
            });
            const appliedUpdates = pickAppliedUpdatesFromCheck(
              result,
              applyResult.updated_entries ?? 0
            );
            autoAppliedInstances += 1;
            autoAppliedEntries += Math.max(0, applyResult.updated_entries ?? 0);
            const estimatedAfterApply = estimatePostApplyUpdateCheck(
              result,
              applyResult.updated_entries ?? 0
            );
            storeScheduledUpdateResult(inst, estimatedAfterApply, checkedAt, null);
            storeScheduledAppliedUpdateResult(
              inst,
              appliedUpdates,
              checkedAt,
              applyResult.warnings ?? []
            );
            if (route === "instance" && selectedId === inst.id) {
              setUpdateCheck(estimatedAfterApply);
              await refreshInstalledMods(inst.id);
            }
            if ((applyResult.warnings?.length ?? 0) > 0) {
              appendInstanceActivity(
                inst.id,
                applyResult.warnings.slice(0, 4).map((warning) => `Update warning: ${warning}`),
                "warn"
              );
            }
          } catch (applyErr: any) {
            const applyMsg = applyErr?.toString?.() ?? String(applyErr);
            storeScheduledUpdateResult(
              inst,
              result,
              checkedAt,
              `Auto-apply failed: ${applyMsg}`
            );
            if (route === "instance" && selectedId === inst.id) {
              setUpdateCheck(result);
            }
          }
        } else {
          storeScheduledUpdateResult(inst, result, checkedAt, null);
          if (route === "instance" && selectedId === inst.id) {
            setUpdateCheck(result);
          }
        }
        instanceSuccess = true;
      } catch (err: any) {
        storeScheduledUpdateResult(
          inst,
          null,
          checkedAt,
          err?.toString?.() ?? String(err)
        );
      } finally {
        completed += 1;
        const elapsedSeconds = Math.max(0, (performance.now() - runStartedPerf) / 1000);
        setScheduledUpdateRunCompleted(completed);
        setScheduledUpdateRunElapsedSeconds(elapsedSeconds);
        if (completed >= instances.length) {
          setScheduledUpdateRunEtaSeconds(0);
        } else if (completed > 0) {
          const remaining = instances.length - completed;
          const avgSecondsPerInstance = elapsedSeconds / completed;
          setScheduledUpdateRunEtaSeconds(Math.max(0, avgSecondsPerInstance * remaining));
        } else {
          setScheduledUpdateRunEtaSeconds(null);
        }
        recordPerfAction(
          "check_instance_updates",
          instanceSuccess ? "ok" : "error",
          instanceStartedAt,
          inst.id
        );
      }
    };

    try {
      let cursor = 0;
      const workers = Array.from({ length: workerLimit }, async () => {
        while (true) {
          const idx = cursor++;
          if (idx >= instances.length) break;
          await processInstance(instances[idx]);
        }
      });
      await Promise.all(workers);
      runSucceeded = true;
      setScheduledUpdateLastRunAt(checkedAt);
      if (reason === "manual") {
        const autoAppliedMsg =
          autoAppliedInstances > 0
            ? ` Auto-applied ${autoAppliedEntries} update${autoAppliedEntries === 1 ? "" : "s"} across ${autoAppliedInstances} instance${autoAppliedInstances === 1 ? "" : "s"}.`
            : "";
        setInstallNotice(
          `Checked ${completed} instance${completed === 1 ? "" : "s"} for ${contentScopeLabel} updates.${autoAppliedMsg}`
        );
      } else if (autoAppliedInstances > 0) {
        setInstallNotice(
          `Auto-applied ${autoAppliedEntries} update${autoAppliedEntries === 1 ? "" : "s"} across ${autoAppliedInstances} instance${autoAppliedInstances === 1 ? "" : "s"}.`
        );
      }
    } catch (err: any) {
      if (reason === "manual") {
        setScheduledUpdateErr(err?.toString?.() ?? String(err));
      }
    } finally {
      const elapsedSeconds = Math.max(0, (performance.now() - runStartedPerf) / 1000);
      setScheduledUpdateRunElapsedSeconds(elapsedSeconds);
      setScheduledUpdateRunEtaSeconds(0);
      recordPerfAction(
        "run_scheduled_update_checks",
        runSucceeded ? "ok" : "error",
        runStartedPerf,
        `${reason}:${instances.length}`
      );
      scheduledUpdateRunningRef.current = false;
      setScheduledUpdateBusy(false);
    }
  }

  async function onCheckUpdates(
    inst: Instance,
    options?: {
      autoApplyIfConfigured?: boolean;
      syncSelectedInstanceMods?: boolean;
      quietSuccessNotice?: boolean;
      contentTypes?: string[];
      persistScheduledCache?: boolean;
    }
  ) {
    const startedAt = performance.now();
    let succeeded = false;
    const contentTypes = (options?.contentTypes ?? [])
      .map((value) => String(value ?? "").trim())
      .filter(Boolean);
    const shouldPersistScheduledCache = options?.persistScheduledCache !== false;
    setUpdateBusy(true);
    setUpdateErr(null);
    try {
      const res = await checkInstanceContentUpdates({
        instanceId: inst.id,
        ...(contentTypes.length > 0 ? { contentTypes } : {}),
      });
      let nextCheck = res;
      const shouldAutoApply =
        Boolean(options?.autoApplyIfConfigured) &&
        res.update_count > 0 &&
        shouldAutoApplyManualChecksForInstance(inst);

      if (shouldAutoApply) {
        const applyResult = await updateAllInstanceContent({
          instanceId: inst.id,
          ...(contentTypes.length > 0 ? { contentTypes } : {}),
        });
        nextCheck = estimatePostApplyUpdateCheck(res, applyResult.updated_entries ?? 0);
        if (!options?.quietSuccessNotice) {
          if ((applyResult.warnings?.length ?? 0) > 0) {
            const warningSummary = summarizeWarnings(applyResult.warnings ?? [], 2);
            setInstallNotice(
              `Recheck complete. Updated ${applyResult.updated_entries} entr${applyResult.updated_entries === 1 ? "y" : "ies"} (${nextCheck.update_count} remaining) with ${applyResult.warnings.length} warning${applyResult.warnings.length === 1 ? "" : "s"}: ${warningSummary}`
            );
          } else {
            setInstallNotice(
              `Recheck complete. Updated ${applyResult.updated_entries} entr${applyResult.updated_entries === 1 ? "y" : "ies"} (${nextCheck.update_count} remaining).`
            );
          }
        }
        if ((applyResult.warnings?.length ?? 0) > 0) {
          appendInstanceActivity(
            inst.id,
            applyResult.warnings.slice(0, 6).map((warning) => `Update warning: ${warning}`),
            "warn"
          );
        }
      } else if (!options?.quietSuccessNotice) {
        if (res.update_count === 0) {
          if ((res.warnings?.length ?? 0) > 0) {
            const warningSummary = summarizeWarnings(res.warnings ?? [], 2);
            setInstallNotice(
              `Checked ${res.checked_entries} entr${res.checked_entries === 1 ? "y" : "ies"}; all up to date with ${res.warnings.length} warning${res.warnings.length === 1 ? "" : "s"}: ${warningSummary}`
            );
          } else {
            setInstallNotice("All tracked content is up to date.");
          }
        } else if (options?.autoApplyIfConfigured && !shouldAutoApplyManualChecksForInstance(inst)) {
          setInstallNotice(
            `${res.update_count} update${res.update_count === 1 ? "" : "s"} found. Auto-apply for manual checks is disabled for this instance.`
          );
        } else {
          setInstallNotice(
            `${res.update_count} update${res.update_count === 1 ? "" : "s"} available.`
          );
        }
      }

      if (selectedId === inst.id) {
        setUpdateCheck(nextCheck);
        if (options?.syncSelectedInstanceMods) {
          await refreshInstalledMods(inst.id);
        }
      }
      if (shouldPersistScheduledCache) {
        storeScheduledUpdateResult(inst, nextCheck, new Date().toISOString(), null);
      }
      succeeded = true;
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setUpdateErr(msg);
      if (shouldPersistScheduledCache) {
        storeScheduledUpdateResult(inst, null, new Date().toISOString(), msg);
      }
    } finally {
      recordPerfAction(
        "check_instance_updates_manual",
        succeeded ? "ok" : "error",
        startedAt,
        inst.id
      );
      setUpdateBusy(false);
    }
  }

  async function onUpdateAll(
    inst: Instance,
    contentView: "mods" | "resourcepacks" | "datapacks" | "shaders" = "mods"
  ) {
    const startedAt = performance.now();
    let succeeded = false;
    const contentTypes = [instanceContentTypeToBackend(contentView)];
    const sectionLabel = instanceContentSectionLabel(contentView);
    setUpdateAllBusy(true);
    setUpdateErr(null);
    setError(null);
    try {
      setInstallNotice(`Updating all ${sectionLabel}… this can take a bit on larger packs.`);
      const res = await updateAllInstanceContent({
        instanceId: inst.id,
        contentTypes,
      });
      if (selectedId === inst.id) {
        void refreshInstalledMods(inst.id);
      }
      const shouldSkipImmediateRecheck = hasGithubRateLimitWarning(res.warnings ?? []);
      if (!shouldSkipImmediateRecheck) {
        window.setTimeout(() => {
          void onCheckUpdates(inst, {
            quietSuccessNotice: true,
            contentTypes,
            persistScheduledCache: false,
          });
        }, 650);
      }
      if (res.warnings.length > 0) {
        const warningSummary = summarizeWarnings(res.warnings, 3);
        setInstallNotice(
          `Updated ${res.updated_entries} entr${res.updated_entries === 1 ? "y" : "ies"} in ${sectionLabel} with ${res.warnings.length} warning${res.warnings.length === 1 ? "" : "s"}: ${warningSummary}`
        );
        appendInstanceActivity(
          inst.id,
          res.warnings.slice(0, 6).map((warning) => `Update warning: ${warning}`),
          "warn"
        );
      } else {
        setInstallNotice(
          `Updated ${res.updated_entries} entr${res.updated_entries === 1 ? "y" : "ies"} in ${sectionLabel}.`
        );
      }
      succeeded = true;
    } catch (e: any) {
      const msg = e?.toString?.() ?? String(e);
      setUpdateErr(msg);
      setError(msg);
    } finally {
      recordPerfAction(
        "update_all_instance_content",
        succeeded ? "ok" : "error",
        startedAt,
        inst.id
      );
      setUpdateAllBusy(false);
    }
  }

  useEffect(() => {
    if (route !== "instance" || !selectedId) {
      installedModsLoadSeqRef.current += 1;
      setInstalledMods([]);
      setInstalledModsInstanceId(null);
      setModsErr(null);
      setUpdateCheck(null);
      setUpdateErr(null);
      setSnapshots([]);
      setRollbackSnapshotId(null);
      setWorldRollbackBusyById({});
      return;
    }
    installedModsLoadSeqRef.current += 1;
    setInstalledMods([]);
    setInstalledModsInstanceId(null);
    setSelectedModVersionIds([]);
    setToggleBusyVersion(null);
    setProviderSwitchBusyKey(null);
    setPinBusyVersion(null);
    setDependencyInstallBusyVersion(null);
    setGithubAttachBusyVersion(null);
    setGithubAttachTarget(null);
    setGithubAttachErr(null);
    refreshInstalledMods(selectedId);
    refreshSnapshots(selectedId);
    void refreshInstanceHealthPanelData(selectedId);
    listInstanceWorlds({ instanceId: selectedId })
      .then((worlds) => setInstanceWorlds(worlds))
      .catch(() => setInstanceWorlds([]));
    setUpdateCheck(null);
    setUpdateErr(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route, selectedId]);

  useEffect(() => {
    if (route !== "instance" || !selectedId || instanceTab !== "content") return;
    setUpdateCheck(null);
    setUpdateErr(null);
  }, [route, selectedId, instanceTab, instanceContentType]);

  useEffect(() => {
    if (route !== "instance" || instanceTab !== "content" || !selectedId) return;
    let cancelled = false;
    const instanceId = selectedId;
    const syncFromDisk = async () => {
      const refreshed = await listInstalledMods(instanceId).catch(() => null);
      if (!refreshed || cancelled) return;
      applyInstalledModsForInstance(instanceId, (prev) => {
        if (
          prev.length === refreshed.length &&
          prev.every((entry, index) => {
            const next = refreshed[index];
            return (
              next &&
              entry.version_id === next.version_id &&
              entry.file_exists === next.file_exists &&
              entry.enabled === next.enabled &&
              entry.source === next.source &&
              entry.filename === next.filename
            );
          })
        ) {
          return prev;
        }
        return refreshed;
      });
    };
    const refreshOnForeground = () => {
      if (document.visibilityState !== "visible") return;
      void syncFromDisk();
    };
    const interval = window.setInterval(() => {
      if (document.visibilityState !== "visible") return;
      void syncFromDisk();
    }, 5000);
    window.addEventListener("focus", refreshOnForeground);
    document.addEventListener("visibilitychange", refreshOnForeground);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
      window.removeEventListener("focus", refreshOnForeground);
      document.removeEventListener("visibilitychange", refreshOnForeground);
    };
  }, [route, instanceTab, selectedId, applyInstalledModsForInstance]);

  useEffect(() => {
    if (route !== "instance" || !selectedId) {
      setInstanceFriendLinkStatus(null);
      return;
    }
    let cancelled = false;
    const loadStatus = () => {
      void getFriendLinkStatus({ instanceId: selectedId })
        .then(async (status) => {
          if (cancelled) return;
          setInstanceFriendLinkStatus(status);
          setFriendLinkStatusByInstance((prev) => ({ ...prev, [selectedId]: status }));
          if (!status.linked) {
            setFriendLinkDriftByInstance((prev) => {
              const next = { ...prev };
              delete next[selectedId];
              return next;
            });
            return;
          }
          const preview = await previewFriendLinkDrift({ instanceId: selectedId }).catch(() => null);
          if (cancelled || !preview) return;
          setFriendLinkDriftByInstance((prev) => ({ ...prev, [selectedId]: preview }));
        })
        .catch(() => {
          if (!cancelled) {
            setInstanceFriendLinkStatus(null);
            setFriendLinkDriftByInstance((prev) => {
              const next = { ...prev };
              delete next[selectedId];
              return next;
            });
          }
        });
    };
    loadStatus();
    const timer = window.setInterval(loadStatus, 10000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [route, selectedId, instanceLinksOpen, friendConflictResult]);

  useEffect(() => {
    void refreshQuickPlayServers({ silent: true });
  }, []);

  useEffect(() => {
    if (selectedId) {
      setQuickPlayDraftBoundInstanceId(selectedId);
    }
  }, [selectedId]);

  useEffect(() => {
    if (route !== "instance" || !selectedId) return;
    void refreshInstanceHistory(selectedId, { silent: true });
  }, [route, selectedId]);

  useEffect(() => {
    if (route !== "instance" || !selectedId) return;
    const timer = window.setInterval(() => {
      if (document.visibilityState !== "visible") return;
      void refreshInstanceHistory(selectedId, { silent: true });
    }, 15000);
    return () => window.clearInterval(timer);
  }, [route, selectedId]);

  useEffect(() => {
    const targetIds = new Set<string>();
    if (selectedId) targetIds.add(selectedId);
    if (route === "home") {
      const homeIds = [...instances]
        .sort((a, b) => (parseDateLike(b.created_at)?.getTime() ?? 0) - (parseDateLike(a.created_at)?.getTime() ?? 0))
        .slice(0, 6)
        .map((inst) => inst.id);
      for (const id of homeIds) targetIds.add(id);
    }
    if (targetIds.size === 0) return;
    let cancelled = false;
    const ids = Array.from(targetIds);
    const loadStatuses = () => {
      void Promise.all(
        ids.map(async (instanceId) => {
          const status = await getFriendLinkStatus({ instanceId }).catch(() => null);
          const preview =
            status?.linked ? await previewFriendLinkDrift({ instanceId }).catch(() => null) : null;
          return { instanceId, status, preview };
        })
      ).then((items) => {
        if (cancelled) return;
        setFriendLinkStatusByInstance((prev) => {
          const next = { ...prev };
          for (const item of items) {
            if (item.status) {
              next[item.instanceId] = item.status;
            } else {
              delete next[item.instanceId];
            }
          }
          return next;
        });
        setFriendLinkDriftByInstance((prev) => {
          const next = { ...prev };
          for (const item of items) {
            if (item.preview && item.status?.linked) {
              next[item.instanceId] = item.preview;
            } else {
              delete next[item.instanceId];
            }
          }
          return next;
        });
        for (const item of items) {
          const signature = friendLinkDriftSignature(item.preview);
          const lastSignature = friendLinkDriftAnnounceRef.current[item.instanceId] ?? "";
          if (item.preview && signature && signature !== lastSignature) {
            if (item.preview.status === "unsynced" && item.preview.total_changes > 0) {
              const modItems = item.preview.items.filter((row) => row.kind === "lock_entry");
              if (modItems.length > 0) {
                const added = modItems.filter((row) => row.change === "added").length;
                const removed = modItems.filter((row) => row.change === "removed").length;
                const changed = modItems.filter((row) => row.change === "changed").length;
                const msg = `Friend Link mod drift detected: +${added} / -${removed} / ~${changed} not synced yet.`;
                appendInstanceActivity(item.instanceId, [msg], "warn");
              } else {
                appendInstanceActivity(
                  item.instanceId,
                  [`Friend Link config drift detected (${item.preview.total_changes} item${item.preview.total_changes === 1 ? "" : "s"}). Mods are aligned.`],
                  "info"
                );
              }
            } else if (lastSignature && item.preview.status === "in_sync") {
              appendInstanceActivity(item.instanceId, ["Friend Link is back in sync."], "success");
            }
          }
          if (item.preview) {
            friendLinkDriftAnnounceRef.current[item.instanceId] = signature;
          } else {
            delete friendLinkDriftAnnounceRef.current[item.instanceId];
          }
        }
      });
    };
    loadStatuses();
    const timer = window.setInterval(loadStatuses, 12000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [route, instances, selectedId, instanceLinksOpen, friendConflictResult]);

  useEffect(() => {
    if (instances.length === 0) return;
    let cancelled = false;

    const runAutoSyncSweep = async () => {
      if (cancelled || friendLinkAutoSyncBusyRef.current) return;
      friendLinkAutoSyncBusyRef.current = true;
      try {
        const activeIds = new Set(instances.map((inst) => inst.id));
        for (const id of Object.keys(friendLinkAutoSyncLastSignatureRef.current)) {
          if (!activeIds.has(id)) delete friendLinkAutoSyncLastSignatureRef.current[id];
        }
        for (const id of Object.keys(friendLinkAutoSyncInFlightRef.current)) {
          if (!activeIds.has(id)) delete friendLinkAutoSyncInFlightRef.current[id];
        }

        for (const inst of instances) {
          if (cancelled) break;

          const prefs = readFriendSyncPrefs(inst.id);
          if (prefs.policy !== "auto_metadata" && prefs.policy !== "auto_all") {
            delete friendLinkAutoSyncLastSignatureRef.current[inst.id];
            continue;
          }
          if (prefs.snoozed_until > Date.now()) continue;
          if (friendLinkAutoSyncInFlightRef.current[inst.id]) continue;

          const status = await getFriendLinkStatus({ instanceId: inst.id }).catch(() => null);
          if (!status?.linked || (status.peers?.length ?? 0) === 0) {
            delete friendLinkAutoSyncLastSignatureRef.current[inst.id];
            continue;
          }

          const preview = await previewFriendLinkDrift({ instanceId: inst.id }).catch(() => null);
          if (!preview || preview.status !== "unsynced" || preview.total_changes <= 0) {
            delete friendLinkAutoSyncLastSignatureRef.current[inst.id];
            continue;
          }

          const signature = friendLinkDriftSignature(preview);
          if (!signature || friendLinkAutoSyncLastSignatureRef.current[inst.id] === signature) {
            continue;
          }

          const guardMax = Math.max(1, Number(status.max_auto_changes ?? 25) || 25);
          if (preview.total_changes > guardMax) {
            friendLinkAutoSyncLastSignatureRef.current[inst.id] = signature;
            appendInstanceActivity(
              inst.id,
              [`Friend Link auto-sync paused: ${preview.total_changes} changes exceed your guardrail (${guardMax}).`],
              "warn"
            );
            continue;
          }

          friendLinkAutoSyncInFlightRef.current[inst.id] = true;
          friendLinkAutoSyncLastSignatureRef.current[inst.id] = signature;
          try {
            const out =
              prefs.policy === "auto_all"
                ? await reconcileFriendLink({ instanceId: inst.id, mode: "manual" })
                : await syncFriendLinkSelected({
                    instanceId: inst.id,
                    keys: preview.items.map((item) => item.key),
                    metadataOnly: true,
                  });

            if (cancelled) break;

            if (out.status === "conflicted") {
              setFriendConflictInstanceId(inst.id);
              setFriendConflictResult(out);
            }

            const tone: "info" | "success" | "warn" =
              out.status === "synced" || out.status === "in_sync" ? "success" : out.status === "blocked_untrusted" ? "warn" : "info";
            appendInstanceActivity(
              inst.id,
              [
                `Friend Link auto-sync: ${out.status}. Applied ${out.actions_applied} change${
                  out.actions_applied === 1 ? "" : "s"
                }.`,
              ],
              tone
            );

            if (out.actions_applied > 0 && route === "instance" && selectedId === inst.id) {
              await refreshInstalledMods(inst.id);
            }

            const [nextStatus, nextPreview] = await Promise.all([
              getFriendLinkStatus({ instanceId: inst.id }).catch(() => null),
              previewFriendLinkDrift({ instanceId: inst.id }).catch(() => null),
            ]);

            if (cancelled) break;

            setFriendLinkStatusByInstance((prev) => {
              const next = { ...prev };
              if (nextStatus) next[inst.id] = nextStatus;
              else delete next[inst.id];
              return next;
            });
            if (selectedId === inst.id) {
              setInstanceFriendLinkStatus(nextStatus);
            }

            setFriendLinkDriftByInstance((prev) => {
              const next = { ...prev };
              if (nextStatus?.linked && nextPreview) next[inst.id] = nextPreview;
              else delete next[inst.id];
              return next;
            });

            const announceSig = friendLinkDriftSignature(nextPreview);
            if (announceSig) friendLinkDriftAnnounceRef.current[inst.id] = announceSig;
            else delete friendLinkDriftAnnounceRef.current[inst.id];
          } catch (err: any) {
            delete friendLinkAutoSyncLastSignatureRef.current[inst.id];
            appendInstanceActivity(
              inst.id,
              [`Friend Link auto-sync failed: ${err?.toString?.() ?? String(err)}`],
              "warn"
            );
          } finally {
            friendLinkAutoSyncInFlightRef.current[inst.id] = false;
          }
        }
      } finally {
        friendLinkAutoSyncBusyRef.current = false;
      }
    };

    void runAutoSyncSweep();
    const timer = window.setInterval(() => {
      void runAutoSyncSweep();
    }, FRIEND_LINK_AUTOSYNC_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [instances, route, selectedId]);

  useEffect(() => {
    if (!selectedId) {
      setInstanceWorlds([]);
      setSelectedModVersionIds([]);
      return;
    }
    // Instance route already fetches worlds in its dedicated effect above.
    if (route === "instance") return;
    if (route !== "modpacks") return;
    listInstanceWorlds({ instanceId: selectedId })
      .then((worlds) => setInstanceWorlds(worlds))
      .catch(() => setInstanceWorlds([]));
  }, [route, selectedId]);

  useEffect(() => {
    if (route !== "library" || instances.length === 0) return;
    const missingModCounts = instances.filter((item) => instanceModCountById[item.id] == null);
    const missingDiskUsage = instances.filter((item) => instanceDiskUsageById[item.id] == null);
    if (missingModCounts.length === 0 && missingDiskUsage.length === 0) return;
    let cancelled = false;
    const run = async () => {
      for (const inst of missingModCounts) {
        try {
          const rows = await listInstalledMods(inst.id);
          if (cancelled) return;
          const modCount = rows.filter((row) => normalizeCreatorEntryType(row.content_type) === "mods").length;
          setInstanceModCountById((prev) =>
            prev[inst.id] === modCount ? prev : { ...prev, [inst.id]: modCount }
          );
        } catch {
          if (cancelled) return;
        }
      }
      for (const inst of missingDiskUsage) {
        try {
          const bytes = await getInstanceDiskUsage({ instanceId: inst.id });
          if (cancelled) return;
          if (typeof bytes === "number" && Number.isFinite(bytes) && bytes >= 0) {
            setInstanceDiskUsageById((prev) =>
              prev[inst.id] === bytes ? prev : { ...prev, [inst.id]: bytes }
            );
          }
        } catch {
          if (cancelled) return;
        }
      }
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, [route, instances, instanceModCountById, instanceDiskUsageById]);

  useEffect(() => {
    if (route !== "library") return;
    void refreshStorageOverview({ force: true });
  }, [route, refreshStorageOverview]);

  useEffect(() => {
    if (!storageManagerSelection || storageManagerSelection === "overview") return;
    const relativePath = storageManagerPathBySelection[storageManagerSelection] ?? "";
    void loadStorageEntries(storageManagerSelection, storageDetailMode, relativePath);
  }, [
    loadStorageEntries,
    storageDetailMode,
    storageManagerPathBySelection,
    storageManagerSelection,
  ]);

  useEffect(() => {
    const valid = new Set(
      installedMods.map((m) => installedEntryUiKey(m))
    );
    setSelectedModVersionIds((prev) => {
      const next = prev.filter((id) => valid.has(id));
      return next.length === prev.length ? prev : next;
    });
  }, [installedMods]);

  useEffect(() => {
    const off = listen<InstallProgressEvent>("mod_install_progress", (event) => {
      const payload = event.payload;
      if (!payload) return;
      setInstallProgress(payload);

      const timingKey = `${payload.instance_id}:${payload.project_id}`;
      const nowPerf = performance.now();
      const stage = String(payload.stage ?? "").toLowerCase();
      const hasKnownTransferTotal =
        stage !== "downloading" || Number(payload.total ?? 0) > 0;
      const rawPercent =
        Number.isFinite(payload.percent as number)
          ? Number(payload.percent)
          : stage === "downloading" && Number(payload.total ?? 0) > 0
            ? (Number(payload.downloaded ?? 0) / Number(payload.total)) * 100
            : null;
      const percent = rawPercent == null ? null : Math.max(0, Math.min(100, rawPercent));

      const existing = installProgressTimingRef.current[timingKey];
      const tracker = existing ?? {
        started_at: nowPerf,
        last_at: nowPerf,
        last_percent: Math.max(0, percent ?? 0),
        rate_percent_per_sec: 0,
        last_downloaded: Math.max(0, Number(payload.downloaded ?? 0)),
        rate_bytes_per_sec: 0,
      };
      const elapsedSeconds = Math.max(0, (nowPerf - tracker.started_at) / 1000);
      let etaSeconds: number | null = null;
      let bytesPerSecond: number | null = null;
      if (percent != null) {
        const deltaPercent = percent - tracker.last_percent;
        const deltaSeconds = Math.max(0.001, (nowPerf - tracker.last_at) / 1000);
        if (deltaPercent > 0.01) {
          const instantRate = deltaPercent / deltaSeconds;
          tracker.rate_percent_per_sec =
            tracker.rate_percent_per_sec > 0
              ? tracker.rate_percent_per_sec * 0.68 + instantRate * 0.32
              : instantRate;
        }
        if (hasKnownTransferTotal && tracker.rate_percent_per_sec > 0.01 && percent < 100) {
          etaSeconds = Math.max(0, (100 - percent) / tracker.rate_percent_per_sec);
        } else if (percent >= 100) {
          etaSeconds = 0;
        }
        tracker.last_percent = Math.max(tracker.last_percent, percent);
      }
      if (stage === "downloading") {
        const downloaded = Math.max(0, Number(payload.downloaded ?? 0));
        const deltaSeconds = Math.max(0.001, (nowPerf - tracker.last_at) / 1000);
        const deltaBytes = downloaded - tracker.last_downloaded;
        if (deltaBytes > 0) {
          const instantRate = deltaBytes / deltaSeconds;
          tracker.rate_bytes_per_sec =
            tracker.rate_bytes_per_sec > 0
              ? tracker.rate_bytes_per_sec * 0.68 + instantRate * 0.32
              : instantRate;
        }
        tracker.last_downloaded = downloaded;
        if (tracker.rate_bytes_per_sec > 1) {
          bytesPerSecond = tracker.rate_bytes_per_sec;
        }
      }
      tracker.last_at = nowPerf;
      installProgressTimingRef.current[timingKey] = tracker;

      setInstallProgressElapsedSeconds(elapsedSeconds);
      setInstallProgressEtaSeconds(etaSeconds);
      setInstallProgressBytesPerSecond(bytesPerSecond);

      if (stage === "completed" || stage === "error") {
        delete installProgressTimingRef.current[timingKey];
        setInstallProgressBytesPerSecond(null);
      }
    });
    return () => {
      off.then((unlisten) => unlisten()).catch(() => null);
    };
  }, []);

  useEffect(() => {
    const off = listen(APP_MENU_CHECK_FOR_UPDATES_EVENT, () => {
      void onCheckAppUpdate({ silent: false });
    });
    return () => {
      off.then((unlisten) => unlisten()).catch(() => null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const off = listen<InstanceLaunchStateEvent>("instance_launch_state", (event) => {
      const payload = event.payload;
      if (!payload) return;
      const status = String(payload.status ?? "").toLowerCase();
      const method = String(payload.method ?? "").toLowerCase();
      const message = String(payload.message ?? "").trim();
      const instanceId = String(payload.instance_id ?? "").trim();
      const launchId = String(payload.launch_id ?? "").trim();
      const lowerMessage = message.toLowerCase();
      const userStopRequested = launchId
        ? userRequestedStopLaunchIdsRef.current.has(launchId)
        : false;
      const isExpectedStopEvent =
        lowerMessage.includes("cancelled by user") ||
        lowerMessage.includes("stop requested") ||
        lowerMessage.includes("instance stop requested") ||
        lowerMessage.includes("stopped by user") ||
        userStopRequested;
      const isCleanExitEvent = /some\(0\)|status\s+0/i.test(message);

      if (instanceId) {
        if (status === "starting" || status === "stopped" || status === "exited") {
          void refreshInstanceHealthPanelData(instanceId);
        }
        if (status === "starting") {
          setLaunchProgressChecksByInstance((prev) => ({
            ...prev,
            [instanceId]: mergeLaunchChecksFromMessage(
              prev[instanceId] ?? emptyLaunchHealthChecks(),
              message
            ),
          }));
        }
        if (status === "running" || status === "stopped" || status === "exited") {
          setLaunchBusyInstanceIds((prev) => prev.filter((id) => id !== instanceId));
          setLaunchCancelBusyInstanceId((prev) => (prev === instanceId ? null : prev));
        }
        if (status === "starting" || status === "running") {
          const label = launchStageBadgeLabel(status, message);
          setLaunchStageByInstance((prev) => ({
            ...prev,
            [instanceId]: {
              status,
              label: label || (status === "running" ? "Running" : "Launching"),
              message,
              updated_at: Date.now(),
            },
          }));
        } else if (status === "stopped" || status === "exited") {
          setLaunchStageByInstance((prev) => {
            const next = { ...prev };
            delete next[instanceId];
            return next;
          });
        }

        if (status === "running") {
          setLaunchFailureByInstance((prev) => {
            if (!prev[instanceId]) return prev;
            const next = { ...prev };
            delete next[instanceId];
            return next;
          });
          if (method === "native") {
            setLaunchHealthByInstance((prev) => {
              if (prev[instanceId]) return prev;
              return {
                ...prev,
                [instanceId]: {
                  first_success_at: new Date().toISOString(),
                  checks: {
                    auth: true,
                    assets: true,
                    libraries: true,
                    starting_java: true,
                  },
                },
              };
            });
          }
          setLaunchProgressChecksByInstance((prev) => {
            if (!prev[instanceId]) return prev;
            const next = { ...prev };
            delete next[instanceId];
            return next;
          });
        } else if (status === "stopped" || status === "exited") {
          setLaunchProgressChecksByInstance((prev) => {
            if (!prev[instanceId]) return prev;
            const next = { ...prev };
            delete next[instanceId];
            return next;
          });

          if (status === "exited" && !isCleanExitEvent && message && !isExpectedStopEvent) {
            setLaunchFailureByInstance((prev) => ({
              ...prev,
              [instanceId]: {
                status,
                method,
                message,
                updated_at: Date.now(),
              },
            }));
          } else if (status === "stopped" && message && !isExpectedStopEvent) {
            setLaunchFailureByInstance((prev) => ({
              ...prev,
              [instanceId]: {
                status,
                method,
                message,
                updated_at: Date.now(),
              },
            }));
          } else if (isExpectedStopEvent) {
            setLaunchFailureByInstance((prev) => {
              if (!prev[instanceId]) return prev;
              const next = { ...prev };
              delete next[instanceId];
              return next;
            });
          }
        }
      }

      if (status === "starting" || status === "running") {
        if (message) setInstallNotice(message);
      } else if (status === "exited") {
        if (isExpectedStopEvent) {
          setInstallNotice("Instance stopped.");
        } else if (isCleanExitEvent) {
          setInstallNotice(message || "Game exited normally.");
        } else if (message) {
          setLauncherErr(message);
        }
      } else if (status === "stopped") {
        if (isExpectedStopEvent) {
          setInstallNotice("Instance stopped.");
        } else if (message) {
          setInstallNotice(message);
        }
      }
      if (launchId && status === "exited") {
        userRequestedStopLaunchIdsRef.current.delete(launchId);
      }

      listRunningInstances()
        .then((items) => {
          const next = normalizeRunningInstancesPayload(items);
          setRunningInstances((prev) => (sameRunningInstances(prev, next) ? prev : next));
        })
        .catch(() => null);
    });
    return () => {
      off.then((unlisten) => unlisten()).catch(() => null);
    };
  }, []);

  useEffect(() => {
    if (!msLoginSessionId) return;
    let cancelled = false;
    const t = window.setInterval(async () => {
      try {
        const state = await pollMicrosoftLogin({ sessionId: msLoginSessionId });
        if (cancelled) return;
        setMsLoginState(state);
        if (state.status === "success") {
          const [accounts, settings] = await Promise.all([
            listLauncherAccounts(),
            getLauncherSettings(),
          ]);
          if (cancelled) return;
          setLauncherAccounts(accounts);
          setLauncherSettingsState(settings);
          setUpdateCheckCadence(normalizeUpdateCheckCadence(settings.update_check_cadence));
          setUpdateAutoApplyMode(normalizeUpdateAutoApplyMode(settings.update_auto_apply_mode));
          setUpdateApplyScope(normalizeUpdateApplyScope(settings.update_apply_scope));
          refreshAccountDiagnostics().catch(() => null);
          setInstallNotice(state.message ?? "Microsoft account connected.");
          setMsLoginSessionId(null);
          setMsCodePromptVisible(false);
          setMsCodePrompt(null);
          setMsCodeCopied(false);
        } else if (state.status === "error") {
          setLauncherErr(state.message ?? "Microsoft login failed.");
          setMsLoginSessionId(null);
          setMsCodePromptVisible(false);
          setMsCodePrompt(null);
          setMsCodeCopied(false);
        }
      } catch {
        // ignore transient polling failures
      }
    }, 1200);
    return () => {
      cancelled = true;
      window.clearInterval(t);
    };
  }, [msLoginSessionId]);

  const showSkinStudio = route === "skins";
  const showSkinViewer = showSkinStudio && skinPreviewEnabled;
  const normalizedPreviewTimeOfDay = useMemo(
    () => normalizeTimeOfDay(previewTimeOfDay),
    [previewTimeOfDay]
  );
  const skinViewerShadowStyle = useMemo(() => {
    const t = normalizedPreviewTimeOfDay / 24;
    const azimuth = t * Math.PI * 2;
    const elevation = clampNumber(Math.sin((t - 0.25) * Math.PI * 2), 0.12, 0.98);
    const daylight = clampNumber((elevation - 0.12) / 0.86, 0, 1);
    const length = 4 + (1 - daylight) * 26;
    const offsetX = Math.cos(azimuth) * length;
    const offsetY = Math.sin(azimuth) * length * 0.36;
    const scaleX = 1.02 + (1 - daylight) * 0.42;
    const scaleY = 0.84 + (1 - daylight) * 0.26;
    const blur = 3 + (1 - daylight) * 5;
    const alpha = 0.12 + (1 - daylight) * 0.16;
    return {
      "--shadow-x": `${offsetX.toFixed(1)}px`,
      "--shadow-y": `${offsetY.toFixed(1)}px`,
      "--shadow-scale-x": scaleX.toFixed(3),
      "--shadow-scale-y": scaleY.toFixed(3),
      "--shadow-blur": `${blur.toFixed(1)}px`,
      "--shadow-alpha": alpha.toFixed(3),
    } as CSSProperties;
  }, [normalizedPreviewTimeOfDay]);
  const skinViewerNameTag =
    accountDiagnostics?.minecraft_username ??
    selectedLauncherAccount?.username ??
    "Player";
  const skinViewerHintText = !skinPreviewEnabled
    ? "3D preview is disabled."
    : skinViewerPreparing
      ? "Preparing 3D preview…"
      : skinViewerBusy
        ? "Loading 3D preview…"
        : "Drag to rotate, scroll to zoom, click to punch, use Play emote for gestures";

  const resolveViewerTexture = async (
    src: string | null,
    cacheRef: { current: Map<string, string> }
  ): Promise<string | null> => {
    if (!src) return null;
    if (!/^https?:/i.test(src)) return src;
    const cached = cacheRef.current.get(src);
    if (cached) return cached;
    try {
      const controller = new AbortController();
      const timeout = window.setTimeout(() => controller.abort(), SKIN_IMAGE_FETCH_TIMEOUT_MS);
      let response: Response;
      try {
        response = await fetch(src, { cache: "force-cache", signal: controller.signal });
      } finally {
        window.clearTimeout(timeout);
      }
      if (!response.ok) return src;
      const blob = await response.blob();
      const objectUrl = URL.createObjectURL(blob);
      cacheRef.current.set(src, objectUrl);
      if (cacheRef.current.size > 40) {
        const oldest = cacheRef.current.keys().next().value as string | undefined;
        if (oldest) {
          const stale = cacheRef.current.get(oldest);
          if (stale?.startsWith("blob:")) URL.revokeObjectURL(stale);
          cacheRef.current.delete(oldest);
        }
      }
      return objectUrl;
    } catch {
      return src;
    }
  };

  useEffect(() => {
    if (route !== "account") return;
    if (!selectedLauncherAccountId || accountDiagnosticsBusy || accountDiagnostics || accountDiagnosticsErr) return;

    let cancelled = false;
    let handle: number | null = null;
    const idleApi = window as Window & {
      requestIdleCallback?: (
        callback: () => void,
        options?: { timeout?: number }
      ) => number;
      cancelIdleCallback?: (id: number) => void;
    };
    const runRefresh = () => {
      if (cancelled) return;
      refreshAccountDiagnostics().catch(() => null);
    };

    if (idleApi.requestIdleCallback) {
      handle = idleApi.requestIdleCallback(runRefresh, { timeout: 2400 });
    } else {
      handle = window.setTimeout(runRefresh, 1800);
    }

    return () => {
      cancelled = true;
      if (handle == null) return;
      if (idleApi.requestIdleCallback && idleApi.cancelIdleCallback) {
        idleApi.cancelIdleCallback(handle);
      } else {
        window.clearTimeout(handle);
      }
    };
  }, [route, selectedLauncherAccountId, accountDiagnosticsBusy, accountDiagnostics, accountDiagnosticsErr]);

  useEffect(() => {
    return () => {
      for (const value of skinTextureCacheRef.current.values()) {
        if (value.startsWith("blob:")) URL.revokeObjectURL(value);
      }
      for (const value of capeTextureCacheRef.current.values()) {
        if (value.startsWith("blob:")) URL.revokeObjectURL(value);
      }
      skinTextureCacheRef.current.clear();
      capeTextureCacheRef.current.clear();
    };
  }, []);

  useEffect(() => {
    setLibraryContextMenu(null);
  }, [route]);

  useEffect(() => {
    return () => {
      if (logJumpAnimationFrameRef.current != null) {
        window.cancelAnimationFrame(logJumpAnimationFrameRef.current);
        logJumpAnimationFrameRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    setSelectedCrashSuspect(null);
  }, [
    selectedId,
    logSourceFilter,
    logSeverityFilter,
    logFilterQuery,
    logQuickFilters.errors,
    logQuickFilters.warnings,
    logQuickFilters.suspects,
    logQuickFilters.crashes,
  ]);

  useEffect(() => {
    if (route !== "instance" || instanceTab !== "logs" || logViewMode !== "live") return;
    setLogAutoFollow(true);
    setLogJumpVisible(false);
  }, [route, instanceTab, logViewMode, selectedId, logSourceFilter]);

  useEffect(() => {
    if (route !== "instance" || instanceTab !== "logs" || logViewMode !== "live") return;
    if (!logAutoFollow) return;
    const viewer = logViewerRef.current;
    if (!viewer) return;
    viewer.scrollTop = viewer.scrollHeight;
    setLogJumpVisible(false);
  }, [
    route,
    instanceTab,
    logViewMode,
    selectedId,
    logSourceFilter,
    logAutoFollow,
    rawLogLinesBySource,
    logFilterQuery,
    logSeverityFilter,
    logQuickFilters.errors,
    logQuickFilters.warnings,
    logQuickFilters.suspects,
    logQuickFilters.crashes,
  ]);

  useEffect(() => {
    if (route !== "instance" || instanceTab !== "logs" || !selectedId) return;
    let cancelled = false;
    let timer: number | null = null;
    const cacheKey = `${selectedId}:${logSourceFilter}`;
    const applyPayload = (incoming: ReadInstanceLogsResult, mode: "replace_tail" | "prepend_older") => {
      let merged: ReadInstanceLogsResult = incoming;
      setRawLogLinesBySource((prev) => {
        const existing = prev[cacheKey] ?? null;
        merged = mergeReadInstanceLogPayload({ existing, incoming, mode });
        return {
          ...prev,
          [cacheKey]: merged,
        };
      });
      const nextBeforeLine = normalizeLogLineNo(merged.next_before_line);
      setLogWindowBySource((prev) => ({
        ...prev,
        [cacheKey]: {
          nextBeforeLine,
          loadingOlder: prev[cacheKey]?.loadingOlder ?? false,
          fullyLoaded: nextBeforeLine == null,
        },
      }));
    };
    const pull = async (silent = false) => {
      const reqId = ++logLoadRequestSeqRef.current;
      if (!silent) setLogLoadBusy(true);
      try {
        const payload = await readInstanceLogs({
          instanceId: selectedId,
          source: logSourceFilter,
          maxLines: logMaxLines,
        });
        if (cancelled || reqId !== logLoadRequestSeqRef.current) return;
        applyPayload(payload, "replace_tail");
        setLogLoadErr(null);
      } catch (err: any) {
        if (cancelled || reqId !== logLoadRequestSeqRef.current) return;
        setLogLoadErr(err?.toString?.() ?? String(err));
      } finally {
        if (!silent && !cancelled && reqId === logLoadRequestSeqRef.current) {
          setLogLoadBusy(false);
        }
      }
    };

    void pull(false);
    if (logSourceFilter === "live") {
      timer = window.setInterval(() => {
        void pull(true);
      }, 1000);
    }

    return () => {
      cancelled = true;
      if (timer != null) window.clearInterval(timer);
    };
  }, [route, instanceTab, selectedId, logSourceFilter, logMaxLines]);

  useEffect(() => {
    if (route === "instance" && instanceTab === "logs") return;
    setLogLoadBusy(false);
  }, [route, instanceTab]);

  const onLoadOlderLogLines = async () => {
    if (route !== "instance" || instanceTab !== "logs" || !selectedId || logSourceFilter === "live") return;
    const cacheKey = `${selectedId}:${logSourceFilter}`;
    const currentWindow = logWindowBySource[cacheKey];
    const beforeLine = normalizeLogLineNo(
      currentWindow?.nextBeforeLine ?? rawLogLinesBySource[cacheKey]?.next_before_line
    );
    if (beforeLine == null) return;
    const reqId = ++logLoadRequestSeqRef.current;
    setLogWindowBySource((prev) => ({
      ...prev,
      [cacheKey]: {
        nextBeforeLine: prev[cacheKey]?.nextBeforeLine ?? beforeLine,
        loadingOlder: true,
        fullyLoaded: prev[cacheKey]?.fullyLoaded ?? false,
      },
    }));
    try {
      const incoming = await readInstanceLogs({
        instanceId: selectedId,
        source: logSourceFilter,
        maxLines: logMaxLines,
        beforeLine,
      });
      if (reqId !== logLoadRequestSeqRef.current) return;
      let merged: ReadInstanceLogsResult = incoming;
      setRawLogLinesBySource((prev) => {
        const existing = prev[cacheKey] ?? null;
        merged = mergeReadInstanceLogPayload({
          existing,
          incoming,
          mode: "prepend_older",
        });
        return {
          ...prev,
          [cacheKey]: merged,
        };
      });
      const nextBeforeLine = normalizeLogLineNo(merged.next_before_line);
      setLogWindowBySource((prev) => ({
        ...prev,
        [cacheKey]: {
          nextBeforeLine,
          loadingOlder: false,
          fullyLoaded: nextBeforeLine == null,
        },
      }));
      setLogLoadErr(null);
    } catch (err: any) {
      if (reqId !== logLoadRequestSeqRef.current) return;
      setLogLoadErr(err?.toString?.() ?? String(err));
      setLogWindowBySource((prev) => ({
        ...prev,
        [cacheKey]: {
          nextBeforeLine: prev[cacheKey]?.nextBeforeLine ?? beforeLine,
          loadingOlder: false,
          fullyLoaded: prev[cacheKey]?.fullyLoaded ?? false,
        },
      }));
    }
  };

  function onLogViewerScroll() {
    const viewer = logViewerRef.current;
    if (!viewer) return;
    const distance = viewer.scrollHeight - (viewer.scrollTop + viewer.clientHeight);
    const atBottom = distance <= 10;
    setLogJumpVisible((prev) => (prev === !atBottom ? prev : !atBottom));
    setLogAutoFollow((prev) => {
      if (atBottom) return true;
      if (!prev) return prev;
      return false;
    });
  }

  function onJumpLogsToBottom() {
    const viewer = logViewerRef.current;
    if (!viewer) return;
    const useReducedMotion = prefersReducedMotion();
    if (logJumpAnimationFrameRef.current != null) {
      window.cancelAnimationFrame(logJumpAnimationFrameRef.current);
      logJumpAnimationFrameRef.current = null;
    }
    const target = Math.max(0, viewer.scrollHeight - viewer.clientHeight);
    if (useReducedMotion) {
      viewer.scrollTop = target;
      setLogAutoFollow(true);
      setLogJumpVisible(false);
      return;
    }

    const start = viewer.scrollTop;
    const distance = target - start;
    if (Math.abs(distance) < 1) {
      setLogAutoFollow(true);
      setLogJumpVisible(false);
      return;
    }

    const durationMs = Math.max(420, Math.min(900, 260 + Math.sqrt(Math.abs(distance)) * 22));
    const startAt = performance.now();
    const hardStopAt = startAt + durationMs + 320;
    const easeInOutSine = (t: number) => 0.5 - 0.5 * Math.cos(Math.PI * t);
    setLogAutoFollow(false);

    const step = (now: number) => {
      const elapsed = now - startAt;
      const progress = Math.max(0, Math.min(1, elapsed / durationMs));
      const dynamicTarget = Math.max(0, viewer.scrollHeight - viewer.clientHeight);
      const eased = easeInOutSine(progress);
      const commanded = start + (dynamicTarget - start) * eased;
      // Dampen per-frame movement so the motion stays smooth even as target moves.
      viewer.scrollTop = viewer.scrollTop + (commanded - viewer.scrollTop) * 0.32;
      const remaining = Math.abs(dynamicTarget - viewer.scrollTop);
      if (progress < 1 || (remaining > 0.9 && now < hardStopAt)) {
        logJumpAnimationFrameRef.current = window.requestAnimationFrame(step);
        return;
      }
      logJumpAnimationFrameRef.current = null;
      viewer.scrollTop = Math.max(0, viewer.scrollHeight - viewer.clientHeight);
      setLogAutoFollow(true);
    };
    logJumpAnimationFrameRef.current = window.requestAnimationFrame(step);
    setLogJumpVisible(false);
  }

  useEffect(() => {
    if (!libraryContextMenu) return;
    const onDocMouseDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (libraryContextMenuRef.current?.contains(target)) return;
      setLibraryContextMenu(null);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setLibraryContextMenu(null);
    };
    const closeMenu = () => setLibraryContextMenu(null);

    document.addEventListener("mousedown", onDocMouseDown);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", closeMenu);
    window.addEventListener("scroll", closeMenu, true);
    return () => {
      document.removeEventListener("mousedown", onDocMouseDown);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", closeMenu);
      window.removeEventListener("scroll", closeMenu, true);
    };
  }, [libraryContextMenu]);

  useEffect(() => {
    setAccountAvatarSourceIdx(0);
  }, [accountDiagnostics?.minecraft_uuid, accountDiagnostics?.skin_url]);

  useEffect(() => {
    let cancelled = false;
    setAccountAvatarFromSkin(null);
    const skinUrl = accountDiagnostics?.skin_url ?? null;
    if (!skinUrl) return;
    renderMinecraftHeadFromSkin(skinUrl, 128)
      .then((url) => {
        if (!cancelled && url) setAccountAvatarFromSkin(url);
      })
      .catch(() => null);
    return () => {
      cancelled = true;
    };
  }, [accountDiagnostics?.skin_url]);

  useEffect(() => {
    let cancelled = false;
    const pending = accountSkinOptions.filter((skin) => {
      const existing = accountSkinThumbs[skin.id];
      return !existing || !existing.front || !existing.back || existing.mode !== "3d";
    });
    if (route !== "skins" || pending.length === 0) return;
    const idleApi = window as Window & {
      requestIdleCallback?: (
        callback: () => void,
        options?: { timeout?: number }
      ) => number;
      cancelIdleCallback?: (id: number) => void;
    };
    let handle: number | null = null;
    let cursor = 0;
    const schedule = () => {
      if (cancelled) return;
      if (idleApi.requestIdleCallback) {
        handle = idleApi.requestIdleCallback(run, { timeout: 1200 });
      } else {
        handle = window.setTimeout(run, 60);
      }
    };
    const run = () => {
      (async () => {
        const nextThumbs: Record<string, AccountSkinThumbSet> = {};
        const chunk = pending.slice(cursor, cursor + 2);
        cursor += chunk.length;
        for (const skin of chunk) {
          const baseSrc = toLocalIconSrc(skin.skin_url);
          if (!baseSrc) continue;
          const sources = skinThumbSourceCandidates(baseSrc);
          let front3d: string | null = null;
          let back3d: string | null = null;
          for (const candidate of sources) {
            front3d = await renderMinecraftSkinThumb3d({
              skinUrl: candidate,
              view: "front",
              size: SKIN_THUMB_3D_SIZE,
            }).catch(() => null);
            back3d = await renderMinecraftSkinThumb3d({
              skinUrl: candidate,
              view: "back",
              size: SKIN_THUMB_3D_SIZE,
            }).catch(() => null);
            if (front3d && back3d) break;
          }
          const prev = accountSkinThumbs[skin.id];
          if (front3d && back3d) {
            const changed =
              prev?.mode !== "3d" || prev.front !== front3d || prev.back !== back3d;
            if (changed) {
              nextThumbs[skin.id] = { front: front3d, back: back3d, mode: "3d" };
            }
          } else {
            const fallbackFront =
              front3d ??
              toLocalIconSrc(skin.preview_url) ??
              (await renderMinecraftHeadFromSkin(baseSrc, 192).catch(() => null)) ??
              "";
            const fallbackBack = back3d ?? fallbackFront;
            if (fallbackFront) {
              const changed =
                prev?.mode !== "fallback" ||
                prev.front !== fallbackFront ||
                prev.back !== fallbackBack;
              if (changed) {
                nextThumbs[skin.id] = {
                  front: fallbackFront,
                  back: fallbackBack,
                  mode: "fallback",
                };
              }
            }
          }
          await new Promise((resolve) => window.setTimeout(resolve, 0));
        }
        if (!cancelled && Object.keys(nextThumbs).length > 0) {
          setAccountSkinThumbs((prev) => ({ ...prev, ...nextThumbs }));
        }
        if (!cancelled && cursor < pending.length) {
          schedule();
        }
      })().catch(() => null);
    };
    schedule();
    return () => {
      cancelled = true;
      if (handle == null) return;
      if (idleApi.requestIdleCallback && idleApi.cancelIdleCallback) {
        idleApi.cancelIdleCallback(handle);
      } else {
        window.clearTimeout(handle);
      }
    };
  }, [route, accountSkinOptions, accountSkinThumbs]);

  useEffect(() => {
    if (!showSkinViewer) {
      setSkinViewerPreparing(false);
      setSkinViewerBusy(false);
      skinViewerNameTagTextRef.current = null;
      skinViewerEmoteTriggerRef.current = null;
      return;
    }
    const stage = accountSkinViewerStageRef.current;
    const canvas = accountSkinViewerCanvasRef.current;
    if (!stage || !canvas || accountSkinViewerRef.current) return;

    let disposed = false;
    let idleHandle: number | null = null;
    const idleApi = window as Window & {
      requestIdleCallback?: (
        callback: () => void,
        options?: { timeout?: number }
      ) => number;
      cancelIdleCallback?: (id: number) => void;
    };
    setSkinViewerPreparing(true);

    const startViewer = () => {
      if (disposed || accountSkinViewerRef.current) return;
      const rect = stage.getBoundingClientRect();
      let viewer: SkinViewer;
      try {
        viewer = new SkinViewer({
          canvas,
          width: Math.max(220, Math.round(rect.width)),
          height: Math.max(280, Math.round(rect.height)),
          zoom: 0.56,
          fov: 46,
        });
        setSkinViewerErr(null);
      } catch (error) {
        setSkinViewerErr(
          error instanceof Error
            ? `3D preview unavailable: ${error.message}`
            : "3D preview unavailable on this device."
        );
        setSkinViewerPreparing(false);
        setSkinViewerBusy(false);
        return;
      }
      viewer.background = null;
      const renderer = (viewer as unknown as { renderer?: { setPixelRatio?: (ratio: number) => void } }).renderer;
      renderer?.setPixelRatio?.(Math.min(window.devicePixelRatio || 1, 1.5));
      viewer.globalLight.intensity = 1.15;
      viewer.cameraLight.intensity = 1.05;
      viewer.playerWrapper.position.y = 1.12;
      viewer.controls.enablePan = false;
      viewer.controls.enableZoom = true;
      viewer.controls.enableDamping = true;
      viewer.controls.dampingFactor = 0.09;
      viewer.controls.rotateSpeed = 0.68;
      viewer.controls.zoomSpeed = 0.88;
      viewer.controls.minDistance = 24;
      viewer.controls.maxDistance = 88;
      viewer.controls.minPolarAngle = 0.24;
      viewer.controls.maxPolarAngle = Math.PI - 0.24;
      viewer.controls.target.set(0, 10.8, 0);
      viewer.controls.update();
      viewer.autoRotate = false;

      const recoilState = {
        startedAt: 0,
        durationMs: 320,
        amount: 0.118,
      };
      const attackState = {
        startedAt: 0,
        durationMs: 300,
        amount: 1,
        arm: "right" as "right" | "left",
      };
      const idleEmoteNames = [
        "wave",
        "nod",
        "celebrate",
        "lookAround",
        "salute",
        "shrug",
        "stretch",
        "bow",
      ] as const;
      const playEmoteNames = [
        "wave",
        "salute",
        "nod",
        "lookAround",
        "stretch",
        "bow",
      ] as const;
      const emoteState = {
        name: null as (typeof idleEmoteNames)[number] | null,
        startedAt: 0,
        durationMs: 0,
        nextAt: performance.now() + 7000 + Math.random() * 5000,
        lastInteractionAt: performance.now(),
      };
      let playEmoteIdx = 0;
      const queueNextEmote = (now: number, minDelayMs = 5600) => {
        emoteState.nextAt = now + minDelayMs + Math.random() * 6800;
      };
      const markInteraction = (minDelayMs = 4200) => {
        const now = performance.now();
        emoteState.lastInteractionAt = now;
        if (!emoteState.name) queueNextEmote(now, minDelayMs);
      };
      const startEmote = (next: (typeof idleEmoteNames)[number]) => {
        emoteState.name = next;
        emoteState.startedAt = performance.now();
        emoteState.durationMs =
          next === "wave"
            ? 2200
            : next === "celebrate"
              ? 1800
              : next === "lookAround"
                ? 2400
                : next === "salute"
                  ? 1500
                  : next === "shrug"
                    ? 1700
                    : next === "stretch"
                      ? 1900
                      : 1400;
      };
      const startRandomEmote = () => {
        const next = idleEmoteNames[Math.floor(Math.random() * idleEmoteNames.length)] ?? "wave";
        startEmote(next);
      };
      const playNextEmote = () => {
        const next = playEmoteNames[playEmoteIdx % playEmoteNames.length] ?? "wave";
        playEmoteIdx = (playEmoteIdx + 1) % playEmoteNames.length;
        startEmote(next);
      };
      skinViewerEmoteTriggerRef.current = (mode = "play") => {
        markInteraction(2200);
        if (mode === "celebrate") {
          startEmote("celebrate");
        } else {
          playNextEmote();
        }
      };
      const tapState = {
        pointerId: -1,
        x: 0,
        y: 0,
        at: 0,
      };
      const triggerAttack = () => {
        markInteraction(5200);
        recoilState.startedAt = performance.now();
        recoilState.amount = 0.118;
        attackState.startedAt = performance.now();
        attackState.amount = 1;
        attackState.arm = "right";
        emoteState.name = null;
      };
      const onPointerDown = (event: PointerEvent) => {
        markInteraction(4200);
        tapState.pointerId = event.pointerId;
        tapState.x = event.clientX;
        tapState.y = event.clientY;
        tapState.at = performance.now();
      };
      const onPointerUp = (event: PointerEvent) => {
        markInteraction(4200);
        if (tapState.pointerId !== event.pointerId) return;
        const dx = event.clientX - tapState.x;
        const dy = event.clientY - tapState.y;
        const distance = Math.hypot(dx, dy);
        const elapsed = performance.now() - tapState.at;
        if (distance <= 9 && elapsed <= 300) {
          triggerAttack();
        }
        tapState.pointerId = -1;
      };
      const onPointerCancel = () => {
        markInteraction(4200);
        tapState.pointerId = -1;
      };
      const onWheel = () => {
        markInteraction(3600);
      };
      const controls = viewer.controls as unknown as {
        addEventListener?: (event: string, cb: () => void) => void;
        removeEventListener?: (event: string, cb: () => void) => void;
      };
      const onControlStart = () => markInteraction(4200);
      const onControlChange = () => markInteraction(2600);
      controls.addEventListener?.("start", onControlStart);
      controls.addEventListener?.("change", onControlChange);
      canvas.addEventListener("pointerdown", onPointerDown);
      canvas.addEventListener("pointerup", onPointerUp);
      canvas.addEventListener("pointercancel", onPointerCancel);
      canvas.addEventListener("wheel", onWheel, { passive: true });
      skinViewerInputCleanupRef.current = () => {
        controls.removeEventListener?.("start", onControlStart);
        controls.removeEventListener?.("change", onControlChange);
        canvas.removeEventListener("pointerdown", onPointerDown);
        canvas.removeEventListener("pointerup", onPointerUp);
        canvas.removeEventListener("pointercancel", onPointerCancel);
        canvas.removeEventListener("wheel", onWheel);
      };

      const idle = new IdleAnimation();
      idle.speed = 0.78;
      idle.addAnimation((player, progress) => {
        const now = performance.now();
        const breathing = Math.sin(progress * 2.4) * 0.055;
        const sway = Math.sin(progress * 0.92) * 0.085;
        const elapsed = now - recoilState.startedAt;
        const t = recoilState.startedAt > 0 ? Math.min(1, Math.max(0, elapsed / recoilState.durationMs)) : 1;
        let recoil = 0;
        if (t < 1) {
          if (t <= 0.19) {
            recoil = recoilState.amount * (t / 0.19);
          } else {
            const release = (t - 0.19) / 0.81;
            recoil = recoilState.amount * Math.pow(1 - release, 2.25);
          }
        }
        const attackElapsed = now - attackState.startedAt;
        const attackT =
          attackState.startedAt > 0
            ? Math.min(1, Math.max(0, attackElapsed / attackState.durationMs))
            : 1;

        if (
          !emoteState.name &&
          attackT >= 1 &&
          now >= emoteState.nextAt &&
          now - emoteState.lastInteractionAt > 5000
        ) {
          startRandomEmote();
        }

        let emoteProgress = -1;
        if (emoteState.name) {
          emoteProgress = Math.min(1, Math.max(0, (now - emoteState.startedAt) / emoteState.durationMs));
          if (emoteProgress >= 1) {
            emoteState.name = null;
            emoteProgress = -1;
            queueNextEmote(now, 5200);
          }
        }
        const emotePeak = emoteProgress >= 0 ? Math.sin(Math.PI * emoteProgress) : 0;
        const emotePulse = emoteProgress >= 0 ? Math.sin(emoteProgress * Math.PI * 2.6) : 0;
        let emoteRightX = 0;
        let emoteRightY = 0;
        let emoteRightZ = 0;
        let emoteLeftX = 0;
        let emoteLeftY = 0;
        let emoteLeftZ = 0;
        let emoteHeadX = 0;
        let emoteHeadY = 0;
        let emoteHeadLift = 0;
        let emoteHeadForward = 0;
        let emoteBodyYaw = 0;
        let emoteBodyPitch = 0;
        let emoteBodyRoll = 0;
        let emoteLift = 0;
        let emoteLegRightX = 0;
        let emoteLegLeftX = 0;
        let emoteRootYaw = 0;
        if (emoteState.name === "wave") {
          emoteRightX = -1.08 * emotePeak;
          emoteRightZ = -0.18 * emotePeak + emotePulse * 0.42 * emotePeak;
          emoteBodyYaw = -0.09 * emotePeak;
        } else if (emoteState.name === "nod") {
          emoteHeadX = Math.sin(emoteProgress * Math.PI * 3.6) * 0.3 * (0.4 + emotePeak * 0.6);
          emoteBodyPitch = -0.05 * emotePeak;
        } else if (emoteState.name === "celebrate") {
          emoteRightX = -1.68 * emotePeak;
          emoteLeftX = -1.56 * emotePeak;
          emoteRightZ = -0.16 * emotePeak;
          emoteLeftZ = 0.16 * emotePeak;
          emoteLift = Math.sin(Math.PI * emoteProgress) * 0.08;
        } else if (emoteState.name === "lookAround") {
          emoteHeadY = Math.sin(emoteProgress * Math.PI * 1.8) * 0.55 * emotePeak;
          emoteHeadX = -0.07 * emotePeak;
          emoteBodyYaw = Math.sin(emoteProgress * Math.PI * 1.8) * 0.13 * emotePeak;
        } else if (emoteState.name === "salute") {
          emoteRightX = -1.3 * emotePeak;
          emoteRightY = -0.36 * emotePeak;
          emoteRightZ = -0.08 * emotePeak;
          emoteHeadX = -0.1 * emotePeak;
          emoteBodyYaw = -0.08 * emotePeak;
        } else if (emoteState.name === "shrug") {
          emoteRightX = -0.48 * emotePeak;
          emoteLeftX = -0.48 * emotePeak;
          emoteRightZ = -0.34 * emotePeak;
          emoteLeftZ = 0.34 * emotePeak;
          emoteHeadY = Math.sin(emoteProgress * Math.PI * 2.1) * 0.2 * emotePeak;
          emoteBodyRoll = Math.sin(emoteProgress * Math.PI * 2.1) * 0.06 * emotePeak;
        } else if (emoteState.name === "stretch") {
          emoteRightX = -1.76 * emotePeak;
          emoteLeftX = -1.76 * emotePeak;
          emoteRightZ = -0.12 * emotePeak;
          emoteLeftZ = 0.12 * emotePeak;
          emoteBodyPitch = -0.13 * emotePeak;
          emoteLift = 0.04 * emotePeak;
        } else if (emoteState.name === "bow") {
          emoteBodyPitch = 0.22 * emotePeak;
          emoteHeadX = 0.28 * emotePeak;
          emoteRightX = -0.22 * emotePeak;
          emoteLeftX = -0.22 * emotePeak;
        }

        let punchWindup = 0;
        let punch = 0;
        if (attackT < 1) {
          if (attackT <= 0.2) {
            punchWindup = attackState.amount * (attackT / 0.2);
          } else if (attackT <= 0.42) {
            punch = attackState.amount * ((attackT - 0.2) / 0.22);
          } else {
            const recover = (attackT - 0.42) / 0.58;
            punch = attackState.amount * Math.pow(1 - recover, 1.65);
          }
        }
        const punchRight = attackState.arm === "right";
        const skin = (player as any).skin;
        const rightArm = skin?.rightArm;
        const leftArm = skin?.leftArm;
        const rightLeg = skin?.rightLeg;
        const leftLeg = skin?.leftLeg;
        const idleArmSwing = Math.sin(progress * 1.9) * 0.045;
        if (rightArm?.rotation) {
          rightArm.rotation.x = idleArmSwing + emoteRightX;
          rightArm.rotation.y = emoteRightY;
          rightArm.rotation.z = emoteRightZ;
        }
        if (leftArm?.rotation) {
          leftArm.rotation.x = -idleArmSwing * 0.68 + emoteLeftX;
          leftArm.rotation.y = emoteLeftY;
          leftArm.rotation.z = emoteLeftZ;
        }
        if (rightLeg?.rotation) {
          rightLeg.rotation.x = emoteLegRightX;
          rightLeg.rotation.y = 0;
          rightLeg.rotation.z = 0;
        }
        if (leftLeg?.rotation) {
          leftLeg.rotation.x = emoteLegLeftX;
          leftLeg.rotation.y = 0;
          leftLeg.rotation.z = 0;
        }
        if (punch > 0) {
          const attackingArm = punchRight ? rightArm : leftArm;
          const supportArm = punchRight ? leftArm : rightArm;
          if (attackingArm?.rotation) {
            attackingArm.rotation.x += 0.26 * punchWindup - 1.92 * punch;
            attackingArm.rotation.y = (punchRight ? -0.12 : 0.12) * punch;
            attackingArm.rotation.z += (punchRight ? -0.28 : 0.28) * punch;
          }
          if (supportArm?.rotation) {
            supportArm.rotation.x += 0.14 * punch + 0.08 * punchWindup;
            supportArm.rotation.z += (punchRight ? 0.08 : -0.08) * punch;
          }
        }
        const combatYaw = (punchRight ? -0.24 : 0.24) * punch + (punchRight ? 0.08 : -0.08) * punchWindup;
        if (skin?.body?.rotation) {
          skin.body.rotation.y = emoteBodyYaw + combatYaw;
          skin.body.rotation.x = -recoil * 0.29 + emoteBodyPitch;
          skin.body.rotation.z = Math.sin(progress * 1.2) * 0.015 + emoteBodyRoll;
        }
        if (skin?.head?.rotation) {
          skin.head.rotation.x = Math.sin(progress * 2.2) * 0.05 - recoil * 0.24 + emoteHeadX - punch * 0.05;
          skin.head.rotation.y = emoteHeadY + (punchRight ? 0.04 : -0.04) * punch;
        }
        if (skin?.head?.position) {
          skin.head.position.y = emoteHeadLift;
          skin.head.position.z = emoteHeadForward;
        }
        player.position.y = 0.06 + breathing + emoteLift;
        player.position.z = -recoil - punch * 0.055;
        player.rotation.x = -recoil * 0.34 - punch * 0.06;
        player.rotation.y = sway + (punchRight ? -0.18 : 0.18) * punch + emoteBodyYaw * 0.26 + emoteRootYaw;
      });
      viewer.animation = idle;

      accountSkinViewerRef.current = viewer;
      skinViewerNameTagTextRef.current = null;
      lastLoadedSkinSrcRef.current = null;
      lastLoadedCapeSrcRef.current = null;
      const resizeObserver = new ResizeObserver(() => {
        const { width, height } = stage.getBoundingClientRect();
        viewer.setSize(Math.max(220, Math.round(width)), Math.max(260, Math.round(height)));
      });
      resizeObserver.observe(stage);
      accountSkinViewerResizeRef.current = resizeObserver;
      setSkinViewerPreparing(false);
      setSkinViewerEpoch((v) => v + 1);

      if (disposed) {
        skinViewerInputCleanupRef.current?.();
        skinViewerInputCleanupRef.current = null;
        resizeObserver.disconnect();
        viewer.dispose();
        skinViewerEmoteTriggerRef.current = null;
      }
    };

    if (idleApi.requestIdleCallback) {
      idleHandle = idleApi.requestIdleCallback(startViewer, { timeout: 260 });
    } else {
      idleHandle = window.setTimeout(startViewer, 110);
    }

    return () => {
      disposed = true;
      setSkinViewerPreparing(false);
      setSkinViewerBusy(false);
      if (idleHandle != null) {
        if (idleApi.cancelIdleCallback && idleApi.requestIdleCallback) {
          idleApi.cancelIdleCallback(idleHandle);
        } else {
          window.clearTimeout(idleHandle);
        }
      }
      skinViewerInputCleanupRef.current?.();
      skinViewerInputCleanupRef.current = null;
      accountSkinViewerResizeRef.current?.disconnect();
      accountSkinViewerResizeRef.current = null;
      accountSkinViewerRef.current?.dispose();
      accountSkinViewerRef.current = null;
      skinViewerNameTagTextRef.current = null;
      skinViewerEmoteTriggerRef.current = null;
    };
  }, [showSkinViewer, route]);

  useEffect(() => {
    if (!showSkinViewer) return;
    const viewer = accountSkinViewerRef.current;
    if (!viewer) return;
    const text = String(skinViewerNameTag ?? "").trim() || "Player";
    if (skinViewerNameTagTextRef.current === text && viewer.nameTag) return;
    viewer.nameTag = new NameTagObject(text, {
      font: "64px Minecraft, system-ui, sans-serif",
      margin: [8, 16, 8, 16],
      textStyle: "rgba(246, 248, 255, 0.98)",
      backgroundStyle: "rgba(18, 24, 36, 0.55)",
      height: 2.8,
      repaintAfterLoaded: true,
    });
    if (viewer.nameTag) {
      viewer.nameTag.position.y = 18.6;
    }
    skinViewerNameTagTextRef.current = text;
  }, [showSkinViewer, skinViewerEpoch, skinViewerNameTag]);

  useEffect(() => {
    if (!showSkinViewer) return;
    const viewer = accountSkinViewerRef.current;
    if (!viewer) return;
    const t = normalizedPreviewTimeOfDay / 24;
    const azimuth = t * Math.PI * 2;
    const elevation = clampNumber(Math.sin((t - 0.25) * Math.PI * 2), 0.12, 0.98);
    const daylight = clampNumber((elevation - 0.12) / 0.86, 0, 1);
    // Keep the model readable at all times, then layer time-of-day variation on top.
    const readableBias = 1 - daylight;
    viewer.globalLight.intensity = 1.2 + daylight * 0.52 + readableBias * 0.22;
    viewer.cameraLight.intensity = 1.08 + daylight * 0.48 + readableBias * 0.3;
    const cameraDistance = viewer.camera.position.length();
    const frontBias = cameraDistance * (0.74 + 0.08 * daylight);
    const sideBias = cameraDistance * (0.12 + 0.16 * daylight);
    viewer.cameraLight.position.set(
      Math.cos(azimuth) * Math.cos(elevation) * sideBias,
      cameraDistance * 0.5 + Math.sin(elevation) * cameraDistance * 0.4,
      frontBias + Math.sin(azimuth) * Math.cos(elevation) * sideBias * 0.45
    );
  }, [showSkinViewer, skinViewerEpoch, normalizedPreviewTimeOfDay]);

  useEffect(() => {
    if (!showSkinViewer) return;
    const viewer = accountSkinViewerRef.current;
    if (!viewer) return;
    const loadWithTimeout = async (task: Promise<void> | void) => {
      await Promise.race([
        Promise.resolve(task),
        new Promise<void>((resolve) => window.setTimeout(resolve, SKIN_VIEWER_LOAD_TIMEOUT_MS)),
      ]);
    };
    let cancelled = false;
    const skinSrc = toLocalIconSrc(selectedAccountSkin?.skin_url) ?? null;
    const capeSrc = toLocalIconSrc(selectedAccountCape?.url) ?? null;
    const skinChanged = skinSrc !== lastLoadedSkinSrcRef.current;
    const capeChanged = capeSrc !== lastLoadedCapeSrcRef.current;
    if (!skinChanged && !capeChanged) {
      setSkinViewerBusy(false);
      return;
    }
    setSkinViewerBusy(true);
    (async () => {
      if (skinChanged) {
        const skinLoadStarted = performance.now();
        try {
          if (skinSrc) {
            const resolvedSkinSrc = await resolveViewerTexture(skinSrc, skinTextureCacheRef);
            if (cancelled) return;
            await loadWithTimeout(viewer.loadSkin(resolvedSkinSrc ?? skinSrc, { model: "auto-detect" }));
          } else {
            viewer.loadSkin(null);
          }
          lastLoadedSkinSrcRef.current = skinSrc;
        } catch {
          // keep the current texture if remote loading fails
        } finally {
          const skinLoadMs = Math.round(performance.now() - skinLoadStarted);
          if (skinLoadMs > 900) {
            console.info(`[perf] skin texture load took ${skinLoadMs}ms (${skinSrc ?? "none"})`);
          }
        }
      }
      if (cancelled) return;
      if (capeChanged) {
        const capeLoadStarted = performance.now();
        try {
          if (capeSrc) {
            const resolvedCapeSrc = await resolveViewerTexture(capeSrc, capeTextureCacheRef);
            if (cancelled) return;
            await loadWithTimeout(viewer.loadCape(resolvedCapeSrc ?? capeSrc, { backEquipment: "cape" }));
          } else {
            viewer.loadCape(null);
          }
          lastLoadedCapeSrcRef.current = capeSrc;
        } catch {
          viewer.loadCape(null);
          lastLoadedCapeSrcRef.current = null;
        } finally {
          const capeLoadMs = Math.round(performance.now() - capeLoadStarted);
          if (capeLoadMs > 900) {
            console.info(`[perf] cape texture load took ${capeLoadMs}ms (${capeSrc ?? "none"})`);
          }
        }
      }
      if (!cancelled) setSkinViewerBusy(false);
    })().catch(() => {
      if (!cancelled) setSkinViewerBusy(false);
    });
    return () => {
      cancelled = true;
    };
  }, [
    showSkinViewer,
    skinViewerEpoch,
    selectedAccountSkin?.skin_url,
    selectedAccountCape?.url,
  ]);

  const commandPaletteItems = useMemo<CommandPaletteItem[]>(() => {
    const paletteInstanceId = selectedId ?? instances[0]?.id ?? null;
    const paletteInstance = paletteInstanceId
      ? instances.find((inst) => inst.id === paletteInstanceId) ?? null
      : null;
    const items: CommandPaletteItem[] = [
      { id: "route:home", label: "Go to Home", group: "Navigation", keywords: ["route"], run: () => setRoute("home") },
      { id: "route:discover", label: "Go to Discover", group: "Navigation", keywords: ["mods", "search"], run: () => setRoute("discover") },
      { id: "route:modpacks", label: "Go to Creator Studio", group: "Navigation", keywords: ["modpack", "creator"], run: () => setRoute("modpacks") },
      { id: "route:library", label: "Go to Library", group: "Navigation", keywords: ["instances"], run: () => setRoute("library") },
      { id: "route:updates", label: "Go to Updates", group: "Navigation", keywords: ["scheduled"], run: () => setRoute("updates") },
      { id: "route:skins", label: "Go to Skins", group: "Navigation", run: () => setRoute("skins") },
      { id: "route:account", label: "Go to Account", group: "Navigation", run: () => setRoute("account") },
      { id: "route:settings", label: "Go to Settings", group: "Navigation", run: () => setRoute("settings") },
      {
        id: "action:create-instance",
        label: "Create instance",
        group: "Actions",
        keywords: ["new", "instance"],
        run: () => setShowCreate(true),
      },
      {
        id: "action:check-content-updates",
        label: "Run content update checks now",
        group: "Actions",
        keywords: ["updates", "scheduled"],
        run: () => void runScheduledUpdateChecks("manual"),
      },
      {
        id: "action:open-instance-settings",
        label: "Open selected instance settings",
        group: "Actions",
        keywords: ["instance", "settings", "modal"],
        run: () => {
          if (!paletteInstanceId) {
            setInstallNotice("Create an instance first.");
            return;
          }
          setSelectedId(paletteInstanceId);
          setRoute("instance");
          setInstanceSettingsOpen(true);
        },
      },
      {
        id: "action:open-instance",
        label: "Open selected instance page",
        group: "Actions",
        keywords: ["instance", "content"],
        run: () => {
          if (!paletteInstanceId) {
            setInstallNotice("Create an instance first.");
            return;
          }
          setSelectedId(paletteInstanceId);
          setRoute("instance");
          setInstanceTab("content");
        },
      },
      {
        id: "action:open-instance-folder",
        label: "Open selected instance folder",
        group: "Actions",
        keywords: ["files", "folder", "path"],
        run: () => {
          if (!paletteInstance) {
            setInstallNotice("Create an instance first.");
            return;
          }
          void onOpenInstancePath(paletteInstance, "instance");
        },
      },
      {
        id: "action:open-mods-folder",
        label: "Open selected mods folder",
        group: "Actions",
        keywords: ["mods", "folder", "jar"],
        run: () => {
          if (!paletteInstance) {
            setInstallNotice("Create an instance first.");
            return;
          }
          void onOpenInstancePath(paletteInstance, "mods");
        },
      },
      {
        id: "action:open-launch-logs",
        label: "Open selected launch logs",
        group: "Actions",
        keywords: ["logs", "crash", "debug"],
        run: () => {
          if (!paletteInstanceId) {
            setInstallNotice("Create an instance first.");
            return;
          }
          setSelectedId(paletteInstanceId);
          setRoute("instance");
          setInstanceTab("logs");
          setLogSourceFilter("latest_launch");
        },
      },
      {
        id: "action:customize-home",
        label: homeCustomizeOpen ? "Finish home customization" : "Customize home widgets",
        group: "Actions",
        keywords: ["home", "widgets", "layout"],
        run: () => {
          setRoute("home");
          setHomeCustomizeOpen((prev) => !prev);
        },
      },
      {
        id: "action:check-app-updates",
        label: "Check app updates now",
        group: "Actions",
        keywords: ["launcher", "update"],
        run: () => void onCheckAppUpdate({ silent: false }),
      },
    ];

    const settingShortcuts: Array<{
      id: string;
      label: string;
      detail?: string;
      advanced?: boolean;
      target: "global" | "instance";
      keywords?: string[];
    }> = [
      { id: "global:appearance", label: "Settings: Appearance", target: "global", keywords: ["theme", "accent"] },
      { id: "global:launch-method", label: "Settings: Default launch method", target: "global", keywords: ["prism", "native"] },
      { id: "global:java-path", label: "Settings: Java executable", target: "global", advanced: true, keywords: ["java"] },
      { id: "global:permissions", label: "Settings: Launch permissions", target: "global", advanced: true, keywords: ["microphone", "voice"] },
      { id: "global:github-api", label: "Settings: GitHub API auth", target: "global", advanced: true, keywords: ["github", "token", "rate limit"] },
      { id: "global:oauth-client", label: "Settings: OAuth client override", target: "global", advanced: true, keywords: ["oauth", "client id"] },
      { id: "global:account", label: "Settings: Microsoft account", target: "global", keywords: ["login"] },
      { id: "global:app-updates", label: "Settings: App updates", target: "global", keywords: ["update"] },
      { id: "global:content-visuals", label: "Settings: Content and visuals", target: "global", keywords: ["visuals", "content"] },
      { id: "instance:general", label: "Instance settings: General", target: "instance", keywords: ["name", "notes"] },
      { id: "instance:installation", label: "Instance settings: Installation", target: "instance", keywords: ["loader", "minecraft"] },
      { id: "instance:java-runtime", label: "Instance settings: Java runtime", target: "instance", advanced: true, keywords: ["java"] },
      { id: "instance:java-memory", label: "Instance settings: Memory", target: "instance", keywords: ["ram"] },
      { id: "instance:jvm-args", label: "Instance settings: JVM arguments", target: "instance", advanced: true, keywords: ["jvm"] },
      { id: "instance:graphics", label: "Instance settings: Window and graphics", target: "instance", keywords: ["window", "graphics"] },
      { id: "instance:hooks", label: "Instance settings: Launch hooks", target: "instance", advanced: true, keywords: ["hooks"] },
    ];

    for (const shortcut of settingShortcuts) {
      items.push({
        id: `setting:${shortcut.id}`,
        label: shortcut.label,
        group: "Settings",
        detail: shortcut.detail,
        keywords: shortcut.keywords,
        run: () => openSettingAnchor(shortcut.id, { advanced: shortcut.advanced, target: shortcut.target }),
      });
    }

    return items;
  }, [instances, selectedId, settingsMode, instanceSettingsMode, homeCustomizeOpen]);

  const installProgressPercentValue = useMemo(() => {
    if (!installProgress) return null;
    const direct = Number(installProgress.percent);
    if (Number.isFinite(direct)) {
      return Math.max(0, Math.min(100, direct));
    }
    const total = Number(installProgress.total ?? 0);
    const downloaded = Number(installProgress.downloaded ?? 0);
    if (Number.isFinite(total) && total > 0 && Number.isFinite(downloaded)) {
      return Math.max(0, Math.min(100, (downloaded / total) * 100));
    }
    return null;
  }, [installProgress]);

  const installProgressTransferText = useMemo(() => {
    if (!installProgress) return "";
    const downloaded = Math.max(0, Number(installProgress.downloaded ?? 0));
    const total = Number(installProgress.total ?? 0);
    const stage = String(installProgress.stage ?? "").toLowerCase();
    if (stage !== "downloading") return "";
    if (Number.isFinite(total) && total > 0) {
      if (total <= 200 && downloaded <= 200) {
        const roundedDownloaded = Math.max(0, Math.floor(downloaded));
        const roundedTotal = Math.max(1, Math.floor(total));
        return `${roundedDownloaded}/${roundedTotal} file${roundedTotal === 1 ? "" : "s"}`;
      }
      return `${formatBytes(downloaded)} / ${formatBytes(total)}`;
    }
    if (downloaded > 0) {
      return `${formatBytes(downloaded)} downloaded`;
    }
    return "";
  }, [installProgress]);

  const installProgressStageLabel = useMemo(() => {
    const stage = String(installProgress?.stage ?? "").toLowerCase();
    switch (stage) {
      case "snapshotting":
      case "resolving":
      case "installing":
        return "Preparing";
      case "downloading":
        return "Downloading";
      case "finalizing":
        return "Finishing";
      case "completed":
        return "Done";
      case "error":
        return "Error";
      default:
        return stage ? stage[0].toUpperCase() + stage.slice(1) : "Working";
    }
  }, [installProgress?.stage]);

  const installProgressTitleText = useMemo(() => {
    if (!installProgress) return "Working…";
    const stage = String(installProgress.stage ?? "").toLowerCase();
    if (stage === "error") return installProgress.message ?? "Install failed";
    if (stage === "completed") return installProgress.message ?? "Install complete";
    if (stage === "downloading") return "Downloading files…";
    if (stage === "finalizing") return "Finishing install…";
    if (stage === "installing" || stage === "resolving" || stage === "snapshotting") {
      return "Preparing install…";
    }
    return installProgress.message ?? "Working…";
  }, [installProgress]);

  const installProgressIndeterminate = useMemo(() => {
    if (!installProgress) return false;
    const stage = String(installProgress.stage ?? "").toLowerCase();
    if (stage === "completed" || stage === "error") return false;
    if (installProgress.percent != null && Number.isFinite(Number(installProgress.percent))) {
      return false;
    }
    if (stage === "downloading") {
      return !(Number(installProgress.total ?? 0) > 0);
    }
    return true;
  }, [installProgress]);

  const installProgressPercentLabel = useMemo(() => {
    if (!installProgress) return "";
    if (installProgressIndeterminate) return installProgressStageLabel;
    const value = installProgressPercentValue;
    if (value == null || !Number.isFinite(value)) return "";
    const stage = String(installProgress.stage ?? "").toLowerCase();
    if (stage === "downloading" && value < 100) {
      return `${Math.max(0, Math.min(100, value)).toFixed(1)}%`;
    }
    return formatPercent(value);
  }, [installProgress, installProgressIndeterminate, installProgressPercentValue, installProgressStageLabel]);

  const installProgressSpeedText = useMemo(() => {
    if (!installProgress) return "";
    if (String(installProgress.stage ?? "").toLowerCase() !== "downloading") return "";
    if (installProgressBytesPerSecond == null || !Number.isFinite(installProgressBytesPerSecond)) {
      return "";
    }
    return `${formatBytes(installProgressBytesPerSecond)}/s`;
  }, [installProgress, installProgressBytesPerSecond]);

  const installProgressShowTransferMetrics =
    String(installProgress?.stage ?? "").toLowerCase() === "downloading";

  function renderContent() {
    if (route === "home") {
      type HomeAttentionItem = {
        id: string;
        tone: "danger" | "warn";
        title: string;
        meta: string;
        action_label: string;
        on_action: () => void;
        action_disabled?: boolean;
      };
      const loaderLabelFor = (inst: Instance) =>
        inst.loader === "neoforge"
          ? "NeoForge"
          : inst.loader === "fabric"
            ? "Fabric"
            : inst.loader === "forge"
              ? "Forge"
              : inst.loader === "quilt"
                ? "Quilt"
                : "Vanilla";
      const recentInstances = [...instances]
        .sort((a, b) => {
          const bTs = parseDateLike(b.created_at)?.getTime() ?? 0;
          const aTs = parseDateLike(a.created_at)?.getTime() ?? 0;
          return bTs - aTs;
        })
        .slice(0, 5);
      const focusInstance = selected ?? recentInstances[0] ?? null;
      const runningIds = new Set(runningInstances.map((run) => run.instance_id));
      const instanceNameById = new Map(instances.map((inst) => [inst.id, inst.name]));
      const instancesById = new Map(instances.map((inst) => [inst.id, inst]));
      const recentActivity = Object.entries(instanceActivityById)
        .flatMap(([instanceId, entries]) =>
          entries.map((entry) => ({
            ...entry,
            instance_id: instanceId,
            instance_name: instanceNameById.get(instanceId) ?? "Unknown instance",
          }))
        )
        .sort((a, b) => b.at - a.at)
        .slice(0, 6);
      const focusLaunchStage = focusInstance ? launchStageByInstance[focusInstance.id] ?? null : null;
      const focusLaunchStageLabel = focusLaunchStage?.label?.trim() || launchStageBadgeLabel(
        focusLaunchStage?.status,
        focusLaunchStage?.message
      );
      const focusHealthScore = focusInstance ? instanceHealthById[focusInstance.id] ?? null : null;
      const focusFriendStatus = focusInstance ? friendLinkStatusByInstance[focusInstance.id] ?? null : null;
      const slowPerfActions = [...perfActions]
        .slice(0, 48)
        .sort((a, b) => b.duration_ms - a.duration_ms)
        .slice(0, 5);
      const topSlowPerfActions = slowPerfActions.slice(0, 2);
      const recentPerfWindow = perfActions.slice(0, 10);
      const recentPerfA = recentPerfWindow.slice(0, 5).map((entry) => Math.max(0, Number(entry.duration_ms) || 0));
      const recentPerfB = recentPerfWindow.slice(5, 10).map((entry) => Math.max(0, Number(entry.duration_ms) || 0));
      const avgPerfA = recentPerfA.length
        ? recentPerfA.reduce((sum, value) => sum + value, 0) / recentPerfA.length
        : null;
      const avgPerfB = recentPerfB.length
        ? recentPerfB.reduce((sum, value) => sum + value, 0) / recentPerfB.length
        : null;
      const perfTrendLabel =
        avgPerfA == null || avgPerfB == null
          ? "Trend: collect more samples"
          : avgPerfA <= avgPerfB * 0.9
            ? "Trend: getting faster"
            : avgPerfA >= avgPerfB * 1.1
              ? "Trend: getting slower"
              : "Trend: stable";
      const homeAttentionItems: HomeAttentionItem[] = [];
      const launchFailures = Object.entries(launchFailureByInstance)
        .sort((a, b) => (b[1]?.updated_at ?? 0) - (a[1]?.updated_at ?? 0))
        .slice(0, 3);
      for (const [instanceId, failure] of launchFailures) {
        const inst = instancesById.get(instanceId);
        if (!inst) continue;
        homeAttentionItems.push({
          id: `launch:${instanceId}`,
          tone: "danger",
          title: `${inst.name}: launch failed`,
          meta: failure?.message || "Open instance logs for details.",
          action_label: "Open instance",
          on_action: () => openInstance(instanceId),
        });
      }
      if (!selectedLauncherAccount) {
        homeAttentionItems.push({
          id: "account:missing",
          tone: "warn",
          title: "Microsoft account not connected",
          meta: "Connect an account to launch with native runtime.",
          action_label: msLoginSessionId ? "Waiting for login…" : "Connect account",
          on_action: onBeginMicrosoftLogin,
          action_disabled: launcherBusy || Boolean(msLoginSessionId),
        });
      }
      if (appUpdaterLastError) {
        homeAttentionItems.push({
          id: "updater:error",
          tone: "danger",
          title: "Launcher update check failed",
          meta: appUpdaterLastError,
          action_label: "Check again",
          on_action: () => {
            void onCheckAppUpdate({ silent: false });
          },
        });
      } else if (appUpdaterState?.available) {
        homeAttentionItems.push({
          id: "updater:available",
          tone: "warn",
          title: `Launcher update available${appUpdaterState.latest_version ? `: v${appUpdaterState.latest_version}` : ""}`,
          meta: "Install now and restart the launcher.",
          action_label: appUpdaterInstallBusy ? "Installing…" : "Install update",
          on_action: () => {
            void onInstallAppUpdate();
          },
          action_disabled: appUpdaterInstallBusy || appUpdaterBusy,
        });
      }
      const updateDebt = scheduledUpdateEntries
        .filter((row) => (row.update_count ?? 0) > 0)
        .slice(0, 3);
      for (const row of updateDebt) {
        homeAttentionItems.push({
          id: `updates:${row.instance_id}`,
          tone: "warn",
          title: `${row.instance_name}: ${row.update_count} update${row.update_count === 1 ? "" : "s"} pending`,
          meta: "Review content updates before your next launch.",
          action_label: "Open updates",
          on_action: () => setRoute("updates"),
        });
      }
      if (focusInstance && focusFriendStatus?.linked && (focusFriendStatus.pending_conflicts_count ?? 0) > 0) {
        homeAttentionItems.push({
          id: `friend:${focusInstance.id}`,
          tone: "danger",
          title: `${focusInstance.name}: Friend Link conflicts`,
          meta: `${focusFriendStatus.pending_conflicts_count} conflict${focusFriendStatus.pending_conflicts_count === 1 ? "" : "s"} require resolution before launch.`,
          action_label: "Resolve now",
          on_action: () => openInstance(focusInstance.id),
        });
      }
      const topAttention = homeAttentionItems.slice(0, 6);
      const hasUpdaterAttention = topAttention.some((item) => item.id.startsWith("updater:"));
      const launcherActionIsInstall = Boolean(appUpdaterState?.available);
      const launcherActionLabel = launcherActionIsInstall
        ? (appUpdaterInstallBusy ? "Installing…" : "Install launcher update")
        : (appUpdaterBusy ? "Checking…" : "Check launcher update");
      const onLauncherMaintenanceAction = () => {
        if (launcherActionIsInstall) {
          void onInstallAppUpdate();
          return;
        }
        void onCheckAppUpdate({ silent: false });
      };
      const homeWidgetLabels: Record<HomeWidgetId, string> = {
        action_required: "Action required",
        launchpad: "Launchpad",
        recent_activity: "Recent activity",
        performance_pulse: "Performance pulse",
        friend_link: "Friend Link readiness",
        maintenance: "Maintenance",
        running_sessions: "Running sessions",
        recent_instances: "Recent instances",
      };
      const homeWidgetDescriptions: Record<HomeWidgetId, string> = {
        action_required: "Urgent blockers that need attention before you launch or update.",
        launchpad: "Shortcuts for the most common launcher actions.",
        recent_activity: "A compact feed of your latest launcher activity.",
        performance_pulse: "Recent timing samples for checks, installs, and launcher actions.",
        friend_link: "Status and follow-up actions for the focused instance's Friend Link setup.",
        maintenance: "Update schedule, launcher upkeep, and maintenance shortcuts.",
        running_sessions: "Instances that are currently active right now.",
        recent_instances: "Quick access to the instances you touched most recently.",
      };
      const orderedHomeLayout = [...homeLayout].sort(
        (a, b) => Number(b.pinned) - Number(a.pinned) || a.order - b.order
      );
      const autoHiddenHomeWidgets = new Set<HomeWidgetId>();
      if (!homeCustomizeOpen) {
        if (recentActivity.length === 0) autoHiddenHomeWidgets.add("recent_activity");
        if (runningInstances.length === 0) autoHiddenHomeWidgets.add("running_sessions");
        if (!focusFriendStatus?.linked) autoHiddenHomeWidgets.add("friend_link");
      }
      const visibleHomeMainWidgets = orderedHomeLayout.filter(
        (item) => item.visible && item.column === "main" && !autoHiddenHomeWidgets.has(item.id)
      );
      const visibleHomeSideWidgets = orderedHomeLayout.filter(
        (item) => item.visible && item.column === "side" && !autoHiddenHomeWidgets.has(item.id)
      );
      const hiddenHomeWidgetCount = homeLayout.filter((item) => !item.visible).length;
      const mainColumnLayoutItems = orderedHomeLayout.filter((item) => item.column === "main");
      const sideColumnLayoutItems = orderedHomeLayout.filter((item) => item.column === "side");

      const homeWidgetCards: Record<HomeWidgetId, ReactNode> = {
        action_required: (
          <div className="card homePanel" id="setting-anchor-global:action-required">
            <div className="homePanelHead">
              <div className="homePanelTitle">Action required</div>
              <span className="chip">{topAttention.length} item{topAttention.length === 1 ? "" : "s"}</span>
            </div>
            {topAttention.length === 0 ? (
              <div className="homeAttentionClear">
                <div className="homeRowTitle">All clear</div>
                <div className="homeRowMeta">No urgent launch, update, or account blockers right now.</div>
              </div>
            ) : (
              <div className="homeAlertList">
                {topAttention.map((item) => (
                  <div key={item.id} className={`homeAlertRow ${item.tone}`}>
                    <div className="homeRowMain">
                      <div className="homeRowTitle">{item.title}</div>
                      <div className="homeRowMeta">{item.meta}</div>
                    </div>
                    <div className="homeRowActions">
                      <button
                        className={`btn ${item.tone === "danger" ? "danger" : ""}`}
                        onClick={item.on_action}
                        disabled={item.action_disabled}
                      >
                        {item.action_label}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        ),
        launchpad: (
          <div className="card homePanel">
            <div className="homePanelHead">
              <div className="homePanelTitle">Launchpad</div>
            </div>
            <div className="homeLaunchpad">
              <button className="homeLaunchAction" onClick={() => setShowCreate(true)}>
                <Icon name="plus" size={18} />
                <div className="homeRowMain">
                  <div className="homeRowTitle">Create instance</div>
                  <div className="homeRowMeta">Start a new profile and version.</div>
                </div>
              </button>
              <button className="homeLaunchAction" onClick={() => setRoute("discover")}>
                <Icon name="compass" size={18} />
                <div className="homeRowMain">
                  <div className="homeRowTitle">Discover content</div>
                  <div className="homeRowMeta">Find mods and install fast.</div>
                </div>
              </button>
              <button className="homeLaunchAction" onClick={() => setRoute("library")}>
                <Icon name="books" size={18} />
                <div className="homeRowMain">
                  <div className="homeRowTitle">Open library</div>
                  <div className="homeRowMeta">Manage all instances in detail.</div>
                </div>
              </button>
              <button className="homeLaunchAction" onClick={() => setRoute("modpacks")}>
                <Icon name="box" size={18} />
                <div className="homeRowMain">
                  <div className="homeRowTitle">Creator Studio</div>
                  <div className="homeRowMeta">Build and apply layered modpacks.</div>
                </div>
              </button>
            </div>
          </div>
        ),
        recent_activity: (
          <div className="card homePanel">
            <div className="homePanelHead">
              <div className="homePanelTitle">Recent activity</div>
              <span className="homeMeta">Latest {recentActivity.length}</span>
            </div>
            {recentActivity.length === 0 ? (
              <div className="homeEmpty">
                <div>No activity yet. Launch an instance or install content to populate this feed.</div>
              </div>
            ) : (
              <div className="homeList">
                {recentActivity.map((item) => (
                  <div key={item.id} className="homeListRow">
                    <div className="homeRowMain">
                      <div className="homeRowTitle">{item.message}</div>
                      <div className="homeRowMeta">
                        {item.instance_name} • {new Date(item.at).toLocaleTimeString()}
                      </div>
                    </div>
                    <div className="homeRowActions">
                      <span className={`chip ${item.tone === "error" ? "danger" : "subtle"}`}>
                        {item.tone}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        ),
        performance_pulse: (
          <details className="card homePanel homeFoldPanel">
            <summary className="homeFoldSummary">
              <div className="homePanelTitle">Latest timings</div>
              <span className="homeMeta">
                Avg {perfActionMetrics ? formatDurationMs(perfActionMetrics.avg_ms) : "n/a"} · P95{" "}
                {perfActionMetrics ? formatDurationMs(perfActionMetrics.p95_ms) : "n/a"}
              </span>
            </summary>
            <div className="homeMeta">{perfTrendLabel}</div>
            {topSlowPerfActions.length === 0 ? (
              <div className="homeEmpty homePerfEmpty">
                <div className="homePerfSkeleton" aria-hidden="true">
                  <span />
                  <span />
                  <span />
                  <span />
                  <span />
                  <span />
                </div>
                <div>Launch an instance and OpenJar will start collecting timing samples here.</div>
              </div>
            ) : (
              <div className="homePerfList">
                {topSlowPerfActions.map((entry) => (
                  <div key={entry.id} className="homeListRow">
                    <div className="homeRowMain">
                      <div className="homeRowTitle">{formatPerfActionLabel(entry.name)}</div>
                      <div className="homeRowMeta">
                        {new Date(entry.finished_at).toLocaleTimeString()}
                        {entry.detail ? ` • ${entry.detail}` : ""}
                      </div>
                    </div>
                    <div className="homeRowActions">
                      <span className="chip">{formatDurationMs(entry.duration_ms)}</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </details>
        ),
        friend_link: (
          <div className="card homePanel">
            <div className="homePanelHead">
              <div className="homePanelTitle">Friend Link readiness</div>
              <span
                className={`chip ${
                  focusFriendStatus?.linked && (focusFriendStatus.pending_conflicts_count ?? 0) > 0
                    ? "danger"
                    : "subtle"
                }`}
              >
                {focusFriendStatus?.linked
                  ? (focusFriendStatus.status || "linked").replace(/_/g, " ")
                  : "not linked"}
              </span>
            </div>
            {focusInstance && focusFriendStatus?.linked ? (
              <>
                <div className="homeMeta">
                  {focusFriendStatus.peers.filter((peer) => peer.online).length}/{focusFriendStatus.peers.length} peers online
                  {(focusFriendStatus.pending_conflicts_count ?? 0) > 0
                    ? ` • ${focusFriendStatus.pending_conflicts_count} conflict${focusFriendStatus.pending_conflicts_count === 1 ? "" : "s"} pending`
                    : " • No pending conflicts"}
                </div>
                <div className="homePanelActions">
                  <button
                    className="btn"
                    onClick={() => void onManualFriendLinkSync(focusInstance.id)}
                    disabled={friendLinkSyncBusyInstanceId === focusInstance.id}
                  >
                    {friendLinkSyncBusyInstanceId === focusInstance.id ? "Syncing…" : "Sync now"}
                  </button>
                  {(focusFriendStatus.pending_conflicts_count ?? 0) > 0 ? (
                    <button className="btn danger" onClick={() => openInstance(focusInstance.id)}>
                      Resolve conflicts
                    </button>
                  ) : null}
                  <button className="btn" onClick={() => openInstance(focusInstance.id)}>
                    Open links
                  </button>
                </div>
              </>
            ) : (
              <div className="homeEmpty">
                <div>Friend Link is not active for the focused instance yet.</div>
                <button
                  className="btn"
                  onClick={() => {
                    if (focusInstance) openInstance(focusInstance.id);
                  }}
                  disabled={!focusInstance}
                >
                  {focusInstance ? "Open instance links" : "Select an instance first"}
                </button>
              </div>
            )}
          </div>
        ),
        maintenance: (
          <div className="card homePanel">
            <div className="homePanelHead">
              <div className="homePanelTitle">Maintenance</div>
            </div>
            <div className="homeList">
              <div className="homeListRow">
                <div className="homeRowMain">
                  <div className="homeRowTitle">Content update schedule</div>
                  <div className="homeRowMeta">
                    {updateCadenceLabel(updateCheckCadence)} • Last run{" "}
                    {scheduledUpdateLastRunAt ? formatDateTime(scheduledUpdateLastRunAt, "Never") : "Never"}
                  </div>
                </div>
                <span className="chip subtle">{scheduledInstancesWithUpdatesCount} with updates</span>
              </div>
              <div className="homeListRow">
                <div className="homeRowMain">
                  <div className="homeRowTitle">Next scheduled check</div>
                  <div className="homeRowMeta">
                    {updateCheckCadence === "off"
                      ? "Disabled"
                      : nextScheduledUpdateRunAt
                        ? formatDateTime(nextScheduledUpdateRunAt, "Pending first check")
                        : "Pending first check"}
                  </div>
                </div>
                <span className={`chip ${scheduledUpdateBusy ? "" : "subtle"}`}>
                  {scheduledUpdateBusy
                    ? `${scheduledUpdateRunCompleted}/${scheduledUpdateRunTotal}`
                    : "idle"}
                </span>
              </div>
            </div>
            <div className="homeMeta">
              {scheduledUpdateBusy ? (
                <>
                  Progress {scheduledUpdateRunCompleted}/{scheduledUpdateRunTotal} • Elapsed{" "}
                  {formatEtaSeconds(scheduledUpdateRunElapsedSeconds)} • ETA{" "}
                  {formatEtaSeconds(scheduledUpdateRunEtaSeconds)}
                </>
              ) : (
                <>Mode: {updateAutoApplyModeLabel(updateAutoApplyMode)} ({updateApplyScopeLabel(updateApplyScope)})</>
              )}
            </div>
            {hasUpdaterAttention ? (
              <div className="homeMeta">Launcher update actions are pinned in Action required.</div>
            ) : null}
            <div className="homePanelActions homeMaintenanceActions">
              <button className="btn" onClick={() => setRoute("updates")}>Open content updates</button>
              <button className="btn primary" onClick={() => void runScheduledUpdateChecks("manual")} disabled={scheduledUpdateBusy}>
                {scheduledUpdateBusy ? "Checking…" : "Run content check"}
              </button>
              {!hasUpdaterAttention ? (
                <button
                  className="btn subtle"
                  onClick={onLauncherMaintenanceAction}
                  disabled={appUpdaterBusy || appUpdaterInstallBusy}
                >
                  {launcherActionLabel}
                </button>
              ) : null}
            </div>
          </div>
        ),
        running_sessions: (
          <div className="card homePanel">
            <div className="homePanelHead">
              <div className="homePanelTitle">Running sessions</div>
              <span className="chip subtle">{runningInstances.length}</span>
            </div>
            {runningInstances.length === 0 ? (
              <div className="homeEmpty">
                <div className="compactEmptyState compactEmptyStateInline">
                  <span className="compactEmptyIcon" aria-hidden="true">
                    <Icon name="play" size={14} />
                  </span>
                  <div className="compactEmptyBody">
                    <div className="compactEmptyTitle">Nothing running right now</div>
                    <div className="compactEmptyText">Hit Play on any instance to start a session.</div>
                  </div>
                </div>
              </div>
            ) : (
              <div className="homeList">
                {runningInstances.slice(0, 5).map((run) => (
                  <div key={run.launch_id} className="homeListRow">
                    <div className="homeRowMain">
                      <div className="homeRowTitle">{run.instance_name}</div>
                      <div className="homeRowMeta">
                        {humanizeToken(run.method)}
                        {run.isolated ? " • Disposable session" : ""}
                        {" • "}Started {formatDateTime(run.started_at, "just now")}
                      </div>
                    </div>
                    <div className="homeRowActions">
                      <button className="btn" onClick={() => openInstance(run.instance_id)}>Open</button>
                      <button className="btn danger" onClick={() => void onStopRunning(run.launch_id)}>Stop</button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        ),
        recent_instances: (
          <div className="card homePanel">
            <div className="homePanelHead">
              <div className="homePanelTitle">Recent instances</div>
            </div>
            {recentInstances.length === 0 ? (
              <div className="homeEmpty">
                <div>No instances yet.</div>
                <button className="btn primary" onClick={() => setShowCreate(true)}>Create instance</button>
              </div>
            ) : (
              <div className="homeList">
                {recentInstances.map((inst) => (
                  <div key={inst.id} className="homeListRow">
                    <div className="homeRowMain">
                      <div className="homeRowTitle">{inst.name}</div>
                      <div className="homeRowMeta">
                        {loaderLabelFor(inst)} • Minecraft {inst.mc_version}
                        {instanceHealthById[inst.id]
                          ? ` • Health ${instanceHealthById[inst.id].grade} ${instanceHealthById[inst.id].score}`
                          : ""}
                      </div>
                    </div>
                    <div className="homeRowActions homeRecentActions">
                      {runningIds.has(inst.id) ? <span className="chip">Running</span> : null}
                      <button className="btn" onClick={() => openInstance(inst.id)}>Open</button>
                      <button
                        className={`btn ${launchBusyInstanceIds.includes(inst.id) ? "danger" : "primary"}`}
                        onClick={() => onPlayInstance(inst)}
                        disabled={launchCancelBusyInstanceId === inst.id}
                      >
                        {launchBusyInstanceIds.includes(inst.id) ? "Cancel" : "Play"}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        ),
      };

      const renderHomeWidget = (layoutItem: HomeWidgetLayoutItem) => {
        const content = homeWidgetCards[layoutItem.id];
        if (!content) return null;
        const canNudgeUp = orderedHomeLayout
          .filter((item) => item.column === layoutItem.column)
          .sort((a, b) => Number(b.pinned) - Number(a.pinned) || a.order - b.order)
          .findIndex((item) => item.id === layoutItem.id) > 0;
        return (
          <div
            key={layoutItem.id}
            className={`homeWidgetShell ${homeCustomizeOpen ? "customizeOn" : ""} ${
              draggedHomeWidgetId === layoutItem.id ? "dragging" : ""
            }`}
            draggable={homeCustomizeOpen}
            onDragStart={(event) => {
              if (!homeCustomizeOpen) return;
              event.dataTransfer.effectAllowed = "move";
              setDraggedHomeWidgetId(layoutItem.id);
            }}
            onDragEnd={() => setDraggedHomeWidgetId(null)}
            onDragOver={(event) => {
              if (!homeCustomizeOpen || draggedHomeWidgetId === layoutItem.id) return;
              event.preventDefault();
            }}
            onDrop={(event) => {
              if (!homeCustomizeOpen) return;
              event.preventDefault();
              reorderHomeWidget(layoutItem.id);
              setDraggedHomeWidgetId(null);
            }}
          >
            {homeCustomizeOpen ? (
              <div className="homeWidgetCustomizeStrip">
                <div className="homeWidgetCustomizeLabelGroup">
                  <span className="chip subtle">{homeWidgetLabels[layoutItem.id]}</span>
                  <span className="homeMeta">Drag to reorder</span>
                </div>
                {layoutItem.pinned ? <span className="chip">Pinned</span> : null}
                {canNudgeUp ? <span className="chip subtle">Can move up</span> : <span className="chip subtle">Top of column</span>}
              </div>
            ) : null}
            {content}
          </div>
        );
      };

      return (
        <div className="homeShell page">
          <section className="card homeSpotlight">
            <div className="homeSpotlightMain">
              <div className="homeSpotlightTopRow">
                <div className="homeKicker">Home</div>
              </div>
              <div className="homeSpotlightTitle">
                {focusInstance ? `Ready to launch ${focusInstance.name}` : "Start your first instance"}
              </div>
              <div className="homeSpotlightSub">
                {focusInstance
                  ? `${loaderLabelFor(focusInstance)} instance on Minecraft ${focusInstance.mc_version}`
                  : "Create an instance, install content, and launch in one flow."}
              </div>
              {focusInstance ? (
                <div className="homeChipRow">
                  {focusHealthScore ? (
                    <span className={`chip ${focusHealthScore.score < 60 ? "danger" : "subtle"}`}>
                      Health {focusHealthScore.grade} ({focusHealthScore.score})
                    </span>
                  ) : null}
                  {runningIds.has(focusInstance.id) ? <span className="chip">Running</span> : null}
                  {focusFriendStatus?.linked ? (
                    <span
                      className={`chip ${
                        (focusFriendStatus.pending_conflicts_count ?? 0) > 0 ? "danger" : "subtle"
                      }`}
                    >
                      {(focusFriendStatus.pending_conflicts_count ?? 0) > 0
                        ? `${focusFriendStatus.pending_conflicts_count} Friend Link conflict${
                            (focusFriendStatus.pending_conflicts_count ?? 0) === 1 ? "" : "s"
                          }`
                        : "Friend Link ready"}
                    </span>
                  ) : null}
                </div>
              ) : null}
              <div className="homePanelActions">
                {focusInstance ? (
                  <>
                    <button
                      className={`btn ${launchBusyInstanceIds.includes(focusInstance.id) ? "danger" : "primary"}`}
                      onClick={() => onPlayInstance(focusInstance)}
                      disabled={launchCancelBusyInstanceId === focusInstance.id}
                    >
                      <Icon name={launchBusyInstanceIds.includes(focusInstance.id) ? "x" : "play"} size={16} />
                      {launchBusyInstanceIds.includes(focusInstance.id)
                        ? (launchCancelBusyInstanceId === focusInstance.id ? "Cancelling…" : "Cancel launch")
                        : "Play now"}
                    </button>
                    <button className="btn" onClick={() => openInstance(focusInstance.id)}>
                      Open instance
                    </button>
                  </>
                ) : (
                  <>
                    <button className="btn primary" onClick={() => setShowCreate(true)}>
                      <Icon name="plus" size={16} />
                      Create instance
                    </button>
                    <button className="btn" onClick={() => setRoute("discover")}>
                      Discover content
                    </button>
                  </>
                )}
              </div>
            </div>
              <div className="homeKpis">
                <div className="homeKpi">
                  <div className="homeKpiLabel">Instances</div>
                  <div className="homeKpiValue">{instances.length}</div>
                </div>
              <div className="homeKpi">
                <div className="homeKpiLabel">Running</div>
                <div className="homeKpiValue">{runningInstances.length}</div>
              </div>
                <div className="homeKpi">
                  <div className="homeKpiLabel">Updates waiting</div>
                  <div className="homeKpiValue">{scheduledUpdatesAvailableTotal}</div>
                </div>
                <div className="homeKpi">
                  <div className="homeKpiLabel">Account</div>
                  <div className="homeKpiValue">{selectedLauncherAccount ? "Online" : "Offline"}</div>
                </div>
              </div>
          </section>

          <section className="card homeControlBar">
            <div className="homeControlBarMain">
              <div className="homeControlBarTitle">Workspace layout</div>
              <div className="homeControlBarMeta">
                {homeCustomizeOpen
                  ? "Drag widgets in the page preview, or use the organizer below to show, hide, pin, and move them."
                  : "Tune what appears on Home and where each section lives."}
              </div>
            </div>
            <div className="homeControlBarActions">
              {hiddenHomeWidgetCount > 0 ? (
                <span className="chip subtle">{hiddenHomeWidgetCount} hidden</span>
              ) : null}
              {homeCustomizeOpen ? (
                <span className="chip subtle">Customization mode</span>
              ) : null}
              <button
                className={`btn ${homeCustomizeOpen ? "primary" : ""}`}
                onClick={() => {
                  setHomeCustomizeOpen((prev) => !prev);
                  setDraggedHomeWidgetId(null);
                }}
              >
                {homeCustomizeOpen ? "Done customizing" : "Customize home"}
              </button>
              {homeCustomizeOpen ? (
                <button className="btn" onClick={resetHomeLayout}>
                  Reset defaults
                </button>
              ) : null}
            </div>
          </section>

          {homeCustomizeOpen ? (
            <div className="card homeCustomizePanel">
              <div className="homeCustomizeHeader">
                <div>
                  <div className="homeCustomizeTitle">Organize your Home page</div>
                  <div className="homeCustomizeHint">
                    Keep the essentials up front. Move sections between columns, pin the important ones, or hide the ones you do not need.
                  </div>
                </div>
              </div>
              <div className="homeCustomizeBoard">
                {[
                  { key: "main", label: "Main column", items: mainColumnLayoutItems as HomeWidgetLayoutItem[] },
                  { key: "side", label: "Side column", items: sideColumnLayoutItems as HomeWidgetLayoutItem[] },
                ].map((group) => (
                  <div key={group.key} className="homeCustomizeColumn">
                    <div className="homeCustomizeColumnHead">
                      <div className="homeCustomizeColumnTitle">{group.label}</div>
                      <span className="chip subtle">
                        {group.items.filter((item) => item.visible).length} shown
                      </span>
                    </div>
                    <div className="homeCustomizeList">
                      {group.items.map((item) => (
                        <div
                          key={`customize:${item.id}`}
                          className={`homeCustomizeItem ${item.visible ? "" : "hidden"}`}
                        >
                          <div className="homeCustomizeItemMain">
                            <div className="homeCustomizeItemTopRow">
                              <div className="homeCustomizeItemTitle">{homeWidgetLabels[item.id]}</div>
                              <div className="homeCustomizeItemStatus">
                                <span className={`chip ${item.visible ? "subtle" : ""}`}>
                                  {item.visible ? "Shown" : "Hidden"}
                                </span>
                                {item.pinned ? <span className="chip">Pinned</span> : null}
                              </div>
                            </div>
                            <div className="homeCustomizeItemMeta">{homeWidgetDescriptions[item.id]}</div>
                          </div>
                          <div className="homeCustomizeItemActions">
                            <button
                              className={`btn ${item.visible ? "primary" : ""}`}
                              onClick={() => patchHomeLayout(item.id, { visible: !item.visible })}
                            >
                              {item.visible ? "Hide" : "Show"}
                            </button>
                            <button
                              className={`btn ${item.pinned ? "primary" : ""}`}
                              onClick={() => patchHomeLayout(item.id, { pinned: !item.pinned })}
                            >
                              {item.pinned ? "Unpin" : "Pin"}
                            </button>
                            <button
                              className="btn"
                              onClick={() =>
                                patchHomeLayout(item.id, {
                                  column: item.column === "main" ? "side" : "main",
                                })
                              }
                            >
                              Send to {item.column === "main" ? "side" : "main"}
                            </button>
                            <button className="btn" onClick={() => nudgeHomeWidget(item.id, -1)}>
                              Up
                            </button>
                            <button className="btn" onClick={() => nudgeHomeWidget(item.id, 1)}>
                              Down
                            </button>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ) : null}

          <div className="homeMainGrid">
            <section
              className={`homeMainCol ${homeCustomizeOpen ? "customizeDropZone" : ""}`}
              onDragOver={(event) => {
                if (!homeCustomizeOpen || !draggedHomeWidgetId) return;
                event.preventDefault();
              }}
              onDrop={(event) => {
                if (!homeCustomizeOpen || !draggedHomeWidgetId) return;
                event.preventDefault();
                moveHomeWidgetToColumn("main");
                setDraggedHomeWidgetId(null);
              }}
            >
              {visibleHomeMainWidgets.length === 0 ? (
                <div className="homeEmpty">No widgets in the main column.</div>
              ) : (
                visibleHomeMainWidgets.map((item) => renderHomeWidget(item))
              )}
            </section>

            <aside
              className={`homeSideCol ${homeCustomizeOpen ? "customizeDropZone" : ""}`}
              onDragOver={(event) => {
                if (!homeCustomizeOpen || !draggedHomeWidgetId) return;
                event.preventDefault();
              }}
              onDrop={(event) => {
                if (!homeCustomizeOpen || !draggedHomeWidgetId) return;
                event.preventDefault();
                moveHomeWidgetToColumn("side");
                setDraggedHomeWidgetId(null);
              }}
            >
              {visibleHomeSideWidgets.length === 0 ? (
                <div className="homeEmpty">No widgets in the side column.</div>
              ) : (
                visibleHomeSideWidgets.map((item) => renderHomeWidget(item))
              )}
            </aside>
          </div>
        </div>
      );
    }

    if (route === "settings") {
      const selectedPermissionsInstance = selectedId
        ? instances.find((item) => item.id === selectedId) ?? null
        : null;
      const selectedPermissionsChecklist: LaunchPermissionChecklistItem[] = selectedPermissionsInstance
        ? preflightReportByInstance[selectedPermissionsInstance.id]?.permissions ?? []
        : [];
      const settingsSelectedAccount = selectedLauncherAccount ?? launcherAccounts[0] ?? null;
      const settingsUpdateStatusLabel = appUpdaterState?.available
        ? `Update ready${appUpdaterState.latest_version ? ` · v${appUpdaterState.latest_version}` : ""}`
        : appUpdaterState
          ? "Up to date"
          : "Not checked yet";
      return (
        <div className="settingsPage">
          <div className="settingsShell">
            <aside className="card settingsSidebar">
              <div className="settingsSidebarHeader">
                <div className="settingsSidebarEyebrow">Launcher settings</div>
                <div className="settingsSidebarTitle">{t("settings.title")}</div>
                <div className="settingsSidebarIntro">{t("settings.intro")}</div>
              </div>

              <div className="settingsSidebarBlock">
                <div className="settingsSidebarLabel">View mode</div>
                <SegmentedControl
                  className="settingsModeToggle"
                  value={settingsMode}
                  onChange={(value) => setSettingsMode(((value ?? "basic") as SettingsMode))}
                  options={[
                    { value: "basic", label: t("settings.mode.basic") },
                    { value: "advanced", label: t("settings.mode.advanced") },
                  ]}
                />
              </div>

              <div className="settingsSidebarBlock">
                <div className="settingsSidebarLabel">Sections</div>
                <div className="settingsRailList">
                  {settingsRailItems.map((item) => (
                    <button
                      key={item.id}
                      className={`settingsRailButton ${activeSettingsRail === item.id ? "active" : ""}`}
                      onClick={() => openSettingAnchor(item.id, { advanced: item.advanced, target: "global" })}
                    >
                      <span className="settingsRailButtonIcon" aria-hidden="true">
                        <Icon name={item.icon} size={16} />
                      </span>
                      <span className="settingsRailButtonText">
                        <span className="settingsRailButtonTitle">{item.label}</span>
                        {item.advanced ? <span className="settingsRailButtonMeta">Advanced</span> : null}
                      </span>
                    </button>
                  ))}
                </div>
              </div>

              <div className="settingsSidebarFooter">
                <span className="chip subtle">{getAppLanguageOption(appLanguage).nativeLabel}</span>
                <span className="chip subtle">
                  {settingsMode === "advanced" ? t("settings.mode.advanced") : t("settings.mode.basic")}
                </span>
              </div>
            </aside>

            <div className="settingsMain">
              <div className="card settingsHeroCard">
                <div className="settingsHeroHeader">
                  <div>
                    <div className="settingsHeroEyebrow">Overview</div>
                    <div className="settingsHeroTitle">Make the launcher feel right before you dive deeper.</div>
                    <div className="settingsHeroSub">
                      Everyday settings stay up front. Advanced controls are still here, but they should stop competing with the basics.
                    </div>
                  </div>
                  <div className="settingsHeroActions">
                    <button className="btn" onClick={() => openSettingAnchor("global:appearance", { target: "global" })}>
                      Appearance
                    </button>
                    <button className="btn" onClick={() => openSettingAnchor("global:launch-method", { target: "global" })}>
                      Launch
                    </button>
                    <button className="btn" onClick={() => openSettingAnchor("global:account", { target: "global" })}>
                      Account
                    </button>
                    <button className="btn" onClick={() => openSettingAnchor("global:app-updates", { target: "global" })}>
                      Updates
                    </button>
                  </div>
                </div>

                <div className="settingsHeroSummaryGrid">
                  <div className="settingsHeroSummaryCard">
                    <div className="settingsHeroSummaryLabel">View mode</div>
                    <div className="settingsHeroSummaryValue">
                      {settingsMode === "advanced" ? t("settings.mode.advanced") : t("settings.mode.basic")}
                    </div>
                    <div className="settingsHeroSummaryMeta">
                      {settingsMode === "advanced"
                        ? "Power-user controls are visible."
                        : "Only the common settings are emphasized."}
                    </div>
                  </div>
                  <div className="settingsHeroSummaryCard">
                    <div className="settingsHeroSummaryLabel">Language</div>
                    <div className="settingsHeroSummaryValue">{getAppLanguageOption(appLanguage).nativeLabel}</div>
                    <div className="settingsHeroSummaryMeta">App UI language</div>
                  </div>
                  <div className="settingsHeroSummaryCard">
                    <div className="settingsHeroSummaryLabel">Default launch</div>
                    <div className="settingsHeroSummaryValue">{humanizeToken(launchMethodPick)}</div>
                    <div className="settingsHeroSummaryMeta">Used when an instance does not override it</div>
                  </div>
                  <div className="settingsHeroSummaryCard">
                    <div className="settingsHeroSummaryLabel">Connected account</div>
                    <div className="settingsHeroSummaryValue">
                      {settingsSelectedAccount?.username ?? "Not connected"}
                    </div>
                    <div className="settingsHeroSummaryMeta">
                      {settingsSelectedAccount ? "Ready for native launch" : "Connect Microsoft to launch natively"}
                    </div>
                  </div>
                  <div className="settingsHeroSummaryCard">
                    <div className="settingsHeroSummaryLabel">App updates</div>
                    <div className="settingsHeroSummaryValue">{settingsUpdateStatusLabel}</div>
                    <div className="settingsHeroSummaryMeta">
                      {appUpdaterState?.checked_at
                        ? `Last check ${formatDateTime(appUpdaterState.checked_at, "Never")}`
                        : "No launcher update check yet"}
                    </div>
                  </div>
                </div>
              </div>

              <div className="settingsLayout">
                <section className="settingsCol">
              <div className="card settingsSectionCard" id="setting-anchor-global:appearance">
                <div className="settingsSectionTitle">{t("settings.appearance.section_title")}</div>
                <div className="p settingsSectionSub">{t("settings.appearance.section_sub")}</div>

                <div className="settingStack">
                  <div>
                    <div className="settingTitle">{t("settings.appearance.theme.title")}</div>
                    <div className="settingSub">{t("settings.appearance.theme.sub")}</div>
                    <div className="row">
                      <button
                        className={`btn stateful ${theme === "dark" ? "active" : ""}`}
                        onClick={() => setTheme("dark")}
                      >
                        {t("settings.appearance.theme.dark")}
                      </button>
                      <button
                        className={`btn stateful ${theme === "light" ? "active" : ""}`}
                        onClick={() => setTheme("light")}
                      >
                        {t("settings.appearance.theme.light")}
                      </button>
                    </div>
                  </div>

                  <div>
                    <div className="settingTitle">{t("settings.appearance.accent.title")}</div>
                    <div className="settingSub">{t("settings.appearance.accent.sub")}</div>
                    <div className="row accentPicker">
                      {ACCENT_OPTIONS.map((opt) => (
                        <button
                          key={opt.value}
                          className={`btn accentChoice ${accentPreset === opt.value ? "selected" : ""}`}
                          onClick={() => setAccentPreset(opt.value)}
                          aria-pressed={accentPreset === opt.value}
                        >
                          <span className={`accentSwatch accent-${opt.value}`} />
                          <span className="accentChoiceLabel">{opt.label}</span>
                          {accentPreset === opt.value ? (
                            <span className="accentChoiceCheck" aria-hidden="true">✓</span>
                          ) : null}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div>
                    <div className="settingTitle">{t("settings.appearance.accent_strength.title")}</div>
                    <div className="settingSub">{t("settings.appearance.accent_strength.sub")}</div>
                    <div className="row">
                      <SegmentedControl
                        value={accentStrength}
                        options={ACCENT_STRENGTH_OPTIONS}
                        onChange={(v) => setAccentStrength((v ?? "normal") as AccentStrength)}
                        variant="scroll"
                      />
                    </div>
                  </div>

                  <div>
                    <div className="settingTitle">{t("settings.appearance.motion.title")}</div>
                    <div className="settingSub">{t("settings.appearance.motion.sub")}</div>
                    <div className="row">
                      <SegmentedControl
                        value={motionPreset}
                        options={MOTION_OPTIONS}
                        onChange={(v) => setMotionPreset((v ?? "standard") as MotionPreset)}
                      />
                    </div>
                    <div className="settingsMotionNote" aria-live="polite">
                      <span className="chip subtle">{MOTION_PROFILE_DETAILS[motionPreset].label}</span>
                      {MOTION_PROFILE_DETAILS[motionPreset].traits.map((trait) => (
                        <span key={trait} className="chip subtle">
                          {trait}
                        </span>
                      ))}
                      <span className="settingsMotionNoteText">
                        {MOTION_PROFILE_DETAILS[motionPreset].summary}
                      </span>
                    </div>
                  </div>

                  <div>
                    <div className="settingTitle">{t("settings.appearance.density.title")}</div>
                    <div className="settingSub">{t("settings.appearance.density.sub")}</div>
                    <div className="row">
                      <SegmentedControl
                        value={densityPreset}
                        options={DENSITY_OPTIONS}
                        onChange={(v) => setDensityPreset((v ?? "comfortable") as DensityPreset)}
                      />
                    </div>
                  </div>

                  <div>
                    <div className="settingTitle">{t("settings.appearance.reset.title")}</div>
                    <div className="settingSub">{t("settings.appearance.reset.sub")}</div>
                    <div className="row">
                      <button className="btn" onClick={onResetUiSettings}>
                        {t("settings.appearance.reset.button")}
                      </button>
                    </div>
                  </div>
                </div>
              </div>

              <div className="card settingsSectionCard" id="setting-anchor-global:language">
                <div className="settingsSectionTitle">{t("settings.language.section_title")}</div>
                <div className="p settingsSectionSub">{t("settings.language.section_sub")}</div>

                <div className="settingStack">
                  <div>
                    <div className="settingTitle">{t("settings.language.preference.title")}</div>
                    <div className="settingSub">{t("settings.language.preference.sub")}</div>
                    <div className="row" style={{ alignItems: "center" }}>
                      <MenuSelect
                        value={appLanguage}
                        labelPrefix={t("settings.language.preference.menu_prefix")}
                        options={appLanguageMenuOptions}
                        onChange={(value) => void onSetAppLanguage(value as AppLanguage)}
                      />
                      <span className="chip">{appLanguageBusy ? t("settings.language.saving") : getAppLanguageOption(appLanguage).nativeLabel}</span>
                    </div>
                  </div>

                  <div className="settingSub">{t("settings.language.warning")}</div>
                </div>
              </div>

              <div className="card settingsSectionCard">
                <div className="settingsSectionTitle">{t("settings.launch.section_title")}</div>
                <div className="p settingsSectionSub">{t("settings.launch.section_sub")}</div>

                <div className="settingStack">
                  <div id="setting-anchor-global:launch-method">
                    <div className="settingTitle">{t("settings.launch.method.title")}</div>
                    <div className="settingSub">{t("settings.launch.method.sub")}</div>
                    <div className="row">
                      <SegmentedControl
                        value={launchMethodPick}
                        onChange={(v) => setLaunchMethodPick((v ?? "native") as LaunchMethod)}
                        options={[
                          { label: t("settings.launch.method.native"), value: "native" },
                          { label: t("settings.launch.method.prism"), value: "prism" },
                        ]}
                      />
                    </div>
                  </div>

                  {settingsMode === "advanced" ? (
                    <>
                      <div id="setting-anchor-global:java-path">
                        <div className="settingTitle">{t("settings.launch.java.title")}</div>
                        <div className="settingSub">{t("settings.launch.java.sub")}</div>
                        <input
                          className="input"
                          value={javaPathDraft}
                          onChange={(e) => setJavaPathDraft(e.target.value)}
                          placeholder="/usr/bin/java or C:\\Program Files\\Java\\bin\\java.exe"
                        />
                        <div className="settingsActionGrid">
                          <button className="btn" onClick={onPickLauncherJavaPath} disabled={launcherBusy}>
                            <span className="btnIcon">
                              <Icon name="upload" size={17} />
                            </span>
                            {t("settings.launch.java.browse")}
                          </button>
                          <button className="btn" onClick={() => void refreshJavaRuntimeCandidates()} disabled={javaRuntimeBusy}>
                            {javaRuntimeBusy
                              ? t("settings.launch.java.detecting")
                              : t("settings.launch.java.detect")}
                          </button>
                          <button
                            className="btn"
                            onClick={() => void openExternalLink("https://adoptium.net/temurin/releases/?version=21")}
                          >
                            {t("settings.launch.java.get_java_21")}
                          </button>
                        </div>
                        {javaRuntimeCandidates.length > 0 ? (
                          <div className="settingListMini">
                            {javaRuntimeCandidates.map((runtime) => (
                              <div key={runtime.path} className="settingListMiniRow">
                                <div style={{ minWidth: 0 }}>
                                  <div style={{ fontWeight: 900 }}>{javaRuntimeDisplayLabel(runtime)}</div>
                                  <div className="muted" style={{ wordBreak: "break-all" }}>{runtime.path}</div>
                                </div>
                                <button
                                  className={`btn stateful ${javaPathDraft.trim() === runtime.path.trim() ? "active" : ""}`}
                                  onClick={() => setJavaPathDraft(runtime.path)}
                                  disabled={launcherBusy}
                                >
                                  {javaPathDraft.trim() === runtime.path.trim()
                                    ? t("settings.launch.java.selected")
                                    : t("settings.launch.java.use")}
                                </button>
                              </div>
                            ))}
                          </div>
                        ) : null}
                      </div>

                      <div id="setting-anchor-global:oauth-client">
                        <div className="settingTitle">{t("settings.launch.oauth.title")}</div>
                        <div className="settingSub">{t("settings.launch.oauth.sub")}</div>
                        <input
                          className="input"
                          value={oauthClientIdDraft}
                          onChange={(e) => setOauthClientIdDraft(e.target.value)}
                          placeholder={t("settings.launch.oauth.placeholder")}
                          style={{ marginTop: 8 }}
                        />
                      </div>
                    </>
                  ) : (
                    <div className="muted">
                      {t("settings.launch.basic_hidden")}
                      <button className="btn" style={{ marginLeft: 8 }} onClick={() => setSettingsMode("advanced")}>
                        {t("settings.launch.switch_to_advanced")}
                      </button>
                    </div>
                  )}

                  <div className="settingsSaveRow">
                    <button className="btn primary" onClick={onSaveLauncherPrefs} disabled={launcherBusy}>
                      {launcherBusy ? t("settings.launch.saving") : t("settings.launch.save")}
                    </button>
                  </div>
                </div>
              </div>
            </section>

            <section className="settingsCol">
              <div className="card settingsSectionCard" id="setting-anchor-global:account">
                <div className="settingsSectionTitle">{t("settings.account.section_title")}</div>
                <div className="p settingsSectionSub">{t("settings.account.section_sub")}</div>

                <div className="row" style={{ marginTop: 8 }}>
                  <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                    {msLoginSessionId ? "Waiting for login…" : "Connect Microsoft"}
                  </button>
                  {msLoginSessionId && msCodePrompt ? (
                    <button className="btn" onClick={() => setMsCodePromptVisible(true)}>
                      Show code
                    </button>
                  ) : null}
                  <button className="btn" onClick={() => setRoute("account")}>
                    Open account page
                  </button>
                  {msLoginState?.message ? <div className="muted">{msLoginState.message}</div> : null}
                </div>

                <div className="settingsAccountList">
                  {launcherAccounts.length === 0 ? (
                    <div className="muted">No connected account yet.</div>
                  ) : (
                    launcherAccounts.map((acct) => {
                      const selectedAccount = launcherSettings?.selected_account_id === acct.id;
                      const manageOpen = settingsAccountManageId === acct.id;
                      return (
                        <div key={acct.id} className="card settingsAccountCard">
                          <div className="settingsAccountRow">
                            <div style={{ minWidth: 0 }}>
                              <div style={{ fontWeight: 900 }}>{acct.username}</div>
                              <div className="muted">{acct.id}</div>
                            </div>
                            <div className="settingsAccountActions">
                              <button
                                className={`btn stateful ${selectedAccount ? "active" : ""}`}
                                onClick={() => onSelectAccount(acct.id)}
                                disabled={launcherBusy}
                              >
                                {selectedAccount ? "Selected" : "Use"}
                              </button>
                              <button
                                className={`btn subtle settingsManageBtn ${manageOpen ? "active" : ""}`}
                                onClick={() =>
                                  setSettingsAccountManageId((prev) => (prev === acct.id ? null : acct.id))
                                }
                                disabled={launcherBusy}
                                aria-expanded={manageOpen}
                                aria-controls={`settings-account-manage-${acct.id}`}
                              >
                                Manage…
                              </button>
                            </div>
                          </div>
                          {manageOpen ? (
                            <div className="settingsAccountManagePanel" id={`settings-account-manage-${acct.id}`}>
                              <div className="settingsAccountManageHint">
                                Disconnect removes this account from this launcher on this device.
                              </div>
                              <button
                                className="btn accountDisconnectBtn"
                                onClick={() => {
                                  if (!window.confirm(`Disconnect ${acct.username} from this launcher?`)) return;
                                  void onLogoutAccount(acct.id);
                                }}
                                disabled={launcherBusy}
                              >
                                Disconnect account
                              </button>
                            </div>
                          ) : null}
                        </div>
                      );
                    })
                  )}
                </div>
              </div>

              <div className="card settingsSectionCard" id="setting-anchor-global:app-updates">
                <div className="settingsSectionTitle">{t("settings.updates.section_title")}</div>
                <div className="p settingsSectionSub">
                  Check for new OpenJar Launcher releases, then install with explicit restart confirmation.
                </div>

                <div className="row">
                  <span className="chip subtle">Current: v{appVersion || "unknown"}</span>
                  {appUpdaterState ? (
                    <span className="chip subtle">
                      Last check: {formatDateTime(appUpdaterState.checked_at, "Never")}
                    </span>
                  ) : null}
                  {appUpdaterState?.available ? (
                    <span className="chip">Update: v{appUpdaterState.latest_version ?? "new"}</span>
                  ) : appUpdaterState ? (
                    <span className="chip subtle">Up to date</span>
                  ) : null}
                </div>
                <div className="settingsActionGrid">
                  <button
                    className="btn"
                    onClick={() => void onCheckAppUpdate({ silent: false })}
                    disabled={appUpdaterBusy || appUpdaterInstallBusy}
                  >
                    {appUpdaterBusy ? "Checking…" : "Check app updates"}
                  </button>
                  <button
                    className={`btn ${appUpdaterState?.available ? "primary" : ""}`}
                    onClick={() => void onInstallAppUpdate()}
                    disabled={!appUpdaterState?.available || appUpdaterBusy || appUpdaterInstallBusy}
                  >
                    {appUpdaterInstallBusy ? "Installing…" : "Install update + restart"}
                  </button>
                </div>
                <div className="settingStackMini">
                  <label className="toggleRow settingsToggleRow">
                    <input
                      type="checkbox"
                      checked={appUpdaterAutoCheck}
                      onChange={() => setAppUpdaterAutoCheck((prev) => !prev)}
                      disabled={appUpdaterBusy || appUpdaterInstallBusy}
                    />
                    <span className="togglePill" />
                    <span>Auto-check on launch</span>
                  </label>
                  <div className="settingSub">Checks for OpenJar Launcher releases when the app starts.</div>
                </div>
                {appUpdaterState?.release_notes ? (
                  <div className="muted" style={{ marginTop: 8, whiteSpace: "pre-wrap" }}>
                    {appUpdaterState.release_notes.slice(0, 280)}
                    {appUpdaterState.release_notes.length > 280 ? "…" : ""}
                  </div>
                ) : null}
              </div>

              <div className="card settingsSectionCard" id="setting-anchor-global:content-visuals">
                <div className="settingsSectionTitle">{t("settings.content.section_title")}</div>
                <div className="p settingsSectionSub">Quick toggles for launcher behavior outside game runtime.</div>

                <div className="settingStack">
                  <div>
                    <div className="settingTitle">Automatic identify local files</div>
                    <div className="settingSub">
                      When enabled, local file imports automatically run Identify local files in Instance and Creator Studio.
                    </div>
                    <label className="toggleRow settingsToggleRow">
                      <input
                        type="checkbox"
                        checked={Boolean(launcherSettings?.auto_identify_local_jars)}
                        onChange={() => void onToggleAutoIdentifyLocalJars()}
                        disabled={autoIdentifyLocalJarsBusy}
                      />
                      <span className="togglePill" />
                      <span>{autoIdentifyLocalJarsBusy ? "Saving…" : "Identify local files automatically"}</span>
                    </label>
                  </div>

                  <div>
                    <div className="settingTitle">3D skin preview</div>
                    <div className="settingSub">
                      Disable this for faster Account and Skins page loads on lower-end hardware.
                    </div>
                    <label className="toggleRow settingsToggleRow">
                      <input
                        type="checkbox"
                        checked={skinPreviewEnabled}
                        onChange={() => setSkinPreviewEnabled((prev) => !prev)}
                      />
                      <span className="togglePill" />
                      <span>Enable 3D skin preview</span>
                    </label>
                  </div>

                  <div id="setting-anchor-global:discord-presence">
                    <div className="settingTitle">Discord Rich Presence</div>
                    <div className="settingSub">
                      Optional status sharing. Never includes server IP, username, world name, or file paths.
                    </div>
                    <label className="toggleRow settingsToggleRow">
                      <input
                        type="checkbox"
                        checked={Boolean(launcherSettings?.discord_presence_enabled ?? true)}
                        onChange={() => void onToggleDiscordPresenceEnabled()}
                        disabled={discordPresenceBusy}
                      />
                      <span className="togglePill" />
                      <span>{discordPresenceBusy ? "Saving…" : "Enable Discord Rich Presence"}</span>
                    </label>
                    <div className="row">
                      <MenuSelect
                        value={String(launcherSettings?.discord_presence_detail_level ?? "minimal")}
                        labelPrefix="Detail"
                        options={[
                          { value: "minimal", label: "Minimal" },
                          { value: "expanded", label: "Expanded" },
                        ]}
                        onChange={(value) =>
                          void onSetDiscordPresenceDetailLevel(
                            String(value ?? "minimal") === "expanded" ? "expanded" : "minimal"
                          )
                        }
                      />
                    </div>
                  </div>
                </div>
              </div>

                {settingsMode === "advanced" ? (
                  <div className="card settingsSectionCard">
                  <div className="settingsSectionTitle">{t("settings.advanced.section_title")}</div>
                  <div className="p settingsSectionSub">Power-user defaults and launch permission controls.</div>
                  <div className="settingStack">
                    <div>
                      <div className="settingTitle">Power-user defaults</div>
                      <div className="settingSub">
                        Extra launcher behavior toggles for advanced workflows.
                      </div>
                      <label className="toggleRow settingsToggleRow">
                        <input
                          type="checkbox"
                          checked={discoverAddTraySticky}
                          onChange={() => setDiscoverAddTraySticky((prev) => !prev)}
                        />
                        <span className="togglePill" />
                        <span>Keep Discover add tray pinned</span>
                      </label>
                      <label className="toggleRow settingsToggleRow">
                        <input
                          type="checkbox"
                          checked={supportBundleIncludeRawLogs}
                          onChange={() => setSupportBundleIncludeRawLogs((prev) => !prev)}
                        />
                        <span className="togglePill" />
                        <span>Include raw logs by default in support bundles</span>
                      </label>
                      <div className="row">
                        <MenuSelect
                          value={String(logMaxLines)}
                          labelPrefix="Default log window"
                          options={LOG_MAX_LINES_OPTIONS}
                          onChange={(v) => {
                            const parsed = Number.parseInt(String(v ?? ""), 10);
                            if (!Number.isFinite(parsed)) return;
                            setLogMaxLines(Math.max(200, Math.min(12000, parsed)));
                          }}
                        />
                      </div>
                    </div>

                    <details className="settingsFoldSection" id="setting-anchor-global:permissions" open>
                      <summary className="settingsFoldSummary">
                        <span className="settingsFoldTitle">Launch permissions</span>
                        <span className="settingsFoldMeta">Microphone checks and prompt controls</span>
                      </summary>
                      <div className="settingsFoldBody">
                        <div className="settingSub">
                          Voice chat instances can auto-trigger a Java microphone permission probe before launch.
                        </div>
                        <label className="toggleRow settingsToggleRow">
                          <input
                            type="checkbox"
                            checked={Boolean(launcherSettings?.auto_trigger_mic_permission_prompt ?? true)}
                            onChange={() => void onToggleAutoMicPermissionPrompt()}
                            disabled={autoMicPromptSettingBusy}
                          />
                          <span className="togglePill" />
                          <span>
                            {autoMicPromptSettingBusy ? "Saving…" : "Enable auto microphone prompt"}
                          </span>
                        </label>
                        <div className="settingsActionGrid">
                          <button className="btn" onClick={() => void openMicrophoneSystemSettings()}>
                            Open microphone settings
                          </button>
                          <button
                            className="btn"
                            onClick={() =>
                              selectedPermissionsInstance
                                ? void triggerInstanceMicrophonePrompt(selectedPermissionsInstance.id)
                                : setInstallNotice("Select an instance first to trigger microphone prompt.")
                            }
                            disabled={!selectedPermissionsInstance}
                          >
                            Trigger selected prompt
                          </button>
                          <button
                            className="btn"
                            onClick={() =>
                              selectedPermissionsInstance
                                ? void refreshInstancePermissionChecklist(selectedPermissionsInstance.id, launchMethodPick)
                                : setInstallNotice("Select an instance first to run a permission re-check.")
                            }
                            disabled={!selectedPermissionsInstance}
                          >
                            Re-check selected instance
                          </button>
                        </div>
                        <div className="muted" style={{ marginTop: 6 }}>
                          Selected instance: {selectedPermissionsInstance?.name ?? "None"}.
                        </div>
                        {selectedPermissionsChecklist.length > 0 ? (
                          <div className="preflightPermissionsList" style={{ marginTop: 8 }}>
                            {selectedPermissionsChecklist.map((perm) => (
                              <div key={`settings-perm:${perm.key}`} className="preflightPermissionRow">
                                <div className="preflightCheckMain">
                                  <div className="preflightCheckTitle">{perm.label}</div>
                                  <div className="preflightCheckMsg">{perm.detail}</div>
                                </div>
                                <span className={`chip ${permissionStatusChipClass(perm.status)}`}>
                                  {permissionStatusLabel(perm.status)}
                                </span>
                              </div>
                            ))}
                          </div>
                        ) : (
                          <div className="muted" style={{ marginTop: 8 }}>
                            No permission report yet for the selected instance. Click Re-check selected instance.
                          </div>
                        )}
                      </div>
                    </details>

                    <details className="settingsFoldSection" id="setting-anchor-global:github-api">
                      <summary className="settingsFoldSummary">
                        <span className="settingsFoldTitle">GitHub API authentication</span>
                        <span className="settingsFoldMeta">Token pool for higher API limits</span>
                      </summary>
                      <div className="settingsFoldBody">
                        <div className="settingSub">
                          Save personal access tokens to secure OS keychain storage for higher GitHub rate limits. Tokens are not stored in launcher settings files.
                        </div>
                        <div className="row settingsInlineBadges">
                          <span className="chip">Tokens are stored in Keychain</span>
                          {githubTokenPoolStatus ? (
                            <span className={`chip ${githubTokenPoolStatus.configured ? "" : "subtle"}`}>
                              {githubTokenPoolStatus.configured
                                ? `${githubTokenPoolStatus.total_tokens} token${githubTokenPoolStatus.total_tokens === 1 ? "" : "s"} configured`
                                : "Ready for first token"}
                            </span>
                          ) : null}
                        </div>
                        <textarea
                          className={`input githubTokenTextarea ${
                            !githubTokenPoolStatus?.configured && !githubTokenPoolDraft.trim() ? "ready" : ""
                          }`}
                          value={githubTokenPoolDraft}
                          onChange={(e) => setGithubTokenPoolDraft(e.target.value)}
                          placeholder="Paste one token per line (or comma/semicolon separated)"
                          rows={4}
                          style={{ marginTop: 8, resize: "vertical", minHeight: 96 }}
                        />
                        {!githubTokenPoolStatus?.configured && !githubTokenPoolDraft.trim() ? (
                          <div className="githubTokenReadyHint">
                            Paste tokens here and click Validate. We store them in secure Keychain storage only.
                          </div>
                        ) : null}
                        <div className="settingsActionGrid">
                          <button
                            className="btn primary"
                            onClick={() => void onSaveGithubTokenPool()}
                            disabled={githubTokenPoolBusy}
                          >
                            {githubTokenPoolBusy ? "Saving…" : "Save GitHub tokens"}
                          </button>
                          <button
                            className="btn"
                            onClick={() => void onValidateGithubTokenPool()}
                            disabled={githubTokenPoolBusy}
                          >
                            {githubTokenPoolBusy ? "Validating…" : "Validate"}
                          </button>
                          <button
                            className="btn"
                            onClick={() => void onClearGithubTokenPool()}
                            disabled={githubTokenPoolBusy}
                          >
                            Clear saved tokens
                          </button>
                          <button
                            className="btn subtle"
                            onClick={() => void refreshGithubTokenPoolStatus()}
                            disabled={githubTokenPoolBusy}
                          >
                            {githubTokenPoolBusy ? "Checking…" : "Refresh status"}
                          </button>
                        </div>
                        {githubTokenPoolStatus ? (
                          <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
                            <div className={githubTokenPoolStatus.keychain_available ? "noticeBox" : "warningBox"}>
                              {githubTokenPoolStatus.message}
                            </div>
                            <div className="muted">
                              Sources: env {githubTokenPoolStatus.env_tokens} · keychain {githubTokenPoolStatus.keychain_tokens}
                              {githubTokenPoolStatus.unauth_rate_limited
                                ? ` · unauth rate-limited${githubTokenPoolStatus.unauth_rate_limit_reset_at ? ` until ${githubTokenPoolStatus.unauth_rate_limit_reset_at}` : ""}`
                                : ""}
                            </div>
                          </div>
                        ) : null}
                        {githubTokenPoolNotice ? (
                          <div className={githubTokenPoolNoticeIsError ? "errorBox" : "noticeBox"} style={{ marginTop: 10 }}>
                            {githubTokenPoolNotice}
                          </div>
                        ) : null}
                      </div>
                    </details>
                  </div>
                </div>
              ) : null}
                </section>
              </div>
            </div>
          </div>

          {launcherErr ? <div className="errorBox" style={{ marginTop: 14 }}>{launcherErr}</div> : null}
        </div>
      );
    }

    if (route === "dev") {
      return (
        <div className="page">
          <div style={{ maxWidth: 980 }}>
            <div className="h1">Developer</div>
            <div className="p">
              Diagnostics and maintainer tools. This section is visible only when `MPM_DEV_MODE=1`.
            </div>

            <div className="card" style={{ padding: 16, marginTop: 14, borderRadius: 22 }}>
              <div style={{ fontWeight: 980, fontSize: 14 }}>CurseForge diagnostics</div>
              <div className="settingSub" style={{ marginTop: 8 }}>
                Release builds can use an injected key. In dev mode you can save a local key in secure keychain storage.
              </div>
              <input
                className="input"
                type="password"
                value={devCurseforgeKeyDraft}
                onChange={(e) => setDevCurseforgeKeyDraft(e.target.value)}
                placeholder="Paste CurseForge API key for local dev"
                style={{ marginTop: 8 }}
              />
              <div className="row">
                <button className="btn primary" onClick={() => void onSaveDevCurseforgeKey()} disabled={devCurseforgeKeyBusy}>
                  {devCurseforgeKeyBusy ? "Saving…" : "Save key"}
                </button>
                <button className="btn" onClick={() => void onClearDevCurseforgeKey()} disabled={devCurseforgeKeyBusy}>
                  Clear saved key
                </button>
                <button className="btn" onClick={() => void refreshCurseforgeApiStatus()} disabled={curseforgeApiBusy}>
                  {curseforgeApiBusy ? "Checking…" : "Check key status"}
                </button>
                <button
                  className="btn"
                  onClick={() =>
                    void openExternalLink("https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key")
                  }
                >
                  Get API key
                </button>
                <button className="btn" onClick={() => void openExternalLink("https://docs.curseforge.com/rest-api/")}>
                  API docs
                </button>
              </div>
              {curseforgeApiStatus ? (
                <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
                  <div className="chip">
                    {curseforgeApiStatus.validated
                      ? "Connected"
                      : curseforgeApiStatus.configured
                        ? "Configured but not validated"
                        : "Not configured"}
                  </div>
                  <div className={curseforgeApiStatus.validated ? "noticeBox" : "errorBox"}>
                    {curseforgeApiStatus.message}
                  </div>
                  {curseforgeApiStatus.configured ? (
                    <div className="muted">
                      Source: {curseforgeApiStatus.env_var ?? "Unknown"} · Key: {curseforgeApiStatus.key_hint ?? "hidden"}
                    </div>
                  ) : (
                    <div className="muted">
                      You can save a key above or use env var fallback: `export MPM_CURSEFORGE_API_KEY=\"your_key_here\"` then restart `tauri:dev`.
                    </div>
                  )}
                </div>
              ) : null}
              {devCurseforgeNotice ? (
                <div className={devCurseforgeNoticeIsError ? "errorBox" : "noticeBox"} style={{ marginTop: 10 }}>
                  {devCurseforgeNotice}
                </div>
              ) : null}
              {launcherErr ? <div className="errorBox" style={{ marginTop: 10 }}>{launcherErr}</div> : null}
            </div>

            <div className="card" style={{ padding: 16, marginTop: 12, borderRadius: 22 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 10 }}>
                <div style={{ fontWeight: 980, fontSize: 14 }}>Performance telemetry (local)</div>
                <button className="btn subtle" onClick={() => setPerfActions([])} disabled={perfActions.length === 0}>
                  Clear samples
                </button>
              </div>
              <div className="settingSub" style={{ marginTop: 8 }}>
                Lightweight UI timings for the latest actions, stored only on this device.
              </div>
              <div className="row" style={{ marginTop: 10, gap: 8, flexWrap: "wrap" }}>
                <span className="chip subtle">{perfActions.length} samples</span>
                <span className="chip subtle">
                  Avg {perfActionMetrics ? formatDurationMs(perfActionMetrics.avg_ms) : "n/a"}
                </span>
                <span className="chip subtle">
                  P95 {perfActionMetrics ? formatDurationMs(perfActionMetrics.p95_ms) : "n/a"}
                </span>
                <span className="chip">
                  Slowest {perfActionMetrics ? formatDurationMs(perfActionMetrics.slowest_ms) : "n/a"}
                </span>
              </div>
              {perfActions.length === 0 ? (
                <div className="muted" style={{ marginTop: 10 }}>
                  No samples yet. Run installs, update checks, or update-all actions to collect timings.
                </div>
              ) : (
                <div className="updatesList" style={{ marginTop: 10, maxHeight: 300, overflow: "auto" }}>
                  {perfActions.slice(0, 24).map((entry) => (
                    <div key={entry.id} className="updatesListRow">
                      <div className="updatesListName">
                        {formatPerfActionLabel(entry.name)}
                        <span className={`chip ${entry.status === "ok" ? "subtle" : ""}`} style={{ marginLeft: 8 }}>
                          {entry.status}
                        </span>
                      </div>
                      <div className="updatesListMeta">
                        {formatDurationMs(entry.duration_ms)} · {new Date(entry.finished_at).toLocaleTimeString()}
                        {entry.detail ? ` · ${entry.detail}` : ""}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      );
    }

    if (route === "updates") {
      const updatesContentScopeSummary = summarizeUpdateContentTypeSelection(
        updatesPageContentTypesNormalized
      );
      const updateScheduleStatus =
        updateCheckCadence === "off"
          ? "Paused"
          : scheduledUpdateBusy
            ? "Checking now"
            : "Scheduled";
      return (
        <div className="page">
          <div style={{ maxWidth: 1100 }}>
            <div className="h1">Updates available</div>
            <div className="p">
              Keep installed content current across your instances, with clearer rules for when checks run and when updates install automatically.
            </div>

            <div className="card updatesScreenSummaryCard">
              <div className="updatesScreenSummaryHeader">
                <div className="updatesScreenSummaryMain">
                  <div className="updatesScreenSummaryEyebrow">Update checks</div>
                  <div className="updatesScreenSummaryTitleRow">
                    <div className="settingTitle">Runs {updateCadenceLabel(updateCheckCadence).toLowerCase()}</div>
                    <span className={`chip ${updateScheduleStatus === "Paused" ? "" : "subtle"}`}>
                      {updateScheduleStatus}
                    </span>
                  </div>
                  <div className="settingSub updatesScreenSummaryLead">
                    Choose what gets checked, how often checks happen, and whether matching updates can install automatically.
                  </div>
                  <div className="updatesScreenSummaryMeta">
                    <span className="chip subtle">
                      Last run: {scheduledUpdateLastRunAt ? formatDateTime(scheduledUpdateLastRunAt, "Never") : "Never"}
                    </span>
                    <span className="chip subtle">
                      Next run: {updateCheckCadence === "off" ? "Disabled" : nextScheduledUpdateRunAt ? formatDateTime(nextScheduledUpdateRunAt, "Pending first check") : "Pending first check"}
                    </span>
                    <span className="chip subtle">Content: {updatesContentScopeSummary}</span>
                    <span className="chip subtle">
                      Auto install: {updateAutoApplyModeLabel(updateAutoApplyMode)}
                    </span>
                    <span className="chip subtle">When allowed: {updateApplyScopeLabel(updateApplyScope)}</span>
                    {scheduledUpdateBusy ? (
                      <span className="chip">
                        Progress {scheduledUpdateRunCompleted}/{scheduledUpdateRunTotal} · ETA{" "}
                        {formatEtaSeconds(scheduledUpdateRunEtaSeconds)}
                      </span>
                    ) : scheduledUpdateRunTotal > 0 && scheduledUpdateRunElapsedSeconds != null ? (
                      <span className="chip subtle">
                        Last run took {formatDurationMs(scheduledUpdateRunElapsedSeconds * 1000)}
                      </span>
                    ) : null}
                  </div>
                </div>
                <div className="updatesScreenSummaryActions">
                  {updatePrefsBusy ? (
                    <span className="chip">Saving…</span>
                  ) : updatePrefsSavedFlash ? (
                    <span className="chip subtle">Saved</span>
                  ) : null}
                  <button
                    className="btn primary"
                    onClick={() =>
                      void runScheduledUpdateChecks("manual", {
                        contentTypes: updatesPageBackendContentTypes,
                      })
                    }
                    disabled={scheduledUpdateBusy}
                  >
                    {scheduledUpdateBusy ? "Checking…" : "Run check now"}
                  </button>
                </div>
              </div>

              <div className="updatesScreenRuleGrid">
                <div className="updatesScreenRuleCard updatesScreenRuleCardWide">
                  <div className="updatesScreenRuleLabel">What to check</div>
                  <div className="updatesScreenRuleHelp">
                    Pick which content types should be included.
                  </div>
                  <MultiSelectDropdown
                    values={updatesPageUseAllContentTypes ? [] : updatesPageContentTypesNormalized}
                    placeholder="All content types"
                    groups={UPDATE_CONTENT_TYPE_GROUPS}
                    showSearch={false}
                    showGroupHeaders={false}
                    itemVariant="menu"
                    panelMinWidth={260}
                    panelEstimatedHeight={320}
                    clearLabel="Use all content types"
                    allSelectedLabel="All content types"
                    disabled={scheduledUpdateBusy}
                    onClear={() => setUpdatesPageContentTypes([])}
                    onChange={(next) => {
                      const normalized = next
                        .map((item) => normalizeUpdatableContentType(item))
                        .filter((item): item is UpdatableContentType => Boolean(item));
                      if (normalized.length === 0) {
                        setUpdatesPageContentTypes([]);
                        return;
                      }
                      const picked = new Set<UpdatableContentType>(normalized);
                      setUpdatesPageContentTypes(
                        ALL_UPDATABLE_CONTENT_TYPES.filter((item) => picked.has(item))
                      );
                    }}
                  />
                </div>
                <div className="updatesScreenRuleCard">
                  <div className="updatesScreenRuleLabel">How often to check</div>
                  <div className="updatesScreenRuleHelp">Set how often background checks run.</div>
                  <MenuSelect
                    value={updateCheckCadence}
                    labelPrefix="How often"
                    buttonLabel={updateCadenceLabel(updateCheckCadence)}
                    onChange={(v) => {
                      const next = normalizeUpdateCheckCadence(v);
                      setUpdateCheckCadence(next);
                      void persistUpdateSchedulerPrefs({ cadence: next });
                    }}
                    options={UPDATE_CADENCE_OPTIONS}
                  />
                </div>
                <div className="updatesScreenRuleCard">
                  <div className="updatesScreenRuleLabel">Automatic installs</div>
                  <div className="updatesScreenRuleHelp">Choose which instances can install updates for you.</div>
                  <MenuSelect
                    value={updateAutoApplyMode}
                    labelPrefix="Automatic installs"
                    buttonLabel={updateAutoApplyModeLabel(updateAutoApplyMode)}
                    onChange={(v) => {
                      const next = normalizeUpdateAutoApplyMode(v);
                      setUpdateAutoApplyMode(next);
                      void persistUpdateSchedulerPrefs({ autoApplyMode: next });
                    }}
                    options={UPDATE_AUTO_APPLY_MODE_OPTIONS}
                  />
                </div>
                <div className="updatesScreenRuleCard">
                  <div className="updatesScreenRuleLabel">When auto-install can run</div>
                  <div className="updatesScreenRuleHelp">Allow auto-install on scheduled checks, or also when you run one manually.</div>
                  <MenuSelect
                    value={updateApplyScope}
                    labelPrefix="When allowed"
                    buttonLabel={updateApplyScopeLabel(updateApplyScope)}
                    onChange={(v) => {
                      const next = normalizeUpdateApplyScope(v);
                      setUpdateApplyScope(next);
                      void persistUpdateSchedulerPrefs({ applyScope: next });
                    }}
                    options={UPDATE_APPLY_SCOPE_OPTIONS}
                  />
                </div>
              </div>

              <div className="updatesScreenStatsRow">
                <span className="chip">{updatesPageUpdatesAvailableTotal} update{updatesPageUpdatesAvailableTotal === 1 ? "" : "s"} waiting</span>
                <span className="chip subtle">{updatesPageInstancesWithUpdatesCount} instance{updatesPageInstancesWithUpdatesCount === 1 ? "" : "s"} need attention</span>
                <span className="chip subtle">{updatesPageVisibleEntries.length} instance{updatesPageVisibleEntries.length === 1 ? "" : "s"} checked</span>
              </div>
              {scheduledAppliedUpdatesRecent.length > 0 ? (
                <div className="updatesScreenAppliedSummary">
                  <div className="updatesCardTitle">Recent automatic installs</div>
                  <div className="updatesList">
                    {scheduledAppliedUpdatesRecent.map((entry) => (
                      <div key={`applied:${entry.instance_id}:${entry.applied_at}`} className="updatesListRow">
                        <div className="updatesListName">
                          {entry.instance_name}
                          <span className="chip subtle" style={{ marginLeft: 8 }}>
                            {entry.updated_entries} updated
                          </span>
                        </div>
                        <div className="updatesListMeta">
                          {formatDateTime(entry.applied_at, "Unknown time")}
                        </div>
                        <div className="updatesScreenAppliedNames">
                          {entry.updates.slice(0, 5).map((u) => (
                            <span
                              key={`applied-chip:${entry.instance_id}:${u.source}:${u.content_type}:${u.project_id}`}
                              className="chip subtle"
                            >
                              {u.name}
                            </span>
                          ))}
                          {entry.updates.length > 5 ? (
                            <span className="chip subtle">+{entry.updates.length - 5} more</span>
                          ) : null}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              ) : null}
              {scheduledUpdateErr ? <div className="errorBox" style={{ marginTop: 10 }}>{scheduledUpdateErr}</div> : null}
            </div>

            {updatesPageVisibleEntries.length === 0 ? (
              <div className="emptyState" style={{ marginTop: 12 }}>
                <div className="emptyTitle">No scheduled update checks yet</div>
                <div className="emptySub">
                  {updateCheckCadence === "off"
                    ? "Scheduled checks are disabled. Use the cadence controls above to enable them."
                    : `Run a check now or wait for the ${updateCadenceLabel(updateCheckCadence).toLowerCase()} schedule.`}
                </div>
              </div>
            ) : (
              <div className="updatesScreenList">
                {updatesPageVisibleEntries.map((row) => {
                  const appliedEntry = scheduledAppliedUpdatesByInstance[row.instance_id] ?? null;
                  return (
                  <div key={row.instance_id} className="card updatesScreenItemCard">
                    <div className="updatesScreenItemHead">
                      <div>
                        <div className="updatesScreenItemTitle">{row.instance_name}</div>
                        <div className="updatesScreenItemMetaRow">
                          <span className={`chip ${row.error ? "" : row.update_count === 0 ? "subtle" : ""}`}>
                            {row.error
                              ? "Check failed"
                              : row.update_count === 0
                                ? "Up to date"
                                : `${row.update_count} update${row.update_count === 1 ? "" : "s"} available`}
                          </span>
                          <span className="chip subtle">
                            Checked {new Date(row.checked_at).toLocaleString()}
                          </span>
                          <span className="chip subtle">
                            {row.checked_entries} entr{row.checked_entries === 1 ? "y" : "ies"} scanned
                          </span>
                        </div>
                      </div>
                      <div className="updatesScreenItemActions">
                        <button className="btn" onClick={() => openInstance(row.instance_id)}>
                          Open instance
                        </button>
                        <button
                          className="btn"
                          onClick={() => {
                            const inst = instances.find((item) => item.id === row.instance_id);
                            if (inst) {
                              void onCheckUpdates(inst, {
                                autoApplyIfConfigured: true,
                                syncSelectedInstanceMods: true,
                                contentTypes: updatesPageBackendContentTypes,
                              });
                            }
                          }}
                          disabled={updateBusy || scheduledUpdateBusy}
                        >
                          Recheck
                        </button>
                      </div>
                    </div>
                    {appliedEntry ? (
                      <div className="updatesScreenAppliedInstance">
                        <div className="updatesCardTitle">
                          Last automatic install
                          <span className="chip subtle" style={{ marginLeft: 8 }}>
                            {appliedEntry.updated_entries} updated
                          </span>
                        </div>
                        <div className="updatesListMeta">
                          {formatDateTime(appliedEntry.applied_at, "Unknown time")}
                        </div>
                        <div className="updatesScreenAppliedNames">
                          {appliedEntry.updates.slice(0, 6).map((u) => (
                            <span
                              key={`applied-instance-chip:${row.instance_id}:${u.source}:${u.content_type}:${u.project_id}`}
                              className="chip subtle"
                            >
                              {u.name}
                            </span>
                          ))}
                          {appliedEntry.updates.length > 6 ? (
                            <span className="chip subtle">+{appliedEntry.updates.length - 6} more</span>
                          ) : null}
                        </div>
                      </div>
                    ) : null}
                    {row.error ? (
                      <div className="errorBox" style={{ marginTop: 8 }}>{row.error}</div>
                    ) : row.update_count === 0 ? (
                      <div className="noticeBox" style={{ marginTop: 8 }}>
                        Everything checked here is already current.
                      </div>
                    ) : (
                      <div className="updatesList" style={{ marginTop: 8 }}>
                        <div className="updatesCardTitle">
                          {row.update_count} update{row.update_count === 1 ? "" : "s"} available
                        </div>
                        {row.updates.slice(0, 8).map((u) => (
                          <div key={`${row.instance_id}:${u.source}:${u.content_type}:${u.project_id}`} className="updatesListRow">
                            <div className="updatesListName">
                              {u.name}
                              <span className="chip subtle" style={{ marginLeft: 8 }}>{u.source}</span>
                              <span className="chip subtle" style={{ marginLeft: 6 }}>{u.content_type}</span>
                            </div>
                            <div className="updatesListMeta">
                              {u.current_version_number} → {u.latest_version_number}
                              {Array.isArray(u.compatibility_notes) && u.compatibility_notes.length > 0 ? (
                                <div className="muted" style={{ marginTop: 4 }}>
                                  {u.compatibility_notes[0]}
                                </div>
                              ) : null}
                            </div>
                          </div>
                        ))}
                        {row.updates.length > 8 ? (
                          <div className="muted">+{row.updates.length - 8} more</div>
                        ) : null}
                      </div>
                    )}
                  </div>
                )})}
              </div>
            )}
          </div>
        </div>
      );
    }

    if (route === "account") {
      const diag = accountDiagnostics;
      const account = diag?.account ?? selectedLauncherAccount;
      const uuid = diag?.minecraft_uuid ?? account?.id ?? null;
      const username = diag?.minecraft_username ?? account?.username ?? "No account connected";
      const skinTexture = toLocalIconSrc(diag?.skin_url) ?? "";
      const avatarSources = minecraftAvatarSources(uuid);
      const avatarSrc =
        toLocalIconSrc(
          avatarSources[Math.min(accountAvatarSourceIdx, Math.max(avatarSources.length - 1, 0))] ?? ""
        ) ?? "";
      const connectionRaw = String(diag?.status ?? (account ? "connected" : "not_connected")).toLowerCase();
      const tokenRaw = String(diag?.token_exchange_status ?? "idle").toLowerCase();
      const isDisconnected =
        !account ||
        connectionRaw.includes("not_connected") ||
        connectionRaw.includes("offline") ||
        connectionRaw.includes("idle");
      const isUnverified = !isDisconnected && (!diag?.entitlements_ok || tokenRaw.includes("error"));
      const accountStatusTone = isDisconnected ? "error" : isUnverified ? "warn" : "ok";
      const accountStatusLabel = isDisconnected
        ? "Not Connected"
        : isUnverified
          ? "Not verified"
          : "Connected / verified";
      const authBannerMessage = isDisconnected
        ? "Your launcher is not connected to a Microsoft account, so native launch and profile sync are unavailable."
        : isUnverified
          ? "Account connected, but entitlement verification is incomplete. Reconnect to refresh auth tokens."
          : diag?.last_error || accountDiagnosticsErr
            ? "Authentication returned an error. Reconnect to re-establish a healthy token chain."
            : null;
      const showAuthBrokenBanner = Boolean(authBannerMessage);

      return (
        <div className="accountPage">
          <div className="h1">Account</div>
          <div className="p">Connection status, launcher profile details, and skin setup in one calmer workspace.</div>

          <div className="accountHero card">
            <div className="accountAvatarWrap">
              {accountAvatarFromSkin ? (
                <img src={accountAvatarFromSkin} alt="Minecraft avatar" />
              ) : skinTexture ? (
                <span className="minecraftHeadPreview" role="img" aria-label="Minecraft avatar">
                  <img src={skinTexture} alt="" className="minecraftHeadLayer base" />
                  <img src={skinTexture} alt="" className="minecraftHeadLayer hat" />
                </span>
              ) : avatarSrc ? (
                <img
                  src={avatarSrc}
                  alt="Minecraft avatar"
                  onError={() => setAccountAvatarSourceIdx((i) => i + 1)}
                />
              ) : (
                <span>{username?.slice(0, 1)?.toUpperCase() ?? "?"}</span>
              )}
            </div>
            <div className="accountHeroMain">
              <div className="accountHeroEyebrow">Minecraft profile</div>
              <div className="accountHeroName">{username}</div>
              <div className="accountHeroMeta">
                <span className={`accountStatusBadge tone-${accountStatusTone}`}>
                  <span className="accountStatusDot" aria-hidden="true" />
                  {accountStatusLabel}
                </span>
                {diag?.entitlements_ok ? <span className="chip">Owns Minecraft</span> : null}
                {diag?.token_exchange_status ? <span className="chip subtle">{humanizeToken(diag.token_exchange_status)}</span> : null}
              </div>
              <div className="accountHeroSub">
                UUID: {uuid ?? "Not available"}
              </div>
              <div className="accountHeroLead">
                {isDisconnected
                  ? "Connect a Microsoft account to unlock native launch, entitlement checks, and profile sync."
                  : isUnverified
                    ? "The account is connected, but verification is still incomplete right now."
                    : "Your launcher account is connected and ready for native launch workflows."}
              </div>
              <div className="row" style={{ marginTop: 10 }}>
                <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                  {msLoginSessionId ? "Waiting for login…" : "Connect / Reconnect"}
                </button>
                {msLoginSessionId && msCodePrompt ? (
                  <button className="btn" onClick={() => setMsCodePromptVisible(true)}>
                    Show code
                  </button>
                ) : null}
                <button className="btn" onClick={() => refreshAccountDiagnostics().catch(() => null)} disabled={accountDiagnosticsBusy}>
                  {accountDiagnosticsBusy ? "Refreshing…" : "Refresh diagnostics"}
                </button>
              </div>
            </div>
          </div>
          {showAuthBrokenBanner ? (
            <div className="card accountAuthBanner">
              <div className="accountAuthBannerMain">
                <div className="accountAuthBannerTitle">Authentication needs attention</div>
                <div className="accountAuthBannerText">{authBannerMessage}</div>
              </div>
              <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                {msLoginSessionId ? "Waiting for login…" : "Reconnect"}
              </button>
            </div>
          ) : null}

          <div className="accountSummaryStrip">
            <div className="accountSummaryCard">
              <div className="accountSummaryLabel">Launch mode</div>
              <div className="accountSummaryValue">{humanizeToken(launcherSettings?.default_launch_method ?? "native")}</div>
            </div>
            <div className="accountSummaryCard">
              <div className="accountSummaryLabel">Update checks</div>
              <div className="accountSummaryValue">{updateCadenceLabel(updateCheckCadence)}</div>
            </div>
            <div className="accountSummaryCard">
              <div className="accountSummaryLabel">Saved skins</div>
              <div className="accountSummaryValue">{savedSkinOptions.length}</div>
            </div>
            <div className="accountSummaryCard">
              <div className="accountSummaryLabel">Connected accounts</div>
              <div className="accountSummaryValue">{launcherAccounts.length}</div>
            </div>
          </div>

          <div className="accountGrid">
            <div className="card accountCard accountCardWide">
              <div className="settingTitle">Profile overview</div>
              <div className="settingSub">The account you are using right now, plus the launcher and skin defaults attached to it.</div>
              <div className="accountProfileSplit">
                <div className="accountSectionBlock">
                  <div className="accountSectionTitle">Launcher defaults</div>
                  <div className="accountDiagList">
                    <div className="accountDiagRow">
                      <span>Default launch mode</span>
                      <strong>{humanizeToken(launcherSettings?.default_launch_method ?? "native")}</strong>
                    </div>
                    <div className="accountDiagRow">
                      <span>Update checks</span>
                      <strong>{updateCadenceLabel(updateCheckCadence)}</strong>
                    </div>
                    <div className="accountDiagRow">
                      <span>Automatic installs</span>
                      <strong>{updateAutoApplyModeLabel(updateAutoApplyMode)}</strong>
                    </div>
                    <div className="accountDiagRow">
                      <span>Current skin</span>
                      <strong>{selectedAccountSkin?.label ?? "None"}</strong>
                    </div>
                    <div className="accountDiagRow">
                      <span>Current cape</span>
                      <strong>{selectedAccountCape?.label ?? "No cape"}</strong>
                    </div>
                  </div>
                </div>
                <div className="accountSectionBlock">
                  <div className="accountSectionTitle">Skin setup</div>
                  <div className="accountDiagList">
                    <div className="accountDiagRow">
                      <span>Saved skins</span>
                      <strong>{savedSkinOptions.length}</strong>
                    </div>
                    <div className="accountDiagRow">
                      <span>Default skins</span>
                      <strong>{defaultSkinOptions.length}</strong>
                    </div>
                    <div className="accountDiagRow">
                      <span>Cape options</span>
                      <strong>{capeOptions.length}</strong>
                    </div>
                    <div className="accountDiagRow">
                      <span>Last diagnostics refresh</span>
                      <strong>{diag?.last_refreshed_at ? new Date(diag.last_refreshed_at).toLocaleString() : "Never"}</strong>
                    </div>
                  </div>
                  <div className="accountSectionActions">
                    <button className="btn" onClick={() => setRoute("skins")}>
                      Open skin studio
                    </button>
                    <label className="toggleRow accountInlineToggle">
                      <input
                        type="checkbox"
                        checked={skinPreviewEnabled}
                        onChange={(event) => setSkinPreviewEnabled(event.target.checked)}
                      />
                      <span className="togglePill" />
                      <span>Use 3D preview in Skin Studio</span>
                    </label>
                  </div>
                </div>
              </div>
            </div>

            <div className="card accountCard accountCardWide">
              <div className="settingTitle">Diagnostics</div>
              <div className="settingSub">Connection health and token state for native launch. Network errors can make these checks look worse than the account really is.</div>
              <div className="accountDiagList">
                <div className="accountDiagRow">
                  <span>Connection</span>
                  <strong className={`accountStatusText tone-${accountStatusTone}`}>{accountStatusLabel}</strong>
                </div>
                <div className="accountDiagRow">
                  <span>Entitlements</span>
                  <strong className={`accountStatusText tone-${diag?.entitlements_ok ? "ok" : "warn"}`}>
                    {diag?.entitlements_ok ? "Verified" : "Not verified"}
                  </strong>
                </div>
                <div className="accountDiagRow">
                  <span>Token status</span>
                  <strong>{humanizeToken(diag?.token_exchange_status ?? "idle")}</strong>
                </div>
                <div className="accountDiagRow">
                  <span>Client ID source</span>
                  <strong>{humanizeToken(diag?.client_id_source ?? "unknown")}</strong>
                </div>
                <div className="accountDiagRow">
                  <span>Last refresh</span>
                  <strong>{diag?.last_refreshed_at ? formatDateTime(diag.last_refreshed_at, "Never") : "Never"}</strong>
                </div>
                {diag?.last_error ? (
                  <div className="errorBox" style={{ marginTop: 8 }}>{diag.last_error}</div>
                ) : null}
                {accountDiagnosticsErr ? (
                  <div className="errorBox" style={{ marginTop: 8 }}>{accountDiagnosticsErr}</div>
                ) : null}
              </div>
            </div>

            <div className="card accountCard">
              <div className="settingTitle">Accounts</div>
              <div className="settingSub">Choose which connected account should be used for native launch.</div>
              <div className="accountAccountsList">
                {launcherAccounts.length === 0 ? (
                  <div className="muted">No connected accounts.</div>
                ) : (
                  launcherAccounts.map((acct) => {
                    const selectedAccount = selectedLauncherAccountId === acct.id;
                    return (
                      <div key={acct.id} className="accountAccountRow">
                        <div className="accountAccountInfo">
                          <div className="accountAccountName">{acct.username}</div>
                          <div className="accountAccountId">{acct.id}</div>
                        </div>
                        <div className="row" style={{ marginTop: 0 }}>
                          <button
                            className={`btn stateful ${selectedAccount ? "active" : ""}`}
                            onClick={() => onSelectAccount(acct.id)}
                            disabled={launcherBusy}
                          >
                            {selectedAccount ? "Selected" : "Use"}
                          </button>
                          <button
                            className="btn accountDisconnectBtn"
                            onClick={() => onLogoutAccount(acct.id)}
                            disabled={launcherBusy}
                          >
                            Disconnect
                          </button>
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </div>

            <div className="card accountCard">
              <div className="settingTitle">Profile assets</div>
              <div className="settingSub">Skins and capes currently returned by the Minecraft profile API.</div>
              <div className="accountDiagList">
                <div className="accountDiagRow">
                  <span>Skins</span>
                  <strong>{diag?.skins?.length ?? 0}</strong>
                </div>
                <div className="accountDiagRow">
                  <span>Capes</span>
                  <strong>{diag?.cape_count ?? 0}</strong>
                </div>
                <div className="accountAssetList">
                  {(diag?.skins ?? []).slice(0, 6).map((skin) => (
                    <div key={`${skin.id}:${skin.url}`} className="accountAssetRow">
                      <span>{skin.variant ?? "Skin"}</span>
                      <a href={skin.url} target="_blank" rel="noreferrer">Open</a>
                    </div>
                  ))}
                  {(diag?.capes ?? []).slice(0, 6).map((cape) => (
                    <div key={`${cape.id}:${cape.url}`} className="accountAssetRow">
                      <span>{cape.alias ?? "Cape"}</span>
                      <a href={cape.url} target="_blank" rel="noreferrer">Open</a>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      );
    }

    if (route === "modpacks") {
      return (
        <div className="creatorStudioRoute" style={{ maxWidth: 1380 }}>
          <div className="creatorStudioIntro card">
            <div className="creatorStudioIntroCopy">
              <div className="creatorStudioEyebrow">Creator Studio</div>
              <div className="h1">Build packs and edit live config.</div>
              <div className="p">
                Creator handles layered modpack assembly and apply previews. Config Editor works directly against instance and world files with rollback safety.
              </div>
              <div className="creatorStudioTabs">
                <SegmentedControl
                  value={modpacksStudioTab === "config" ? "config" : "creator"}
                  onChange={(v) => setModpacksStudioTab((v as any) ?? "creator")}
                  options={[
                    { value: "creator", label: "Creator" },
                    { value: "config", label: "Config Editor" },
                  ]}
                  variant="scroll"
                />
              </div>
            </div>
          </div>

          {modpacksStudioTab === "config" ? (
            <div className="creatorStudioPanelWrap">
              <ModpacksConfigEditor
                instances={instances}
                selectedInstanceId={selectedId}
                onSelectInstance={setSelectedId}
                onManageInstances={() => setRoute("library")}
                runningInstanceIds={runningInstances.map((run) => run.instance_id)}
              />
            </div>
          ) : (
            <div className="creatorStudioPanelWrap">
              <ModpackMaker
                instances={instances}
                selectedInstanceId={selectedId}
                autoIdentifyLocalJarsEnabled={Boolean(launcherSettings?.auto_identify_local_jars)}
                onSelectInstance={setSelectedId}
                onOpenDiscover={(context) => {
                  setDiscoverAddContext(context ?? null);
                  setDiscoverAddTrayExpanded(true);
                  setRoute("discover");
                }}
                isDevMode={isDevMode}
                onNotice={(message) => setInstallNotice(message)}
                onError={(message) => setError(message)}
              />
            </div>
          )}
        </div>
      );
    }

    if (route === "discover") {
      const selectedInst = instances.find((i) => i.id === selectedId) ?? null;
      const discoverIncludesGithub = effectiveDiscoverSources.includes("github");
      const discoverIncludesCurseforge = effectiveDiscoverSources.includes("curseforge");
      const discoverOnlyCurseforge =
        effectiveDiscoverSources.length === 1 && effectiveDiscoverSources[0] === "curseforge";
      const discoverFilterSupportNotes: string[] = [];
      if (discoverIncludesGithub) {
        if (discoverContentType !== "mods") {
          discoverFilterSupportNotes.push("GitHub source currently supports mods only.");
        } else if (
          filterLoaders.length > 0 ||
          Boolean(filterVersion) ||
          filterCategories.length > 0
        ) {
          discoverFilterSupportNotes.push(
            "GitHub source filters are best-effort: loader/version/category checks rely on repository topics and release asset naming."
          );
        }
      }
      if (discoverIncludesCurseforge) {
        if (discoverContentType === "mods" && discoverOnlyCurseforge) {
          discoverFilterSupportNotes.push(
            "CurseForge-only searches currently ignore the loader filter."
          );
        }
        if (filterCategories.length > 0) {
          discoverFilterSupportNotes.push(
            "CurseForge category matching is best-effort because provider category vocabularies differ."
          );
        }
      }
      if (
        effectiveDiscoverSources.length > 1 &&
        (filterLoaders.length > 0 || Boolean(filterVersion) || filterCategories.length > 0)
      ) {
        discoverFilterSupportNotes.push(
          "Multi-source search combines provider results; filter precision varies by provider."
        );
      }
      const discoverFilterSupportNotice = discoverFilterSupportNotes.length
        ? discoverFilterSupportNotes.join(" ")
        : null;
      const activeDiscoverFilterCount =
        (filterVersion ? 1 : 0) +
        (filterLoaders.length > 0 ? 1 : 0) +
        (filterCategories.length > 0 ? 1 : 0) +
        (discoverAllVersions ? 1 : 0);
      const discoverPlaceholder =
        discoverContentType === "shaderpacks"
          ? "Search shaderpacks…"
          : discoverContentType === "resourcepacks"
            ? "Search resourcepacks…"
            : discoverContentType === "datapacks"
            ? "Search datapacks…"
            : discoverContentType === "modpacks"
              ? "Search modpacks…"
              : "Search mods…";
      const discoverContentTypeLabel =
        DISCOVER_CONTENT_OPTIONS.find((option) => option.value === discoverContentType)?.label ?? discoverContentType;

      return (
        <div className="discoverPage" style={{ maxWidth: 1400 }}>
          <div className="h1">Discover content</div>
          <div className="p">Search Modrinth, CurseForge, or GitHub and install directly into instances.</div>
          {discoverAddContext ? (
            <div className={`discoverAddTray${discoverAddTraySticky ? " discoverAddTraySticky" : ""}`}>
              <div className="discoverAddTrayHeader">
                <div>
                  <div className="discoverAddTrayTitle">
                    Adding to {discoverAddContext.modpackName}
                    {discoverAddContext.layerName ? ` / ${discoverAddContext.layerName}` : ""}
                  </div>
                  <div className="discoverAddTraySub">
                    Use <strong>Add to modpack</strong> on any result. This tray tracks what you added in this session.
                  </div>
                </div>
                <div className="discoverAddTrayActions">
                  <button
                    className="btn"
                    onClick={() => {
                      setRoute("modpacks");
                      setModpacksStudioTab("creator");
                    }}
                  >
                    Open Creator Studio
                  </button>
                  <button
                    className="btn"
                    onClick={() => setDiscoverAddTrayExpanded((prev) => !prev)}
                    title="Show or hide added items."
                  >
                    {discoverAddTrayExpanded ? "Hide additions" : "Show additions"}
                  </button>
                  <button
                    className="btn"
                    onClick={() => setDiscoverAddTraySticky((prev) => !prev)}
                    title="Keep this tray pinned while you scroll results."
                  >
                    {discoverAddTraySticky ? "Unpin tray" : "Pin tray"}
                  </button>
                  <button
                    className="btn"
                    onClick={() => {
                      setDiscoverAddContext(null);
                      setDiscoverAddTrayItems([]);
                    }}
                  >
                    Clear add target
                  </button>
                </div>
              </div>

              <div className="discoverAddTrayStats">
                <span className="chip subtle">Added this session: {discoverAddTrayItems.length}</span>
                <span className="chip subtle">Target layer: {discoverAddContext.layerName ?? "Default"}</span>
                {discoverAddTrayItems[0] ? (
                  <span className="chip subtle">Last added: {formatDateTime(discoverAddTrayItems[0].addedAt, "just now")}</span>
                ) : (
                  <span className="chip subtle">No items added yet</span>
                )}
              </div>

              {discoverAddTrayExpanded ? (
                <div className="discoverAddTrayList">
                  {discoverAddTrayItems.length === 0 ? (
                    <div className="discoverAddTrayEmpty">
                      Add content from Discover results and it will appear here.
                    </div>
                  ) : (
                    discoverAddTrayItems.slice(0, 8).map((item) => (
                      <div key={item.id} className="discoverAddTrayItem">
                        <div className="discoverAddTrayItemMain">
                          <div className="discoverAddTrayItemTitle">{item.title}</div>
                          <div className="discoverAddTrayItemMeta">
                            {item.projectId} · {item.source} · {item.contentType} · {item.layerName}
                          </div>
                        </div>
                        <span className="chip subtle">{formatDateTime(item.addedAt, "just now")}</span>
                      </div>
                    ))
                  )}
                </div>
              ) : null}

              {discoverAddTrayItems.length > 8 ? (
                <div className="discoverAddTrayOverflow muted">
                  Showing latest 8 of {discoverAddTrayItems.length} items.
                </div>
              ) : null}
            </div>
          ) : null}

          <div className="discoverWorkspace">
            <div className="discoverWorkspaceTop">
              <div>
                <div className="discoverWorkspaceEyebrow">Search setup</div>
                <div className="discoverWorkspaceTitle">Find content by type, source, and compatibility.</div>
              </div>
              <div className="discoverWorkspaceStats">
                <span className="chip subtle">{activeDiscoverFilterCount} active filter{activeDiscoverFilterCount === 1 ? "" : "s"}</span>
                <span className="chip subtle">{totalHits} result{totalHits === 1 ? "" : "s"}</span>
              </div>
            </div>

            <div className="topRow" style={{ marginBottom: 8 }}>
              <SegmentedControl
                value={discoverContentType}
                onChange={(v) => {
                  setDiscoverContentType((v as DiscoverContentType) ?? "mods");
                  setFilterLoaders([]);
                  setOffset(0);
                }}
                options={DISCOVER_CONTENT_OPTIONS}
                variant="scroll"
              />
            </div>

            <div className="topRow discoverSearchRow">
              <div className="searchGrow">
                <input
                  className="input"
                  value={q}
                  onChange={(e) => {
                    setQ(e.target.value);
                    if (discoverErr) setDiscoverErr(null);
                  }}
                  placeholder={discoverPlaceholder}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") runSearch(0);
                  }}
                />
              </div>

              <div className="discoverSearchActions">
                <MenuSelect
                  value={index}
                  labelPrefix="Sort"
                  options={DISCOVER_SORT_OPTIONS.map((o) => ({ value: o.value, label: o.label }))}
                  onChange={(v) => {
                    setIndex(v as any);
                    setOffset(0);
                  }}
                />

                <MenuSelect
                  value={String(limit)}
                  labelPrefix="View"
                  options={DISCOVER_VIEW_OPTIONS}
                  align="end"
                  onChange={(v) => {
                    setLimit(parseInt(v, 10));
                    setOffset(0);
                  }}
                />

                <div className="filterCtrl filterCtrlSource">
                  <MultiSelectDropdown
                    values={discoverSources}
                    placeholder="Sources: All"
                    allSelectedLabel="Sources: All"
                    groups={DISCOVER_SOURCE_GROUPS}
                    showSearch={false}
                    showGroupHeaders={false}
                    itemVariant="menu"
                    clearLabel="All sources"
                    panelMinWidth={220}
                    panelEstimatedHeight={176}
                    onChange={(values) => {
                      const next = normalizeDiscoverProviderSources(values);
                      setDiscoverSources(next.length > 0 ? next : [...DISCOVER_PROVIDER_SOURCES]);
                      setOffset(0);
                    }}
                    onClear={() => {
                      setDiscoverSources([...DISCOVER_PROVIDER_SOURCES]);
                      setOffset(0);
                    }}
                  />
                </div>

                <button className="btn primary" onClick={() => runSearch(0)} disabled={discoverBusy}>
                  {discoverBusy ? "Searching…" : "Search"}
                </button>
              </div>
            </div>

            <div className="topRow discoverFilterRow">
              <div className="discoverFiltersRight">
                <div className="filterCtrl filterCtrlVersion">
                  <Dropdown
                    value={filterVersion}
                    placeholder="Game version: Any"
                    groups={groupedDiscoverVersions}
                    includeAny
                    onPick={(v) => {
                      setFilterVersion(v);
                      setOffset(0);
                    }}
                  />
                </div>

                <div className="filterCtrl filterCtrlLoader">
                  <MultiSelectDropdown
                    values={filterLoaders}
                    placeholder="Loaders: Any"
                    groups={DISCOVER_LOADER_GROUPS}
                    showSearch={false}
                    showGroupHeaders={false}
                    disabled={discoverContentType !== "mods" || discoverOnlyCurseforge}
                    onChange={(v) => {
                      if (discoverContentType !== "mods") return;
                      if (discoverOnlyCurseforge) return;
                      setFilterLoaders(v);
                      setOffset(0);
                    }}
                  />
                </div>

                <div className="filterCtrl filterCtrlCategory">
                  <MultiSelectDropdown
                    values={filterCategories}
                    placeholder="Categories: Any"
                    groups={MOD_CATEGORY_GROUPS}
                    searchPlaceholder="Search categories…"
                    onChange={(v) => {
                      setFilterCategories(v);
                      setOffset(0);
                    }}
                  />
                </div>

                <label className="checkboxRow discoverCheckboxRow">
                  <span
                    className={`checkbox ${discoverAllVersions ? "checked" : ""}`}
                    onClick={() => setDiscoverAllVersions(!discoverAllVersions)}
                    role="checkbox"
                    aria-checked={discoverAllVersions}
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        setDiscoverAllVersions(!discoverAllVersions);
                      }
                    }}
                  >
                    {discoverAllVersions ? "✓" : ""}
                  </span>
                  Show all versions
                </label>

                <button
                  className="btn discoverClearBtn"
                  onClick={() => {
                    setFilterVersion(null);
                    setFilterLoaders([]);
                    setFilterCategories([]);
                    setOffset(0);
                  }}
                  disabled={!filterVersion && filterLoaders.length === 0 && filterCategories.length === 0}
                >
                  Clear filters
                </button>
              </div>
            </div>

            {discoverFilterSupportNotice ? (
              <div className="warningBox" style={{ marginTop: 8 }}>{discoverFilterSupportNotice}</div>
            ) : null}
          </div>

          {discoverErr ? <div className="errorBox">{discoverErr}</div> : null}

          <div className="discoverResultsHeader">
            <div className="discoverResultsInfo">
              <div className="discoverResultsTitleRow">
                <div className="discoverResultsTitle">Results</div>
                <span className="chip subtle">{discoverContentTypeLabel}</span>
                <span className="chip subtle">
                  {effectiveDiscoverSources.length} source{effectiveDiscoverSources.length === 1 ? "" : "s"}
                </span>
              </div>
              <div className="discoverResultsSub">
                {discoverBusy
                  ? "Refreshing matches..."
                  : `Showing ${hits.length} of ${totalHits} result${totalHits === 1 ? "" : "s"} on page ${page} of ${pages}.`}
              </div>
            </div>
            <div className="discoverResultsPager">
              <div className="pager pagerTop">
                <button
                  className="btn"
                  onClick={() => runSearch(Math.max(0, offset - limit))}
                  disabled={discoverBusy || offset === 0}
                >
                  ← Prev
                </button>
                <div className="pagerLabel">
                  Page {page} / {pages}
                </div>
                <button
                  className="btn"
                  onClick={() => runSearch(Math.min((pages - 1) * limit, offset + limit))}
                  disabled={discoverBusy || offset + limit >= totalHits}
                >
                  Next →
                </button>
              </div>
            </div>
          </div>

          <div className="resultsGrid">
            {hits.map((h) => (
              <div
                className="resultCard"
                key={`${h.source}:${h.project_id}`}
                onClick={() => {
                    if (h.source === "modrinth") {
                      openProject(h.project_id, (h.content_type as DiscoverContentType) ?? discoverContentType);
                      return;
                    }
                    if (h.source === "curseforge") {
                      openCurseforgeProject(h.project_id, (h.content_type as DiscoverContentType) ?? discoverContentType);
                      return;
                    }
                    if (h.source === "github") {
                      void openGithubProject(h, (h.content_type as DiscoverContentType) ?? discoverContentType);
                      return;
                    }
                    if (h.external_url?.trim()) {
                      void openExternalLink(h.external_url.trim());
                    }
                }}
              >
                <div className="resultIcon">
                  <RemoteImage src={h.icon_url} alt={`${h.title} icon`} fallback={<div>⬚</div>} />
                </div>

                <div className="resultBody">
                  <div className="resultTitleRow">
                    <div className="resultTitle">{h.title}</div>
                  </div>
                  <div className="resultDesc">{h.description}</div>
                  <div className="resultMetaRow">
                    <span className="chip subtle">{providerSourceLabel(h.source)}</span>
                    <span>by {h.author}</span>
                    <span>↓ {formatCompact(h.downloads)}</span>
                    <span>♥ {formatCompact(h.follows)}</span>
                    {h.source === "github" && githubInstallStateChipLabel(h.install_state) ? (
                      <span className={githubStatusChipClass("installability", h.install_state)}>
                        {githubInstallStateChipLabel(h.install_state)}
                      </span>
                    ) : null}
                    {h.categories?.slice(0, 3)?.map((c) => (
                      <span key={c} className="chip">
                        {c}
                      </span>
                    ))}
                  </div>
                  {h.source === "github" && githubInstallSummary(h) ? (
                    <div className="muted" style={{ marginTop: 8, fontSize: 12.5 }}>
                      {githubInstallSummary(h)}
                    </div>
                  ) : null}
                </div>

                <div
                  className="resultActions"
                  style={{ alignSelf: "center" }}
                  onClick={(e) => e.stopPropagation()}
                >
                  <button
                    className="btn ghost"
                    onClick={() => {
                      if (h.source === "modrinth") {
                        openProject(h.project_id, (h.content_type as DiscoverContentType) ?? discoverContentType);
                        return;
                      }
                      if (h.source === "curseforge") {
                        openCurseforgeProject(h.project_id, (h.content_type as DiscoverContentType) ?? discoverContentType);
                        return;
                      }
                      if (h.source === "github") {
                        void openGithubProject(h, (h.content_type as DiscoverContentType) ?? discoverContentType);
                        return;
                      }
                      if (h.external_url?.trim()) {
                        void openExternalLink(h.external_url.trim());
                      }
                    }}
                  >
                    View
                  </button>
                  <button
                    className="btn"
                    onClick={() =>
                      openAddToModpack({
                        source: normalizeDiscoverSource(h.source),
                        projectId: h.project_id,
                        title: h.title,
                        contentType:
                          (h.content_type as DiscoverContentType) === "modpacks"
                            ? "modpacks"
                            : ((h.content_type as DiscoverContentType) ?? discoverContentType),
                        slug: h.slug ?? null,
                        iconUrl: h.icon_url,
                        description: h.description,
                      }, discoverAddContext ? { modpackId: discoverAddContext.modpackId, layerId: discoverAddContext.layerId ?? null } : undefined)
                    }
                    title={
                      h.content_type === "modpacks"
                        ? "Import modpacks as template layers from Creator Studio"
                        : "Add to a Modpack Maker layer"
                    }
                    disabled={h.content_type === "modpacks"}
                  >
                    Add to modpack
                  </button>
                  <button
                    className="btn primary installAction"
                    onClick={() =>
                      openInstall({
                        source: normalizeDiscoverSource(h.source),
                        projectId: h.project_id,
                        title: h.title,
                        contentType:
                          (h.content_type as DiscoverContentType) === "modpacks"
                            ? "modpacks"
                            : ((h.content_type as DiscoverContentType) ?? discoverContentType),
                        iconUrl: h.icon_url,
                        description: h.description,
                        installSupported: githubResultInstallSupported(h),
                        installNote: githubResultInstallNote(h),
                      })
                    }
                    title={
                      h.content_type === "modpacks"
                        ? "Modpacks are imported as templates"
                        : !githubResultInstallSupported(h)
                          ? githubResultInstallNote(h) ?? "This provider result cannot be installed directly yet."
                          : "Install to instance"
                    }
                    disabled={h.content_type === "modpacks" || !githubResultInstallSupported(h)}
                  >
                    <Icon name="download" /> {h.content_type === "modpacks" ? "Template only" : "Install"}
                  </button>
                </div>
              </div>
            ))}

            {hits.length === 0 && !discoverBusy ? (
              <div className="card" style={{ padding: 16, borderRadius: 22, color: "var(--muted)" }}>
                No results.
              </div>
            ) : null}
          </div>

          <div className="pager">
            <button
              className="btn"
              onClick={() => runSearch(Math.max(0, offset - limit))}
              disabled={discoverBusy || offset === 0}
            >
              ← Prev
            </button>
            <div className="pagerLabel">
              Page {page} / {pages}
            </div>
            <button
              className="btn"
              onClick={() => runSearch(Math.min((pages - 1) * limit, offset + limit))}
              disabled={discoverBusy || offset + limit >= totalHits}
            >
              Next →
            </button>
          </div>
        </div>
      );
    }


    if (route === "instance") {
      const inst = instances.find((i) => i.id === selectedId);

      if (!inst) {
        return (
          <div className="page">
            <div className="card" style={{ padding: 14 }}>
              <div className="h2">Instance not found</div>
              <div className="muted" style={{ marginTop: 6 }}>
                This instance may have been deleted or not loaded yet.
              </div>
              <div style={{ marginTop: 12 }}>
                <button className="btn" onClick={() => setRoute("library")}>
                  Back to Library
                </button>
              </div>
            </div>
          </div>
        );
      }

      const loaderLabel =
        inst.loader === "neoforge"
          ? "NeoForge"
          : inst.loader === "fabric"
            ? "Fabric"
            : inst.loader === "forge"
              ? "Forge"
              : inst.loader === "quilt"
                ? "Quilt"
                : "Vanilla";
      const instSettings = normalizeInstanceSettings(inst.settings);
      const instanceDiskUsageBytes = Number(instanceDiskUsageById[inst.id] ?? 0);
      const instanceLastRunMeta = instanceLastRunMetadataById[inst.id] ?? null;
      const instancePlaytime = instancePlaytimeById[inst.id] ?? null;
      const instanceLastRunReport = instanceRunReportById[inst.id] ?? null;
      const instancePreflightReport = preflightReportByInstance[inst.id] ?? null;
      const instancePermissionChecklist: LaunchPermissionChecklistItem[] =
        instancePreflightReport?.permissions ?? [];
      const setupNeededPermissions = instancePermissionChecklist.filter((item) =>
        micPermissionNeedsAction(item)
      );
      const unavailableCheckPermissions = instancePermissionChecklist.filter((item) =>
        micPermissionCheckUnavailable(item)
      );
      const grantedPermissions = instancePermissionChecklist.filter(
        (item) => String(item.status ?? "").trim().toLowerCase() === "granted"
      );
      const notRequiredPermissions = instancePermissionChecklist.filter(
        (item) => String(item.status ?? "").trim().toLowerCase() === "not_required"
      );
      const permissionStatusTagLabel =
        permissionChecklistBusyByInstance[inst.id]
          ? "Perms checking…"
          : setupNeededPermissions.length > 0
            ? "Setup needed"
            : unavailableCheckPermissions.length > 0
              ? unavailableCheckPermissions.some(
                    (item) => String(item.key ?? "").trim().toLowerCase() === "microphone"
                  )
                ? "Mic manual check"
                : "Manual check"
            : grantedPermissions.length > 0
              ? "Perms ready"
              : notRequiredPermissions.length === instancePermissionChecklist.length &&
                  instancePermissionChecklist.length > 0
                ? "No perms needed"
                : "Perms unknown";
      const permissionStatusTagTitle =
        setupNeededPermissions.length > 0
          ? `Needs setup: ${setupNeededPermissions
              .slice(0, 3)
              .map((item) => item.label)
              .join(", ")}. Manage details in Settings > Advanced > Launch permissions.`
          : unavailableCheckPermissions.length > 0
            ? `${unavailableCheckPermissions
                .slice(0, 3)
                .map((item) => item.label)
                .join(", ")} auto-check is unavailable on this setup. Open Settings > Advanced > Launch permissions to re-check or open OS privacy settings.`
          : grantedPermissions.length > 0
            ? `Permissions ready: ${grantedPermissions
                .slice(0, 3)
                .map((item) => item.label)
                .join(", ")}.`
            : notRequiredPermissions.length === instancePermissionChecklist.length &&
                instancePermissionChecklist.length > 0
              ? "No required launch permissions detected for this instance."
              : "Permission status is not available yet.";
      const permissionStatusChipIsActionable = Boolean(
        setupNeededPermissions.length > 0 || unavailableCheckPermissions.length > 0
      );
      const instanceLastLaunchAt = instanceLastRunMeta?.lastLaunchAt ?? null;
      const instanceLastExitKindRaw = String(instanceLastRunMeta?.lastExitKind ?? "").trim().toLowerCase();
      const instanceLastExitAt = instanceLastRunMeta?.lastExitAt ?? null;
      const instancePlayedSeconds = Math.max(0, Number(instancePlaytime?.totalSeconds ?? 0));
      const instancePlayedLabel = instancePlayedSeconds > 0
        ? formatDurationMs(instancePlayedSeconds * 1000)
        : "0s";
      const showNativeOnlyPlaytimeHint = String(instancePlaytime?.trackingScope ?? "").toLowerCase() === "native_only";
      const autoBackupsEnabled =
        Number(instSettings.world_backup_interval_minutes ?? 0) > 0 &&
        Number(instSettings.world_backup_retention_count ?? 0) > 0;
      let latestInstanceBackupAt: string | null = null;
      for (const world of instanceWorlds) {
        const candidate = String(world.latest_backup_at ?? "").trim();
        if (!candidate) continue;
        if (!latestInstanceBackupAt) {
          latestInstanceBackupAt = candidate;
          continue;
        }
        const currentTs = parseDateLike(latestInstanceBackupAt)?.getTime() ?? 0;
        const candidateTs = parseDateLike(candidate)?.getTime() ?? 0;
        if (candidateTs >= currentTs) {
          latestInstanceBackupAt = candidate;
        }
      }
      const instanceRunStatusLabel =
        instanceLastExitKindRaw === "success"
          ? "Successful launch"
          : instanceLastExitKindRaw === "crashed"
            ? "Crashed"
            : "Unknown";
      const requiredJavaMajor = requiredJavaMajorForMcVersion(inst.mc_version);
      const launchHooksDraft = instanceLaunchHooksById[inst.id] ?? defaultLaunchHooksDraft();
      const setLaunchHooksDraft = (patch: Partial<InstanceLaunchHooksDraft>) => {
        setInstanceLaunchHooksById((prev) => ({
          ...prev,
          [inst.id]: {
            ...(prev[inst.id] ?? defaultLaunchHooksDraft()),
            ...patch,
          },
        }));
      };
      const modEntries = installedContentSummary.modEntries;
      const resourcepackEntries = installedContentSummary.resourcepackEntries;
      const shaderpackEntries = installedContentSummary.shaderpackEntries;
      const datapackEntries = installedContentSummary.datapackEntries;
      const visibleInstalledMods = installedContentSummary.visibleInstalledMods;
      const currentContentSectionLabel = instanceContentSectionLabel(instanceContentType);
      const currentContentEntryCount =
        instanceContentType === "mods"
          ? modEntries.length
          : instanceContentType === "resourcepacks"
            ? resourcepackEntries.length
            : instanceContentType === "datapacks"
              ? datapackEntries.length
              : shaderpackEntries.length;
      const instanceContentActiveFilterCount = [
        instanceQuery.trim() ? 1 : 0,
        instanceFilterWarningsOnly || instanceFilterState !== "all" ? 1 : 0,
        instanceFilterSource !== "all" ? 1 : 0,
        instanceFilterMissing !== "all" ? 1 : 0,
        instanceSort !== "name_asc" ? 1 : 0,
      ].reduce((sum, value) => sum + value, 0);
      const runningForInstance = runningByInstanceId.get(inst.id) ?? [];
      const quickPlayServersForInstance = quickPlayServers.filter(
        (server) => (server.bound_instance_id ?? inst.id) === inst.id
      );
      const hasRunningForInstance = runningForInstance.length > 0;
      const isLaunchBusyForInstance = launchBusyInstanceIds.includes(inst.id);
      const hasNativeRunningForInstance = runningForInstance.some(
        (r) => String(r.method ?? "").toLowerCase() === "native"
      );
      const concurrentNativeLaunch = hasNativeRunningForInstance && launchMethodPick === "native";
      const hasDisposableRuntimeSession = runningForInstance.some((run) => Boolean(run.isolated));
      const launchActionTitle = isLaunchBusyForInstance
        ? "Cancel current launch"
        : concurrentNativeLaunch
          ? "Start another native run in a disposable session. World, config, and mod changes from the extra run stay temporary; only Minecraft settings sync back."
          : `Launch with ${launchMethodPick === "native" ? "native launcher" : "Prism Launcher"}`;
      const launchFailure = launchFailureByInstance[inst.id] ?? null;
      const hasLaunchFailure = Boolean(launchFailure);
      const launchHealth = launchHealthByInstance[inst.id] ?? null;
      const showLaunchHealthBanner = Boolean(launchHealth) && !launchHealthDismissedByInstance[inst.id];
      const showOpenLaunchLogAction =
        hasNativeRunningForInstance || String(launchFailure?.method ?? "").toLowerCase() === "native";
      const launchStage = launchStageByInstance[inst.id] ?? null;
      const launchStageLabel = launchStage?.label?.trim() || launchStageBadgeLabel(
        launchStage?.status,
        launchStage?.message
      );
      const compactHeroActions =
        hasRunningForInstance ||
        isLaunchBusyForInstance ||
        hasLaunchFailure ||
        launchStage?.status === "starting" ||
        launchStage?.status === "running";
      const selectableVisibleEntries = installedContentSummary.selectableVisibleEntries;
      const selectedVisibleEntryCount = installedContentSummary.selectedVisibleEntryCount;
      const allVisibleEntriesSelected =
        selectableVisibleEntries.length > 0 &&
        selectedVisibleEntryCount === selectableVisibleEntries.length;
      const selectedInstalledEntryCount = installedContentSummary.selectedInstalledEntryCount;
      const instanceActivity = instanceActivityById[inst.id] ?? [];
      const instanceHistory = instanceHistoryById[inst.id] ?? [];
      const timelineCutoffMs = Number(timelineClearedAtByInstance[inst.id] ?? 0);
      const recentActivityFilter = recentActivityFilterByInstance[inst.id] ?? "all";
      const recentRetentionCutoffMs = Date.now() - RECENT_ACTIVITY_WINDOW_HOURS * 60 * 60 * 1000;
      const recentActivityEntriesRaw = [
        ...instanceActivity
          .filter((entry) => Number(entry.at) > timelineCutoffMs && Number(entry.at) >= recentRetentionCutoffMs)
          .map((entry) =>
            toRecentActivityEntry({
              id: `activity:${entry.id}`,
              atMs: Number(entry.at) || 0,
              tone: entry.tone,
              message: entry.message,
              rawKind: "live_activity",
              sourceLabel: "Live activity",
            })
          ),
        ...instanceHistory
          .map((event) => ({
            event,
            atMs: parseDateLike(event.at)?.getTime() ?? 0,
          }))
          .filter(({ atMs }) => atMs > timelineCutoffMs && atMs >= recentRetentionCutoffMs)
          .map(({ event, atMs }) =>
            toRecentActivityEntry({
              id: `history:${event.id}`,
              atMs,
              tone: inferActivityTone(`${event.kind} ${event.summary}`),
              message: event.summary,
              rawKind: event.kind,
              sourceLabel: humanizeToken(event.kind),
            })
          ),
      ].sort((a, b) => b.atMs - a.atMs);
      const recentActivityRetentionLabel = `Last ${RECENT_ACTIVITY_WINDOW_HOURS}h · max ${RECENT_ACTIVITY_LIMIT} events`;
      const showEarlierRecentActivityBucket = RECENT_ACTIVITY_WINDOW_HOURS > 48;
      const isInstanceActivityPanelOpen = instanceActivityPanelOpenByInstance[inst.id] ?? true;
      const showInstanceActivityPane = instanceTab !== "content" || isInstanceActivityPanelOpen;
      const selectedSnapshot =
        snapshots.find((s) => s.id === (rollbackSnapshotId ?? snapshots[0]?.id)) ?? snapshots[0] ?? null;
      const resolveSnapshotProjectLabel = (rawId: string, sourceHint?: "modrinth" | "curseforge" | null) => {
        const candidate = String(rawId ?? "").trim();
        if (!candidate) return null;
        const byProjectId = scopedInstalledMods.find((mod) => {
          const projectId = String(mod.project_id ?? "").trim();
          if (!projectId) return false;
          if (projectId === candidate) return true;
          if (sourceHint === "curseforge" && (projectId === `cf:${candidate}` || projectId.endsWith(`:${candidate}`))) {
            return true;
          }
          return false;
        });
        if (byProjectId?.name?.trim()) return byProjectId.name.trim();
        return null;
      };
      const instanceFriendDrift = friendLinkDriftByInstance[inst.id] ?? null;
      const friendUnsyncedBadge = friendLinkDriftBadge(instanceFriendDrift);
      const friendUnsyncedBadgeTooltip = friendLinkDriftBadgeTooltip(instanceFriendDrift);
      const friendLinkNeedsAttention =
        Boolean(instanceFriendLinkStatus?.linked) &&
        ((instanceFriendLinkStatus?.pending_conflicts_count ?? 0) > 0 || Boolean(friendUnsyncedBadge));
      const friendLinkStatusLabel = !instanceFriendLinkStatus?.linked
        ? "Unlinked"
        : (instanceFriendLinkStatus.pending_conflicts_count ?? 0) > 0
          ? `${instanceFriendLinkStatus.pending_conflicts_count} conflict${instanceFriendLinkStatus.pending_conflicts_count === 1 ? "" : "s"}`
          : friendUnsyncedBadge
            ? `Unsynced ${friendUnsyncedBadge}`
          : instanceFriendLinkStatus.status
            ? instanceFriendLinkStatus.status.replace(/_/g, " ")
            : "Linked";
      const showFriendLinkReadinessCard =
        Boolean(instanceFriendLinkStatus?.linked) &&
        ((instanceFriendLinkStatus?.pending_conflicts_count ?? 0) > 0 ||
          Boolean(friendUnsyncedBadge) ||
          friendLinkSyncBusyInstanceId === inst.id);
      const instanceHealth = instanceHealthById[inst.id] ?? null;
      const instanceHealthPanelPrefs = instanceHealthPanelPrefsByInstance[inst.id] ?? {};
      const hideInstanceHealthPanel = Boolean(instanceHealthPanelPrefs.hidden);
      const friendOnlinePeers = (instanceFriendLinkStatus?.peers ?? []).filter((peer) => peer.online).length;
      const friendPeerTotal = instanceFriendLinkStatus?.peers?.length ?? 0;
      const autoProfileRecommendation = selectedInstanceAutoProfileRecommendation;
      const autoProfileAppliedAt = autoProfileAppliedHintsByInstance[inst.id] ?? null;
      const autoProfileRecSignature = autoProfileRecommendation
        ? autoProfileSignature(autoProfileRecommendation)
        : null;
      const autoProfileMatchesCurrentSettings =
        Boolean(autoProfileRecommendation) &&
        Number(inst.settings?.memory_mb ?? 0) === Number(autoProfileRecommendation?.memory_mb ?? 0) &&
        String(inst.settings?.jvm_args ?? "").trim() ===
          String(autoProfileRecommendation?.jvm_args ?? "").trim() &&
        String(inst.settings?.graphics_preset ?? "").trim().toLowerCase() ===
          String(autoProfileRecommendation?.graphics_preset ?? "").trim().toLowerCase();
      const showAutoProfileBanner =
        Boolean(autoProfileRecommendation) &&
        instanceTab === "content" &&
        !autoProfileMatchesCurrentSettings &&
        !autoProfileAppliedAt &&
        !friendLinkNeedsAttention &&
        (!hasLaunchFailure || autoProfileRecommendation?.confidence !== "low") &&
        autoProfileDismissedByInstance[inst.id] !== autoProfileRecSignature;

      const activeLogCacheKey = `${inst.id}:${logSourceFilter}`;
      const activeLogPayload = rawLogLinesBySource[activeLogCacheKey] ?? null;
      const activeLogWindow = logWindowBySource[activeLogCacheKey] ?? {
        nextBeforeLine: normalizeLogLineNo(activeLogPayload?.next_before_line),
        loadingOlder: false,
        fullyLoaded: normalizeLogLineNo(activeLogPayload?.next_before_line) == null,
      };
      const normalizedUpdatedAt = Number(activeLogPayload?.updated_at ?? Date.now());
      const parsedSourceLines =
        activeLogPayload?.available && Array.isArray(activeLogPayload.lines)
          ? activeLogPayload.lines.map((line, idx) =>
              toInstanceLogLine({
                raw: line.raw,
                source: logSourceFilter,
                index: idx,
                updatedAt: normalizedUpdatedAt,
                severity: line.severity,
                timestamp: line.timestamp,
                lineNo: line.line_no,
              })
            )
          : [];
      const analysisSourceLines = parsedSourceLines;
      const allLogLines =
        parsedSourceLines.length > 0
          ? parsedSourceLines
          : fallbackInstanceLogLines({
              source: logSourceFilter,
              instanceId: inst.id,
              hasRunning: hasRunningForInstance,
              message:
                logLoadErr ||
                activeLogPayload?.message ||
                (activeLogPayload?.available ? "Log file is currently empty." : null),
            });
      const logSourcePath = String(activeLogPayload?.path ?? "").trim();
      const sourceTotalLines = Number(activeLogPayload?.total_lines ?? allLogLines.length);
      const sourceLoadedLines = parsedSourceLines.length;
      const sourceTruncated = activeLogWindow.nextBeforeLine != null;
      const activeQuickFilters = QUICK_LOG_FILTER_OPTIONS.filter((item) => logQuickFilters[item.id]).map(
        (item) => item.id
      );
      const normalizedLogQuery = logFilterQuery.trim().toLowerCase();
      const quickFilterMatch = (line: InstanceLogLine) => {
        if (activeQuickFilters.length === 0) return true;
        const text = line.message.toLowerCase();
        const matches: Record<QuickLogFilter, boolean> = {
          errors: line.severity === "error" || /exception|failed|fatal/.test(text),
          warnings: line.severity === "warn" || /\bwarn(?:ing)?\b/.test(text),
          suspects:
            /mod|mixin|plugin|jar|inject|caused by|suspect/.test(text) ||
            /\.(jar|dll)\b/.test(text),
          crashes:
            /crash|fatal|exception|exit code -1|segmentation|stacktrace|crash report/.test(text),
        };
        return activeQuickFilters.some((id) => matches[id]);
      };
      const visibleLogLines = allLogLines.filter((line) => {
        if (logSeverityFilter !== "all" && line.severity !== logSeverityFilter) return false;
        if (!quickFilterMatch(line)) return false;
        if (!normalizedLogQuery) return true;
        const searchable = `${line.message} ${severityLabel(line.severity)} ${line.source}`.toLowerCase();
        return searchable.includes(normalizedLogQuery);
      });
      const hiddenByFilters = Math.max(0, sourceLoadedLines - visibleLogLines.length);
      const crashSuspects = detectCrashSuspectsFromMessages(
        visibleLogLines.map((line) => ({
          message: line.message,
          severity: line.severity,
        }))
      );
      const copiedLogText = visibleLogLines
        .map(
          (line) =>
            `[${formatLogTimestamp(line.timestamp)}] ${severityLabel(line.severity).toUpperCase()} ${line.message}`
        )
        .join("\n");

      return (
        <div className="page">
          <div className={`instanceLayout ${showInstanceActivityPane ? "" : "activityCollapsed"}`}>
            <section className="instanceMainPane">
              <div className="breadcrumbRow">
                <button className="crumbLink" onClick={() => setRoute("library")} aria-label="Back to Library">
                  Library
                </button>
                <span className="crumbSep">›</span>
                <span className="crumbCurrent" title={inst.name}>{inst.name || "Instance"}</span>
                <span className="crumbSep">›</span>
                <span className="crumbCurrent">{instanceTab === "content" ? "Content" : instanceTab === "worlds" ? "Worlds" : "Logs"}</span>
              </div>

              {!selectedLauncherAccount ? (
                <div className="card instanceWarningBanner">
                  <div className="instanceWarningTitle">Cannot reach authentication servers</div>
                  <div className="instanceWarningText">
                    Connect a Minecraft account to launch with the native runtime.
                  </div>
                </div>
              ) : null}

              {showLaunchHealthBanner ? (
                <div className="card instanceNoticeCard noticeSuccess">
                  <div className="instanceNoticeHead">
                    <div>
                      <div style={{ fontWeight: 950 }}>Launch health check passed</div>
                      <div className="muted">
                        First native launch succeeded for this instance.
                      </div>
                    </div>
                    <button
                      className="btn"
                      onClick={() =>
                        setLaunchHealthDismissedByInstance((prev) => ({
                          ...prev,
                          [inst.id]: true,
                        }))
                      }
                    >
                      Dismiss
                    </button>
                  </div>
                  <div className="instanceNoticeChips">
                    <span className="chip">Auth ✓</span>
                    <span className="chip">Assets ✓</span>
                    <span className="chip">Libraries ✓</span>
                    <span className="chip">Starting Java ✓</span>
                  </div>
                </div>
              ) : null}

              {hasLaunchFailure ? (
                <div className="card instanceNoticeCard noticeDanger">
                  <div className="instanceNoticeHead">
                    <div>
                      <div style={{ fontWeight: 950 }}>Last launch did not complete</div>
                      <div className="muted">{launchFailure?.message || "Check native launch log for details."}</div>
                    </div>
                    <div className="instanceNoticeActions">
                      <button
                        className="btn"
                        onClick={() => void prepareLaunchFixPlan(inst)}
                        disabled={launchFixBusyInstanceId === inst.id}
                      >
                        {launchFixBusyInstanceId === inst.id ? "Building fixes…" : "Fix my instance"}
                      </button>
                      <button className="btn" onClick={() => setSupportBundleModalInstanceId(inst.id)}>
                        Export support bundle
                      </button>
                      <button className="btn" onClick={() => void onOpenLaunchLog(inst)}>
                        <span className="btnIcon">
                          <Icon name="folder" size={16} />
                        </span>
                        Open launch log
                      </button>
                    </div>
                  </div>
                </div>
              ) : null}

              <div
                className={`instanceHealthPanelWrap ${hideInstanceHealthPanel ? "collapsed" : "expanded"}`}
                aria-hidden={hideInstanceHealthPanel}
              >
                <div className="card instanceNoticeCard instanceHealthPanelCard">
                  <div className="instanceHealthHeader">
                    <div className="instanceHealthTitleBlock">
                      <div style={{ fontWeight: 900 }}>Instance health</div>
                      <div className="muted">Disk, launch, and backup status at a glance.</div>
                      {instanceHealth ? (
                        <div className="instanceHealthScoreSummary">
                          Health {instanceHealth.grade} ({instanceHealth.score})
                          {instanceHealth.reasons.length > 0
                            ? ` • ${instanceHealth.reasons.join(" • ")}`
                            : " • No immediate blockers detected."}
                        </div>
                      ) : null}
                    </div>
                  </div>
                  <div className="row" style={{ marginTop: 10, gap: 8, flexWrap: "wrap" }}>
                    <button
                      className="chip subtle chipButton"
                      type="button"
                      title="Total size of this instance folder on disk. Open the storage manager for details."
                      onClick={() => openStorageManager(storageSelectionForInstance(inst.id))}
                    >
                      Disk {formatFileSize(instanceDiskUsageBytes)}
                    </button>
                    <span className="chip subtle" title="Timestamp captured when launch starts.">
                      Last launch {instanceLastLaunchAt ? formatDateTime(instanceLastLaunchAt) : "Never"}
                    </span>
                    <span className="chip subtle" title="Best-effort timestamp of last known exit state.">
                      Last exit {instanceLastExitAt ? formatDateTime(instanceLastExitAt) : "Unknown"}
                    </span>
                    <span className="chip subtle" title="Most recent auto world backup across this instance.">
                      Backup {latestInstanceBackupAt ? formatDateTime(latestInstanceBackupAt) : "None yet"}
                    </span>
                    <span
                      className="chip subtle"
                      title={showNativeOnlyPlaytimeHint ? "Native launch sessions are tracked exactly. Prism launch tracking is not available yet." : "Accumulated tracked playtime."}
                    >
                      Time played {instancePlayedLabel}
                    </span>
                    <span className={`chip ${autoBackupsEnabled ? "subtle" : "danger"}`}>
                      Auto backups {autoBackupsEnabled ? "On" : "Off"}
                    </span>
                    <span className={`chip ${instanceLastExitKindRaw === "crashed" ? "danger" : "subtle"}`}>
                      Status {instanceRunStatusLabel}
                    </span>
                    <button
                      className={`chip chipButton ${setupNeededPermissions.length > 0 ? "danger" : "subtle"}`}
                      title={permissionStatusTagTitle}
                      onClick={() => {
                        if (permissionChecklistBusyByInstance[inst.id]) {
                          setInstallNotice("Permission check already in progress.");
                          return;
                        }
                        if (permissionStatusChipIsActionable) {
                          setInstallNotice("Opened Launch permissions in Settings.");
                          openSettingAnchor("global:permissions", { advanced: true, target: "global" });
                          return;
                        }
                        setInstallNotice("Re-checking launch permissions…");
                        void refreshInstancePermissionChecklist(inst.id, launchMethodPick);
                      }}
                    >
                      {permissionStatusTagLabel}
                    </button>
                  </div>
                  {instanceLastRunReport ? (
                    <div className="instanceRunReportPreview">
                      <div className="rowBetween" style={{ gap: 10 }}>
                        <div style={{ fontWeight: 860 }}>Last run report</div>
                        <span className={`chip ${instanceLastRunReport.exitKind === "crashed" ? "danger" : "subtle"}`}>
                          {instanceLastRunReport.exitKind}
                          {typeof instanceLastRunReport.exitCode === "number"
                            ? ` (${instanceLastRunReport.exitCode})`
                            : ""}
                        </span>
                      </div>
                      <div className="muted" style={{ marginTop: 4 }}>
                        {instanceLastRunReport.createdAt
                          ? `Captured ${formatDateTime(instanceLastRunReport.createdAt)}`
                          : "Captured on last launch."}
                        {instanceLastRunReport.phase ? ` • Phase: ${instanceLastRunReport.phase.replace(/_/g, " ")}` : ""}
                      </div>
                      <div className="row" style={{ marginTop: 8, gap: 6, flexWrap: "wrap" }}>
                        {(instanceLastRunReport.topCauses ?? []).slice(0, 3).map((cause) => (
                          <span key={cause} className="chip subtle">{cause}</span>
                        ))}
                      </div>
                    </div>
                  ) : (
                    <div className="muted" style={{ marginTop: 8 }}>
                      Last run report appears here after launch.
                    </div>
                  )}
                </div>
              </div>

              <div className="instPageTop">
                <div className="instHero">
                  <div className="instHeroIcon">
                    <Icon name="box" size={22} />
                  </div>
                  <div className="instHeroText">
                    <div className="instTitle">{inst.name || "Untitled instance"}</div>
                    <div className="instMetaRow">
                      <span className="chip">{loaderLabel} {inst.mc_version}</span>
                      {launchStageLabel ? (
                        <span className="chip">{launchStage?.status === "starting" ? `Launching: ${launchStageLabel}` : launchStageLabel}</span>
                      ) : (
                        <span className="chip subtle">{hasRunningForInstance ? "Running" : "Never played"}</span>
                      )}
                      {hasLaunchFailure ? <span className="chip">Last launch failed</span> : null}
                    </div>
                  </div>
                </div>

                <div className={`instHeroActions ${compactHeroActions ? "compact" : ""}`}>
                  <MenuSelect
                    value={launchMethodPick}
                    labelPrefix={compactHeroActions ? "Mode" : "Launch"}
                    options={[
                      { value: "native", label: "Native" },
                      { value: "prism", label: "Prism" },
                    ]}
                    onChange={(v) => setLaunchMethodPick((v as LaunchMethod) ?? "native")}
                  />
                  <button
                    className={`btn instanceLaunchBtn ${isLaunchBusyForInstance ? "danger" : "primary"}`}
                    onClick={() => onPlayInstance(inst, launchMethodPick)}
                    disabled={launchCancelBusyInstanceId === inst.id}
                    title={launchActionTitle}
                  >
                    <span className="btnIcon">
                      <Icon name={isLaunchBusyForInstance ? "x" : "play"} size={18} />
                    </span>
                    {isLaunchBusyForInstance
                      ? (launchCancelBusyInstanceId === inst.id ? "Cancelling…" : "Cancel")
                      : "Launch"}
                  </button>
                  {hasRunningForInstance ? (
                    <button
                      className="btn danger"
                      onClick={() => onStopRunning(runningForInstance[0].launch_id)}
                      title={
                        runningForInstance.length > 1
                          ? "Stop the most recent running session for this instance"
                          : undefined
                      }
                    >
                      Stop
                    </button>
                  ) : null}
                  <div className="instHeroTools">
                    {showOpenLaunchLogAction ? (
                      <button
                        className={`btn ${compactHeroActions ? "subtle instHeroIconBtn" : ""}`}
                        onClick={() => void onOpenLaunchLog(inst)}
                        title={compactHeroActions ? "Open launch log" : undefined}
                        aria-label="Open launch log"
                      >
                        <span className="btnIcon">
                          <Icon name="folder" size={16} />
                        </span>
                        {compactHeroActions ? null : "Open launch log"}
                      </button>
                    ) : null}
                    <button
                      className={`btn ${friendLinkNeedsAttention ? "warning" : "subtle"} ${
                        compactHeroActions ? "instHeroIconBtn" : ""
                      }`}
                      onClick={() => setInstanceLinksOpen(true)}
                      title={
                        friendUnsyncedBadge
                          ? `Open Modpack and Friend Link management (${friendLinkStatusLabel}). ${friendUnsyncedBadgeTooltip}`
                          : `Open Modpack and Friend Link management (${friendLinkStatusLabel})`
                      }
                      aria-label="Open links"
                    >
                      <span className="btnIcon">
                        <Icon name="layers" size={18} />
                      </span>
                      {compactHeroActions ? null : "Links"}
                      {compactHeroActions ? (
                        friendLinkNeedsAttention ? <span className="instHeroToolDot" /> : null
                      ) : (
                        <span
                          className={`instanceLinkStatusPill ${friendLinkNeedsAttention ? "alert" : ""}`}
                          title={friendUnsyncedBadge ? friendUnsyncedBadgeTooltip : "Friend Link status"}
                        >
                          {friendLinkStatusLabel}
                        </span>
                      )}
                    </button>
                    <button
                      className={`btn subtle ${compactHeroActions ? "instHeroIconBtn" : ""}`}
                      onClick={() =>
                        setInstanceHealthPanelPrefsByInstance((prev) => ({
                          ...prev,
                          [inst.id]: {
                            ...(prev[inst.id] ?? {}),
                            hidden: !hideInstanceHealthPanel,
                          },
                        }))
                      }
                      title={hideInstanceHealthPanel ? "Show instance health panel" : "Hide instance health panel"}
                      aria-label={hideInstanceHealthPanel ? "Show instance health panel" : "Hide instance health panel"}
                    >
                      {compactHeroActions ? (
                        <span className="btnIcon">
                          <Icon name="sliders" size={18} />
                        </span>
                      ) : hideInstanceHealthPanel ? (
                        "Show health"
                      ) : (
                        "Hide health"
                      )}
                    </button>
                    <button
                      className={`btn settingsSpin ${compactHeroActions ? "instHeroIconBtn" : ""}`}
                      onClick={() => {
                        setInstanceLinksOpen(false);
                        setInstanceSettingsSection("general");
                        setInstanceSettingsOpen(true);
                      }}
                      title="Open instance settings"
                      aria-label="Open instance settings"
                    >
                      <span className="btnIcon">
                        <Icon name="gear" size={18} className="navIcon navAnimGear" />
                      </span>
                    </button>
                  </div>
                </div>
              </div>

              {showFriendLinkReadinessCard ? (
                <div className="card instanceNoticeCard">
                  <div className="instanceNoticeHead instanceNoticeHeadWrap">
                    <div>
                      <div style={{ fontWeight: 950 }} title={friendUnsyncedBadge ? friendUnsyncedBadgeTooltip : "Friend Link readiness status for this instance."}>
                        Friend Link readiness:{" "}
                        {friendUnsyncedBadge
                          ? `unsynced (${friendUnsyncedBadge})`
                          : (instanceFriendLinkStatus.status || "linked").replace(/_/g, " ")}
                      </div>
                      <div className="muted">
                        {friendOnlinePeers}/{friendPeerTotal} peer{friendPeerTotal === 1 ? "" : "s"} online
                        {instanceFriendLinkStatus.pending_conflicts_count > 0
                          ? ` • ${instanceFriendLinkStatus.pending_conflicts_count} conflict${instanceFriendLinkStatus.pending_conflicts_count === 1 ? "" : "s"} pending`
                          : friendUnsyncedBadge
                            ? ` • Unsynced ${friendUnsyncedBadge}`
                            : " • No pending conflicts"}
                      </div>
                    </div>
                    <div className="instanceNoticeActions">
                      <button
                        className="btn"
                        onClick={() => void onManualFriendLinkSync(inst.id)}
                        disabled={friendLinkSyncBusyInstanceId === inst.id}
                        title="Run Friend Link sync now."
                      >
                        {friendLinkSyncBusyInstanceId === inst.id ? "Syncing…" : "Sync now"}
                      </button>
                      {instanceFriendLinkStatus.pending_conflicts_count > 0 ? (
                        <button
                          className="btn danger"
                          onClick={async () => {
                            const out = await reconcileFriendLink({ instanceId: inst.id, mode: "manual" }).catch(
                              () => null
                            );
                            if (out?.status === "conflicted") {
                              setFriendConflictInstanceId(inst.id);
                              setFriendConflictResult(out);
                            }
                          }}
                        >
                          Resolve conflicts
                        </button>
                      ) : null}
                      <button
                        className="btn"
                        data-oj-tooltip="Open full Links panel with sync policy, trust, and selective sync controls."
                        onClick={() => setInstanceLinksOpen(true)}
                      >
                        Open links
                      </button>
                    </div>
                  </div>
                </div>
              ) : null}

              {showAutoProfileBanner && autoProfileRecommendation ? (
                <div className="card instanceNoticeCard">
                  <div className="instanceNoticeHead instanceNoticeHeadWrap">
                    <div>
                      <div style={{ fontWeight: 950 }}>
                        Smart profile: {Math.round(autoProfileRecommendation.memory_mb / 1024)} GB · {autoProfileRecommendation.graphics_preset}
                      </div>
                      <div className="muted">
                        {autoProfileRecommendation.confidence} confidence
                        {autoProfileRecommendation.reasons.length > 0
                          ? ` • ${autoProfileRecommendation.reasons.join(" • ")}`
                          : ""}
                      </div>
                    </div>
                    <div className="instanceNoticeActions">
                      <button
                        className="btn"
                        onClick={() => {
                          setInstanceSettingsSection("java");
                          setInstanceSettingsOpen(true);
                        }}
                      >
                        Review
                      </button>
                      <button
                        className="btn primary"
                        onClick={() => applyAutoProfileRecommendation(inst, autoProfileRecommendation)}
                        disabled={instanceSettingsBusy}
                      >
                        Apply recommendation
                      </button>
                      <button
                        className="btn subtle"
                        onClick={() => {
                          if (!autoProfileRecSignature) return;
                          setAutoProfileDismissedByInstance((prev) => ({
                            ...prev,
                            [inst.id]: autoProfileRecSignature,
                          }));
                          setInstallNotice("Smart profile card dismissed for this recommendation.");
                        }}
                      >
                        Dismiss
                      </button>
                    </div>
                  </div>
                </div>
              ) : null}

              <div className="instTabsRow">
                <SegmentedControl
                  className="instPrimaryTabs"
                  value={instanceTab}
                  onChange={(v) => setInstanceTab(v as any)}
                  options={[
                    { label: "Content", value: "content" },
                    { label: "Worlds", value: "worlds" },
                    { label: "Logs", value: "logs" },
                  ]}
                />
                <div className="instTabsActions">
                  {instanceTab === "content" ? (
                    <>
                      <button className="btn primary installAction" onClick={() => setRoute("discover")}>
                        <span className="btnIcon">
                          <Icon name="plus" size={18} />
                        </span>
                        Install content
                      </button>
                      <MenuSelect
                        value="__menu"
                        labelPrefix="Actions"
                        buttonLabel="More"
                        compact
                        compactPanelMinWidth={196}
                        onChange={(value) => {
                          const action = (value as InstanceContentBulkAction | null) ?? "__menu";
                          if (action === "__menu") return;
                          if (action === "add_local") {
                            if (importingInstanceId === inst.id) {
                              setInstallNotice("Local file import is already in progress for this instance.");
                              return;
                            }
                            void onAddContentFromFile(inst, instanceContentType);
                            return;
                          }
                          if (action === "identify_local") {
                            if (localResolverBusyRef.current[inst.id]) {
                              setInstallNotice("Identify local files is already running for this instance.");
                              return;
                            }
                            void runLocalResolverBackfill(inst.id, "all", {
                              silent: false,
                              refreshListAfterResolve: true,
                              contentTypes: [instanceContentTypeToBackend(instanceContentType)],
                            });
                            return;
                          }
                          if (action === "clean_missing") {
                            if (
                              cleanMissingBusyInstanceId === inst.id ||
                              modsBusy ||
                              Boolean(localResolverBusyRef.current[inst.id])
                            ) {
                              setInstallNotice("Clean missing entries is not available while other content tasks are running.");
                              return;
                            }
                            void onCleanMissingInstalledEntries(inst, instanceContentType);
                          }
                        }}
                        options={[
                          { value: "__menu", label: "More" },
                          {
                            value: "add_local",
                            label: importingInstanceId === inst.id ? "Add local file (busy)" : "Add local file",
                          },
                          {
                            value: "identify_local",
                            label: localResolverBusyRef.current[inst.id]
                              ? "Identify local files (busy)"
                              : "Identify local files",
                          },
                          {
                            value: "clean_missing",
                            label:
                              cleanMissingBusyInstanceId === inst.id ||
                              modsBusy ||
                              Boolean(localResolverBusyRef.current[inst.id])
                                ? "Clean missing entries (busy)"
                                : "Clean missing entries",
                          },
                        ]}
                        align="end"
                      />
                      <button
                        className="btn subtle"
                        onClick={() =>
                          setInstanceActivityPanelOpenByInstance((prev) => ({
                            ...prev,
                            [inst.id]: !isInstanceActivityPanelOpen,
                          }))
                        }
                        aria-pressed={isInstanceActivityPanelOpen}
                        data-oj-tooltip={
                          isInstanceActivityPanelOpen
                            ? "Collapse the right-side activity panel."
                            : "Show the right-side activity panel."
                        }
                      >
                        <span className="btnIcon">
                          <Icon name="layers" size={16} />
                        </span>
                        {isInstanceActivityPanelOpen ? "Hide activity" : "Show activity"}
                      </button>
                    </>
                  ) : instanceTab === "worlds" ? (
                    <>
                      <button
                        className="btn"
                        onClick={async () => {
                          const worlds = await listInstanceWorlds({ instanceId: inst.id }).catch(() => [] as InstanceWorld[]);
                          setInstanceWorlds(worlds);
                          setInstallNotice("World list refreshed.");
                        }}
                      >
                        Refresh
                      </button>
                      <button className="btn" onClick={() => onOpenInstancePath(inst, "instance")}>
                        <span className="btnIcon">
                          <Icon name="folder" size={16} />
                        </span>
                        Open instance folder
                      </button>
                    </>
                  ) : (
                    <></>
                  )}
                </div>
              </div>

              <div className="card instPanel">
                {instanceTab === "content" ? (
                  <div className="instanceContentWrap">
                    <div className="instanceContentWorkspaceCard">
                      <div className="instanceContentWorkspaceHead">
                        <div className="instanceContentWorkspaceIntro">
                          <div className="instanceContentWorkspaceEyebrow">Instance content</div>
                          <div className="instanceContentWorkspaceTitle">Installed {currentContentSectionLabel}</div>
                          <div className="instanceContentWorkspaceSub">
                            Search, sort, update, and clean up this section without leaving the list.
                          </div>
                        </div>
                        <div className="instanceContentWorkspaceStats">
                          <span className="chip subtle">
                            {visibleInstalledMods.length} visible of {currentContentEntryCount}
                          </span>
                          {instanceContentActiveFilterCount > 0 ? (
                            <span className="chip subtle">
                              {instanceContentActiveFilterCount} active filter{instanceContentActiveFilterCount === 1 ? "" : "s"}
                            </span>
                          ) : null}
                          <span className="chip subtle">{currentContentSectionLabel}</span>
                        </div>
                      </div>

                      <div className="instanceContentTopRow">
                        <SegmentedControl
                          value={instanceContentType}
                          onChange={(v) => setInstanceContentType((v as any) ?? "mods")}
                          options={[
                            { label: "Installed mods", value: "mods" },
                            { label: "Resource packs", value: "resourcepacks" },
                            { label: "Datapacks", value: "datapacks" },
                            { label: "Shaders", value: "shaders" },
                          ]}
                          variant="scroll"
                          className="instanceContentTabs"
                        />
                      </div>

                      <div className="instToolbar instToolbarSolo">
                        <div className="instToolbarLeft">
                          <Icon name="search" size={18} />
                          <input
                            className="input"
                            value={instanceQuery}
                            onChange={(e) => setInstanceQuery(e.target.value)}
                            placeholder={`Search ${currentContentSectionLabel}…`}
                          />
                          <button
                            className={`iconBtn instToolbarClearBtn ${instanceQuery ? "" : "hidden"}`}
                            onClick={() => setInstanceQuery("")}
                            aria-label="Clear search"
                            disabled={!instanceQuery}
                            data-oj-tooltip="Clear search"
                          >
                            <Icon name="x" size={18} />
                          </button>
                        </div>
                        <div className="instToolbarRight instanceContentFilterRow">
                          <MenuSelect
                            value={instanceFilterWarningsOnly ? "warnings" : instanceFilterState}
                            labelPrefix="State"
                            onChange={(value) => {
                              if (value === "warnings") {
                                setInstanceFilterWarningsOnly(true);
                                setInstanceFilterState("all");
                                return;
                              }
                              setInstanceFilterWarningsOnly(false);
                              setInstanceFilterState((value as any) ?? "all");
                            }}
                            options={[
                              { value: "all", label: "All" },
                              { value: "enabled", label: "Enabled" },
                              { value: "disabled", label: "Disabled" },
                              ...(instanceContentType === "mods" && hasDependencyWarningsInScope
                                ? [{ value: "warnings", label: "Warnings" }]
                                : []),
                            ]}
                            align="start"
                          />
                          <MenuSelect
                            value={instanceFilterSource}
                            labelPrefix="Source"
                            onChange={(value) => setInstanceFilterSource((value as any) ?? "all")}
                            options={[
                              { value: "all", label: "All" },
                              { value: "modrinth", label: "Modrinth" },
                              { value: "curseforge", label: "CurseForge" },
                              { value: "github", label: "GitHub" },
                              { value: "local", label: "Local" },
                              { value: "other", label: "Other" },
                            ]}
                            align="start"
                          />
                          <MenuSelect
                            value={instanceFilterMissing}
                            labelPrefix="Files"
                            onChange={(value) => setInstanceFilterMissing((value as any) ?? "all")}
                            options={[
                              { value: "all", label: "All" },
                              { value: "present", label: "Present on disk" },
                              { value: "missing", label: "Missing on disk" },
                            ]}
                            align="start"
                          />
                          <MenuSelect
                            value={instanceSort}
                            labelPrefix="Sort"
                            onChange={(value) => setInstanceSort((value as InstanceContentSort | null) ?? "name_asc")}
                            options={[
                              { value: "recently_added", label: "Recently added" },
                              { value: "name_asc", label: "Name A-Z" },
                              { value: "name_desc", label: "Name Z-A" },
                              { value: "source", label: "Source" },
                              { value: "enabled_first", label: "Enabled first" },
                              { value: "disabled_first", label: "Disabled first" },
                            ]}
                            align="start"
                          />
                          <button
                            className="btn subtle instanceContentClearBtn"
                            onClick={() => {
                              setInstanceQuery("");
                              setInstanceFilterState("all");
                              setInstanceFilterSource("all");
                              setInstanceFilterMissing("all");
                              setInstanceFilterWarningsOnly(false);
                              setInstanceSort("name_asc");
                            }}
                          >
                            Clear filters
                          </button>
                        </div>
                      </div>

                      <div className="instanceContentMaintenanceGrid">
                        <div className="instanceContentMaintenancePanel">
                          <div className="instanceContentMaintenancePanelHead">
                            <div className="instanceContentMaintenanceTitle">Updates</div>
                            <div className="instanceContentMaintenanceSub">
                              Check this section, then update everything in one pass.
                            </div>
                          </div>
                          <div className="instanceContentUpdateRow">
                            <button
                              className="btn"
                              onClick={() =>
                                onCheckUpdates(inst, {
                                  contentTypes: [instanceContentTypeToBackend(instanceContentType)],
                                  persistScheduledCache: false,
                                })
                              }
                              disabled={updateBusy || updateAllBusy}
                            >
                              {updateBusy ? "Checking…" : "Refresh"}
                            </button>
                            <button
                              className="btn primary"
                              onClick={() => onUpdateAll(inst, instanceContentType)}
                              disabled={updateAllBusy || updateBusy || (updateCheck?.update_count ?? 0) === 0}
                            >
                              {updateAllBusy ? "Updating…" : `Update all ${currentContentSectionLabel}`}
                            </button>
                            <span className="muted instanceContentControlHint">
                              {updateCheck?.update_count
                                ? `${updateCheck.update_count} update${updateCheck.update_count === 1 ? "" : "s"} ready`
                                : `No check yet for ${currentContentSectionLabel}`}
                            </span>
                          </div>
                        </div>

                        <div className="instanceContentMaintenancePanel">
                          <div className="instanceContentMaintenancePanelHead">
                            <div className="instanceContentMaintenanceTitle">Snapshots</div>
                            <div className="instanceContentMaintenanceSub">
                              Roll back to the last safe point if something goes sideways.
                            </div>
                          </div>
                          {snapshots.length > 0 ? (
                            <div className="instanceSnapshotRow">
                              <MenuSelect
                                value={rollbackSnapshotId ?? snapshots[0].id}
                                labelPrefix="Snapshot"
                                options={snapshots.slice(0, 30).map((s) => ({
                                  value: s.id,
                                  label: formatSnapshotOptionLabel(s, resolveSnapshotProjectLabel),
                                }))}
                                align="start"
                                onChange={(v) => setRollbackSnapshotId(v)}
                              />
                              <button
                                className="btn instanceSnapshotRollbackBtn"
                                onClick={() => onRollbackToSnapshot(inst, rollbackSnapshotId)}
                                disabled={rollbackBusy}
                                title={
                                  selectedSnapshot
                                    ? `Rollback to ${formatSnapshotOptionLabel(selectedSnapshot, resolveSnapshotProjectLabel)}`
                                    : "Rollback to latest snapshot"
                                }
                              >
                                {rollbackBusy ? "Rolling back…" : "Rollback"}
                              </button>
                            </div>
                          ) : (
                            <div className="muted instanceContentControlHint">
                              Installing or updating content creates a snapshot automatically.
                            </div>
                          )}
                        </div>
                      </div>
                    </div>

                    {updateErr ? <div className="errorBox" style={{ marginTop: 4 }}>{updateErr}</div> : null}

                    <div className="instanceContentResultsShell">
                      {updateCheck ? (
                        <div className="card updatesCard">
                          <div className="updatesCardHead">
                            <div>
                              <div className="updatesCardEyebrow">Release check</div>
                              <div className="updatesCardTitle">
                                {updateCheck.update_count === 0
                                  ? `Checked ${updateCheck.checked_entries} entr${updateCheck.checked_entries === 1 ? "y" : "ies"} - all up to date`
                                  : `${updateCheck.update_count} update${updateCheck.update_count === 1 ? "" : "s"} available`}
                              </div>
                            </div>
                            <span className="chip subtle">{currentContentSectionLabel}</span>
                          </div>
                          {updateCheck.update_count > 0 ? (
                            <div className="updatesList">
                              {updateCheck.updates.slice(0, 8).map((u) => (
                                <div key={`${u.source}:${u.content_type}:${u.project_id}`} className="updatesListRow">
                                  <div className="updatesListName">
                                    {u.name}
                                    <span className="chip subtle" style={{ marginLeft: 8 }}>{u.source}</span>
                                    <span className="chip subtle" style={{ marginLeft: 6 }}>{u.content_type}</span>
                                  </div>
                                  <div className="updatesListMeta">
                                    {u.current_version_number} → {u.latest_version_number}
                                    {Array.isArray(u.compatibility_notes) && u.compatibility_notes.length > 0 ? (
                                      <div className="muted" style={{ marginTop: 4 }}>
                                        {u.compatibility_notes[0]}
                                      </div>
                                    ) : null}
                                  </div>
                                </div>
                              ))}
                              {updateCheck.updates.length > 8 ? (
                                <div className="muted">+{updateCheck.updates.length - 8} more</div>
                              ) : null}
                            </div>
                          ) : null}
                        </div>
                      ) : null}

                      {modsBusy ? (
                        <div className="emptyState">
                          <div className="emptyTitle">Loading installed content…</div>
                        </div>
                      ) : modsErr ? (
                        <div className="errorBox" style={{ marginTop: 8 }}>{modsErr}</div>
                      ) : visibleInstalledMods.length === 0 ? (
                        <div className="emptyState">
                          <div className="emptyTitle">No {instanceContentType} installed</div>
                          <div className="emptySub">Install from Discover or apply a preset.</div>
                        </div>
                      ) : (
                        <div className={`instanceModsTable ${selectedInstalledEntryCount > 0 ? "hasStickyActions" : ""}`}>
                        <div className="instanceModsHeaderRow">
                          <div className="instanceModsHeaderSelect">
                            <input
                              type="checkbox"
                              className="instanceModsSelectCheck"
                              checked={allVisibleEntriesSelected}
                              onChange={(e) =>
                                onToggleAllVisibleModSelection(visibleInstalledMods, e.target.checked)
                              }
                              disabled={selectableVisibleEntries.length === 0 || toggleBusyVersion === "__bulk__"}
                              aria-label={
                                allVisibleEntriesSelected ? "Unselect all visible entries" : "Select all visible entries"
                              }
                            />
                          </div>
                          <div className="instanceModsHeaderName">Name</div>
                          <div className="instanceModsHeaderUpdated">Updated</div>
                          <div className="instanceModsHeaderAction">Action</div>
                        </div>
                        {visibleInstalledMods.map((m) => {
                          const entryKey = installedEntryUiKey(m);
                          const iconKey = installedIconCacheKey(m);
                          const iconSrc = installedIconFailedByKey[iconKey]
                            ? null
                            : installedIconCache[iconKey] ?? null;
                          const activeProviderSource = normalizeProviderSource(m.source);
                          const providerCandidates = installedProviderCandidates(m);
                          const providerBadgeCandidates = installedProviderBadgeCandidates(m);
                          const hasGithubCandidate = providerCandidates.some(
                            (candidate) => normalizeProviderSource(candidate.source) === "github"
                          );
                          const addedAtMs = Number(m.added_at ?? 0);
                          const addedAtLabel =
                            Number.isFinite(addedAtMs) && addedAtMs > 0
                              ? new Date(addedAtMs).toLocaleDateString(undefined, {
                                  month: "short",
                                  day: "numeric",
                                  year: "numeric",
                                })
                              : null;
                          const rowMetaParts = [
                            m.enabled ? "Enabled" : "Disabled",
                            ...(m.pinned_version ? ["Pinned"] : []),
                            ...(m.file_exists ? [] : ["Missing file"]),
                            ...(m.target_worlds?.length ? [`Worlds: ${m.target_worlds.join(", ")}`] : []),
                          ];
                          const rightMetaParts = [
                            (m.version_number ?? "").trim() || "Unknown version",
                            ...(addedAtLabel ? [`Added ${addedAtLabel}`] : []),
                          ];
                          return (
                            <div
                              key={`${inst.id}:${entryKey}`}
                              className={`instanceModsRow ${m.enabled ? "" : "disabled"} ${
                                selectedModVersionIdSet.has(entryKey) ? "selected" : ""
                              }`}
                              role="button"
                              tabIndex={0}
                              onClick={(event) => {
                                if (eventTargetsInteractiveControl(event)) return;
                                void openInstalledModDetails(m);
                              }}
                              onKeyDown={(event) => {
                                if (eventTargetsInteractiveControl(event)) return;
                                if (event.key !== "Enter" && event.key !== " ") return;
                                event.preventDefault();
                                void openInstalledModDetails(m);
                              }}
                              data-oj-tooltip="Open content details"
                            >
                              <div className="instanceModsSelectCell">
                                <input
                                  type="checkbox"
                                  className="instanceModsSelectCheck"
                                  checked={selectedModVersionIdSet.has(entryKey)}
                                  data-row-action="true"
                                  onPointerDown={(e) => e.stopPropagation()}
                                  onClick={(e) => e.stopPropagation()}
                                  onChange={(e) => onToggleModSelection(entryKey, e.target.checked)}
                                  disabled={!m.file_exists || toggleBusyVersion === "__bulk__"}
                                  aria-label={`Select ${m.name}`}
                                />
                              </div>
                              <div className="instanceModsNameCell">
                                <LazyInstalledModIcon
                                  alt={`${m.name} icon`}
                                  src={iconSrc}
                                  onVisible={() => requestInstalledModIcon(m)}
                                  onError={() => markInstalledModIconFailed(m)}
                                />
                                <div className="instanceModsNameText">
                                  <div className="instanceModsNameTitle">{m.name}</div>
                                  <div className="instanceModsNameMeta">{rowMetaParts.join(" · ")}</div>
                                  <div className="instanceModsProviderBadges">
                                    {!m.file_exists ? (
                                      <span className="instanceProviderBadge missing">Missing on disk</span>
                                    ) : null}
                                    <DependencyBadge
                                      warnings={m.local_analysis?.warnings ?? []}
                                      busy={
                                        dependencyInstallBusyVersion === entryKey ||
                                        toggleBusyVersion === entryKey ||
                                        toggleBusyVersion === "__bulk__" ||
                                        providerSwitchBusyKey?.startsWith(`${entryKey}:`) ||
                                        pinBusyVersion === entryKey
                                      }
                                      onClick={() => void onInstallMissingDependencies(inst, m)}
                                    />
                                    {providerBadgeCandidates.map((candidate) => {
                                      const source = normalizeProviderSource(candidate.source);
                                      const label = providerSourceLabel(candidate.source);
                                      const isActive = source === normalizeProviderSource(m.source);
                                      const candidateExplain = providerCandidateExplain(candidate);
                                      const switchKey = `${entryKey}:${source}`;
                                      const isBusy = providerSwitchBusyKey === switchKey;
                                      const canSwitch =
                                        (source === "modrinth" ||
                                          source === "curseforge" ||
                                          source === "github") &&
                                        source !== activeProviderSource;
                                      const badgeLabel = label;
                                      if (!canSwitch) {
                                        return (
                                          <span
                                            key={`${candidate.source}:${candidate.project_id}:${candidate.version_id}`}
                                            className={`instanceProviderBadge ${isActive ? "active" : ""}`}
                                            data-oj-tooltip={
                                              candidateExplain ??
                                              (isActive
                                                ? `${label} is currently active`
                                                : `${label} is available as a provider option`)
                                            }
                                          >
                                            {badgeLabel}
                                          </span>
                                        );
                                      }
                                      return (
                                        <button
                                          key={`${candidate.source}:${candidate.project_id}:${candidate.version_id}`}
                                          type="button"
                                          className={`instanceProviderBadge ${isActive ? "active" : ""} ${canSwitch ? "clickable" : ""}`}
                                          data-row-action="true"
                                          onPointerDown={(event) => event.stopPropagation()}
                                          onClick={(event) => {
                                            event.stopPropagation();
                                            void onSetInstalledModProvider(inst, m, source);
                                          }}
                                          disabled={
                                            isBusy ||
                                            toggleBusyVersion === entryKey ||
                                            toggleBusyVersion === "__bulk__"
                                          }
                                          data-oj-tooltip={
                                            candidateExplain ??
                                            (isActive
                                              ? `${label} is currently active for metadata and actions`
                                              : `Switch to ${label} metadata and actions`)
                                          }
                                        >
                                          {isBusy ? "Switching…" : badgeLabel}
                                        </button>
                                      );
                                    })}
                                    <button
                                      className={`instanceProviderBadge clickable ${
                                        m.pinned_version ? "active" : ""
                                      }`}
                                      type="button"
                                      data-row-action="true"
                                      onPointerDown={(event) => event.stopPropagation()}
                                      onClick={(event) => {
                                        event.stopPropagation();
                                        void onToggleInstalledModPin(inst, m);
                                      }}
                                      disabled={
                                        pinBusyVersion === entryKey ||
                                        toggleBusyVersion === entryKey ||
                                        toggleBusyVersion === "__bulk__"
                                      }
                                      data-oj-tooltip={
                                        m.pinned_version
                                          ? `Pinned to ${m.pinned_version}. Click to unpin.`
                                          : "Pin this entry to current version (skips update-all)."
                                      }
                                    >
                                      {pinBusyVersion === entryKey
                                        ? "Saving…"
                                        : m.pinned_version
                                          ? "Pinned"
                                          : "Pin"}
                                    </button>
                                    {normalizeCreatorEntryType(m.content_type) === "mods" &&
                                    (activeProviderSource === "local" ||
                                      activeProviderSource === "github") ? (
                                      <button
                                        className="instanceProviderBadge clickable"
                                        type="button"
                                        data-row-action="true"
                                        onPointerDown={(event) => event.stopPropagation()}
                                        onClick={(event) => {
                                          event.stopPropagation();
                                          beginAttachInstalledModGithubRepo(inst, m);
                                        }}
                                        disabled={
                                          githubAttachBusyVersion === entryKey ||
                                          toggleBusyVersion === entryKey ||
                                          toggleBusyVersion === "__bulk__"
                                        }
                                        data-oj-tooltip={
                                          hasGithubCandidate
                                            ? "Save or replace the GitHub repository hint"
                                            : "Save a GitHub repository hint manually"
                                        }
                                      >
                                        {githubAttachBusyVersion === entryKey
                                          ? "Saving…"
                                          : hasGithubCandidate
                                            ? "Update GitHub Hint"
                                            : "Save GitHub Hint"}
                                      </button>
                                    ) : null}
                                  </div>
                                </div>
                              </div>

                              <div className="instanceModsUpdatedCell">
                                <div className="instanceModsFilenamePrimary" data-oj-tooltip={m.filename}>
                                  {m.filename}
                                </div>
                                <div className="instanceModsVersionMeta">{rightMetaParts.join(" · ")}</div>
                              </div>

                              <div className="instanceModsActionCell">
                                <div className="instanceModsActionRow">
                                  <button
                                    className={`instanceActionIconBtn instanceActionToggleBtn ${m.enabled ? "enabled" : "disabled"}`}
                                    type="button"
                                    data-row-action="true"
                                    onClick={(event) => {
                                      event.stopPropagation();
                                      void onToggleInstalledMod(inst, m, !m.enabled);
                                    }}
                                    disabled={toggleBusyVersion === entryKey || toggleBusyVersion === "__bulk__" || !m.file_exists}
                                    aria-label={
                                      m.enabled
                                        ? `${installedContentTypeLabel(m.content_type)} enabled, click to disable`
                                        : `${installedContentTypeLabel(m.content_type)} disabled, click to enable`
                                    }
                                    data-oj-tooltip={m.enabled ? "Disable this entry" : "Enable this entry"}
                                  >
                                    <Icon
                                      name={toggleBusyVersion === entryKey ? "sparkles" : m.enabled ? "check_circle" : "slash_circle"}
                                      size={19}
                                    />
                                  </button>
                                  <button
                                    className="instanceActionIconBtn instanceActionDeleteBtn"
                                    type="button"
                                    data-row-action="true"
                                    onClick={(event) => {
                                      event.stopPropagation();
                                      void onDeleteInstalledMod(inst, m);
                                    }}
                                    disabled={toggleBusyVersion === entryKey || toggleBusyVersion === "__bulk__"}
                                    aria-label={`Delete ${installedContentTypeLabel(m.content_type)}`}
                                    data-oj-tooltip={`Remove this ${installedContentTypeLabel(m.content_type)} from the instance`}
                                  >
                                    <Icon name={toggleBusyVersion === entryKey ? "sparkles" : "trash"} size={19} />
                                  </button>
                                </div>
                              </div>
                            </div>
                          );
                        })}
                        </div>
                      )}
                    </div>

                    {selectedInstalledEntryCount > 0 ? (
                      <div className="instanceModsStickyBar">
                        <div className="instanceModsStickyTitle">
                          Apply to selected {instanceContentSectionLabel(instanceContentType)} · {selectedInstalledEntryCount} selected · {installedContentSummary.visibleInstalledMods.length} visible
                        </div>
                        <div className="instanceModsStickyActions">
                          <button
                            className="btn primary"
                            onClick={() => void onBulkToggleSelectedMods(inst, true)}
                            disabled={selectedInstalledEntryCount === 0 || toggleBusyVersion === "__bulk__"}
                          >
                            {toggleBusyVersion === "__bulk__" ? "Applying…" : "Enable selected"}
                          </button>
                          <button
                            className="btn danger"
                            onClick={() => void onBulkToggleSelectedMods(inst, false)}
                            disabled={selectedInstalledEntryCount === 0 || toggleBusyVersion === "__bulk__"}
                          >
                            {toggleBusyVersion === "__bulk__" ? "Applying…" : "Disable selected"}
                          </button>
                          <button
                            className="btn"
                            onClick={() => setSelectedModVersionIds([])}
                            disabled={selectedInstalledEntryCount === 0 || toggleBusyVersion === "__bulk__"}
                          >
                            Clear selection
                          </button>
                        </div>
                      </div>
                    ) : null}
                  </div>
                ) : instanceTab === "worlds" ? (
                  <div className="instanceSectionShell">
                    <div className="instanceSectionHeader">
                      <div>
                        <div className="instanceSectionTitle">Worlds</div>
                        <div className="instanceSectionMeta">World saves discovered in this instance folder.</div>
                      </div>
                      <div className="instanceSectionActions">
                        <span className="chip subtle">
                          Auto backup every {instSettings.world_backup_interval_minutes} min
                        </span>
                        <button
                          className="btn"
                          onClick={async () => {
                            const worlds = await listInstanceWorlds({ instanceId: inst.id }).catch(() => [] as InstanceWorld[]);
                            setInstanceWorlds(worlds);
                            setInstallNotice("World list refreshed.");
                          }}
                        >
                          Refresh
                        </button>
                      </div>
                    </div>
                    {instanceWorlds.length === 0 ? (
                      <div className="instanceWorldsEmpty">
                        <div className="instanceWorldsIconWrap">
                          <Icon name="sparkles" size={30} />
                        </div>
                        <div className="emptyTitle">You don't have any worlds yet.</div>
                        <div className="emptySub">Create a world in Minecraft, then refresh this list.</div>
                        <div className="instanceWorldsActions">
                          <button className="btn" onClick={() => onOpenInstancePath(inst, "saves")}>
                            <span className="btnIcon">
                              <Icon name="folder" size={16} />
                            </span>
                            Open saves folder
                          </button>
                        </div>
                      </div>
                    ) : (
                      <div className="instanceWorldGrid">
                        {(() => {
                          const instanceRunning = (runningByInstanceId.get(inst.id)?.length ?? 0) > 0;
                          return instanceWorlds.map((world) => {
                            const hasBackup = Boolean(world.latest_backup_at) && (world.backup_count ?? 0) > 0;
                            const rollbackBusyForWorld = Boolean(worldRollbackBusyById[world.id]);
                            const rollbackDisabled = !hasBackup || rollbackBusyForWorld;
                            return (
                              <div key={world.id} className="card instanceWorldCard">
                                <div className="instanceWorldCardTop">
                                  <div className="instanceWorldName">{world.name}</div>
                                  <span className="chip subtle">
                                    {hasBackup
                                      ? `${world.backup_count ?? 0} backup${(world.backup_count ?? 0) === 1 ? "" : "s"}`
                                      : "No backup yet"}
                                  </span>
                                </div>
                                <div className="muted">{world.path}</div>
                                <div className="instanceWorldBackupMeta">
                                  {hasBackup
                                    ? `Latest backup: ${formatDateTime(world.latest_backup_at)}`
                                    : `Auto backup runs every ${instSettings.world_backup_interval_minutes} minutes while Minecraft is running.`}
                                </div>
                                <div className="instanceWorldActions">
                                  <button className="btn" onClick={() => onOpenInstancePath(inst, "saves")}>
                                    <span className="btnIcon">
                                      <Icon name="folder" size={15} />
                                    </span>
                                    Open saves
                                  </button>
                                  <button
                                    className="btn primary"
                                    onClick={() => void onRollbackWorldBackup(inst, world)}
                                    disabled={rollbackDisabled}
                                    title={
                                      instanceRunning
                                        ? "Stop Minecraft first, then rollback."
                                        : hasBackup
                                          ? "Restore this world from the latest auto-backup."
                                          : "No auto-backup available yet."
                                    }
                                  >
                                    {rollbackBusyForWorld ? "Rolling back…" : "Rollback latest backup"}
                                  </button>
                                </div>
                              </div>
                            );
                          });
                        })()}
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="instanceSectionShell">
                    <div className="instanceSectionHeader">
                      <div>
                        <div className="instanceSectionTitle">Logs</div>
                        <div className="instanceSectionMeta">Clean log view with quick filters and suspects.</div>
                      </div>
                    </div>
                    <div className="instanceLogsModeRow">
                      <SegmentedControl
                        value={logViewMode}
                        onChange={(v) => setLogViewMode((v as LogViewMode) ?? "live")}
                        options={[
                          { value: "live", label: "Live" },
                          { value: "analyze", label: "Analyze" },
                        ]}
                      />
                      <span className="chip subtle">
                        {logViewMode === "live"
                          ? `${visibleLogLines.length} visible · ${sourceLoadedLines.toLocaleString()} / ${sourceTotalLines.toLocaleString()} loaded`
                          : "Paste logs or analyze current source"}
                      </span>
                    </div>
                    {logViewMode === "live" ? (
                      <div className="instanceLogsShell">
                        <div className="instanceLogsMain">
                          <div className="instanceLogsToolbar">
                            <div className="instToolbarLeft instanceLogSearch">
                              <Icon name="search" size={18} />
                              <input
                                className="input"
                                value={logFilterQuery}
                                onChange={(e) => setLogFilterQuery(e.target.value)}
                                placeholder="Search log lines…"
                              />
                              {logFilterQuery ? (
                                <button
                                  className="iconBtn"
                                  onClick={() => setLogFilterQuery("")}
                                  aria-label="Clear search"
                                >
                                  <Icon name="x" size={18} />
                                </button>
                              ) : null}
                            </div>
                            <MenuSelect
                              value={logSeverityFilter}
                              labelPrefix="Level"
                              options={LOG_SEVERITY_OPTIONS}
                              onChange={(v) =>
                                setLogSeverityFilter(
                                  (v as "all" | InstanceLogSeverity | null) ?? "all"
                                )
                              }
                            />
                            <MenuSelect
                              value={logSourceFilter}
                              labelPrefix="Source"
                              options={LOG_SOURCE_OPTIONS}
                              onChange={(v) =>
                                setLogSourceFilter((v as InstanceLogSource | null) ?? "live")
                              }
                            />
                            <MenuSelect
                              value={String(logMaxLines)}
                              labelPrefix="Lines"
                              options={LOG_MAX_LINES_OPTIONS}
                              onChange={(v) => {
                                const parsed = Number.parseInt(v, 10);
                                if (!Number.isFinite(parsed)) return;
                                setLogMaxLines(Math.max(200, Math.min(12000, parsed)));
                              }}
                            />
                            <button
                              className="btn"
                              onClick={async () => {
                                try {
                                  await navigator.clipboard.writeText(
                                    copiedLogText || "No visible log lines to copy."
                                  );
                                  setInstallNotice("Copied visible log lines.");
                                } catch {
                                  setInstallNotice("Could not copy visible log lines.");
                                }
                              }}
                            >
                              Copy
                            </button>
                            <button
                              className="btn"
                              onClick={() => {
                                setLogFilterQuery("");
                                setLogSeverityFilter("all");
                                setLogQuickFilters({
                                  errors: false,
                                  warnings: false,
                                  suspects: false,
                                  crashes: false,
                                });
                                setSelectedCrashSuspect(null);
                                setInstallNotice("Log filters cleared.");
                              }}
                            >
                              Clear
                            </button>
                            <button
                              className="btn"
                              onClick={() => {
                                if (logSourceFilter === "latest_crash") {
                                  void onOpenInstancePath(inst, "crash-log");
                                } else {
                                  void onOpenInstancePath(inst, "launch-log");
                                }
                              }}
                            >
                              Open file
                            </button>
                          </div>

                          <div className="instanceLogsQuickFilters">
                            <span className="instanceLogsQuickLabel">Quick filters</span>
                            {QUICK_LOG_FILTER_OPTIONS.map((opt) => (
                              <button
                                key={opt.id}
                                className={`instanceLogQuickChip ${logQuickFilters[opt.id] ? "on" : ""}`}
                                onClick={() =>
                                  setLogQuickFilters((prev) => ({
                                    ...prev,
                                    [opt.id]: !prev[opt.id],
                                  }))
                                }
                              >
                                {opt.label}
                              </button>
                            ))}
                          </div>

                          <div className="instanceLogsMetaStrip">
                            <span className="instanceLogsMetaStatus">{logLoadBusy ? "Refreshing…" : "Ready"}</span>
                            {logSourcePath ? (
                              <span className="instanceLogsMetaPath" title={logSourcePath}>
                                {logSourcePath}
                              </span>
                            ) : null}
                            <span className="chip subtle">
                              Loaded {sourceLoadedLines.toLocaleString()} / {sourceTotalLines.toLocaleString()}
                            </span>
                            <span className="chip subtle">Visible {visibleLogLines.length.toLocaleString()}</span>
                            <span className="chip subtle">Hidden by filters {hiddenByFilters.toLocaleString()}</span>
                            {sourceTruncated ? <span className="chip subtle">Older lines hidden</span> : null}
                            {logSourceFilter !== "live" && activeLogWindow.nextBeforeLine != null ? (
                              <button
                                className="btn"
                                onClick={() => void onLoadOlderLogLines()}
                                disabled={activeLogWindow.loadingOlder}
                              >
                                {activeLogWindow.loadingOlder ? "Loading older…" : "Load older lines"}
                              </button>
                            ) : null}
                          </div>
                          {logLoadErr ? <div className="instanceLogsInlineErr">{logLoadErr}</div> : null}

                          <div
                            className="instanceLogsViewer"
                            ref={logViewerRef}
                            onScroll={onLogViewerScroll}
                          >
                            {visibleLogLines.length === 0 ? (
                              <div className="instanceLogsEmpty">
                                No log lines match your current filters.
                              </div>
                            ) : (
                              <div className="instanceLogRows">
                                {visibleLogLines.map((line) => {
                                  const suspectMatch = selectedCrashSuspect
                                    ? line.message.toLowerCase().includes(selectedCrashSuspect)
                                    : false;
                                  return (
                                    <div
                                      key={line.id}
                                      className={`instanceLogRow sev-${line.severity} ${suspectMatch ? "suspectHit" : ""}`}
                                    >
                                      <span className={`instanceLogSeverityPill sev-${line.severity}`}>
                                        {severityShort(line.severity)}
                                      </span>
                                      <div className="instanceLogRowMain">
                                        <span className="instanceLogTimestamp">
                                          {formatLogTimestamp(line.timestamp)}
                                        </span>
                                        <span className="instanceLogMessage">{line.message}</span>
                                      </div>
                                    </div>
                                  );
                                })}
                              </div>
                            )}
                            {logJumpVisible ? (
                              <button className="btn instanceLogsJumpBtn" onClick={onJumpLogsToBottom}>
                                Jump to latest
                              </button>
                            ) : null}
                          </div>
                        </div>

                        <aside className="instanceLogSuspects">
                          <div className="instanceLogSuspectsHead">
                            <div className="instanceLogSuspectsTitle">Crash suspects</div>
                            <span className="chip subtle">Ranked</span>
                          </div>
                          <div className="instanceLogSuspectsSub">
                            Ranked from visible lines with weighted crash heuristics.
                          </div>
                          <div className="instanceLogSuspectList">
                            {crashSuspects.length === 0 ? (
                              <div className="instanceLogSuspectsEmpty">
                                No strong suspects detected.
                              </div>
                            ) : (
                              crashSuspects.map((suspect) => (
                                <button
                                  key={suspect.id}
                                  className={`instanceLogSuspectItem ${selectedCrashSuspect === suspect.id ? "on" : ""}`}
                                  onClick={() =>
                                    setSelectedCrashSuspect((prev) =>
                                      prev === suspect.id ? null : suspect.id
                                    )
                                  }
                                >
                                  <span>{suspect.label}</span>
                                  <span className="chip subtle">
                                    {suspect.matches} · {Math.round(suspect.confidence * 100)}%
                                  </span>
                                </button>
                              ))
                            )}
                          </div>
                        </aside>
                      </div>
                    ) : (
                      <div className="instanceAnalyzeWrap">
                        <div className="instanceAnalyzeInput">
                          <div className="instanceAnalyzeTitle">Analyze logs</div>
                          <div className="instanceAnalyzeSub">
                            Paste log text below or drop a file here to run offline analysis.
                          </div>
                          <textarea
                            className="textarea instanceAnalyzeTextarea"
                            value={logAnalyzeInput}
                            onChange={(e) => {
                              setLogAnalyzeInput(e.target.value);
                              setLogAnalyzeResult(null);
                              setLogAnalyzeSourcesUsed([]);
                              setLogAnalyzeMissingCrash(false);
                            }}
                            placeholder="Paste logs here…"
                          />
                          <div className="row" style={{ marginTop: 6 }}>
                            <button
                              className="btn primary"
                              onClick={() => {
                                const result = analyzeLogText(logAnalyzeInput);
                                setLogAnalyzeResult(result);
                                setLogAnalyzeSourcesUsed([]);
                                setLogAnalyzeMissingCrash(false);
                                setInstallNotice(`Analyzed ${result.totalLines} log line${result.totalLines === 1 ? "" : "s"}.`);
                              }}
                              disabled={!logAnalyzeInput.trim() || logAnalyzeBusy}
                            >
                              {logAnalyzeBusy ? "Analyzing…" : "Analyze"}
                            </button>
                            <button
                              className="btn"
                              onClick={() => {
                                void (async () => {
                                  if (analysisSourceLines.length === 0 || logAnalyzeBusy) return;
                                  setLogAnalyzeBusy(true);
                                  try {
                                    const sourceOrder: InstanceLogSource[] = [logSourceFilter];
                                    if (!sourceOrder.includes("latest_launch")) {
                                      sourceOrder.push("latest_launch");
                                    }
                                    if (!sourceOrder.includes("latest_crash")) {
                                      sourceOrder.push("latest_crash");
                                    }

                                    const sourceRows = new Map<InstanceLogSource, InstanceLogLine[]>();
                                    sourceRows.set(logSourceFilter, analysisSourceLines);
                                    let missingCrash = false;

                                    for (const source of sourceOrder) {
                                      if (source === logSourceFilter) continue;
                                      const cacheKey = `${inst.id}:${source}`;
                                      let payload = rawLogLinesBySource[cacheKey] ?? null;
                                      if (!payload || !payload.available) {
                                        try {
                                          payload = await readInstanceLogs({
                                            instanceId: inst.id,
                                            source,
                                            maxLines: logMaxLines,
                                          });
                                        } catch {
                                          payload = null;
                                        }
                                      }
                                      const rows =
                                        payload?.available && Array.isArray(payload.lines)
                                          ? payload.lines.map((line, idx) =>
                                              toInstanceLogLine({
                                                raw: line.raw,
                                                source,
                                                index: idx,
                                                updatedAt: Number(payload?.updated_at ?? Date.now()),
                                                severity: line.severity,
                                                timestamp: line.timestamp,
                                                lineNo: line.line_no,
                                              })
                                            )
                                          : [];
                                      if (rows.length > 0) {
                                        sourceRows.set(source, rows);
                                      } else if (source === "latest_crash") {
                                        missingCrash = true;
                                      }
                                    }

                                    const dedupe = new Set<string>();
                                    const combined: InstanceLogLine[] = [];
                                    for (const source of sourceOrder) {
                                      const rows = sourceRows.get(source) ?? [];
                                      for (const row of rows) {
                                        const dedupeKey = `${source}:${row.lineNo ?? "x"}:${row.message}`;
                                        if (dedupe.has(dedupeKey)) continue;
                                        dedupe.add(dedupeKey);
                                        combined.push(row);
                                      }
                                    }
                                    const cappedCombined =
                                      combined.length > 18000 ? combined.slice(combined.length - 18000) : combined;
                                    const result = analyzeLogLines(
                                      cappedCombined.map((line) => ({
                                        message: line.message,
                                        severity: line.severity,
                                        source: line.source,
                                        lineNo: line.lineNo,
                                        timestamp: line.timestamp,
                                      }))
                                    );
                                    const sourcesUsed = sourceOrder.filter(
                                      (source) => (sourceRows.get(source) ?? []).length > 0
                                    );
                                    setLogAnalyzeResult(result);
                                    setLogAnalyzeSourcesUsed(sourcesUsed);
                                    setLogAnalyzeMissingCrash(missingCrash);
                                    setInstallNotice(
                                      `Analyzed ${result.totalLines} line${result.totalLines === 1 ? "" : "s"} from ${sourcesUsed.length} source${sourcesUsed.length === 1 ? "" : "s"}.`
                                    );
                                  } finally {
                                    setLogAnalyzeBusy(false);
                                  }
                                })();
                              }}
                              disabled={analysisSourceLines.length === 0 || logAnalyzeBusy}
                            >
                              {logAnalyzeBusy ? "Analyzing…" : "Analyze current source"}
                            </button>
                            <button
                              className="btn"
                              onClick={() => void prepareLaunchFixPlan(inst)}
                              disabled={launchFixBusyInstanceId === inst.id}
                            >
                              {launchFixBusyInstanceId === inst.id ? "Building fixes…" : "Fix my instance"}
                            </button>
                          </div>
                          {logAnalyzeSourcesUsed.length > 0 ? (
                            <div className="instanceAnalyzeSourceRow">
                              <span className="instanceAnalyzeSourcesLabel">Sources used</span>
                              {logAnalyzeSourcesUsed.map((source) => (
                                <span key={source} className="chip subtle">
                                  {sourceLabel(source)}
                                </span>
                              ))}
                              {logAnalyzeMissingCrash ? (
                                <span className="chip subtle">No crash report found</span>
                              ) : null}
                            </div>
                          ) : null}
                        </div>
                        <div className="instanceAnalyzeResults">
                          <div className="instanceAnalyzeCard">
                            <div className="instanceAnalyzeCardTitle">Summary</div>
                            {logAnalyzeResult ? (
                              <div className="muted">
                                {logAnalyzeResult.totalLines} lines · {logAnalyzeResult.errorCount} errors · {logAnalyzeResult.warnCount} warnings · {logAnalyzeResult.infoCount} info
                              </div>
                            ) : (
                              <div className="muted">Run Analyze to generate a summary.</div>
                            )}
                          </div>
                          <div className="instanceAnalyzeCard">
                            <div className="instanceAnalyzeCardTitle">Likely causes</div>
                            {logAnalyzeResult && logAnalyzeResult.likelyCauses.length > 0 ? (
                              <div className="instanceAnalyzeCardBody instanceAnalyzeList">
                                {logAnalyzeResult.likelyCauses.slice(0, 4).map((cause) => (
                                  <div key={cause.id} className="instanceAnalyzeItem">
                                    <div className="rowBetween">
                                      <span>{cause.title}</span>
                                      <span className="chip subtle">{Math.round(cause.confidence * 100)}%</span>
                                    </div>
                                    <div className="muted">{cause.reason}</div>
                                    {logAnalyzeResult.evidenceByCause?.[cause.id]?.[0] ? (
                                      <div className="muted">
                                        Evidence: {logAnalyzeResult.evidenceByCause[cause.id][0]}
                                      </div>
                                    ) : null}
                                  </div>
                                ))}
                              </div>
                            ) : (
                              <div className="muted">No high-confidence root cause detected yet.</div>
                            )}
                          </div>
                          <div className="instanceAnalyzeCard">
                            <div className="instanceAnalyzeCardTitle">Failed mods</div>
                            {logAnalyzeResult && logAnalyzeResult.failedMods.length > 0 ? (
                              <div className="instanceAnalyzeCardBody instanceAnalyzeList">
                                {logAnalyzeResult.failedMods.slice(0, 6).map((mod) => (
                                  <div key={mod.id} className="instanceAnalyzeItem">
                                    <div className="rowBetween">
                                      <span>{mod.label}</span>
                                      <span className="chip subtle">{Math.round(mod.confidence * 100)}%</span>
                                    </div>
                                    <div className="muted">{mod.reason}</div>
                                  </div>
                                ))}
                              </div>
                            ) : (
                              <div className="muted">No explicit failed mod lines detected.</div>
                            )}
                          </div>
                          <div className="instanceAnalyzeCard">
                            <div className="instanceAnalyzeCardTitle">Suspects</div>
                            {logAnalyzeResult && logAnalyzeResult.suspects.length > 0 ? (
                              <div className="instanceAnalyzeCardBody instanceAnalyzeList">
                                {logAnalyzeResult.suspects.slice(0, 6).map((suspect) => (
                                  <div key={suspect.id} className="rowBetween">
                                    <span>{suspect.label}</span>
                                    <span className="chip subtle">
                                      {suspect.matches} · {Math.round(suspect.confidence * 100)}%
                                    </span>
                                  </div>
                                ))}
                              </div>
                            ) : (
                              <div className="muted">No strong suspects detected yet.</div>
                            )}
                          </div>
                          <div className="instanceAnalyzeCard">
                            <div className="instanceAnalyzeCardTitle">Key errors</div>
                            {logAnalyzeResult && logAnalyzeResult.keyErrors.length > 0 ? (
                              <div className="instanceAnalyzeCardBody instanceAnalyzeList">
                                {logAnalyzeResult.keyErrors.map((line, idx) => (
                                  <div key={`${idx}:${line.slice(0, 24)}`} className="muted">{line}</div>
                                ))}
                              </div>
                            ) : (
                              <div className="muted">No error lines detected.</div>
                            )}
                          </div>
                          <div className="instanceAnalyzeCard">
                            <div className="instanceAnalyzeCardTitle">Confidence notes</div>
                            {logAnalyzeResult && (logAnalyzeResult.confidenceNotes?.length ?? 0) > 0 ? (
                              <div className="instanceAnalyzeCardBody instanceAnalyzeList">
                                {(logAnalyzeResult.confidenceNotes ?? []).map((note, idx) => (
                                  <div key={`${idx}:${note.slice(0, 18)}`} className="muted">{note}</div>
                                ))}
                              </div>
                            ) : (
                              <div className="muted">Run Analyze to generate confidence notes.</div>
                            )}
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </div>
            </section>

            {showInstanceActivityPane ? (
              <aside className="instanceSidePane">
                <ActivityFeed
                  entriesRaw={recentActivityEntriesRaw}
                  loading={Boolean(instanceHistoryBusyById[inst.id])}
                  filter={recentActivityFilter}
                  onFilterChange={(value) =>
                    setRecentActivityFilterByInstance((prev) => ({
                      ...prev,
                      [inst.id]: value,
                    }))
                  }
                  retentionLabel={recentActivityRetentionLabel}
                  onOpenFullHistory={() => openFullHistory(inst.id)}
                  onClearRecent={() => {
                    const now = Date.now();
                    setInstanceActivityById((prev) => ({
                      ...prev,
                      [inst.id]: [],
                    }));
                    setInstanceHistoryById((prev) => ({
                      ...prev,
                      [inst.id]: [],
                    }));
                    setTimelineClearedAtByInstance((prev) => ({
                      ...prev,
                      [inst.id]: now,
                    }));
                  }}
                  canClear={recentActivityEntriesRaw.length > 0}
                  windowMs={RECENT_ACTIVITY_COALESCE_WINDOW_MS}
                  limit={RECENT_ACTIVITY_LIMIT}
                  showEarlierBucket={showEarlierRecentActivityBucket}
                />
              <div className="card instanceSideCard instanceSessionCard">
                <div className="librarySideTitle">Session</div>
                <div className="instanceSessionStack">
                  <div className="instanceSessionBlock">
                    <div className="instanceSessionLabel">Runtime</div>
                    {hasRunningForInstance ? (
                      <>
                        <div className="instanceSessionValue">
                          {runningForInstance.length} running session{runningForInstance.length === 1 ? "" : "s"}
                        </div>
                        {hasDisposableRuntimeSession ? (
                          <div className="muted instanceSessionSub">
                            Extra native runs use disposable runtime sessions. Only Minecraft settings sync back.
                          </div>
                        ) : null}
                      </>
                    ) : (
                      <div className="compactEmptyState">
                        <span className="compactEmptyIcon" aria-hidden="true">
                          <Icon name="play" size={14} />
                        </span>
                        <div className="compactEmptyBody">
                          <div className="compactEmptyTitle">Nothing running right now</div>
                          <div className="compactEmptyText">Hit Play on this instance to start.</div>
                        </div>
                      </div>
                    )}
                  </div>
                  <div className="instanceSessionBlock">
                    <div className="instanceSessionLabel">Playing as</div>
                    {selectedLauncherAccount ? (
                      <>
                        <div className="libraryAccountName">{selectedLauncherAccount.username}</div>
                        <div className="libraryAccountId muted">{selectedLauncherAccount.id}</div>
                      </>
                    ) : (
                      <div className="muted instanceSessionSub">Select account in Settings or Account page.</div>
                    )}
                  </div>
                </div>
              </div>
              <div className="card instanceSideCard">
                <div className="librarySideTitle">File actions</div>
                <div className="libraryQuickActions">
                  <button className="btn" onClick={() => onOpenInstancePath(inst, "instance")}>
                    <Icon name="folder" size={16} />
                    Open instance folder
                  </button>
                  <button className="btn" onClick={() => onOpenInstancePath(inst, "mods")}>
                    <Icon name="folder" size={16} />
                    Open mods folder
                  </button>
                  <button className="btn" onClick={() => onExportModsZip(inst)}>
                    <Icon name="download" size={16} />
                    Export mods zip
                  </button>
                  <button className="btn" onClick={() => setSupportBundleModalInstanceId(inst.id)}>
                    <Icon name="download" size={16} />
                    Export support bundle
                  </button>
                </div>
              </div>
              <div className="card instanceSideCard">
                <div className="librarySideTitle">Quick Play</div>
                <div className="muted quickPlaySub">
                  Launch straight into a server with this instance.
                </div>
                <div className="quickPlayForm">
                  <label className="quickPlayField">
                    <span className="quickPlayLabel">Server name</span>
                    <input
                      className="input"
                      placeholder="My server"
                      value={quickPlayDraftName}
                      onChange={(e) => setQuickPlayDraftName(e.target.value)}
                      disabled={quickPlayBusy}
                    />
                  </label>
                  <label className="quickPlayField">
                    <span className="quickPlayLabel">Host</span>
                    <input
                      className="input"
                      placeholder="example.org"
                      value={quickPlayDraftHost}
                      onChange={(e) => setQuickPlayDraftHost(e.target.value)}
                      disabled={quickPlayBusy}
                    />
                  </label>
                  <div className="quickPlayMetaRow">
                    <label className="quickPlayField">
                      <span className="quickPlayLabel">Port</span>
                      <input
                        className="input quickPlayPortInput"
                        placeholder="25565"
                        value={quickPlayDraftPort}
                        onChange={(e) => setQuickPlayDraftPort(e.target.value)}
                        disabled={quickPlayBusy}
                      />
                    </label>
                    <div className="quickPlayField quickPlayBind">
                      <span className="quickPlayLabel">Bind</span>
                      <MenuSelect
                        value={quickPlayDraftBoundInstanceId}
                        labelPrefix="Bind"
                        buttonLabel={
                          quickPlayDraftBoundInstanceId === "none"
                            ? "Current instance"
                            : instances.find((entry) => entry.id === quickPlayDraftBoundInstanceId)?.name ||
                              "Current instance"
                        }
                        options={[
                          { value: "none", label: "Current instance" },
                          ...instances.map((entry) => ({
                            value: entry.id,
                            label: entry.name || entry.id,
                          })),
                        ]}
                        onChange={(value) => setQuickPlayDraftBoundInstanceId(String(value ?? "none"))}
                      />
                    </div>
                  </div>
                  <div className="quickPlayActions">
                    <button
                      className="btn primary quickPlaySaveBtn"
                      onClick={() => void onSaveQuickPlayServer(inst)}
                      disabled={quickPlayBusy}
                    >
                      {quickPlayBusy ? "Saving…" : "Save server"}
                    </button>
                    <button
                      className="btn subtle quickPlayRefreshBtn"
                      onClick={() => void refreshQuickPlayServers()}
                      disabled={quickPlayBusy}
                    >
                      Refresh
                    </button>
                  </div>
                </div>
                {quickPlayErr ? <div className="errorBox" style={{ marginTop: 8 }}>{quickPlayErr}</div> : null}
                {quickPlayServersForInstance.length > 0 ? (
                  <div className="settingListMini quickPlayList">
                    {quickPlayServersForInstance.map((server) => (
                      <div key={server.id} className="settingListMiniRow">
                        <div style={{ minWidth: 0 }}>
                          <div style={{ fontWeight: 900 }}>{server.name}</div>
                          <div className="muted">
                            {server.host}:{server.port}
                          </div>
                        </div>
                        <div className="quickPlayServerActions">
                          <button
                            className="btn"
                            onClick={() => void onLaunchQuickPlayServer(server, inst)}
                            disabled={quickPlayBusy}
                          >
                            Play
                          </button>
                          <button
                            className="btn danger"
                            onClick={() => void onRemoveQuickPlayServer(server.id)}
                            disabled={quickPlayBusy}
                          >
                            Remove
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="compactEmptyState compactEmptyStateInline" style={{ marginTop: 8 }}>
                    <span className="compactEmptyIcon" aria-hidden="true">
                      <Icon name="upload" size={14} />
                    </span>
                    <div className="compactEmptyBody">
                      <div className="compactEmptyTitle">No quick-play servers yet</div>
                      <div className="compactEmptyText">Save a server above for one-click launches.</div>
                    </div>
                  </div>
                )}
              </div>
              </aside>
            ) : null}
          </div>

          {instanceLinksOpen && (
            <Modal
              title={`${inst.name || "Instance"} links`}
              titleNode={
                <div className="instSettingsCrumb">
                  <span className="instSettingsCrumbIcon" aria-hidden="true">
                    <Icon name="layers" size={15} />
                  </span>
                  <span className="instSettingsCrumbName">{inst.name || "Instance"}</span>
                  <span className="instSettingsCrumbSep">›</span>
                  <span className="instSettingsCrumbLabel">Links</span>
                </div>
              }
              onClose={() => setInstanceLinksOpen(false)}
              size="wide"
            >
              <div className="modalBody">
                <InstanceModpackCard
                  instance={inst}
                  isDevMode={isDevMode}
                  enableAutoSync={false}
                  onNotice={(message) => setInstallNotice(message)}
                  onError={(message) => setError(message)}
                  onFriendStatusChange={(instanceId, status) => {
                    setFriendLinkStatusByInstance((prev) => {
                      const next = { ...prev };
                      if (status) next[instanceId] = status;
                      else delete next[instanceId];
                      return next;
                    });
                    if (selectedId === instanceId) {
                      setInstanceFriendLinkStatus(status);
                    }
                  }}
                  onDriftPreviewChange={(instanceId, preview) => {
                    setFriendLinkDriftByInstance((prev) => {
                      const next = { ...prev };
                      if (preview) next[instanceId] = preview;
                      else delete next[instanceId];
                      return next;
                    });
                    const signature = friendLinkDriftSignature(preview);
                    if (signature) friendLinkDriftAnnounceRef.current[instanceId] = signature;
                    else delete friendLinkDriftAnnounceRef.current[instanceId];
                  }}
                  onActivity={(instanceId, message, tone) => appendInstanceActivity(instanceId, [message], tone)}
                  onFriendConflict={(instanceId, result) => {
                    setFriendConflictInstanceId(instanceId);
                    setFriendConflictResult(result);
                  }}
                  onContentSync={(instanceId) => {
                    if (route === "instance" && selectedId === instanceId) {
                      void refreshInstalledMods(instanceId);
                    }
                  }}
                />
              </div>
            </Modal>
          )}

          {instanceSettingsOpen && (
            <Modal
              title={`${inst.name || "Instance"} settings`}
              className="instanceSettingsModal"
              titleNode={
                <div className="instSettingsCrumb">
                  <span className="instSettingsCrumbIcon" aria-hidden="true">
                    <Icon name="box" size={15} />
                  </span>
                  <span className="instSettingsCrumbName">{inst.name || "Instance"}</span>
                  <span className="instSettingsCrumbSep">›</span>
                  <span className="instSettingsCrumbLabel">Settings</span>
                </div>
              }
              onClose={() => setInstanceSettingsOpen(false)}
              size="wide"
            >
              <div className="modalBody instSettingsModalBody">
                <div className="instSettings">
                  <div className="instSettingsNav">
                    {[
                      { id: "general", label: "General", icon: "sliders", advanced: false },
                      { id: "installation", label: "Installation", icon: "box", advanced: false },
                      { id: "graphics", label: "Window", icon: "sparkles", advanced: false },
                      { id: "java", label: "Java and memory", icon: "cpu", advanced: false },
                      { id: "content", label: "Launch hooks", icon: "layers", advanced: true },
                    ]
                      .filter((item) => instanceSettingsMode === "advanced" || !item.advanced)
                      .map((s) => (
                    <button
                      key={s.id}
                      className={"instSettingsNavItem" + (instanceSettingsSection === s.id ? " active" : "")}
                      onClick={() => setInstanceSettingsSection(s.id as "general" | "installation" | "java" | "graphics" | "content")}
                    >
                      <span className="navIco">
                        <Icon name={s.icon as any} size={18} />
                      </span>
                      {s.label}
                    </button>
                    ))}

                  <div className="instSettingsNavMeta">
                    <div className="instSettingsNavMetaLabel">Editing mode</div>
                    <SegmentedControl
                      value={instanceSettingsMode}
                      onChange={(value) =>
                        setInstanceSettingsMode(((value ?? "basic") as SettingsMode))
                      }
                      options={[
                        { value: "basic", label: "Basic" },
                        { value: "advanced", label: "Advanced" },
                      ]}
                    />
                    <div className="instSettingsNavMetaStatus">
                      <span className={`chip ${instanceSettingsBusy ? "" : "subtle"}`}>
                        {instanceSettingsBusy ? "Saving…" : "Auto-save on change"}
                      </span>
                      <div className="muted">Changes are saved as you edit this instance.</div>
                    </div>
                  </div>

                  <div className="instSettingsNavFooter">
                    <button
                      className="btn danger"
                      onClick={() => {
                        requestDelete(inst);
                      }}
                    >
                      <span className="btnIcon">
                        <Icon name="trash" size={18} />
                      </span>
                      Delete instance
                    </button>
                  </div>
                </div>

                  <div className="instSettingsBody">
                  <div className="instSettingsStatusRow">
                    <span className="chip subtle">Auto-save on toggle/change</span>
                    <SegmentedControl
                      value={instanceSettingsMode}
                      onChange={(value) =>
                        setInstanceSettingsMode(((value ?? "basic") as SettingsMode))
                      }
                      options={[
                        { value: "basic", label: "Basic" },
                        { value: "advanced", label: "Advanced" },
                      ]}
                    />
                    {instanceSettingsBusy ? <span className="chip">Saving…</span> : <span className="chip">Saved</span>}
                  </div>
                  {instanceSettingsSection === "general" && (
                    <>
                      <div className="h2 sectionHead" id="setting-anchor-instance:general">
                        General
                      </div>

                      <div className="settingGrid">
                        <div className="settingCard">
                          <div className="settingTitle">Name</div>
                          <div className="settingSub">Displayed in Library and sidebar.</div>
                          <input
                            className="input"
                            value={instanceNameDraft}
                            onChange={(e) => setInstanceNameDraft(e.target.value)}
                            onBlur={() => void onCommitInstanceName(inst)}
                            placeholder="Instance name"
                            disabled={instanceSettingsBusy}
                          />
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">Icon</div>
                          <div className="settingSub">Used for quick access in the sidebar.</div>
                          <div className="instanceIconPreviewRow">
                            <div className="instCardIcon">
                              {inst.icon_path ? (
                                <LocalImage path={inst.icon_path} alt="" fallback={<Icon name="box" size={19} />} />
                              ) : (
                                <Icon name="box" size={19} />
                              )}
                            </div>
                            <div className="muted">
                              {inst.icon_path ? "Custom icon selected" : "Using default icon"}
                            </div>
                          </div>
                          <div className="row">
                            <button className="btn" onClick={() => void onSelectInstanceIcon(inst)} disabled={busy === "instance-icon" || instanceSettingsBusy}>
                              <span className="btnIcon">
                                <Icon name="upload" size={18} />
                              </span>
                              Select icon
                            </button>
                            <button className="btn" onClick={() => void onRemoveInstanceIcon(inst)} disabled={busy === "instance-icon" || instanceSettingsBusy || !inst.icon_path}>
                              <span className="btnIcon">
                                <Icon name="x" size={18} />
                              </span>
                              Remove icon
                            </button>
                          </div>
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">Notes</div>
                          <div className="settingSub">Personal reminder for this instance.</div>
                          <textarea
                            className="textarea"
                            value={instanceNotesDraft}
                            onChange={(e) => setInstanceNotesDraft(e.target.value)}
                            onBlur={() => void onCommitInstanceNotes(inst)}
                            placeholder="Write a quick note…"
                            disabled={instanceSettingsBusy}
                          />
                        </div>
                      </div>
                    </>
                  )}

                  {instanceSettingsSection === "installation" && (
                    <>
                      <div className="h2 sectionHead" id="setting-anchor-instance:installation">
                        Installation
                      </div>

                      <div className="settingGrid">
                        <div className="settingCard">
                          <div className="settingTitle">Loader</div>
                          <div className="settingSub">Switch between supported loaders for this instance.</div>
                          <SegmentedControl
                            value={inst.loader}
                            onChange={(v) => {
                              const nextLoader = (v ?? inst.loader) as Loader;
                              if (nextLoader === inst.loader) return;
                              void persistInstanceChanges(inst, { loader: nextLoader }, `Loader set to ${nextLoader}.`);
                            }}
                            options={[
                              { label: "Vanilla", value: "vanilla" },
                              { label: "Fabric", value: "fabric" },
                              { label: "Forge", value: "forge" },
                              { label: "NeoForge", value: "neoforge" },
                              { label: "Quilt", value: "quilt" },
                            ]}
                            variant="scroll"
                          />
                        </div>

                        <div className="settingCard settingCardVersion">
                          <MenuSelect
                            value={inst.mc_version}
                            labelPrefix="Version"
                            onChange={(v) => {
                              if (v === inst.mc_version) return;
                              void persistInstanceChanges(inst, { mcVersion: v }, `Minecraft version set to ${v}.`);
                            }}
                            options={instanceVersionOptions}
                            placement="top"
                          />
                          <div className="settingTitle settingTitleAfterControl">Game version</div>
                          <div className="settingSub">Shown in Discover filters and install prompts.</div>
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">Instance location</div>
                          <div className="settingSub">Where files are stored on disk.</div>
                          <div className="pathRow">
                            <input
                              className="input"
                              value={`Instance ID: ${inst.id}`}
                              readOnly
                            />
                            <button className="btn" onClick={() => void onOpenInstancePath(inst, "instance")}>
                              <span className="btnIcon">
                                <Icon name="folder" size={18} />
                              </span>
                              Open
                            </button>
                          </div>
                        </div>

	                        <div className="settingCard">
	                          <div className="settingTitle">Updates</div>
	                          <div className="settingSub">Control install and update behavior for this instance.</div>
                          <label className="toggleRow">
                            <input
                              type="checkbox"
                              checked={instSettings.auto_update_installed_content}
                              onChange={(e) =>
                                void persistInstanceChanges(
                                  inst,
                                  { settings: { auto_update_installed_content: e.target.checked } },
                                  "Update preference saved."
                                )
                              }
                              disabled={instanceSettingsBusy}
                            />
                            <span className="togglePill" />
                            <span>Auto-update installed content</span>
                          </label>
                          <label className="toggleRow">
                            <input
                              type="checkbox"
                              checked={instSettings.prefer_release_builds}
                              onChange={(e) =>
                                void persistInstanceChanges(
                                  inst,
                                  { settings: { prefer_release_builds: e.target.checked } },
                                  "Update preference saved."
                                )
                              }
                              disabled={instanceSettingsBusy}
                            />
                            <span className="togglePill" />
	                            <span>Prefer release builds</span>
	                          </label>
	                        </div>

	                        <div className="settingCard">
	                          <div className="settingTitle">Minecraft settings sync</div>
	                          <div className="settingSub">
	                            Keep your in-game options aligned across instances.
	                          </div>
	                          <label className="toggleRow">
	                            <input
	                              type="checkbox"
	                              checked={instSettings.sync_minecraft_settings}
	                              onChange={(e) =>
	                                void persistInstanceChanges(
	                                  inst,
	                                  { settings: { sync_minecraft_settings: e.target.checked } },
	                                  `Minecraft settings sync ${e.target.checked ? "enabled" : "disabled"}.`
	                                )
	                              }
	                              disabled={instanceSettingsBusy}
	                            />
	                            <span className="togglePill" />
	                            <span>Sync on launch</span>
	                          </label>
	                          {instSettings.sync_minecraft_settings ? (
	                            <>
                              {instances.filter((item) => item.id !== inst.id).length > 0 ? (
                                <MenuSelect
                                  value={
                                    instSettings.sync_minecraft_settings_target === "none" ||
                                    instSettings.sync_minecraft_settings_target === "all" ||
                                    instances.some((item) => item.id === instSettings.sync_minecraft_settings_target)
                                      ? instSettings.sync_minecraft_settings_target
                                      : "none"
                                  }
                                  labelPrefix="Sync target"
                                  onChange={(v) =>
                                    void persistInstanceChanges(
                                      inst,
                                      { settings: { sync_minecraft_settings_target: String(v ?? "none") } },
                                      `Minecraft settings sync target updated.`
                                    )
                                  }
                                  options={[
                                    { value: "none", label: "No target selected" },
                                    ...instances
                                      .filter((item) => item.id !== inst.id)
                                      .map((item) => ({
                                        value: item.id,
                                        label: `Only ${item.name || item.id}`,
                                      })),
                                    ...(instanceSettingsMode === "advanced"
                                      ? [{ value: "all", label: "All instances (advanced)" }]
                                      : []),
                                  ]}
                                />
                              ) : null}
			                              <div className="muted" style={{ marginTop: 8 }}>
			                                {instances.filter((item) => item.id !== inst.id).length === 0
			                                  ? "Add another instance to choose a sync target."
			                                  : instSettings.sync_minecraft_settings_target === "all"
                                      ? "Advanced mode fan-out: syncs options files to every other instance."
                                      : instSettings.sync_minecraft_settings_target === "none"
                                        ? "Choose one target instance to avoid accidental fan-out overwrites."
			                                  : "Syncs options files (options.txt, optionsof.txt, optionsshaders.txt, servers.dat) right before launch."}
			                              </div>
		                            </>
		                          ) : null}
	                        </div>
	                      </div>
	                    </>
	                  )}

                  {instanceSettingsSection === "java" && (
                    <>
                      <div className="h2 sectionHead" id="setting-anchor-instance:java">
                        Java and memory
                      </div>

                      <div className="settingGrid">
                        {instanceSettingsMode === "advanced" ? (
                          <div className="settingCard" id="setting-anchor-instance:java-runtime">
                            <div className="settingTitle">Java runtime</div>
                            <div className="settingSub">
                              Use a per-instance override, or leave blank to use launcher default.
                            </div>
                            <input
                              className="input"
                              value={instanceJavaPathDraft}
                              onChange={(e) => setInstanceJavaPathDraft(e.target.value)}
                              onBlur={() => void onCommitInstanceJavaPath(inst)}
                              placeholder="Blank = use launcher Java path"
                              disabled={instanceSettingsBusy}
                            />
                            <div className="row">
                              <button className="btn" onClick={() => void onPickInstanceJavaPath(inst)} disabled={instanceSettingsBusy}>
                                <span className="btnIcon">
                                  <Icon name="upload" size={17} />
                                </span>
                                Browse…
                              </button>
                              <button className="btn" onClick={() => void refreshJavaRuntimeCandidates()} disabled={javaRuntimeBusy}>
                                {javaRuntimeBusy ? "Detecting…" : "Detect runtimes"}
                              </button>
                              <button
                                className="btn"
                                onClick={() => void openExternalLink("https://adoptium.net/temurin/releases/?version=21")}
                              >
                                Get Java 21
                              </button>
                            </div>
                            <div className="muted" style={{ marginTop: 8 }}>
                              Minecraft {inst.mc_version} requires Java {requiredJavaMajor}+.
                            </div>
                            {javaRuntimeCandidates.length > 0 ? (
                              <div className="settingListMini">
                                {javaRuntimeCandidates.slice(0, 5).map((runtime) => (
                                  <div key={runtime.path} className="settingListMiniRow">
                                    <div style={{ minWidth: 0 }}>
                                      <div style={{ fontWeight: 900 }}>{javaRuntimeDisplayLabel(runtime)}</div>
                                      <div className="muted" style={{ wordBreak: "break-all" }}>{runtime.path}</div>
                                    </div>
                                    <button
                                      className={`btn stateful ${instanceJavaPathDraft.trim() === runtime.path.trim() ? "active" : ""}`}
                                      onClick={() => {
                                        setInstanceJavaPathDraft(runtime.path);
                                        void persistInstanceChanges(
                                          inst,
                                          { settings: { java_path: runtime.path } },
                                          "Instance Java path updated."
                                        );
                                      }}
                                      disabled={instanceSettingsBusy}
                                    >
                                      {instanceJavaPathDraft.trim() === runtime.path.trim() ? "Selected" : "Use"}
                                    </button>
                                  </div>
                                ))}
                              </div>
                            ) : null}
                          </div>
                        ) : (
                          <div className="settingCard">
                            <div className="settingTitle">Java runtime override</div>
                            <div className="settingSub">
                              Hidden in Basic mode. Switch to Advanced to set per-instance Java executable.
                            </div>
                          </div>
                        )}

                        <div className="settingCard" id="setting-anchor-instance:java-memory">
                          <div className="settingTitle">Memory</div>
                          <div className="settingSub">Set Java heap size in MB for this instance.</div>
                          <div className="row">
                            <input
                              className="input"
                              type="number"
                              min={512}
                              max={65536}
                              step={256}
                              value={instanceMemoryDraft}
                              onChange={(e) => setInstanceMemoryDraft(e.target.value)}
                              onBlur={() => void onCommitInstanceMemory(inst)}
                              disabled={instanceSettingsBusy}
                            />
                            <button
                              className="btn"
                              onClick={() => {
                                setInstanceMemoryDraft("4096");
                                void persistInstanceChanges(inst, { settings: { memory_mb: 4096 } }, "Memory reset to 4096 MB.");
                              }}
                              disabled={instanceSettingsBusy}
                            >
                              Reset
                            </button>
                          </div>
                          <div className="row">
                            {[2048, 4096, 6144, 8192].map((presetMb) => (
                              <button
                                key={presetMb}
                                className={`btn stateful ${Number(instanceMemoryDraft) === presetMb ? "active" : ""}`}
                                onClick={() => {
                                  setInstanceMemoryDraft(String(presetMb));
                                  void persistInstanceChanges(
                                    inst,
                                    { settings: { memory_mb: presetMb } },
                                    `Memory set to ${presetMb} MB.`
                                  );
                                }}
                                disabled={instanceSettingsBusy}
                              >
                                {Math.round(presetMb / 1024)} GB
                              </button>
                            ))}
                          </div>
                          <div className="muted" style={{ marginTop: 8 }}>
                            Recommended: 4096 MB for medium packs, 6144-8192 MB for heavier packs.
                          </div>
                        </div>

                        {instanceSettingsMode === "advanced" ? (
                          <div className="settingCard" id="setting-anchor-instance:jvm-args">
                            <div className="settingTitle">JVM arguments</div>
                            <div className="settingSub">Advanced users only. Saved per instance.</div>
                            <textarea
                              className="textarea"
                              placeholder="-XX:+UseG1GC -XX:MaxGCPauseMillis=80"
                              value={instanceJvmArgsDraft}
                              onChange={(e) => setInstanceJvmArgsDraft(e.target.value)}
                              onBlur={() => void onCommitInstanceJvmArgs(inst)}
                              disabled={instanceSettingsBusy}
                            />
                          </div>
                        ) : null}

                        {autoProfileRecommendation ? (
                          <div className="settingCard">
                            <div className="settingTitle">Smart auto-profile recommendation</div>
                            <div className="settingSub">
                              Suggestion based on enabled mod count and recent launch outcomes.
                            </div>
                            <div className="row" style={{ marginTop: 8, gap: 8, flexWrap: "wrap" }}>
                              <span className="chip subtle">{autoProfileRecommendation.confidence} confidence</span>
                              <span className="chip">{Math.round(autoProfileRecommendation.memory_mb / 1024)} GB RAM</span>
                              <span className="chip subtle">{autoProfileRecommendation.graphics_preset}</span>
                            </div>
                            <div className="muted" style={{ marginTop: 8 }}>
                              {autoProfileRecommendation.reasons.join(" • ") || "Balanced recommendation."}
                            </div>
                            <div className="muted" style={{ marginTop: 8, wordBreak: "break-word" }}>
                              JVM args: <code>{autoProfileRecommendation.jvm_args}</code>
                            </div>
                            {autoProfileAppliedAt ? (
                              <div className="muted" style={{ marginTop: 6 }}>
                                Last applied {formatDateTime(autoProfileAppliedAt, "recently")}
                              </div>
                            ) : null}
                            <div className="row">
                              <button
                                className="btn primary"
                                onClick={() => applyAutoProfileRecommendation(inst, autoProfileRecommendation)}
                                disabled={instanceSettingsBusy}
                              >
                                Apply recommendation
                              </button>
                            </div>
                          </div>
                        ) : null}
                      </div>
                    </>
                  )}

                  {instanceSettingsSection === "graphics" && (
                    <>
                      <div className="h2 sectionHead" id="setting-anchor-instance:graphics">
                        Window
                      </div>

                      <div className="settingGrid">
                        <div className="settingCard">
                          <div className="settingTitle">Window behavior</div>
                          <div className="settingSub">Saved per instance. Off now minimizes the launcher instead of hiding it, so logs stay accessible.</div>
                          <label className="toggleRow">
                            <input
                              type="checkbox"
                              checked={instSettings.keep_launcher_open_while_playing}
                              onChange={(e) =>
                                void persistInstanceChanges(
                                  inst,
                                  { settings: { keep_launcher_open_while_playing: e.target.checked } },
                                  "Window behavior saved."
                                )
                              }
                              disabled={instanceSettingsBusy}
                            />
                            <span className="togglePill" />
                            <span>Keep launcher open while playing</span>
                          </label>
                          <label className="toggleRow">
                            <input
                              type="checkbox"
                              checked={instSettings.close_launcher_on_game_exit}
                              onChange={(e) =>
                                void persistInstanceChanges(
                                  inst,
                                  { settings: { close_launcher_on_game_exit: e.target.checked } },
                                  "Window behavior saved."
                                )
                              }
                              disabled={instanceSettingsBusy}
                            />
                            <span className="togglePill" />
                            <span>Close launcher on game exit</span>
                          </label>
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">Visual preset</div>
                          <div className="settingSub">Controls optional graphics defaults for this instance.</div>
                          <MenuSelect
                            value={instSettings.graphics_preset}
                            labelPrefix="Preset"
                            onChange={(v) =>
                              void persistInstanceChanges(
                                inst,
                                {
                                  settings: {
                                    graphics_preset: v,
                                  },
                                },
                                `Graphics preset set to ${v}.`
                              )
                            }
                            options={[
                              { value: "Performance", label: "Performance" },
                              { value: "Balanced", label: "Balanced" },
                              { value: "Quality", label: "Quality" },
                            ]}
                          />
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">Optional display features</div>
                          <div className="settingSub">Toggles that can improve image quality at runtime.</div>
                          <label className="toggleRow">
                            <input
                              type="checkbox"
                              checked={instSettings.enable_shaders}
                              onChange={(e) =>
                                void persistInstanceChanges(
                                  inst,
                                  { settings: { enable_shaders: e.target.checked } },
                                  "Graphics preference saved."
                                )
                              }
                              disabled={instanceSettingsBusy}
                            />
                            <span className="togglePill" />
                            <span>Enable shaders</span>
                          </label>
                          <label className="toggleRow">
                            <input
                              type="checkbox"
                              checked={instSettings.force_vsync}
                              onChange={(e) =>
                                void persistInstanceChanges(
                                  inst,
                                  { settings: { force_vsync: e.target.checked } },
                                  "Graphics preference saved."
                                )
                              }
                              disabled={instanceSettingsBusy}
                            />
                            <span className="togglePill" />
                            <span>Force vsync</span>
                          </label>
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">World safety backups</div>
                          <div className="settingSub">
                            Auto-back up worlds while Minecraft is running for this instance. Changes apply on next launch.
                          </div>
                          <MenuSelect
                            value={String(instSettings.world_backup_interval_minutes)}
                            labelPrefix="Interval"
                            onChange={(v) =>
                              void persistInstanceChanges(
                                inst,
                                { settings: { world_backup_interval_minutes: Number(v) } },
                                "World backup interval saved."
                              )
                            }
                            options={WORLD_BACKUP_INTERVAL_OPTIONS}
                          />
                          <div style={{ height: 8 }} />
                          <MenuSelect
                            value={String(instSettings.world_backup_retention_count)}
                            labelPrefix="Retention"
                            onChange={(v) =>
                              void persistInstanceChanges(
                                inst,
                                { settings: { world_backup_retention_count: Number(v) } },
                                "World backup retention saved."
                              )
                            }
                            options={WORLD_BACKUP_RETENTION_OPTIONS}
                          />
                          <div className="muted" style={{ marginTop: 8 }}>
                            Backups run every {instSettings.world_backup_interval_minutes} min and keep{" "}
                            {instSettings.world_backup_retention_count} per world.
                          </div>
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">Snapshot retention</div>
                          <div className="settingSub">
                            Control how many instance snapshots to keep and how long they are retained.
                          </div>
                          <MenuSelect
                            value={String(instSettings.snapshot_retention_count)}
                            labelPrefix="Count"
                            onChange={(v) =>
                              void persistInstanceChanges(
                                inst,
                                { settings: { snapshot_retention_count: Number(v) } },
                                "Snapshot retention saved."
                              )
                            }
                            options={SNAPSHOT_RETENTION_OPTIONS}
                          />
                          <div style={{ height: 8 }} />
                          <MenuSelect
                            value={String(instSettings.snapshot_max_age_days)}
                            labelPrefix="Age"
                            onChange={(v) =>
                              void persistInstanceChanges(
                                inst,
                                { settings: { snapshot_max_age_days: Number(v) } },
                                "Snapshot retention saved."
                              )
                            }
                            options={SNAPSHOT_MAX_AGE_OPTIONS}
                          />
                          <div className="muted" style={{ marginTop: 8 }}>
                            Keep up to {instSettings.snapshot_retention_count} snapshots and auto-delete after{" "}
                            {instSettings.snapshot_max_age_days} days.
                          </div>
                        </div>
                      </div>
                    </>
                  )}

                  {instanceSettingsMode === "advanced" && instanceSettingsSection === "content" && (
                    <>
                      <div className="h2 sectionHead" id="setting-anchor-instance:hooks">
                        Launch hooks
                      </div>

                      <div className="settingGrid">
                        <div className="settingCard">
                          <div className="settingTitle">Game launch hooks</div>
                          <div className="settingSub">
                            Hooks run system commands before and after launching Minecraft for this instance.
                          </div>
                          <label className="toggleRow">
                            <input
                              type="checkbox"
                              checked={launchHooksDraft.enabled}
                              onChange={(e) => setLaunchHooksDraft({ enabled: e.target.checked })}
                            />
                            <span className="togglePill" />
                            <span>Custom launch hooks</span>
                          </label>
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">Pre-launch</div>
                          <div className="settingSub">Runs before the instance is launched.</div>
                          <input
                            className="input"
                            value={launchHooksDraft.pre_launch}
                            onChange={(e) => setLaunchHooksDraft({ pre_launch: e.target.value })}
                            placeholder="Enter pre-launch command..."
                            disabled={!launchHooksDraft.enabled}
                          />
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">Wrapper</div>
                          <div className="settingSub">Wrapper command used for launching Minecraft.</div>
                          <input
                            className="input"
                            value={launchHooksDraft.wrapper}
                            onChange={(e) => setLaunchHooksDraft({ wrapper: e.target.value })}
                            placeholder="Enter wrapper command..."
                            disabled={!launchHooksDraft.enabled}
                          />
                        </div>

                        <div className="settingCard">
                          <div className="settingTitle">Post-exit</div>
                          <div className="settingSub">Runs after the game closes.</div>
                          <input
                            className="input"
                            value={launchHooksDraft.post_exit}
                            onChange={(e) => setLaunchHooksDraft({ post_exit: e.target.value })}
                            placeholder="Enter post-exit command..."
                            disabled={!launchHooksDraft.enabled}
                          />
                        </div>
                      </div>
                    </>
                  )}
                  </div>
                </div>
              </div>
            </Modal>
          )}
        </div>
      );
    }

    if (route === "skins") {
      return (
        <div className="page">
          <div style={{ maxWidth: 1360 }}>
            <div className="h1">Skins</div>
            <div className="p">Manage skins and capes with live 3D preview.</div>

            <div className="accountSkinsStudio accountSkinsStudioLibrary skinsRouteLayoutRef card">
              <div className="accountSkinViewerPane">
                <div className="accountSkinTitleRow">
                  <div className="accountSkinHeading">Skins</div>
                  <span className="accountSkinBeta">Beta</span>
                </div>
                <div className="accountSkinSub">Interactive 3D preview. Drag to rotate your player.</div>
                <div className="accountSkinNamePlate" title={selectedLauncherAccount?.username ?? "No account connected"}>
                  {selectedLauncherAccount?.username ?? "No account connected"}
                </div>
                <div
                  ref={accountSkinViewerStageRef}
                  className="accountSkinViewerStage"
                  style={skinViewerShadowStyle}
                >
                  <canvas ref={accountSkinViewerCanvasRef} className="accountSkinViewerCanvas" />
                  <div className="accountSkinViewerShadow" />
                </div>
                {skinViewerErr ? <div className="errorBox">{skinViewerErr}</div> : null}
                <div className="accountSkinViewerHint">{skinViewerHintText}</div>
                {!skinPreviewEnabled ? (
                  <div className="row" style={{ marginTop: 8 }}>
                    <button className="btn" onClick={() => setSkinPreviewEnabled(true)}>
                      Enable 3D preview
                    </button>
                  </div>
                ) : null}
                <div className="accountSkinViewerActions">
                  <button
                    className="btn primary"
                    onClick={() => void onApplySelectedAppearance()}
                    disabled={accountAppearanceBusy || !selectedLauncherAccountId || !selectedAccountSkin}
                  >
                    {accountAppearanceBusy ? "Applying…" : "Apply skin & cape in-game"}
                  </button>
                  <button
                    className="btn"
                    onClick={onPlaySkinViewerEmote}
                    disabled={!skinPreviewEnabled || skinViewerPreparing}
                  >
                    Play emote
                  </button>
                  <button className="btn" onClick={onCycleAccountCape} disabled={capeOptions.length <= 1}>
                    Change cape
                  </button>
                  {selectedAccountSkin?.origin === "custom" ? (
                    <button className="btn danger" onClick={onRemoveSelectedCustomSkin}>
                      Remove skin
                    </button>
                  ) : null}
                </div>
                <div className="accountSkinViewerHint">
                  Cape: {selectedAccountCape?.label ?? "No cape"}
                </div>
              </div>

              <div className="accountSkinLibraryPane skinsLibraryPane skinsLibraryRef">
                <div className="skinsRefHeadRow">
                  <div className="skinsRefSectionTitle">Saved skins</div>
                  <div className="skinsLibraryStats">
                    <span className="chip subtle">{savedSkinOptions.length} saved</span>
                    <span className="chip subtle">{defaultSkinOptions.length} default</span>
                  </div>
                </div>

                <div className="skinsLibrarySelection skinsLibrarySelectionRef">
                  <span className="chip">Selected</span>
                  <strong>{selectedAccountSkin?.label ?? "None"}</strong>
                  <span className="skinsLibrarySelectionMeta">
                    {selectedAccountSkin?.origin === "custom"
                      ? "Custom"
                      : selectedAccountSkin?.origin === "profile"
                        ? "Profile"
                        : "Default"}
                  </span>
                  {selectedAccountSkin?.origin === "custom" ? (
                    <div className="skinsLibraryRenameRow">
                      <input
                        className="input skinsLibraryRenameInput"
                        value={skinRenameDraft}
                        onChange={(event) => setSkinRenameDraft(event.target.value)}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            event.preventDefault();
                            onRenameSelectedCustomSkin();
                          }
                        }}
                        placeholder="Rename selected skin"
                      />
                      <button
                        className="btn"
                        onClick={onRenameSelectedCustomSkin}
                        disabled={
                          !skinRenameDraft.trim() ||
                          skinRenameDraft.trim() === (selectedAccountSkin.label ?? "").trim()
                        }
                      >
                        Rename
                      </button>
                    </div>
                  ) : null}
                </div>

                <div className="accountSkinCardGrid accountSkinCardGridSaved skinsRefSavedGrid">
                  <button className="accountSkinAddCard accountSkinAddCardRef" onClick={onAddCustomSkin}>
                    <span className="accountSkinAddPlus">+</span>
                    <span>Add a skin</span>
                  </button>
                  {savedSkinOptions.map((skin) => {
                    const active = selectedAccountSkin?.id === skin.id;
                    const thumbSet = accountSkinThumbs[skin.id];
                    const frontThumb =
                      thumbSet?.front ??
                      toLocalIconSrc(skin.preview_url) ??
                      toLocalIconSrc(skin.skin_url) ??
                      "";
                    const backThumb = thumbSet?.back ?? frontThumb;
                    return (
                      <button
                        key={skin.id}
                        className={`accountSkinChoiceCard skinChoiceSaved skinChoiceSavedRef ${active ? "active" : ""}`}
                        onClick={() => setSelectedAccountSkinId(skin.id)}
                      >
                        {active ? (
                          <span className="accountSkinSelectedCheck" aria-hidden="true">
                            <Icon name="check_circle" size={15} />
                          </span>
                        ) : null}
                        <div className="accountSkinChoiceThumb">
                          <div className="accountSkinChoiceThumbInner">
                            <div className="accountSkinChoiceFace accountSkinChoiceFaceFront">
                              {frontThumb ? (
                                <img src={frontThumb} alt={`${skin.label} front preview`} />
                              ) : (
                                <span>{skin.label.slice(0, 1).toUpperCase()}</span>
                              )}
                            </div>
                            <div className="accountSkinChoiceFace accountSkinChoiceFaceBack">
                              {backThumb ? (
                                <img src={backThumb} alt={`${skin.label} back preview`} />
                              ) : (
                                <span>{skin.label.slice(0, 1).toUpperCase()}</span>
                              )}
                            </div>
                          </div>
                        </div>
                        <div className="accountSkinChoiceLabel">{skin.label}</div>
                        <div className="accountSkinChoiceMeta">
                          {skin.origin === "custom" ? "Custom" : "Profile"}
                        </div>
                      </button>
                    );
                  })}
                </div>

                <div className="skinsRefSectionTitle skinsRefDefaultTitle">Default skins</div>
                <div className="accountSkinCardGrid accountSkinCardGridDefault skinsRefDefaultGrid">
                  {defaultSkinOptions.map((skin) => {
                    const active = selectedAccountSkin?.id === skin.id;
                    const thumbSet = accountSkinThumbs[skin.id];
                    const frontThumb =
                      thumbSet?.front ??
                      toLocalIconSrc(skin.preview_url) ??
                      toLocalIconSrc(skin.skin_url) ??
                      "";
                    const backThumb = thumbSet?.back ?? frontThumb;
                    return (
                      <button
                        key={skin.id}
                        className={`accountSkinChoiceCard skinChoiceCompact skinChoiceCompactRef ${active ? "active" : ""}`}
                        onClick={() => setSelectedAccountSkinId(skin.id)}
                      >
                        {active ? (
                          <span className="accountSkinSelectedCheck" aria-hidden="true">
                            <Icon name="check_circle" size={15} />
                          </span>
                        ) : null}
                        <div className="accountSkinChoiceThumb">
                          <div className="accountSkinChoiceThumbInner">
                            <div className="accountSkinChoiceFace accountSkinChoiceFaceFront">
                              {frontThumb ? (
                                <img src={frontThumb} alt={`${skin.label} front preview`} />
                              ) : (
                                <span>{skin.label.slice(0, 1).toUpperCase()}</span>
                              )}
                            </div>
                            <div className="accountSkinChoiceFace accountSkinChoiceFaceBack">
                              {backThumb ? (
                                <img src={backThumb} alt={`${skin.label} back preview`} />
                              ) : (
                                <span>{skin.label.slice(0, 1).toUpperCase()}</span>
                              )}
                            </div>
                          </div>
                        </div>
                        <div className="accountSkinChoiceLabel">{skin.label}</div>
                        <div className="accountSkinChoiceMeta">Default</div>
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>
          </div>
        </div>
      );
    }

    // Library (dashboard layout + custom context menu)
    const loaderLabelFor = (inst: Instance) =>
      inst.loader === "neoforge"
        ? "NeoForge"
        : inst.loader === "fabric"
          ? "Fabric"
          : inst.loader === "forge"
            ? "Forge"
            : inst.loader === "quilt"
              ? "Quilt"
              : "Vanilla";

    const visibleInstances =
      libraryScope === "downloaded"
        ? []
        : instances.filter((x) => x.name.toLowerCase().includes(libraryQuery.toLowerCase()));

    const filtered = [...visibleInstances].sort((a, b) => {
      if (librarySort === "name") {
        return a.name.localeCompare(b.name, undefined, { sensitivity: "base", numeric: true });
      }
      const bTs = parseDateLike(b.created_at)?.getTime() ?? 0;
      const aTs = parseDateLike(a.created_at)?.getTime() ?? 0;
      return bTs - aTs;
    });

    const grouped = (() => {
      if (libraryGroupBy === "none") {
        return [{ key: "all", label: "All instances", items: filtered }];
      }
      const map = new Map<string, Instance[]>();
      for (const inst of filtered) {
        const key = libraryGroupBy === "loader" ? loaderLabelFor(inst) : inst.mc_version;
        if (!map.has(key)) map.set(key, []);
        map.get(key)!.push(inst);
      }
      return Array.from(map.entries()).map(([key, items]) => ({
        key,
        label: key,
        items,
      }));
    })();

    const runningIds = new Set(runningInstances.map((run) => run.instance_id));
    const recentlyPlayed = [...instances]
      .map((inst) => {
        const lastLaunchAt = instanceLastRunMetadataById[inst.id]?.lastLaunchAt ?? inst.created_at;
        return { inst, lastLaunchAtMs: parseDateLike(lastLaunchAt)?.getTime() ?? 0 };
      })
      .sort((a, b) => b.lastLaunchAtMs - a.lastLaunchAtMs)
      .slice(0, 3);
    const knownModsTotal = Object.entries(instanceModCountById)
      .filter(([instanceId, count]) => instances.some((item) => item.id === instanceId) && Number.isFinite(count))
      .reduce((sum, [, count]) => sum + Math.max(0, Number(count ?? 0)), 0);
    const libraryStorageDisplay = storageOverview
      ? formatBytes(Number(storageOverview.total_bytes ?? 0))
      : storageOverviewError
        ? "Unavailable"
        : "Scanning…";
    const storageOverviewWarnings = storageOverview?.warnings ?? [];
    const needsLibraryGrowthPrompt = instances.length < 3;
    const totalRunningCount = runningInstances.length;
    const customInstancesCount = instances.length;
    const recentInstanceCreatedAt = filtered[0]?.created_at ?? null;

    return (
      <div className="page">
        <div className="libraryLayout">
          <section className="libraryMainPane">
            <div className="card libraryHeroCard">
              <div className="libraryHeroHead">
                <div className="libraryHeroMain">
                  <div className="libraryHeroEyebrow">Instance library</div>
                  <div className="libraryHeroTitle">Library</div>
                  <div className="libraryHeroSub">
                    Open an instance to manage content, settings, worlds, and launch state.
                  </div>
                </div>

                <div className="libraryHeroActions">
                  <button className="btn primary" onClick={() => setShowCreate(true)}>
                    <span className="btnIcon">
                      <Icon name="plus" size={18} className="navIcon plusIcon navAnimPlus" />
                    </span>
                    Create new instance
                  </button>
                </div>
              </div>

              <div className="libraryHeroStats">
                <div className="libraryHeroStat">
                  <div className="libraryHeroStatLabel">Instances</div>
                  <div className="libraryHeroStatValue">{instances.length}</div>
                  <div className="libraryHeroStatMeta">
                    {customInstancesCount} custom instance{customInstancesCount === 1 ? "" : "s"}
                  </div>
                </div>
                <div className="libraryHeroStat">
                  <div className="libraryHeroStatLabel">Running</div>
                  <div className="libraryHeroStatValue">{totalRunningCount}</div>
                  <div className="libraryHeroStatMeta">
                    {totalRunningCount === 0 ? "Nothing active right now" : "Minecraft currently in progress"}
                  </div>
                </div>
                <div className="libraryHeroStat">
                  <div className="libraryHeroStatLabel">Known mods</div>
                  <div className="libraryHeroStatValue">{knownModsTotal.toLocaleString()}</div>
                  <div className="libraryHeroStatMeta">
                    Tracked across every visible instance
                  </div>
                </div>
                <div className="libraryHeroStat">
                  <div className="libraryHeroStatLabel">Newest</div>
                  <div className="libraryHeroStatValue">
                    {recentInstanceCreatedAt ? formatDate(recentInstanceCreatedAt) : "None"}
                  </div>
                  <div className="libraryHeroStatMeta">
                    Most recently created instance
                  </div>
                </div>
              </div>
            </div>

            <>
                {!selectedLauncherAccount ? (
                  <div className="libraryStatusBanner card">
                    <div className="libraryStatusTitle">Sign in to Microsoft</div>
                    <div className="libraryStatusText">
                      Connect your Minecraft account to launch with the native launcher.
                    </div>
                    <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                      {msLoginSessionId ? "Waiting for login..." : "Connect account"}
                    </button>
                  </div>
                ) : null}

                <div className="libraryTopRow">
                  <div className="libraryPrimaryControls">
                    <SegmentedControl
                      value={libraryScope}
                      onChange={(v) => setLibraryScope(v as any)}
                      options={[
                        { label: "All instances", value: "all" },
                        { label: "Downloaded", value: "downloaded" },
                        { label: "Custom", value: "custom" },
                      ]}
                    />

                    <div className="librarySearch">
                      <Icon name="search" size={18} />
                      <input
                        className="input"
                        placeholder="Search instances..."
                        value={libraryQuery}
                        onChange={(e) => setLibraryQuery(e.target.value)}
                      />
                      {libraryQuery && (
                        <button className="iconBtn" onClick={() => setLibraryQuery("")} aria-label="Clear">
                          <Icon name="x" size={18} />
                        </button>
                      )}
                    </div>
                  </div>

                  <div className="libraryRight">
                    <MenuSelect
                      value={librarySort}
                      labelPrefix="Sort"
                      onChange={(v) => setLibrarySort(v as "recent" | "name")}
                      options={[
                        { value: "recent", label: "Recently created" },
                        { value: "name", label: "Name" },
                      ]}
                      align="start"
                    />
                    <MenuSelect
                      value={libraryGroupBy}
                      labelPrefix="Group"
                      onChange={(v) => setLibraryGroupBy(v as LibraryGroupBy)}
                      options={[
                        { value: "none", label: "None" },
                        { value: "loader", label: "Loader" },
                        { value: "version", label: "Game version" },
                      ]}
                      align="start"
                    />
                  </div>
                </div>

                {libraryScope === "downloaded" ? (
              <div className="card" style={{ marginTop: 12 }}>
                <div className="emptyState">
                  <div className="emptyTitle">No downloaded instances yet</div>
                  <div className="emptySub">
                    Later, installed Modrinth modpacks will appear here. For now, create a custom
                    instance.
                  </div>
                </div>
              </div>
                ) : filtered.length === 0 ? (
              <div className="card" style={{ marginTop: 12 }}>
                <div className="emptyState">
                  <div className="emptyTitle">No instances found</div>
                  <div className="emptySub">
                    Create an instance to start managing mods and versions.
                  </div>
                  <div style={{ marginTop: 12 }}>
                    <button className="btn primary" onClick={() => setShowCreate(true)}>
                      <span className="btnIcon">
                        <Icon name="plus" size={18} className="navIcon plusIcon navAnimPlus" />
                      </span>
                      Create new instance
                    </button>
                  </div>
                </div>
              </div>
                ) : (
              <div className="libraryGroupList">
                {grouped.map((group) => (
                  <section key={group.key} className="libraryGroupSection">
                    {libraryGroupBy !== "none" ? (
                      <div className="libraryGroupHeader">
                        <div>{group.label}</div>
                        <div className="chip subtle">{group.items.length}</div>
                      </div>
                    ) : null}
                    <div className="libraryGrid">
                      {group.items.map((inst) => {
                        const active = inst.id === selectedId;
                        const loaderLabel = loaderLabelFor(inst);
                        const isRunning = runningIds.has(inst.id);
                        const runningLaunch = runningInstances.find((run) => run.instance_id === inst.id) ?? null;
                        const launchStage = launchStageByInstance[inst.id] ?? null;
                        const launchStageLabel = launchStage?.label?.trim() || launchStageBadgeLabel(
                          launchStage?.status,
                          launchStage?.message
                        );
                        const instanceModCount = Number(instanceModCountById[inst.id] ?? 0);
                        const createdLabel = formatDate(inst.created_at);
                        return (
                          <article
                            key={inst.id}
                            className={`instCard ${active ? "active" : ""} ${isRunning ? "running" : ""}`}
                            onClick={() => openInstance(inst.id)}
                            onContextMenu={(event) => {
                              event.preventDefault();
                              event.stopPropagation();
                              setLibraryContextMenu({
                                instanceId: inst.id,
                                x: event.clientX,
                                y: event.clientY,
                              });
                            }}
                          >
                            <div className="instCardHead">
                              <div className="instCardIcon">
                                {inst.icon_path ? (
                                  <LocalImage path={inst.icon_path} alt="" fallback={<Icon name="box" size={19} />} />
                                ) : (
                                  <Icon name="box" size={19} />
                                )}
                              </div>
                              <div className="instCardHeadText">
                                <div className="instCardTitle">{inst.name}</div>
                                <div className="instCardSub">
                                  {loaderLabel} · Minecraft {inst.mc_version}
                                </div>
                              </div>
                            </div>

                            <div className="instCardMeta">
                              <span className="chip">{loaderLabel}</span>
                              <span className="chip">{inst.mc_version}</span>
                              <span className="chip subtle">{instanceModCount} mod{instanceModCount === 1 ? "" : "s"}</span>
                              <span className="chip subtle">Created {createdLabel}</span>
                              {isRunning ? <span className="chip">Running</span> : null}
                              {!isRunning && launchStageLabel ? (
                                <span className="chip">{launchStage?.status === "starting" ? `Launching: ${launchStageLabel}` : launchStageLabel}</span>
                              ) : null}
                            </div>

                            <div className="instCardActions" onClick={(event) => event.stopPropagation()}>
                              {runningLaunch ? (
                                <button className="btn" onClick={() => onStopRunning(runningLaunch.launch_id)}>
                                  Stop
                                </button>
                              ) : (
                                <button
                                  className={`btn ${launchBusyInstanceIds.includes(inst.id) ? "danger" : "primary"}`}
                                  onClick={() => onPlayInstance(inst)}
                                  disabled={launchCancelBusyInstanceId === inst.id}
                                >
                                  <Icon name={launchBusyInstanceIds.includes(inst.id) ? "x" : "play"} size={16} />
                                  {launchBusyInstanceIds.includes(inst.id)
                                    ? (launchCancelBusyInstanceId === inst.id ? "Cancelling…" : "Cancel launch")
                                    : "Play"}
                                </button>
                              )}
                              <button className="btn" onClick={() => openInstance(inst.id)}>
                                View instance
                              </button>
                            </div>
                          </article>
                        );
                      })}
                    </div>
                  </section>
                ))}
              </div>
                )}
              </>
          </section>

          <aside className="librarySidePane">
            <div className="card librarySideCard">
              <div className="librarySideTitle">Instances running</div>
              <div className="libraryRunCount">{runningInstances.length}</div>
              {runningInstances.length === 0 ? (
                <div className="compactEmptyState">
                  <span className="compactEmptyIcon" aria-hidden="true">
                    <Icon name="play" size={15} />
                  </span>
                  <div className="compactEmptyBody">
                    <div className="compactEmptyTitle">Nothing running right now</div>
                    <div className="compactEmptyText">Hit Play on any instance to launch Minecraft.</div>
                  </div>
                </div>
              ) : (
                <div className="libraryRunList">
                  {runningInstances.slice(0, 5).map((run) => (
                    <div key={run.launch_id} className="libraryRunRow">
                      <span>{run.instance_name}</span>
                      <div style={{ display: "flex", gap: 6, flexWrap: "wrap", justifyContent: "flex-end" }}>
                        <span className="chip subtle">{run.method}</span>
                        {run.isolated ? <span className="chip subtle">Disposable</span> : null}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="card librarySideCard">
              <div className="librarySideTitle">Recently played</div>
              {recentlyPlayed.length === 0 ? (
                <div className="muted">No launch history yet.</div>
              ) : (
                <div className="libraryRecentList">
                  {recentlyPlayed.map((row) => (
                    <button
                      key={row.inst.id}
                      className="libraryRecentRow"
                      onClick={() => openInstance(row.inst.id)}
                      title={formatDateTime(row.inst.created_at, "Unknown time")}
                    >
                      <span className="libraryRecentName">{row.inst.name}</span>
                      <span className="libraryRecentMeta">{relativeTimeFromMs(row.lastLaunchAtMs)}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>

            <button
              className="card librarySideCard libraryStorageCard"
              onClick={() => openStorageManager("overview")}
              type="button"
            >
              <div className="libraryStorageCardMain">
                <div className="librarySideTitle">Storage usage</div>
                <div className="libraryStorageStat">{libraryStorageDisplay}</div>
                <div className="muted">
                  {storageOverviewError
                    ? "Storage scan failed. Open the manager for details."
                    : storageOverviewBusy && !storageOverview
                    ? "Scanning launcher + instance storage…"
                    : `${knownModsTotal.toLocaleString()} total mods across ${instances.length} instance${instances.length === 1 ? "" : "s"}.`}
                </div>
              </div>
              <div className="libraryStorageCardMeta">
                {storageOverview ? (
                  <>
                    <span className="chip subtle">Reclaimable {formatBytes(storageOverview.reclaimable_bytes)}</span>
                    {storageOverviewWarnings.length > 0 ? (
                      <span className="chip subtle">{storageOverviewWarnings.length} warning{storageOverviewWarnings.length === 1 ? "" : "s"}</span>
                    ) : null}
                  </>
                ) : (
                  <span className="chip subtle">{storageOverviewBusy ? "Scanning…" : "Open manager"}</span>
                )}
              </div>
            </button>

            {needsLibraryGrowthPrompt ? (
              <div className="card librarySideCard libraryPromptCard">
                <div className="librarySideTitle">Create your next instance</div>
                <div className="muted">
                  Try a Vanilla profile, a Fabric performance build, or a Creator Studio test instance.
                </div>
                <button className="btn primary" onClick={() => setShowCreate(true)}>
                  <Icon name="plus" size={16} />
                  Create instance
                </button>
              </div>
            ) : null}

            <div className="card librarySideCard">
              <div className="librarySideTitle">Playing as</div>
              {selectedLauncherAccount ? (
                <>
                  <div className="libraryAccountName">{selectedLauncherAccount.username}</div>
                  <div className="libraryAccountId muted">{selectedLauncherAccount.id}</div>
                  <div className="row" style={{ marginTop: 10 }}>
                    <button className="btn" onClick={() => setRoute("account")}>
                      Account page
                    </button>
                  </div>
                </>
              ) : (
                <>
                  <div className="muted">No Minecraft account connected.</div>
                  <div className="row" style={{ marginTop: 10 }}>
                    <button className="btn primary" onClick={onBeginMicrosoftLogin} disabled={launcherBusy}>
                      {msLoginSessionId ? "Waiting..." : "Sign in"}
                    </button>
                  </div>
                </>
              )}
            </div>

            <div className="card librarySideCard">
              <div className="librarySideTitle">Quick actions</div>
              <div className="libraryQuickActions">
                <button className="btn" onClick={() => setRoute("discover")}>Discover mods</button>
                <button className="btn primary" onClick={() => setShowCreate(true)}>Create instance</button>
              </div>
            </div>
          </aside>
        </div>
      </div>
    );
  }

  const appUpdateAvailable = Boolean(appUpdaterState?.available);
  const appUpdateBannerVisibleRaw =
    appUpdaterBusy ||
    appUpdaterInstallBusy ||
    Boolean(appUpdaterLastError) ||
    Boolean(appUpdaterState?.available);
  const appUpdateBannerVisible =
    appUpdateBannerVisibleRaw && appUpdateBannerDismissedKey !== appUpdateBannerStateKey;
  const appUpdateBannerTitle = appUpdaterInstallBusy
    ? "Installing app update…"
    : appUpdaterBusy
      ? "Checking for OpenJar Launcher updates…"
      : appUpdaterLastError
        ? "App update check failed"
      : appUpdateAvailable
        ? `Update available${appUpdaterState?.latest_version ? `: v${appUpdaterState.latest_version}` : ""}`
        : appUpdaterState
          ? `OpenJar Launcher is up to date (v${appUpdaterState.current_version || appVersion}).`
          : "App updates";
  const appUpdateBannerMeta = appUpdaterLastError
    ? appUpdaterLastError
    : appUpdaterState?.checked_at
    ? `Last checked ${formatDateTime(appUpdaterState.checked_at, "just now")}`
    : "Use Check now to verify updates.";

  useEffect(() => {
    if (appUpdateBannerVisible) {
      setAppUpdateBannerMounted(true);
      setAppUpdateBannerExiting(false);
      return;
    }
    if (!appUpdateBannerMounted) return;
    setAppUpdateBannerExiting(true);
    const timer = window.setTimeout(() => {
      setAppUpdateBannerMounted(false);
      setAppUpdateBannerExiting(false);
    }, APP_UPDATE_BANNER_ANIMATION_MS);
    return () => window.clearTimeout(timer);
  }, [appUpdateBannerVisible, appUpdateBannerMounted]);

  return (
    <div className="appWrap">
      <aside className="navRail">
        <NavButton active={route === "home"} label={t("nav.home")} onClick={() => setRoute("home")}>
          <Icon name="home" className="navIcon navHomeIcon" />
        </NavButton>

        <NavButton active={route === "discover"} label={t("nav.discover")} onClick={() => setRoute("discover")}>
          <Icon name="compass" className="navIcon compassIcon navAnimCompass" />
        </NavButton>

        <NavButton className="boxPulse" active={route === "modpacks"} label={t("nav.creator_studio")} onClick={() => setRoute("modpacks")}>
          <Icon name="box" className="navIcon navAnimBox" />
        </NavButton>

        <NavButton
          className="booksTilt"
          active={route === "library"}
          label={t("nav.library")}
          onClick={() => setRoute("library")}
        >
          <Icon name="books" className="navIcon navAnimBooks" />
        </NavButton>

        <NavButton
          active={route === "updates"}
          label={t("nav.updates")}
          onClick={() => setRoute("updates")}
          badge={scheduledUpdatesAvailableTotal}
        >
          <Icon name="bell" className="navIcon" />
        </NavButton>

        <NavButton
          active={route === "skins"}
          label={t("nav.skins")}
          onClick={() => setRoute("skins")}
        >
          <Icon name="skin" className="navIcon" />
        </NavButton>

        <div className="navDivider" />

	        {instances.length > 0 && (
	          <div className="navInstances">
	            {instances.slice(0, 6).map((inst) => (
	              <NavButton
	                key={inst.id}
	                variant="accent"
	                active={route === "instance" && selectedId === inst.id}
	                label={inst.name}
	                onClick={() => openInstance(inst.id)}
	              >
		                <div className="instAvatar" aria-hidden="true">
		                  {inst.icon_path ? (
                        <LocalImage
                          path={inst.icon_path}
                          alt=""
                          fallback={inst.name.trim() ? inst.name.trim().slice(0, 1).toUpperCase() : "?"}
                        />
                      ) : (
		                    inst.name.trim() ? inst.name.trim().slice(0, 1).toUpperCase() : "?"
                      )}
		                </div>
	              </NavButton>
	            ))}
	          </div>
	        )}

	        {instances.length > 0 && <div className="navDivider" />}

        <NavButton
          className="plusJump"
          active={false}
          label={t("nav.create_instance")}
          onClick={() => setShowCreate(true)}
        >
          <Icon name="plus" className="navIcon plusIcon navAnimPlus" />
        </NavButton>

        <div className="navBottom">
          {isDevMode ? (
            <NavButton active={route === "dev"} label={t("nav.dev")} onClick={() => setRoute("dev")}>
              <Icon name="sparkles" className="navIcon" />
            </NavButton>
          ) : null}
          <NavButton className="profileBounce" active={route === "account"} label={t("nav.account")} onClick={() => setRoute("account")}>
            <Icon name="user" className="navIcon navAnimUser" />
          </NavButton>
          <NavButton className="settingsSpin" active={route === "settings"} label={t("nav.settings")} onClick={() => setRoute("settings")}>
            <Icon name="gear" className="navIcon navAnimGear" />
          </NavButton>
        </div>
      </aside>

      <main className="content">
        {appUpdateBannerMounted ? (
          <div
            className={`appUpdateBanner card ${appUpdateAvailable ? "available" : ""} ${
              appUpdaterBusy ? "checking" : ""
            } ${appUpdaterLastError ? "error" : ""} ${appUpdateBannerExiting ? "exit" : "enter"}`}
          >
            <div className="appUpdateBannerMain">
              <div className="appUpdateBannerTitle">{appUpdateBannerTitle}</div>
              <div className="appUpdateBannerMeta">
                {appUpdateBannerMeta}
                {appUpdaterState?.pub_date
                  ? ` • Published ${formatDateTime(appUpdaterState.pub_date, "Unknown date")}`
                  : ""}
              </div>
            </div>
            <div className="appUpdateBannerActions">
              <button
                className="btn"
                onClick={() => void onCheckAppUpdate({ silent: false })}
                disabled={appUpdaterBusy || appUpdaterInstallBusy}
              >
                {appUpdaterBusy ? "Checking…" : "Check now"}
              </button>
              <button
                className="btn subtle"
                onClick={() => setAppUpdateBannerDismissedKey(appUpdateBannerStateKey)}
                disabled={appUpdaterInstallBusy}
              >
                Dismiss
              </button>
              {appUpdateAvailable ? (
                <button
                  className="btn primary"
                  onClick={() => void onInstallAppUpdate()}
                  disabled={appUpdaterInstallBusy || appUpdaterBusy}
                >
                  {appUpdaterInstallBusy ? "Installing…" : "Update now"}
                </button>
              ) : null}
            </div>
          </div>
        ) : null}
        {error ? (
          <div className="errorBox topStatusBanner statusBanner">
            <div className="statusBannerMessage">{error}</div>
            <div className="statusBannerActions">
              <button className="btn subtle" onClick={() => setError(null)}>
                Dismiss
              </button>
            </div>
          </div>
        ) : null}
        {curseforgeBlockedRecoveryPrompt ? (
          <div className="warningBox topStatusBanner statusBanner">
            <div className="statusBannerMessage">
              CurseForge blocked automated download for
              {" "}
              <strong>{curseforgeBlockedRecoveryPrompt.target.title}</strong>.
              Open the project page and import the file locally.
            </div>
            <div className="statusBannerActions">
              <button
                className="btn"
                onClick={() => void openExternalLink(curseforgeBlockedRecoveryPrompt.projectUrl)}
              >
                Open CurseForge page
              </button>
              <button
                className="btn primary"
                onClick={() => {
                  const instance = instances.find(
                    (item) => item.id === curseforgeBlockedRecoveryPrompt.instanceId
                  );
                  if (!instance) {
                    setError("Could not find the target instance for local import.");
                    return;
                  }
                  const contentLabel = localImportTypeLabel(
                    curseforgeBlockedRecoveryPrompt.contentView
                  );
                  setCurseforgeBlockedRecoveryPrompt(null);
                  setInstallNotice(
                    `Select the downloaded ${contentLabel} file to import it into ${instance.name}.`
                  );
                  void onAddContentFromFile(
                    instance,
                    curseforgeBlockedRecoveryPrompt.contentView
                  );
                }}
              >
                Import local file
              </button>
              <button
                className="btn subtle"
                onClick={() => setCurseforgeBlockedRecoveryPrompt(null)}
              >
                Dismiss
              </button>
            </div>
          </div>
        ) : null}
        {installNotice
          ? (() => {
              const tone = inferNoticeTone(installNotice);
              const bannerClass =
                tone === "error" ? "errorBox" : tone === "warning" ? "warningBox" : "noticeBox";
              const dismissButtonClass = "btn ghost statusBannerDismiss";
              return (
                <div className={`${bannerClass} topStatusBanner statusBanner`}>
                  <div className="statusBannerMessage">{installNotice}</div>
                  <div className="statusBannerActions">
                    <button className={dismissButtonClass} onClick={() => setInstallNotice(null)}>
                      Dismiss
                    </button>
                  </div>
                </div>
              );
            })()
          : null}
        {renderContent()}
      </main>

      <CommandPalette
        open={commandPaletteOpen}
        items={commandPaletteItems}
        onClose={() => setCommandPaletteOpen(false)}
      />

      {libraryContextMenu && libraryContextMenuStyle && libraryContextTarget
        ? createPortal(
            <div
              ref={libraryContextMenuRef}
              className="libraryContextMenu"
              style={libraryContextMenuStyle}
            >
              <button
                className="libraryContextItem"
                disabled={launchCancelBusyInstanceId === libraryContextTarget.id}
                onClick={() => {
                  setLibraryContextMenu(null);
                  void onPlayInstance(libraryContextTarget);
                }}
              >
                <Icon name={launchBusyInstanceIds.includes(libraryContextTarget.id) ? "x" : "play"} size={16} />
                {launchBusyInstanceIds.includes(libraryContextTarget.id) ? "Cancel launch" : "Play"}
              </button>
              <button
                className="libraryContextItem"
                onClick={() => {
                  setLibraryContextMenu(null);
                  setRoute("discover");
                  setInstallNotice(
                    `Open a mod and choose "${libraryContextTarget.name}" in Install to instance.`
                  );
                }}
              >
                <Icon name="download" size={16} />
                Add content
              </button>
              <div className="libraryContextDivider" />
              <button
                className="libraryContextItem"
                onClick={() => {
                  setLibraryContextMenu(null);
                  openInstance(libraryContextTarget.id);
                }}
              >
                <Icon name="books" size={16} />
                View instance
              </button>
              <button
                className="libraryContextItem"
                onClick={() => {
                  setLibraryContextMenu(null);
                  void onOpenInstancePath(libraryContextTarget, "instance");
                }}
              >
                <Icon name="folder" size={16} />
                Open folder
              </button>
              <button
                className="libraryContextItem"
                onClick={() => {
                  setLibraryContextMenu(null);
                  void onOpenInstancePath(libraryContextTarget, "mods");
                }}
              >
                <Icon name="folder" size={16} />
                Open mods folder
              </button>
              <button
                className="libraryContextItem"
                onClick={() => {
                  setLibraryContextMenu(null);
                  void onExportModsZip(libraryContextTarget);
                }}
              >
                <Icon name="upload" size={16} />
                Export mods zip
              </button>
              <button
                className="libraryContextItem"
                onClick={() => {
                  setLibraryContextMenu(null);
                  setSupportBundleModalInstanceId(libraryContextTarget.id);
                }}
              >
                <Icon name="download" size={16} />
                Export support bundle
              </button>
              <div className="libraryContextDivider" />
              <button
                className="libraryContextItem danger"
                onClick={() => {
                  setLibraryContextMenu(null);
                  requestDelete(libraryContextTarget);
                }}
              >
                <Icon name="trash" size={16} />
                Delete
              </button>
            </div>,
            document.body
          )
        : null}

      {storageManagerSelection ? (
        (() => {
          const selection = storageManagerSelection;
          const parsed = parseStorageSelection(selection);
          const overview = storageOverview;
          const instanceSummary =
            parsed.scope === "instance"
              ? overview?.instance_summaries.find((item) => item.instance_id === parsed.instanceId) ?? null
              : null;
          const relativePath = storageManagerPathBySelection[selection] ?? "";
          const requestKey = storageRequestKey(selection, storageDetailMode, relativePath);
          const entryRows = storageEntriesByKey[requestKey] ?? [];
          const entryError = storageEntriesErrorByKey[requestKey] ?? null;
          const entryBusy = storageEntriesBusyKey === requestKey;
          const revealLabel = storageRevealActionLabel();
          const railItems = [
            { key: "overview" as StorageManagerSelection, label: "Overview", meta: formatBytes(overview?.total_bytes ?? 0) },
            { key: "app" as StorageManagerSelection, label: "App storage", meta: formatBytes(overview?.app_bytes ?? 0) },
            { key: "cache" as StorageManagerSelection, label: "Shared cache", meta: formatBytes(overview?.shared_cache_bytes ?? 0) },
            ...((overview?.instance_summaries ?? []).map((summary) => ({
              key: storageSelectionForInstance(summary.instance_id),
              label: summary.instance_name,
              meta: formatBytes(summary.total_bytes),
            })) as Array<{ key: StorageManagerSelection; label: string; meta: string }>),
          ];
          const breakdownRows =
            parsed.scope === "app"
              ? storageAppBreakdownForScope(overview, "app")
              : parsed.scope === "cache"
                ? storageAppBreakdownForScope(overview, "cache")
                : parsed.scope === "instance" && instanceSummary
                  ? storageInstanceBreakdown(instanceSummary)
                  : [
                      { key: "total", label: "Total", bytes: Number(overview?.total_bytes ?? 0) },
                      { key: "app", label: "App storage", bytes: Number(overview?.app_bytes ?? 0) },
                      { key: "cache", label: "Shared cache", bytes: Number(overview?.shared_cache_bytes ?? 0) },
                      { key: "instances", label: "Instances", bytes: Number(overview?.instances_bytes ?? 0) },
                    ];
          const visibleBreakdownRows = breakdownRows.filter((row) => Number(row.bytes ?? 0) > 0);
          const railInstanceCount = overview?.instance_summaries?.length ?? 0;
          const selectedTotal =
            parsed.scope === "overview"
              ? Number(overview?.total_bytes ?? 0)
              : parsed.scope === "app"
                ? Number(overview?.app_bytes ?? 0)
                : parsed.scope === "cache"
                  ? Number(overview?.shared_cache_bytes ?? 0)
                  : Number(instanceSummary?.total_bytes ?? 0);
          const selectionTitle =
            parsed.scope === "overview"
              ? "Overview"
              : parsed.scope === "app"
                ? "App storage"
                : parsed.scope === "cache"
                  ? "Shared cache"
                  : instanceSummary?.instance_name ?? "Instance storage";
          const recommendationRows =
            parsed.scope === "overview"
              ? overview?.cleanup_recommendations ?? []
              : parsed.scope === "cache"
                ? (overview?.cleanup_recommendations ?? []).filter((item) => item.scope === "cache")
                : parsed.scope === "instance" && parsed.instanceId
                  ? (overview?.cleanup_recommendations ?? []).filter(
                      (item) => item.scope === `instance:${parsed.instanceId}`
                    )
                  : [];
          const normalizedRelativePath =
            relativePath === "root"
              ? ""
              : relativePath.startsWith("root/")
                ? relativePath.slice("root/".length)
                : relativePath;
          const breadcrumbParts = normalizedRelativePath ? normalizedRelativePath.split("/").filter(Boolean) : [];
          const showBreakdownRows =
            selectedTotal > 0 &&
            (visibleBreakdownRows.length > 0 ? visibleBreakdownRows : breakdownRows).some(
              (row) => Number(row.bytes ?? 0) > 0
            );
          return (
            <Modal
              title="Storage manager"
              size="xwide"
              className="storageManagerModal"
              onClose={() => setStorageManagerSelection(null)}
            >
              <div className="modalBody storageManagerBody">
                <div className="storageManagerHero">
                  <div>
                    <div className="storageManagerEyebrow">OpenJar managed storage</div>
                    <div className="storageManagerTotal">{formatBytes(overview?.total_bytes ?? 0)}</div>
                    <div className="muted">
                      {overview
                        ? `Last scanned ${formatDateTime(overview.scanned_at, "just now")}`
                        : storageOverviewBusy
                          ? "Scanning storage…"
                          : "Run a scan to see launcher + instance usage."}
                    </div>
                  </div>
                  <div className="storageManagerHeroActions">
                    <button
                      className="btn"
                      type="button"
                      onClick={() => void refreshStorageOverview({ force: true, clearEntries: true })}
                      disabled={storageOverviewBusy}
                    >
                      {storageOverviewBusy ? "Refreshing…" : "Refresh"}
                    </button>
                    <button
                      className="btn primary"
                      type="button"
                      disabled={storageCleanupBusy || recommendationRows.length === 0}
                      onClick={() =>
                        void performStorageCleanup(
                          recommendationRows.map((item) => item.action_id),
                          {
                            description: `Run safe cleanup${recommendationRows.length > 0 ? ` and reclaim about ${formatBytes(
                              recommendationRows.reduce(
                                (sum, item) => sum + Number(item.reclaimable_bytes ?? 0),
                                0
                              )
                            )}` : ""}?`,
                            buttonId: "all",
                          }
                        )
                      }
                    >
                      {storageCleanupBusy && storageActionBusyId === "all" ? "Cleaning…" : "Run safe cleanup"}
                    </button>
                  </div>
                </div>

                <div className="storageSummaryStrip">
                  <div className="storageSummaryCard">
                    <div className="storageSummaryLabel">Total</div>
                    <div className="storageSummaryValue">{formatBytes(overview?.total_bytes ?? 0)}</div>
                  </div>
                  <div className="storageSummaryCard">
                    <div className="storageSummaryLabel">App storage</div>
                    <div className="storageSummaryValue">{formatBytes(overview?.app_bytes ?? 0)}</div>
                  </div>
                  <div className="storageSummaryCard">
                    <div className="storageSummaryLabel">Instances</div>
                    <div className="storageSummaryValue">{formatBytes(overview?.instances_bytes ?? 0)}</div>
                  </div>
                  <div className="storageSummaryCard">
                    <div className="storageSummaryLabel">Reclaimable now</div>
                    <div className="storageSummaryValue">{formatBytes(overview?.reclaimable_bytes ?? 0)}</div>
                  </div>
                </div>

                {storageManagerNotice ? <div className="noticeBox">{storageManagerNotice}</div> : null}
                {storageOverviewError ? <div className="errorBox">{storageOverviewError}</div> : null}
                {overview?.warnings?.length ? (
                  <div className="warningBox">{summarizeWarnings(overview.warnings, 4)}</div>
                ) : null}

                <div className="storageManagerLayout">
                  <aside className="storageManagerRail">
                    <div className="storageRailHeader">
                      <div className="storageRailHeaderTitle">Locations</div>
                      <div className="storageRailHeaderMeta">
                        {railInstanceCount} instance{railInstanceCount === 1 ? "" : "s"} tracked
                      </div>
                    </div>
                    {railItems.map((item) => (
                      <button
                        key={item.key}
                        type="button"
                        className={`storageRailItem ${selection === item.key ? "active" : ""}`}
                        onClick={() => {
                          setStorageManagerSelection(item.key);
                          setStorageManagerNotice(null);
                        }}
                      >
                        <span>{item.label}</span>
                        <span className="storageRailMeta">{item.meta}</span>
                      </button>
                    ))}
                  </aside>

                  <section className="storageManagerPane">
                    <div className="storagePaneHeader">
                      <div>
                        <div className="storagePaneTitle">{selectionTitle}</div>
                        <div className="muted">
                          {parsed.scope === "overview"
                            ? "Track the biggest storage consumers across the launcher and your instances."
                            : parsed.scope === "instance"
                              ? `${instanceSummary?.instance_name ?? "Instance"} is using ${formatBytes(
                                  instanceSummary?.total_bytes ?? 0
                                )}.`
                              : `${selectionTitle} is using ${formatBytes(selectedTotal)}.`}
                        </div>
                        <div className="storagePaneMetaRow">
                          <span className="chip subtle">{formatBytes(selectedTotal)}</span>
                          {parsed.scope !== "overview" ? (
                            <span className="chip subtle">
                              {storageDetailMode === "folders" ? "Largest folders" : "Largest files"}
                            </span>
                          ) : null}
                          {normalizedRelativePath ? (
                            <span className="chip subtle">Path: {normalizedRelativePath}</span>
                          ) : parsed.scope !== "overview" ? (
                            <span className="chip subtle">Path: root</span>
                          ) : null}
                        </div>
                      </div>
                      {parsed.scope !== "overview" ? (
                        <div className="storagePaneActions">
                          <SegmentedControl
                            value={storageDetailMode}
                            onChange={(value) => setStorageDetailMode((value ?? "folders") as StorageDetailMode)}
                            options={[
                              { label: "Largest folders", value: "folders" },
                              { label: "Largest files", value: "files" },
                            ]}
                          />
                          <button
                            className="btn"
                            type="button"
                            onClick={() => void revealStoragePath(selection, normalizedRelativePath || undefined)}
                          >
                            {revealLabel}
                          </button>
                        </div>
                      ) : null}
                    </div>

                    {parsed.scope !== "overview" && breadcrumbParts.length > 0 ? (
                      <div className="storageBreadcrumbRow">
                        <button
                          className="chip subtle chipButton"
                          type="button"
                          onClick={() =>
                            setStorageManagerPathBySelection((prev) => ({ ...prev, [selection]: "" }))
                          }
                        >
                          Root
                        </button>
                        {breadcrumbParts.map((part, index) => {
                          const nextPath = breadcrumbParts.slice(0, index + 1).join("/");
                          return (
                            <button
                              key={nextPath}
                              className="chip subtle chipButton"
                              type="button"
                              onClick={() =>
                                setStorageManagerPathBySelection((prev) => ({
                                  ...prev,
                                  [selection]: nextPath,
                                }))
                              }
                            >
                              {part}
                            </button>
                          );
                        })}
                        {normalizedRelativePath ? (
                          <button
                            className="chip subtle chipButton"
                            type="button"
                            onClick={() => {
                              const nextParts = breadcrumbParts.slice(0, -1);
                              setStorageManagerPathBySelection((prev) => ({
                                ...prev,
                                [selection]: nextParts.join("/"),
                              }));
                            }}
                          >
                            Up one level
                          </button>
                        ) : null}
                      </div>
                    ) : null}

                    <div className="storagePaneGrid">
                      <div className="storagePaneCard">
                        <div className="storageSectionHeader">
                          <div>
                            <div className="storageSectionTitle">Breakdown</div>
                            <div className="storageSectionSub">See which buckets are taking the most space.</div>
                          </div>
                          <span className="chip subtle">{formatBytes(selectedTotal)}</span>
                        </div>
                        <div className="storageBreakdownList">
                          {showBreakdownRows ? (
                            (visibleBreakdownRows.length > 0 ? visibleBreakdownRows : breakdownRows).map((row) => {
                              const percent =
                                selectedTotal > 0 ? (Number(row.bytes ?? 0) / selectedTotal) * 100 : 0;
                              return (
                                <div key={row.key} className="storageBreakdownRow">
                                  <div className="storageBreakdownMeta">
                                    <div className="storageBreakdownLabel">{row.label}</div>
                                    <div className="storageBreakdownValue">{formatBytes(row.bytes)}</div>
                                  </div>
                                  <div className="storageBreakdownBar">
                                    <div
                                      className="storageBreakdownBarFill"
                                      style={{ width: `${Math.max(2, Math.min(100, percent || 0))}%` }}
                                    />
                                  </div>
                                  <div className="storageBreakdownPercent">{formatPercent(percent)}</div>
                                </div>
                              );
                            })
                          ) : (
                            <div className="storageEmptyState">
                              <div className="storageEmptyStateTitle">Nothing significant here yet</div>
                              <div className="storageEmptyStateText">
                                This location is effectively empty right now, so there is no meaningful breakdown to show.
                              </div>
                            </div>
                          )}
                        </div>
                      </div>

                      <div className="storagePaneCard">
                        <div className="storageSectionHeader">
                          <div>
                            <div className="storageSectionTitle">
                              {parsed.scope === "overview"
                                ? "Top cleanup ideas"
                                : storageDetailMode === "folders"
                                  ? "Largest folders"
                                  : "Largest files"}
                            </div>
                            <div className="storageSectionSub">
                              {parsed.scope === "overview"
                                ? "Start with safe cleanup ideas that free space right away."
                                : "Open the biggest entries first to understand where the size is coming from."}
                            </div>
                          </div>
                          {parsed.scope !== "overview" ? (
                            <button
                              className="btn subtle"
                              type="button"
                              onClick={() =>
                                void loadStorageEntries(selection, storageDetailMode, normalizedRelativePath, {
                                  force: true,
                                })
                              }
                            >
                              Refresh list
                            </button>
                          ) : null}
                        </div>

                        {parsed.scope === "overview" ? (
                          <div className="storageRecommendationList">
                            {recommendationRows.length === 0 ? (
                              <div className="storageEmptyState">
                                <div className="storageEmptyStateTitle">No safe cleanup actions right now</div>
                                <div className="storageEmptyStateText">
                                  OpenJar is not seeing any low-risk cleanup wins at the moment.
                                </div>
                              </div>
                            ) : (
                              recommendationRows.slice(0, 8).map((recommendation) => (
                                <div key={recommendation.action_id} className="storageRecommendationRow">
                                  <div>
                                    <div className="storageRecommendationTitle">
                                      {storageCleanupRecommendationLabel(recommendation)}
                                    </div>
                                    <div className="muted">{recommendation.description}</div>
                                  </div>
                                  <button
                                    className="btn"
                                    type="button"
                                    disabled={storageCleanupBusy}
                                    onClick={() =>
                                      void performStorageCleanup([recommendation.action_id], {
                                        description: `Run "${recommendation.title}" and reclaim about ${formatBytes(
                                          recommendation.reclaimable_bytes
                                        )}?`,
                                        buttonId: recommendation.action_id,
                                      })
                                    }
                                  >
                                    {storageCleanupBusy && storageActionBusyId === recommendation.action_id
                                      ? "Cleaning…"
                                      : "Run"}
                                  </button>
                                </div>
                              ))
                            )}
                          </div>
                        ) : entryError ? (
                          <div className="errorBox">{entryError}</div>
                        ) : entryBusy && entryRows.length === 0 ? (
                          <div className="muted">Scanning {storageDetailMode === "folders" ? "folders" : "files"}…</div>
                        ) : entryRows.length === 0 ? (
                          <div className="storageEmptyState">
                            <div className="storageEmptyStateTitle">Nothing notable in this location yet</div>
                            <div className="storageEmptyStateText">
                              Try another folder, switch between folders and files, or refresh the list after a new scan.
                            </div>
                          </div>
                        ) : (
                          <div className="storageEntryList">
                            {entryRows.map((row) => (
                              <div key={`${row.relative_path}:${row.bytes}`} className="storageEntryRow">
                                <button
                                  className={`storageEntryMain ${row.is_dir ? "isDir" : ""}`}
                                  type="button"
                                  onClick={() => {
                                    if (row.is_dir && storageDetailMode === "folders") {
                                      setStorageManagerPathBySelection((prev) => ({
                                        ...prev,
                                        [selection]: row.relative_path,
                                      }));
                                      return;
                                    }
                                    void revealStoragePath(selection, row.relative_path);
                                  }}
                                >
                                  <div className="storageEntryLabel">{row.name}</div>
                                  <div className="storageEntryMeta">
                                    <span>{formatBytes(row.bytes)}</span>
                                    {row.modified_at ? (
                                      <span>Updated {formatDateTime(new Date(row.modified_at).toISOString())}</span>
                                    ) : null}
                                  </div>
                                  <div className="storageEntryPath">{row.relative_path || row.name}</div>
                                </button>
                                <button
                                  className="btn subtle"
                                  type="button"
                                  onClick={() => void revealStoragePath(selection, row.relative_path)}
                                >
                                  Reveal
                                </button>
                              </div>
                            ))}
                          </div>
                        )}
                      </div>
                    </div>

                    {parsed.scope === "overview" ? (
                      <div className="storagePaneCard">
                        <div className="storageSectionHeader">
                          <div>
                            <div className="storageSectionTitle">Heaviest instances</div>
                            <div className="storageSectionSub">Jump straight into the largest instance folders.</div>
                          </div>
                        </div>
                        <div className="storageInstanceOverviewList">
                          {(overview?.instance_summaries ?? []).slice(0, 8).map((summary) => (
                            <button
                              key={summary.instance_id}
                              className="storageInstanceOverviewRow"
                              type="button"
                              onClick={() =>
                                setStorageManagerSelection(storageSelectionForInstance(summary.instance_id))
                              }
                            >
                              <div>
                                <div className="storageRecommendationTitle">{summary.instance_name}</div>
                                <div className="muted">
                                  Reclaimable {formatBytes(summary.reclaimable_bytes)} • Mods {formatBytes(summary.mods)}
                                </div>
                              </div>
                              <span className="chip subtle">{formatBytes(summary.total_bytes)}</span>
                            </button>
                          ))}
                        </div>
                      </div>
                    ) : recommendationRows.length > 0 ? (
                      <div className="storagePaneCard">
                        <div className="storageSectionHeader">
                          <div>
                            <div className="storageSectionTitle">Safe cleanup</div>
                            <div className="storageSectionSub">Only actions the launcher considers low-risk to automate.</div>
                          </div>
                        </div>
                        <div className="storageRecommendationList">
                          {recommendationRows.map((recommendation) => (
                            <div key={recommendation.action_id} className="storageRecommendationRow">
                              <div>
                                <div className="storageRecommendationTitle">
                                  {storageCleanupRecommendationLabel(recommendation)}
                                </div>
                                <div className="muted">{recommendation.description}</div>
                              </div>
                              <button
                                className="btn"
                                type="button"
                                disabled={storageCleanupBusy}
                                onClick={() =>
                                  void performStorageCleanup([recommendation.action_id], {
                                    description: `Run "${recommendation.title}" and reclaim about ${formatBytes(
                                      recommendation.reclaimable_bytes
                                    )}?`,
                                    buttonId: recommendation.action_id,
                                  })
                                }
                              >
                                {storageCleanupBusy && storageActionBusyId === recommendation.action_id
                                  ? "Cleaning…"
                                  : "Run"}
                              </button>
                            </div>
                          ))}
                        </div>
                      </div>
                    ) : null}
                  </section>
                </div>
              </div>
            </Modal>
          );
        })()
      ) : null}

      {friendConflictResult && friendConflictInstanceId ? (
        <Modal
          title="Resolve Friend Link Conflicts"
          size="wide"
          onClose={() => {
            if (friendConflictResolveBusy) return;
            setFriendConflictResult(null);
            setFriendConflictInstanceId(null);
          }}
        >
          <div className="modalBody">
            <div className="p" style={{ marginTop: 0 }}>
              Launch is blocked until Friend Link conflicts are resolved for this instance.
            </div>
            <div className="card" style={{ marginTop: 8, padding: 10, borderRadius: 12 }}>
              <div className="rowBetween">
                <div style={{ fontWeight: 900 }}>Conflicts</div>
                <span className="chip subtle">{friendConflictResult.conflicts.length}</span>
              </div>
              <div style={{ display: "grid", gap: 8, marginTop: 10 }}>
                {friendConflictResult.conflicts.slice(0, 24).map((conflict) => (
                  <div key={conflict.id} className="rowBetween">
                    <div>
                      <div style={{ fontWeight: 700 }}>{conflict.key}</div>
                      <div className="muted">{conflict.kind}</div>
                    </div>
                    <span className="chip subtle">{conflict.peer_id}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
          <div className="footerBar">
            <button
              className="btn"
              disabled={friendConflictResolveBusy}
              onClick={async () => {
                setFriendConflictResolveBusy(true);
                try {
                  const out = await resolveFriendLinkConflicts({
                    instanceId: friendConflictInstanceId,
                    resolution: { keep_all_mine: true },
                  });
                  if (out.status === "conflicted") {
                    setFriendConflictResult(out);
                    setInstallNotice(`Still ${out.conflicts.length} unresolved conflict(s).`);
                  } else {
                    setFriendConflictResult(null);
                    setFriendConflictInstanceId(null);
                    setInstallNotice(`Friend Link resolved: ${out.status}.`);
                  }
                } catch (e: any) {
                  setError(e?.toString?.() ?? String(e));
                } finally {
                  setFriendConflictResolveBusy(false);
                }
              }}
            >
              {friendConflictResolveBusy ? "Resolving…" : "Keep all mine"}
            </button>
            <button
              className="btn primary"
              disabled={friendConflictResolveBusy}
              onClick={async () => {
                setFriendConflictResolveBusy(true);
                try {
                  const out = await resolveFriendLinkConflicts({
                    instanceId: friendConflictInstanceId,
                    resolution: { take_all_theirs: true },
                  });
                  if (out.status === "conflicted") {
                    setFriendConflictResult(out);
                    setInstallNotice(`Still ${out.conflicts.length} unresolved conflict(s).`);
                  } else {
                    setFriendConflictResult(null);
                    setFriendConflictInstanceId(null);
                    setInstallNotice(`Friend Link resolved: ${out.status}.`);
                  }
                } catch (e: any) {
                  setError(e?.toString?.() ?? String(e));
                } finally {
                  setFriendConflictResolveBusy(false);
                }
              }}
            >
              {friendConflictResolveBusy ? "Resolving…" : "Take all theirs"}
            </button>
          </div>
        </Modal>
      ) : null}

      {preflightReportModal ? (
        (() => {
          const micPermissionItem = (preflightReportModal.report.permissions ?? []).find(
            (item) => item.key === "microphone"
          );
          const micPermissionStatus = String(micPermissionItem?.status ?? "").toLowerCase();
          const showMicPermissionPrompt = Boolean(
            micPermissionItem?.required &&
              ["denied", "not_determined"].includes(micPermissionStatus)
          );
          const canTriggerMicPromptInModal = Boolean(
            isMacDesktopPlatform() &&
              micPermissionItem?.required &&
              ["denied", "not_determined"].includes(micPermissionStatus)
          );
          return (
            <Modal
              title="Launch compatibility checks"
              onClose={() => setPreflightReportModal(null)}
            >
              <div className="modalBody preflightModalBody">
                <div
                  className={`preflightSummaryCard ${
                    preflightReportModal.report.status === "blocked" ? "blocked" : "warning"
                  }`}
                >
                  <div className="preflightSummaryTitle">
                    {showMicPermissionPrompt
                      ? "Voice chat needs microphone permission"
                      : preflightReportModal.report.status === "blocked"
                        ? "Launch is currently blocked"
                        : "Launch can continue with warnings"}
                  </div>
                  <div className="preflightSummaryMeta">
                    {preflightReportModal.report.blocking_count} blocker
                    {preflightReportModal.report.blocking_count === 1 ? "" : "s"} ·{" "}
                    {preflightReportModal.report.warning_count} warning
                    {preflightReportModal.report.warning_count === 1 ? "" : "s"}
                  </div>
                  {showMicPermissionPrompt ? (
                    <div className="preflightMicPrompt">
                      This instance uses voice chat. Java/Minecraft needs microphone permission.
                    </div>
                  ) : null}
                </div>
                {(preflightReportModal.report.permissions ?? []).length > 0 ? (
                  <div className="preflightPermissionsCard">
                    <div className="preflightPermissionsTitle">Permissions checklist</div>
                    <div className="preflightPermissionsList">
                      {(preflightReportModal.report.permissions ?? []).map((perm) => (
                        <div key={perm.key} className="preflightPermissionRow">
                          <div className="preflightCheckMain">
                            <div className="preflightCheckTitle">{perm.label}</div>
                            <div className="preflightCheckMsg">{perm.detail}</div>
                          </div>
                          <span className={`chip ${permissionStatusChipClass(perm.status)}`}>
                            {permissionStatusLabel(perm.status)}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                ) : null}
                <div className="preflightChecksList">
                  {preflightReportModal.report.items.length === 0 ? (
                    <div className="preflightCheckItem">
                      <div className="preflightCheckMain">
                        <div className="preflightCheckTitle">No issues detected</div>
                        <div className="preflightCheckMsg">
                          Compatibility checks passed for this launch mode.
                        </div>
                      </div>
                      <span className="chip subtle">ok</span>
                    </div>
                  ) : (
                    preflightReportModal.report.items.map((item) => (
                      <div
                        key={`${item.code}:${item.title}`}
                        className={`preflightCheckItem ${item.blocking ? "blocker" : "warning"}`}
                      >
                        <div className="preflightCheckMain">
                          <div className="preflightCheckTitle">{item.title}</div>
                          <div className="preflightCheckMsg">{item.message}</div>
                        </div>
                        <span className={`chip ${item.blocking ? "danger" : "subtle"}`}>
                          {item.severity}
                        </span>
                      </div>
                    ))
                  )}
                </div>
              </div>
              <div className="footerBar">
                {showMicPermissionPrompt ? (
                  <>
                    {canTriggerMicPromptInModal ? (
                      <button
                        className="btn"
                        onClick={() => void triggerInstanceMicrophonePrompt(preflightReportModal.instanceId)}
                      >
                        Trigger Java prompt
                      </button>
                    ) : null}
                    <button className="btn" onClick={() => void openMicrophoneSystemSettings()}>
                      Open System Settings
                    </button>
                  </>
                ) : (
                  <button
                    className="btn"
                    onClick={() => {
                      setPreflightReportModal(null);
                      setRoute("instance");
                      setInstanceSettingsSection("java");
                      setInstanceSettingsOpen(true);
                    }}
                  >
                    Open Java settings
                  </button>
                )}
                <button
                  className="btn"
                  onClick={() =>
                    void openInstancePath({
                      instanceId: preflightReportModal.instanceId,
                      target: "mods",
                    }).catch(() => null)
                  }
                >
                  Open mods folder
                </button>
                <button
                  className="btn"
                  onClick={() =>
                    void runLocalResolverBackfill(preflightReportModal.instanceId, "all", {
                      silent: false,
                      refreshListAfterResolve: true,
                    })
                  }
                >
                  Identify local files
                </button>
                {preflightReportModal.report.status === "blocked" ? (
                  <button
                    className="btn danger"
                    onClick={() => {
                      const modal = preflightReportModal;
                      if (!modal) return;
                      const fingerprint = launchCompatibilityFingerprint(modal.report);
                      const expiresAt = Date.now() + 24 * 60 * 60 * 1000;
                      setPreflightIgnoreByInstance((prev) => ({
                        ...prev,
                        [modal.instanceId]: {
                          fingerprint,
                          expires_at: expiresAt,
                        },
                      }));
                      setPreflightReportModal(null);
                      setInstallNotice(
                        `Ignoring identical compatibility blockers for this instance until ${formatDateTime(
                          new Date(expiresAt).toISOString()
                        )}.`
                      );
                      const inst = instances.find((item) => item.id === modal.instanceId);
                      if (inst) {
                        void onPlayInstance(inst, modal.method);
                      }
                    }}
                  >
                    Launch anyway (ignore 24h)
                  </button>
                ) : null}
                <button
                  className="btn primary"
                  onClick={async () => {
                    const report = await preflightLaunchCompatibility({
                      instanceId: preflightReportModal.instanceId,
                      method: preflightReportModal.method,
                    }).catch(() => null);
                    if (!report) return;
                    setPreflightReportByInstance((prev) => ({
                      ...prev,
                      [preflightReportModal.instanceId]: report,
                    }));
                    setPreflightReportModal((prev) => (prev ? { ...prev, report } : null));
                    if (report.status !== "blocked") {
                      setInstallNotice("Preflight blockers cleared.");
                    }
                  }}
                >
                  Re-check
                </button>
              </div>
            </Modal>
          );
        })()
      ) : null}

      {launchFixModalInstanceId ? (
        (() => {
          const inst = instances.find((item) => item.id === launchFixModalInstanceId);
          const plan = inst ? launchFixPlanByInstance[inst.id] : null;
          const draft = inst ? launchFixPlanDraftByInstance[inst.id] ?? [] : [];
          const applyResult = inst ? launchFixApplyResultByInstance[inst.id] : null;
          const runReport = inst ? instanceRunReportById[inst.id] ?? null : null;
          const topCauses = (runReport?.topCauses?.length ? runReport.topCauses : plan?.causes ?? []).slice(0, 3);
          const findings = (runReport?.findings ?? []).slice(0, 6);
          const recentChanges = (runReport?.recentChanges ?? []).slice(-8);
          if (!inst || !plan) return null;
          return (
            <Modal
              title={`Fix My Instance · ${inst.name}`}
              size="wide"
              onClose={() => setLaunchFixModalInstanceId(null)}
            >
              <div className="modalBody">
                <div className="p" style={{ marginTop: 0 }}>
                  Local diagnosis only. Review likely causes, recent changes, then apply reversible actions.
                </div>
                <div className="card launchFixExplainCard">
                  <div className="rowBetween">
                    <div style={{ fontWeight: 900 }}>Why this likely happened</div>
                    {runReport ? (
                      <span className={`chip ${runReport.exitKind === "crashed" ? "danger" : "subtle"}`}>
                        {runReport.exitKind}
                        {typeof runReport.exitCode === "number" ? ` (${runReport.exitCode})` : ""}
                      </span>
                    ) : null}
                  </div>
                  <div className="row launchFixCauseChips">
                    {topCauses.length > 0 ? (
                      topCauses.map((cause) => (
                        <span key={cause} className="chip subtle">{cause}</span>
                      ))
                    ) : (
                      <span className="muted">No high-confidence causes available yet.</span>
                    )}
                  </div>
                  {findings.length > 0 ? (
                    <div className="launchFixFindingsList">
                      {findings.map((finding) => (
                        <div key={finding.id} className="launchFixFindingItem">
                          <div className="rowBetween" style={{ gap: 10 }}>
                            <div style={{ fontWeight: 800 }}>{finding.title}</div>
                            <span className="chip subtle">{Math.round((finding.confidence ?? 0) * 100)}%</span>
                          </div>
                          <div className="muted">{finding.explanation}</div>
                          {finding.modId ? (
                            <div className="muted">Mod: {finding.modId}</div>
                          ) : null}
                          {finding.evidence?.[0] ? (
                            <div className="muted">Evidence: {finding.evidence[0]}</div>
                          ) : null}
                        </div>
                      ))}
                    </div>
                  ) : null}
                  <div style={{ marginTop: 10, fontWeight: 820 }}>What changed recently?</div>
                  {recentChanges.length > 0 ? (
                    <div className="launchFixRecentList">
                      {recentChanges.map((entry) => (
                        <div key={entry.id} className="launchFixRecentItem">
                          <span className="chip subtle">{entry.kind.replace(/_/g, " ")}</span>
                          <span>{entry.summary}</span>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="muted" style={{ marginTop: 4 }}>
                      No recent instance changes were recorded yet.
                    </div>
                  )}
                </div>
                <div className="card launchFixActionsCard">
                  <div className="rowBetween">
                    <div style={{ fontWeight: 900 }}>What to try next</div>
                    <span className="chip subtle">{draft.filter((item) => item.selected).length}/{draft.length} selected</span>
                  </div>
                  <div className="launchFixActionList">
                    {draft.map((action) => (
                      <label key={action.id} className="launchFixActionRow">
                        <div className="row" style={{ marginTop: 0, gap: 8 }}>
                          <input
                            type="checkbox"
                            checked={action.selected}
                            onChange={(e) =>
                              setLaunchFixPlanDraftByInstance((prev) => ({
                                ...prev,
                                [inst.id]: (prev[inst.id] ?? []).map((row) =>
                                  row.id === action.id ? { ...row, selected: e.target.checked } : row
                                ),
                              }))
                            }
                          />
                          <div className="launchFixActionMain">
                            <div style={{ fontWeight: 800 }}>{action.title}</div>
                            <div className="muted">{action.detail}</div>
                            <div className="muted">
                              Dry run: {launchFixDryRunByActionId[action.id] ?? launchFixActionDryRunSummary(action)}
                            </div>
                          </div>
                        </div>
                        <div className="launchFixActionMeta">
                          <span className="chip subtle">{action.kind}</span>
                          <button
                            type="button"
                            className="btn subtle"
                            onClick={(event) => {
                              event.preventDefault();
                              void previewLaunchFixAction(inst, action);
                            }}
                          >
                            Dry run
                          </button>
                        </div>
                      </label>
                    ))}
                  </div>
                </div>
                {applyResult ? (
                  <div className="noticeBox" style={{ marginTop: 10 }}>
                    Applied {applyResult.applied}, failed {applyResult.failed}, skipped {applyResult.skipped}.
                  </div>
                ) : null}
              </div>
              <div className="footerBar">
                <button
                  className="btn"
                  onClick={() => void prepareLaunchFixPlan(inst)}
                  disabled={launchFixBusyInstanceId === inst.id}
                >
                  {launchFixBusyInstanceId === inst.id ? "Refreshing…" : "Refresh plan"}
                </button>
                <button
                  className="btn primary"
                  onClick={() => void applyLaunchFixPlan(inst)}
                  disabled={launchFixApplyBusyInstanceId === inst.id}
                >
                  {launchFixApplyBusyInstanceId === inst.id ? "Applying…" : "Apply selected fixes"}
                </button>
              </div>
            </Modal>
          );
        })()
      ) : null}

      {fullHistoryModalInstanceId ? (
        (() => {
          const inst = instances.find((item) => item.id === fullHistoryModalInstanceId);
          if (!inst) return null;
          const filter = fullHistoryFilterByInstance[inst.id] ?? "all";
          const search = String(fullHistorySearchByInstance[inst.id] ?? "");
          const busy = Boolean(fullHistoryBusyByInstance[inst.id]);
          const hasMore = Boolean(fullHistoryHasMoreByInstance[inst.id]);
          const rows = (fullHistoryByInstance[inst.id] ?? [])
            .map((event) =>
              toRecentActivityEntry({
                id: `modal:${event.id}`,
                atMs: parseDateLike(event.at)?.getTime() ?? 0,
                tone: inferActivityTone(`${event.kind} ${event.summary}`),
                message: event.summary,
                rawKind: event.kind,
                sourceLabel: humanizeToken(event.kind),
              })
            )
            .filter((entry) => {
              if (filter !== "all" && entry.category !== filter) return false;
              const normalizedSearch = search.trim().toLowerCase();
              if (!normalizedSearch) return true;
              const text = `${entry.message} ${entry.target} ${entry.rawKind}`.toLowerCase();
              return text.includes(normalizedSearch);
            });
          return (
            <Modal
              title={`Instance events · ${inst.name}`}
              size="wide"
              className="fullHistoryModal"
              onClose={() => setFullHistoryModalInstanceId(null)}
            >
              <div className="modalBody">
                <FullHistoryView
                  rows={rows}
                  filter={filter}
                  onFilterChange={(value) =>
                    setFullHistoryFilterByInstance((prev) => ({
                      ...prev,
                      [inst.id]: value,
                    }))
                  }
                  search={search}
                  onSearchChange={(value) =>
                    setFullHistorySearchByInstance((prev) => ({
                      ...prev,
                      [inst.id]: value,
                    }))
                  }
                  busy={busy}
                  hasMore={hasMore}
                  onRefresh={() => void loadFullHistoryPage(inst.id, { reset: true })}
                  onLoadOlder={() => void loadFullHistoryPage(inst.id)}
                  storeLimit={INSTANCE_HISTORY_STORE_LIMIT}
                  coalesceWindowMs={RECENT_ACTIVITY_COALESCE_WINDOW_MS}
                />
              </div>
            </Modal>
          );
        })()
      ) : null}

      {supportBundleModalInstanceId ? (
        (() => {
          const inst = instances.find((item) => item.id === supportBundleModalInstanceId);
          if (!inst) return null;
          return (
            <Modal
              title={`Export support bundle · ${inst.name}`}
              onClose={() => {
                if (supportBundleBusy) return;
                setSupportBundleModalInstanceId(null);
              }}
            >
              <div className="modalBody">
                <div className="p" style={{ marginTop: 0 }}>
                  Exports redacted logs, installed mods, config allowlist, and recent timing telemetry.
                </div>
                <label className="toggleRow" style={{ marginTop: 12 }}>
                  <input
                    type="checkbox"
                    checked={supportBundleIncludeRawLogs}
                    onChange={(e) => setSupportBundleIncludeRawLogs(e.target.checked)}
                    disabled={supportBundleBusy}
                  />
                  <span className="togglePill" />
                  <span>Include raw (unredacted) logs</span>
                </label>
                <div className="muted" style={{ marginTop: 8 }}>
                  Redaction is enabled by default. Only include raw logs when explicitly requested for debugging.
                </div>
              </div>
              <div className="footerBar">
                <button className="btn" onClick={() => setSupportBundleModalInstanceId(null)} disabled={supportBundleBusy}>
                  Cancel
                </button>
                <button
                  className="btn primary"
                  onClick={() => void onExportSupportBundle(inst, supportBundleIncludeRawLogs)}
                  disabled={supportBundleBusy}
                >
                  {supportBundleBusy ? "Exporting…" : "Export bundle"}
                </button>
              </div>
            </Modal>
          );
        })()
      ) : null}

      {showCreate ? (
        <Modal
          title="Creating an instance"
          className="createInstanceModal"
          onClose={() => (busy ? null : setShowCreate(false))}
        >
          <div className="modalBody createInstanceModalBody">
            <SegTabs
              tabs={[
                { id: "custom", label: "Custom" },
                { id: "file", label: "From File" },
                { id: "launcher", label: "Import From Launcher" },
              ]}
              active={createMode}
              onChange={(id) => setCreateMode(id as any)}
            />

            <div style={{ height: 18 }} />

            <div className="split">
              <div className="iconBox" title={createIconPath ?? "No icon selected"}>
                {createIconPath ? (
                  <LocalImage
                    path={createIconPath}
                    alt="Instance icon preview"
                    fallback={<div style={{ fontSize: 54, fontWeight: 900, opacity: 0.6 }}>⬚</div>}
                  />
                ) : (
                  <div style={{ fontSize: 54, fontWeight: 900, opacity: 0.6 }}>⬚</div>
                )}
              </div>

              <div>
                <div className="toolbarRow">
                  <button className="btn" onClick={() => void onPickCreateIcon()} disabled={busy !== null}>
                    <span className="btnIcon">
                      <Icon name="upload" size={17} />
                    </span>
                    Select icon
                  </button>
                  <button className="btn" onClick={() => setCreateIconPath(null)} disabled={busy !== null || !createIconPath}>
                    <span className="btnIcon">
                      <Icon name="x" size={17} />
                    </span>
                    Remove icon
                  </button>
                </div>

                {createMode === "custom" ? (
                  <>
                    <div className="sectionLabel sectionLabelTight">
                      Name
                    </div>
                    <input
                      className="input"
                      value={name}
                      onChange={(e) => setName(e.target.value)}
                      placeholder="e.g. Horror Pack"
                    />

                    <div className="sectionLabel">Loader</div>
                    <div className="pillRow">
                      {(["vanilla", "fabric", "forge", "neoforge", "quilt"] as Loader[]).map((value) => (
                        <div
                          key={value}
                          className={`pill ${loader === value ? "active" : ""}`}
                          onClick={() => setLoader(value)}
                        >
                          {loader === value ? "✓ " : ""}
                          {value === "vanilla"
                            ? "Vanilla"
                            : value === "neoforge"
                              ? "NeoForge"
                              : value[0].toUpperCase() + value.slice(1)}
                        </div>
                      ))}
                    </div>

                    <div className="sectionLabel">Game version</div>
                    <div className="rowBetween createVersionRow">
                      <div style={{ flex: 1 }}>
                        <Dropdown
                          value={mcVersion}
                          placeholder="Select game version"
                          groups={groupedCreateVersions}
                          onPick={setMcVersion}
                          placement="top"
                        />
                        {manifestError ? (
                          <div style={{ marginTop: 8, color: "var(--muted2)", fontWeight: 900, fontSize: 12 }}>
                            Couldn’t fetch official list (using fallback).
                          </div>
                        ) : null}
                      </div>

                      <div
                        className="checkboxRow"
                        onClick={() => setCreateAllVersions((v) => !v)}
                        title="Includes snapshots / pre-releases / RCs"
                      >
                        <div className={`checkbox ${createAllVersions ? "checked" : ""}`}>
                          <div />
                        </div>
                        <div>Show all versions</div>
                      </div>
                    </div>
                  </>
                ) : null}

                {createMode === "file" ? (
                  <>
                    <div className="sectionLabel sectionLabelTight">Modpack archive</div>
                    <div className="toolbarRow">
                      <button className="btn" onClick={() => void onPickCreateModpackFile()} disabled={busy !== null}>
                        <span className="btnIcon">
                          <Icon name="upload" size={17} />
                        </span>
                        Select .mrpack/.zip
                      </button>
                      <button className="btn" onClick={() => setCreatePackFilePath(null)} disabled={busy !== null || !createPackFilePath}>
                        <span className="btnIcon">
                          <Icon name="x" size={17} />
                        </span>
                        Clear
                      </button>
                    </div>
                    <input
                      className="input"
                      value={createPackFilePath ?? ""}
                      onChange={(e) => setCreatePackFilePath(e.target.value)}
                      placeholder="/path/to/modpack.mrpack"
                    />
                    <div className="sectionLabel">Instance name (optional)</div>
                    <input
                      className="input"
                      value={name}
                      onChange={(e) => setName(e.target.value)}
                      placeholder="Defaults to pack name"
                    />
                  </>
                ) : null}

                {createMode === "launcher" ? (
                  <>
                    <div className="sectionLabel sectionLabelTight">Launcher source</div>
                    <div className="toolbarRow">
                      <button className="btn" onClick={() => void refreshLauncherImportSources()} disabled={launcherImportBusy || busy !== null}>
                        {launcherImportBusy ? "Refreshing…" : "Refresh sources"}
                      </button>
                    </div>
                    <MenuSelect
                      value={selectedLauncherImportSourceId ?? ""}
                      labelPrefix="Source"
                      onChange={(v) => setSelectedLauncherImportSourceId(v)}
                      options={launcherImportSources.map((item) => ({
                        value: item.id,
                        label: `${item.label} · ${item.loader} · ${item.mc_version}`,
                      }))}
                      placement="top"
                    />
                    <div className="sectionLabel">Instance name (optional)</div>
                    <input
                      className="input"
                      value={name}
                      onChange={(e) => setName(e.target.value)}
                      placeholder="Defaults to source name"
                    />
                  </>
                ) : null}
              </div>
            </div>
          </div>

          <div className="footerBar">
            <button className="btn" onClick={() => setShowCreate(false)} disabled={busy !== null}>
              ✕ Cancel
            </button>
            <button
              className="btn primary"
              onClick={onCreate}
              disabled={
                busy !== null ||
                (createMode === "custom" && (!name.trim() || !mcVersion)) ||
                (createMode === "file" && !createPackFilePath) ||
                (createMode === "launcher" && !selectedLauncherImportSourceId)
              }
            >
              + {busy === "create" ? "Creating…" : "Create"}
            </button>
          </div>
        </Modal>
      ) : null}

      {installTarget ? (
        <Modal title="Install to instance" size="wide" onClose={() => setInstallTarget(null)}>
          <div className="modalBody">
            <div className="installModHeader">
              <div className="resultIcon" style={{ width: 56, height: 56, borderRadius: 16 }}>
                <RemoteImage src={installTarget.iconUrl} alt={`${installTarget.title} icon`} fallback={<div>⬚</div>} />
              </div>
              <div>
                <div className="h3" style={{ margin: 0 }}>{installTarget.title}</div>
                <div className="p" style={{ marginTop: 4 }}>
                  Source: {installTarget.source}. Type: {installTarget.contentType}.
                  {installTarget.contentType === "mods"
                    ? installTarget.source === "github"
                      ? " For GitHub mods, install requires explicit loader/game-version hints in release metadata."
                      : " The app will pick the latest compatible version (loader + game version) and install required dependencies when available."
                    : installTarget.contentType === "datapacks"
                      ? " Datapacks install into world datapacks folders. Direct install targets all detected worlds on that instance."
                      : " The app will install the latest compatible file and track it in lockfile."}
                </div>
              </div>
            </div>

            {installTarget.installNote ? (
              <div className={installTarget.installSupported === false ? "warningBox" : "noticeBox"}>
                {installTarget.installNote}
              </div>
            ) : null}

            {installProgress && installProgress.project_id === installTarget.projectId ? (
              <div className="card installProgressCard">
                <div className="installProgressTitle">
                  {installProgressTitleText}
                </div>
                <div className="installProgressBar">
                  <div
                    className={`installProgressFill ${installProgress.stage}${installProgressIndeterminate ? " indeterminate" : ""}`}
                    style={{ width: `${installProgressPercentValue ?? 0}%` }}
                  />
                </div>
                <div className="installProgressMeta">
                  <span>{installProgressPercentLabel || "Working…"}</span>
                  <span>{installProgressStageLabel}</span>
                  {installProgressShowTransferMetrics && installProgressTransferText ? (
                    <span>{installProgressTransferText}</span>
                  ) : null}
                  {installProgressShowTransferMetrics && installProgressSpeedText ? (
                    <span>{installProgressSpeedText}</span>
                  ) : null}
                  {installProgressShowTransferMetrics && installProgressElapsedSeconds != null ? (
                    <span>Elapsed {formatEtaSeconds(installProgressElapsedSeconds)}</span>
                  ) : null}
                  {installProgress.stage === "completed" ? (
                    <span>ETA done</span>
                  ) : installProgress.stage === "error" ? (
                    <span>ETA unavailable</span>
                  ) : installProgressShowTransferMetrics && !installProgressIndeterminate ? (
                    <span>ETA {formatEtaSeconds(installProgressEtaSeconds)}</span>
                  ) : null}
                </div>
              </div>
            ) : null}

            <div style={{ marginTop: 12 }}>
              <div className="searchPill" style={{ width: "100%" }}>
                <Icon name="search" />
                <input
                  className="input"
                  placeholder="Search instances…"
                  value={installInstanceQuery}
                  onChange={(e) => setInstallInstanceQuery(e.target.value)}
                />
              </div>
            </div>

            <div className="installList">
              {instances
                .filter((i) => {
                  const q = installInstanceQuery.trim().toLowerCase();
                  if (!q) return true;
                  return i.name.toLowerCase().includes(q);
                })
                .map((inst) => {
                  const preview = installPlanPreview[inst.id];
                  const previewBusy = installPlanPreviewBusy[inst.id];
                  const previewErr = installPlanPreviewErr[inst.id];
                  return (
                    <div key={inst.id} className="installRow">
                      <div className="installRowLeft">
                        <div className="installInstanceIcon">
                          <Icon name="box" />
                        </div>
                        <div>
                          <div className="installRowName">{inst.name}</div>
                          <div className="installRowMeta">
                            <span className="chip">{inst.loader}</span>
                            <span className="chip">{inst.mc_version}</span>
                          </div>
                          <div className={`installRowPreview ${previewErr ? "error" : ""}`}>
                            {installTarget.contentType !== "mods"
                              ? installTarget.contentType === "datapacks"
                                ? "Datapack will be copied to all detected worlds for this instance."
                                : `Will install 1 ${installTarget.contentType === "shaderpacks" ? "shaderpack" : installTarget.contentType === "resourcepacks" ? "resourcepack" : "item"}.`
                              : previewBusy
                              ? "Checking required dependencies…"
                              : previewErr
                                ? "Dependency preview unavailable."
                                : preview
                                  ? `Will install: ${preview.will_install_mods} mod${preview.will_install_mods === 1 ? "" : "s"}${preview.dependency_mods > 0 ? ` (${preview.dependency_mods} required dependenc${preview.dependency_mods === 1 ? "y" : "ies"})` : ""}`
                                  : "Checking required dependencies…"}
                          </div>
                        </div>
                      </div>

                      <button
                        className="btn primary installAction"
                        onClick={() => onInstallToInstance(inst)}
                        disabled={
                          installTarget.installSupported === false ||
                          (installingKey !== null &&
                            installingKey !==
                              `${inst.id}:${installTarget.source}:${installTarget.contentType}:${installTarget.projectId}`)
                        }
                      >
                        <Icon name="download" />
                        {installingKey === `${inst.id}:${installTarget.source}:${installTarget.contentType}:${installTarget.projectId}`
                          ? `Installing ${installProgressPercentLabel || ""}`.trim()
                          : "Install"}
                      </button>
                    </div>
                  );
                })}

              {instances.length === 0 ? (
                <div className="emptyState" style={{ marginTop: 8 }}>
                  No instances yet — create one first.
                </div>
              ) : null}
            </div>
          </div>

          <div className="footerBar">
            <button className="btn" onClick={() => setInstallTarget(null)}>
              Close
            </button>
            <button
              className="btn"
              onClick={() => {
                setInstallTarget(null);
                setShowCreate(true);
              }}
            >
              + Create new instance
            </button>
          </div>
        </Modal>
      ) : null}

      {modpackAddTarget ? (
        <Modal title="Add to modpack" size="wide" onClose={() => setModpackAddTarget(null)}>
          <div className="modalBody">
            <div className="installModHeader">
              <div className="resultIcon" style={{ width: 56, height: 56, borderRadius: 16 }}>
                {modpackAddTarget.iconUrl ? <img src={modpackAddTarget.iconUrl} alt="" /> : <div>⬚</div>}
              </div>
              <div>
                <div className="h3" style={{ margin: 0 }}>{modpackAddTarget.title}</div>
                <div className="p" style={{ marginTop: 4 }}>
                  Add this discover result to a Modpack Maker layer. Type: {modpackAddTarget.contentType}. Source: {modpackAddTarget.source}.
                </div>
              </div>
            </div>

            {modpackAddErr ? <div className="errorBox">{modpackAddErr}</div> : null}

            {modpackAddSpecsBusy ? (
              <div className="card" style={{ marginTop: 10, padding: 12, borderRadius: 14 }}>
                Loading modpacks...
              </div>
            ) : modpackAddSpecs.length === 0 ? (
              <div className="card" style={{ marginTop: 10, padding: 12, borderRadius: 14 }}>
                <div style={{ fontWeight: 900 }}>No modpack specs found.</div>
                <div className="muted" style={{ marginTop: 6 }}>
                  Create a modpack in Creator Studio first, then return to Discover and add entries here.
                </div>
                <div className="row" style={{ marginTop: 10 }}>
                  <button
                    className="btn"
                    onClick={() => {
                      setModpackAddTarget(null);
                      setRoute("modpacks");
                      setModpacksStudioTab("creator");
                    }}
                  >
                    Open Creator Studio
                  </button>
                </div>
              </div>
            ) : (
              <div className="card" style={{ marginTop: 10, padding: 12, borderRadius: 14 }}>
                <div style={{ display: "grid", gap: 10, gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))" }}>
                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Modpack</span>
                    <select
                      className="input"
                      value={modpackAddSpecId}
                      onChange={(e) => {
                        const nextId = e.target.value;
                        setModpackAddSpecId(nextId);
                        const nextSpec = modpackAddSpecs.find((spec) => spec.id === nextId);
                        setModpackAddLayerId(nextSpec ? defaultModpackLayerId(nextSpec) : "");
                      }}
                      title="Target modpack spec."
                    >
                      {modpackAddSpecs.map((spec) => (
                        <option key={spec.id} value={spec.id}>{spec.name}</option>
                      ))}
                    </select>
                  </label>

                  <label style={{ display: "grid", gap: 6 }}>
                    <span className="muted">Layer</span>
                    <select
                      className="input"
                      value={modpackAddLayerId}
                      onChange={(e) => setModpackAddLayerId(e.target.value)}
                      title="User Additions is the normal place to add mods."
                    >
                      {(selectedModpackAddSpec?.layers ?? []).map((layer) => (
                        <option key={layer.id} value={layer.id}>
                          {layer.name}{layer.is_frozen ? " (frozen)" : ""}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>

                <div className="row" style={{ gap: 8, marginTop: 10, flexWrap: "wrap" }}>
                  <label style={{ display: "grid", gap: 6, minWidth: 180 }}>
                    <span className="muted">Requirement</span>
                    <select
                      className="input"
                      value={modpackAddRequired ? "required" : "optional"}
                      onChange={(e) => setModpackAddRequired(e.target.value === "required")}
                    >
                      <option value="required">Required</option>
                      <option value="optional">Optional</option>
                    </select>
                  </label>
                  <label style={{ display: "grid", gap: 6, minWidth: 180 }}>
                    <span className="muted">Default state</span>
                    <select
                      className="input"
                      value={modpackAddEnabledByDefault ? "enabled" : "disabled"}
                      onChange={(e) => setModpackAddEnabledByDefault(e.target.value === "enabled")}
                    >
                      <option value="enabled">Enabled</option>
                      <option value="disabled">Disabled</option>
                    </select>
                  </label>
                  <label style={{ display: "grid", gap: 6, minWidth: 180 }}>
                    <span className="muted">Channel policy</span>
                    <select
                      className="input"
                      value={modpackAddChannelPolicy}
                      onChange={(e) => setModpackAddChannelPolicy((e.target.value as any) ?? "stable")}
                    >
                      <option value="stable">Stable only</option>
                      <option value="beta">Allow beta</option>
                      <option value="alpha">Allow alpha</option>
                    </select>
                  </label>
                  <label style={{ display: "grid", gap: 6, minWidth: 180 }}>
                    <span className="muted">Fallback policy</span>
                    <select
                      className="input"
                      value={modpackAddFallbackPolicy}
                      onChange={(e) => setModpackAddFallbackPolicy((e.target.value as any) ?? "inherit")}
                    >
                      <option value="inherit">Inherit global</option>
                      <option value="strict">Strict</option>
                      <option value="smart">Smart</option>
                      <option value="loose">Loose</option>
                    </select>
                  </label>
                </div>

                <details style={{ marginTop: 10 }}>
                  <summary style={{ cursor: "pointer" }}>Advanced entry options</summary>
                  <div style={{ marginTop: 8, display: "grid", gap: 8, gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))" }}>
                    <label style={{ display: "grid", gap: 6 }}>
                      <span className="muted">Pinned version/file id</span>
                      <input
                        className="input"
                        value={modpackAddPinnedVersion}
                        onChange={(e) => setModpackAddPinnedVersion(e.target.value)}
                        placeholder="Optional"
                      />
                    </label>
                    <label style={{ display: "grid", gap: 6 }}>
                      <span className="muted">Notes</span>
                      <input
                        className="input"
                        value={modpackAddNotes}
                        onChange={(e) => setModpackAddNotes(e.target.value)}
                        placeholder="Optional"
                      />
                    </label>
                  </div>
                </details>
              </div>
            )}
          </div>

          <div className="footerBar">
            <button className="btn" onClick={() => setModpackAddTarget(null)} disabled={modpackAddBusy}>
              Close
            </button>
            <button
              className="btn primary"
              onClick={() => void onAddDiscoverTargetToModpack()}
              disabled={modpackAddBusy || modpackAddSpecsBusy || modpackAddSpecs.length === 0}
            >
              {modpackAddBusy ? "Adding..." : "Add to modpack"}
            </button>
          </div>
        </Modal>
      ) : null}

      {projectOpen || projectBusy || projectErr ? (
        <Modal
          title={projectOpen?.title ?? (projectBusy ? "Loading…" : "Mod details")}
          onClose={closeProjectOverlays}
          size="wide"
        >
          <div className="modalBody">
            {projectErr ? <div className="errorBox">{projectErr}</div> : null}
            {projectBusy && !projectOpen ? (
              <div className="card" style={{ padding: 16, borderRadius: 22 }}>
                Loading…
              </div>
            ) : null}

            {projectOpen ? (
              <div className="projectDetailWrap">
                <div className="card projectHeroCard">
                  <div className="projectHeroAura" />

                  <div className="projectHero">
                    <div className="resultIcon projectIcon projectIconLarge">
                      <RemoteImage
                        src={projectOpen.icon_url}
                        alt={`${projectOpen.title} icon`}
                        fallback={<div>⬚</div>}
                      />
                    </div>

                    <div className="projectHeroMain">
                      <div className="projectEyebrow">
                        Modrinth • {projectOpen.slug || projectOpen.id}
                      </div>
                      <div className="projectHeroTitleRow">
                        <div className="projectHeroTitle">{projectOpen.title}</div>
                        <div className="chip">Updated {formatDate(latestProjectVersion?.date_published)}</div>
                      </div>
                      <div className="p projectHeroDesc">{projectOpen.description}</div>

                      <div className="projectChipRow">
                        <span className="chip">Client: {humanizeToken(projectOpen.client_side)}</span>
                        <span className="chip">Server: {humanizeToken(projectOpen.server_side)}</span>
                        {projectOpen.categories?.slice(0, 8).map((c) => (
                          <span key={c} className="chip">
                            {humanizeToken(c)}
                          </span>
                        ))}
                      </div>
                    </div>
                  </div>

                  <div className="projectStatsGrid">
                    <div className="projectStatCard">
                      <div className="projectStatLabel">Downloads</div>
                      <div className="projectStatValue">{formatCompact(projectOpen.downloads)}</div>
                    </div>
                    <div className="projectStatCard">
                      <div className="projectStatLabel">Likes</div>
                      <div className="projectStatValue">{formatCompact(projectOpen.followers)}</div>
                    </div>
                    <div className="projectStatCard">
                      <div className="projectStatLabel">Versions</div>
                      <div className="projectStatValue">{sortedProjectVersions.length || projectOpen.versions.length}</div>
                    </div>
                    <div className="projectStatCard">
                      <div className="projectStatLabel">Team members</div>
                      <div className="projectStatValue">{projectMembers.length || "—"}</div>
                    </div>
                  </div>

                  <div className="projectActionRow">
                    {projectPageUrl ? (
                      <a className="btn projectActionBtn" href={projectPageUrl} target="_blank" rel="noreferrer">
                        Open on Modrinth
                      </a>
                    ) : null}
                    {projectPageUrl ? (
                      <button className="btn projectActionBtn" onClick={() => copyProjectText("Link", projectPageUrl)}>
                        Copy link
                      </button>
                    ) : null}
                    <button className="btn projectActionBtn" onClick={() => copyProjectText("Project ID", projectOpen.id)}>
                      Copy project ID
                    </button>
                    {latestPrimaryFile ? (
                      <button
                        className="btn projectActionBtn"
                        onClick={() => copyProjectText("Primary file", latestPrimaryFile.filename)}
                      >
                        Copy primary file
                      </button>
                    ) : null}
                  </div>

                  {projectCopyNotice ? <div className="projectCopyNotice">{projectCopyNotice}</div> : null}
                </div>

                <div className="projectTabSticky">
                  <SegmentedControl
                    value={projectDetailTab}
                    options={PROJECT_DETAIL_TABS}
                    onChange={(v) => setProjectDetailTab((v ?? "overview") as ProjectDetailTab)}
                    variant="scroll"
                    className="projectTabBar"
                  />
                </div>

                {projectDetailTab === "overview" ? (
                  <>
                    <div className="projectOverviewCols">
                      <div className="projectOverviewCol">
                        <div className="card projectSectionCard projectSectionLatest">
                          <div className="projectSectionTitle">Latest release</div>
                          {latestProjectVersion ? (
                            <div className="projectLatestCard">
                              <div className="projectVersionTitle">{latestProjectVersion.version_number}</div>
                              <div className="projectVersionMeta">
                                <span>{formatDate(latestProjectVersion.date_published)}</span>
                                <span>↓ {formatCompact(latestProjectVersion.downloads ?? 0)}</span>
                                <span>{latestProjectVersion.files.length} files</span>
                              </div>
                              {latestPrimaryFile ? (
                                <div className="projectLatestFile">
                                  {latestPrimaryFile.filename} • {formatFileSize(latestPrimaryFile.size)}
                                </div>
                              ) : null}
                            </div>
                          ) : (
                            <div className="muted">No release data available.</div>
                          )}
                        </div>

                        <div className="card projectSectionCard projectSectionDesc">
                          <div className="projectSectionTitle">Description</div>
                          <MarkdownBlock
                            className="projectBodyText projectMarkdown"
                            text={projectOpen.body?.trim() ? projectOpen.body : projectOpen.description}
                          />
                        </div>

                        <div className="card projectSectionCard projectSectionLinks">
                          <div className="projectSectionTitle">Links</div>
                          <div className="projectLinks">
                            {[
                              { label: "Website", href: projectOpen.link_urls?.homepage },
                              { label: "Source", href: projectOpen.source_url },
                              { label: "Issues", href: projectOpen.issues_url },
                              { label: "Wiki", href: projectOpen.wiki_url ?? undefined },
                              { label: "Discord", href: projectOpen.discord_url ?? undefined },
                            ]
                              .filter((x) => !!x.href)
                              .map((x) => (
                                <a
                                  key={x.label}
                                  className="projectLinkBtn"
                                  href={x.href}
                                  target="_blank"
                                  rel="noreferrer"
                                >
                                  {x.label}
                                </a>
                              ))}
                            {!projectOpen.link_urls?.homepage &&
                            !projectOpen.source_url &&
                            !projectOpen.issues_url &&
                            !projectOpen.wiki_url &&
                            !projectOpen.discord_url ? (
                              <div className="muted">No external links provided.</div>
                            ) : null}
                          </div>
                        </div>
                      </div>

                      <div className="projectOverviewCol">
                        <div className="card projectSectionCard projectSectionCompat">
                          <div className="projectSectionTitle">Compatibility</div>
                          <div className="projectFacetGroup">
                            <div className="projectFacetLabel">Loaders</div>
                            <div className="projectFacetWrap">
                              {projectLoaderFacets.length ? (
                                projectLoaderFacets.map((loaderName) => (
                                  <span key={loaderName} className="chip">
                                    {humanizeToken(loaderName)}
                                  </span>
                                ))
                              ) : (
                                <span className="muted">No loader data.</span>
                              )}
                            </div>
                          </div>
                          <div className="projectFacetGroup">
                            <div className="projectFacetLabel">Game versions</div>
                            <div className="projectFacetWrap">
                              {projectGameVersionFacets.length ? (
                                projectGameVersionFacets.map((gameVersion) => (
                                  <span key={gameVersion} className="chip">
                                    {gameVersion}
                                  </span>
                                ))
                              ) : (
                                <span className="muted">No game version data.</span>
                              )}
                            </div>
                          </div>
                        </div>

                        <div className="card projectSectionCard projectSectionTeam">
                          <div className="projectSectionTitle">Team</div>
                          {projectMembers.length === 0 ? (
                            <div className="muted">No member data returned.</div>
                          ) : (
                            <div className="projectMemberList">
                              {projectMembers.slice(0, 10).map((m) => {
                                const displayName = m.user.name || m.user.username;
                                return (
                                  <div key={`${m.role}:${m.user.username}`} className="projectMemberRow">
                                    <div className="projectMemberIdentity">
                                      <div className="projectMemberAvatar">
                                        {m.user.avatar_url ? (
                                          <img src={m.user.avatar_url} alt={displayName} />
                                        ) : (
                                          displayName.slice(0, 1).toUpperCase()
                                        )}
                                      </div>
                                      <div>
                                        <div className="projectMemberName">{displayName}</div>
                                        <div className="projectMemberRole">@{m.user.username}</div>
                                      </div>
                                    </div>
                                    <div className="chip">{humanizeToken(m.role)}</div>
                                  </div>
                                );
                              })}
                            </div>
                          )}
                        </div>
                      </div>
                    </div>
                  </>
                ) : null}

                {projectDetailTab === "versions" ? (
                  <div className="card projectSectionCard">
                    <div className="projectSectionTitle">Versions</div>
                    {sortedProjectVersions.length === 0 ? (
                      <div className="muted">No version list available.</div>
                    ) : (
                      <div className="projectVersionList">
                        {sortedProjectVersions.slice(0, 30).map((v) => {
                          const primaryFile =
                            v.files.find((f) => f.primary) ?? v.files[0] ?? null;
                          return (
                            <div key={v.id} className="projectVersionRow">
                              <div className="projectVersionMain">
                                <div className="projectVersionTitle">{v.version_number}</div>
                                <div className="projectVersionMeta">
                                  <span>{formatDate(v.date_published)}</span>
                                  <span>{v.loaders.join(", ") || "Loader n/a"}</span>
                                  <span>{v.game_versions.slice(0, 5).join(", ") || "Version n/a"}</span>
                                </div>
                                {primaryFile ? (
                                  <div className="projectVersionFile">
                                    {primaryFile.filename} • {formatFileSize(primaryFile.size)}
                                  </div>
                                ) : null}
                              </div>

                              <div className="projectVersionAside">
                                <div className="chip">↓ {formatCompact(v.downloads ?? 0)}</div>
                                <div className="chip">{v.files.length} file{v.files.length === 1 ? "" : "s"}</div>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                ) : null}

                {projectDetailTab === "changelog" ? (
                  <div className="card projectSectionCard">
                    <div className="projectSectionTitle">Recent changelogs</div>
                    {changelogVersions.length === 0 ? (
                      <div className="muted">No changelogs were returned by Modrinth for recent versions.</div>
                    ) : (
                      <div className="projectChangelogList">
                        {changelogVersions.map((v) => (
                          <div key={`changelog:${v.id}`} className="projectChangeItem">
                            <div className="projectChangeHeader">
                              <div className="projectVersionTitle">{v.version_number}</div>
                              <div className="chip">{formatDate(v.date_published)}</div>
                            </div>
                            <pre className="projectBodyText projectChangelogText">
                              {toReadableBody(v.changelog)}
                            </pre>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ) : null}

                <div className="p projectInstallHint">
                  Install downloads the latest compatible jar and required dependencies for your selected instance, then writes everything to lockfile.
                </div>
              </div>
            ) : null}
          </div>

          <div className="footerBar">
            <button className="btn" onClick={closeProjectOverlays}>
              Close
            </button>
            <button
              className="btn"
              disabled={!projectOpen || projectOpenContentType === "modpacks"}
              title={projectOpen ? (projectOpenContentType === "modpacks" ? "Import modpacks as template layers from Creator Studio" : "Add this project to a modpack layer") : "Loading..."}
              onClick={() => {
                if (!projectOpen) return;
                void openAddToModpack({
                  source: "modrinth",
                  projectId: projectOpen.id,
                  title: projectOpen.title,
                  contentType: projectOpenContentType,
                  slug: projectOpen.slug ?? null,
                  iconUrl: projectOpen.icon_url,
                  description: projectOpen.description,
                }, discoverAddContext ? { modpackId: discoverAddContext.modpackId, layerId: discoverAddContext.layerId ?? null } : undefined);
              }}
            >
              Add to modpack
            </button>
            <button
              className="btn primary installAction"
              disabled={!projectOpen || projectOpenContentType === "modpacks"}
              title={projectOpen ? (projectOpenContentType === "modpacks" ? "Use Import template in Modpacks & Presets" : "Install to an instance") : "Loading..."}
              onClick={() => {
                if (!projectOpen) return;
                openInstall({
                  source: "modrinth",
                  projectId: projectOpen.id,
                  title: projectOpen.title,
                  contentType: projectOpenContentType,
                  iconUrl: projectOpen.icon_url,
                  description: projectOpen.description,
                });
              }}
            >
              <Icon name="download" /> {projectOpenContentType === "modpacks" ? "Template only" : "Install to instance"}
            </button>
          </div>
        </Modal>
      ) : null}

      {curseforgeOpen || curseforgeBusy || curseforgeErr ? (
        <Modal
          title={curseforgeOpen?.title ?? (curseforgeBusy ? "Loading…" : "CurseForge details")}
          onClose={closeProjectOverlays}
          size="wide"
        >
          <div className="modalBody">
            {curseforgeErr ? <div className="errorBox">{curseforgeErr}</div> : null}
            {curseforgeBusy && !curseforgeOpen ? (
              <div className="card" style={{ padding: 16, borderRadius: 22 }}>
                Loading…
              </div>
            ) : null}

            {curseforgeOpen ? (
              <div className="projectDetailWrap">
                <div className="card projectHeroCard">
                  <div className="projectHeroAura" />

                  <div className="projectHero">
                    <div className="resultIcon projectIcon projectIconLarge">
                      <RemoteImage
                        src={curseforgeOpen.icon_url}
                        alt={`${curseforgeOpen.title} icon`}
                        fallback={<div>⬚</div>}
                      />
                    </div>

                    <div className="projectHeroMain">
                      <div className="projectEyebrow">
                        CurseForge • {curseforgeOpen.slug || curseforgeOpen.project_id}
                      </div>
                      <div className="projectHeroTitleRow">
                        <div className="projectHeroTitle">{curseforgeOpen.title}</div>
                        <div className="chip">Updated {formatDate(curseforgeOpen.date_modified)}</div>
                      </div>
                      <div className="p projectHeroDesc">{curseforgeOpen.summary}</div>
                      <div className="projectChipRow">
                        {curseforgeOpen.categories.slice(0, 8).map((c) => (
                          <span key={c} className="chip">
                            {humanizeToken(c)}
                          </span>
                        ))}
                      </div>
                    </div>
                  </div>

                  <div className="projectStatsGrid">
                    <div className="projectStatCard">
                      <div className="projectStatLabel">Downloads</div>
                      <div className="projectStatValue">{formatCompact(curseforgeOpen.downloads)}</div>
                    </div>
                    <div className="projectStatCard">
                      <div className="projectStatLabel">Files</div>
                      <div className="projectStatValue">{curseforgeOpen.files.length}</div>
                    </div>
                    <div className="projectStatCard">
                      <div className="projectStatLabel">Authors</div>
                      <div className="projectStatValue">
                        {curseforgeOpen.author_names.length ? curseforgeOpen.author_names.length : "—"}
                      </div>
                    </div>
                    <div className="projectStatCard">
                      <div className="projectStatLabel">Provider</div>
                      <div className="projectStatValue">CurseForge</div>
                    </div>
                  </div>

                  <div className="projectActionRow">
                    {curseforgeOpen.external_url ? (
                      <a className="btn projectActionBtn" href={curseforgeOpen.external_url} target="_blank" rel="noreferrer">
                        Open on CurseForge
                      </a>
                    ) : null}
                    {curseforgeOpen.external_url ? (
                      <button
                        className="btn projectActionBtn"
                        onClick={() => copyProjectText("Link", curseforgeOpen.external_url!)}
                      >
                        Copy link
                      </button>
                    ) : null}
                    <button
                      className="btn projectActionBtn"
                      onClick={() => copyProjectText("Project ID", curseforgeOpen.project_id)}
                    >
                      Copy project ID
                    </button>
                  </div>

                  {projectCopyNotice ? <div className="projectCopyNotice">{projectCopyNotice}</div> : null}
                </div>

                <div className="projectTabSticky">
                  <SegmentedControl
                    value={curseforgeDetailTab}
                    options={CURSEFORGE_DETAIL_TABS}
                    onChange={(v) => setCurseforgeDetailTab((v ?? "overview") as CurseforgeDetailTab)}
                    variant="scroll"
                    className="projectTabBar"
                  />
                </div>

                {curseforgeDetailTab === "overview" ? (
                  <div className="projectOverviewCols">
                    <div className="projectOverviewCol">
                      <div className="card projectSectionCard projectSectionDesc">
                        <div className="projectSectionTitle">Description</div>
                        <RichTextBlock
                          className="projectBodyText projectMarkdown projectRichHtml"
                          text={curseforgeOpen.description || curseforgeOpen.summary}
                        />
                      </div>
                    </div>
                    <div className="projectOverviewCol">
                      <div className="card projectSectionCard projectSectionTeam">
                        <div className="projectSectionTitle">Authors</div>
                        {curseforgeOpen.author_names.length === 0 ? (
                          <div className="muted">No author data returned.</div>
                        ) : (
                          <div className="projectMemberList">
                            {curseforgeOpen.author_names.map((name) => (
                              <div key={name} className="projectMemberRow">
                                <div className="projectMemberIdentity">
                                  <div className="projectMemberAvatar">{name.slice(0, 1).toUpperCase()}</div>
                                  <div>
                                    <div className="projectMemberName">{name}</div>
                                    <div className="projectMemberRole">CurseForge author</div>
                                  </div>
                                </div>
                              </div>
                            ))}
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                ) : null}

                {curseforgeDetailTab === "files" ? (
                  <div className="card projectSectionCard">
                    <div className="projectSectionTitle">Files</div>
                    {curseforgeOpen.files.length === 0 ? (
                      <div className="muted">No file list available.</div>
                    ) : (
                      <div className="projectVersionList">
                        {curseforgeOpen.files.slice(0, 40).map((f) => (
                          <div key={f.file_id} className="projectVersionRow">
                            <div className="projectVersionMain">
                              <div className="projectVersionTitle">
                                {f.display_name || f.file_name || `File ${f.file_id}`}
                              </div>
                              <div className="projectVersionMeta">
                                <span>{formatDate(f.file_date)}</span>
                                <span>{f.file_name}</span>
                              </div>
                              {f.game_versions.length > 0 ? (
                                <div className="projectVersionFile">
                                  {f.game_versions.slice(0, 10).join(", ")}
                                </div>
                              ) : null}
                            </div>
                            <div className="projectVersionAside">
                              <div className="chip">#{f.file_id}</div>
                              {f.download_url ? (
                                <a className="chip" href={f.download_url} target="_blank" rel="noreferrer">
                                  Direct URL
                                </a>
                              ) : null}
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ) : null}

                {curseforgeDetailTab === "changelog" ? (
                  <div className="card projectSectionCard">
                    <div className="projectSectionTitle">Changelog</div>
                    <div className="muted">
                      CurseForge project-level changelogs are not consistently available via the current API. Use the files tab and the CurseForge page for detailed release notes.
                    </div>
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>

          <div className="footerBar">
            <button className="btn" onClick={closeProjectOverlays}>
              Close
            </button>
            <button
              className="btn"
              disabled={!curseforgeOpen || curseforgeOpenContentType === "modpacks"}
              title={curseforgeOpen ? (curseforgeOpenContentType === "modpacks" ? "Import modpacks as template layers from Creator Studio" : "Add this project to a modpack layer") : "Loading..."}
              onClick={() => {
                if (!curseforgeOpen) return;
                void openAddToModpack({
                  source: "curseforge",
                  projectId: curseforgeOpen.project_id,
                  title: curseforgeOpen.title,
                  contentType: curseforgeOpenContentType,
                  slug: curseforgeOpen.slug ?? null,
                  iconUrl: curseforgeOpen.icon_url,
                  description: curseforgeOpen.summary,
                }, discoverAddContext ? { modpackId: discoverAddContext.modpackId, layerId: discoverAddContext.layerId ?? null } : undefined);
              }}
            >
              Add to modpack
            </button>
            <button
              className="btn primary installAction"
              disabled={!curseforgeOpen || curseforgeOpenContentType === "modpacks"}
              title={curseforgeOpen ? (curseforgeOpenContentType === "modpacks" ? "Use Import template in Modpacks & Presets" : "Install to an instance") : "Loading..."}
              onClick={() => {
                if (!curseforgeOpen) return;
                openInstall({
                  source: "curseforge",
                  projectId: curseforgeOpen.project_id,
                  title: curseforgeOpen.title,
                  contentType: curseforgeOpenContentType,
                  iconUrl: curseforgeOpen.icon_url,
                  description: curseforgeOpen.summary,
                });
              }}
            >
              <Icon name="download" /> {curseforgeOpenContentType === "modpacks" ? "Template only" : "Install to instance"}
            </button>
          </div>
        </Modal>
      ) : null}

      {githubOpen || githubBusy || githubErr ? (
        <Modal
          title={githubDetail?.title || githubOpen?.title || (githubBusy ? "Loading…" : "GitHub details")}
          onClose={closeProjectOverlays}
          size="wide"
        >
          <div className="modalBody">
            {githubErr ? <div className="errorBox">{githubErr}</div> : null}
            {githubBusy && !githubDetail ? (
              <div className="card" style={{ padding: 16, borderRadius: 22 }}>
                Loading…
              </div>
            ) : null}
            {githubOpen ? (
              <div className="projectDetailWrap">
                <div className="card projectHeroCard">
                  <div className="projectHeroAura" />
                  <div className="projectHero">
                    <div className="resultIcon projectIcon projectIconLarge">
                      <RemoteImage
                        src={githubDetail?.icon_url ?? githubOpen.icon_url}
                        alt={`${githubDetail?.title || githubOpen.title} icon`}
                        fallback={<div>⬚</div>}
                      />
                    </div>
                    <div className="projectHeroMain">
                      <div className="projectEyebrow">
                        GitHub • {githubDetail?.project_id || githubOpen.project_id}
                      </div>
                      <div className="projectHeroTitleRow">
                        <div className="projectHeroTitle">{githubDetail?.title || githubOpen.title}</div>
                        {(githubDetail?.date_modified || githubOpen.date_modified) ? (
                          <div className="chip">
                            Updated {formatDate(githubDetail?.date_modified || githubOpen.date_modified)}
                          </div>
                        ) : null}
                      </div>
                      <div className="p projectHeroDesc">
                        {githubDetail?.summary || githubOpen.description || "GitHub repository"}
                      </div>
                      <div className="projectChipRow">
                        <span className="chip">Owner: {githubDetail?.owner || githubOpen.author || "Unknown"}</span>
                        <span className="chip">Stars: {formatCompact(githubDetail?.stars ?? githubOpen.downloads)}</span>
                        {typeof githubDetail?.forks === "number" ? (
                          <span className="chip">Forks: {formatCompact(githubDetail.forks)}</span>
                        ) : null}
                        {githubInstallStateChipLabel(githubInstallState(githubOpen, githubDetail)) ? (
                          <span
                            className={githubStatusChipClass("installability", githubInstallState(githubOpen, githubDetail))}
                          >
                            {githubInstallStateChipLabel(githubInstallState(githubOpen, githubDetail))}
                          </span>
                        ) : null}
                        {(githubDetail?.categories ?? githubOpen.categories ?? []).slice(0, 8).map((tag) => (
                          <span key={tag} className="chip">{tag}</span>
                        ))}
                      </div>
                    </div>
                  </div>
                  {githubOpen.reason ? (
                    <div className="noticeBox" style={{ marginTop: 10 }}>{githubOpen.reason}</div>
                  ) : null}
                  {githubResultInstallNote(githubOpen, githubDetail) ? (
                    <div
                      className={
                        githubResultInstallSupported(githubOpen, githubDetail)
                          ? "noticeBox"
                          : "warningBox"
                      }
                      style={{ marginTop: 10 }}
                    >
                      {githubResultInstallNote(githubOpen, githubDetail)}
                    </div>
                  ) : null}
                  {githubDetail?.warning ? (
                    <div className="warningBox" style={{ marginTop: 10 }}>{githubDetail.warning}</div>
                  ) : null}
                </div>

                <div className="projectTabSticky">
                  <SegmentedControl
                    value={githubDetailTab}
                    options={GITHUB_DETAIL_TABS}
                    onChange={(v) => setGithubDetailTab((v ?? "overview") as GithubDetailTab)}
                    variant="scroll"
                    className="projectTabBar"
                  />
                </div>

                {githubDetailTab === "overview" ? (
                  <div className="projectOverviewCols">
                    <div className="projectOverviewCol">
                      <div className="card projectSectionCard projectSectionDesc">
                        <div className="projectSectionTitle">Quick info</div>
                        <div className="projectBodyText">
                          {githubDetail?.summary || githubOpen.description || "No summary provided."}
                          <div className="muted" style={{ marginTop: 8 }}>
                            Full project documentation is in the README tab.
                          </div>
                        </div>
                      </div>
                      <div className="card projectSectionCard projectSectionLinks">
                        <div className="projectSectionTitle">Links</div>
                        <div className="projectLinks">
                          {[
                            { label: "Repository", href: githubDetail?.external_url ?? githubOpen.external_url ?? undefined },
                            { label: "Releases", href: githubDetail?.releases_url ?? undefined },
                            { label: "Issues", href: githubDetail?.issues_url ?? undefined },
                            { label: "Homepage", href: githubDetail?.homepage_url ?? undefined },
                            { label: "README", href: githubDetail?.readme_html_url ?? undefined },
                          ]
                            .filter((x) => Boolean(x.href))
                            .map((x) => (
                              <a
                                key={x.label}
                                className="projectLinkBtn"
                                href={x.href}
                                target="_blank"
                                rel="noreferrer"
                              >
                                {x.label}
                              </a>
                            ))}
                        </div>
                      </div>
                    </div>
                    <div className="projectOverviewCol">
                      <div className="card projectSectionCard projectSectionCompat">
                        <div className="projectSectionTitle">Repository stats</div>
                        <div className="projectFacetGroup">
                          <div className="projectFacetWrap">
                            <span className="chip">Stars: {formatCompact(githubDetail?.stars ?? 0)}</span>
                            <span className="chip">Forks: {formatCompact(githubDetail?.forks ?? 0)}</span>
                            <span className="chip">Watchers: {formatCompact(githubDetail?.watchers ?? 0)}</span>
                            <span className="chip">Open issues: {formatCompact(githubDetail?.open_issues ?? 0)}</span>
                            <span className="chip">Releases: {formatCompact(githubDetail?.releases?.length ?? 0)}</span>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                ) : null}

                {githubDetailTab === "releases" ? (
                  <div className="card projectSectionCard">
                    <div className="projectSectionTitle">Releases</div>
                    {!githubDetail || githubDetail.releases.length === 0 ? (
                      <div className="muted">No release list available.</div>
                    ) : (
                      <div className="projectVersionList">
                        {githubDetail.releases.slice(0, 25).map((release) => (
                          <div key={release.id} className="projectVersionRow">
                            <div className="projectVersionMain">
                              <div className="projectVersionTitle">
                                {release.name || release.tag_name}
                              </div>
                              <div className="projectVersionMeta">
                                <span>{release.tag_name}</span>
                                <span>{release.published_at ? formatDate(release.published_at) : "Unknown date"}</span>
                                {release.prerelease ? <span>Pre-release</span> : null}
                                {release.draft ? <span>Draft</span> : null}
                              </div>
                              {release.assets.length > 0 ? (
                                <div className="projectVersionFile">
                                  {release.assets.slice(0, 6).map((asset) => asset.name).join(", ")}
                                </div>
                              ) : (
                                <div className="muted">No assets</div>
                              )}
                            </div>
                            <div className="projectVersionAside">
                              <div className="chip">{release.assets.length} files</div>
                              {release.external_url ? (
                                <a className="chip" href={release.external_url} target="_blank" rel="noreferrer">
                                  Open
                                </a>
                              ) : null}
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ) : null}

                {githubDetailTab === "readme" ? (
                  <div className="card projectSectionCard projectSectionDesc">
                    <div className="projectSectionTitle">README</div>
                    {githubDetail?.readme_markdown ? (
                      <GithubReadmeMarkdown
                        className="projectBodyText projectMarkdown"
                        text={githubDetail.readme_markdown}
                        readmeHtmlUrl={githubDetail.readme_html_url}
                        readmeSourceUrl={githubDetail.readme_source_url}
                      />
                    ) : (
                      <div className="muted">
                        README is not available from GitHub API right now.
                        {githubDetail?.readme_html_url ? (
                          <>
                            {" "}
                            <a href={githubDetail.readme_html_url} target="_blank" rel="noreferrer">
                              Open README on GitHub
                            </a>
                            .
                          </>
                        ) : null}
                      </div>
                    )}
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>
          <div className="footerBar">
            <button className="btn" onClick={closeProjectOverlays}>Close</button>
            <button
              className="btn"
              onClick={() => {
                const direct = githubDetail?.external_url?.trim() ?? githubOpen?.external_url?.trim() ?? "";
                if (direct) {
                  void openExternalLink(direct);
                  return;
                }
                const parsed = parseGithubProjectId(githubDetail?.project_id ?? githubOpen?.project_id ?? "");
                if (parsed) {
                  void openExternalLink(`https://github.com/${parsed}`);
                }
              }}
            >
              Open on GitHub
            </button>
            <button
              className="btn primary installAction"
              disabled={
                !githubOpen ||
                githubOpen.content_type === "modpacks" ||
                !githubResultInstallSupported(githubOpen, githubDetail)
              }
              onClick={() => {
                if (!githubOpen) return;
                openInstall({
                  source: "github",
                  projectId: githubDetail?.project_id ?? githubOpen.project_id,
                  title: githubDetail?.title ?? githubOpen.title,
                  contentType:
                    (githubOpen.content_type as DiscoverContentType) === "modpacks"
                      ? "modpacks"
                      : ((githubOpen.content_type as DiscoverContentType) ?? "mods"),
                  iconUrl: githubDetail?.icon_url ?? githubOpen.icon_url ?? null,
                  description: githubDetail?.summary ?? githubOpen.description ?? null,
                  installSupported: githubResultInstallSupported(githubOpen, githubDetail),
                  installNote: githubResultInstallNote(githubOpen, githubDetail),
                });
              }}
            >
              <Icon name="download" /> Install to instance
            </button>
          </div>
        </Modal>
      ) : null}

      {githubAttachTarget ? (
        <Modal
          title={`Save GitHub Repo Hint`}
          onClose={() => {
            if (githubAttachBusyVersion === installedEntryUiKey(githubAttachTarget.mod)) return;
            setGithubAttachTarget(null);
            setGithubAttachErr(null);
          }}
        >
          <div className="modalBody">
            <div className="card" style={{ padding: 14, borderRadius: 18, display: "grid", gap: 10 }}>
                <div className="muted">
                  Save a GitHub repo hint for <strong>{githubAttachTarget.mod.name}</strong> in{" "}
                  <strong>{githubAttachTarget.instanceName}</strong>.
                </div>
              <label style={{ display: "grid", gap: 6 }}>
                <span className="muted">Repository</span>
                <input
                  className="input"
                  placeholder="owner/repo or https://github.com/owner/repo"
                  value={githubAttachInput}
                  onChange={(event) => {
                    setGithubAttachInput(event.target.value);
                    if (githubAttachErr) setGithubAttachErr(null);
                  }}
                  onKeyDown={(event) => {
                    if (event.key !== "Enter") return;
                    event.preventDefault();
                    void submitAttachInstalledModGithubRepo();
                  }}
                  autoFocus
                />
              </label>
              {githubAttachErr ? (
                <div className="errorBox">{githubAttachErr}</div>
              ) : (
                <div className="muted">
                  If GitHub API is rate-limited, the launcher saves the repo hint and leaves GitHub provider activation pending verification.
                </div>
              )}
            </div>
          </div>
          <div className="footerBar">
            <button
              className="btn"
              onClick={() => {
                setGithubAttachTarget(null);
                setGithubAttachErr(null);
              }}
              disabled={githubAttachBusyVersion === installedEntryUiKey(githubAttachTarget.mod)}
            >
              Cancel
            </button>
            <button
              className="btn primary"
              onClick={() => void submitAttachInstalledModGithubRepo()}
              disabled={githubAttachBusyVersion === installedEntryUiKey(githubAttachTarget.mod)}
            >
              {githubAttachBusyVersion === installedEntryUiKey(githubAttachTarget.mod) ? "Saving…" : "Save Hint"}
            </button>
          </div>
        </Modal>
      ) : null}

      {deleteTarget ? (
        <div className="modalOverlay dangerVignette noBlur" onMouseDown={() => (busy === "delete" ? null : setDeleteTarget(null))}>
          <div className="deleteConfirmDialog" onMouseDown={(e) => e.stopPropagation()}>
            <div className="deleteConfirmHeader">
              <div className="deleteConfirmTitle">Are you sure you want to delete this instance?</div>
              <button
                className="iconBtn"
                onClick={() => setDeleteTarget(null)}
                disabled={busy === "delete"}
                aria-label="Close delete dialog"
              >
                <Icon name="x" size={20} />
              </button>
            </div>

            <div className="deleteConfirmBody">
              If you proceed, all data for your instance will be removed. You will not be able to recover it.
            </div>

            <div className="deleteConfirmActions">
              <button className="btn dangerSolid" onClick={onDelete} disabled={busy === "delete"}>
                <Icon name="trash" size={17} /> {busy === "delete" ? "Deleting…" : "Delete"}
              </button>
              <button className="btn" onClick={() => setDeleteTarget(null)} disabled={busy === "delete"}>
                <Icon name="x" size={17} /> Cancel
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {msLoginSessionId && msCodePromptVisible && msCodePrompt ? (
        <div className="modalOverlay msCodeOverlay noBlur" onMouseDown={() => setMsCodePromptVisible(false)}>
          <div className="msCodeDialog" onMouseDown={(e) => e.stopPropagation()}>
            <div className="msCodeTitle">Enter this code to continue sign-in</div>
            <div className="msCodeSub">Open Microsoft device login and paste this code.</div>
            <div className="msCodeValue" aria-live="polite">{msCodePrompt.code}</div>
            <div className="msCodeActions">
              <button className="btn primary" onClick={copyMicrosoftCode}>
                {msCodeCopied ? "Copied" : "Copy code"}
              </button>
              <button
                className="btn"
                onClick={() => void openExternalLink(msCodePrompt.verificationUrl)}
              >
                Open login page
              </button>
              <button className="btn" onClick={() => setMsCodePromptVisible(false)}>
                Hide
              </button>
            </div>
            <div className="muted">
              Waiting for Microsoft confirmation…
            </div>
          </div>
        </div>
      ) : null}
      <GlobalTooltipLayer />
    </div>
  );
}
