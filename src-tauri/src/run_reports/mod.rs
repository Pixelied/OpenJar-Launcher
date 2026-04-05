mod classifier;

use crate::{
    app_instances_dir, find_instance, friend_link, instance_dir_for_id, instance_dir_for_instance,
    latest_crash_report_path, latest_launch_log_path, list_snapshots, normalize_instance_settings,
    now_iso, now_millis, read_lockfile, required_java_major_for_mc, Lockfile,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const RUN_REPORTS_STORE_FILE: &str = "run_reports.v1.json";
const INSTANCE_EVENTS_STORE_FILE: &str = "events.v1.json";
const MAX_RUN_REPORTS: usize = 40;
const MAX_INSTANCE_EVENTS: usize = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunArtifactRef {
    pub kind: String,
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFinding {
    pub id: String,
    pub category: String,
    pub title: String,
    pub explanation: String,
    pub confidence: f64,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub likely_fix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceHistoryEvent {
    pub id: String,
    pub at: String,
    pub kind: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSuggestedAction {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub dry_run: String,
    pub reversible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRunReport {
    pub id: String,
    pub instance_id: String,
    pub created_at: String,
    pub launch_method: String,
    pub mc_version: String,
    pub loader: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_major: Option<u32>,
    pub required_java_major: u32,
    pub memory_mb: u32,
    pub jvm_args: String,
    pub exit_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<RunArtifactRef>,
    #[serde(default)]
    pub findings: Vec<RunFinding>,
    #[serde(default)]
    pub top_causes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default)]
    pub recent_changes: Vec<InstanceHistoryEvent>,
    #[serde(default)]
    pub suggested_actions: Vec<RunSuggestedAction>,
}

#[derive(Debug, Clone)]
pub struct CaptureRunReportInput {
    pub instance_id: String,
    pub launch_method: String,
    pub exit_kind: String,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
    pub java_path: Option<String>,
    pub java_major: Option<u32>,
    pub launch_log_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResetItem {
    pub path: String,
    pub backup_path: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetConfigFilesResult {
    pub dry_run: bool,
    pub reset_count: usize,
    pub skipped_count: usize,
    pub backups_created: usize,
    #[serde(default)]
    pub items: Vec<ConfigResetItem>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunReportsStoreV1 {
    version: u32,
    #[serde(default)]
    reports: Vec<InstanceRunReport>,
}

impl Default for RunReportsStoreV1 {
    fn default() -> Self {
        Self {
            version: 1,
            reports: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstanceEventsStoreV1 {
    version: u32,
    #[serde(default)]
    events: Vec<InstanceHistoryEvent>,
}

impl Default for InstanceEventsStoreV1 {
    fn default() -> Self {
        Self {
            version: 1,
            events: Vec::new(),
        }
    }
}

fn run_reports_store_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(RUN_REPORTS_STORE_FILE)
}

fn instance_events_store_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(INSTANCE_EVENTS_STORE_FILE)
}

fn read_run_reports_store(instance_dir: &Path) -> RunReportsStoreV1 {
    let path = run_reports_store_path(instance_dir);
    if !path.exists() {
        return RunReportsStoreV1::default();
    }
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<RunReportsStoreV1>(&raw).unwrap_or_default(),
        Err(_) => RunReportsStoreV1::default(),
    }
}

fn write_run_reports_store(
    instance_dir: &Path,
    mut store: RunReportsStoreV1,
) -> Result<(), String> {
    store.version = 1;
    if store.reports.len() > MAX_RUN_REPORTS {
        store.reports.truncate(MAX_RUN_REPORTS);
    }
    let raw = serde_json::to_string_pretty(&store)
        .map_err(|e| format!("serialize run reports store failed: {e}"))?;
    fs::write(run_reports_store_path(instance_dir), raw)
        .map_err(|e| format!("write run reports store failed: {e}"))
}

fn read_instance_events_store(instance_dir: &Path) -> InstanceEventsStoreV1 {
    let path = instance_events_store_path(instance_dir);
    if !path.exists() {
        return InstanceEventsStoreV1::default();
    }
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<InstanceEventsStoreV1>(&raw).unwrap_or_default(),
        Err(_) => InstanceEventsStoreV1::default(),
    }
}

fn write_instance_events_store(
    instance_dir: &Path,
    mut store: InstanceEventsStoreV1,
) -> Result<(), String> {
    store.version = 1;
    if store.events.len() > MAX_INSTANCE_EVENTS {
        let drop = store.events.len().saturating_sub(MAX_INSTANCE_EVENTS);
        store.events.drain(0..drop);
    }
    let raw = serde_json::to_string_pretty(&store)
        .map_err(|e| format!("serialize instance events store failed: {e}"))?;
    fs::write(instance_events_store_path(instance_dir), raw)
        .map_err(|e| format!("write instance events store failed: {e}"))
}

fn read_log_preview(path: &Path) -> String {
    if !path.exists() {
        return String::new();
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return String::new();
    };
    let mut lines = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() > 2500 {
        let start = lines.len().saturating_sub(2500);
        lines = lines[start..].to_vec();
    }
    lines.join("\n")
}

fn clean_for_event_summary(raw: &str) -> String {
    let collapsed = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if collapsed.len() <= 180 {
        return collapsed;
    }
    format!("{}...", &collapsed[..177])
}

fn action_id(prefix: &str) -> String {
    format!("{}_{}", prefix, now_millis())
}

fn build_suggested_actions(
    instance_dir: &Path,
    lock: &Lockfile,
    findings: &[RunFinding],
    required_java_major: u32,
) -> Vec<RunSuggestedAction> {
    let mut out = Vec::<RunSuggestedAction>::new();

    let snapshot = list_snapshots(instance_dir)
        .ok()
        .and_then(|items| items.into_iter().next());
    if let Some(item) = snapshot {
        out.push(RunSuggestedAction {
            id: action_id("rollback_snapshot"),
            kind: "rollback_snapshot".to_string(),
            title: "Rollback to latest snapshot".to_string(),
            detail: "Restore the most recent content snapshot for this instance.".to_string(),
            dry_run: format!(
                "Would restore snapshot '{}' created at {}.",
                item.id, item.created_at
            ),
            reversible: true,
            payload: Some(serde_json::json!({
                "snapshotId": item.id,
            })),
        });
    }

    if findings
        .iter()
        .any(|item| item.id == "java_version_mismatch")
    {
        out.push(RunSuggestedAction {
            id: action_id("open_java_settings"),
            kind: "open_java_settings".to_string(),
            title: "Open Java settings".to_string(),
            detail: "Review Java runtime selection with the recommended major preselected."
                .to_string(),
            dry_run: format!(
                "Would open Java settings and recommend Java {}+ for this instance.",
                required_java_major
            ),
            reversible: true,
            payload: Some(serde_json::json!({
                "requiredJavaMajor": required_java_major,
            })),
        });
    }

    let mut reset_paths = Vec::<String>::new();
    for finding in findings {
        if finding.id != "config_parse_error" {
            continue;
        }
        if let Some(path) = finding.file_path.as_ref() {
            reset_paths.push(path.clone());
        }
    }
    reset_paths.sort();
    reset_paths.dedup();
    if !reset_paths.is_empty() {
        let preview = reset_paths
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        out.push(RunSuggestedAction {
            id: action_id("reset_configs"),
            kind: "reset_config_files".to_string(),
            title: "Reset broken config file(s)".to_string(),
            detail: "Backs up each file first, then resets only flagged files.".to_string(),
            dry_run: format!(
                "Would back up and reset {} config file(s): {}",
                reset_paths.len(),
                if preview.is_empty() {
                    "(paths unavailable)"
                } else {
                    &preview
                }
            ),
            reversible: true,
            payload: Some(serde_json::json!({
                "paths": reset_paths,
            })),
        });
    }

    let suspect_tokens = findings
        .iter()
        .filter_map(|finding| finding.mod_id.as_ref())
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>();
    if !suspect_tokens.is_empty() {
        let mut version_ids = Vec::<String>::new();
        let mut labels = Vec::<String>::new();
        for entry in &lock.entries {
            if !entry.enabled || crate::normalize_lock_content_type(&entry.content_type) != "mods" {
                continue;
            }
            let haystack = format!(
                "{} {} {}",
                entry.project_id.to_lowercase(),
                entry.name.to_lowercase(),
                entry.filename.to_lowercase()
            );
            if !suspect_tokens.iter().any(|token| haystack.contains(token)) {
                continue;
            }
            if version_ids.contains(&entry.version_id) {
                continue;
            }
            version_ids.push(entry.version_id.clone());
            labels.push(entry.name.clone());
            if version_ids.len() >= 5 {
                break;
            }
        }
        if !version_ids.is_empty() {
            out.push(RunSuggestedAction {
                id: action_id("disable_suspects"),
                kind: "disable_suspect_mods".to_string(),
                title: "Disable suspect mod(s)".to_string(),
                detail: "Temporarily disable high-signal suspect mods from this report."
                    .to_string(),
                dry_run: format!(
                    "Would disable {} mod(s): {}",
                    labels.len(),
                    labels.join(", ")
                ),
                reversible: true,
                payload: Some(serde_json::json!({
                    "versionIds": version_ids,
                })),
            });
        }
    }

    out.push(RunSuggestedAction {
        id: action_id("open_logs"),
        kind: "open_logs".to_string(),
        title: "Open latest logs".to_string(),
        detail: "Open launch/crash logs in Finder/Explorer for deeper inspection.".to_string(),
        dry_run: "Would open the best available latest launch/crash log path.".to_string(),
        reversible: true,
        payload: None,
    });
    out.push(RunSuggestedAction {
        id: action_id("export_support_bundle"),
        kind: "export_support_bundle".to_string(),
        title: "Export support bundle".to_string(),
        detail: "Create a redacted support bundle for debugging and sharing.".to_string(),
        dry_run: "Would open support-bundle export with redaction enabled by default.".to_string(),
        reversible: true,
        payload: None,
    });

    out
}

pub(crate) fn record_instance_event_for_dir(
    instance_dir: &Path,
    kind: &str,
    summary: &str,
) -> Result<(), String> {
    let clean_kind = kind.trim().to_lowercase();
    let clean_summary = clean_for_event_summary(summary);
    if clean_kind.is_empty() || clean_summary.is_empty() {
        return Ok(());
    }
    let mut store = read_instance_events_store(instance_dir);
    store.events.push(InstanceHistoryEvent {
        id: format!("evt_{}_{}", now_millis(), clean_kind.replace(' ', "_")),
        at: now_iso(),
        kind: clean_kind,
        summary: clean_summary,
    });
    write_instance_events_store(instance_dir, store)
}

pub(crate) fn record_instance_event(
    instances_dir: &Path,
    instance_id: &str,
    kind: &str,
    summary: &str,
) -> Result<(), String> {
    let instance_dir = instance_dir_for_id(instances_dir, instance_id)?;
    record_instance_event_for_dir(&instance_dir, kind, summary)
}

pub(crate) fn log_instance_event(
    app: &tauri::AppHandle,
    instance_id: &str,
    kind: &str,
    summary: &str,
) -> Result<(), String> {
    let instances_dir = app_instances_dir(app)?;
    record_instance_event(&instances_dir, instance_id, kind, summary)
}

pub(crate) fn list_instance_history_events(
    app: &tauri::AppHandle,
    instance_id: &str,
    limit: usize,
    before_at: Option<&str>,
    kinds: Option<&[String]>,
) -> Result<Vec<InstanceHistoryEvent>, String> {
    let instances_dir = app_instances_dir(app)?;
    let instance_dir = instance_dir_for_id(&instances_dir, instance_id)?;
    let mut events = read_instance_events_store(&instance_dir).events;
    if let Some(before) = before_at {
        let before_trimmed = before.trim();
        if !before_trimmed.is_empty() {
            events.retain(|item| item.at.as_str() < before_trimmed);
        }
    }
    if let Some(filter) = kinds {
        let wanted = filter
            .iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();
        if !wanted.is_empty() {
            events.retain(|item| wanted.contains(&item.kind.to_ascii_lowercase()));
        }
    }
    events.sort_by(|a, b| b.at.cmp(&a.at));
    let cap = limit.clamp(1, MAX_INSTANCE_EVENTS);
    events.truncate(cap);
    Ok(events)
}

pub(crate) fn capture_and_store_run_report(
    app: &tauri::AppHandle,
    input: CaptureRunReportInput,
) -> Result<InstanceRunReport, String> {
    let instances_dir = app_instances_dir(app)?;
    let instance = find_instance(&instances_dir, &input.instance_id)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let instance_settings = normalize_instance_settings(instance.settings.clone());
    let required_java_major = required_java_major_for_mc(&instance.mc_version);

    let launch_log_path = input
        .launch_log_path
        .or_else(|| latest_launch_log_path(&instance_dir));
    let crash_log_path = latest_crash_report_path(&instance_dir);

    let launch_log_text = launch_log_path
        .as_ref()
        .map(|path| read_log_preview(path))
        .unwrap_or_default();
    let crash_log_text = crash_log_path
        .as_ref()
        .map(|path| read_log_preview(path))
        .unwrap_or_default();

    let lock = read_lockfile(&instances_dir, &input.instance_id).unwrap_or_default();
    let classifier_out = classifier::classify(&classifier::ClassifierInput {
        instance: &instance,
        lock: &lock,
        instance_dir: &instance_dir,
        launch_log_text: &launch_log_text,
        crash_log_text: &crash_log_text,
        java_major: input.java_major,
        required_java_major,
        exit_code: input.exit_code,
        exit_message: input.message.as_deref(),
    });
    let _classifier_hints = (
        &classifier_out.suspect_mod_tokens,
        &classifier_out.config_paths,
    );

    let mut top_causes = classifier_out
        .findings
        .iter()
        .filter(|item| item.id != "failure_phase" && item.id != "nonzero_exit_code")
        .take(3)
        .map(|item| item.title.clone())
        .collect::<Vec<_>>();
    if top_causes.is_empty() {
        top_causes.push("No high-confidence cause detected".to_string());
    }

    let mut recent_changes = read_instance_events_store(&instance_dir)
        .events
        .into_iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>();
    recent_changes.reverse();

    let suggested_actions = build_suggested_actions(
        &instance_dir,
        &lock,
        &classifier_out.findings,
        required_java_major,
    );

    let mut artifacts = Vec::<RunArtifactRef>::new();
    if let Some(path) = launch_log_path.as_ref() {
        artifacts.push(RunArtifactRef {
            kind: "latest_launch".to_string(),
            path: path.display().to_string(),
            exists: path.exists(),
        });
    }
    if let Some(path) = crash_log_path.as_ref() {
        artifacts.push(RunArtifactRef {
            kind: "latest_crash".to_string(),
            path: path.display().to_string(),
            exists: path.exists(),
        });
    }

    let report = InstanceRunReport {
        id: format!("run_{}_{}", now_millis(), input.launch_method),
        instance_id: input.instance_id.clone(),
        created_at: now_iso(),
        launch_method: input.launch_method,
        mc_version: instance.mc_version.clone(),
        loader: instance.loader.clone(),
        java_path: input.java_path.or_else(|| {
            (!instance_settings.java_path.trim().is_empty())
                .then(|| instance_settings.java_path.clone())
        }),
        java_major: input.java_major,
        required_java_major,
        memory_mb: instance_settings.memory_mb,
        jvm_args: instance_settings.jvm_args.clone(),
        exit_kind: input.exit_kind,
        exit_code: input.exit_code,
        message: input.message,
        artifacts,
        findings: classifier_out.findings,
        top_causes,
        phase: classifier_out.phase,
        recent_changes,
        suggested_actions,
    };

    let mut store = read_run_reports_store(&instance_dir);
    store.reports.insert(0, report.clone());
    if store.reports.len() > MAX_RUN_REPORTS {
        store.reports.truncate(MAX_RUN_REPORTS);
    }
    write_run_reports_store(&instance_dir, store)?;

    Ok(report)
}

pub(crate) fn latest_run_report(
    app: &tauri::AppHandle,
    instance_id: &str,
) -> Result<Option<InstanceRunReport>, String> {
    let instances_dir = app_instances_dir(app)?;
    let instance_dir = instance_dir_for_id(&instances_dir, instance_id)?;
    let store = read_run_reports_store(&instance_dir);
    Ok(store.reports.into_iter().next())
}

pub(crate) fn list_run_reports(
    app: &tauri::AppHandle,
    instance_id: &str,
    limit: usize,
) -> Result<Vec<InstanceRunReport>, String> {
    let instances_dir = app_instances_dir(app)?;
    let instance_dir = instance_dir_for_id(&instances_dir, instance_id)?;
    let store = read_run_reports_store(&instance_dir);
    Ok(store
        .reports
        .into_iter()
        .take(limit.clamp(1, MAX_RUN_REPORTS))
        .collect())
}

fn reset_default_content_for_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".json") {
        return "{}\n";
    }
    ""
}

pub(crate) fn reset_instance_config_files_with_backup(
    app: &tauri::AppHandle,
    instance_id: &str,
    paths: &[String],
    dry_run: bool,
) -> Result<ResetConfigFilesResult, String> {
    let instances_dir = app_instances_dir(app)?;
    let instance_dir = instance_dir_for_id(&instances_dir, instance_id)?;

    let mut unique_paths = paths
        .iter()
        .map(|value| value.trim().replace('\\', "/"))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    unique_paths.sort();
    unique_paths.dedup();
    if unique_paths.is_empty() {
        return Ok(ResetConfigFilesResult {
            dry_run,
            reset_count: 0,
            skipped_count: 0,
            backups_created: 0,
            items: Vec::new(),
            message: "No config paths were provided.".to_string(),
        });
    }

    let backup_root = instance_dir
        .join("config_backups")
        .join(format!("reset-{}", now_millis()));
    let mut items = Vec::<ConfigResetItem>::new();
    let mut reset_count = 0usize;
    let mut skipped_count = 0usize;
    let mut backups_created = 0usize;

    for path in &unique_paths {
        let read_result = match friend_link::state::read_instance_config_file(
            &instances_dir,
            instance_id,
            path,
        ) {
            Ok(value) => value,
            Err(err) => {
                skipped_count += 1;
                items.push(ConfigResetItem {
                    path: path.clone(),
                    backup_path: String::new(),
                    status: "skipped".to_string(),
                    message: err,
                });
                continue;
            }
        };
        if !read_result.editable {
            skipped_count += 1;
            items.push(ConfigResetItem {
                path: read_result.path,
                backup_path: String::new(),
                status: "skipped".to_string(),
                message: read_result
                    .readonly_reason
                    .unwrap_or_else(|| "File is not editable".to_string()),
            });
            continue;
        }

        let backup_path = match friend_link::state::safe_join_under(&backup_root, &read_result.path)
        {
            Ok(path) => path,
            Err(err) => {
                skipped_count += 1;
                items.push(ConfigResetItem {
                    path: read_result.path,
                    backup_path: String::new(),
                    status: "skipped".to_string(),
                    message: err,
                });
                continue;
            }
        };

        let original_content = read_result.content.unwrap_or_default();
        let reset_content = reset_default_content_for_path(&read_result.path);

        if !dry_run {
            if let Some(parent) = backup_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir config backup directory failed: {e}"))?;
            }
            fs::write(&backup_path, original_content.as_bytes())
                .map_err(|e| format!("write config backup failed: {e}"))?;
            backups_created += 1;

            friend_link::state::write_instance_config_file(
                &instances_dir,
                instance_id,
                &read_result.path,
                reset_content,
                None,
            )?;
            reset_count += 1;
            items.push(ConfigResetItem {
                path: read_result.path,
                backup_path: backup_path.display().to_string(),
                status: "reset".to_string(),
                message: "Backed up and reset config file.".to_string(),
            });
        } else {
            reset_count += 1;
            items.push(ConfigResetItem {
                path: read_result.path,
                backup_path: backup_path.display().to_string(),
                status: "planned".to_string(),
                message: "Would back up then reset this config file.".to_string(),
            });
        }
    }

    if !dry_run && reset_count > 0 {
        let summary = format!(
            "Reset {} config file(s) with {} backup file(s).",
            reset_count, backups_created
        );
        let _ = record_instance_event_for_dir(&instance_dir, "config_reset", &summary);
    }

    Ok(ResetConfigFilesResult {
        dry_run,
        reset_count,
        skipped_count,
        backups_created,
        items,
        message: if dry_run {
            format!(
                "Dry run: {} file(s) would be reset, {} skipped.",
                reset_count, skipped_count
            )
        } else {
            format!("Reset {} file(s), skipped {}.", reset_count, skipped_count)
        },
    })
}
