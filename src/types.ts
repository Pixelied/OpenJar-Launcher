export type Loader = "fabric" | "forge" | "quilt" | "neoforge" | "vanilla";

export type Instance = {
  id: string;
  name: string;
  mc_version: string;
  loader: Loader;
  created_at: string;
  icon_path?: string | null;
  settings?: InstanceSettings;
};

export type InstanceSettings = {
  keep_launcher_open_while_playing: boolean;
  close_launcher_on_game_exit: boolean;
  notes: string;
  auto_update_installed_content: boolean;
  prefer_release_builds: boolean;
  java_path: string;
  memory_mb: number;
  jvm_args: string;
  graphics_preset: "Performance" | "Balanced" | "Quality" | string;
  enable_shaders: boolean;
  force_vsync: boolean;
  world_backup_interval_minutes: number;
  world_backup_retention_count: number;
  snapshot_retention_count: number;
  snapshot_max_age_days: number;
};

export type InstalledMod = {
  source: "modrinth" | string;
  project_id: string;
  version_id: string;
  name: string;
  version_number: string;
  filename: string;
  content_type?: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | "modpacks" | string;
  target_scope?: "instance" | "world" | string;
  target_worlds?: string[];
  pinned_version?: string | null;
  enabled: boolean;
  file_exists: boolean;
  hashes?: Record<string, string>;
};

export type InstallProgressEvent = {
  instance_id: string;
  project_id: string;
  stage: "resolving" | "downloading" | "completed" | "error";
  downloaded: number;
  total?: number | null;
  percent?: number | null;
  message?: string | null;
};

export type InstallPlanPreview = {
  total_mods: number;
  dependency_mods: number;
  will_install_mods: number;
};

export type ModUpdateInfo = {
  project_id: string;
  name: string;
  current_version_id: string;
  current_version_number: string;
  latest_version_id: string;
  latest_version_number: string;
};

export type ModUpdateCheckResult = {
  checked_mods: number;
  update_count: number;
  updates: ModUpdateInfo[];
};

export type UpdateAllResult = {
  checked_mods: number;
  updated_mods: number;
};

export type ContentUpdateInfo = {
  source: "modrinth" | "curseforge" | string;
  content_type: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | string;
  project_id: string;
  name: string;
  current_version_id: string;
  current_version_number: string;
  latest_version_id: string;
  latest_version_number: string;
  enabled: boolean;
  target_worlds: string[];
  latest_file_name?: string;
  latest_download_url?: string;
  latest_hashes?: Record<string, string>;
  required_dependencies?: string[];
};

export type ContentUpdateCheckResult = {
  checked_entries: number;
  update_count: number;
  updates: ContentUpdateInfo[];
  warnings: string[];
};

export type UpdateAllContentResult = {
  checked_entries: number;
  updated_entries: number;
  warnings: string[];
  by_source: Record<string, number>;
  by_content_type: Record<string, number>;
};

export type LaunchResult = {
  method: "prism" | "native" | string;
  launch_id?: string | null;
  pid?: number | null;
  prism_instance_id?: string | null;
  prism_root?: string | null;
  message: string;
};

export type LaunchMethod = "prism" | "native";

export type LaunchCompatibilityItem = {
  code: string;
  title: string;
  message: string;
  severity: "blocker" | "warning" | "info" | string;
  blocking: boolean;
};

export type LaunchCompatibilityReport = {
  instance_id: string;
  status: "ok" | "warning" | "blocked" | string;
  checked_at: string;
  blocking_count: number;
  warning_count: number;
  unresolved_local_entries: number;
  items: LaunchCompatibilityItem[];
};

export type LocalResolverMatch = {
  key: string;
  from_source: string;
  to_source: string;
  project_id: string;
  version_id: string;
  name: string;
  version_number: string;
  confidence: "deterministic" | "high" | string;
  reason: string;
};

export type LocalResolverResult = {
  instance_id: string;
  scanned_entries: number;
  resolved_entries: number;
  remaining_local_entries: number;
  matches: LocalResolverMatch[];
  warnings: string[];
};

export type LaunchFixAction = {
  id: string;
  kind: "toggle_mod" | "install_dependency" | "open_config" | "rerun_preflight" | string;
  title: string;
  detail: string;
  selected: boolean;
  payload?: Record<string, unknown>;
};

export type LaunchFixPlan = {
  instance_id: string;
  generated_at: string;
  source: "log_analysis" | string;
  causes: string[];
  actions: LaunchFixAction[];
};

export type LaunchFixApplyResult = {
  applied: number;
  failed: number;
  skipped: number;
  messages: string[];
};

export type InstanceHealthScore = {
  score: number;
  grade: "A" | "B" | "C" | "D" | "F";
  reasons: string[];
};

export type AutoProfileRecommendation = {
  memory_mb: number;
  jvm_args: string;
  graphics_preset: "Performance" | "Balanced" | "Quality" | string;
  confidence: "high" | "medium" | "low" | string;
  reasons: string[];
};

export type SupportPerfAction = {
  id: string;
  name: string;
  detail?: string | null;
  status: "ok" | "error" | string;
  duration_ms: number;
  finished_at: number;
};

export type SupportBundleResult = {
  output_path: string;
  files_count: number;
  redactions_applied: number;
  message: string;
};

export type CreatorConflictSuggestion = {
  id: string;
  conflict_code: string;
  title: string;
  detail: string;
  patch_preview: string;
  risk: "low" | "medium" | "high" | string;
};

export type UpdateCheckCadence =
  | "off"
  | "hourly"
  | "every_3_hours"
  | "every_6_hours"
  | "every_12_hours"
  | "daily"
  | "weekly"
  | string;

export type UpdateAutoApplyMode = "never" | "opt_in_instances" | "all_instances" | string;

export type UpdateApplyScope = "scheduled_only" | "scheduled_and_manual" | string;

export type LauncherSettings = {
  default_launch_method: LaunchMethod;
  java_path: string;
  oauth_client_id: string;
  update_check_cadence: UpdateCheckCadence;
  update_auto_apply_mode?: UpdateAutoApplyMode;
  update_apply_scope?: UpdateApplyScope;
  selected_account_id?: string | null;
  auto_identify_local_jars?: boolean;
};

export type LauncherAccount = {
  id: string;
  username: string;
  added_at: string;
};

export type AccountCosmeticSummary = {
  id: string;
  state: string;
  url: string;
  alias?: string | null;
  variant?: string | null;
};

export type AccountDiagnostics = {
  status: "connected" | "not_connected" | "error" | string;
  last_refreshed_at: string;
  selected_account_id?: string | null;
  account?: LauncherAccount | null;
  minecraft_uuid?: string | null;
  minecraft_username?: string | null;
  entitlements_ok: boolean;
  token_exchange_status: string;
  skin_url?: string | null;
  cape_count: number;
  skins: AccountCosmeticSummary[];
  capes: AccountCosmeticSummary[];
  last_error?: string | null;
  client_id_source: string;
};

export type BeginMicrosoftLoginResult = {
  session_id: string;
  auth_url: string;
  user_code?: string | null;
  verification_uri?: string | null;
};

export type JavaRuntimeCandidate = {
  path: string;
  major: number;
  version_line: string;
};

export type CurseforgeApiStatus = {
  configured: boolean;
  env_var?: string | null;
  key_hint?: string | null;
  validated: boolean;
  message: string;
};

export type MicrosoftLoginState = {
  status: "pending" | "success" | "error" | string;
  message?: string | null;
  account?: LauncherAccount | null;
};

export type RunningInstance = {
  launch_id: string;
  instance_id: string;
  instance_name: string;
  method: LaunchMethod | string;
  pid: number;
  started_at: string;
  log_path?: string | null;
};

export type InstanceLogSourceApi = "live" | "latest_launch" | "latest_crash";

export type ReadInstanceLogsLine = {
  raw: string;
  line_no?: number;
  timestamp?: string | null;
  severity?: "error" | "warn" | "info" | "debug" | "trace" | string | null;
  source: InstanceLogSourceApi | string;
};

export type ReadInstanceLogsResult = {
  source: InstanceLogSourceApi | string;
  path: string;
  available: boolean;
  total_lines: number;
  returned_lines: number;
  truncated: boolean;
  start_line_no?: number | null;
  end_line_no?: number | null;
  next_before_line?: number | null;
  lines: ReadInstanceLogsLine[];
  updated_at: number;
  message?: string | null;
};

export type ExportModsResult = {
  output_path: string;
  files_count: number;
};

export type OpenInstancePathResult = {
  target:
    | "instance"
    | "mods"
    | "resourcepacks"
    | "shaderpacks"
    | "saves"
    | "launch-log"
    | "crash-log"
    | string;
  path: string;
};

export type RevealConfigEditorFileResult = {
  opened_path: string;
  revealed_file: boolean;
  virtual_file: boolean;
  message: string;
};

export type CreateInstanceFromModpackFileResult = {
  instance: Instance;
  imported_files: number;
  warnings: string[];
};

export type LauncherImportSource = {
  id: string;
  source_kind: "vanilla" | "prism" | string;
  label: string;
  mc_version: string;
  loader: Loader;
  source_path: string;
};

export type ImportInstanceFromLauncherResult = {
  instance: Instance;
  imported_files: number;
};

export type InstanceWorld = {
  id: string;
  name: string;
  path: string;
  latest_backup_id?: string | null;
  latest_backup_at?: string | null;
  backup_count?: number;
};

export type WorldConfigFileEntry = {
  path: string;
  size_bytes: number;
  modified_at: number;
  editable: boolean;
  kind: string;
  readonly_reason?: string | null;
};

export type ReadWorldConfigFileResult = {
  path: string;
  editable: boolean;
  kind: string;
  size_bytes: number;
  modified_at: number;
  readonly_reason?: string | null;
  content?: string | null;
  preview?: string | null;
};

export type WriteWorldConfigFileResult = {
  path: string;
  size_bytes: number;
  modified_at: number;
  message: string;
};

export type InstanceConfigFileEntry = {
  path: string;
  size_bytes: number;
  modified_at: number;
  editable: boolean;
  kind: string;
  readonly_reason?: string | null;
};

export type ReadInstanceConfigFileResult = {
  path: string;
  editable: boolean;
  kind: string;
  size_bytes: number;
  modified_at: number;
  readonly_reason?: string | null;
  content?: string | null;
  preview?: string | null;
};

export type WriteInstanceConfigFileResult = {
  path: string;
  size_bytes: number;
  modified_at: number;
  message: string;
};

export type FriendLinkInvite = {
  invite_code: string;
  group_id: string;
  expires_at: string;
  bootstrap_peer_endpoint: string;
  protocol_version: number;
};

export type FriendLinkPeer = {
  peer_id: string;
  display_name: string;
  endpoint: string;
  online: boolean;
  last_seen_at?: string | null;
};

export type FriendLinkStatus = {
  instance_id: string;
  linked: boolean;
  group_id?: string | null;
  local_peer_id?: string | null;
  display_name?: string | null;
  listener_endpoint?: string | null;
  allowlist: string[];
  peers: FriendLinkPeer[];
  pending_conflicts_count: number;
  status: string;
  last_good_hash?: string | null;
};

export type FriendSyncItemKind = "lock_entry" | "config_file" | string;

export type FriendSyncConflict = {
  id: string;
  kind: FriendSyncItemKind;
  key: string;
  peer_id: string;
  mine_hash: string;
  theirs_hash: string;
  mine_preview?: string | null;
  theirs_preview?: string | null;
};

export type FriendLinkReconcileAction = {
  kind: FriendSyncItemKind;
  key: string;
  peer_id: string;
  applied: boolean;
  message: string;
};

export type FriendLinkReconcileResult = {
  status: string;
  mode: "manual" | "prelaunch" | string;
  actions_applied: number;
  actions_pending: number;
  actions: FriendLinkReconcileAction[];
  conflicts: FriendSyncConflict[];
  warnings: string[];
  blocked_reason?: string | null;
  local_state_hash: string;
  last_good_hash?: string | null;
  offline_peers: number;
};

export type ConflictResolutionPayload = {
  keep_all_mine?: boolean;
  take_all_theirs?: boolean;
  items?: Array<{
    conflict_id: string;
    resolution: "keep_mine" | "take_theirs" | "skip_for_now" | string;
  }>;
};

export type FriendLinkDebugBundleResult = {
  path: string;
};

export type SnapshotMeta = {
  id: string;
  created_at: string;
  reason: string;
};

export type RollbackResult = {
  snapshot_id: string;
  created_at: string;
  restored_files: number;
  message: string;
};

export type WorldRollbackResult = {
  world_id: string;
  backup_id: string;
  created_at: string;
  restored_files: number;
  message: string;
};

export type DiscoverSource = "modrinth" | "curseforge" | "all";
export type DiscoverContentType = "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | "modpacks";

export type DiscoverSearchHit = {
  source: "modrinth" | "curseforge" | string;
  project_id: string;
  title: string;
  description: string;
  author: string;
  downloads: number;
  follows: number;
  icon_url?: string | null;
  categories: string[];
  versions: string[];
  date_modified: string;
  content_type: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | "modpacks" | string;
  slug?: string | null;
  external_url?: string | null;
};

export type DiscoverSearchResult = {
  hits: DiscoverSearchHit[];
  offset: number;
  limit: number;
  total_hits: number;
};

export type CurseforgeProjectFileDetail = {
  file_id: string;
  display_name: string;
  file_name: string;
  file_date: string;
  game_versions: string[];
  download_url?: string | null;
};

export type CurseforgeProjectDetail = {
  source: "curseforge" | string;
  project_id: string;
  title: string;
  slug?: string | null;
  summary: string;
  description: string;
  author_names: string[];
  downloads: number;
  categories: string[];
  icon_url?: string | null;
  date_modified: string;
  external_url?: string | null;
  files: CurseforgeProjectFileDetail[];
};

export type PresetsJsonIoResult = {
  path: string;
  items: number;
};

export type CreatorPresetEntry = {
  source: "modrinth" | "curseforge" | string;
  project_id: string;
  title: string;
  content_type: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | "modpacks" | string;
  pinned_version?: string | null;
  target_scope?: "instance" | "world" | string;
  target_worlds?: string[];
  enabled?: boolean;
};

export type CreatorPresetSettings = {
  dependency_policy?: string;
  conflict_strategy?: string;
  provider_priority?: string[];
  snapshot_before_apply?: boolean;
  apply_order?: string[];
  datapack_target_policy?: string;
};

export type CreatorPreset = {
  id: string;
  name: string;
  created_at: string;
  source_instance_id: string;
  source_instance_name: string;
  entries: CreatorPresetEntry[];
  settings?: CreatorPresetSettings;
};

export type PresetApplyPreview = {
  valid: boolean;
  installable_entries: number;
  skipped_disabled_entries: number;
  missing_world_targets: string[];
  provider_warnings: string[];
  duplicate_entries: number;
};

export type PresetApplyResult = {
  message: string;
  installed_entries: number;
  skipped_entries: number;
  failed_entries: number;
  snapshot_id?: string | null;
  by_content_type: Record<string, number>;
};

export type EntryKey = {
  provider: "modrinth" | "curseforge" | string;
  project_id: string;
  content_type: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | string;
};

export type ModEntry = {
  provider: "modrinth" | "curseforge" | string;
  project_id: string;
  slug?: string | null;
  content_type: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | string;
  required: boolean;
  pin?: string | null;
  channel_policy: "stable" | "beta" | "alpha" | "inherit" | string;
  fallback_policy: "strict" | "smart" | "loose" | "inherit" | string;
  replacement_group?: string | null;
  notes?: string | null;
  disabled_by_default?: boolean;
  optional?: boolean;
  target_scope?: "instance" | "world" | string;
  target_worlds?: string[];
  local_file_name?: string | null;
  local_file_path?: string | null;
  local_sha512?: string | null;
  local_fingerprints?: number[];
};

export type EntriesDelta = {
  add: ModEntry[];
  remove: EntryKey[];
  override: ModEntry[];
};

export type LayerSource = {
  kind: string;
  source?: string | null;
  project_id?: string | null;
  spec_id?: string | null;
  imported_at?: string | null;
};

export type Layer = {
  id: string;
  name: string;
  source?: LayerSource | null;
  is_frozen?: boolean;
  entries_delta: EntriesDelta;
};

export type Profile = {
  id: string;
  name: string;
  optional_entry_states: Record<string, boolean>;
};

export type ResolutionSettings = {
  global_fallback_mode: "strict" | "smart" | "loose" | string;
  channel_allowance: "stable" | "beta" | "alpha" | string;
  allow_cross_minor: boolean;
  allow_cross_major: boolean;
  prefer_stable: boolean;
  max_fallback_distance: number;
  dependency_mode: "detect_only" | "auto_add" | string;
  partial_apply_unsafe?: boolean;
};

export type ModpackSpec = {
  id: string;
  name: string;
  description?: string | null;
  tags?: string[];
  created_at: string;
  updated_at: string;
  layers: Layer[];
  profiles: Profile[];
  settings: ResolutionSettings;
};

export type TargetInstanceSnapshot = {
  id: string;
  name: string;
  mc_version: string;
  loader: Loader | string;
  loader_version?: string | null;
  java_version?: string | null;
};

export type ResolvedMod = {
  source: "modrinth" | "curseforge" | string;
  content_type: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | string;
  project_id: string;
  name: string;
  version_id: string;
  version_number: string;
  filename: string;
  download_url?: string | null;
  curseforge_file_id?: number | null;
  hashes?: Record<string, string>;
  enabled: boolean;
  target_worlds: string[];
  rationale_text: string;
  added_by_dependency?: boolean;
  required?: boolean;
};

export type FailedMod = {
  source: "modrinth" | "curseforge" | string;
  content_type: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | string;
  project_id: string;
  name: string;
  reason_code:
    | "NoCompatibleMinecraftVersion"
    | "NoCompatibleLoader"
    | "OnlyPrereleaseAvailable"
    | "DependencyMissing"
    | "DependencyIncompatible"
    | "ProviderError"
    | "ProjectNotFound"
    | "DownloadNotAvailable"
    | "ConflictBlocked"
    | string;
  reason_text: string;
  actionable_hint: string;
  constraints_snapshot: string;
  required: boolean;
};

export type ResolutionConflict = {
  code: string;
  message: string;
  keys: string[];
};

export type ResolutionPlan = {
  id: string;
  modpack_id: string;
  modpack_updated_at_stamp: string;
  target: TargetInstanceSnapshot;
  profile_id?: string | null;
  settings: ResolutionSettings;
  resolved_mods: ResolvedMod[];
  failed_mods: FailedMod[];
  conflicts: ResolutionConflict[];
  warnings: string[];
  confidence_score: number;
  confidence_label: "High" | "Medium" | "Risky" | string;
  created_at: string;
};

export type LockSnapshotEntry = {
  source: "modrinth" | "curseforge" | string;
  content_type: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | string;
  project_id: string;
  name: string;
  version_id: string;
  version_number: string;
  enabled: boolean;
  target_worlds: string[];
};

export type LockSnapshot = {
  id: string;
  instance_id: string;
  plan_id: string;
  created_at: string;
  entries: LockSnapshotEntry[];
  instance_snapshot_id?: string | null;
};

export type InstanceModpackLinkState = {
  instance_id: string;
  mode: "linked" | "unlinked" | string;
  modpack_id: string;
  profile_id?: string | null;
  last_plan_id?: string | null;
  last_lock_snapshot_id?: string | null;
  last_applied_at?: string | null;
  last_confidence_label?: string | null;
};

export type DriftItem = {
  source: "modrinth" | "curseforge" | string;
  content_type: "mods" | "shaderpacks" | "resourcepacks" | "datapacks" | string;
  project_id: string;
  name: string;
  expected_version?: string | null;
  current_version?: string | null;
};

export type DriftReport = {
  instance_id: string;
  status: "in_sync" | "drifted" | "unlinked" | "no_snapshot" | string;
  added: DriftItem[];
  removed: DriftItem[];
  version_changed: DriftItem[];
  created_at: string;
};

export type LayerDiffResult = {
  layer_id?: string | null;
  added: ModEntry[];
  removed: EntryKey[];
  overridden: ModEntry[];
  conflicts: ResolutionConflict[];
  warnings: string[];
};

export type MigrationSkippedItem = {
  id: string;
  name: string;
  reason: string;
};

export type MigrationReport = {
  migrated_count: number;
  skipped_count: number;
  skipped_items: MigrationSkippedItem[];
  created_spec_ids: string[];
};

export type ModpackApplyResult = {
  message: string;
  applied_entries: number;
  skipped_entries: number;
  failed_entries: number;
  snapshot_id?: string | null;
  plan_id: string;
  lock_snapshot_id?: string | null;
  warnings: string[];
};

export type InstanceModpackStatus = {
  instance_id: string;
  link?: InstanceModpackLinkState | null;
  last_plan?: ResolutionPlan | null;
  drift?: DriftReport | null;
};

export type SpecIoResult = {
  path: string;
  items: number;
};

export type SeedDevResult = {
  created_spec_id: string;
  created_instance_id: string;
  message: string;
};

export type ModpackLocalResolverMatch = {
  key: string;
  from_source: string;
  to_source: string;
  project_id: string;
  version_id: string;
  name: string;
  version_number: string;
  confidence: string;
  reason: string;
};

export type ModpackLocalResolverResult = {
  spec: ModpackSpec;
  scanned_entries: number;
  resolved_entries: number;
  remaining_local_entries: number;
  matches: ModpackLocalResolverMatch[];
  warnings: string[];
};

export type ModpackImportLocalJarsResult = {
  spec: ModpackSpec;
  added_entries: number;
  updated_entries: number;
  resolved_entries: number;
  remaining_local_entries: number;
  items: ModpackImportLocalJarItemResult[];
  warnings: string[];
};

export type ModpackImportLocalJarItemResult = {
  index: number;
  path: string;
  file_name: string;
  status: "queued" | "running" | "added" | "updated_deduped" | "skipped" | "failed" | string;
  message: string;
  dedupe_basis?: "provider" | "local_sha512" | "filename" | string | null;
  duplicate_of?: string | null;
  source_hint?: string | null;
  resolved: boolean;
};

export type ModpackImportLocalJarProgressEvent = {
  modpack_id: string;
  layer_id: string;
  index: number;
  total: number;
  path: string;
  status: "queued" | "running" | "added" | "updated_deduped" | "skipped" | "failed" | string;
  message?: string | null;
};
