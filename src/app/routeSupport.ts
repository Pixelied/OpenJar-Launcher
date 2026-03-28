import { open as shellOpen } from "@tauri-apps/api/shell";

import type {
  DiscoverContentType,
  DiscoverSearchHit,
  DiscoverSource,
  GithubInstallState,
  JavaRuntimeCandidate,
  LaunchPermissionChecklistItem,
  ProviderCandidate,
} from "../types";
import type { ModrinthIndex } from "../modrinth";

export type AccentPreset = "neutral" | "blue" | "emerald" | "amber" | "rose" | "violet" | "teal";
export type AccentStrength = "subtle" | "normal" | "vivid" | "max";
export type MotionPreset = "calm" | "standard" | "expressive";
export type DensityPreset = "comfortable" | "compact";
export type DiscoverProviderSource = Exclude<DiscoverSource, "all">;
export type SchedulerCadence =
  | "off"
  | "hourly"
  | "every_3_hours"
  | "every_6_hours"
  | "every_12_hours"
  | "daily"
  | "weekly";
export type SchedulerAutoApplyMode = "never" | "opt_in_instances" | "all_instances";
export type SchedulerApplyScope = "scheduled_only" | "scheduled_and_manual";
export type UpdatableContentType = "mods" | "resourcepacks" | "datapacks" | "shaderpacks";
export type SettingsMode = "basic" | "advanced";
export type DiscoverAddContext = {
  modpackId: string;
  modpackName: string;
  layerId?: string | null;
  layerName?: string | null;
};
export type DiscoverAddTrayItem = {
  id: string;
  title: string;
  projectId: string;
  source: DiscoverSource;
  contentType: DiscoverContentType;
  modpackName: string;
  layerName: string;
  addedAt: string;
};
export type LibraryGroupBy = "none" | "loader" | "version";
export type AccountSkinOption = {
  id: string;
  label: string;
  skinUrl?: string | null;
  capeUrl?: string | null;
  source: "saved" | "default" | "cape";
  variant?: string | null;
  metadata?: string | null;
};
export type AccountSkinThumbSet = {
  body3d?: string | null;
  head2d?: string | null;
};
export type Cat = { id: string; label: string };
export type CatGroup = { group: string; items: Cat[] };

export function launchStageBadgeLabel(status?: string | null, message?: string | null) {
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

export function relativeTimeFromMs(atMs: number): string {
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

export function javaRuntimeDisplayLabel(runtime: JavaRuntimeCandidate): string {
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

export function providerSourceLabel(value?: string | null): string {
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

export function githubInstallStateChipLabel(value?: string | null): string | null {
  const normalized = normalizeGithubInstallState(value);
  if (normalized === "unsupported") return "no compatible release";
  return null;
}

export function githubStatusChipClass(kind: "verification" | "installability", value?: string | null) {
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

export function githubInstallSummary(
  hit: DiscoverSearchHit | null | undefined,
  detail?: { install_summary?: string | null } | null
): string | null {
  const detailSummary = String(detail?.install_summary ?? "").trim();
  if (detailSummary) return detailSummary;
  const hitSummary = String(hit?.install_summary ?? "").trim();
  return hitSummary || null;
}

function githubInstallState(
  hit: DiscoverSearchHit | null | undefined,
  detail?: { install_state?: string | null } | null
): GithubInstallState {
  return normalizeGithubInstallState(detail?.install_state ?? hit?.install_state);
}

export function githubResultInstallSupported(
  hit: DiscoverSearchHit | null | undefined,
  detail?: { install_state?: string | null } | null
): boolean {
  return githubInstallState(hit, detail) !== "unsupported";
}

export function githubResultInstallNote(
  hit: DiscoverSearchHit | null | undefined,
  detail?: { install_summary?: string | null; install_state?: string | null } | null
): string | null {
  return githubInstallSummary(hit, detail);
}

export function normalizeDiscoverSource(value?: string | null): DiscoverSource {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (normalized === "modrinth" || normalized === "curseforge" || normalized === "github") {
    return normalized;
  }
  return "modrinth";
}

export function updateCadenceLabel(cadence: SchedulerCadence): string {
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

export function updateAutoApplyModeLabel(mode: SchedulerAutoApplyMode): string {
  switch (mode) {
    case "opt_in_instances":
      return "Only chosen instances";
    case "all_instances":
      return "All instances";
    default:
      return "Do not auto-install";
  }
}

export function updateApplyScopeLabel(scope: SchedulerApplyScope): string {
  return scope === "scheduled_and_manual" ? "Scheduled runs and Run check now" : "Scheduled runs only";
}

export function permissionStatusLabel(status: string) {
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

export function permissionStatusChipClass(status: string) {
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

export function normalizeDiscoverProviderSources(values: readonly string[]): DiscoverProviderSource[] {
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

export async function openExternalLink(url: string) {
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

export const DISCOVER_SORT_OPTIONS: { value: ModrinthIndex; label: string }[] = [
  { value: "relevance", label: "Relevance" },
  { value: "downloads", label: "Downloads" },
  { value: "follows", label: "Followers" },
  { value: "newest", label: "Newest" },
  { value: "updated", label: "Recently updated" },
];

export const DISCOVER_VIEW_OPTIONS: { value: string; label: string }[] = [
  { value: "10", label: "10" },
  { value: "20", label: "20" },
  { value: "30", label: "30" },
  { value: "50", label: "50" },
];

export const DISCOVER_PROVIDER_SOURCES: DiscoverProviderSource[] = ["modrinth", "curseforge", "github"];

export const DISCOVER_SOURCE_OPTIONS: { value: DiscoverProviderSource; label: string }[] = [
  { value: "modrinth", label: "Modrinth" },
  { value: "curseforge", label: "CurseForge" },
  { value: "github", label: "GitHub" },
];

export const DISCOVER_SOURCE_GROUPS: CatGroup[] = [
  {
    group: "Sources",
    items: DISCOVER_SOURCE_OPTIONS.map((option) => ({ id: option.value, label: option.label })),
  },
];

export const DISCOVER_CONTENT_OPTIONS: { value: DiscoverContentType; label: string }[] = [
  { value: "mods", label: "Mods" },
  { value: "shaderpacks", label: "Shaderpacks" },
  { value: "resourcepacks", label: "Resourcepacks" },
  { value: "datapacks", label: "Datapacks" },
  { value: "modpacks", label: "Modpacks" },
];

export const DISCOVER_LOADER_GROUPS: CatGroup[] = [
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

export const ACCENT_OPTIONS: { value: AccentPreset; label: string }[] = [
  { value: "neutral", label: "Neutral" },
  { value: "blue", label: "Blue" },
  { value: "emerald", label: "Emerald" },
  { value: "amber", label: "Amber" },
  { value: "rose", label: "Rose" },
  { value: "violet", label: "Violet" },
  { value: "teal", label: "Teal" },
];

export const ACCENT_STRENGTH_OPTIONS: { value: AccentStrength; label: string }[] = [
  { value: "subtle", label: "Subtle" },
  { value: "normal", label: "Normal" },
  { value: "vivid", label: "Vivid" },
  { value: "max", label: "Max" },
];

export const MOTION_OPTIONS: { value: MotionPreset; label: string }[] = [
  { value: "calm", label: "Calm" },
  { value: "standard", label: "Standard" },
  { value: "expressive", label: "Expressive" },
];

export const MOTION_PROFILE_DETAILS: Record<
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

export const DENSITY_OPTIONS: { value: DensityPreset; label: string }[] = [
  { value: "comfortable", label: "Comfortable" },
  { value: "compact", label: "Compact" },
];

export const UPDATE_CADENCE_OPTIONS: { value: SchedulerCadence; label: string }[] = [
  { value: "off", label: "Disabled" },
  { value: "hourly", label: "Every hour" },
  { value: "every_3_hours", label: "Every 3 hours" },
  { value: "every_6_hours", label: "Every 6 hours" },
  { value: "every_12_hours", label: "Every 12 hours" },
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
];

export const UPDATE_AUTO_APPLY_MODE_OPTIONS: { value: SchedulerAutoApplyMode; label: string }[] = [
  { value: "never", label: "Do not install automatically" },
  { value: "opt_in_instances", label: "Only instances you marked for auto-install" },
  { value: "all_instances", label: "Install updates for every instance" },
];

export const UPDATE_APPLY_SCOPE_OPTIONS: { value: SchedulerApplyScope; label: string }[] = [
  { value: "scheduled_only", label: "Only during scheduled runs" },
  { value: "scheduled_and_manual", label: "During scheduled runs and Run check now" },
];

export const UPDATE_CONTENT_TYPE_OPTIONS: { value: UpdatableContentType; label: string }[] = [
  { value: "mods", label: "Mods" },
  { value: "resourcepacks", label: "Resourcepacks" },
  { value: "datapacks", label: "Datapacks" },
  { value: "shaderpacks", label: "Shaders" },
];

export const ALL_UPDATABLE_CONTENT_TYPES: UpdatableContentType[] = UPDATE_CONTENT_TYPE_OPTIONS.map(
  (item) => item.value
);

export const UPDATE_CONTENT_TYPE_GROUPS: { group: string; items: { id: string; label: string }[] }[] = [
  {
    group: "Update content types",
    items: UPDATE_CONTENT_TYPE_OPTIONS.map((item) => ({
      id: item.value,
      label: item.label,
    })),
  },
];

export const LOG_MAX_LINES_OPTIONS: { value: string; label: string }[] = [
  { value: "400", label: "400" },
  { value: "1200", label: "1,200" },
  { value: "2500", label: "2,500" },
  { value: "5000", label: "5,000" },
  { value: "8000", label: "8,000" },
  { value: "12000", label: "12,000" },
];

export const MOD_CATEGORY_GROUPS: CatGroup[] = [
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
  {
    group: "Client & UI",
    items: [
      { id: "library", label: "Library / API" },
      { id: "library-api", label: "Library API" },
      { id: "map-information", label: "Map information" },
      { id: "utility-qol", label: "Utility / QoL" },
      { id: "cosmetic", label: "Cosmetic" },
    ],
  },
];
