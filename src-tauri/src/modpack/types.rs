use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryKey {
    pub provider: String,
    pub project_id: String,
    #[serde(default = "default_content_type")]
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModEntry {
    pub provider: String,
    pub project_id: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default = "default_content_type")]
    pub content_type: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub pin: Option<String>,
    #[serde(default = "default_channel_policy")]
    pub channel_policy: String,
    #[serde(default = "default_inherit")]
    pub fallback_policy: String,
    #[serde(default)]
    pub replacement_group: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub disabled_by_default: bool,
    #[serde(default)]
    pub optional: bool,
    #[serde(default = "default_target_scope")]
    pub target_scope: String,
    #[serde(default)]
    pub target_worlds: Vec<String>,
    #[serde(default)]
    pub local_file_name: Option<String>,
    #[serde(default)]
    pub local_file_path: Option<String>,
    #[serde(default)]
    pub local_sha512: Option<String>,
    #[serde(default)]
    pub local_fingerprints: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntriesDelta {
    #[serde(default)]
    pub add: Vec<ModEntry>,
    #[serde(default)]
    pub remove: Vec<EntryKey>,
    #[serde(rename = "override", default)]
    pub override_entries: Vec<ModEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerSource {
    pub kind: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub spec_id: Option<String>,
    #[serde(default)]
    pub imported_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub source: Option<LayerSource>,
    #[serde(default)]
    pub is_frozen: bool,
    pub entries_delta: EntriesDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub optional_entry_states: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionSettings {
    #[serde(default = "default_fallback_mode")]
    pub global_fallback_mode: String,
    #[serde(default = "default_channel_policy")]
    pub channel_allowance: String,
    #[serde(default)]
    pub allow_cross_minor: bool,
    #[serde(default)]
    pub allow_cross_major: bool,
    #[serde(default = "default_true")]
    pub prefer_stable: bool,
    #[serde(default = "default_max_fallback_distance")]
    pub max_fallback_distance: u32,
    #[serde(default = "default_dependency_mode")]
    pub dependency_mode: String,
    #[serde(default)]
    pub partial_apply_unsafe: bool,
}

impl Default for ResolutionSettings {
    fn default() -> Self {
        Self {
            global_fallback_mode: default_fallback_mode(),
            channel_allowance: default_channel_policy(),
            allow_cross_minor: true,
            allow_cross_major: false,
            prefer_stable: true,
            max_fallback_distance: default_max_fallback_distance(),
            dependency_mode: default_dependency_mode(),
            partial_apply_unsafe: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackSpec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub settings: ResolutionSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetInstanceSnapshot {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: String,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub java_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedMod {
    pub source: String,
    pub content_type: String,
    pub project_id: String,
    pub name: String,
    pub version_id: String,
    pub version_number: String,
    pub filename: String,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub curseforge_file_id: Option<i64>,
    #[serde(default)]
    pub hashes: HashMap<String, String>,
    pub enabled: bool,
    #[serde(default)]
    pub target_worlds: Vec<String>,
    pub rationale_text: String,
    #[serde(default)]
    pub added_by_dependency: bool,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedMod {
    pub source: String,
    pub content_type: String,
    pub project_id: String,
    pub name: String,
    pub reason_code: String,
    pub reason_text: String,
    pub actionable_hint: String,
    pub constraints_snapshot: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionConflict {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionPlan {
    pub id: String,
    pub modpack_id: String,
    pub modpack_updated_at_stamp: String,
    pub target: TargetInstanceSnapshot,
    #[serde(default)]
    pub profile_id: Option<String>,
    pub settings: ResolutionSettings,
    #[serde(default)]
    pub resolved_mods: Vec<ResolvedMod>,
    #[serde(default)]
    pub failed_mods: Vec<FailedMod>,
    #[serde(default)]
    pub conflicts: Vec<ResolutionConflict>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub confidence_score: f64,
    pub confidence_label: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockSnapshotEntry {
    pub source: String,
    pub content_type: String,
    pub project_id: String,
    pub name: String,
    pub version_id: String,
    pub version_number: String,
    pub enabled: bool,
    #[serde(default)]
    pub target_worlds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockSnapshot {
    pub id: String,
    pub instance_id: String,
    pub plan_id: String,
    pub created_at: String,
    #[serde(default)]
    pub entries: Vec<LockSnapshotEntry>,
    #[serde(default)]
    pub instance_snapshot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceModpackLinkState {
    pub instance_id: String,
    pub mode: String,
    pub modpack_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub last_plan_id: Option<String>,
    #[serde(default)]
    pub last_lock_snapshot_id: Option<String>,
    #[serde(default)]
    pub last_applied_at: Option<String>,
    #[serde(default)]
    pub last_confidence_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftItem {
    pub source: String,
    pub content_type: String,
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub expected_version: Option<String>,
    #[serde(default)]
    pub current_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    pub instance_id: String,
    pub status: String,
    #[serde(default)]
    pub added: Vec<DriftItem>,
    #[serde(default)]
    pub removed: Vec<DriftItem>,
    #[serde(default)]
    pub version_changed: Vec<DriftItem>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDiffResult {
    #[serde(default)]
    pub layer_id: Option<String>,
    #[serde(default)]
    pub added: Vec<ModEntry>,
    #[serde(default)]
    pub removed: Vec<EntryKey>,
    #[serde(default)]
    pub overridden: Vec<ModEntry>,
    #[serde(default)]
    pub conflicts: Vec<ResolutionConflict>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSkippedItem {
    pub id: String,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub migrated_count: usize,
    pub skipped_count: usize,
    #[serde(default)]
    pub skipped_items: Vec<MigrationSkippedItem>,
    #[serde(default)]
    pub created_spec_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackApplyResult {
    pub message: String,
    pub applied_entries: usize,
    pub skipped_entries: usize,
    pub failed_entries: usize,
    #[serde(default)]
    pub snapshot_id: Option<String>,
    pub plan_id: String,
    #[serde(default)]
    pub lock_snapshot_id: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceModpackStatus {
    pub instance_id: String,
    #[serde(default)]
    pub link: Option<InstanceModpackLinkState>,
    #[serde(default)]
    pub last_plan: Option<ResolutionPlan>,
    #[serde(default)]
    pub drift: Option<DriftReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackStoreV1 {
    pub version: u32,
    #[serde(default)]
    pub specs: Vec<ModpackSpec>,
    #[serde(default)]
    pub plans: Vec<ResolutionPlan>,
    #[serde(default)]
    pub lock_snapshots: Vec<LockSnapshot>,
    #[serde(default)]
    pub instance_links: Vec<InstanceModpackLinkState>,
}

impl Default for ModpackStoreV1 {
    fn default() -> Self {
        Self {
            version: 1,
            specs: vec![],
            plans: vec![],
            lock_snapshots: vec![],
            instance_links: vec![],
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModpackIdArgs {
    #[serde(alias = "modpackId", alias = "id")]
    pub modpack_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertModpackSpecArgs {
    pub spec: ModpackSpec,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DuplicateModpackSpecArgs {
    #[serde(alias = "modpackId", alias = "id")]
    pub modpack_id: String,
    #[serde(default)]
    pub new_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteModpackSpecArgs {
    #[serde(alias = "modpackId", alias = "id")]
    pub modpack_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportModpackSpecJsonArgs {
    #[serde(alias = "inputPath")]
    pub input_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportModpackSpecJsonArgs {
    #[serde(alias = "modpackId")]
    pub modpack_id: String,
    #[serde(alias = "outputPath")]
    pub output_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportLayerFromProviderArgs {
    #[serde(alias = "modpackId")]
    pub modpack_id: String,
    #[serde(alias = "layerName")]
    pub layer_name: String,
    pub source: String,
    #[serde(alias = "projectId")]
    pub project_id: String,
    #[serde(alias = "projectTitle", default)]
    pub project_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportLayerFromSpecArgs {
    #[serde(alias = "targetModpackId")]
    pub target_modpack_id: String,
    #[serde(alias = "sourceModpackId")]
    pub source_modpack_id: String,
    #[serde(alias = "layerName")]
    pub layer_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayerRefArgs {
    #[serde(alias = "modpackId")]
    pub modpack_id: String,
    #[serde(alias = "layerId")]
    pub layer_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveModpackArgs {
    #[serde(alias = "modpackId")]
    pub modpack_id: String,
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    #[serde(alias = "profileId", default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub settings: Option<ResolutionSettings>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyModpackPlanArgs {
    #[serde(alias = "planId")]
    pub plan_id: String,
    #[serde(alias = "linkMode", default)]
    pub link_mode: Option<String>,
    #[serde(alias = "partialApplyUnsafe", default)]
    pub partial_apply_unsafe: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstanceArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreviewUpdateFromInstanceArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    #[serde(alias = "modpackId")]
    pub modpack_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyUpdateFromInstanceArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    #[serde(alias = "modpackId")]
    pub modpack_id: String,
    #[serde(alias = "layerName", default)]
    pub layer_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MigrateLegacyCreatorPresetsArgs {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeedDevModpackDataArgs {
    #[serde(alias = "instanceName", default)]
    pub instance_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpecIoResult {
    pub path: String,
    pub items: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeedDevResult {
    pub created_spec_id: String,
    pub created_instance_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportLocalJarsToLayerArgs {
    #[serde(alias = "modpackId")]
    pub modpack_id: String,
    #[serde(alias = "layerId")]
    pub layer_id: String,
    #[serde(alias = "filePaths")]
    pub file_paths: Vec<String>,
    #[serde(alias = "autoIdentify", default)]
    pub auto_identify: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveLocalModpackEntriesArgs {
    #[serde(alias = "modpackId")]
    pub modpack_id: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(alias = "layerId", default)]
    pub layer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModpackLocalResolverMatch {
    pub key: String,
    pub from_source: String,
    pub to_source: String,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub confidence: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModpackLocalResolverResult {
    pub spec: ModpackSpec,
    pub scanned_entries: usize,
    pub resolved_entries: usize,
    pub remaining_local_entries: usize,
    #[serde(default)]
    pub matches: Vec<ModpackLocalResolverMatch>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModpackImportLocalJarItemResult {
    pub index: usize,
    pub path: String,
    pub file_name: String,
    pub status: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub dedupe_basis: Option<String>,
    #[serde(default)]
    pub duplicate_of: Option<String>,
    #[serde(default)]
    pub source_hint: Option<String>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModpackImportLocalJarProgressEvent {
    pub modpack_id: String,
    pub layer_id: String,
    pub index: usize,
    pub total: usize,
    pub path: String,
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModpackImportLocalJarsResult {
    pub spec: ModpackSpec,
    pub added_entries: usize,
    pub updated_entries: usize,
    pub resolved_entries: usize,
    pub remaining_local_entries: usize,
    #[serde(default)]
    pub items: Vec<ModpackImportLocalJarItemResult>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

pub fn default_true() -> bool {
    true
}

pub fn default_content_type() -> String {
    "mods".to_string()
}

pub fn default_target_scope() -> String {
    "instance".to_string()
}

pub fn default_channel_policy() -> String {
    "stable".to_string()
}

pub fn default_inherit() -> String {
    "inherit".to_string()
}

pub fn default_fallback_mode() -> String {
    "smart".to_string()
}

pub fn default_dependency_mode() -> String {
    "detect_only".to_string()
}

pub fn default_max_fallback_distance() -> u32 {
    3
}
