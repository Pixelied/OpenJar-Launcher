use crate::*;
use chrono::Local;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use tauri::Manager;
use uuid::Uuid;
use zip::write::FileOptions;

fn log_instance_event_best_effort(
    app: &tauri::AppHandle,
    instance_id: &str,
    kind: &str,
    summary: String,
) {
    if let Err(err) = crate::run_reports::log_instance_event(app, instance_id, kind, &summary) {
        eprintln!(
            "instance history event write failed for '{}' [{}]: {}",
            instance_id, kind, err
        );
    }
}

fn create_instance_snapshot_with_event_best_effort(
    app: &tauri::AppHandle,
    instances_dir: &Path,
    instance_id: &str,
    reason: &str,
) {
    match create_instance_snapshot(instances_dir, instance_id, reason) {
        Ok(meta) => {
            log_instance_event_best_effort(
                app,
                instance_id,
                "snapshot_created",
                format!(
                    "Created snapshot '{}' (reason: {}).",
                    meta.id,
                    if meta.reason.trim().is_empty() {
                        "manual"
                    } else {
                        meta.reason.trim()
                    }
                ),
            );
        }
        Err(err) => {
            eprintln!(
                "snapshot creation failed for '{}' (reason '{}'): {}",
                instance_id, reason, err
            );
        }
    }
}

const DISCOVER_PROVIDER_SOURCES: [&str; 3] = ["modrinth", "curseforge", "github"];

fn create_preinstall_snapshot_with_event_best_effort(
    app: &tauri::AppHandle,
    instances_dir: &Path,
    instance_id: &str,
    reason: &str,
) {
    create_instance_snapshot_with_event_best_effort(app, instances_dir, instance_id, reason);
}

fn instance_mutation_lock(instance_id: &str) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = locks.lock().expect("instance mutation lock registry");
    guard
        .entry(instance_id.trim().to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn github_query_variant_limit() -> usize {
    if crate::github_has_configured_tokens() {
        1
    } else {
        0
    }
}

fn discover_has_explicit_source_subset(selected_sources: &[String]) -> bool {
    selected_sources.len() < DISCOVER_PROVIDER_SOURCES.len()
}

fn github_install_state(
    compatible_release_found: bool,
    release_list_available: bool,
) -> &'static str {
    if compatible_release_found {
        "ready"
    } else if release_list_available {
        "unsupported"
    } else {
        "checking"
    }
}

fn capture_run_report_best_effort(
    app: &tauri::AppHandle,
    input: crate::run_reports::CaptureRunReportInput,
) {
    let instance_id = input.instance_id.clone();
    if let Err(err) = crate::run_reports::capture_and_store_run_report(app, input) {
        eprintln!("run report capture failed for '{}': {}", instance_id, err);
    }
}

fn normalized_content_type_hint(raw: Option<&str>) -> Option<String> {
    let value = raw.unwrap_or_default().trim();
    if value.is_empty() {
        None
    } else {
        Some(normalize_lock_content_type(value))
    }
}

fn normalized_filename_hint(raw: Option<&str>) -> Option<String> {
    let value = raw.unwrap_or_default().trim();
    if value.is_empty() {
        return None;
    }
    let safe = sanitize_filename(value);
    if safe.is_empty() {
        Some(value.to_ascii_lowercase())
    } else {
        Some(safe.to_ascii_lowercase())
    }
}

fn find_lock_entry_index(
    lock: &Lockfile,
    version_id: &str,
    content_type_hint: Option<&str>,
    filename_hint: Option<&str>,
) -> Result<usize, String> {
    let mut candidates = lock
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.version_id == version_id)
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err("installed mod entry not found".to_string());
    }
    if candidates.len() == 1 {
        return Ok(candidates[0]);
    }

    let content_hint = normalized_content_type_hint(content_type_hint);
    if let Some(ref expected) = content_hint {
        let filtered = candidates
            .iter()
            .copied()
            .filter(|idx| {
                normalize_lock_content_type(&lock.entries[*idx].content_type) == *expected
            })
            .collect::<Vec<_>>();
        if !filtered.is_empty() {
            candidates = filtered;
        }
    }

    let filename_hint = normalized_filename_hint(filename_hint);
    if let Some(ref expected) = filename_hint {
        let filtered = candidates
            .iter()
            .copied()
            .filter(|idx| {
                sanitize_filename(&lock.entries[*idx].filename).to_ascii_lowercase() == *expected
            })
            .collect::<Vec<_>>();
        if !filtered.is_empty() {
            candidates = filtered;
        }
    }

    Ok(candidates[0])
}

fn collect_known_enabled_mod_ids_for_dependency_checks(
    lock: &Lockfile,
    instance_dir: &Path,
    instance_loader: &str,
) -> HashSet<String> {
    let mut out = HashSet::<String>::new();

    for entry in lock
        .entries
        .iter()
        .filter(|entry| entry.enabled && normalize_lock_content_type(&entry.content_type) == "mods")
    {
        for mod_id in entry
            .local_analysis
            .as_ref()
            .map(|analysis| analysis.mod_ids.clone())
            .unwrap_or_default()
        {
            if let Some(normalized) = normalize_local_mod_id(&mod_id) {
                out.insert(normalized);
            }
        }

        if let Some(normalized) = normalize_local_mod_id(&entry.project_id) {
            out.insert(normalized);
        }
        for candidate in &entry.provider_candidates {
            if let Some(normalized) = normalize_local_mod_id(&candidate.project_id) {
                out.insert(normalized);
            }
        }

        let has_local_ids = entry
            .local_analysis
            .as_ref()
            .map(|analysis| !analysis.mod_ids.is_empty())
            .unwrap_or(false);
        if has_local_ids {
            continue;
        }
        if let Ok(Some(path)) = local_entry_file_read_path(instance_dir, entry) {
            if let Ok(bytes) = fs::read(&path) {
                let scanned =
                    analyze_local_mod_file(&entry.filename, &bytes, Some(instance_loader), None);
                for mod_id in scanned.mod_ids {
                    if let Some(normalized) = normalize_local_mod_id(&mod_id) {
                        out.insert(normalized);
                    }
                }
            }
        }
    }

    out
}

const MINECRAFT_SETTINGS_SYNC_FILES: &[&str] = &[
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "servers.dat",
];

fn copy_file_atomic(src: &Path, dst: &Path) -> Result<(), String> {
    let parent = dst
        .parent()
        .ok_or_else(|| format!("target file has no parent: '{}'", dst.display()))?;
    fs::create_dir_all(parent).map_err(|e| {
        format!(
            "create target parent directory failed for '{}': {e}",
            parent.display()
        )
    })?;
    let tmp = parent.join(format!(
        ".openjar-sync-{}.tmp",
        Uuid::new_v4().to_string().replace('-', "")
    ));
    fs::copy(src, &tmp).map_err(|e| {
        format!(
            "copy settings file '{}' to temporary '{}' failed: {e}",
            src.display(),
            tmp.display()
        )
    })?;
    if dst.exists() {
        fs::remove_file(dst).map_err(|e| {
            format!(
                "remove existing settings file '{}' before atomic replace failed: {e}",
                dst.display()
            )
        })?;
    }
    fs::rename(&tmp, dst).map_err(|e| {
        format!(
            "atomic rename settings file '{}' -> '{}' failed: {e}",
            tmp.display(),
            dst.display()
        )
    })?;
    Ok(())
}

fn sync_instance_minecraft_settings_before_launch(
    instances_dir: &Path,
    source_instance: &Instance,
    source_settings: &InstanceSettings,
) -> Result<usize, String> {
    if !source_settings.sync_minecraft_settings {
        return Ok(0);
    }
    let source_dir = instance_dir_for_instance(instances_dir, source_instance);
    let source_target = source_settings.sync_minecraft_settings_target.trim();
    let index = read_index(instances_dir)?;
    let target_ids: Vec<String> = if source_target.eq_ignore_ascii_case("all") {
        index
            .instances
            .iter()
            .filter(|item| item.id != source_instance.id)
            .map(|item| item.id.clone())
            .collect()
    } else if source_target.eq_ignore_ascii_case("none") || source_target.is_empty() {
        Vec::new()
    } else {
        index
            .instances
            .iter()
            .find(|item| item.id == source_target && item.id != source_instance.id)
            .map(|item| vec![item.id.clone()])
            .unwrap_or_default()
    };
    if target_ids.is_empty() {
        return Ok(0);
    }

    let mut copied_files = 0usize;
    let mut attempted_files = 0usize;
    let mut skipped_missing_source = 0usize;
    let mut mirrored_dot_minecraft = 0usize;
    for filename in MINECRAFT_SETTINGS_SYNC_FILES {
        let Some(source_file) = resolve_minecraft_settings_source_file(&source_dir, filename)
        else {
            skipped_missing_source += target_ids.len();
            continue;
        };
        for target_id in &target_ids {
            attempted_files += 1;
            let target_dir = instance_dir_for_id(instances_dir, target_id)?;
            fs::create_dir_all(&target_dir).map_err(|e| {
                format!(
                    "create target instance dir for settings sync failed ('{}'): {e}",
                    target_dir.display()
                )
            })?;
            let (copied, mirrored) =
                copy_minecraft_settings_file_to_target_dirs(&source_file, &target_dir, filename)?;
            copied_files += copied;
            mirrored_dot_minecraft += mirrored;
        }
    }
    eprintln!(
        "minecraft settings sync summary for '{}': attempted={}, copied={}, mirrored_dot_minecraft={}, skipped_missing_source={}, target_count={}",
        source_instance.id,
        attempted_files,
        copied_files,
        mirrored_dot_minecraft,
        skipped_missing_source,
        target_ids.len()
    );
    Ok(copied_files)
}

fn file_modified_time_or_epoch(path: &Path) -> SystemTime {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn resolve_minecraft_settings_source_file(source_dir: &Path, filename: &str) -> Option<PathBuf> {
    let root = source_dir.join(filename);
    let dot_mc = source_dir.join(".minecraft").join(filename);
    match (root.is_file(), dot_mc.is_file()) {
        (true, true) => {
            if file_modified_time_or_epoch(&dot_mc) > file_modified_time_or_epoch(&root) {
                Some(dot_mc)
            } else {
                Some(root)
            }
        }
        (true, false) => Some(root),
        (false, true) => Some(dot_mc),
        (false, false) => None,
    }
}

fn copy_minecraft_settings_file_to_target_dirs(
    source_file: &Path,
    target_dir: &Path,
    filename: &str,
) -> Result<(usize, usize), String> {
    let mut copied = 0usize;
    let mut mirrored_dot_minecraft = 0usize;
    let root_target = target_dir.join(filename);
    copy_file_atomic(source_file, &root_target)?;
    copied += 1;

    let dot_mc_dir = target_dir.join(".minecraft");
    if dot_mc_dir.exists() && dot_mc_dir.is_dir() {
        let dot_mc_target = dot_mc_dir.join(filename);
        copy_file_atomic(source_file, &dot_mc_target)?;
        copied += 1;
        mirrored_dot_minecraft += 1;
    }
    Ok((copied, mirrored_dot_minecraft))
}

fn hydrate_instance_settings_from_prism(
    prism_mc_dir: &Path,
    app_instance_dir: &Path,
) -> Result<usize, String> {
    let mut copied = 0usize;
    for filename in MINECRAFT_SETTINGS_SYNC_FILES {
        let prism_file = prism_mc_dir.join(filename);
        if !prism_file.is_file() {
            continue;
        }
        let target = app_instance_dir.join(filename);
        copy_file_atomic(&prism_file, &target)?;
        copied += 1;
    }
    Ok(copied)
}

fn reconcile_runtime_session_minecraft_settings(
    runtime_session_dir: &Path,
    app_instance_dir: &Path,
) -> Result<usize, String> {
    let mut copied = 0usize;
    for filename in MINECRAFT_SETTINGS_SYNC_FILES {
        let Some(source_file) =
            resolve_minecraft_settings_source_file(runtime_session_dir, filename)
        else {
            continue;
        };
        let (file_copied, _) =
            copy_minecraft_settings_file_to_target_dirs(&source_file, app_instance_dir, filename)?;
        copied += file_copied;
    }
    Ok(copied)
}

fn isolated_native_launch_success_message() -> &'static str {
    "Native launch started in disposable isolated mode. This extra run uses a temporary copy of the instance; only Minecraft settings sync back on exit."
}

fn run_instance_settings_sync_before_launch(
    app: &tauri::AppHandle,
    instances_dir: &Path,
    instance: &Instance,
    instance_settings: &InstanceSettings,
) {
    match sync_instance_minecraft_settings_before_launch(instances_dir, instance, instance_settings)
    {
        Ok(copied) if copied > 0 => {
            log_instance_event_best_effort(
                app,
                &instance.id,
                "settings_sync",
                format!("Synced Minecraft settings to {} file target(s).", copied),
            );
        }
        Ok(_) => {}
        Err(err) => {
            eprintln!(
                "minecraft settings sync before launch failed for '{}': {}",
                instance.id, err
            );
        }
    }
}

fn is_jar_filename(filename: &str) -> bool {
    Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("jar"))
        .unwrap_or(false)
}

fn provider_matches_have_transient_github_verification_issue(
    matches: &[LocalImportedProviderMatch],
) -> Option<String> {
    matches.iter().find_map(|item| {
        if !item.source.trim().eq_ignore_ascii_case("github") {
            return None;
        }
        if !item.confidence.trim().eq_ignore_ascii_case("manual") {
            return None;
        }
        if github_reason_is_transient_verification_failure(&item.reason) {
            return Some(item.reason.clone());
        }
        None
    })
}

fn mod_filename_identity_key(filename: &str) -> Option<String> {
    const NOISE: &[&str] = &[
        "mc",
        "minecraft",
        "forge",
        "fabric",
        "quilt",
        "neoforge",
        "neo",
        "loader",
        "mod",
        "mods",
        "jar",
        "client",
        "server",
        "api",
        "v",
    ];
    let stem = Path::new(filename)
        .file_stem()?
        .to_str()?
        .trim()
        .to_ascii_lowercase();
    if stem.is_empty() {
        return None;
    }
    let parts = stem
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| {
            if token.is_empty() {
                return false;
            }
            if token.chars().all(|ch| ch.is_ascii_digit()) {
                return false;
            }
            !NOISE.contains(token)
        })
        .take(4)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("_"))
    }
}

fn list_mod_jar_filenames(mods_dir: &Path) -> Result<Vec<String>, String> {
    if !mods_dir.exists() {
        return Ok(vec![]);
    }
    let mut jars: Vec<String> = Vec::new();
    let read = fs::read_dir(mods_dir)
        .map_err(|e| format!("read mods directory '{}' failed: {e}", mods_dir.display()))?;
    for ent in read {
        let ent = ent.map_err(|e| format!("read mods entry failed: {e}"))?;
        let meta = ent.metadata().map_err(|e| {
            format!(
                "read mods entry metadata '{}' failed: {e}",
                ent.path().display()
            )
        })?;
        if !meta.is_file() {
            continue;
        }
        let Some(name) = ent.file_name().to_str().map(|value| value.to_string()) else {
            continue;
        };
        if !is_jar_filename(&name) {
            continue;
        }
        jars.push(name);
    }
    jars.sort_by_key(|name| name.to_ascii_lowercase());
    Ok(jars)
}

fn detect_mod_filename_key_collisions(jar_filenames: &[String]) -> Vec<(String, Vec<String>)> {
    let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
    for filename in jar_filenames {
        let Some(key) = mod_filename_identity_key(filename) else {
            continue;
        };
        grouped.entry(key).or_default().push(filename.clone());
    }
    let mut out = grouped
        .into_iter()
        .filter_map(|(key, mut names)| {
            if names.len() < 2 {
                return None;
            }
            names.sort_by_key(|name| name.to_ascii_lowercase());
            Some((key, names))
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn remove_conflicting_local_mod_entries_for_filename(
    lock: &mut Lockfile,
    instance_dir: &Path,
    keep_filename: &str,
) -> Result<Vec<String>, String> {
    let Some(keep_key) = mod_filename_identity_key(keep_filename) else {
        return Ok(vec![]);
    };
    let mut removed: Vec<String> = Vec::new();
    let mut retained: Vec<LockEntry> = Vec::with_capacity(lock.entries.len());
    for entry in lock.entries.drain(..) {
        let is_local_mod = entry.source.trim().eq_ignore_ascii_case("local")
            && normalize_lock_content_type(&entry.content_type) == "mods";
        let collides = is_local_mod
            && mod_filename_identity_key(&entry.filename)
                .map(|key| key == keep_key)
                .unwrap_or(false);
        if !collides {
            retained.push(entry);
            continue;
        }

        let same_filename = entry.filename.eq_ignore_ascii_case(keep_filename);
        let label = if same_filename {
            format!(
                "{} (local lock entry replaced by provider-managed install)",
                entry.filename
            )
        } else {
            entry.filename.clone()
        };
        removed.push(label);

        if same_filename {
            continue;
        }
        let (enabled_path, disabled_path) = mod_paths(instance_dir, &entry.filename);
        if enabled_path.exists() {
            fs::remove_file(&enabled_path).map_err(|e| {
                format!(
                    "remove conflicting local mod file '{}' failed: {e}",
                    enabled_path.display()
                )
            })?;
        }
        if disabled_path.exists() {
            fs::remove_file(&disabled_path).map_err(|e| {
                format!(
                    "remove conflicting local disabled mod file '{}' failed: {e}",
                    disabled_path.display()
                )
            })?;
        }
    }
    lock.entries = retained;
    Ok(removed)
}

fn append_runtime_mod_diagnostics(
    launch_log_file: &mut File,
    runtime_dir: &Path,
) -> Result<Vec<(String, Vec<String>)>, String> {
    let game_dir_display = runtime_dir
        .canonicalize()
        .unwrap_or_else(|_| runtime_dir.to_path_buf());
    let mods_dir = runtime_dir.join("mods");
    let mods_dir_display = mods_dir.canonicalize().unwrap_or_else(|_| mods_dir.clone());
    let jar_filenames = list_mod_jar_filenames(&mods_dir)?;
    let collisions = detect_mod_filename_key_collisions(&jar_filenames);
    writeln!(
        launch_log_file,
        "[OpenJar] Launch preflight: game_dir={}",
        game_dir_display.display()
    )
    .map_err(|e| format!("write launch preflight game dir failed: {e}"))?;
    writeln!(
        launch_log_file,
        "[OpenJar] Launch preflight: mods_dir={}",
        mods_dir_display.display()
    )
    .map_err(|e| format!("write launch preflight mods dir failed: {e}"))?;
    writeln!(
        launch_log_file,
        "[OpenJar] Launch preflight: mods_jars_count={}",
        jar_filenames.len()
    )
    .map_err(|e| format!("write launch preflight mods jar count failed: {e}"))?;
    if jar_filenames.is_empty() {
        writeln!(
            launch_log_file,
            "[OpenJar] Launch preflight: mods_jars=<none>"
        )
        .map_err(|e| format!("write launch preflight empty jar list failed: {e}"))?;
    } else {
        for name in &jar_filenames {
            writeln!(
                launch_log_file,
                "[OpenJar] Launch preflight: mod_jar={name}"
            )
            .map_err(|e| format!("write launch preflight jar list failed: {e}"))?;
        }
    }
    if collisions.is_empty() {
        writeln!(
            launch_log_file,
            "[OpenJar] Launch preflight: mod_key_collisions=<none>"
        )
        .map_err(|e| format!("write launch preflight collision status failed: {e}"))?;
    } else {
        for (key, names) in &collisions {
            writeln!(
                launch_log_file,
                "[OpenJar] Launch preflight: mod_key_collision={key} => {}",
                names.join(", ")
            )
            .map_err(|e| format!("write launch preflight collisions failed: {e}"))?;
        }
    }
    Ok(collisions)
}

#[tauri::command]
pub(crate) fn get_launcher_settings(app: tauri::AppHandle) -> Result<LauncherSettings, String> {
    read_launcher_settings(&app)
}

#[tauri::command]
pub(crate) fn get_dev_mode_state() -> Result<bool, String> {
    Ok(is_dev_mode_enabled())
}

fn open_path_in_shell_with_audit(
    path: &Path,
    create_if_missing: bool,
    context: &str,
) -> Result<(), String> {
    eprintln!("shell-open [{context}]: {}", path.display());
    open_path_in_shell(path, create_if_missing)
}

fn reveal_path_in_shell_with_audit(
    path: &Path,
    allow_parent_fallback: bool,
    context: &str,
) -> Result<(PathBuf, bool), String> {
    eprintln!("shell-reveal [{context}]: {}", path.display());
    reveal_path_in_shell(path, allow_parent_fallback)
}

fn resolve_renderer_allowed_instance_icon_path(
    app: &tauri::AppHandle,
    path: &Path,
) -> Result<PathBuf, String> {
    let instances_dir = app_instances_dir(app)?;
    let resolved_instances_dir =
        fs::canonicalize(&instances_dir).unwrap_or_else(|_| instances_dir.clone());
    let resolved = fs::canonicalize(path).map_err(|e| format!("image file not found: {e}"))?;
    let relative = resolved
        .strip_prefix(&resolved_instances_dir)
        .map_err(|_| "Only launcher-managed instance icons may be read without a picker grant.".to_string())?;
    let Some(parent) = relative.parent() else {
        return Err("Only launcher-managed instance icons may be read without a picker grant.".to_string());
    };
    if parent.components().count() != 1 {
        return Err("Only launcher-managed instance icons may be read without a picker grant.".to_string());
    }
    let file_stem = resolved
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if file_stem != "icon" {
        return Err("Only launcher-managed instance icons may be read without a picker grant.".to_string());
    }
    Ok(resolved)
}

#[tauri::command]
pub(crate) fn pick_instance_icon_file(
    state: tauri::State<AppState>,
) -> Result<Option<GrantedImagePathResult>, String> {
    let Some(path) = pick_single_file_dialog(
        "Images",
        &["png", "jpg", "jpeg", "webp", "bmp", "gif"],
    )?
    else {
        return Ok(None);
    };
    let preview_data_url = local_image_data_url_for_path(&path)?;
    let granted = register_external_path_grant(&state, EXTERNAL_PATH_PURPOSE_INSTANCE_ICON, path, false)?;
    Ok(Some(GrantedImagePathResult {
        grant_id: granted.grant_id,
        display_path: granted.display_path,
        preview_data_url,
    }))
}

#[tauri::command]
pub(crate) fn pick_external_open_path_grants(
    state: tauri::State<AppState>,
    args: PickExternalOpenPathGrantsArgs,
) -> Result<Vec<GrantedPathResult>, String> {
    let purpose = normalize_external_open_path_purpose(&args.purpose)?;
    let wants_multiple = args.multiple.unwrap_or(false);

    let (filter_name, extensions): (&str, &[&str]) = match purpose {
        EXTERNAL_PATH_PURPOSE_MODPACK_ARCHIVE_IMPORT => {
            if wants_multiple {
                return Err("Modpack archive picker only supports a single file.".to_string());
            }
            ("Modpack archive", &["mrpack", "zip"])
        }
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT | EXTERNAL_PATH_PURPOSE_MODPACK_SPEC_IMPORT => {
            if wants_multiple {
                return Err("JSON import picker only supports a single file.".to_string());
            }
            ("JSON", &["json"])
        }
        EXTERNAL_PATH_PURPOSE_LOCAL_MOD_IMPORT | EXTERNAL_PATH_PURPOSE_MODPACK_LOCAL_JAR_IMPORT => {
            external_local_content_filter(args.content_type.as_deref().unwrap_or("mods"))?
        }
        _ => return Err("Unsupported open-picker purpose".to_string()),
    };

    let picked_paths = if wants_multiple {
        pick_multiple_files_dialog(filter_name, extensions)?.unwrap_or_default()
    } else {
        pick_single_file_dialog(filter_name, extensions)?
            .into_iter()
            .collect::<Vec<_>>()
    };

    let mut granted = Vec::with_capacity(picked_paths.len());
    for path in picked_paths {
        granted.push(register_external_path_grant(&state, purpose, path, false)?);
    }
    Ok(granted)
}

#[tauri::command]
pub(crate) fn pick_external_save_path_grant(
    state: tauri::State<AppState>,
    args: PickExternalSavePathGrantArgs,
) -> Result<Option<GrantedPathResult>, String> {
    let purpose = normalize_external_save_path_purpose(&args.purpose)?;
    let (filter_name, extensions): (&str, &[&str]) = match purpose {
        EXTERNAL_PATH_PURPOSE_PRESETS_EXPORT | EXTERNAL_PATH_PURPOSE_MODPACK_SPEC_EXPORT => {
            ("JSON", &["json"])
        }
        EXTERNAL_PATH_PURPOSE_INSTANCE_MODS_EXPORT | EXTERNAL_PATH_PURPOSE_SUPPORT_BUNDLE_EXPORT => {
            ("Zip archive", &["zip"])
        }
        _ => return Err("Unsupported save-picker purpose".to_string()),
    };
    let Some(path) = pick_save_file_dialog(args.suggested_name.as_deref(), filter_name, extensions)? else {
        return Ok(None);
    };
    Ok(Some(register_external_path_grant(
        &state,
        purpose,
        path,
        true,
    )?))
}

#[tauri::command]
pub(crate) fn set_dev_curseforge_api_key(
    _app: tauri::AppHandle,
    args: SetDevCurseforgeApiKeyArgs,
) -> Result<String, String> {
    if !is_dev_mode_enabled() {
        return Err("Dev mode is disabled. Set MPM_DEV_MODE=1 and restart.".to_string());
    }
    let trimmed = args.key.trim().to_string();
    if trimmed.is_empty() {
        return Err("API key cannot be empty.".to_string());
    }
    keyring_set_dev_curseforge_key(&trimmed)?;
    if let Err(err) = verify_dev_curseforge_key_secure_storage_write(&trimmed) {
        log_secure_storage_verification_warning("dev CurseForge API key", &err);
    }
    std::env::set_var(DEV_RUNTIME_CURSEFORGE_API_KEY_ENV, &trimmed);
    Ok("Saved dev CurseForge API key. It is active immediately for this app session.".to_string())
}

#[tauri::command]
pub(crate) fn clear_dev_curseforge_api_key(app: tauri::AppHandle) -> Result<String, String> {
    if !is_dev_mode_enabled() {
        return Err("Dev mode is disabled. Set MPM_DEV_MODE=1 and restart.".to_string());
    }
    let _ = app;
    std::env::remove_var(DEV_RUNTIME_CURSEFORGE_API_KEY_ENV);
    keyring_delete_dev_curseforge_key()?;
    Ok("Cleared saved dev CurseForge API key.".to_string())
}

#[tauri::command]
pub(crate) fn get_curseforge_api_status() -> Result<CurseforgeApiStatus, String> {
    let Some((api_key, source)) = curseforge_api_key_with_source() else {
        return Ok(CurseforgeApiStatus {
            configured: false,
            env_var: None,
            key_hint: None,
            validated: false,
            message: "No CurseForge API key configured for this build. Maintainers can inject MPM_CURSEFORGE_API_KEY_BUILTIN at build time, use Dev mode secure key storage, or set MPM_CURSEFORGE_API_KEY for local development.".to_string(),
        });
    };

    let client = build_http_client()?;
    let url = format!(
        "{}/games/{}",
        CURSEFORGE_API_BASE, CURSEFORGE_GAME_ID_MINECRAFT
    );
    let resp = client.get(&url).header("x-api-key", api_key.clone()).send();

    match resp {
        Ok(response) => {
            if response.status().is_success() {
                Ok(CurseforgeApiStatus {
                    configured: true,
                    env_var: Some(source),
                    key_hint: Some(mask_secret(&api_key)),
                    validated: true,
                    message: "CurseForge API key is valid.".to_string(),
                })
            } else {
                let status = response.status().as_u16();
                let body = response.text().unwrap_or_default();
                let trimmed = body.chars().take(220).collect::<String>();
                Ok(CurseforgeApiStatus {
                    configured: true,
                    env_var: Some(source),
                    key_hint: Some(mask_secret(&api_key)),
                    validated: false,
                    message: if trimmed.is_empty() {
                        format!("CurseForge API key validation failed (HTTP {}).", status)
                    } else {
                        format!(
                            "CurseForge API key validation failed (HTTP {}): {}",
                            status, trimmed
                        )
                    },
                })
            }
        }
        Err(e) => Ok(CurseforgeApiStatus {
            configured: true,
            env_var: Some(source),
            key_hint: Some(mask_secret(&api_key)),
            validated: false,
            message: format!(
                "Could not validate CurseForge key right now (network/request error): {}",
                e
            ),
        }),
    }
}

#[tauri::command]
pub(crate) fn get_github_token_pool_status() -> Result<GithubTokenPoolStatus, String> {
    Ok(github_token_pool_status())
}

#[tauri::command]
pub(crate) fn set_github_token_pool(
    args: SetGithubTokenPoolArgs,
) -> Result<GithubTokenPoolStatus, String> {
    keyring_set_github_token_pool(&args.tokens)?;
    github_invalidate_token_pool_cache();
    Ok(github_token_pool_status())
}

#[tauri::command]
pub(crate) fn clear_github_token_pool() -> Result<GithubTokenPoolStatus, String> {
    keyring_delete_github_token_pool()?;
    github_invalidate_token_pool_cache();
    Ok(github_token_pool_status())
}

#[tauri::command]
pub(crate) fn set_launcher_settings(
    app: tauri::AppHandle,
    args: SetLauncherSettingsArgs,
) -> Result<LauncherSettings, String> {
    let mut settings = read_launcher_settings(&app)?;
    if let Some(method) = args.default_launch_method {
        let parsed = LaunchMethod::parse(&method)
            .ok_or_else(|| "defaultLaunchMethod must be prism or native".to_string())?;
        settings.default_launch_method = parsed;
    }
    if let Some(app_language) = args.app_language {
        settings.app_language = normalize_app_language(&app_language);
    }
    if let Some(java_path) = args.java_path {
        settings.java_path = java_path.trim().to_string();
    }
    if let Some(client_id) = args.oauth_client_id {
        settings.oauth_client_id = client_id.trim().to_string();
    }
    if let Some(cadence) = args.update_check_cadence {
        settings.update_check_cadence = normalize_update_check_cadence(&cadence);
    }
    if let Some(mode) = args.update_auto_apply_mode {
        settings.update_auto_apply_mode = normalize_update_auto_apply_mode(&mode);
    }
    if let Some(scope) = args.update_apply_scope {
        settings.update_apply_scope = normalize_update_apply_scope(&scope);
    }
    if let Some(auto_identify) = args.auto_identify_local_jars {
        settings.auto_identify_local_jars = auto_identify;
    }
    if let Some(auto_trigger_prompt) = args.auto_trigger_mic_permission_prompt {
        settings.auto_trigger_mic_permission_prompt = auto_trigger_prompt;
    }
    if let Some(enabled) = args.discord_presence_enabled {
        settings.discord_presence_enabled = enabled;
    }
    if let Some(level) = args.discord_presence_detail_level {
        settings.discord_presence_detail_level = normalize_discord_presence_detail_level(&level);
    }
    write_launcher_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub(crate) fn list_launcher_accounts(
    app: tauri::AppHandle,
) -> Result<Vec<LauncherAccount>, String> {
    read_launcher_accounts(&app)
}

#[tauri::command]
pub(crate) fn select_launcher_account(
    app: tauri::AppHandle,
    args: SelectLauncherAccountArgs,
) -> Result<LauncherSettings, String> {
    let accounts = read_launcher_accounts(&app)?;
    if !accounts.iter().any(|a| a.id == args.account_id) {
        return Err("Account not found".to_string());
    }
    let mut settings = read_launcher_settings(&app)?;
    settings.selected_account_id = Some(args.account_id);
    write_launcher_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub(crate) fn logout_microsoft_account(
    app: tauri::AppHandle,
    args: LogoutMicrosoftAccountArgs,
) -> Result<Vec<LauncherAccount>, String> {
    let mut accounts = read_launcher_accounts(&app)?;
    let removed_account = accounts
        .iter()
        .find(|account| account.id == args.account_id)
        .cloned();
    accounts.retain(|a| a.id != args.account_id);
    write_launcher_accounts(&app, &accounts)?;
    let mut settings = read_launcher_settings(&app)?;
    let removed_selected =
        settings.selected_account_id.as_deref() == Some(args.account_id.as_str());
    if removed_selected {
        settings.selected_account_id = None;
        write_launcher_settings(&app, &settings)?;
    }
    if let Some(account) = removed_account.as_ref() {
        if let Err(e) = keyring_delete_refresh_token_for_account(account) {
            eprintln!(
                "keyring delete failed for account {}: {}",
                args.account_id, e
            );
        }
        if let Err(e) = remove_refresh_token_recovery_fallback(&app, account) {
            eprintln!(
                "refresh-token recovery fallback cleanup failed for account {}: {}",
                args.account_id, e
            );
        }
        #[cfg(debug_assertions)]
        if let Err(e) = remove_refresh_token_debug_fallback(&app, account) {
            eprintln!(
                "debug refresh-token fallback cleanup failed for account {}: {}",
                args.account_id, e
            );
        }
    } else {
        delete_refresh_token_everywhere(&app, &args.account_id);
        if let Err(e) = remove_refresh_token_recovery_fallback_for_key(&app, &args.account_id) {
            eprintln!(
                "refresh-token recovery fallback cleanup failed for account key {}: {}",
                args.account_id, e
            );
        }
    }
    if removed_selected {
        if let Err(e) = keyring_delete_selected_refresh_token() {
            eprintln!("keyring delete failed for selected refresh alias: {}", e);
        }
    }
    Ok(accounts)
}

#[tauri::command]
pub(crate) async fn begin_microsoft_login(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BeginMicrosoftLoginResult, String> {
    let (client_id, client_id_source) = resolve_oauth_client_id_with_source(&app)?;
    let session_id = format!("ms_{}", Uuid::new_v4());
    let client_id_for_flow = client_id.clone();
    let flow = run_blocking_task("microsoft device code", move || {
        let client = build_http_client()?;
        microsoft_begin_device_code(&client, &client_id_for_flow)
    })
    .await?;
    let verification_uri = flow.verification_uri.clone();
    let user_code = flow.user_code.clone();
    let interval = if flow.interval == 0 { 5 } else { flow.interval };
    let expires_in = if flow.expires_in == 0 {
        900
    } else {
        flow.expires_in
    };
    let pending_message = flow
        .message
        .clone()
        .unwrap_or_else(|| format!("Open {} and enter code {}", verification_uri, user_code));

    set_login_session_state(
        &state.login_sessions,
        &session_id,
        "pending",
        Some(pending_message),
        None,
    );

    let sessions = state.login_sessions.clone();
    let app_for_thread = app.clone();
    let session_id_for_thread = session_id.clone();
    let client_id_for_thread = client_id.clone();
    let client_id_source_for_thread = client_id_source.clone();
    thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(expires_in + 20);
        let mut poll_interval_secs = interval.max(2);

        let client = match build_http_client() {
            Ok(c) => c,
            Err(e) => {
                set_login_session_state(
                    &sessions,
                    &session_id_for_thread,
                    "error",
                    Some(format!("build http client failed: {e}")),
                    None,
                );
                return;
            }
        };

        loop {
            if matches!(
                get_login_session_status(&sessions, &session_id_for_thread).as_deref(),
                Some("cancelled")
            ) {
                return;
            }

            if Instant::now() >= deadline {
                set_login_session_state(
                    &sessions,
                    &session_id_for_thread,
                    "error",
                    Some("Microsoft login timed out. Please try again.".to_string()),
                    None,
                );
                return;
            }

            let params = [
                ("client_id", client_id_for_thread.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", flow.device_code.as_str()),
            ];
            let response = match client
                .post(MS_TOKEN_URL)
                .header("Accept", "application/json")
                .form(&params)
                .send()
            {
                Ok(r) => r,
                Err(e) => {
                    set_login_session_state(
                        &sessions,
                        &session_id_for_thread,
                        "error",
                        Some(format!("Microsoft device token polling failed: {e}")),
                        None,
                    );
                    return;
                }
            };

            if matches!(
                get_login_session_status(&sessions, &session_id_for_thread).as_deref(),
                Some("cancelled")
            ) {
                return;
            }

            if response.status().is_success() {
                let token = match response.json::<MsoTokenResponse>() {
                    Ok(v) => v,
                    Err(e) => {
                        set_login_session_state(
                            &sessions,
                            &session_id_for_thread,
                            "error",
                            Some(format!("parse Microsoft device token response failed: {e}")),
                            None,
                        );
                        return;
                    }
                };

                let result = (|| -> Result<LauncherAccount, String> {
                    let refresh = token.refresh_token.ok_or_else(|| {
                        "Microsoft login did not return refresh token.".to_string()
                    })?;
                    let mc_access = microsoft_access_to_mc_token(&client, &token.access_token)?;
                    ensure_minecraft_entitlement(&client, &mc_access)?;
                    let profile = fetch_minecraft_profile(&client, &mc_access)?;
                    let account = LauncherAccount {
                        id: profile.id,
                        username: profile.name,
                        added_at: now_iso(),
                    };
                    persist_refresh_token_for_launcher_account_with_app(
                        &app_for_thread,
                        &account,
                        &refresh,
                    )?;
                    upsert_launcher_account(&app_for_thread, &account)?;

                    let mut settings = read_launcher_settings(&app_for_thread)?;
                    settings.selected_account_id = Some(account.id.clone());
                    write_launcher_settings(&app_for_thread, &settings)?;
                    Ok(account)
                })();

                match result {
                    Ok(account) => {
                        set_login_session_state(
                            &sessions,
                            &session_id_for_thread,
                            "success",
                            Some("Microsoft account connected.".to_string()),
                            Some(account),
                        );
                    }
                    Err(err) => {
                        set_login_session_state(
                            &sessions,
                            &session_id_for_thread,
                            "error",
                            Some(err),
                            None,
                        );
                    }
                }
                return;
            }

            let err_body = response
                .text()
                .unwrap_or_else(|_| "unknown token polling error".to_string());
            let parsed = serde_json::from_str::<serde_json::Value>(&err_body).ok();
            let err_code = parsed
                .as_ref()
                .and_then(|v| v.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let err_desc = parsed
                .as_ref()
                .and_then(|v| v.get("error_description"))
                .and_then(|v| v.as_str())
                .unwrap_or(err_body.as_str());

            if err_code.eq_ignore_ascii_case("authorization_pending") {
                thread::sleep(Duration::from_secs(poll_interval_secs));
                continue;
            }
            if err_code.eq_ignore_ascii_case("slow_down") {
                poll_interval_secs = (poll_interval_secs + 2).min(15);
                thread::sleep(Duration::from_secs(poll_interval_secs));
                continue;
            }
            if err_code.eq_ignore_ascii_case("authorization_declined")
                || err_code.eq_ignore_ascii_case("bad_verification_code")
                || err_code.eq_ignore_ascii_case("expired_token")
            {
                set_login_session_state(
                    &sessions,
                    &session_id_for_thread,
                    "error",
                    Some(format!("Microsoft login cancelled/expired: {err_desc}")),
                    None,
                );
                return;
            }

            set_login_session_state(
                &sessions,
                &session_id_for_thread,
                "error",
                Some(normalize_microsoft_login_error(
                    err_code,
                    err_desc,
                    &client_id_source_for_thread,
                )),
                None,
            );
            return;
        }
    });

    if let Err(e) = tauri::api::shell::open(&app.shell_scope(), verification_uri.clone(), None) {
        set_login_session_state(
            &state.login_sessions,
            &session_id,
            "pending",
            Some(format!(
                "Open {} and enter code {} (browser auto-open failed: {})",
                verification_uri, user_code, e
            )),
            None,
        );
    }

    Ok(BeginMicrosoftLoginResult {
        session_id,
        auth_url: verification_uri.clone(),
        user_code: Some(user_code),
        verification_uri: Some(verification_uri),
    })
}

#[tauri::command]
pub(crate) fn poll_microsoft_login(
    state: tauri::State<AppState>,
    args: PollMicrosoftLoginArgs,
) -> Result<MicrosoftLoginState, String> {
    let guard = state
        .login_sessions
        .lock()
        .map_err(|_| "lock login sessions failed".to_string())?;
    guard
        .get(&args.session_id)
        .cloned()
        .ok_or_else(|| "login session not found".to_string())
}

#[tauri::command]
pub(crate) fn cancel_microsoft_login(
    state: tauri::State<AppState>,
    args: CancelMicrosoftLoginArgs,
) -> Result<MicrosoftLoginState, String> {
    let mut guard = state
        .login_sessions
        .lock()
        .map_err(|_| "lock login sessions failed".to_string())?;
    let entry = guard
        .get_mut(&args.session_id)
        .ok_or_else(|| "login session not found".to_string())?;
    entry.status = "cancelled".to_string();
    entry.message = Some("Microsoft sign-in cancelled.".to_string());
    entry.account = None;
    Ok(entry.clone())
}

#[tauri::command]
pub(crate) fn list_running_instances(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<Vec<RunningInstance>, String> {
    let mut guard = state
        .running
        .lock()
        .map_err(|_| "lock running instances failed".to_string())?;
    let mut finished: Vec<String> = Vec::new();
    for (id, proc_entry) in guard.iter_mut() {
        if let Ok(mut child) = proc_entry.child.lock() {
            if let Ok(Some(status)) = child.try_wait() {
                finished.push(id.clone());
                emit_launch_state(
                    &app,
                    &proc_entry.meta.instance_id,
                    Some(&proc_entry.meta.launch_id),
                    &proc_entry.meta.method,
                    "exited",
                    &format!("Game exited with status {:?}", status.code()),
                );
            }
        }
    }
    for id in finished {
        guard.remove(&id);
    }
    let mut out: Vec<RunningInstance> = guard
        .values()
        .map(|v| {
            let mut meta = v.meta.clone();
            if meta.log_path.is_none() {
                meta.log_path = v.log_path.as_ref().map(|p| p.display().to_string());
            }
            meta
        })
        .collect();
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(out)
}

#[tauri::command]
pub(crate) fn stop_running_instance(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: StopRunningInstanceArgs,
) -> Result<(), String> {
    let removed = {
        let mut guard = state
            .running
            .lock()
            .map_err(|_| "lock running instances failed".to_string())?;
        guard.remove(&args.launch_id)
    };
    let Some(proc_entry) = removed else {
        return Err("Running instance not found".to_string());
    };
    {
        let mut stop_guard = state
            .stop_requested_launches
            .lock()
            .map_err(|_| "lock stop requested launches failed".to_string())?;
        stop_guard.insert(args.launch_id.clone());
    }
    if let Ok(mut child) = proc_entry.child.lock() {
        let _ = child.kill();
    }
    emit_launch_state(
        &app,
        &proc_entry.meta.instance_id,
        Some(&proc_entry.meta.launch_id),
        &proc_entry.meta.method,
        "stopped",
        "Instance stop requested.",
    );
    Ok(())
}

#[tauri::command]
pub(crate) fn cancel_instance_launch(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: CancelInstanceLaunchArgs,
) -> Result<String, String> {
    let instance_id = args.instance_id.trim();
    if instance_id.is_empty() {
        return Err("instanceId is required".to_string());
    }

    mark_launch_cancel_request(&state, instance_id)?;

    let mut stopped_any = false;
    let removed = {
        let mut guard = state
            .running
            .lock()
            .map_err(|_| "lock running instances failed".to_string())?;
        let keys = guard
            .iter()
            .filter_map(|(id, proc_entry)| {
                if proc_entry.meta.instance_id == instance_id {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut removed_entries = Vec::new();
        for key in keys {
            if let Some(entry) = guard.remove(&key) {
                removed_entries.push(entry);
            }
        }
        removed_entries
    };

    for proc_entry in removed {
        stopped_any = true;
        if let Ok(mut stop_guard) = state.stop_requested_launches.lock() {
            stop_guard.insert(proc_entry.meta.launch_id.clone());
        }
        if let Ok(mut child) = proc_entry.child.lock() {
            let _ = child.kill();
        }
        emit_launch_state(
            &app,
            &proc_entry.meta.instance_id,
            Some(&proc_entry.meta.launch_id),
            &proc_entry.meta.method,
            "stopped",
            "Launch cancelled by user.",
        );
    }

    if stopped_any {
        Ok("Launch cancellation requested. Stop signal sent.".to_string())
    } else {
        Ok("Launch cancellation requested.".to_string())
    }
}

#[tauri::command]
pub(crate) fn open_instance_path(
    app: tauri::AppHandle,
    args: OpenInstancePathArgs,
) -> Result<OpenInstancePathResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let (target, resolved_path, create_if_missing) =
        resolve_target_instance_path(&instance_dir, &args.target)?;
    open_path_in_shell_with_audit(&resolved_path, create_if_missing, "instance_path")?;
    Ok(OpenInstancePathResult {
        target,
        path: resolved_path.display().to_string(),
    })
}

#[tauri::command]
pub(crate) fn reveal_config_editor_file(
    app: tauri::AppHandle,
    args: RevealConfigEditorFileArgs,
) -> Result<RevealConfigEditorFileResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let scope = args.scope.trim().to_lowercase();

    if scope == "instance" {
        if let Some(path) = args
            .path
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
        {
            let read = crate::friend_link::state::read_instance_config_file(
                &instances_dir,
                &args.instance_id,
                &path,
            )?;
            let resolved =
                crate::friend_link::state::resolve_instance_file_path_from_instances_dir(
                    &instances_dir,
                    &args.instance_id,
                    &read.path,
                )?;
            let (opened, revealed_file) =
                reveal_path_in_shell_with_audit(&resolved, true, "config_editor_instance_file")?;
            return Ok(RevealConfigEditorFileResult {
                opened_path: opened.display().to_string(),
                revealed_file,
                virtual_file: false,
                message: if revealed_file {
                    "Revealed file in your file manager.".to_string()
                } else {
                    "Opened containing folder.".to_string()
                },
            });
        }

        let (opened, _) =
            reveal_path_in_shell_with_audit(&instance_dir, false, "config_editor_instance_root")?;
        return Ok(RevealConfigEditorFileResult {
            opened_path: opened.display().to_string(),
            revealed_file: false,
            virtual_file: false,
            message: "Opened the instance folder.".to_string(),
        });
    }

    if scope == "world" {
        let world_id = args
            .world_id
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "worldId is required for world scope".to_string())?;
        let file_path = args
            .path
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "path is required for world scope".to_string())?;
        let world_root = world_root_dir(&instances_dir, &args.instance_id, &world_id)?;
        let (resolved, _) = resolve_world_file_path(&world_root, &file_path, true)?;
        let (opened, revealed_file) =
            reveal_path_in_shell_with_audit(&resolved, true, "config_editor_world_file")?;
        return Ok(RevealConfigEditorFileResult {
            opened_path: opened.display().to_string(),
            revealed_file,
            virtual_file: false,
            message: if revealed_file {
                "Revealed file in your file manager.".to_string()
            } else {
                "Opened containing folder.".to_string()
            },
        });
    }

    Err("scope must be instance or world".to_string())
}

#[tauri::command]
pub(crate) fn read_instance_logs(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: ReadInstanceLogsArgs,
) -> Result<ReadInstanceLogsResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let source_raw = args.source.trim().to_lowercase();
    let max_lines = args.max_lines.unwrap_or(2500).clamp(200, 12000);
    let before_line = args.before_line;

    let (source, path) = match source_raw.as_str() {
        "latest_crash" | "latest-crash" | "crash" => (
            "latest_crash".to_string(),
            latest_crash_report_path(&instance_dir),
        ),
        "latest_launch" | "latest-launch" | "launch" => (
            "latest_launch".to_string(),
            latest_launch_log_path(&instance_dir),
        ),
        "live" => {
            let guard = state
                .running
                .lock()
                .map_err(|_| "lock running instances failed".to_string())?;
            let mut best: Option<(String, PathBuf)> = None;
            for proc_entry in guard.values() {
                if proc_entry.meta.instance_id != args.instance_id
                    || !proc_entry.meta.method.eq_ignore_ascii_case("native")
                {
                    continue;
                }
                let Some(path) = proc_entry.log_path.clone() else {
                    continue;
                };
                match &best {
                    Some((started_at, _))
                        if started_at.as_str() >= proc_entry.meta.started_at.as_str() => {}
                    _ => {
                        best = Some((proc_entry.meta.started_at.clone(), path));
                    }
                }
            }
            (
                "live".to_string(),
                best.map(|(_, path)| path)
                    .or_else(|| latest_launch_log_path(&instance_dir)),
            )
        }
        _ => return Err("source must be live, latest_launch, or latest_crash".to_string()),
    };

    let Some(path) = path else {
        return Ok(ReadInstanceLogsResult {
            source,
            path: String::new(),
            available: false,
            total_lines: 0,
            returned_lines: 0,
            truncated: false,
            start_line_no: None,
            end_line_no: None,
            next_before_line: None,
            lines: Vec::new(),
            updated_at: 0,
            message: Some("No log file found for this source yet.".to_string()),
        });
    };

    if !path.exists() || !path.is_file() {
        return Ok(ReadInstanceLogsResult {
            source,
            path: path.display().to_string(),
            available: false,
            total_lines: 0,
            returned_lines: 0,
            truncated: false,
            start_line_no: None,
            end_line_no: None,
            next_before_line: None,
            lines: Vec::new(),
            updated_at: 0,
            message: Some("Log file does not exist yet.".to_string()),
        });
    }

    let (lines, total_lines, truncated, start_line_no, end_line_no, next_before_line) =
        read_windowed_log_lines(&path, &source, max_lines, before_line)?;
    let updated_at = fs::metadata(&path)
        .map(|meta| modified_millis(&meta))
        .unwrap_or(0);
    Ok(ReadInstanceLogsResult {
        source,
        path: path.display().to_string(),
        available: true,
        total_lines,
        returned_lines: lines.len(),
        truncated,
        start_line_no,
        end_line_no,
        next_before_line,
        lines,
        updated_at,
        message: None,
    })
}

#[tauri::command]
pub(crate) async fn list_instance_snapshots(
    app: tauri::AppHandle,
    args: ListInstanceSnapshotsArgs,
) -> Result<Vec<SnapshotMeta>, String> {
    run_blocking_task("list instance snapshots", move || {
        list_instance_snapshots_inner(app, args)
    })
    .await
}

fn list_instance_snapshots_inner(
    app: tauri::AppHandle,
    args: ListInstanceSnapshotsArgs,
) -> Result<Vec<SnapshotMeta>, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    list_snapshots(&instance_dir)
}

#[tauri::command]
pub(crate) fn rollback_instance(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: RollbackInstanceArgs,
) -> Result<RollbackResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    {
        let guard = state
            .running
            .lock()
            .map_err(|_| "lock running instances failed".to_string())?;
        if guard
            .values()
            .any(|entry| entry.meta.instance_id == args.instance_id)
        {
            return Err(
                "Stop the running Minecraft session before rolling back this instance.".to_string(),
            );
        }
    }
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let snapshots = list_snapshots(&instance_dir)?;
    if snapshots.is_empty() {
        return Err("No snapshots found for this instance".to_string());
    }
    let selected = if let Some(snapshot_id) = args.snapshot_id.as_ref() {
        snapshots
            .into_iter()
            .find(|s| s.id == *snapshot_id)
            .ok_or_else(|| "Snapshot not found".to_string())?
    } else {
        snapshots
            .into_iter()
            .next()
            .ok_or_else(|| "No snapshots found for this instance".to_string())?
    };

    let snapshot_dir = snapshots_dir(&instance_dir).join(&selected.id);
    let lock_raw = fs::read_to_string(snapshot_lock_path(&snapshot_dir))
        .map_err(|e| format!("read snapshot lock failed: {e}"))?;
    let lock: Lockfile =
        serde_json::from_str(&lock_raw).map_err(|e| format!("parse snapshot lock failed: {e}"))?;

    let restored_files =
        restore_instance_content_zip(&snapshot_content_zip_path(&snapshot_dir), &instance_dir)?;
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;
    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "snapshot_rollback",
        format!(
            "Rolled back to snapshot '{}' and restored {} file(s).",
            selected.id, restored_files
        ),
    );

    Ok(RollbackResult {
        snapshot_id: selected.id,
        created_at: selected.created_at,
        restored_files,
        message: "Rollback complete.".to_string(),
    })
}

#[tauri::command]
pub(crate) async fn list_instance_worlds(
    app: tauri::AppHandle,
    args: ListInstanceWorldsArgs,
) -> Result<Vec<InstanceWorld>, String> {
    run_blocking_task("list instance worlds", move || {
        list_instance_worlds_inner(app, args)
    })
    .await
}

fn list_instance_worlds_inner(
    app: tauri::AppHandle,
    args: ListInstanceWorldsArgs,
) -> Result<Vec<InstanceWorld>, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let saves_dir = instance_dir.join("saves");
    fs::create_dir_all(&saves_dir).map_err(|e| format!("mkdir saves failed: {e}"))?;

    let world_backups = list_world_backups(&instance_dir).unwrap_or_default();
    let mut backup_count_by_world: HashMap<String, usize> = HashMap::new();
    let mut latest_backup_by_world: HashMap<String, WorldBackupMeta> = HashMap::new();
    for meta in world_backups {
        *backup_count_by_world
            .entry(meta.world_id.clone())
            .or_insert(0) += 1;
        latest_backup_by_world
            .entry(meta.world_id.clone())
            .or_insert(meta);
    }

    let mut out = Vec::new();
    let entries = fs::read_dir(&saves_dir).map_err(|e| format!("read saves dir failed: {e}"))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read save entry failed: {e}"))?;
        let path = ent.path();
        if !path.is_dir() {
            continue;
        }
        let name = ent.file_name().to_string_lossy().to_string();
        if name.trim().is_empty() {
            continue;
        }
        let latest = latest_backup_by_world.get(&name);
        out.push(InstanceWorld {
            id: name.clone(),
            name: name.clone(),
            path: path.display().to_string(),
            latest_backup_id: latest.map(|m| m.id.clone()),
            latest_backup_at: latest.map(|m| m.created_at.clone()),
            backup_count: backup_count_by_world.get(&name).copied().unwrap_or(0),
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub(crate) async fn get_instance_disk_usage(
    app: tauri::AppHandle,
    args: GetInstanceDiskUsageArgs,
) -> Result<u64, String> {
    run_blocking_task("get instance disk usage", move || {
        let instances_dir = app_instances_dir(&app)?;
        let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
        Ok(dir_total_size_bytes(&instance_dir))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_storage_usage_overview(
    app: tauri::AppHandle,
) -> Result<StorageUsageOverview, String> {
    run_blocking_task("get storage usage overview", move || {
        Ok(scan_storage_usage_overview(&app))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_storage_usage_entries(
    app: tauri::AppHandle,
    args: GetStorageUsageEntriesArgs,
) -> Result<Vec<StorageUsageEntry>, String> {
    run_blocking_task("get storage usage entries", move || {
        let scope = args.scope.trim().to_ascii_lowercase();
        let mode = args.mode.trim().to_ascii_lowercase();
        let limit = normalize_storage_detail_limit(args.limit);
        let (root, path_kind, instance_id) =
            storage_scope_root(&app, &scope, args.instance_id.as_deref())?;
        let (base, _) = resolve_storage_base_path(&scope, &root, args.relative_path.as_deref())?;
        match mode.as_str() {
            "folders" => storage_collect_folder_entries(
                &scope,
                &root,
                &base,
                &path_kind,
                instance_id.as_deref(),
                limit,
            ),
            "files" => storage_collect_file_entries(
                &scope,
                &root,
                &base,
                &path_kind,
                instance_id.as_deref(),
                limit,
            ),
            _ => Err("storage mode must be folders or files".to_string()),
        }
    })
    .await
}

#[tauri::command]
pub(crate) async fn run_storage_cleanup(
    app: tauri::AppHandle,
    args: RunStorageCleanupArgs,
) -> Result<StorageCleanupResult, String> {
    run_blocking_task("run storage cleanup", move || {
        let mut reclaimed_bytes = 0u64;
        let mut actions_run = 0usize;
        let mut messages = Vec::new();
        let mut seen = HashSet::<String>::new();

        let instances_dir = app_instances_dir(&app)?;
        let index = read_index(&instances_dir).unwrap_or_default();
        let instances_by_id = index
            .instances
            .into_iter()
            .map(|inst| (inst.id.clone(), inst))
            .collect::<HashMap<_, _>>();
        let mut selected_instance_ids = args
            .instance_ids
            .unwrap_or_default()
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if selected_instance_ids.is_empty() {
            selected_instance_ids = instances_by_id.keys().cloned().collect();
        }

        for raw_action in args.action_ids {
            let action = raw_action.trim();
            if action.is_empty() || !seen.insert(action.to_string()) {
                continue;
            }
            if action == STORAGE_ACTION_CLEAR_SHARED_CACHE {
                let cache_dir = launcher_cache_dir(&app)?;
                let bytes = dir_total_size_bytes(&cache_dir);
                remove_path_if_exists(&cache_dir)?;
                fs::create_dir_all(&cache_dir).map_err(|e| {
                    format!(
                        "recreate launcher cache '{}' failed: {e}",
                        cache_dir.display()
                    )
                })?;
                reclaimed_bytes = reclaimed_bytes.saturating_add(bytes);
                actions_run += 1;
                messages.push(format!(
                    "Cleared shared cache and reclaimed {}.",
                    format_storage_size_label(bytes)
                ));
                continue;
            }

            let (action_kind, action_instance_ids) = if let Some(instance_id) =
                action.strip_prefix(&format!("{}:", STORAGE_ACTION_PRUNE_RUNTIME_SESSIONS))
            {
                (
                    STORAGE_ACTION_PRUNE_RUNTIME_SESSIONS,
                    vec![instance_id.to_string()],
                )
            } else if let Some(instance_id) =
                action.strip_prefix(&format!("{}:", STORAGE_ACTION_PRUNE_SNAPSHOTS))
            {
                (
                    STORAGE_ACTION_PRUNE_SNAPSHOTS,
                    vec![instance_id.to_string()],
                )
            } else if let Some(instance_id) =
                action.strip_prefix(&format!("{}:", STORAGE_ACTION_PRUNE_WORLD_BACKUPS))
            {
                (
                    STORAGE_ACTION_PRUNE_WORLD_BACKUPS,
                    vec![instance_id.to_string()],
                )
            } else if matches!(
                action,
                STORAGE_ACTION_PRUNE_RUNTIME_SESSIONS
                    | STORAGE_ACTION_PRUNE_SNAPSHOTS
                    | STORAGE_ACTION_PRUNE_WORLD_BACKUPS
            ) {
                (action, selected_instance_ids.clone())
            } else {
                return Err(format!("Unknown storage cleanup action '{action}'"));
            };

            for instance_id in action_instance_ids {
                let inst = instances_by_id
                    .get(&instance_id)
                    .ok_or_else(|| format!("Instance '{instance_id}' was not found"))?;
                let instance_dir = instance_dir_for_instance(&instances_dir, inst);
                let settings = normalize_instance_settings(inst.settings.clone());
                let targets = match action_kind {
                    STORAGE_ACTION_PRUNE_RUNTIME_SESSIONS => storage_stale_runtime_session_targets(
                        &instance_dir,
                        Duration::from_secs(STALE_RUNTIME_SESSION_MAX_AGE_HOURS * 3600),
                    )?,
                    STORAGE_ACTION_PRUNE_SNAPSHOTS => storage_snapshot_cleanup_targets(
                        &instance_dir,
                        settings.snapshot_retention_count as usize,
                        settings.snapshot_max_age_days as i64,
                    )?,
                    STORAGE_ACTION_PRUNE_WORLD_BACKUPS => storage_world_backup_cleanup_targets(
                        &instance_dir,
                        settings.world_backup_retention_count as usize,
                    )?,
                    _ => Vec::new(),
                };
                if targets.is_empty() {
                    continue;
                }
                let bytes = targets
                    .iter()
                    .map(|path| dir_total_size_bytes(path))
                    .sum::<u64>();
                for target in targets {
                    remove_path_if_exists(&target)?;
                }
                reclaimed_bytes = reclaimed_bytes.saturating_add(bytes);
                actions_run += 1;
                let action_label = match action_kind {
                    STORAGE_ACTION_PRUNE_RUNTIME_SESSIONS => "Removed stale runtime sessions",
                    STORAGE_ACTION_PRUNE_SNAPSHOTS => "Pruned old snapshots",
                    STORAGE_ACTION_PRUNE_WORLD_BACKUPS => "Pruned old world backups",
                    _ => "Cleaned storage",
                };
                messages.push(format!(
                    "{} for {} and reclaimed {}.",
                    action_label,
                    inst.name,
                    format_storage_size_label(bytes)
                ));
            }
        }

        Ok(StorageCleanupResult {
            reclaimed_bytes,
            actions_run,
            messages,
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn reveal_storage_usage_path(
    app: tauri::AppHandle,
    args: RevealStorageUsagePathArgs,
) -> Result<StorageRevealResult, String> {
    run_blocking_task("reveal storage usage path", move || {
        let scope = args.scope.trim().to_ascii_lowercase();
        let (root, _, _) = storage_scope_root(&app, &scope, args.instance_id.as_deref())?;
        let (target, _) = resolve_storage_base_path(&scope, &root, args.relative_path.as_deref())?;
        let (opened, revealed_file) =
            reveal_path_in_shell_with_audit(&target, true, "storage_usage_path")?;
        Ok(StorageRevealResult {
            opened_path: opened.display().to_string(),
            revealed_file,
            message: if revealed_file {
                "Revealed file in your file manager.".to_string()
            } else {
                "Opened storage location in your file manager.".to_string()
            },
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_instance_last_run_metadata(
    app: tauri::AppHandle,
    args: GetInstanceLastRunMetadataArgs,
) -> Result<InstanceLastRunMetadata, String> {
    run_blocking_task("get instance last-run metadata", move || {
        let instances_dir = app_instances_dir(&app)?;
        read_instance_last_run_metadata(&instances_dir, &args.instance_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_instance_playtime(
    app: tauri::AppHandle,
    args: GetInstancePlaytimeArgs,
) -> Result<InstancePlaytimeSummary, String> {
    run_blocking_task("get instance playtime", move || {
        let instances_dir = app_instances_dir(&app)?;
        instance_playtime_summary(&instances_dir, &args.instance_id)
    })
    .await
}

#[derive(Debug, Deserialize)]
pub(crate) struct GetInstanceLastRunReportArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
}

#[tauri::command]
pub(crate) async fn get_instance_last_run_report(
    app: tauri::AppHandle,
    args: GetInstanceLastRunReportArgs,
) -> Result<Option<crate::run_reports::InstanceRunReport>, String> {
    run_blocking_task("get instance last-run report", move || {
        crate::run_reports::latest_run_report(&app, &args.instance_id)
    })
    .await
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListInstanceRunReportsArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[tauri::command]
pub(crate) async fn list_instance_run_reports(
    app: tauri::AppHandle,
    args: ListInstanceRunReportsArgs,
) -> Result<Vec<crate::run_reports::InstanceRunReport>, String> {
    run_blocking_task("list instance run reports", move || {
        crate::run_reports::list_run_reports(&app, &args.instance_id, args.limit.unwrap_or(10))
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_instance_history_events(
    app: tauri::AppHandle,
    args: ListInstanceHistoryEventsArgs,
) -> Result<Vec<crate::run_reports::InstanceHistoryEvent>, String> {
    run_blocking_task("list instance history events", move || {
        crate::run_reports::list_instance_history_events(
            &app,
            &args.instance_id,
            args.limit.unwrap_or(50),
            args.before_at.as_deref(),
            args.kinds.as_deref(),
        )
    })
    .await
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResetInstanceConfigFilesArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(alias = "dryRun", default)]
    pub dry_run: bool,
}

#[tauri::command]
pub(crate) async fn reset_instance_config_files_with_backup(
    app: tauri::AppHandle,
    args: ResetInstanceConfigFilesArgs,
) -> Result<crate::run_reports::ResetConfigFilesResult, String> {
    run_blocking_task("reset instance config files", move || {
        crate::run_reports::reset_instance_config_files_with_backup(
            &app,
            &args.instance_id,
            &args.paths,
            args.dry_run,
        )
    })
    .await
}

fn running_instance_ids(state: &tauri::State<AppState>) -> Result<HashSet<String>, String> {
    let guard = state
        .running
        .lock()
        .map_err(|_| "lock running instances failed".to_string())?;
    Ok(guard
        .values()
        .map(|entry| entry.meta.instance_id.clone())
        .collect::<HashSet<_>>())
}

fn collect_world_config_files_recursive(
    world_root: &Path,
    current: &Path,
    out: &mut Vec<WorldConfigFileEntry>,
) -> Result<(), String> {
    let entries = fs::read_dir(current).map_err(|e| format!("read world directory failed: {e}"))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read world entry failed: {e}"))?;
        let path = ent.path();
        let meta =
            fs::symlink_metadata(&path).map_err(|e| format!("read world metadata failed: {e}"))?;
        let file_type = meta.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_world_config_files_recursive(world_root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(world_root)
            .map_err(|_| "failed to compute relative world file path".to_string())?;
        let rel_text = rel
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        if rel_text.is_empty() {
            continue;
        }

        let mut sample = Vec::new();
        if let Ok(mut file) = File::open(&path) {
            let mut buf = [0u8; 1024];
            if let Ok(read_len) = file.read(&mut buf) {
                sample.extend_from_slice(&buf[..read_len]);
            }
        }
        let text_like = file_is_text_like(&path, &sample);
        let kind = infer_world_file_kind(&path, text_like);
        let readonly_reason = describe_non_editable_reason(&kind, text_like);
        out.push(WorldConfigFileEntry {
            path: rel_text,
            size_bytes: meta.len(),
            modified_at: modified_millis(&meta),
            editable: readonly_reason.is_none(),
            kind,
            readonly_reason,
        });
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn list_world_config_files(
    app: tauri::AppHandle,
    args: ListWorldConfigFilesArgs,
) -> Result<Vec<WorldConfigFileEntry>, String> {
    let instances_dir = app_instances_dir(&app)?;
    let world_root = world_root_dir(&instances_dir, &args.instance_id, &args.world_id)?;
    let mut out = Vec::new();
    collect_world_config_files_recursive(&world_root, &world_root, &mut out)?;
    out.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub(crate) fn read_world_config_file(
    app: tauri::AppHandle,
    args: ReadWorldConfigFileArgs,
) -> Result<ReadWorldConfigFileResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let world_root = world_root_dir(&instances_dir, &args.instance_id, &args.world_id)?;
    let (resolved_path, normalized_path) = resolve_world_file_path(&world_root, &args.path, true)?;
    let meta = fs::metadata(&resolved_path)
        .map_err(|e| format!("read world file metadata failed: {e}"))?;
    if !meta.is_file() {
        return Err("Requested world file is not a file".to_string());
    }

    let mut file =
        File::open(&resolved_path).map_err(|e| format!("open world file failed: {e}"))?;
    let mut sample_buf = vec![0u8; 4096];
    let read_len = file
        .read(&mut sample_buf)
        .map_err(|e| format!("read world file failed: {e}"))?;
    sample_buf.truncate(read_len);
    let text_like = file_is_text_like(&resolved_path, &sample_buf[..sample_buf.len().min(1024)]);
    let kind = infer_world_file_kind(&resolved_path, text_like);
    let readonly_reason = describe_non_editable_reason(&kind, text_like);
    if readonly_reason.is_some() {
        let preview =
            format_binary_preview(&sample_buf[..sample_buf.len().min(512)], meta.len(), &kind);
        return Ok(ReadWorldConfigFileResult {
            path: normalized_path,
            editable: false,
            kind,
            size_bytes: meta.len(),
            modified_at: modified_millis(&meta),
            readonly_reason,
            content: Some(preview),
            preview: Some("hex".to_string()),
        });
    }

    let mut bytes = sample_buf;
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("read world file failed: {e}"))?;
    let content =
        String::from_utf8(bytes).map_err(|_| "File is not valid UTF-8 text.".to_string())?;
    Ok(ReadWorldConfigFileResult {
        path: normalized_path,
        editable: true,
        kind,
        size_bytes: meta.len(),
        modified_at: modified_millis(&meta),
        readonly_reason: None,
        content: Some(content),
        preview: None,
    })
}

#[tauri::command]
pub(crate) fn write_world_config_file(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: WriteWorldConfigFileArgs,
) -> Result<WriteWorldConfigFileResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let running_ids = running_instance_ids(&state)?;
    if running_ids.contains(&args.instance_id) {
        return Err("Stop the running Minecraft session before saving world files.".to_string());
    }

    let world_root = world_root_dir(&instances_dir, &args.instance_id, &args.world_id)?;
    let (resolved_path, normalized_path) = resolve_world_file_path(&world_root, &args.path, true)?;
    let before_meta = fs::metadata(&resolved_path)
        .map_err(|e| format!("read world file metadata failed: {e}"))?;
    if !before_meta.is_file() {
        return Err("Requested world file is not a file".to_string());
    }
    if let Some(expected_modified_at) = args.expected_modified_at {
        let actual_modified_at = modified_millis(&before_meta);
        if expected_modified_at != actual_modified_at {
            return Err("File changed on disk. Reload and try saving again.".to_string());
        }
    }

    let mut sample = args.content.as_bytes().to_vec();
    if sample.len() > 1024 {
        sample.truncate(1024);
    }
    let text_like = file_is_text_like(&resolved_path, &sample);
    let kind = infer_world_file_kind(&resolved_path, text_like);
    if describe_non_editable_reason(&kind, text_like).is_some() {
        return Err("Binary or unsupported world file cannot be edited.".to_string());
    }

    let parent = resolved_path
        .parent()
        .ok_or_else(|| "Invalid world file path".to_string())?;
    let tmp_name = format!(".mpm-write-{}.tmp", Uuid::new_v4());
    let tmp_path = parent.join(tmp_name);
    fs::write(&tmp_path, args.content.as_bytes())
        .map_err(|e| format!("write temp world file failed: {e}"))?;
    if let Err(err) = fs::rename(&tmp_path, &resolved_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("replace world file failed: {err}"));
    }
    let after_meta = fs::metadata(&resolved_path)
        .map_err(|e| format!("read world file metadata failed: {e}"))?;
    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "world_config_edit",
        format!(
            "Updated world '{}' config file '{}'.",
            args.world_id, normalized_path
        ),
    );
    Ok(WriteWorldConfigFileResult {
        path: normalized_path,
        size_bytes: after_meta.len(),
        modified_at: modified_millis(&after_meta),
        message: "World file saved.".to_string(),
    })
}

#[tauri::command]
pub(crate) fn rollback_instance_world_backup(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: RollbackInstanceWorldBackupArgs,
) -> Result<WorldRollbackResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    {
        let guard = state
            .running
            .lock()
            .map_err(|_| "lock running instances failed".to_string())?;
        if guard
            .values()
            .any(|entry| entry.meta.instance_id == args.instance_id)
        {
            return Err(
                "Stop the running Minecraft session before rolling back this world.".to_string(),
            );
        }
    }
    let world_id = args.world_id.trim();
    if world_id.is_empty() {
        return Err("World ID is required".to_string());
    }
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let backups = list_world_backups(&instance_dir)?;
    let selected = if let Some(backup_id) = args.backup_id.as_ref() {
        backups
            .into_iter()
            .find(|b| b.world_id == world_id && b.id == *backup_id)
            .ok_or_else(|| "World backup not found".to_string())?
    } else {
        backups
            .into_iter()
            .find(|b| b.world_id == world_id)
            .ok_or_else(|| "No world backup found for this world yet".to_string())?
    };

    let backup_dir = world_backups_dir(&instance_dir).join(&selected.id);
    let world_dir = instance_dir.join("saves").join(world_id);
    let restored_files = restore_world_backup_zip(&world_backup_zip_path(&backup_dir), &world_dir)?;
    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "world_rollback",
        format!(
            "Rolled back world '{}' to backup '{}' and restored {} file(s).",
            world_id, selected.id, restored_files
        ),
    );
    Ok(WorldRollbackResult {
        world_id: world_id.to_string(),
        backup_id: selected.id.clone(),
        created_at: selected.created_at.clone(),
        restored_files,
        message: "World rollback complete.".to_string(),
    })
}

fn install_discover_content_inner(
    app: tauri::AppHandle,
    args: &InstallDiscoverContentArgs,
    snapshot_reason: Option<&str>,
) -> Result<InstalledMod, String> {
    let source = args.source.trim().to_lowercase();
    let content_type = normalize_lock_content_type(&args.content_type);
    if content_type == "modpacks" {
        return Err(
            "Modpacks are template-only here. Use Import as Template in Modpacks & Presets."
                .to_string(),
        );
    }

    if content_type == "mods" {
        if source == "curseforge" {
            return install_curseforge_mod_inner(
                app,
                InstallCurseforgeModArgs {
                    instance_id: args.instance_id.clone(),
                    project_id: args.project_id.clone(),
                    project_title: args.project_title.clone(),
                },
                snapshot_reason,
            );
        }
        if source == "github" {
            let instances_dir = app_instances_dir(&app)?;
            let instance = find_instance(&instances_dir, &args.instance_id)?;
            let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
            let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;
            let client = build_http_client()?;

            emit_install_progress(
                &app,
                InstallProgressEvent {
                    instance_id: args.instance_id.clone(),
                    project_id: args.project_id.clone(),
                    stage: "resolving".to_string(),
                    downloaded: 0,
                    total: Some(1),
                    percent: Some(4.0),
                    message: Some("Resolving compatible GitHub release…".to_string()),
                },
            );

            if let Some(reason) = snapshot_reason {
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "snapshotting".to_string(),
                        downloaded: 0,
                        total: None,
                        percent: None,
                        message: Some("Preparing install…".to_string()),
                    },
                );
                create_preinstall_snapshot_with_event_best_effort(
                    &app,
                    &instances_dir,
                    &args.instance_id,
                    reason,
                );
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "resolving".to_string(),
                        downloaded: 0,
                        total: Some(1),
                        percent: Some(4.0),
                        message: Some("Preparing GitHub install…".to_string()),
                    },
                );
            }

            let new_entry = install_github_content_inner(
                &instance,
                &instance_dir,
                &mut lock,
                &client,
                &args.project_id,
                args.project_title.as_deref(),
                &content_type,
                &args.target_worlds,
                |stage, message, percent| {
                    emit_install_progress(
                        &app,
                        InstallProgressEvent {
                            instance_id: args.instance_id.clone(),
                            project_id: args.project_id.clone(),
                            stage: stage.to_string(),
                            downloaded: 0,
                            total: Some(1),
                            percent,
                            message: Some(message.to_string()),
                        },
                    );
                },
                |downloaded_bytes, total_bytes| {
                    let ratio = match total_bytes {
                        Some(total) if total > 0 => downloaded_bytes as f64 / total as f64,
                        _ => unknown_progress_ratio(downloaded_bytes),
                    };
                    let visible_percent = if downloaded_bytes > 0 {
                        35.0 + ratio * 64.0
                    } else {
                        35.0
                    };
                    emit_install_progress(
                        &app,
                        InstallProgressEvent {
                            instance_id: args.instance_id.clone(),
                            project_id: args.project_id.clone(),
                            stage: "downloading".to_string(),
                            downloaded: downloaded_bytes,
                            total: total_bytes,
                            percent: Some(visible_percent.clamp(35.0, 99.4)),
                            message: Some(format!(
                                "Downloading GitHub mod… · {}",
                                format_download_meter(downloaded_bytes, total_bytes)
                            )),
                        },
                    );
                },
            )?;
            lock.entries
                .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            emit_install_progress(
                &app,
                InstallProgressEvent {
                    instance_id: args.instance_id.clone(),
                    project_id: args.project_id.clone(),
                    stage: "finalizing".to_string(),
                    downloaded: 0,
                    total: None,
                    percent: None,
                    message: Some("Finishing install…".to_string()),
                },
            );
            write_lockfile(&instances_dir, &args.instance_id, &lock)?;
            log_instance_event_best_effort(
                &app,
                &args.instance_id,
                "content_install",
                format!("Installed mod '{}' via GitHub.", new_entry.name),
            );
            emit_install_progress(
                &app,
                InstallProgressEvent {
                    instance_id: args.instance_id.clone(),
                    project_id: args.project_id.clone(),
                    stage: "completed".to_string(),
                    downloaded: 1,
                    total: Some(1),
                    percent: Some(100.0),
                    message: Some("GitHub mod install complete".to_string()),
                },
            );
            return Ok(lock_entry_to_installed(&instance_dir, &new_entry));
        }
        let modrinth_reason = snapshot_reason;
        return install_modrinth_mod_inner(
            app,
            InstallModrinthModArgs {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                project_title: args.project_title.clone(),
            },
            modrinth_reason,
        );
    }

    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let content_label = content_type_display_name(&content_type);
    let source_label = if source == "curseforge" {
        "CurseForge"
    } else if source == "modrinth" {
        "Modrinth"
    } else if source == "github" {
        "GitHub"
    } else {
        "Provider"
    };

    if source == "github" {
        return Err(format!(
            "GitHub provider currently supports mods only, not {}.",
            content_label
        ));
    }

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "resolving".to_string(),
            downloaded: 0,
            total: Some(1),
            percent: None,
            message: Some(format!(
                "Resolving compatible {source_label} {content_label} file…"
            )),
        },
    );

    if let Some(reason) = snapshot_reason {
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "snapshotting".to_string(),
                downloaded: 0,
                total: None,
                percent: None,
                message: Some("Preparing install…".to_string()),
            },
        );
        create_preinstall_snapshot_with_event_best_effort(
            &app,
            &instances_dir,
            &args.instance_id,
            reason,
        );
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "resolving".to_string(),
                downloaded: 0,
                total: Some(1),
                percent: None,
                message: Some(format!("Preparing {source_label} {content_label} install…")),
            },
        );
    }

    let new_entry = if source == "curseforge" {
        let api_key = curseforge_api_key().ok_or_else(missing_curseforge_key_message)?;
        install_curseforge_content_inner(
            &instance,
            &instance_dir,
            &mut lock,
            &client,
            &api_key,
            &args.project_id,
            args.project_title.as_deref(),
            &content_type,
            &args.target_worlds,
            None,
            |downloaded_bytes, total_bytes| {
                let visible_percent = match total_bytes {
                    Some(total) if total > 0 => {
                        let ratio = downloaded_bytes as f64 / total as f64;
                        Some((28.0 + ratio * 60.0).clamp(28.0, 98.4))
                    }
                    _ => None,
                };
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "downloading".to_string(),
                        downloaded: downloaded_bytes,
                        total: total_bytes,
                        percent: visible_percent,
                        message: Some(format!(
                            "Downloading {source_label} {content_label}… · {}",
                            format_download_meter(downloaded_bytes, total_bytes)
                        )),
                    },
                );
            },
        )?
    } else {
        install_modrinth_content_inner(
            &instance,
            &instance_dir,
            &mut lock,
            &client,
            &args.project_id,
            args.project_title.as_deref(),
            &content_type,
            &args.target_worlds,
            |downloaded_bytes, total_bytes| {
                let visible_percent = match total_bytes {
                    Some(total) if total > 0 => {
                        let ratio = downloaded_bytes as f64 / total as f64;
                        Some((28.0 + ratio * 60.0).clamp(28.0, 98.4))
                    }
                    _ => None,
                };
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "downloading".to_string(),
                        downloaded: downloaded_bytes,
                        total: total_bytes,
                        percent: visible_percent,
                        message: Some(format!(
                            "Downloading {source_label} {content_label}… · {}",
                            format_download_meter(downloaded_bytes, total_bytes)
                        )),
                    },
                );
            },
        )?
    };

    lock.entries
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "finalizing".to_string(),
            downloaded: 0,
            total: None,
            percent: None,
            message: Some("Finishing install…".to_string()),
        },
    );
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;
    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "content_install",
        format!(
            "Installed {} '{}' via {}.",
            content_type_display_name(&content_type),
            new_entry.name,
            if source == "curseforge" {
                "CurseForge"
            } else if source == "github" {
                "GitHub"
            } else {
                "Modrinth"
            }
        ),
    );
    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "completed".to_string(),
            downloaded: 1,
            total: Some(1),
            percent: Some(100.0),
            message: Some(format!(
                "{} {} install complete",
                source_label, content_label
            )),
        },
    );
    Ok(lock_entry_to_installed(&instance_dir, &new_entry))
}

#[tauri::command]
pub(crate) async fn install_discover_content(
    app: tauri::AppHandle,
    args: InstallDiscoverContentArgs,
) -> Result<InstalledMod, String> {
    run_blocking_task("install discover content", move || {
        let subject = snapshot_install_subject(args.project_title.as_deref(), &args.project_id);
        let reason = format!("before-install-discover:{subject}");
        install_discover_content_inner(app, &args, Some(reason.as_str()))
    })
    .await
}

#[tauri::command]
pub(crate) fn preview_preset_apply(
    app: tauri::AppHandle,
    args: PreviewPresetApplyArgs,
) -> Result<PresetApplyPreview, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let spec = modpack::legacy_creator_preset_to_spec(&args.preset);
    let plan = modpack::resolver::resolve_modpack(
        &client,
        &instance,
        &spec,
        Some("recommended"),
        Some(spec.settings.clone()),
    )?;

    let skipped_disabled = args.preset.entries.iter().filter(|e| !e.enabled).count();
    let installable = plan.resolved_mods.len();
    let missing_world_targets = plan
        .failed_mods
        .iter()
        .filter(|f| {
            f.reason_code.eq_ignore_ascii_case("NoWorldTargets")
                || f.reason_text.to_lowercase().contains("world")
        })
        .map(|f| format!("{} ({})", f.name, f.project_id))
        .collect::<Vec<_>>();

    let mut provider_warnings = plan.warnings.clone();
    provider_warnings.extend(
        plan.failed_mods
            .iter()
            .filter(|f| f.required)
            .map(|f| format!("{}: {}", f.name, f.reason_text)),
    );
    provider_warnings.sort();
    provider_warnings.dedup();

    let duplicates = plan.conflicts.len();
    let valid = plan.failed_mods.iter().all(|f| !f.required);
    Ok(PresetApplyPreview {
        valid,
        installable_entries: installable,
        skipped_disabled_entries: skipped_disabled,
        missing_world_targets,
        provider_warnings,
        duplicate_entries: duplicates,
    })
}

#[tauri::command]
pub(crate) async fn apply_preset_to_instance(
    app: tauri::AppHandle,
    args: ApplyPresetToInstanceArgs,
) -> Result<PresetApplyResult, String> {
    run_blocking_task("apply preset to instance", move || {
        apply_preset_to_instance_inner(app, args)
    })
    .await
}

fn apply_preset_to_instance_inner(
    app: tauri::AppHandle,
    args: ApplyPresetToInstanceArgs,
) -> Result<PresetApplyResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let spec = modpack::legacy_creator_preset_to_spec(&args.preset);
    let plan = modpack::resolver::resolve_modpack(
        &client,
        &instance,
        &spec,
        Some("recommended"),
        Some(spec.settings.clone()),
    )?;

    let legacy_skipped = args.preset.entries.iter().filter(|e| !e.enabled).count();
    let (result, _lock_snapshot, _link) =
        modpack::apply::apply_plan_to_instance(&app, &plan, "unlinked", false)?;

    let mut by_content_type: HashMap<String, usize> = HashMap::new();
    for item in &plan.resolved_mods {
        *by_content_type
            .entry(normalize_lock_content_type(&item.content_type))
            .or_insert(0) += 1;
    }

    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "preset_apply",
        format!(
            "Applied preset '{}' (installed {}, failed {}).",
            args.preset.name, result.applied_entries, result.failed_entries
        ),
    );

    Ok(PresetApplyResult {
        message: result.message,
        installed_entries: result.applied_entries,
        skipped_entries: result.skipped_entries + legacy_skipped,
        failed_entries: result.failed_entries,
        snapshot_id: result.snapshot_id,
        by_content_type,
    })
}

#[tauri::command]
pub(crate) async fn search_discover_content(
    args: SearchDiscoverContentArgs,
) -> Result<DiscoverSearchResult, String> {
    run_blocking_task("search discover content", move || {
        std::panic::catch_unwind(|| search_discover_content_inner(args))
            .map_err(|_| "Discover search encountered an unexpected error".to_string())?
    })
    .await
}

fn discover_hit_dedupe_key(hit: &DiscoverSearchHit) -> String {
    let source = hit.source.trim().to_ascii_lowercase();
    let project_id = hit.project_id.trim().to_ascii_lowercase();
    if !project_id.is_empty() {
        return format!("{source}::{project_id}");
    }
    let slug = normalize_provider_match_key(hit.slug.as_deref().unwrap_or_default());
    let title = normalize_provider_match_key(&hit.title);
    format!(
        "{}::{}::{}",
        source,
        if title.is_empty() { "untitled" } else { &title },
        if slug.is_empty() { "noslug" } else { &slug },
    )
}

fn merge_discover_hits_in_place(
    base: &mut Vec<DiscoverSearchHit>,
    incoming: Vec<DiscoverSearchHit>,
    query: &str,
) {
    let mut index_by_key: HashMap<String, usize> = HashMap::new();
    for (index, hit) in base.iter().enumerate() {
        index_by_key.insert(discover_hit_dedupe_key(hit), index);
    }

    for hit in incoming {
        let key = discover_hit_dedupe_key(&hit);
        if let Some(existing_index) = index_by_key.get(&key).copied() {
            let replace_existing = {
                let existing = &base[existing_index];
                let existing_score = discover_hit_query_score(existing, query);
                let candidate_score = discover_hit_query_score(&hit, query);
                candidate_score > existing_score
                    || (candidate_score == existing_score
                        && (hit.downloads > existing.downloads
                            || hit.date_modified > existing.date_modified))
            };
            if replace_existing {
                base[existing_index] = hit;
            }
            continue;
        }
        index_by_key.insert(key, base.len());
        base.push(hit);
    }
}

fn gather_provider_hits_with_query_variants(
    client: &Client,
    args: &SearchDiscoverContentArgs,
    pool_limit: usize,
    max_variants: usize,
    swallow_base_error: bool,
    provider_search: fn(
        &Client,
        &SearchDiscoverContentArgs,
    ) -> Result<DiscoverSearchResult, String>,
) -> Result<(Vec<DiscoverSearchHit>, usize), String> {
    let requested_pool = pool_limit.max(args.limit).max(20);
    let mut search_args = args.clone();
    search_args.offset = 0;
    search_args.limit = requested_pool;

    let base = match provider_search(client, &search_args) {
        Ok(value) => value,
        Err(_) if swallow_base_error => DiscoverSearchResult {
            hits: vec![],
            offset: 0,
            limit: requested_pool,
            total_hits: 0,
        },
        Err(err) => return Err(err),
    };
    let mut hits = base.hits;
    let mut total_hits = base.total_hits.max(hits.len());
    let query = args.query.trim();

    if max_variants > 0 && !query.is_empty() && hits.len() < requested_pool {
        for variant in discover_query_variants(query)
            .into_iter()
            .take(max_variants)
        {
            let mut variant_args = search_args.clone();
            variant_args.query = variant;
            let extra = provider_search(client, &variant_args).unwrap_or(DiscoverSearchResult {
                hits: vec![],
                offset: 0,
                limit: requested_pool,
                total_hits: 0,
            });
            total_hits = total_hits.max(extra.total_hits);
            merge_discover_hits_in_place(&mut hits, extra.hits, query);
            if hits.len() >= requested_pool {
                break;
            }
        }
    }

    sort_discover_hits(&mut hits, &args.index, Some(query));
    hits.truncate(requested_pool);
    total_hits = total_hits.max(hits.len());
    Ok((hits, total_hits))
}

fn normalized_discover_sources(args: &SearchDiscoverContentArgs) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let raw_values: Vec<String> = if !args.sources.is_empty() {
        args.sources.clone()
    } else {
        vec![args.source.clone().unwrap_or_else(|| "all".to_string())]
    };
    for raw in raw_values {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized == "all" || normalized.is_empty() {
            continue;
        }
        if !DISCOVER_PROVIDER_SOURCES.contains(&normalized.as_str()) {
            continue;
        }
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    if out.is_empty() {
        DISCOVER_PROVIDER_SOURCES
            .iter()
            .map(|value| value.to_string())
            .collect()
    } else {
        out
    }
}

fn search_discover_content_inner(
    args: SearchDiscoverContentArgs,
) -> Result<DiscoverSearchResult, String> {
    let source = args
        .source
        .as_deref()
        .unwrap_or("all")
        .trim()
        .to_lowercase();
    let selected_sources = normalized_discover_sources(&args);
    let explicit_source_subset = discover_has_explicit_source_subset(&selected_sources);
    let normalized_content_type = normalize_discover_content_type(&args.content_type);
    let client = build_http_client()?;

    let pool_limit = args
        .offset
        .saturating_add(args.limit)
        .saturating_add(12)
        .max(args.limit)
        .max(24);

    if selected_sources.len() == 1 && selected_sources[0] == "modrinth" {
        let (hits, total_hits) = gather_provider_hits_with_query_variants(
            &client,
            &args,
            pool_limit,
            5,
            false,
            search_modrinth_discover,
        )?;
        let sliced = hits
            .into_iter()
            .skip(args.offset)
            .take(args.limit)
            .collect::<Vec<_>>();
        return Ok(DiscoverSearchResult {
            hits: sliced,
            offset: args.offset,
            limit: args.limit,
            total_hits,
        });
    }
    if selected_sources.len() == 1 && selected_sources[0] == "curseforge" {
        let (hits, total_hits) = gather_provider_hits_with_query_variants(
            &client,
            &args,
            pool_limit,
            4,
            false,
            search_curseforge_discover,
        )?;
        let sliced = hits
            .into_iter()
            .skip(args.offset)
            .take(args.limit)
            .collect::<Vec<_>>();
        return Ok(DiscoverSearchResult {
            hits: sliced,
            offset: args.offset,
            limit: args.limit,
            total_hits,
        });
    }
    if selected_sources.len() == 1 && selected_sources[0] == "github" {
        let (hits, total_hits) = gather_provider_hits_with_query_variants(
            &client,
            &args,
            pool_limit.max(GITHUB_DISCOVER_MIN_RESULT_POOL),
            github_query_variant_limit(),
            false,
            search_github_discover,
        )?;
        let sliced = hits
            .into_iter()
            .skip(args.offset)
            .take(args.limit)
            .collect::<Vec<_>>();
        return Ok(DiscoverSearchResult {
            hits: sliced,
            offset: args.offset,
            limit: args.limit,
            total_hits,
        });
    }

    let mut sub = args.clone();
    sub.offset = 0;
    sub.limit = pool_limit.max(GITHUB_DISCOVER_MIN_RESULT_POOL / 2);

    let include_modrinth = selected_sources.iter().any(|value| value == "modrinth");
    let include_curseforge = selected_sources.iter().any(|value| value == "curseforge");
    let include_github = selected_sources.iter().any(|value| value == "github");

    let (modrinth_hits, modrinth_total) = if include_modrinth {
        gather_provider_hits_with_query_variants(
            &client,
            &sub,
            sub.limit,
            4,
            true,
            search_modrinth_discover,
        )?
    } else {
        (Vec::new(), 0)
    };
    let (curseforge_hits, curseforge_total) =
        if include_curseforge && curseforge_api_key().is_some() {
            gather_provider_hits_with_query_variants(
                &client,
                &sub,
                sub.limit,
                3,
                true,
                search_curseforge_discover,
            )?
        } else {
            (Vec::new(), 0)
        };

    let mut merged = modrinth_hits;
    merge_discover_hits_in_place(&mut merged, curseforge_hits, args.query.trim());
    let mut github_total = 0usize;
    if include_github && normalized_content_type == "mods" {
        if explicit_source_subset {
            let (github_hits, total) = gather_provider_hits_with_query_variants(
                &client,
                &sub,
                sub.limit.max(GITHUB_DISCOVER_MIN_RESULT_POOL),
                github_query_variant_limit(),
                true,
                search_github_discover,
            )?;
            github_total = total;
            merge_discover_hits_in_place(&mut merged, github_hits, args.query.trim());
        } else if source == "all" && merged.len() < GITHUB_DISCOVER_LOW_HITS_THRESHOLD {
            let github_fallback_hits = search_github_discover_fallback(&client, &sub, &merged);
            if !github_fallback_hits.is_empty() {
                github_total = github_fallback_hits.len();
                merge_discover_hits_in_place(&mut merged, github_fallback_hits, args.query.trim());
            }
        }
    }
    sort_discover_hits(&mut merged, &args.index, Some(args.query.trim()));
    if source == "all" && !explicit_source_subset && normalized_content_type == "mods" {
        merged = blend_discover_hits_prefer_modrinth(merged);
    }
    let mut total_hits = modrinth_total
        .saturating_add(curseforge_total)
        .saturating_add(github_total)
        .max(merged.len());
    if source == "all" && normalized_content_type == "mods" {
        total_hits = total_hits.max(merged.len());
    }
    let hits = merged
        .into_iter()
        .skip(args.offset)
        .take(args.limit)
        .collect::<Vec<_>>();

    Ok(DiscoverSearchResult {
        hits,
        offset: args.offset,
        limit: args.limit,
        total_hits,
    })
}

#[tauri::command]
pub(crate) async fn get_curseforge_project_detail(
    args: GetCurseforgeProjectArgs,
) -> Result<CurseforgeProjectDetail, String> {
    run_blocking_task("curseforge project detail", move || {
        get_curseforge_project_detail_inner(args)
    })
    .await
}

fn get_curseforge_project_detail_inner(
    args: GetCurseforgeProjectArgs,
) -> Result<CurseforgeProjectDetail, String> {
    let api_key = curseforge_api_key().ok_or_else(missing_curseforge_key_message)?;
    let project_id = parse_curseforge_project_id(&args.project_id)?;
    let client = build_http_client()?;

    let mod_resp = client
        .get(format!("{}/mods/{}", CURSEFORGE_API_BASE, project_id))
        .header("Accept", "application/json")
        .header("x-api-key", api_key.clone())
        .send()
        .map_err(|e| format!("CurseForge project lookup failed: {e}"))?;
    if !mod_resp.status().is_success() {
        return Err(format!(
            "CurseForge project lookup failed with status {}",
            mod_resp.status()
        ));
    }
    let project = mod_resp
        .json::<CurseforgeModResponse>()
        .map_err(|e| format!("parse CurseForge project failed: {e}"))?
        .data;
    let detail_content_type =
        infer_curseforge_project_content_type(&project, args.content_type.as_deref());

    let desc_url = format!("{}/mods/{}/description", CURSEFORGE_API_BASE, project_id);
    let description = match client
        .get(&desc_url)
        .header("Accept", "application/json")
        .header("x-api-key", api_key.clone())
        .send()
    {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>() {
            Ok(v) => v
                .get("data")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| project.summary.clone()),
            Err(_) => project.summary.clone(),
        },
        _ => project.summary.clone(),
    };

    let files_resp = client
        .get(format!(
            "{}/mods/{}/files?pageSize=60&index=0",
            CURSEFORGE_API_BASE, project_id
        ))
        .header("Accept", "application/json")
        .header("x-api-key", api_key)
        .send()
        .map_err(|e| format!("CurseForge files lookup failed: {e}"))?;
    if !files_resp.status().is_success() {
        return Err(format!(
            "CurseForge files lookup failed with status {}",
            files_resp.status()
        ));
    }
    let mut files = files_resp
        .json::<CurseforgeFilesResponse>()
        .map_err(|e| format!("parse CurseForge files failed: {e}"))?
        .data;
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    let detail_files = files
        .into_iter()
        .take(40)
        .map(|f| CurseforgeProjectFileDetail {
            file_id: f.id.to_string(),
            display_name: f.display_name,
            file_name: f.file_name,
            file_date: f.file_date,
            game_versions: f.game_versions,
            download_url: f.download_url,
        })
        .collect::<Vec<_>>();

    let project_id_text = project.id.to_string();
    let external_url = Some(curseforge_external_project_url(
        &project_id_text,
        project.slug.as_deref(),
        &detail_content_type,
    ));
    let author_names = project
        .authors
        .into_iter()
        .map(|a| a.name)
        .collect::<Vec<_>>();
    let categories = project
        .categories
        .into_iter()
        .map(|c| c.name)
        .filter(|c| !c.trim().is_empty())
        .collect::<Vec<_>>();

    Ok(CurseforgeProjectDetail {
        source: "curseforge".to_string(),
        project_id: format!("cf:{}", project_id_text),
        title: project.name,
        slug: project.slug,
        summary: project.summary,
        description,
        author_names,
        downloads: project.download_count.max(0.0) as u64,
        categories,
        icon_url: project.logo.map(|l| l.url),
        date_modified: project.date_modified,
        external_url,
        files: detail_files,
    })
}

#[tauri::command]
pub(crate) async fn get_github_project_detail(
    args: GetGithubProjectArgs,
) -> Result<GithubProjectDetail, String> {
    run_blocking_task("github project detail", move || {
        get_github_project_detail_inner(args)
    })
    .await
}

fn get_github_project_detail_inner(
    args: GetGithubProjectArgs,
) -> Result<GithubProjectDetail, String> {
    let project_id = args.project_id.trim();
    if project_id.is_empty() {
        return Err("projectId is required".to_string());
    }
    let (owner, repo_name) = parse_github_project_id(project_id)?;
    let project_key = github_project_key(&owner, &repo_name);
    let default_external_url = format!("https://github.com/{owner}/{repo_name}");
    let default_icon_url = Some(format!("https://github.com/{owner}.png?size=96"));
    let client = build_http_client()?;

    let mut warning_parts: Vec<String> = Vec::new();
    let mut auth_or_rate_limit_warning: Option<String> = None;
    let push_warning = |warning_parts: &mut Vec<String>,
                        auth_or_rate_limit_warning: &mut Option<String>,
                        warning: String| {
        if github_error_is_auth_or_rate_limit(&warning) {
            if auth_or_rate_limit_warning.is_none() {
                *auth_or_rate_limit_warning = Some(warning);
            }
            return;
        }
        if !warning_parts.iter().any(|existing| existing == &warning) {
            warning_parts.push(warning);
        }
    };

    let repo = match fetch_github_repo(&client, &owner, &repo_name) {
        Ok(value) => {
            if let Some(reason) = github_repo_policy_rejection_reason(&value) {
                warning_parts.push(format!("Safety policy warning: {reason}."));
            }
            Some(value)
        }
        Err(err) => {
            push_warning(&mut warning_parts, &mut auth_or_rate_limit_warning, err);
            None
        }
    };

    let mut releases_raw = Vec::<GithubRelease>::new();
    let mut release_list_available = false;
    match fetch_github_releases(&client, &owner, &repo_name) {
        Ok(mut value) => {
            value.sort_by_key(|release| std::cmp::Reverse(github_release_sort_key(release)));
            releases_raw = value;
            release_list_available = true;
        }
        Err(err) => {
            let lower = err.to_ascii_lowercase();
            if !lower.contains("404") {
                push_warning(
                    &mut warning_parts,
                    &mut auth_or_rate_limit_warning,
                    format!("Release list unavailable: {err}"),
                );
            }
        }
    }

    let mut readme_markdown: Option<String> = None;
    let mut readme_html_url: Option<String> = None;
    let mut readme_source_url: Option<String> = None;
    match fetch_github_readme(&client, &owner, &repo_name) {
        Ok(readme) => {
            readme_markdown = decode_github_readme_markdown(&readme)
                .map(|value| value.chars().take(180_000).collect::<String>());
            let download = readme.download_url.trim();
            if !download.is_empty() {
                readme_source_url = Some(download.to_string());
            }
            let html = readme.html_url.trim();
            if !html.is_empty() {
                readme_html_url = Some(html.to_string());
            }
        }
        Err(err) => {
            let lower = err.to_ascii_lowercase();
            if !lower.contains("404") {
                push_warning(
                    &mut warning_parts,
                    &mut auth_or_rate_limit_warning,
                    format!("README unavailable: {err}"),
                );
            }
        }
    }

    let title = repo
        .as_ref()
        .map(github_repo_title)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| repo_name.clone());
    let summary = repo
        .as_ref()
        .and_then(|value| value.description.clone())
        .unwrap_or_else(|| format!("GitHub repository {owner}/{repo_name}"));
    let external_url = repo
        .as_ref()
        .map(github_repo_external_url)
        .unwrap_or_else(|| default_external_url.clone());
    let issues_url = format!("{external_url}/issues");
    let releases_url = format!("{external_url}/releases");
    let icon_url = repo
        .as_ref()
        .and_then(github_owner_avatar_url)
        .or(default_icon_url);
    let date_modified = repo
        .as_ref()
        .and_then(|value| value.pushed_at.clone().or_else(|| value.updated_at.clone()))
        .unwrap_or_else(|| {
            releases_raw
                .iter()
                .find_map(|release| {
                    release
                        .published_at
                        .clone()
                        .or_else(|| release.created_at.clone())
                })
                .unwrap_or_default()
        });
    let compatibility_seed = repo
        .as_ref()
        .map(github_repo_title)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| repo_name.clone());
    let (compatible_release, _) = if let Some(repo_ref) = repo.as_ref() {
        github_select_release_with_repo_hints(
            &client,
            repo_ref,
            &releases_raw,
            &compatibility_seed,
            None,
            None,
            None,
        )
    } else {
        (None, HashSet::new())
    };
    let install_state = Some(
        github_install_state(compatible_release.is_some(), release_list_available).to_string(),
    );
    let install_summary = match install_state.as_deref() {
        Some("unsupported") => Some(
            "This repository does not currently expose an installable GitHub release for this app flow."
                .to_string(),
        ),
        Some("checking") => Some(
            "GitHub install compatibility could not be confirmed right now.".to_string(),
        ),
        _ => None,
    };

    let releases = releases_raw
        .into_iter()
        .filter(|release| !release.draft)
        .take(25)
        .map(|release| {
            let tag_name = if release.tag_name.trim().is_empty() {
                format!("release-{}", release.id)
            } else {
                release.tag_name.trim().to_string()
            };
            let name = release
                .name
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| tag_name.clone());
            let published_at = release
                .published_at
                .clone()
                .or_else(|| release.created_at.clone())
                .unwrap_or_default();
            let assets = release
                .assets
                .iter()
                .take(20)
                .map(|asset| GithubProjectReleaseAssetDetail {
                    name: asset.name.clone(),
                    download_url: asset.browser_download_url.clone(),
                    size: asset.size,
                    content_type: asset.content_type.clone(),
                })
                .collect::<Vec<_>>();
            let external = release
                .html_url
                .trim()
                .to_string()
                .chars()
                .take(4096)
                .collect::<String>();
            GithubProjectReleaseDetail {
                id: format!("gh_release:{}", release.id),
                tag_name,
                name,
                published_at,
                prerelease: release.prerelease,
                draft: release.draft,
                external_url: if external.is_empty() {
                    Some(format!("{releases_url}/tag/{}", release.tag_name.trim()))
                } else {
                    Some(external)
                },
                assets,
            }
        })
        .collect::<Vec<_>>();

    if readme_html_url.is_none() {
        readme_html_url = repo
            .as_ref()
            .and_then(|value| {
                if value.default_branch.trim().is_empty() {
                    None
                } else {
                    Some(format!(
                        "{external_url}/blob/{}/README.md",
                        value.default_branch.trim()
                    ))
                }
            })
            .or_else(|| Some(format!("{external_url}/blob/HEAD/README.md")));
    }

    let homepage_url = repo
        .as_ref()
        .and_then(|value| value.homepage.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(warning) = auth_or_rate_limit_warning {
        warning_parts.insert(0, warning);
    }

    let warning = if warning_parts.is_empty() {
        None
    } else {
        Some(warning_parts.join(" "))
    };

    Ok(GithubProjectDetail {
        source: "github".to_string(),
        project_id: project_key,
        title,
        owner: owner.clone(),
        summary: summary.clone(),
        description: summary,
        stars: repo
            .as_ref()
            .map(|value| value.stargazers_count)
            .unwrap_or(0),
        forks: repo.as_ref().map(|value| value.forks_count).unwrap_or(0),
        watchers: repo.as_ref().map(|value| value.watchers_count).unwrap_or(0),
        open_issues: repo
            .as_ref()
            .map(|value| value.open_issues_count)
            .unwrap_or(0),
        categories: repo
            .as_ref()
            .map(|value| value.topics.clone())
            .unwrap_or_default(),
        icon_url,
        date_modified,
        external_url: Some(external_url),
        releases_url: Some(releases_url),
        issues_url: Some(issues_url),
        homepage_url,
        readme_markdown,
        readme_html_url,
        readme_source_url,
        releases,
        warning,
        install_state,
        install_summary,
    })
}

#[tauri::command]
pub(crate) fn import_provider_modpack_template(
    args: ImportProviderModpackArgs,
) -> Result<CreatorPreset, String> {
    let source = args.source.trim().to_lowercase();
    let client = build_http_client()?;
    if source == "curseforge" {
        let api_key = curseforge_api_key().ok_or_else(missing_curseforge_key_message)?;
        return import_curseforge_modpack_template_inner(
            &client,
            &api_key,
            &args.project_id,
            args.project_title.as_deref(),
        );
    }
    import_modrinth_modpack_template_inner(&client, &args.project_id, args.project_title.as_deref())
}

#[tauri::command]
pub(crate) fn export_presets_json(
    state: tauri::State<AppState>,
    args: ExportPresetsJsonArgs,
) -> Result<PresetsJsonIoResult, String> {
    let items = if let Some(arr) = args.payload.as_array() {
        arr.len()
    } else if let Some(arr) = args.payload.get("presets").and_then(|v| v.as_array()) {
        arr.len()
    } else {
        return Err("Preset payload must be an array or { presets: [] }".to_string());
    };

    let path = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_EXPORT,
        &args.grant_id,
    )?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir export directory failed: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(&args.payload)
        .map_err(|e| format!("serialize presets failed: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("write presets file failed: {e}"))?;

    Ok(PresetsJsonIoResult {
        path: path.display().to_string(),
        items,
    })
}

#[tauri::command]
pub(crate) fn import_presets_json(
    state: tauri::State<AppState>,
    args: ImportPresetsJsonArgs,
) -> Result<serde_json::Value, String> {
    let path = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT,
        &args.grant_id,
    )?;
    let raw = fs::read_to_string(&path).map_err(|e| format!("read presets file failed: {e}"))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse presets file failed: {e}"))?;

    if parsed.is_array() || parsed.get("presets").and_then(|v| v.as_array()).is_some() {
        Ok(parsed)
    } else {
        Err("Preset file must contain an array or { presets: [] }".to_string())
    }
}

#[tauri::command]
pub(crate) async fn get_selected_account_diagnostics(
    app: tauri::AppHandle,
) -> Result<AccountDiagnostics, String> {
    run_blocking_task("account diagnostics", move || {
        get_selected_account_diagnostics_inner(app)
    })
    .await
}

fn get_selected_account_diagnostics_inner(
    app: tauri::AppHandle,
) -> Result<AccountDiagnostics, String> {
    let total_started = Instant::now();
    let settings = read_launcher_settings(&app)?;
    let mut diag = make_account_diagnostics_base(&settings);
    let Some(selected_id) = settings.selected_account_id.clone() else {
        return Ok(diag);
    };

    let mut accounts = read_launcher_accounts(&app)?;
    let Some(mut account) = accounts.iter().find(|a| a.id == selected_id).cloned() else {
        return Ok(fail_account_diag(
            diag,
            "account-not-found",
            "Selected account is missing. Reconnect Microsoft account.".to_string(),
        ));
    };
    diag.account = Some(account.clone());

    let (client_id, source) = match resolve_oauth_client_id_with_source(&app) {
        Ok(v) => v,
        Err(e) => return Ok(fail_account_diag(diag, "oauth-client-id-missing", e)),
    };
    diag.client_id_source = source;

    let refresh = match keyring_get_refresh_token_for_account(&app, &account, &accounts) {
        Ok(v) => v,
        Err(e)
            if e.starts_with("No refresh token found in secure storage")
                || e.starts_with("Multiple secure refresh tokens were found") =>
        {
            let Some(repaired) =
                maybe_repair_selected_account_with_available_token(&app, &account, &accounts)?
            else {
                return Ok(fail_account_diag(diag, "refresh-token-read-failed", e));
            };
            account = repaired;
            diag.account = Some(account.clone());
            match keyring_get_refresh_token_for_account(&app, &account, &accounts) {
                Ok(v) => v,
                Err(err) => return Ok(fail_account_diag(diag, "refresh-token-read-failed", err)),
            }
        }
        Err(e) => return Ok(fail_account_diag(diag, "refresh-token-read-failed", e)),
    };

    let client = match build_http_client() {
        Ok(c) => c,
        Err(e) => {
            return Ok(fail_account_diag(
                diag,
                "http-client-build-failed",
                format!("build http client failed: {e}"),
            ))
        }
    };

    let refresh_started = Instant::now();
    let refreshed = match microsoft_refresh_access_token(&client, &client_id, &refresh) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[account_diag] microsoft-refresh-failed after {}ms",
                refresh_started.elapsed().as_millis()
            );
            return Ok(fail_account_diag(diag, "microsoft-refresh-failed", e));
        }
    };
    let refresh_ms = refresh_started.elapsed().as_millis();
    if refresh_ms > 350 {
        eprintln!("[account_diag] microsoft_refresh_access_token: {refresh_ms}ms");
    }
    if let Some(new_refresh) = refreshed.refresh_token.as_ref() {
        if let Err(e) =
            persist_refresh_token_for_launcher_account_with_app(&app, &account, new_refresh)
        {
            return Ok(fail_account_diag(diag, "refresh-token-write-failed", e));
        }
    }

    let token_exchange_started = Instant::now();
    let mc_access = match microsoft_access_to_mc_token(&client, &refreshed.access_token) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[account_diag] token-exchange-failed after {}ms",
                token_exchange_started.elapsed().as_millis()
            );
            return Ok(fail_account_diag(diag, "token-exchange-failed", e));
        }
    };
    let token_exchange_ms = token_exchange_started.elapsed().as_millis();
    if token_exchange_ms > 350 {
        eprintln!("[account_diag] microsoft_access_to_mc_token: {token_exchange_ms}ms");
    }
    diag.token_exchange_status = "minecraft-token-ok".to_string();

    let entitlements_started = Instant::now();
    if let Err(e) = ensure_minecraft_entitlement(&client, &mc_access) {
        eprintln!(
            "[account_diag] entitlements-check-failed after {}ms",
            entitlements_started.elapsed().as_millis()
        );
        return Ok(fail_account_diag(diag, "entitlements-check-failed", e));
    }
    let entitlements_ms = entitlements_started.elapsed().as_millis();
    if entitlements_ms > 350 {
        eprintln!("[account_diag] ensure_minecraft_entitlement: {entitlements_ms}ms");
    }
    diag.entitlements_ok = true;

    let profile_started = Instant::now();
    let profile = match fetch_minecraft_profile(&client, &mc_access) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[account_diag] profile-fetch-failed after {}ms",
                profile_started.elapsed().as_millis()
            );
            return Ok(fail_account_diag(diag, "profile-fetch-failed", e));
        }
    };
    let profile_ms = profile_started.elapsed().as_millis();
    if profile_ms > 350 {
        eprintln!("[account_diag] fetch_minecraft_profile: {profile_ms}ms");
    }

    diag.minecraft_uuid = Some(profile.id.clone());
    diag.minecraft_username = Some(profile.name.clone());
    diag.skins = summarize_cosmetics(&profile.skins);
    diag.capes = summarize_cosmetics(&profile.capes);
    diag.cape_count = diag.capes.len();
    diag.skin_url = diag
        .skins
        .iter()
        .find(|s| s.state.eq_ignore_ascii_case("active"))
        .map(|s| s.url.clone())
        .or_else(|| diag.skins.first().map(|s| s.url.clone()));

    let mut synced_account = account.clone();
    let token_for_new_id = refreshed.refresh_token.as_ref().unwrap_or(&refresh);
    let mut account_changed = false;
    if synced_account.id != profile.id {
        let old_account_id = synced_account.id.clone();
        synced_account.id = profile.id.clone();
        if let Err(e) = persist_refresh_token_for_launcher_account_with_app(
            &app,
            &synced_account,
            token_for_new_id,
        ) {
            return Ok(fail_account_diag(diag, "refresh-token-write-failed", e));
        }
        accounts.retain(|a| a.id != old_account_id && a.id != synced_account.id);
        account_changed = true;
    }
    if synced_account.username != profile.name {
        synced_account.username = profile.name.clone();
        account_changed = true;
    }
    if account_changed {
        accounts.push(synced_account.clone());
        accounts.sort_by(|a, b| a.username.to_lowercase().cmp(&b.username.to_lowercase()));
        if let Err(e) = write_launcher_accounts(&app, &accounts) {
            return Ok(fail_account_diag(diag, "account-sync-failed", e));
        }
        let mut settings_to_write = settings.clone();
        settings_to_write.selected_account_id = Some(synced_account.id.clone());
        if let Err(e) = write_launcher_settings(&app, &settings_to_write) {
            return Ok(fail_account_diag(diag, "account-sync-failed", e));
        }
        diag.account = Some(synced_account);
    }

    diag.status = "connected".to_string();
    diag.token_exchange_status = "ok".to_string();
    diag.last_error = None;
    let total_ms = total_started.elapsed().as_millis();
    if total_ms > 600 {
        eprintln!("[account_diag] get_selected_account_diagnostics total: {total_ms}ms");
    }
    Ok(diag)
}

#[tauri::command]
pub(crate) async fn apply_selected_account_appearance(
    app: tauri::AppHandle,
    args: ApplySelectedAccountAppearanceArgs,
) -> Result<AccountDiagnostics, String> {
    run_blocking_task("apply selected account appearance", move || {
        apply_selected_account_appearance_inner(app, args)
    })
    .await
}

fn apply_selected_account_appearance_inner(
    app: tauri::AppHandle,
    args: ApplySelectedAccountAppearanceArgs,
) -> Result<AccountDiagnostics, String> {
    if !args.apply_skin && !args.apply_cape {
        return Err("Nothing to apply. Select skin and/or cape first.".to_string());
    }

    let settings = read_launcher_settings(&app)?;
    let client = build_http_client()?;
    let (account, mc_access_token) = build_selected_microsoft_auth(&app, &client, &settings)?;

    if args.apply_skin {
        let source = args
            .skin_source
            .as_deref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "No skin selected to apply.".to_string())?;
        apply_minecraft_skin(
            &client,
            &mc_access_token,
            source,
            args.skin_variant.as_deref(),
        )?;
    }

    if args.apply_cape {
        apply_minecraft_cape(&client, &mc_access_token, args.cape_id.as_deref())?;
    }

    let mut diag = make_account_diagnostics_base(&settings);
    diag.account = Some(account);
    if let Ok((_, source)) = resolve_oauth_client_id_with_source(&app) {
        diag.client_id_source = source;
    }
    diag.status = "connected".to_string();
    diag.token_exchange_status = "ok".to_string();
    diag.entitlements_ok = true;
    diag.last_error = None;

    let profile = fetch_minecraft_profile(&client, &mc_access_token)
        .map_err(|e| format!("post-apply profile refresh failed: {e}"))?;
    diag.minecraft_uuid = Some(profile.id.clone());
    diag.minecraft_username = Some(profile.name.clone());
    diag.skins = summarize_cosmetics(&profile.skins);
    diag.capes = summarize_cosmetics(&profile.capes);
    diag.cape_count = diag.capes.len();
    diag.skin_url = diag
        .skins
        .iter()
        .find(|s| s.state.eq_ignore_ascii_case("active"))
        .map(|s| s.url.clone())
        .or_else(|| diag.skins.first().map(|s| s.url.clone()));

    Ok(diag)
}

#[tauri::command]
pub(crate) async fn export_instance_mods_zip(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: ExportInstanceModsZipArgs,
) -> Result<ExportModsResult, String> {
    let grant_path = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_INSTANCE_MODS_EXPORT,
        &args.grant_id,
    )?;
    run_blocking_task("export instance mods zip", move || {
        export_instance_mods_zip_inner(app, args, grant_path)
    })
    .await
}

fn export_instance_mods_zip_inner(
    app: tauri::AppHandle,
    args: ExportInstanceModsZipArgs,
    output: PathBuf,
) -> Result<ExportModsResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mods_dir = instance_dir.join("mods");
    if !mods_dir.exists() {
        return Err("Instance mods folder does not exist".to_string());
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir export directory failed: {e}"))?;
    }

    let file = File::create(&output).map_err(|e| format!("create zip failed: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut files_count = 0usize;

    let read = fs::read_dir(&mods_dir).map_err(|e| format!("read mods directory failed: {e}"))?;
    for ent in read {
        let ent = ent.map_err(|e| format!("read mods entry failed: {e}"))?;
        let path = ent.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "invalid file name in mods directory".to_string())?;
        let lower = name.to_lowercase();
        if !(lower.ends_with(".jar") || lower.ends_with(".disabled")) {
            continue;
        }
        let mut src = File::open(&path).map_err(|e| format!("open '{}' failed: {e}", name))?;
        zip.start_file(name, options)
            .map_err(|e| format!("zip write header failed: {e}"))?;
        std::io::copy(&mut src, &mut zip)
            .map_err(|e| format!("zip write '{}' failed: {e}", name))?;
        files_count += 1;
    }

    zip.finish()
        .map_err(|e| format!("finalize zip failed: {e}"))?;

    Ok(ExportModsResult {
        output_path: output.display().to_string(),
        files_count,
    })
}

#[tauri::command]
pub(crate) fn list_instances(app: tauri::AppHandle) -> Result<Vec<Instance>, String> {
    let dir = app_instances_dir(&app)?;
    let mut idx = read_index(&dir)?;
    if migrate_instance_folder_names(&dir, &mut idx)? {
        write_index(&dir, &idx)?;
    }
    Ok(idx.instances)
}

fn create_instance_internal(
    app: &tauri::AppHandle,
    clean_name: String,
    clean_mc: String,
    loader_lc: String,
    origin: String,
    icon_path: Option<PathBuf>,
    settings: InstanceSettings,
) -> Result<Instance, String> {
    if clean_name.trim().is_empty() {
        return Err("name is required".to_string());
    }
    if clean_mc.trim().is_empty() {
        return Err("mc_version is required".to_string());
    }
    if parse_loader_for_instance(&loader_lc).is_none() {
        return Err("loader must be one of vanilla/fabric/forge/neoforge/quilt".to_string());
    }

    let dir = app_instances_dir(app)?;
    let mut idx = read_index(&dir)?;
    if migrate_instance_folder_names(&dir, &mut idx)? {
        write_index(&dir, &idx)?;
    }
    let folder_name = allocate_instance_folder_name(&dir, &idx, &clean_name, None, None);

    let mut inst = Instance {
        id: gen_id(),
        name: clean_name,
        origin: normalize_instance_origin(&origin),
        folder_name: Some(folder_name.clone()),
        mc_version: clean_mc,
        loader: loader_lc,
        created_at: now_iso(),
        icon_path: None,
        settings: normalize_instance_settings(settings),
    };

    let inst_dir = dir.join(folder_name);
    fs::create_dir_all(inst_dir.join("mods")).map_err(|e| format!("mkdir mods failed: {e}"))?;
    fs::create_dir_all(inst_dir.join("config")).map_err(|e| format!("mkdir config failed: {e}"))?;
    fs::create_dir_all(inst_dir.join("resourcepacks"))
        .map_err(|e| format!("mkdir resourcepacks failed: {e}"))?;
    fs::create_dir_all(inst_dir.join("shaderpacks"))
        .map_err(|e| format!("mkdir shaderpacks failed: {e}"))?;
    fs::create_dir_all(inst_dir.join("saves")).map_err(|e| format!("mkdir saves failed: {e}"))?;

    if let Some(icon_source) = icon_path {
        inst.icon_path = Some(copy_instance_icon_to_dir(&icon_source, &inst_dir)?);
    }

    write_instance_meta(&inst_dir, &inst)?;
    idx.instances.insert(0, inst.clone());
    write_index(&dir, &idx)?;
    write_lockfile(&dir, &inst.id, &Lockfile::default())?;

    Ok(inst)
}

#[tauri::command]
pub(crate) fn create_instance(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: CreateInstanceArgs,
) -> Result<Instance, String> {
    let loader_lc = parse_loader_for_instance(&args.loader)
        .ok_or_else(|| "loader must be one of vanilla/fabric/forge/neoforge/quilt".to_string())?;

    let clean_name = sanitize_name(&args.name);
    if clean_name.is_empty() {
        return Err("name is required".into());
    }
    let clean_mc = args.mc_version.trim().to_string();
    if clean_mc.is_empty() {
        return Err("mc_version is required".into());
    }
    let mut settings = InstanceSettings::default();
    settings.loader_version_strategy = normalize_loader_version_strategy(
        args.loader_version_strategy.as_deref().unwrap_or("stable"),
    );
    settings.custom_loader_version = args.custom_loader_version.unwrap_or_default().trim().to_string();
    let icon_path = args
        .icon_grant_id
        .as_deref()
        .map(|grant_id| consume_external_path_grant(&state, EXTERNAL_PATH_PURPOSE_INSTANCE_ICON, grant_id))
        .transpose()?;
    create_instance_internal(
        &app,
        clean_name,
        clean_mc,
        loader_lc,
        "custom".to_string(),
        icon_path,
        settings,
    )
}

#[tauri::command]
pub(crate) fn create_instance_from_modpack_file(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: CreateInstanceFromModpackFileArgs,
) -> Result<CreateInstanceFromModpackFileResult, String> {
    let file_path = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_MODPACK_ARCHIVE_IMPORT,
        &args.grant_id,
    )?;
    let (default_name, mc_version, loader, override_roots, mut warnings) =
        parse_modpack_file_info(&file_path)?;
    let final_name = sanitize_name(args.name.as_deref().unwrap_or(&default_name));
    if final_name.trim().is_empty() {
        return Err("Imported modpack name is empty.".to_string());
    }
    let icon_path = args
        .icon_grant_id
        .as_deref()
        .map(|grant_id| consume_external_path_grant(&state, EXTERNAL_PATH_PURPOSE_INSTANCE_ICON, grant_id))
        .transpose()?;
    let instance = create_instance_internal(
        &app,
        final_name,
        mc_version,
        loader,
        "downloaded".to_string(),
        icon_path,
        InstanceSettings::default(),
    )?;
    let instances_dir = app_instances_dir(&app)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let imported_files =
        extract_overrides_from_modpack(&file_path, &instance_dir, &override_roots)?;
    if imported_files == 0 {
        warnings.push("No override files were found in the archive.".to_string());
    }
    Ok(CreateInstanceFromModpackFileResult {
        instance,
        imported_files,
        warnings,
    })
}

#[tauri::command]
pub(crate) fn list_launcher_import_sources() -> Result<Vec<LauncherImportSource>, String> {
    Ok(list_launcher_import_sources_inner())
}

#[tauri::command]
pub(crate) fn import_instance_from_launcher(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: ImportInstanceFromLauncherArgs,
) -> Result<ImportInstanceFromLauncherResult, String> {
    let source = list_launcher_import_sources_inner()
        .into_iter()
        .find(|s| s.id == args.source_id)
        .ok_or_else(|| "Selected launcher source was not found.".to_string())?;
    let source_path = PathBuf::from(source.source_path.trim());
    if !source_path.exists() || !source_path.is_dir() {
        return Err("Source launcher directory is unavailable.".to_string());
    }
    let fallback_name = format!("{} import", source.label);
    let final_name = sanitize_name(args.name.as_deref().unwrap_or(&fallback_name));
    if final_name.trim().is_empty() {
        return Err("Imported instance name is required.".to_string());
    }
    let loader = parse_loader_for_instance(&source.loader).unwrap_or_else(|| "vanilla".to_string());
    let icon_path = args
        .icon_grant_id
        .as_deref()
        .map(|grant_id| consume_external_path_grant(&state, EXTERNAL_PATH_PURPOSE_INSTANCE_ICON, grant_id))
        .transpose()?;
    let instance = create_instance_internal(
        &app,
        final_name,
        source.mc_version.clone(),
        loader,
        "downloaded".to_string(),
        icon_path,
        InstanceSettings::default(),
    )?;
    let instances_dir = app_instances_dir(&app)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let imported_files = copy_launcher_source_into_instance(&source_path, &instance_dir)?;
    Ok(ImportInstanceFromLauncherResult {
        instance,
        imported_files,
    })
}

#[tauri::command]
pub(crate) fn update_instance(
    app: tauri::AppHandle,
    args: UpdateInstanceArgs,
) -> Result<Instance, String> {
    let dir = app_instances_dir(&app)?;
    let mut idx = read_index(&dir)?;
    if migrate_instance_folder_names(&dir, &mut idx)? {
        write_index(&dir, &idx)?;
    }
    let pos = idx
        .instances
        .iter()
        .position(|x| x.id == args.instance_id)
        .ok_or_else(|| "instance not found".to_string())?;
    let prev_inst = idx.instances[pos].clone();
    let mut inst = prev_inst.clone();
    let prev_dir = instance_dir_for_instance(&dir, &inst);
    let mut folder_name_override: Option<String> = None;

    if let Some(name) = args.name.as_ref() {
        let clean_name = sanitize_name(name);
        if clean_name.is_empty() {
            return Err("name is required".to_string());
        }
        inst.name = clean_name;
        let next_folder = allocate_instance_folder_name(
            &dir,
            &idx,
            &inst.name,
            Some(&inst.id),
            inst.folder_name.as_deref(),
        );
        folder_name_override = Some(next_folder);
    }
    if let Some(mc_version) = args.mc_version.as_ref() {
        let clean_mc = mc_version.trim().to_string();
        if clean_mc.is_empty() {
            return Err("mc_version is required".to_string());
        }
        inst.mc_version = clean_mc;
    }
    if let Some(loader) = args.loader.as_ref() {
        let parsed = parse_loader_for_instance(loader).ok_or_else(|| {
            "loader must be one of vanilla/fabric/forge/neoforge/quilt".to_string()
        })?;
        inst.loader = parsed;
    }
    if let Some(settings) = args.settings {
        inst.settings = normalize_instance_settings(settings);
    } else {
        inst.settings = normalize_instance_settings(inst.settings);
    }

    if let Some(next_folder) = folder_name_override {
        inst.folder_name = Some(next_folder);
    } else if inst
        .folder_name
        .as_ref()
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
    {
        inst.folder_name = Some(inst.id.clone());
    }

    let inst_dir = instance_dir_for_instance(&dir, &inst);
    if prev_dir != inst_dir && prev_dir.exists() && !inst_dir.exists() {
        fs::rename(&prev_dir, &inst_dir).map_err(|e| {
            format!(
                "rename instance folder failed ({} -> {}): {e}",
                prev_dir.display(),
                inst_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&inst_dir).map_err(|e| format!("mkdir instance dir failed: {e}"))?;
    write_instance_meta(&inst_dir, &inst)?;
    idx.instances[pos] = inst.clone();
    write_index(&dir, &idx)?;

    if prev_inst.name != inst.name
        || prev_inst.mc_version != inst.mc_version
        || prev_inst.loader != inst.loader
    {
        let mut changes: Vec<String> = Vec::new();
        if prev_inst.name != inst.name {
            changes.push(format!("name '{}' -> '{}'", prev_inst.name, inst.name));
        }
        if prev_inst.mc_version != inst.mc_version {
            changes.push(format!(
                "Minecraft {} -> {}",
                prev_inst.mc_version, inst.mc_version
            ));
        }
        if prev_inst.loader != inst.loader {
            changes.push(format!("loader {} -> {}", prev_inst.loader, inst.loader));
        }
        log_instance_event_best_effort(
            &app,
            &inst.id,
            "instance_updated",
            format!("Updated instance profile: {}.", changes.join(", ")),
        );
    }

    let java_changed = prev_inst.settings.java_path.trim() != inst.settings.java_path.trim();
    if java_changed {
        let previous = if prev_inst.settings.java_path.trim().is_empty() {
            "default".to_string()
        } else {
            prev_inst.settings.java_path.trim().to_string()
        };
        let next = if inst.settings.java_path.trim().is_empty() {
            "default".to_string()
        } else {
            inst.settings.java_path.trim().to_string()
        };
        log_instance_event_best_effort(
            &app,
            &inst.id,
            "java_changed",
            format!("Java runtime changed: {} -> {}.", previous, next),
        );
    }

    let mut settings_changes: Vec<&str> = Vec::new();
    if prev_inst.settings.keep_launcher_open_while_playing
        != inst.settings.keep_launcher_open_while_playing
    {
        settings_changes.push("keep-launcher-open");
    }
    if prev_inst.settings.close_launcher_on_game_exit != inst.settings.close_launcher_on_game_exit {
        settings_changes.push("close-on-exit");
    }
    if prev_inst.settings.sync_minecraft_settings != inst.settings.sync_minecraft_settings
        || prev_inst.settings.sync_minecraft_settings_target
            != inst.settings.sync_minecraft_settings_target
    {
        settings_changes.push("settings-sync");
    }
    if prev_inst.settings.auto_update_installed_content
        != inst.settings.auto_update_installed_content
        || prev_inst.settings.prefer_release_builds != inst.settings.prefer_release_builds
    {
        settings_changes.push("update-policy");
    }
    if prev_inst.settings.memory_mb != inst.settings.memory_mb
        || prev_inst.settings.jvm_args != inst.settings.jvm_args
    {
        settings_changes.push("java-runtime-flags");
    }
    if prev_inst.settings.notes != inst.settings.notes {
        settings_changes.push("notes");
    }
    if prev_inst.settings.graphics_preset != inst.settings.graphics_preset
        || prev_inst.settings.enable_shaders != inst.settings.enable_shaders
        || prev_inst.settings.force_vsync != inst.settings.force_vsync
    {
        settings_changes.push("display");
    }
    if prev_inst.settings.world_backup_interval_minutes
        != inst.settings.world_backup_interval_minutes
        || prev_inst.settings.world_backup_retention_count
            != inst.settings.world_backup_retention_count
        || prev_inst.settings.snapshot_retention_count != inst.settings.snapshot_retention_count
        || prev_inst.settings.snapshot_max_age_days != inst.settings.snapshot_max_age_days
    {
        settings_changes.push("retention");
    }
    if !settings_changes.is_empty() {
        log_instance_event_best_effort(
            &app,
            &inst.id,
            "settings_changed",
            format!(
                "Updated instance settings: {}.",
                settings_changes.join(", ")
            ),
        );
    }

    Ok(inst)
}

#[tauri::command]
pub(crate) fn detect_java_runtimes() -> Result<Vec<JavaRuntimeCandidate>, String> {
    Ok(detect_java_runtimes_inner())
}

#[tauri::command]
pub(crate) fn set_instance_icon(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: SetInstanceIconArgs,
) -> Result<Instance, String> {
    let dir = app_instances_dir(&app)?;
    let mut idx = read_index(&dir)?;
    let pos = idx
        .instances
        .iter()
        .position(|x| x.id == args.instance_id)
        .ok_or_else(|| "instance not found".to_string())?;

    let mut inst = idx.instances[pos].clone();
    let inst_dir = instance_dir_for_instance(&dir, &inst);
    fs::create_dir_all(&inst_dir).map_err(|e| format!("mkdir instance dir failed: {e}"))?;

    let next_icon_path = args
        .icon_grant_id
        .as_deref()
        .map(|grant_id| consume_external_path_grant(&state, EXTERNAL_PATH_PURPOSE_INSTANCE_ICON, grant_id))
        .transpose()?;
    inst.icon_path = if let Some(path) = next_icon_path {
        Some(copy_instance_icon_to_dir(&path, &inst_dir)?)
    } else {
        clear_instance_icon_files(&inst_dir)?;
        None
    };

    write_instance_meta(&inst_dir, &inst)?;
    idx.instances[pos] = inst.clone();
    write_index(&dir, &idx)?;
    Ok(inst)
}

#[tauri::command]
pub(crate) fn read_local_image_data_url(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: ReadLocalImageDataUrlArgs,
) -> Result<String, String> {
    let path = if let Some(grant_id) = args
        .grant_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        consume_external_path_grant(&state, EXTERNAL_PATH_PURPOSE_INSTANCE_ICON, grant_id)?
    } else if let Some(path) = args
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        resolve_renderer_allowed_instance_icon_path(&app, Path::new(path))?
    } else {
        return Err("path is required".to_string());
    };
    local_image_data_url_for_path(&path)
}

#[tauri::command]
pub(crate) fn delete_instance(
    app: tauri::AppHandle,
    args: DeleteInstanceArgs,
) -> Result<(), String> {
    let dir = app_instances_dir(&app)?;
    let mut idx = read_index(&dir)?;
    if migrate_instance_folder_names(&dir, &mut idx)? {
        write_index(&dir, &idx)?;
    }

    let target = idx
        .instances
        .iter()
        .find(|x| x.id == args.id)
        .cloned()
        .ok_or_else(|| "instance not found".to_string())?;

    let before = idx.instances.len();
    idx.instances.retain(|x| x.id != args.id);
    if idx.instances.len() == before {
        return Err("instance not found".into());
    }

    let inst_dir = instance_dir_for_instance(&dir, &target);
    if inst_dir.exists() {
        fs::remove_dir_all(inst_dir).map_err(|e| format!("remove dir failed: {e}"))?;
    }

    write_index(&dir, &idx)?;
    Ok(())
}

fn install_modrinth_mod_inner(
    app: tauri::AppHandle,
    args: InstallModrinthModArgs,
    snapshot_reason: Option<&str>,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let mods_dir = instance_dir.join("mods");
    fs::create_dir_all(&mods_dir).map_err(|e| format!("mkdir mods failed: {e}"))?;

    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "resolving".into(),
            downloaded: 0,
            total: None,
            percent: Some(1.0),
            message: Some("Resolving compatible versions and required dependencies…".into()),
        },
    );

    let client = build_http_client()?;

    let plan = resolve_modrinth_install_plan(&client, &instance, &args.project_id)?;
    let total_mods = plan.len();
    let dependency_mods = total_mods.saturating_sub(1);
    let total_actions = count_plan_install_actions(&instance_dir, &lock, &plan);

    if total_actions > 0 {
        if let Some(reason) = snapshot_reason {
            emit_install_progress(
                &app,
                InstallProgressEvent {
                    instance_id: args.instance_id.clone(),
                    project_id: args.project_id.clone(),
                    stage: "snapshotting".into(),
                    downloaded: 0,
                    total: None,
                    percent: None,
                    message: Some("Preparing install…".into()),
                },
            );
            create_preinstall_snapshot_with_event_best_effort(
                &app,
                &instances_dir,
                &args.instance_id,
                reason,
            );
            emit_install_progress(
                &app,
                InstallProgressEvent {
                    instance_id: args.instance_id.clone(),
                    project_id: args.project_id.clone(),
                    stage: "resolving".into(),
                    downloaded: 0,
                    total: Some(total_actions as u64),
                    percent: Some(if total_actions == 0 { 100.0 } else { 2.0 }),
                    message: Some("Preparing install…".into()),
                },
            );
        }
    }

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "resolving".into(),
            downloaded: 0,
            total: Some(total_actions as u64),
            percent: Some(if total_actions == 0 { 100.0 } else { 2.0 }),
            message: Some(format!(
                "Install plan ready: {} mod(s) total ({} required dependencies)",
                total_mods, dependency_mods
            )),
        },
    );

    let mut root_installed: Option<InstalledMod> = None;
    let mut completed_actions: usize = 0;
    let mut removed_local_conflicts: Vec<String> = Vec::new();
    let mut lock_changed = false;

    for item in plan {
        let safe_filename =
            safe_mod_filename(&item.project_id, &item.version.id, &item.file.filename);

        if is_plan_entry_up_to_date(&instance_dir, &lock, &item) {
            if item.project_id == args.project_id {
                if let Some(existing) = lock
                    .entries
                    .iter()
                    .find(|e| e.project_id == args.project_id)
                {
                    root_installed = Some(lock_entry_to_installed(&instance_dir, existing));
                }
            }
            continue;
        }

        let final_path = mods_dir.join(&safe_filename);
        let tmp_path = mods_dir.join(format!("{safe_filename}.part"));
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "downloading".into(),
                downloaded: completed_actions as u64,
                total: Some(total_actions as u64),
                percent: Some(if total_actions == 0 {
                    100.0
                } else {
                    let base = (completed_actions as f64 / total_actions as f64) * 100.0;
                    if completed_actions == 0 {
                        base.max(0.2)
                    } else {
                        base
                    }
                }),
                message: Some(format!("Installing {} ({safe_filename})", item.project_id)),
            },
        );
        let mut stream_result = download_stream_to_temp_with_retry(
            &client,
            &item.file.url,
            &item.project_id,
            &tmp_path,
            |downloaded_bytes, total_bytes| {
                let ratio = match total_bytes {
                    Some(total) if total > 0 => downloaded_bytes as f64 / total as f64,
                    _ => unknown_progress_ratio(downloaded_bytes),
                };
                let overall = if total_actions == 0 {
                    100.0
                } else {
                    ((completed_actions as f64 + ratio) / total_actions as f64) * 100.0
                };
                let visible_overall = overall.max(0.2);
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "downloading".into(),
                        downloaded: downloaded_bytes,
                        total: total_bytes,
                        percent: Some(visible_overall.clamp(0.0, 99.4)),
                        message: Some(format!(
                            "Installing {} ({safe_filename}) · {}",
                            item.project_id,
                            format_download_meter(downloaded_bytes, total_bytes)
                        )),
                    },
                );
            },
        )?;

        if final_path.exists() {
            fs::remove_file(&final_path).map_err(|e| format!("remove old mod file failed: {e}"))?;
        }
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "installing".into(),
                downloaded: completed_actions as u64,
                total: Some(total_actions as u64),
                percent: None,
                message: Some(format!("Installing {} into the instance…", safe_filename)),
            },
        );
        let post_process_started = Instant::now();
        fs::rename(&tmp_path, &final_path).map_err(|e| format!("move mod file failed: {e}"))?;
        stream_result.profile.post_process_ms = post_process_started.elapsed().as_millis();
        maybe_log_download_profile(&item.project_id, &stream_result.profile);

        remove_replaced_entries_for_project(
            &mut lock,
            &instance_dir,
            &item.project_id,
            Some(&safe_filename),
        )?;
        let removed = remove_conflicting_local_mod_entries_for_filename(
            &mut lock,
            &instance_dir,
            &safe_filename,
        )?;
        if !removed.is_empty() {
            removed_local_conflicts.extend(removed);
        }

        let fallback_name = item
            .version
            .name
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| item.project_id.clone());
        let resolved_name = if item.project_id == args.project_id {
            if let Some(title) = args.project_title.as_ref() {
                let clean = title.trim();
                if clean.is_empty() {
                    fetch_project_title(&client, &item.project_id).unwrap_or(fallback_name)
                } else {
                    clean.to_string()
                }
            } else {
                fetch_project_title(&client, &item.project_id).unwrap_or(fallback_name)
            }
        } else {
            fallback_name
        };

        let new_entry = LockEntry {
            source: "modrinth".into(),
            project_id: item.project_id.clone(),
            version_id: item.version.id.clone(),
            name: canonical_lock_entry_name("mods", &safe_filename, &resolved_name),
            version_number: item.version.version_number.clone(),
            filename: safe_filename,
            content_type: "mods".to_string(),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            pinned_version: None,
            enabled: true,
            hashes: {
                let mut hashes = item.file.hashes.clone();
                if !stream_result.sha512.trim().is_empty() {
                    hashes
                        .entry("sha512".to_string())
                        .or_insert_with(|| stream_result.sha512.clone());
                }
                hashes
            },
            provider_candidates: vec![ProviderCandidate {
                source: "modrinth".to_string(),
                project_id: item.project_id.clone(),
                version_id: item.version.id.clone(),
                name: resolved_name.clone(),
                version_number: item.version.version_number.clone(),
                confidence: None,
                reason: None,
                verification_status: None,
            }],
            local_analysis: None,
        };

        lock.entries.push(new_entry.clone());
        lock_changed = true;

        if item.project_id == args.project_id {
            root_installed = Some(lock_entry_to_installed(&instance_dir, &new_entry));
        }

        completed_actions += 1;
    }

    if root_installed.is_none() {
        if let Some(root_entry) = lock
            .entries
            .iter()
            .find(|e| e.project_id == args.project_id)
        {
            root_installed = Some(lock_entry_to_installed(&instance_dir, root_entry));
        }
    }

    let root_installed =
        root_installed.ok_or_else(|| "Root mod was not installed in lockfile".to_string())?;

    if lock_changed {
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "finalizing".into(),
                downloaded: completed_actions as u64,
                total: Some(total_actions as u64),
                percent: None,
                message: Some("Finishing install…".into()),
            },
        );
        lock.entries
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        write_lockfile(&instances_dir, &args.instance_id, &lock)?;
    }

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "completed".into(),
            downloaded: completed_actions as u64,
            total: Some(total_actions as u64),
            percent: Some(100.0),
            message: Some(format!(
                "Installed {} mod(s) ({} dependency mods)",
                total_mods, dependency_mods
            )),
        },
    );
    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "mod_install",
        format!(
            "Installed/updated mod '{}' ({} dependencies).",
            root_installed.name, dependency_mods
        ),
    );
    if !removed_local_conflicts.is_empty() {
        removed_local_conflicts.sort_by_key(|value| value.to_ascii_lowercase());
        removed_local_conflicts.dedup();
        let preview = removed_local_conflicts
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>();
        log_instance_event_best_effort(
            &app,
            &args.instance_id,
            "mod_install_cleanup",
            format!(
                "Removed {} conflicting local mod entr{} after provider install: {}.",
                removed_local_conflicts.len(),
                if removed_local_conflicts.len() == 1 {
                    "y"
                } else {
                    "ies"
                },
                preview.join(", ")
            ),
        );
    }

    Ok(root_installed)
}

#[tauri::command]
pub(crate) async fn install_modrinth_mod(
    app: tauri::AppHandle,
    args: InstallModrinthModArgs,
) -> Result<InstalledMod, String> {
    run_blocking_task("install modrinth mod", move || {
        let mutation_lock = instance_mutation_lock(&args.instance_id);
        let _guard = mutation_lock
            .lock()
            .map_err(|_| "instance mutation lock poisoned".to_string())?;
        let subject = snapshot_install_subject(args.project_title.as_deref(), &args.project_id);
        let reason = format!("before-install-modrinth:{subject}");
        install_modrinth_mod_inner(app, args, Some(reason.as_str()))
    })
    .await
}

#[tauri::command]
pub(crate) async fn install_curseforge_mod(
    app: tauri::AppHandle,
    args: InstallCurseforgeModArgs,
) -> Result<InstalledMod, String> {
    run_blocking_task("install curseforge mod", move || {
        let mutation_lock = instance_mutation_lock(&args.instance_id);
        let _guard = mutation_lock
            .lock()
            .map_err(|_| "instance mutation lock poisoned".to_string())?;
        let subject = snapshot_install_subject(args.project_title.as_deref(), &args.project_id);
        let reason = format!("before-install-curseforge:{subject}");
        install_curseforge_mod_inner(app, args, Some(reason.as_str()))
    })
    .await
}

fn install_curseforge_mod_inner(
    app: tauri::AppHandle,
    args: InstallCurseforgeModArgs,
    snapshot_reason: Option<&str>,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let api_key = curseforge_api_key().ok_or_else(missing_curseforge_key_message)?;
    let client = build_http_client()?;
    let root_mod_id = parse_curseforge_project_id(&args.project_id)?;
    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "resolving".to_string(),
            downloaded: 0,
            total: None,
            percent: Some(1.0),
            message: Some("Resolving CurseForge metadata and dependency chain…".to_string()),
        },
    );

    let install_plan = resolve_curseforge_dependency_chain(
        &client,
        &api_key,
        &instance,
        root_mod_id,
        |resolved_count, pending_count| {
            let denom = (resolved_count + pending_count).max(1) as f64;
            let ratio = resolved_count as f64 / denom;
            let percent = (1.0 + ratio * 28.0).clamp(1.0, 34.0);
            let detail = if pending_count > 0 {
                format!(
                    "Resolved {} project(s), {} pending…",
                    resolved_count, pending_count
                )
            } else {
                format!(
                    "Resolved {} project(s), preparing downloads…",
                    resolved_count
                )
            };
            emit_install_progress(
                &app,
                InstallProgressEvent {
                    instance_id: args.instance_id.clone(),
                    project_id: args.project_id.clone(),
                    stage: "resolving".to_string(),
                    downloaded: resolved_count as u64,
                    total: Some((resolved_count + pending_count) as u64),
                    percent: Some(percent),
                    message: Some(format!("Resolving CurseForge metadata… {detail}")),
                },
            );
        },
    )?;
    let total_actions = install_plan.len().max(1);

    if let Some(reason) = snapshot_reason {
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "snapshotting".to_string(),
                downloaded: 0,
                total: None,
                percent: None,
                message: Some("Preparing install…".to_string()),
            },
        );
        create_preinstall_snapshot_with_event_best_effort(
            &app,
            &instances_dir,
            &args.instance_id,
            reason,
        );
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "resolving".to_string(),
                downloaded: 0,
                total: Some(total_actions as u64),
                percent: Some(34.0),
                message: Some("Preparing install…".to_string()),
            },
        );
    }
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let mut root_entry: Option<LockEntry> = None;
    let mut removed_local_conflicts: Vec<String> = Vec::new();
    for (idx, plan_item) in install_plan.iter().enumerate() {
        let is_root = plan_item.mod_id == root_mod_id;
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "downloading".to_string(),
                downloaded: idx as u64,
                total: Some(total_actions as u64),
                percent: Some(
                    if idx == 0 {
                        0.2
                    } else {
                        (idx as f64) / (total_actions as f64) * 100.0
                    }
                    .clamp(0.0, 99.0),
                ),
                message: Some(if is_root {
                    "Downloading selected CurseForge mod…".to_string()
                } else {
                    format!(
                        "Downloading required dependency {}/{}…",
                        idx + 1,
                        total_actions
                    )
                }),
            },
        );

        let entry = install_curseforge_content_inner(
            &instance,
            &instance_dir,
            &mut lock,
            &client,
            &api_key,
            &plan_item.mod_id.to_string(),
            if is_root {
                args.project_title.as_deref()
            } else {
                None
            },
            "mods",
            &[],
            Some(&plan_item.file),
            |downloaded_bytes, total_bytes| {
                let ratio = match total_bytes {
                    Some(total) if total > 0 => downloaded_bytes as f64 / total as f64,
                    _ => unknown_progress_ratio(downloaded_bytes),
                };
                let overall = ((idx as f64 + ratio) / total_actions as f64) * 100.0;
                let visible_overall = overall.max(0.2);
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "downloading".to_string(),
                        downloaded: downloaded_bytes,
                        total: total_bytes,
                        percent: Some(visible_overall.clamp(0.0, 99.4)),
                        message: Some(if is_root {
                            format!(
                                "Downloading selected CurseForge mod… · {}",
                                format_download_meter(downloaded_bytes, total_bytes)
                            )
                        } else {
                            format!(
                                "Downloading required dependency {}/{}… · {}",
                                idx + 1,
                                total_actions,
                                format_download_meter(downloaded_bytes, total_bytes)
                            )
                        }),
                    },
                );
            },
        )?;
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "installing".to_string(),
                downloaded: idx as u64,
                total: Some(total_actions as u64),
                percent: None,
                message: Some(if is_root {
                    format!("Installing {} into the instance…", entry.filename)
                } else {
                    format!(
                        "Installing dependency {}/{} into the instance…",
                        idx + 1,
                        total_actions
                    )
                }),
            },
        );
        let removed = remove_conflicting_local_mod_entries_for_filename(
            &mut lock,
            &instance_dir,
            &entry.filename,
        )?;
        if !removed.is_empty() {
            removed_local_conflicts.extend(removed);
        }
        if is_root {
            root_entry = Some(entry);
        }
    }

    lock.entries
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "finalizing".to_string(),
            downloaded: total_actions as u64,
            total: Some(total_actions as u64),
            percent: None,
            message: Some("Finishing install…".to_string()),
        },
    );
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;

    let entry =
        root_entry.ok_or_else(|| "Failed to resolve selected CurseForge project".to_string())?;

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id,
            stage: "completed".to_string(),
            downloaded: total_actions as u64,
            total: Some(total_actions as u64),
            percent: Some(100.0),
            message: Some(if total_actions > 1 {
                format!(
                    "CurseForge install complete ({} required dependencies)",
                    total_actions.saturating_sub(1)
                )
            } else {
                "CurseForge install complete".to_string()
            }),
        },
    );
    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "mod_install",
        format!(
            "Installed/updated CurseForge mod '{}' ({} dependencies).",
            entry.name,
            total_actions.saturating_sub(1)
        ),
    );
    if !removed_local_conflicts.is_empty() {
        removed_local_conflicts.sort_by_key(|value| value.to_ascii_lowercase());
        removed_local_conflicts.dedup();
        let preview = removed_local_conflicts
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>();
        log_instance_event_best_effort(
            &app,
            &args.instance_id,
            "mod_install_cleanup",
            format!(
                "Removed {} conflicting local mod entr{} after provider install: {}.",
                removed_local_conflicts.len(),
                if removed_local_conflicts.len() == 1 {
                    "y"
                } else {
                    "ies"
                },
                preview.join(", ")
            ),
        );
    }

    Ok(lock_entry_to_installed(&instance_dir, &entry))
}

#[tauri::command]
pub(crate) async fn preview_modrinth_install(
    app: tauri::AppHandle,
    args: InstallModrinthModArgs,
) -> Result<InstallPlanPreview, String> {
    run_blocking_task("preview modrinth install", move || {
        preview_modrinth_install_inner(app, args)
    })
    .await
}

fn preview_modrinth_install_inner(
    app: tauri::AppHandle,
    args: InstallModrinthModArgs,
) -> Result<InstallPlanPreview, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;

    let client = build_http_client()?;

    let plan = resolve_modrinth_install_plan(&client, &instance, &args.project_id)?;
    let total_mods = plan.len();
    let dependency_mods = total_mods.saturating_sub(1);
    let will_install_mods = count_plan_install_actions(&instance_dir, &lock, &plan);

    Ok(InstallPlanPreview {
        total_mods,
        dependency_mods,
        will_install_mods,
    })
}

#[tauri::command]
pub(crate) async fn import_local_mod_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: ImportLocalModFileArgs,
) -> Result<InstalledMod, String> {
    let source_path = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_LOCAL_MOD_IMPORT,
        &args.grant_id,
    )?;
    run_blocking_task("import local mod file", move || {
        import_local_mod_file_inner(app, args, source_path)
    })
    .await
}

fn import_local_mod_file_inner(
    app: tauri::AppHandle,
    args: ImportLocalModFileArgs,
    source_path: PathBuf,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let normalized_content_type =
        normalize_lock_content_type(args.content_type.as_deref().unwrap_or("mods"));
    if !is_supported_local_content_type(&normalized_content_type) {
        return Err("Unsupported content type for local import".to_string());
    }

    if !source_path.exists() || !source_path.is_file() {
        return Err("Selected file does not exist".into());
    }
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !local_file_extension_allowed(&normalized_content_type, &ext) {
        return Err(format!(
            "Only {} files are supported for local {} import",
            local_file_extension_hint(&normalized_content_type),
            content_type_display_name(&normalized_content_type)
        ));
    }

    let source_name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid file name")?;
    let safe_filename = sanitize_filename(source_name);
    if safe_filename.is_empty() {
        return Err("Invalid file name".into());
    }

    let file_bytes = fs::read(&source_path).map_err(|e| format!("read file failed: {e}"))?;
    if normalized_content_type == "mods" {
        ensure_local_mod_loader_compatible(&instance, &safe_filename, &file_bytes)?;
        let mods_dir = instance_dir.join("mods");
        fs::create_dir_all(&mods_dir).map_err(|e| format!("mkdir mods failed: {e}"))?;
        let disabled_path = mods_dir.join(format!("{safe_filename}.disabled"));
        if disabled_path.exists() {
            fs::remove_file(&disabled_path)
                .map_err(|e| format!("cleanup disabled mod failed: {e}"))?;
        }
    }

    let worlds = if normalized_content_type == "datapacks" {
        let requested = if let Some(target_worlds) =
            args.target_worlds.clone().filter(|list| !list.is_empty())
        {
            target_worlds
        } else {
            list_instance_world_names(&instance_dir)?
        };
        normalize_target_worlds_for_datapack(&instance_dir, &requested)?
    } else {
        vec![]
    };
    write_download_to_content_targets(
        &instance_dir,
        &normalized_content_type,
        &safe_filename,
        &worlds,
        &file_bytes,
    )?;

    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let previous_pin = lock
        .entries
        .iter()
        .find(|entry| {
            entry.filename == safe_filename
                && normalize_lock_content_type(&entry.content_type) == normalized_content_type
        })
        .and_then(|entry| entry.pinned_version.clone());
    let known_mod_ids =
        collect_known_enabled_mod_ids_for_dependency_checks(&lock, &instance_dir, &instance.loader);
    let local_analysis = if normalized_content_type == "mods" {
        Some(analyze_local_mod_file(
            &safe_filename,
            &file_bytes,
            Some(&instance.loader),
            Some(&known_mod_ids),
        ))
    } else {
        None
    };

    let detected_provider_matches = Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .build()
        .ok()
        .map(|client| {
            detect_provider_matches_for_local_mod(
                &client,
                &file_bytes,
                &safe_filename,
                normalized_content_type == "mods",
                None,
            )
        });
    let detected_provider_matches = detected_provider_matches.unwrap_or_default();
    let detected_provider =
        select_preferred_provider_match(&detected_provider_matches, None).cloned();
    let detected_provider_candidates = to_provider_candidates(&detected_provider_matches);
    let detected_provider_for_activation = detected_provider
        .as_ref()
        .filter(|found| provider_match_is_auto_activatable(found))
        .cloned();
    lock.entries.retain(|e| {
        !(e.filename == safe_filename
            && normalize_lock_content_type(&e.content_type) == normalized_content_type)
    });

    if let Some(found) = detected_provider_for_activation.as_ref() {
        remove_replaced_entries_for_content(
            &mut lock,
            &instance_dir,
            &found.project_id,
            &normalized_content_type,
        )?;
    }

    let new_entry = if let Some(found) = detected_provider_for_activation {
        LockEntry {
            source: found.source,
            project_id: found.project_id,
            version_id: found.version_id,
            name: canonical_lock_entry_name(&normalized_content_type, &safe_filename, &found.name),
            version_number: found.version_number,
            filename: safe_filename.clone(),
            content_type: normalized_content_type.clone(),
            target_scope: if normalized_content_type == "datapacks" {
                "world".to_string()
            } else {
                "instance".to_string()
            },
            target_worlds: worlds.clone(),
            pinned_version: previous_pin.clone(),
            enabled: true,
            hashes: found.hashes,
            provider_candidates: detected_provider_candidates,
            local_analysis: local_analysis.clone(),
        }
    } else {
        let project_id = format!(
            "local:{}:{}",
            normalized_content_type,
            safe_filename.to_lowercase()
        );
        LockEntry {
            source: "local".into(),
            project_id,
            version_id: format!("local_{}", now_millis()),
            name: canonical_lock_entry_name(
                &normalized_content_type,
                &safe_filename,
                &infer_local_name(&safe_filename),
            ),
            version_number: "local-file".into(),
            filename: safe_filename.clone(),
            content_type: normalized_content_type.clone(),
            target_scope: if normalized_content_type == "datapacks" {
                "world".to_string()
            } else {
                "instance".to_string()
            },
            target_worlds: worlds,
            pinned_version: previous_pin,
            enabled: true,
            hashes: HashMap::new(),
            provider_candidates: detected_provider_candidates,
            local_analysis,
        }
    };

    lock.entries.push(new_entry.clone());
    lock.entries
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;
    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "local_import",
        format!(
            "Imported local {} file '{}'.",
            content_type_display_name(&normalized_content_type),
            new_entry.name
        ),
    );

    Ok(lock_entry_to_installed(&instance_dir, &new_entry))
}

fn lock_entry_has_untrusted_github_mapping(entry: &LockEntry) -> bool {
    if !entry.source.trim().eq_ignore_ascii_case("github") {
        return false;
    }
    if parse_github_project_id(&entry.project_id).is_err() {
        return true;
    }
    let github_candidates = entry
        .provider_candidates
        .iter()
        .filter(|candidate| candidate.source.trim().eq_ignore_ascii_case("github"))
        .collect::<Vec<_>>();
    if github_candidates.is_empty() {
        return true;
    }
    !github_candidates
        .iter()
        .any(|candidate| provider_candidate_is_auto_activatable(candidate))
}

fn resolve_local_mod_sources_inner(
    app: &tauri::AppHandle,
    instance_id: &str,
    mode: &str,
    requested_content_types: Option<&[String]>,
) -> Result<LocalResolverResult, String> {
    let instances_dir = app_instances_dir(app)?;
    let instance = find_instance(&instances_dir, instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, instance_id)?;
    let mut lock = read_lockfile(&instances_dir, instance_id)?;
    let strict_local_only = !mode.trim().eq_ignore_ascii_case("all");
    let content_types_filter: HashSet<String> = if let Some(requested) = requested_content_types {
        let mut allowed = requested
            .iter()
            .map(|value| normalize_lock_content_type(value))
            .filter(|value| is_supported_local_content_type(value))
            .collect::<HashSet<_>>();
        if allowed.is_empty() {
            supported_local_content_types()
                .iter()
                .map(|value| value.to_string())
                .collect()
        } else {
            std::mem::take(&mut allowed)
        }
    } else {
        supported_local_content_types()
            .iter()
            .map(|value| value.to_string())
            .collect()
    };

    let client = Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| format!("build http client failed: {e}"))?;

    let mut scanned_entries = 0usize;
    let mut resolved_entries = 0usize;
    let mut matches: Vec<LocalResolverMatch> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut changed = false;

    let mut known_mod_ids =
        collect_known_enabled_mod_ids_for_dependency_checks(&lock, &instance_dir, &instance.loader);
    for idx in 0..lock.entries.len() {
        let source = lock.entries[idx].source.trim().to_ascii_lowercase();
        let is_local = source == "local";
        let is_supported_provider_source =
            source == "modrinth" || source == "curseforge" || source == "github";
        let should_revalidate_untrusted_github =
            !strict_local_only && lock_entry_has_untrusted_github_mapping(&lock.entries[idx]);
        if strict_local_only && !is_local {
            continue;
        }
        let entry_content_type = normalize_lock_content_type(&lock.entries[idx].content_type);
        let should_revalidate_github_in_all = !strict_local_only
            && source == "github"
            && entry_content_type == "mods"
            && lock.entries[idx]
                .version_id
                .trim()
                .starts_with("gh_release:");
        let should_repair_existing_github_mapping = !strict_local_only
            && source == "github"
            && entry_content_type == "mods"
            && (should_revalidate_untrusted_github || should_revalidate_github_in_all);
        let should_scan_non_local_entry = !strict_local_only
            && !is_local
            && (!is_supported_provider_source
                || lock.entries[idx].provider_candidates.is_empty()
                || should_revalidate_untrusted_github
                || should_revalidate_github_in_all);
        if !is_local && !should_scan_non_local_entry {
            continue;
        }
        if !content_types_filter.contains(&entry_content_type) {
            continue;
        }
        scanned_entries += 1;
        let filename = lock.entries[idx].filename.clone();
        let existing = local_entry_file_read_path(&instance_dir, &lock.entries[idx])?;
        let Some(read_path) = existing else {
            warnings.push(format!("Skipped '{}': file missing on disk.", filename));
            continue;
        };
        let file_bytes = match fs::read(&read_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                warnings.push(format!("Skipped '{}': read failed ({err}).", filename));
                continue;
            }
        };
        if entry_content_type == "mods" {
            let analysis = analyze_local_mod_file(
                &filename,
                &file_bytes,
                Some(&instance.loader),
                Some(&known_mod_ids),
            );
            for mod_id in &analysis.mod_ids {
                if let Some(normalized) = normalize_local_mod_id(mod_id) {
                    known_mod_ids.insert(normalized);
                }
            }
            let had_analysis = lock.entries[idx].local_analysis.is_some();
            lock.entries[idx].local_analysis = Some(analysis);
            if !had_analysis {
                changed = true;
            }
        }
        let found_matches = detect_provider_matches_for_local_mod(
            &client,
            &file_bytes,
            &filename,
            entry_content_type == "mods",
            if should_revalidate_github_in_all {
                Some(lock.entries[idx].project_id.as_str())
            } else {
                None
            },
        );
        let github_verification_unavailable =
            provider_matches_have_transient_github_verification_issue(&found_matches);
        let preferred_source_hint = if should_revalidate_github_in_all {
            Some("github")
        } else {
            None
        };
        let preferred_match =
            select_preferred_provider_match(&found_matches, preferred_source_hint);
        let preferred_is_activatable = preferred_match
            .map(provider_match_is_auto_activatable)
            .unwrap_or(false);
        let key_before = local_entry_key(&lock.entries[idx]);
        let from_source = lock.entries[idx].source.clone();
        let existing_project_id = lock.entries[idx].project_id.clone();
        let existing_project_id_valid = parse_github_project_id(&existing_project_id).is_ok();

        if should_repair_existing_github_mapping {
            if !existing_project_id_valid {
                let revert_reason = format!(
                    "Existing GitHub mapping has invalid repository ID '{}'.",
                    existing_project_id.trim()
                );
                lock.entries[idx].source = "local".to_string();
                lock.entries[idx].project_id =
                    format!("local:{}", sanitize_filename(&lock.entries[idx].filename));
                lock.entries[idx].version_id = format!("local_{}", Uuid::new_v4());
                lock.entries[idx].version_number = "local-file".to_string();
                lock.entries[idx]
                    .provider_candidates
                    .retain(|candidate| !candidate.source.trim().eq_ignore_ascii_case("github"));
                changed = true;
                resolved_entries += 1;
                warnings.push(format!(
                    "Reverted GitHub mapping for '{}' to local: {}",
                    filename, revert_reason
                ));
                matches.push(LocalResolverMatch {
                    key: key_before,
                    from_source,
                    to_source: "local".to_string(),
                    project_id: lock.entries[idx].project_id.clone(),
                    version_id: lock.entries[idx].version_id.clone(),
                    name: lock.entries[idx].name.clone(),
                    version_number: lock.entries[idx].version_number.clone(),
                    confidence: "manual".to_string(),
                    reason: revert_reason,
                });
                continue;
            }

            if let Some(transient_reason) = github_verification_unavailable {
                if preferred_match.is_some() && preferred_is_activatable {
                    // A strong activatable match exists, so we can safely switch despite transient API issues.
                } else {
                    warnings.push(format!(
                        "Kept existing GitHub mapping for '{}': repository/release verification is temporarily unavailable ({}).",
                        filename, transient_reason
                    ));
                }
            } else if preferred_match.is_none() {
                warnings.push(format!(
                    "Kept existing GitHub mapping for '{}': no hard contradictory evidence was found during revalidation.",
                    filename
                ));
                continue;
            } else if !preferred_is_activatable {
                if let Some(found) = preferred_match {
                    warnings.push(format!(
                        "Kept existing GitHub mapping for '{}': detected match is {} confidence and remains non-active ({})",
                        filename, found.confidence, found.reason
                    ));
                }
            }
        }
        let Some(found) = preferred_match else {
            continue;
        };
        if is_local
            && found.source.trim().eq_ignore_ascii_case("github")
            && !provider_match_is_auto_activatable(found)
        {
            warnings.push(format!(
                "Kept '{}' as local: GitHub candidate is {} confidence and will stay non-active ({})",
                filename, found.confidence, found.reason
            ));
        }
        let before_candidates = lock.entries[idx].provider_candidates.clone();
        let mut before_candidate_keys = before_candidates
            .iter()
            .map(provider_candidate_identity_key)
            .collect::<Vec<_>>();
        before_candidate_keys.sort();

        if (is_local
            || !is_supported_provider_source
            || should_revalidate_untrusted_github
            || should_revalidate_github_in_all)
            && provider_match_is_auto_activatable(found)
        {
            apply_provider_match_to_lock_entry(&mut lock.entries[idx], found);
        }
        if lock.entries[idx].provider_candidates.is_empty() {
            lock.entries[idx].provider_candidates =
                lock_entry_provider_candidates(&lock.entries[idx]);
        }
        for candidate in to_provider_candidates(&found_matches) {
            upsert_provider_candidate(&mut lock.entries[idx], candidate);
        }

        let source_changed = lock.entries[idx].source != from_source;
        let mut after_candidate_keys = lock.entries[idx]
            .provider_candidates
            .iter()
            .map(provider_candidate_identity_key)
            .collect::<Vec<_>>();
        after_candidate_keys.sort();
        let candidates_changed = after_candidate_keys != before_candidate_keys;
        if !source_changed && !candidates_changed {
            continue;
        }
        resolved_entries += 1;
        changed = true;
        let preferred_for_report = if source_changed {
            Some(lock.entries[idx].source.as_str())
        } else {
            Some(from_source.as_str())
        };
        let found_for_report =
            select_preferred_provider_match(&found_matches, preferred_for_report).unwrap_or(found);
        matches.push(LocalResolverMatch {
            key: key_before,
            from_source,
            to_source: lock.entries[idx].source.clone(),
            project_id: found_for_report.project_id.clone(),
            version_id: found_for_report.version_id.clone(),
            name: found_for_report.name.clone(),
            version_number: found_for_report.version_number.clone(),
            confidence: found_for_report.confidence.clone(),
            reason: found_for_report.reason.clone(),
        });
    }

    if changed {
        lock.entries
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        write_lockfile(&instances_dir, instance_id, &lock)?;
        let github_reverts = matches
            .iter()
            .filter(|item| {
                item.from_source.trim().eq_ignore_ascii_case("github")
                    && item.to_source.trim().eq_ignore_ascii_case("local")
            })
            .count();
        let github_activations = matches
            .iter()
            .filter(|item| item.to_source.trim().eq_ignore_ascii_case("github"))
            .count();
        log_instance_event_best_effort(
            app,
            instance_id,
            "local_resolve",
            format!(
                "Resolved {} local entry provider mapping(s) (GitHub activated: {}, GitHub reverted: {}).",
                resolved_entries, github_activations, github_reverts
            ),
        );
    }

    let remaining_local_entries = lock
        .entries
        .iter()
        .filter(|entry| entry.source.trim().eq_ignore_ascii_case("local"))
        .count();

    Ok(LocalResolverResult {
        instance_id: instance_id.to_string(),
        scanned_entries,
        resolved_entries,
        remaining_local_entries,
        matches,
        warnings,
    })
}

#[tauri::command]
pub(crate) async fn resolve_local_mod_sources(
    app: tauri::AppHandle,
    args: ResolveLocalModSourcesArgs,
) -> Result<LocalResolverResult, String> {
    run_blocking_task("resolve local mod sources", move || {
        let mode = args.mode.unwrap_or_else(|| "missing_only".to_string());
        resolve_local_mod_sources_inner(
            &app,
            &args.instance_id,
            &mode,
            args.content_types.as_deref(),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn check_instance_content_updates(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<ContentUpdateCheckResult, String> {
    run_blocking_task("check instance content updates", move || {
        check_instance_content_updates_command_inner(app, args)
    })
    .await
}

fn check_instance_content_updates_command_inner(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<ContentUpdateCheckResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let content_type_filter = normalize_update_content_type_filter(args.content_types.as_deref());
    check_instance_content_updates_inner(
        &client,
        &instance,
        &lock,
        UpdateScope::AllContent,
        content_type_filter.as_ref(),
    )
}

fn canonical_curseforge_dependency_id(raw: &str) -> Option<String> {
    parse_curseforge_project_id(raw)
        .ok()
        .filter(|value| *value > 0)
        .map(|value| format!("cf:{value}"))
}

fn canonical_update_project_key(source: &str, project_id: &str) -> Option<String> {
    let normalized_source = source.trim().to_ascii_lowercase();
    let normalized_project = project_id.trim();
    if normalized_project.is_empty() {
        return None;
    }
    if normalized_source == "modrinth" {
        return Some(format!(
            "modrinth:{}",
            normalized_project.to_ascii_lowercase()
        ));
    }
    if normalized_source == "curseforge" {
        return canonical_curseforge_dependency_id(normalized_project)
            .map(|value| format!("curseforge:{value}"));
    }
    Some(format!(
        "{}:{}",
        normalized_source,
        normalized_project.to_ascii_lowercase()
    ))
}

fn canonical_update_project_key_for_item(update: &ContentUpdateInfo) -> Option<String> {
    canonical_update_project_key(&update.source, &update.project_id)
}

fn canonical_dependency_project_key_for_update(
    update: &ContentUpdateInfo,
    dependency: &str,
) -> Option<String> {
    let normalized_source = update.source.trim().to_ascii_lowercase();
    if normalized_source == "modrinth" {
        return canonical_update_project_key("modrinth", dependency);
    }
    if normalized_source == "curseforge" {
        return canonical_update_project_key("curseforge", dependency);
    }
    None
}

fn order_updates_by_required_dependencies(
    updates: &[ContentUpdateInfo],
) -> (Vec<usize>, Vec<String>) {
    if updates.len() <= 1 {
        return ((0..updates.len()).collect(), vec![]);
    }

    let mut warnings: Vec<String> = Vec::new();
    let mut key_to_index: HashMap<String, usize> = HashMap::new();
    for (idx, update) in updates.iter().enumerate() {
        if let Some(key) = canonical_update_project_key_for_item(update) {
            key_to_index.entry(key).or_insert(idx);
        }
    }

    let mut edges: Vec<Vec<usize>> = vec![vec![]; updates.len()];
    let mut indegree: Vec<usize> = vec![0; updates.len()];
    for (idx, update) in updates.iter().enumerate() {
        let self_key = canonical_update_project_key_for_item(update);
        let mut seen_deps: HashSet<usize> = HashSet::new();
        for dependency in &update.required_dependencies {
            let Some(dep_key) = canonical_dependency_project_key_for_update(update, dependency)
            else {
                continue;
            };
            if self_key
                .as_ref()
                .map(|value| value == &dep_key)
                .unwrap_or(false)
            {
                continue;
            }
            let Some(dep_idx) = key_to_index.get(&dep_key).copied() else {
                continue;
            };
            if seen_deps.insert(dep_idx) {
                edges[dep_idx].push(idx);
                indegree[idx] += 1;
            }
        }
    }

    let mut ready: BTreeSet<usize> = BTreeSet::new();
    for (idx, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            ready.insert(idx);
        }
    }

    let mut ordered: Vec<usize> = Vec::with_capacity(updates.len());
    while let Some(next) = ready.first().copied() {
        ready.remove(&next);
        ordered.push(next);
        for dependent in edges[next].iter().copied() {
            if indegree[dependent] == 0 {
                continue;
            }
            indegree[dependent] -= 1;
            if indegree[dependent] == 0 {
                ready.insert(dependent);
            }
        }
    }

    if ordered.len() < updates.len() {
        let mut remaining = (0..updates.len())
            .filter(|idx| !ordered.contains(idx))
            .collect::<Vec<_>>();
        remaining.sort_unstable();
        warnings.push(format!(
            "Update dependency cycle detected across {} entr{}; continuing with deterministic fallback order.",
            remaining.len(),
            if remaining.len() == 1 { "y" } else { "ies" }
        ));
        ordered.extend(remaining);
    }

    (ordered, warnings)
}

fn missing_required_dependencies_for_update(
    lock: &Lockfile,
    update: &ContentUpdateInfo,
) -> Vec<String> {
    if normalize_lock_content_type(&update.content_type) != "mods" {
        return vec![];
    }
    let source = update.source.trim().to_ascii_lowercase();
    if source != "modrinth" && source != "curseforge" {
        return vec![];
    }
    let mut missing: Vec<String> = Vec::new();
    for dependency in &update.required_dependencies {
        if source == "modrinth" {
            let project_id = dependency.trim();
            if project_id.is_empty() {
                continue;
            }
            if !lock_has_enabled_modrinth_mod(lock, project_id) {
                missing.push(project_id.to_string());
            }
            continue;
        }
        let Some(mod_id) = parse_curseforge_project_id(dependency).ok() else {
            continue;
        };
        if mod_id > 0 && !lock_has_enabled_curseforge_mod(lock, mod_id) {
            missing.push(format!("cf:{mod_id}"));
        }
    }
    missing.sort();
    missing.dedup();
    missing
}

fn update_all_instance_content_inner(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<UpdateAllContentResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let content_type_filter = normalize_update_content_type_filter(args.content_types.as_deref());
    let check = check_instance_content_updates_inner(
        &client,
        &instance,
        &lock,
        UpdateScope::AllContent,
        content_type_filter.as_ref(),
    )?;

    if !check.updates.is_empty() {
        create_instance_snapshot_with_event_best_effort(
            &app,
            &instances_dir,
            &args.instance_id,
            "before-update-all",
        );
    }
    let mut updated_entries = 0usize;
    let mut warnings = check.warnings.clone();
    let mut by_source: HashMap<String, usize> = HashMap::new();
    let mut by_content_type: HashMap<String, usize> = HashMap::new();
    let (ordered_indexes, mut ordering_warnings) =
        order_updates_by_required_dependencies(&check.updates);
    warnings.append(&mut ordering_warnings);
    let ordered_updates = ordered_indexes
        .into_iter()
        .filter_map(|idx| check.updates.get(idx).cloned())
        .collect::<Vec<_>>();
    let cf_key = curseforge_api_key();
    let prefetch_worker_cap = adaptive_update_prefetch_worker_cap(&ordered_updates);
    let prefetched_downloads =
        prefetch_update_downloads(&client, &ordered_updates, prefetch_worker_cap);

    for (idx, update) in ordered_updates.iter().enumerate() {
        let current_lock = read_lockfile(&instances_dir, &args.instance_id)?;
        let missing_dependencies = missing_required_dependencies_for_update(&current_lock, update);
        if !missing_dependencies.is_empty() {
            warnings.push(format!(
                "Skipped update '{}' ({}): missing required dependencies [{}].",
                update.name,
                update.project_id,
                missing_dependencies.join(", ")
            ));
            continue;
        }
        let mut used_fast_path = false;
        let install_result = match try_fast_install_content_update(
            &instances_dir,
            &instance,
            &args,
            &client,
            cf_key.as_deref(),
            update,
            prefetched_downloads.get(&idx),
        ) {
            Ok(Some(installed)) => {
                used_fast_path = true;
                Ok(installed)
            }
            Ok(None) => install_discover_content_inner(
                app.clone(),
                &InstallDiscoverContentArgs {
                    instance_id: args.instance_id.clone(),
                    source: update.source.clone(),
                    project_id: update.project_id.clone(),
                    project_title: Some(update.name.clone()),
                    content_type: update.content_type.clone(),
                    target_worlds: update.target_worlds.clone(),
                },
                None,
            ),
            Err(fast_err) => {
                if update.source.trim().eq_ignore_ascii_case("curseforge")
                    && error_mentions_forbidden(&fast_err)
                {
                    warnings.push(format!(
                        "Skipped CurseForge update '{}' ({}): provider blocked automated download (403).",
                        update.name, update.project_id
                    ));
                    continue;
                }
                warnings.push(format!(
                    "Fast update fallback for '{}': {}",
                    update.name, fast_err
                ));
                install_discover_content_inner(
                    app.clone(),
                    &InstallDiscoverContentArgs {
                        instance_id: args.instance_id.clone(),
                        source: update.source.clone(),
                        project_id: update.project_id.clone(),
                        project_title: Some(update.name.clone()),
                        content_type: update.content_type.clone(),
                        target_worlds: update.target_worlds.clone(),
                    },
                    None,
                )
            }
        };

        match install_result {
            Ok(installed) => {
                if installed.version_id.trim() == update.current_version_id.trim() {
                    warnings.push(format!(
                        "No version change for '{}' (still {}).",
                        update.name,
                        if installed.version_number.trim().is_empty() {
                            installed.version_id.clone()
                        } else {
                            installed.version_number.clone()
                        }
                    ));
                    continue;
                }
                if !used_fast_path && update.content_type == "mods" && !update.enabled {
                    let disable_res = set_installed_mod_enabled(
                        app.clone(),
                        SetInstalledModEnabledArgs {
                            instance_id: args.instance_id.clone(),
                            version_id: installed.version_id,
                            content_type: Some(installed.content_type),
                            filename: Some(installed.filename),
                            enabled: false,
                        },
                    );
                    if let Err(err) = disable_res {
                        warnings.push(format!(
                            "Updated '{}' but failed to keep it disabled: {}",
                            update.name, err
                        ));
                    }
                }
                updated_entries += 1;
                *by_source.entry(update.source.clone()).or_insert(0) += 1;
                *by_content_type
                    .entry(update.content_type.clone())
                    .or_insert(0) += 1;
            }
            Err(err) => {
                warnings.push(format!("Failed to update '{}': {}", update.name, err));
            }
        }
    }

    if updated_entries > 0 {
        log_instance_event_best_effort(
            &app,
            &args.instance_id,
            "content_update_all",
            format!(
                "Updated {} content entries from update-all.",
                updated_entries
            ),
        );
    }
    Ok(UpdateAllContentResult {
        checked_entries: check.checked_entries,
        updated_entries,
        warnings,
        by_source,
        by_content_type,
    })
}

#[tauri::command]
pub(crate) async fn update_all_instance_content(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<UpdateAllContentResult, String> {
    run_blocking_task("update all instance content", move || {
        update_all_instance_content_inner(app, args)
    })
    .await
}

#[tauri::command]
pub(crate) async fn check_modrinth_updates(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<ModUpdateCheckResult, String> {
    run_blocking_task("check modrinth updates", move || {
        check_modrinth_updates_inner(app, args)
    })
    .await
}

fn check_modrinth_updates_inner(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<ModUpdateCheckResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let content = check_instance_content_updates_inner(
        &client,
        &instance,
        &lock,
        UpdateScope::ModrinthModsOnly,
        None,
    )?;
    Ok(content_updates_to_modrinth_result(content))
}

#[tauri::command]
pub(crate) async fn update_all_modrinth_mods(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<UpdateAllResult, String> {
    run_blocking_task("update all modrinth mods", move || {
        update_all_modrinth_mods_inner(app, args)
    })
    .await
}

fn update_all_modrinth_mods_inner(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<UpdateAllResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let check = check_instance_content_updates_inner(
        &client,
        &instance,
        &lock,
        UpdateScope::ModrinthModsOnly,
        None,
    )?;
    if !check.updates.is_empty() {
        create_instance_snapshot_with_event_best_effort(
            &app,
            &instances_dir,
            &args.instance_id,
            "before-update-all",
        );
    }
    let mut updated_mods = 0usize;
    let mut update_indexes = order_updates_by_required_dependencies(&check.updates).0;
    if update_indexes.is_empty() {
        update_indexes = (0..check.updates.len()).collect();
    }
    for idx in update_indexes {
        let Some(update) = check.updates.get(idx) else {
            continue;
        };
        let current_lock = read_lockfile(&instances_dir, &args.instance_id)?;
        if !missing_required_dependencies_for_update(&current_lock, update).is_empty() {
            continue;
        }
        if let Ok(_) = install_discover_content_inner(
            app.clone(),
            &InstallDiscoverContentArgs {
                instance_id: args.instance_id.clone(),
                source: "modrinth".to_string(),
                project_id: update.project_id.clone(),
                project_title: Some(update.name.clone()),
                content_type: "mods".to_string(),
                target_worlds: vec![],
            },
            None,
        ) {
            updated_mods += 1;
        }
    }
    if updated_mods > 0 {
        log_instance_event_best_effort(
            &app,
            &args.instance_id,
            "mods_update_all",
            format!("Updated {} mod(s) from Modrinth update-all.", updated_mods),
        );
    }
    Ok(UpdateAllResult {
        checked_mods: check.checked_entries,
        updated_mods,
    })
}

#[tauri::command]
pub(crate) fn list_quick_play_servers(
    app: tauri::AppHandle,
) -> Result<Vec<QuickPlayServerEntry>, String> {
    let store = read_quick_play_servers(&app)?;
    Ok(store.servers)
}

#[tauri::command]
pub(crate) fn upsert_quick_play_server(
    app: tauri::AppHandle,
    args: UpsertQuickPlayServerArgs,
) -> Result<Vec<QuickPlayServerEntry>, String> {
    let mut store = read_quick_play_servers(&app)?;
    let name = args.name.trim().to_string();
    if name.is_empty() {
        return Err("Server name is required.".to_string());
    }
    let host = normalize_quick_play_host(&args.host)
        .ok_or_else(|| "Server host is invalid.".to_string())?;
    let port = normalize_quick_play_port(args.port);
    let bound_instance_id = args
        .bound_instance_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(instance_id) = bound_instance_id.as_ref() {
        let instances_dir = app_instances_dir(&app)?;
        let _ = find_instance(&instances_dir, instance_id)?;
    }
    let target_id = args
        .id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("qps_{}", Uuid::new_v4().simple()));
    if let Some(existing) = store.servers.iter_mut().find(|entry| entry.id == target_id) {
        existing.name = name;
        existing.host = host;
        existing.port = port;
        existing.bound_instance_id = bound_instance_id;
    } else {
        store.servers.push(QuickPlayServerEntry {
            id: target_id,
            name,
            host,
            port,
            bound_instance_id,
            last_used_at: None,
        });
    }
    write_quick_play_servers(&app, &store)?;
    let refreshed = read_quick_play_servers(&app)?;
    Ok(refreshed.servers)
}

#[tauri::command]
pub(crate) fn remove_quick_play_server(
    app: tauri::AppHandle,
    args: RemoveQuickPlayServerArgs,
) -> Result<Vec<QuickPlayServerEntry>, String> {
    let mut store = read_quick_play_servers(&app)?;
    let before = store.servers.len();
    store.servers.retain(|entry| entry.id != args.id);
    if store.servers.len() == before {
        return Err("Quick play server not found.".to_string());
    }
    write_quick_play_servers(&app, &store)?;
    let refreshed = read_quick_play_servers(&app)?;
    Ok(refreshed.servers)
}

#[tauri::command]
pub(crate) async fn launch_quick_play_server(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: LaunchQuickPlayServerArgs,
) -> Result<LaunchResult, String> {
    let mut store = read_quick_play_servers(&app)?;
    let idx = store
        .servers
        .iter()
        .position(|entry| entry.id == args.server_id)
        .ok_or_else(|| "Quick play server not found.".to_string())?;
    let server = store.servers[idx].clone();
    let instance_id = if let Some(override_id) = args.instance_id.as_ref() {
        let trimmed = override_id.trim();
        if trimmed.is_empty() {
            return Err("Instance override is empty.".to_string());
        }
        trimmed.to_string()
    } else if let Some(bound) = server.bound_instance_id.as_ref() {
        bound.clone()
    } else {
        return Err("No instance is bound to this server. Choose an instance first.".to_string());
    };

    let result = launch_instance(
        app.clone(),
        state,
        LaunchInstanceArgs {
            instance_id: instance_id.clone(),
            method: args.method.clone(),
            quick_play_host: Some(server.host.clone()),
            quick_play_port: Some(server.port),
        },
    )
    .await?;

    store.servers[idx].last_used_at = Some(now_iso());
    if store.servers[idx].bound_instance_id.is_none() {
        store.servers[idx].bound_instance_id = Some(instance_id);
    }
    let _ = write_quick_play_servers(&app, &store);
    Ok(result)
}

#[tauri::command]
pub(crate) async fn launch_instance(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: LaunchInstanceArgs,
) -> Result<LaunchResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_settings = normalize_instance_settings(instance.settings.clone());
    let app_instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let settings = read_launcher_settings(&app)?;
    let method = if let Some(input) = args.method.as_ref() {
        LaunchMethod::parse(input).ok_or_else(|| "method must be prism or native".to_string())?
    } else {
        settings.default_launch_method.clone()
    };
    let quick_play_host = match args.quick_play_host.as_ref() {
        Some(value) => Some(
            normalize_quick_play_host(value)
                .ok_or_else(|| "Quick play host is invalid.".to_string())?,
        ),
        None => None,
    };
    let quick_play_port = normalize_quick_play_port(args.quick_play_port);
    let quick_play_active = quick_play_host.is_some();
    clear_launch_cancel_request(&state, &instance.id)?;
    if let Err(err) = mark_instance_launch_triggered(&instances_dir, &instance.id) {
        eprintln!(
            "instance last-run metadata launch marker write failed for '{}': {}",
            instance.id, err
        );
    }
    let method_for_report = method.clone();
    let launch_result: Result<LaunchResult, String> = match method {
        LaunchMethod::Prism => {
            if is_launch_cancel_requested(&state, &instance.id)? {
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Prism.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }
            let prism_root = prism_root_dir()?;
            let prism_instance_id = find_prism_instance_id(&prism_root, &instance)?;
            let prism_mc_dir = prism_root
                .join("instances")
                .join(&prism_instance_id)
                .join("minecraft");
            match hydrate_instance_settings_from_prism(&prism_mc_dir, &app_instance_dir) {
                Ok(copied) if copied > 0 => {
                    log_instance_event_best_effort(
                        &app,
                        &instance.id,
                        "settings_sync",
                        format!(
                            "Refreshed {} Minecraft settings file(s) from Prism before cross-instance sync.",
                            copied
                        ),
                    );
                }
                Ok(_) => {}
                Err(err) => {
                    eprintln!(
                        "prism settings hydration before launch failed for '{}': {}",
                        instance.id, err
                    );
                }
            }
            run_instance_settings_sync_before_launch(
                &app,
                &instances_dir,
                &instance,
                &instance_settings,
            );

            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Prism.as_str(),
                "starting",
                "Preparing Prism sync…",
            );
            sync_prism_instance_content(&app_instance_dir, &prism_mc_dir)?;
            if is_launch_cancel_requested(&state, &instance.id)? {
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Prism.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }
            launch_prism_instance(
                &prism_root,
                &prism_instance_id,
                quick_play_host.as_deref(),
                if quick_play_active {
                    Some(quick_play_port)
                } else {
                    None
                },
            )?;
            clear_launch_cancel_request(&state, &instance.id)?;

            Ok(LaunchResult {
                method: "prism".to_string(),
                launch_id: None,
                pid: None,
                prism_instance_id: Some(prism_instance_id),
                prism_root: Some(prism_root.display().to_string()),
                message: if let Some(host) = quick_play_host.as_ref() {
                    format!(
                        "Synced mods/config to Prism instance and launched it (quick join {}:{}).",
                        host, quick_play_port
                    )
                } else {
                    "Synced mods/config to Prism instance and launched it.".into()
                },
            })
        }
        LaunchMethod::Native => {
            let mut existing_native_runs_for_instance = 0usize;
            {
                let mut guard = state
                    .running
                    .lock()
                    .map_err(|_| "lock running instances failed".to_string())?;
                let mut finished: Vec<String> = Vec::new();
                for (id, proc_entry) in guard.iter_mut() {
                    if proc_entry.meta.instance_id != instance.id
                        || !proc_entry.meta.method.eq_ignore_ascii_case("native")
                    {
                        continue;
                    }
                    if let Ok(mut child) = proc_entry.child.lock() {
                        if let Ok(Some(_)) = child.try_wait() {
                            finished.push(id.clone());
                        } else {
                            existing_native_runs_for_instance += 1;
                        }
                    }
                }
                for id in finished {
                    guard.remove(&id);
                }
            }
            if is_launch_cancel_requested(&state, &instance.id)? {
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Native.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }
            run_instance_settings_sync_before_launch(
                &app,
                &instances_dir,
                &instance,
                &instance_settings,
            );

            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Preparing native launch…",
            );

            let java_executable = if !instance_settings.java_path.trim().is_empty() {
                let p = PathBuf::from(instance_settings.java_path.trim());
                if !p.exists() {
                    return Err(format!(
                        "Instance Java path does not exist: {}",
                        instance_settings.java_path
                    ));
                }
                p.display().to_string()
            } else {
                resolve_java_executable(&settings)?
            };
            let (java_major, java_version_line) = detect_java_major(&java_executable)?;
            let required_java = required_java_major_for_mc(&instance.mc_version);
            if java_major < required_java {
                return Err(format!(
                    "Java {} detected ({}), but Minecraft {} needs Java {}+. Update Java path in Instance Settings > Java & Memory or Settings > Launcher.",
                    java_major, java_version_line, instance.mc_version, required_java
                ));
            }
            if is_launch_cancel_requested(&state, &instance.id)? {
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Native.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }

            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Refreshing Microsoft session…",
            );
            let app_for_auth = app.clone();
            let settings_for_auth = settings.clone();
            let instance_for_auth = instance.clone();
            let (account, mc_access_token, loader, loader_version) =
                await_launch_stage_with_cancel(
                    &app,
                    &state,
                    &instance.id,
                    LaunchMethod::Native.as_str(),
                    "Authentication",
                    150,
                    async move {
                        tauri::async_runtime::spawn_blocking(move || {
                            resolve_native_auth_and_loader(
                                &app_for_auth,
                                &settings_for_auth,
                                &instance_for_auth,
                            )
                        })
                        .await
                        .map_err(|e| format!("native auth task join failed: {e}"))?
                    },
                )
                .await?;

            let launch_id = format!("native_{}", Uuid::new_v4());
            let use_isolated_runtime_session = existing_native_runs_for_instance > 0;
            let runtime_session_cleanup_dir = if use_isolated_runtime_session {
                Some(
                    app_instance_dir
                        .join("runtime_sessions")
                        .join(launch_id.replace(':', "_")),
                )
            } else {
                None
            };
            let runtime_dir = runtime_session_cleanup_dir
                .clone()
                .unwrap_or_else(|| app_instance_dir.clone());
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                if use_isolated_runtime_session {
                    "Preparing isolated runtime session…"
                } else {
                    "Preparing instance files…"
                },
            );
            let app_instance_dir_for_sync = app_instance_dir.clone();
            let app_for_sync = app.clone();
            let runtime_dir_for_sync = runtime_dir.clone();
            let use_isolated_runtime_for_sync = use_isolated_runtime_session;
            await_launch_stage_with_cancel(
                &app,
                &state,
                &instance.id,
                LaunchMethod::Native.as_str(),
                if use_isolated_runtime_for_sync {
                    "Isolated runtime prep"
                } else {
                    "Runtime preparation"
                },
                150,
                async move {
                    tauri::async_runtime::spawn_blocking(move || {
                        if !use_isolated_runtime_for_sync {
                            reconcile_legacy_runtime_into_instance(&app_instance_dir_for_sync)?;
                        }
                        let _ = cleanup_stale_runtime_sessions_for_instance(
                            &app_instance_dir_for_sync,
                            Duration::from_secs(STALE_RUNTIME_SESSION_MAX_AGE_HOURS * 3600),
                        );
                        fs::create_dir_all(&runtime_dir_for_sync)
                            .map_err(|e| format!("mkdir native runtime failed: {e}"))?;
                        if use_isolated_runtime_for_sync {
                            sync_instance_runtime_content_isolated(
                                &app_instance_dir_for_sync,
                                &runtime_dir_for_sync,
                            )?;
                        } else {
                            ensure_instance_content_dirs(&runtime_dir_for_sync)?;
                        }
                        let cache_dir = launcher_cache_dir(&app_for_sync)?;
                        fs::create_dir_all(&cache_dir)
                            .map_err(|e| format!("mkdir launcher cache failed: {e}"))?;
                        wire_shared_cache(&cache_dir, &runtime_dir_for_sync)?;
                        Ok(())
                    })
                    .await
                    .map_err(|e| format!("runtime preparation task join failed: {e}"))?
                },
            )
            .await?;

            let runtime_dir_str = runtime_dir.display().to_string();
            let mc_version = instance.mc_version.clone();
            let username = account.username.clone();
            let profile_id = account.id.clone();

            let mut launcher = OpenLauncher::new(
                &runtime_dir_str,
                &java_executable,
                ol_version::Version {
                    minecraft_version: mc_version,
                    loader,
                    loader_version,
                },
            )
            .await;
            launcher.auth(ol_auth::Auth::new(
                "msa".to_string(),
                "{}".to_string(),
                username,
                profile_id,
                mc_access_token,
            ));
            launcher.jvm_arg(&format!("-Xmx{}M", instance_settings.memory_mb));
            for arg in effective_jvm_args(&instance_settings.jvm_args) {
                launcher.jvm_arg(&arg);
            }
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Installing game version files…",
            );
            await_launch_stage_with_cancel(
                &app,
                &state,
                &instance.id,
                LaunchMethod::Native.as_str(),
                "Version install",
                300,
                async {
                    launcher
                        .install_version()
                        .await
                        .map_err(|e| format!("native install version failed: {e}"))
                },
            )
            .await?;
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Installing assets…",
            );
            await_launch_stage_with_cancel(
                &app,
                &state,
                &instance.id,
                LaunchMethod::Native.as_str(),
                "Assets install",
                900,
                async {
                    launcher
                        .install_assets()
                        .await
                        .map_err(|e| format!("native install assets failed: {e}"))
                },
            )
            .await?;
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Installing libraries…",
            );
            await_launch_stage_with_cancel(
                &app,
                &state,
                &instance.id,
                LaunchMethod::Native.as_str(),
                "Libraries install",
                900,
                async {
                    launcher
                        .install_libraries()
                        .await
                        .map_err(|e| format!("native install libraries failed: {e}"))
                },
            )
            .await?;

            let persistent_logs_dir = launch_logs_dir(&app_instance_dir);
            fs::create_dir_all(&persistent_logs_dir)
                .map_err(|e| format!("create launch logs directory failed: {e}"))?;
            let launch_log_file_name = format!(
                "{}-{}.log",
                Local::now().format("%Y%m%d-%H%M%S"),
                launch_id.replace(':', "_")
            );
            let launch_log_path = persistent_logs_dir.join(launch_log_file_name);
            let mut launch_log_file = File::create(&launch_log_path)
                .map_err(|e| format!("create native launch log failed: {e}"))?;
            let collisions = append_runtime_mod_diagnostics(&mut launch_log_file, &runtime_dir)?;
            if let Some((_, names)) = collisions
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("essential"))
            {
                let duplicate_list = names.join(", ");
                return Err(format!(
                    "Detected multiple Essential jars in the effective mods folder before launch: {duplicate_list}. Keep one Essential jar in '{}', then retry.",
                    runtime_dir.join("mods").display()
                ));
            }
            let launch_log_file_err = launch_log_file
                .try_clone()
                .map_err(|e| format!("clone native launch log handle failed: {e}"))?;
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Starting Java process…",
            );
            let mut command = launcher
                .command()
                .map_err(|e| format!("native launch command build failed: {e}"))?;
            if let Some(host) = quick_play_host.as_ref() {
                command.arg("--server").arg(host);
                command.arg("--port").arg(quick_play_port.to_string());
            }
            command.stdout(Stdio::from(launch_log_file));
            command.stderr(Stdio::from(launch_log_file_err));
            let mut child = command
                .spawn()
                .map_err(|e| format!("native launch spawn failed: {e}"))?;
            if is_launch_cancel_requested(&state, &instance.id)? {
                let _ = child.kill();
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Native.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }
            thread::sleep(Duration::from_millis(900));
            if let Ok(Some(status)) = child.try_wait() {
                if let Err(err) = mark_instance_launch_exit(&instances_dir, &instance.id, "crashed")
                {
                    eprintln!(
                        "instance last-run metadata crash marker write failed for '{}': {}",
                        instance.id, err
                    );
                }
                let tail = tail_lines_from_file(&launch_log_path, 24)
                    .map(|t| format!("\nRecent native-launch.log:\n{t}"))
                    .unwrap_or_default();
                return Err(format!(
                    "Native launch exited immediately with status {:?}. Check Java version/runtime mods. Log file: {}{}",
                    status.code(),
                    launch_log_path.display(),
                    tail
                ));
            }

            let pid = child.id();
            if let Err(err) = register_native_play_session_start(
                &instances_dir,
                &instance.id,
                &launch_id,
                pid,
                use_isolated_runtime_session,
            ) {
                eprintln!(
                    "playtime active session registration failed for '{}': {}",
                    instance.id, err
                );
            }
            let child = Arc::new(Mutex::new(child));
            let keep_launcher_open = instance_settings.keep_launcher_open_while_playing;
            let close_launcher_on_exit = instance_settings.close_launcher_on_game_exit;
            let world_backup_interval_secs =
                u64::from(instance_settings.world_backup_interval_minutes.clamp(5, 15)) * 60;
            let world_backup_retention_count =
                usize::try_from(instance_settings.world_backup_retention_count.clamp(1, 2))
                    .unwrap_or(1);
            let log_path_text = launch_log_path.display().to_string();
            let running_meta = RunningInstance {
                launch_id: launch_id.clone(),
                instance_id: instance.id.clone(),
                instance_name: instance.name.clone(),
                method: "native".to_string(),
                isolated: use_isolated_runtime_session,
                pid,
                started_at: now_iso(),
                log_path: Some(log_path_text),
            };
            {
                let mut guard = state
                    .running
                    .lock()
                    .map_err(|_| "lock running instances failed".to_string())?;
                guard.insert(
                    launch_id.clone(),
                    RunningProcess {
                        meta: running_meta.clone(),
                        child: child.clone(),
                        log_path: Some(launch_log_path.clone()),
                    },
                );
            }
            clear_launch_cancel_request(&state, &instance.id)?;
            if !keep_launcher_open {
                if let Some(window) = app.get_window("main") {
                    let _ = window.minimize();
                }
            }
            emit_launch_state(
                &app,
                &instance.id,
                Some(&launch_id),
                LaunchMethod::Native.as_str(),
                "running",
                if use_isolated_runtime_session {
                    isolated_native_launch_success_message()
                } else {
                    "Native launch started."
                },
            );

            let running_state = state.running.clone();
            let stop_requested_state = state.stop_requested_launches.clone();
            let app_for_thread = app.clone();
            let launch_id_for_thread = launch_id.clone();
            let instance_id_for_thread = instance.id.clone();
            let instances_dir_for_thread = instances_dir.clone();
            let keep_launcher_open_for_thread = keep_launcher_open;
            let close_launcher_on_exit_for_thread = close_launcher_on_exit;
            let world_backup_interval_secs_for_thread = world_backup_interval_secs;
            let world_backup_retention_count_for_thread = world_backup_retention_count;
            let run_world_backups_for_thread = !use_isolated_runtime_session;
            let runtime_session_cleanup_for_thread = runtime_session_cleanup_dir.clone();
            let app_instance_dir_for_thread = app_instance_dir.clone();
            let java_executable_for_thread = java_executable.clone();
            let java_major_for_thread = Some(java_major);
            let launch_log_path_for_thread = launch_log_path.clone();
            thread::spawn(move || {
                let mut next_world_backup_at =
                    Instant::now() + Duration::from_secs(world_backup_interval_secs_for_thread);
                let (mut exit_kind, exit_code, exit_message) = loop {
                    if run_world_backups_for_thread && Instant::now() >= next_world_backup_at {
                        let _ = create_world_backups_for_instance(
                            &instances_dir_for_thread,
                            &instance_id_for_thread,
                            "auto-world-backup",
                            world_backup_retention_count_for_thread,
                        );
                        next_world_backup_at = Instant::now()
                            + Duration::from_secs(world_backup_interval_secs_for_thread);
                    }
                    let waited = if let Ok(mut c) = child.lock() {
                        match c.try_wait() {
                            Ok(Some(status)) => Some((
                                if status.success() {
                                    "success".to_string()
                                } else {
                                    "crashed".to_string()
                                },
                                status.code(),
                                format!("Game exited with status {:?}", status.code()),
                            )),
                            Ok(None) => None,
                            Err(e) => Some((
                                "crashed".to_string(),
                                None,
                                format!("Failed to wait for game process: {e}"),
                            )),
                        }
                    } else {
                        Some((
                            "crashed".to_string(),
                            None,
                            "Failed to lock child process handle.".to_string(),
                        ))
                    };
                    if let Some(result) = waited {
                        break result;
                    }
                    thread::sleep(Duration::from_millis(450));
                };
                if let Ok(mut guard) = running_state.lock() {
                    guard.remove(&launch_id_for_thread);
                }
                let user_requested_stop = stop_requested_state
                    .lock()
                    .ok()
                    .map(|mut guard| guard.remove(&launch_id_for_thread))
                    .unwrap_or(false);
                let exit_message = if user_requested_stop {
                    exit_kind = "stopped".to_string();
                    "Instance stopped by user.".to_string()
                } else {
                    exit_message
                };
                if let Err(err) = mark_instance_launch_exit(
                    &instances_dir_for_thread,
                    &instance_id_for_thread,
                    &exit_kind,
                ) {
                    eprintln!(
                        "instance last-run metadata exit marker write failed for '{}': {}",
                        instance_id_for_thread, err
                    );
                }
                if let Err(err) = finalize_native_play_session(
                    &instances_dir_for_thread,
                    &instance_id_for_thread,
                    &launch_id_for_thread,
                    &exit_kind,
                    false,
                ) {
                    eprintln!(
                        "playtime session finalize failed for '{}': {}",
                        instance_id_for_thread, err
                    );
                }
                capture_run_report_best_effort(
                    &app_for_thread,
                    crate::run_reports::CaptureRunReportInput {
                        instance_id: instance_id_for_thread.clone(),
                        launch_method: LaunchMethod::Native.as_str().to_string(),
                        exit_kind: exit_kind.clone(),
                        exit_code,
                        message: Some(exit_message.clone()),
                        java_path: Some(java_executable_for_thread.clone()),
                        java_major: java_major_for_thread,
                        launch_log_path: Some(launch_log_path_for_thread.clone()),
                    },
                );
                if let Some(path) = runtime_session_cleanup_for_thread {
                    match reconcile_runtime_session_minecraft_settings(
                        &path,
                        &app_instance_dir_for_thread,
                    ) {
                        Ok(copied) if copied > 0 => {
                            eprintln!(
                                "native runtime session '{}' copied {} Minecraft settings file(s) back to '{}'",
                                path.display(),
                                copied,
                                app_instance_dir_for_thread.display()
                            );
                        }
                        Ok(_) => {}
                        Err(err) => {
                            eprintln!(
                                "native runtime session settings reconciliation failed for '{}' -> '{}': {}",
                                path.display(),
                                app_instance_dir_for_thread.display(),
                                err
                            );
                        }
                    }
                    let _ = remove_path_if_exists(&path);
                }
                emit_launch_state(
                    &app_for_thread,
                    &instance_id_for_thread,
                    Some(&launch_id_for_thread),
                    LaunchMethod::Native.as_str(),
                    "exited",
                    &exit_message,
                );
                if close_launcher_on_exit_for_thread {
                    app_for_thread.exit(0);
                    return;
                }
                if !keep_launcher_open_for_thread {
                    if let Some(window) = app_for_thread.get_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            });

            Ok(LaunchResult {
                method: "native".to_string(),
                launch_id: Some(launch_id),
                pid: Some(pid),
                prism_instance_id: None,
                prism_root: None,
                message: if use_isolated_runtime_session {
                    isolated_native_launch_success_message().to_string()
                } else if let Some(host) = quick_play_host.as_ref() {
                    format!(
                        "Native launch started (quick join {}:{}).",
                        host, quick_play_port
                    )
                } else {
                    "Native launch started.".to_string()
                },
            })
        }
    };

    match &launch_result {
        Ok(result) => {
            if matches!(method_for_report, LaunchMethod::Prism) {
                if let Err(err) = mark_instance_launch_exit(&instances_dir, &instance.id, "success")
                {
                    eprintln!(
                        "instance last-run metadata prism success marker write failed for '{}': {}",
                        instance.id, err
                    );
                }
                capture_run_report_best_effort(
                    &app,
                    crate::run_reports::CaptureRunReportInput {
                        instance_id: instance.id.clone(),
                        launch_method: LaunchMethod::Prism.as_str().to_string(),
                        exit_kind: "success".to_string(),
                        exit_code: None,
                        message: Some(result.message.clone()),
                        java_path: None,
                        java_major: None,
                        launch_log_path: None,
                    },
                );
            }
        }
        Err(err) => {
            let lower = err.to_lowercase();
            let exit_kind = if lower.contains("cancelled") {
                "stopped"
            } else {
                "crashed"
            };
            let short_reason = if lower.contains("java") {
                "java_runtime"
            } else if lower.contains("cancel") {
                "cancelled"
            } else if lower.contains("auth") || lower.contains("microsoft") {
                "auth"
            } else if lower.contains("preflight") || lower.contains("blocked") {
                "compatibility"
            } else {
                "unknown"
            };
            log_instance_event_best_effort(
                &app,
                &instance.id,
                "launch_failed",
                format!("Launch failed ({short_reason}): {err}"),
            );
            if let Err(mark_err) =
                mark_instance_launch_exit(&instances_dir, &instance.id, exit_kind)
            {
                eprintln!(
                    "instance last-run metadata error marker write failed for '{}': {}",
                    instance.id, mark_err
                );
            }
            let java_path = if !instance_settings.java_path.trim().is_empty() {
                Some(instance_settings.java_path.clone())
            } else if !settings.java_path.trim().is_empty() {
                Some(settings.java_path.clone())
            } else {
                None
            };
            capture_run_report_best_effort(
                &app,
                crate::run_reports::CaptureRunReportInput {
                    instance_id: instance.id.clone(),
                    launch_method: method_for_report.as_str().to_string(),
                    exit_kind: exit_kind.to_string(),
                    exit_code: None,
                    message: Some(err.clone()),
                    java_path,
                    java_major: None,
                    launch_log_path: None,
                },
            );
        }
    }

    launch_result
}

fn count_occurrences(text: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    text.match_indices(needle).count()
}

fn replace_with_count(
    text: String,
    needle: &str,
    replacement: &str,
    applied: &mut usize,
) -> String {
    if needle.is_empty() || !text.contains(needle) {
        return text;
    }
    *applied += count_occurrences(&text, needle);
    text.replace(needle, replacement)
}

fn is_uuid_like(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '-'));
    if t.len() != 36 {
        return false;
    }
    let bytes = t.as_bytes();
    for (idx, ch) in bytes.iter().enumerate() {
        let ok = if [8, 13, 18, 23].contains(&idx) {
            *ch == b'-'
        } else {
            (*ch as char).is_ascii_hexdigit()
        };
        if !ok {
            return false;
        }
    }
    true
}

fn is_ipv4_like(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !(c.is_ascii_digit() || c == '.'));
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    for part in parts {
        if part.is_empty() || part.len() > 3 {
            return false;
        }
        if let Ok(value) = part.parse::<u16>() {
            if value > 255 {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

fn redact_sensitive_text(input: &str) -> (String, usize) {
    let mut out = input.to_string();
    let mut redactions = 0usize;
    if let Some(home) = home_dir() {
        let home_text = home.display().to_string();
        if !home_text.trim().is_empty() {
            out = replace_with_count(out, &home_text, "<HOME>", &mut redactions);
        }
    }

    let token_keys = [
        "access_token",
        "refresh_token",
        "authorization",
        "bearer",
        "xuid",
        "session",
    ];

    let mut lines_out: Vec<String> = Vec::new();
    for raw_line in out.lines() {
        let mut line = raw_line.to_string();
        let lower = line.to_ascii_lowercase();
        for key in token_keys {
            if !lower.contains(key) {
                continue;
            }
            if let Some(pos) = line.find('=') {
                line = format!("{}=<REDACTED>", line[..pos].trim_end());
                redactions += 1;
                break;
            }
            if let Some(pos) = line.find(':') {
                line = format!("{}: <REDACTED>", line[..pos].trim_end());
                redactions += 1;
                break;
            }
        }

        let words: Vec<String> = line
            .split_whitespace()
            .map(|token| {
                if is_uuid_like(token) || is_ipv4_like(token) {
                    redactions += 1;
                    "[REDACTED]".to_string()
                } else {
                    token.to_string()
                }
            })
            .collect();
        lines_out.push(words.join(" "));
    }
    (lines_out.join("\n"), redactions)
}

fn write_zip_text(
    zip: &mut zip::ZipWriter<File>,
    path: &str,
    text: &str,
    opts: FileOptions,
    files_count: &mut usize,
) -> Result<(), String> {
    zip.start_file(path, opts)
        .map_err(|e| format!("zip write header failed for '{path}': {e}"))?;
    zip.write_all(text.as_bytes())
        .map_err(|e| format!("zip write failed for '{path}': {e}"))?;
    *files_count += 1;
    Ok(())
}

fn detect_duplicate_enabled_mod_filenames(
    lock: &Lockfile,
    instance_dir: &Path,
) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for entry in &lock.entries {
        if !entry.enabled || normalize_lock_content_type(&entry.content_type) != "mods" {
            continue;
        }
        let (enabled_path, _) = mod_paths(instance_dir, &entry.filename);
        if !enabled_path.exists() {
            continue;
        }
        let key = entry.filename.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        *counts.entry(key).or_insert(0) += 1;
    }
    let mut out: Vec<(String, usize)> =
        counts.into_iter().filter(|(_, count)| *count > 1).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[derive(Debug, Clone, Deserialize)]
struct KnownModConflictRule {
    id: String,
    mods: Vec<String>,
    message: String,
}

fn known_mod_conflict_rules() -> &'static Vec<KnownModConflictRule> {
    static RULES: OnceLock<Vec<KnownModConflictRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        serde_json::from_str(include_str!("../data/known_mod_conflicts.json")).unwrap_or_else(
            |err| {
                eprintln!("parse known mod conflicts failed: {err}");
                vec![]
            },
        )
    })
}

fn instance_loader_accepts_mod_loader_hint(instance_loader: &str, mod_loader_hint: &str) -> bool {
    let loader = instance_loader.trim().to_ascii_lowercase();
    let hint = mod_loader_hint.trim().to_ascii_lowercase();
    if loader.is_empty() || hint.is_empty() {
        return true;
    }
    match loader.as_str() {
        "fabric" => hint == "fabric" || hint == "quilt",
        "quilt" => hint == "quilt" || hint == "fabric",
        "forge" => hint == "forge",
        "neoforge" => hint == "neoforge",
        "vanilla" => hint == "vanilla",
        _ => loader == hint,
    }
}

fn collect_enabled_mod_analyses(
    lock: &Lockfile,
    instance_dir: &Path,
    instance_loader: &str,
) -> Vec<(String, LocalModAnalysis)> {
    let mut out: Vec<(String, LocalModAnalysis)> = Vec::new();
    for entry in &lock.entries {
        if !entry.enabled || normalize_lock_content_type(&entry.content_type) != "mods" {
            continue;
        }
        let (enabled_path, _) = mod_paths(instance_dir, &entry.filename);
        if !enabled_path.exists() {
            continue;
        }
        let analysis = if let Some(existing) = entry.local_analysis.clone() {
            existing
        } else {
            match fs::read(&enabled_path) {
                Ok(bytes) => {
                    analyze_local_mod_file(&entry.filename, &bytes, Some(instance_loader), None)
                }
                Err(_) => continue,
            }
        };
        out.push((entry.name.clone(), analysis));
    }
    out
}

fn detect_duplicate_enabled_mod_ids(
    analyses: &[(String, LocalModAnalysis)],
) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (_, analysis) in analyses {
        let unique_for_entry = analysis
            .mod_ids
            .iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();
        for mod_id in unique_for_entry {
            *counts.entry(mod_id).or_insert(0) += 1;
        }
    }
    let mut out = counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn detect_wrong_loader_enabled_mods(
    analyses: &[(String, LocalModAnalysis)],
    instance_loader: &str,
) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for (entry_name, analysis) in analyses {
        if analysis.loader_hints.is_empty() {
            continue;
        }
        if analysis
            .loader_hints
            .iter()
            .any(|hint| instance_loader_accepts_mod_loader_hint(instance_loader, hint))
        {
            continue;
        }
        out.push((entry_name.clone(), analysis.loader_hints.clone()));
    }
    out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    out
}

fn detect_missing_required_enabled_mod_dependencies(
    analyses: &[(String, LocalModAnalysis)],
) -> Vec<(String, Vec<String>)> {
    let installed_ids = analyses
        .iter()
        .flat_map(|(_, analysis)| analysis.mod_ids.iter())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>();
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for (entry_name, analysis) in analyses {
        let mut missing = analysis
            .required_dependencies
            .iter()
            .filter_map(|dep| normalize_local_mod_id(dep))
            .filter(|dep| !installed_ids.contains(dep))
            .collect::<Vec<_>>();
        missing.sort();
        missing.dedup();
        if !missing.is_empty() {
            out.push((entry_name.clone(), missing));
        }
    }
    out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    out
}

fn detect_known_enabled_mod_conflicts(
    analyses: &[(String, LocalModAnalysis)],
) -> Vec<(String, String, Vec<String>)> {
    let installed_ids = analyses
        .iter()
        .flat_map(|(_, analysis)| analysis.mod_ids.iter())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>();
    let mut out: Vec<(String, String, Vec<String>)> = Vec::new();
    for rule in known_mod_conflict_rules() {
        let required = rule
            .mods
            .iter()
            .filter_map(|value| normalize_local_mod_id(value))
            .collect::<Vec<_>>();
        if required.len() < 2 {
            continue;
        }
        if required.iter().all(|value| installed_ids.contains(value)) {
            out.push((rule.id.clone(), rule.message.clone(), required));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[tauri::command]
pub(crate) async fn trigger_instance_microphone_permission_prompt(
    app: tauri::AppHandle,
    args: PreflightLaunchCompatibilityArgs,
) -> Result<String, String> {
    run_blocking_task("trigger instance microphone permission prompt", move || {
        trigger_instance_microphone_permission_prompt_inner(&app, &args.instance_id)
    })
    .await
}

fn trigger_instance_microphone_permission_prompt_inner(
    app: &tauri::AppHandle,
    instance_id: &str,
) -> Result<String, String> {
    let instances_dir = app_instances_dir(app)?;
    let instance = find_instance(&instances_dir, instance_id)?;
    let instance_settings = normalize_instance_settings(instance.settings.clone());
    let settings = read_launcher_settings(app)?;
    let java_executable = if !instance_settings.java_path.trim().is_empty() {
        instance_settings.java_path.trim().to_string()
    } else {
        resolve_java_executable(&settings)?
    };
    crate::permissions::trigger_java_microphone_permission_prompt(&java_executable)
}

#[tauri::command]
pub(crate) fn open_microphone_system_settings() -> Result<String, String> {
    crate::permissions::open_microphone_system_settings()
}

#[tauri::command]
pub(crate) fn preflight_launch_compatibility(
    app: tauri::AppHandle,
    args: PreflightLaunchCompatibilityArgs,
) -> Result<LaunchCompatibilityReport, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_settings = normalize_instance_settings(instance.settings.clone());
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let settings = read_launcher_settings(&app)?;

    let mut items: Vec<LaunchCompatibilityItem> = Vec::new();
    let launch_method = args
        .method
        .as_deref()
        .and_then(LaunchMethod::parse)
        .unwrap_or(LaunchMethod::Native);
    let mut resolved_native_java_executable: Option<String> = None;
    if launch_method == LaunchMethod::Native {
        let required_java = required_java_major_for_mc(&instance.mc_version);
        let java_executable = if !instance_settings.java_path.trim().is_empty() {
            instance_settings.java_path.trim().to_string()
        } else {
            resolve_java_executable(&settings).unwrap_or_default()
        };
        if !java_executable.trim().is_empty() {
            resolved_native_java_executable = Some(java_executable.clone());
        }
        if java_executable.trim().is_empty() {
            items.push(LaunchCompatibilityItem {
                code: "JAVA_PATH_UNRESOLVED".to_string(),
                title: "Java runtime path missing".to_string(),
                message: "Could not resolve a Java executable for this instance.".to_string(),
                severity: "blocker".to_string(),
                blocking: true,
            });
        } else if let Ok((java_major, version_line)) = detect_java_major(&java_executable) {
            if java_major < required_java {
                items.push(LaunchCompatibilityItem {
                    code: "JAVA_VERSION_INCOMPATIBLE".to_string(),
                    title: "Java version is too old".to_string(),
                    message: format!(
                        "Java {java_major} detected ({version_line}), but Minecraft {} needs Java {}+.",
                        instance.mc_version, required_java
                    ),
                    severity: "blocker".to_string(),
                    blocking: true,
                });
            }
        } else {
            items.push(LaunchCompatibilityItem {
                code: "JAVA_VERSION_CHECK_FAILED".to_string(),
                title: "Could not verify Java version".to_string(),
                message: "Launch may fail until Java runtime is verified.".to_string(),
                severity: "warning".to_string(),
                blocking: false,
            });
        }
    }

    let mut missing_enabled_mods = 0usize;
    let mut missing_enabled_non_mods = 0usize;
    for entry in &lock.entries {
        if !entry.enabled {
            continue;
        }
        if !entry_file_exists(&instance_dir, entry) {
            if normalize_lock_content_type(&entry.content_type) == "mods" {
                missing_enabled_mods += 1;
            } else {
                missing_enabled_non_mods += 1;
            }
        }
    }

    let permission_eval = crate::permissions::evaluate_launch_permissions(
        &lock,
        launch_method == LaunchMethod::Native,
        resolved_native_java_executable.as_deref(),
    );
    for signal in &permission_eval.signals {
        items.push(LaunchCompatibilityItem {
            code: signal.code.to_string(),
            title: signal.title.clone(),
            message: signal.message.clone(),
            severity: signal.severity.to_string(),
            blocking: signal.blocking,
        });
    }
    if missing_enabled_mods > 0 {
        items.push(LaunchCompatibilityItem {
            code: "MISSING_ENABLED_MOD_FILES".to_string(),
            title: "Enabled mod file missing".to_string(),
            message: format!("{missing_enabled_mods} enabled mod entries are missing on disk."),
            severity: "blocker".to_string(),
            blocking: true,
        });
    }
    if missing_enabled_non_mods > 0 {
        items.push(LaunchCompatibilityItem {
            code: "MISSING_ENABLED_NONMOD_FILES".to_string(),
            title: "Some enabled non-mod content is missing".to_string(),
            message: format!(
                "{missing_enabled_non_mods} enabled non-mod entries are missing on disk. This usually does not block launch."
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    let duplicates = detect_duplicate_enabled_mod_filenames(&lock, &instance_dir);
    if !duplicates.is_empty() {
        let preview = duplicates
            .iter()
            .take(3)
            .map(|(name, count)| format!("{name} ({count})"))
            .collect::<Vec<_>>()
            .join(", ");
        items.push(LaunchCompatibilityItem {
            code: "DUPLICATE_MOD_FILENAMES".to_string(),
            title: "Possible duplicate enabled mods".to_string(),
            message: format!(
                "Detected duplicate enabled mod filenames: {preview}. This can cause odd behavior, but may still launch."
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    let enabled_mod_analyses = collect_enabled_mod_analyses(&lock, &instance_dir, &instance.loader);
    let duplicate_mod_ids = detect_duplicate_enabled_mod_ids(&enabled_mod_analyses);
    if !duplicate_mod_ids.is_empty() {
        let preview = duplicate_mod_ids
            .iter()
            .take(4)
            .map(|(mod_id, count)| format!("{mod_id} ({count})"))
            .collect::<Vec<_>>()
            .join(", ");
        items.push(LaunchCompatibilityItem {
            code: "DUPLICATE_MOD_IDS".to_string(),
            title: "Duplicate mod IDs detected".to_string(),
            message: format!(
                "Multiple enabled mods declare the same mod ID: {preview}. Disable duplicates to avoid undefined behavior."
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    let wrong_loader_mods =
        detect_wrong_loader_enabled_mods(&enabled_mod_analyses, &instance.loader);
    if !wrong_loader_mods.is_empty() {
        let preview = wrong_loader_mods
            .iter()
            .take(4)
            .map(|(entry, hints)| format!("{entry} [{}]", hints.join(", ")))
            .collect::<Vec<_>>()
            .join(", ");
        items.push(LaunchCompatibilityItem {
            code: "WRONG_LOADER_MODS".to_string(),
            title: "Enabled mods target a different loader".to_string(),
            message: format!(
                "Some enabled mods do not match instance loader '{}': {preview}.",
                instance.loader
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    let missing_required_mod_deps =
        detect_missing_required_enabled_mod_dependencies(&enabled_mod_analyses);
    if !missing_required_mod_deps.is_empty() {
        let preview = missing_required_mod_deps
            .iter()
            .take(3)
            .map(|(entry, deps)| format!("{entry} -> {}", deps.join(", ")))
            .collect::<Vec<_>>()
            .join(" | ");
        items.push(LaunchCompatibilityItem {
            code: "MISSING_REQUIRED_MOD_DEPENDENCIES".to_string(),
            title: "Required mod dependencies are missing".to_string(),
            message: format!(
                "One or more enabled mods are missing required dependencies: {preview}. Install the missing dependencies or disable the mod."
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    let known_conflicts = detect_known_enabled_mod_conflicts(&enabled_mod_analyses);
    if !known_conflicts.is_empty() {
        let preview = known_conflicts
            .iter()
            .take(3)
            .map(|(_, message, mods)| format!("{} [{}]", message, mods.join(", ")))
            .collect::<Vec<_>>()
            .join(" | ");
        items.push(LaunchCompatibilityItem {
            code: "KNOWN_MOD_CONFLICTS".to_string(),
            title: "Known mod conflict combinations detected".to_string(),
            message: format!(
                "Detected known mod conflicts among enabled mods: {preview}. Review compatibility notes before launching."
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    let unresolved_local_entries = lock
        .entries
        .iter()
        .filter(|entry| entry.source.trim().eq_ignore_ascii_case("local"))
        .count();
    if unresolved_local_entries > 0 {
        items.push(LaunchCompatibilityItem {
            code: "LOCAL_ENTRIES_UNRESOLVED".to_string(),
            title: "Some local imports are unresolved".to_string(),
            message: format!(
                "{unresolved_local_entries} local entries are still source:\"local\" and may hide dependency/update metadata."
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    if let Ok(store) = friend_link::store::read_store(&app) {
        if let Some(session) = friend_link::store::get_session(&store, &args.instance_id) {
            if !session.pending_conflicts.is_empty() {
                items.push(LaunchCompatibilityItem {
                    code: "FRIEND_LINK_PENDING_CONFLICTS".to_string(),
                    title: "Friend Link has pending conflicts".to_string(),
                    message: format!(
                        "{} conflicts pending; prelaunch reconcile may block launch.",
                        session.pending_conflicts.len()
                    ),
                    severity: "warning".to_string(),
                    blocking: false,
                });
            }
        }
    }

    let blocking_count = items.iter().filter(|item| item.blocking).count();
    let warning_count = items
        .iter()
        .filter(|item| !item.blocking && item.severity == "warning")
        .count();
    let status = if blocking_count > 0 {
        "blocked"
    } else if warning_count > 0 {
        "warning"
    } else {
        "ok"
    };

    Ok(LaunchCompatibilityReport {
        instance_id: args.instance_id,
        status: status.to_string(),
        checked_at: now_iso(),
        blocking_count,
        warning_count,
        unresolved_local_entries,
        items,
        permissions: permission_eval.checklist,
        mic_requirement: permission_eval.mic_requirement,
    })
}

#[tauri::command]
pub(crate) async fn export_instance_support_bundle(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: ExportInstanceSupportBundleArgs,
) -> Result<SupportBundleResult, String> {
    let output = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_SUPPORT_BUNDLE_EXPORT,
        &args.grant_id,
    )?;
    run_blocking_task("export instance support bundle", move || {
        export_instance_support_bundle_inner(app, args, output)
    })
    .await
}

fn export_instance_support_bundle_inner(
    app: tauri::AppHandle,
    args: ExportInstanceSupportBundleArgs,
    output: PathBuf,
) -> Result<SupportBundleResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let include_raw_logs = args.include_raw_logs.unwrap_or(false);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir export directory failed: {e}"))?;
    }

    let file = File::create(&output).map_err(|e| format!("create support bundle failed: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut files_count = 0usize;
    let mut redactions_applied = 0usize;

    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let installed = lock
        .entries
        .iter()
        .map(|entry| lock_entry_to_installed(&instance_dir, entry))
        .collect::<Vec<_>>();
    let installed_raw = serde_json::to_string_pretty(&installed)
        .map_err(|e| format!("serialize installed mods failed: {e}"))?;
    write_zip_text(
        &mut zip,
        "mods/installed_mods.json",
        &installed_raw,
        opts,
        &mut files_count,
    )?;

    let allowlist = friend_link::state::default_allowlist();
    let config_files = friend_link::state::collect_allowlisted_config_files(
        &instances_dir,
        &args.instance_id,
        &allowlist,
    )
    .unwrap_or_default();
    for file in &config_files {
        let (redacted, count) = redact_sensitive_text(&file.content);
        redactions_applied += count;
        write_zip_text(
            &mut zip,
            &format!("config/{}.redacted", file.path),
            &redacted,
            opts,
            &mut files_count,
        )?;
    }

    let log_targets = [
        ("logs/latest_launch", latest_launch_log_path(&instance_dir)),
        ("logs/latest_crash", latest_crash_report_path(&instance_dir)),
    ];
    for (base_name, maybe_path) in log_targets {
        let Some(path) = maybe_path else {
            continue;
        };
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let (redacted, count) = redact_sensitive_text(&raw);
        redactions_applied += count;
        write_zip_text(
            &mut zip,
            &format!("{base_name}.redacted.log"),
            &redacted,
            opts,
            &mut files_count,
        )?;
        if include_raw_logs {
            write_zip_text(
                &mut zip,
                &format!("{base_name}.raw.log"),
                &raw,
                opts,
                &mut files_count,
            )?;
        }
    }

    let perf_json = serde_json::to_string_pretty(&args.perf_actions)
        .map_err(|e| format!("serialize perf actions failed: {e}"))?;
    write_zip_text(
        &mut zip,
        "telemetry/perf_actions.json",
        &perf_json,
        opts,
        &mut files_count,
    )?;

    let manifest = serde_json::json!({
        "format": "openjar-support-bundle/v1",
        "generated_at": now_iso(),
        "instance": {
            "id": instance.id,
            "name": instance.name,
            "mc_version": instance.mc_version,
            "loader": instance.loader
        },
        "include_raw_logs": include_raw_logs,
        "files_count": files_count,
        "redactions_applied": redactions_applied,
        "config_files": config_files.len(),
        "mod_entries": installed.len(),
    });
    write_zip_text(
        &mut zip,
        "manifest.json",
        &serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("serialize manifest failed: {e}"))?,
        opts,
        &mut files_count,
    )?;

    zip.finish()
        .map_err(|e| format!("finalize support bundle failed: {e}"))?;

    Ok(SupportBundleResult {
        output_path: output.display().to_string(),
        files_count,
        redactions_applied,
        message: "Support bundle exported.".to_string(),
    })
}

#[tauri::command]
pub(crate) fn list_installed_mods(
    app: tauri::AppHandle,
    args: ListInstalledModsArgs,
) -> Result<Vec<InstalledMod>, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;

    let mut out: Vec<InstalledMod> = lock
        .entries
        .iter()
        .map(|e| lock_entry_to_installed(&instance_dir, e))
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

fn requested_content_type_filter(requested: Option<&[String]>) -> Option<HashSet<String>> {
    let values = requested?;
    let mut out: HashSet<String> = HashSet::new();
    for raw in values {
        let normalized = match raw.trim().to_ascii_lowercase().as_str() {
            "mods" | "mod" => Some("mods"),
            "resourcepacks" | "resourcepack" => Some("resourcepacks"),
            "shaderpacks" | "shaderpack" | "shaders" | "shader" => Some("shaderpacks"),
            "datapacks" | "datapack" => Some("datapacks"),
            "modpacks" | "modpack" => Some("modpacks"),
            _ => None,
        };
        if let Some(value) = normalized {
            out.insert(value.to_string());
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn prune_missing_entries_from_lock(
    lock: &mut Lockfile,
    instance_dir: &Path,
    content_type_filter: Option<&HashSet<String>>,
) -> Vec<String> {
    let mut removed_names: Vec<String> = Vec::new();
    let mut retained_entries: Vec<LockEntry> = Vec::with_capacity(lock.entries.len());

    for entry in lock.entries.drain(..) {
        let normalized_content_type = normalize_lock_content_type(&entry.content_type);
        let should_prune_check = content_type_filter
            .map(|allow| allow.contains(&normalized_content_type))
            .unwrap_or(true);
        if should_prune_check && !entry_file_exists(instance_dir, &entry) {
            removed_names.push(canonical_lock_entry_name(
                &normalized_content_type,
                &entry.filename,
                &entry.name,
            ));
            continue;
        }
        retained_entries.push(entry);
    }

    lock.entries = retained_entries;
    removed_names.sort_by_key(|name| name.to_ascii_lowercase());
    removed_names
}

#[tauri::command]
pub(crate) fn prune_missing_installed_entries(
    app: tauri::AppHandle,
    args: PruneMissingInstalledEntriesArgs,
) -> Result<PruneMissingInstalledEntriesResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let content_type_filter = requested_content_type_filter(args.content_types.as_deref());

    let removed_names =
        prune_missing_entries_from_lock(&mut lock, &instance_dir, content_type_filter.as_ref());
    let removed_count = removed_names.len();
    let remaining_count = lock.entries.len();

    if removed_count > 0 {
        write_lockfile(&instances_dir, &args.instance_id, &lock)?;
        log_instance_event_best_effort(
            &app,
            &args.instance_id,
            "content_prune_missing",
            format!(
                "Cleaned {} missing entr{} from lock metadata.",
                removed_count,
                if removed_count == 1 { "y" } else { "ies" }
            ),
        );
    }

    let filter_label = if let Some(filter) = content_type_filter {
        let mut labels = filter.into_iter().collect::<Vec<_>>();
        labels.sort();
        labels.join(",")
    } else {
        "all".to_string()
    };
    eprintln!(
        "prune missing entries summary for '{}': removed={}, remaining={}, filter={}",
        args.instance_id, removed_count, remaining_count, filter_label
    );

    Ok(PruneMissingInstalledEntriesResult {
        instance_id: args.instance_id,
        removed_count,
        remaining_count,
        removed_names,
    })
}

#[tauri::command]
pub(crate) fn set_installed_mod_enabled(
    app: tauri::AppHandle,
    args: SetInstalledModEnabledArgs,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;

    let idx = find_lock_entry_index(
        &lock,
        &args.version_id,
        args.content_type.as_deref(),
        args.filename.as_deref(),
    )?;

    let mut changed = false;
    {
        let entry = &mut lock.entries[idx];
        let content_type = normalize_lock_content_type(&entry.content_type);
        let content_label = content_type_display_name(&content_type);

        if entry.enabled != args.enabled {
            match content_type.as_str() {
                "mods" | "resourcepacks" | "shaderpacks" => {
                    let (enabled_path, disabled_path) = if content_type == "mods" {
                        mod_paths(&instance_dir, &entry.filename)
                    } else {
                        content_paths_for_type(&instance_dir, &content_type, &entry.filename)
                    };
                    if args.enabled {
                        if enabled_path.exists() {
                            // already in place
                        } else if disabled_path.exists() {
                            if enabled_path.exists() {
                                fs::remove_file(&enabled_path).map_err(|e| {
                                    format!("remove existing enabled file failed: {e}")
                                })?;
                            }
                            fs::rename(&disabled_path, &enabled_path)
                                .map_err(|e| format!("enable {} failed: {e}", content_label))?;
                        } else {
                            return Err(format!("{} file not found on disk", content_label));
                        }
                    } else if disabled_path.exists() {
                        // already disabled path
                    } else if enabled_path.exists() {
                        if disabled_path.exists() {
                            fs::remove_file(&disabled_path).map_err(|e| {
                                format!("remove existing disabled file failed: {e}")
                            })?;
                        }
                        fs::rename(&enabled_path, &disabled_path)
                            .map_err(|e| format!("disable {} failed: {e}", content_label))?;
                    } else {
                        return Err(format!("{} file not found on disk", content_label));
                    }
                }
                "datapacks" => {
                    let target_worlds = if entry.target_worlds.is_empty() {
                        list_instance_world_names(&instance_dir)?
                    } else {
                        entry.target_worlds.clone()
                    };
                    let mut found_any = false;
                    for world in &target_worlds {
                        let (enabled_path, disabled_path) =
                            datapack_world_paths(&instance_dir, world, &entry.filename);
                        if args.enabled {
                            if enabled_path.exists() {
                                found_any = true;
                                continue;
                            }
                            if disabled_path.exists() {
                                found_any = true;
                                fs::rename(&disabled_path, &enabled_path).map_err(|e| {
                                    format!(
                                        "enable {} failed for world '{}': {e}",
                                        content_label, world
                                    )
                                })?;
                            }
                        } else {
                            if disabled_path.exists() {
                                found_any = true;
                                continue;
                            }
                            if enabled_path.exists() {
                                found_any = true;
                                fs::rename(&enabled_path, &disabled_path).map_err(|e| {
                                    format!(
                                        "disable {} failed for world '{}': {e}",
                                        content_label, world
                                    )
                                })?;
                            }
                        }
                    }
                    if !found_any {
                        return Err(format!("{} file not found on disk", content_label));
                    }
                }
                _ => {
                    return Err("Enable/disable is not supported for this content type".to_string())
                }
            }

            entry.enabled = args.enabled;
            changed = true;
        }
    }

    if changed {
        write_lockfile(&instances_dir, &args.instance_id, &lock)?;
        log_instance_event_best_effort(
            &app,
            &args.instance_id,
            "content_toggle",
            format!(
                "{} '{}' ({}).",
                if args.enabled { "Enabled" } else { "Disabled" },
                lock.entries[idx].name,
                content_type_display_name(&normalize_lock_content_type(
                    &lock.entries[idx].content_type
                ))
            ),
        );
    }

    let entry = lock.entries[idx].clone();
    Ok(lock_entry_to_installed(&instance_dir, &entry))
}

#[tauri::command]
pub(crate) fn set_installed_mod_pin(
    app: tauri::AppHandle,
    args: SetInstalledModPinArgs,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;

    let idx = find_lock_entry_index(
        &lock,
        &args.version_id,
        args.content_type.as_deref(),
        args.filename.as_deref(),
    )?;

    let entry = &mut lock.entries[idx];
    let next_pin = match args.pin.as_ref() {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => Some(value.trim().to_string()),
        None => Some(entry.version_id.clone()),
    };
    let changed = entry.pinned_version != next_pin;
    entry.pinned_version = next_pin;
    if changed {
        write_lockfile(&instances_dir, &args.instance_id, &lock)?;
        log_instance_event_best_effort(
            &app,
            &args.instance_id,
            "content_pin",
            format!(
                "{} pin for '{}'.",
                if lock.entries[idx]
                    .pinned_version
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
                {
                    "Set"
                } else {
                    "Cleared"
                },
                lock.entries[idx].name
            ),
        );
    }

    Ok(lock_entry_to_installed(&instance_dir, &lock.entries[idx]))
}

#[tauri::command]
pub(crate) fn set_installed_mod_provider(
    app: tauri::AppHandle,
    args: SetInstalledModProviderArgs,
) -> Result<InstalledMod, String> {
    let mutation_lock = instance_mutation_lock(&args.instance_id);
    let _guard = mutation_lock
        .lock()
        .map_err(|_| "instance mutation lock poisoned".to_string())?;
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;

    let idx = find_lock_entry_index(
        &lock,
        &args.version_id,
        args.content_type.as_deref(),
        args.filename.as_deref(),
    )?;

    let requested_source = args.source.trim().to_ascii_lowercase();
    if requested_source.is_empty() {
        return Err("source is required".to_string());
    }

    let entry = &mut lock.entries[idx];
    if entry.provider_candidates.is_empty() {
        entry.provider_candidates = lock_entry_provider_candidates(entry);
    }

    let candidate = select_provider_candidate_for_source(entry, &requested_source)?;
    validate_provider_switch(entry, &requested_source, &candidate)?;

    apply_provider_candidate_to_lock_entry(entry, &candidate);
    entry.provider_candidates = compact_provider_candidates(entry.provider_candidates.clone());
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;

    let updated = lock.entries[idx].clone();
    Ok(lock_entry_to_installed(&instance_dir, &updated))
}

fn select_provider_candidate_for_source(
    entry: &LockEntry,
    requested_source: &str,
) -> Result<ProviderCandidate, String> {
    if requested_source == "github" {
        let github_candidates = entry
            .provider_candidates
            .iter()
            .filter(|item| item.source.trim().eq_ignore_ascii_case("github"))
            .collect::<Vec<_>>();
        if github_candidates.is_empty() {
            return Err("Requested provider is not available for this entry".to_string());
        }
        let distinct_projects = github_candidates
            .iter()
            .map(|item| item.project_id.trim().to_ascii_lowercase())
            .collect::<HashSet<_>>();
        if entry.source.trim().eq_ignore_ascii_case("local") && distinct_projects.len() > 1 {
            return Err(
                "Multiple GitHub candidates were found for this local mod. Reattach GitHub to choose the correct repository before switching providers."
                    .to_string(),
            );
        }
        return github_candidates
            .iter()
            .find(|item| provider_candidate_is_auto_activatable(item))
            .or_else(|| {
                github_candidates.iter().find(|item| {
                    item.version_id
                        .trim()
                        .to_ascii_lowercase()
                        .starts_with("gh_release:")
                })
            })
            .or_else(|| github_candidates.first())
            .cloned()
            .cloned()
            .ok_or_else(|| "Requested provider is not available for this entry".to_string());
    }
    entry
        .provider_candidates
        .iter()
        .find(|item| item.source.trim().eq_ignore_ascii_case(requested_source))
        .cloned()
        .ok_or_else(|| "Requested provider is not available for this entry".to_string())
}

fn validate_provider_switch(
    entry: &LockEntry,
    requested_source: &str,
    candidate: &ProviderCandidate,
) -> Result<(), String> {
    if requested_source != "github" {
        return Ok(());
    }
    if parse_github_project_id(&candidate.project_id).is_err() {
        return Err(
            "GitHub provider switch blocked: repository ID is invalid. Reattach GitHub with a valid owner/repo first."
                .to_string(),
        );
    }
    let explicit_local_override = entry.source.trim().eq_ignore_ascii_case("local");
    let confidence = candidate
        .confidence
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let verification_status = candidate
        .verification_status
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let reason = candidate
        .reason
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let activation_safe = provider_candidate_is_auto_activatable(candidate);
    let manual_verified = confidence == "manual" && !reason.contains("unverified");
    let pending_verification = matches!(
        verification_status.as_str(),
        "manual_unverified" | "unavailable" | "deferred"
    ) || (confidence == "manual" && reason.contains("unverified"));
    if pending_verification && !explicit_local_override {
        return Err(
            "This GitHub repository mapping is pending verification. Re-run Attach GitHub when GitHub API is available before switching providers.".to_string()
        );
    }
    if !activation_safe && !manual_verified && !explicit_local_override {
        return Err(
            "GitHub provider switch blocked: this mapping is medium/low confidence and stays as a non-active candidate. Reattach or re-identify until confidence is high/deterministic."
                .to_string(),
        );
    }
    Ok(())
}

fn provider_candidate_identity_key(candidate: &ProviderCandidate) -> String {
    format!(
        "{}:{}",
        candidate.source.trim().to_ascii_lowercase(),
        candidate.project_id.trim().to_ascii_lowercase()
    )
}

fn upsert_provider_candidate(entry: &mut LockEntry, candidate: ProviderCandidate) {
    if entry.provider_candidates.is_empty() {
        entry.provider_candidates = lock_entry_provider_candidates(entry);
    }
    let key = provider_candidate_identity_key(&candidate);
    let mut replaced = false;
    for existing in &mut entry.provider_candidates {
        if provider_candidate_identity_key(existing) == key {
            *existing = candidate.clone();
            replaced = true;
            break;
        }
    }
    if !replaced {
        entry.provider_candidates.push(candidate);
    }
    entry.provider_candidates = compact_provider_candidates(entry.provider_candidates.clone());
}

fn lock_entry_allows_manual_github_attach(entry: &LockEntry) -> bool {
    if normalize_lock_content_type(&entry.content_type) != "mods" {
        return false;
    }
    entry.source.trim().eq_ignore_ascii_case("local")
        || entry.source.trim().eq_ignore_ascii_case("github")
}

#[tauri::command]
pub(crate) fn attach_installed_mod_github_repo(
    app: tauri::AppHandle,
    args: AttachInstalledModGithubRepoArgs,
) -> Result<InstalledMod, String> {
    let mutation_lock = instance_mutation_lock(&args.instance_id);
    let _guard = mutation_lock
        .lock()
        .map_err(|_| "instance mutation lock poisoned".to_string())?;
    let instances_dir = app_instances_dir(&app)?;
    let _instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let activate = args.activate.unwrap_or(true);

    let idx = find_lock_entry_index(
        &lock,
        &args.version_id,
        args.content_type.as_deref(),
        args.filename.as_deref(),
    )?;

    let github_repo = args.github_repo.trim();
    if github_repo.is_empty() {
        return Err("GitHub repository is required (owner/repo or URL).".to_string());
    }
    let (owner, repo_name) = parse_github_project_id(github_repo)?;
    let project_key = github_project_key(&owner, &repo_name);
    let client = build_http_client()?;

    let mut repo_for_title: Option<GithubRepository> = None;
    let repo_fetch = fetch_github_repo(&client, &owner, &repo_name);
    match repo_fetch {
        Ok(repo) => {
            if let Some(reason) = github_repo_policy_rejection_reason(&repo) {
                return Err(format!(
                    "GitHub repository rejected by safety policy: {reason}."
                ));
            }
            repo_for_title = Some(repo);
        }
        Err(err) => {
            let auth_or_limit = github_error_is_auth_or_rate_limit(&err);
            if !auth_or_limit {
                return Err(err);
            }
            eprintln!(
                "github repo attach proceeding without repo metadata due to API auth/rate-limit error: {}",
                err
            );
        }
    }
    let can_activate = activate;

    let attached_name = {
        let entry = &mut lock.entries[idx];
        if !lock_entry_allows_manual_github_attach(entry) {
            return Err(
                "Manual GitHub repo attach is available only for local or existing GitHub entries."
                    .to_string(),
            );
        }

        let mut candidate = ProviderCandidate {
            source: "github".to_string(),
            project_id: project_key.clone(),
            version_id: "gh_repo_unverified".to_string(),
            name: if entry.name.trim().is_empty() {
                repo_for_title
                    .as_ref()
                    .map(github_repo_title)
                    .unwrap_or_else(|| format!("{owner}/{repo_name}"))
            } else {
                entry.name.clone()
            },
            version_number: if entry.version_number.trim().is_empty() {
                "unknown".to_string()
            } else {
                entry.version_number.clone()
            },
            confidence: Some("manual".to_string()),
            reason: Some(if repo_for_title.is_some() {
                "Manual GitHub repository attachment saved pending release verification."
                    .to_string()
            } else {
                "Manual GitHub repository attachment (unverified; GitHub API unavailable)."
                    .to_string()
            }),
            verification_status: Some(if repo_for_title.is_some() {
                "manual_unverified".to_string()
            } else {
                "unavailable".to_string()
            }),
        };

        let mut matched_hashes: Option<HashMap<String, String>> = None;
        if let Some(read_path) = local_entry_file_read_path(&instance_dir, entry)? {
            let file_bytes = fs::read(&read_path).map_err(|e| {
                format!(
                    "read local mod file for GitHub attach '{}' failed: {e}",
                    read_path.display()
                )
            })?;
            let github_matches = detect_provider_matches_for_local_mod(
                &client,
                &file_bytes,
                &entry.filename,
                true,
                Some(&project_key),
            )
            .into_iter()
            .filter(|found| {
                found.source.trim().eq_ignore_ascii_case("github")
                    && found.project_id.trim().eq_ignore_ascii_case(&project_key)
            })
            .collect::<Vec<_>>();
            if let Some(found) = select_preferred_provider_match(&github_matches, Some("github")) {
                candidate = found.to_provider_candidate();
                if !found.hashes.is_empty() {
                    matched_hashes = Some(found.hashes.clone());
                }
            } else if let Some(reason) =
                provider_matches_have_transient_github_verification_issue(&github_matches)
            {
                candidate.reason = Some(format!(
                    "Manual GitHub repository attachment saved, but release verification is unavailable right now ({}).",
                    reason
                ));
                candidate.verification_status = Some("unavailable".to_string());
            } else {
                candidate.reason = Some(
                    "Manual GitHub repository attachment saved, but no verified release asset matched the local file yet."
                        .to_string(),
                );
                candidate.verification_status = Some("manual_unverified".to_string());
            }
        }

        upsert_provider_candidate(entry, candidate.clone());
        if can_activate && provider_candidate_is_auto_activatable(&candidate) {
            apply_provider_candidate_to_lock_entry(entry, &candidate);
        }
        if let Some(hashes) = matched_hashes {
            entry.hashes = hashes;
        }
        entry.name.clone()
    };

    let activation_applied = can_activate
        && lock.entries[idx]
            .source
            .trim()
            .eq_ignore_ascii_case("github")
        && lock.entries[idx]
            .project_id
            .trim()
            .eq_ignore_ascii_case(&project_key);
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;
    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "provider_attach",
        format!(
            "Saved GitHub repository hint '{}' for '{}'{}.",
            project_key,
            attached_name,
            if activation_applied {
                " and activated the GitHub provider"
            } else {
                " (verification is still pending before provider activation)"
            }
        ),
    );

    let updated = lock.entries[idx].clone();
    Ok(lock_entry_to_installed(&instance_dir, &updated))
}

#[tauri::command]
pub(crate) fn remove_installed_mod(
    app: tauri::AppHandle,
    args: RemoveInstalledModArgs,
) -> Result<InstalledMod, String> {
    let mutation_lock = instance_mutation_lock(&args.instance_id);
    let _guard = mutation_lock
        .lock()
        .map_err(|_| "instance mutation lock poisoned".to_string())?;
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;

    let idx = find_lock_entry_index(
        &lock,
        &args.version_id,
        args.content_type.as_deref(),
        args.filename.as_deref(),
    )?;
    let entry = lock.entries.remove(idx);
    let content_type = normalize_lock_content_type(&entry.content_type);

    match content_type.as_str() {
        "mods" | "resourcepacks" | "shaderpacks" => {
            let (enabled_path, disabled_path) = if content_type == "mods" {
                mod_paths(&instance_dir, &entry.filename)
            } else {
                content_paths_for_type(&instance_dir, &content_type, &entry.filename)
            };
            if enabled_path.exists() {
                fs::remove_file(&enabled_path).map_err(|e| {
                    format!(
                        "remove {} file '{}' failed: {e}",
                        content_type_display_name(&content_type),
                        enabled_path.display()
                    )
                })?;
            }
            if disabled_path.exists() {
                fs::remove_file(&disabled_path).map_err(|e| {
                    format!(
                        "remove disabled {} file '{}' failed: {e}",
                        content_type_display_name(&content_type),
                        disabled_path.display()
                    )
                })?;
            }
        }
        "datapacks" => {
            let target_worlds = if entry.target_worlds.is_empty() {
                list_instance_world_names(&instance_dir)?
            } else {
                entry.target_worlds.clone()
            };
            for world in &target_worlds {
                let (enabled_path, disabled_path) =
                    datapack_world_paths(&instance_dir, world, &entry.filename);
                if enabled_path.exists() {
                    fs::remove_file(&enabled_path).map_err(|e| {
                        format!(
                            "remove datapack file '{}' failed: {e}",
                            enabled_path.display()
                        )
                    })?;
                }
                if disabled_path.exists() {
                    fs::remove_file(&disabled_path).map_err(|e| {
                        format!(
                            "remove disabled datapack file '{}' failed: {e}",
                            disabled_path.display()
                        )
                    })?;
                }
            }
        }
        _ => {
            return Err("Delete is not supported for this content type".to_string());
        }
    }

    write_lockfile(&instances_dir, &args.instance_id, &lock)?;
    log_instance_event_best_effort(
        &app,
        &args.instance_id,
        "content_remove",
        format!(
            "Removed {} '{}'.",
            content_type_display_name(&content_type),
            entry.name
        ),
    );
    Ok(lock_entry_to_installed(&instance_dir, &entry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("openjar-{label}-{nanos}"))
    }

    fn sample_entry(source: &str, filename: &str, enabled: bool) -> LockEntry {
        LockEntry {
            source: source.to_string(),
            project_id: format!("{source}:{filename}"),
            version_id: format!("v-{filename}"),
            name: filename.to_string(),
            version_number: "1".to_string(),
            filename: filename.to_string(),
            content_type: "mods".to_string(),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            pinned_version: None,
            enabled,
            hashes: HashMap::new(),
            provider_candidates: vec![],
            local_analysis: None,
        }
    }

    fn sample_entry_with_content_type(
        source: &str,
        filename: &str,
        enabled: bool,
        content_type: &str,
    ) -> LockEntry {
        let mut entry = sample_entry(source, filename, enabled);
        entry.content_type = normalize_lock_content_type(content_type);
        entry.target_scope = if entry.content_type == "datapacks" {
            "world".to_string()
        } else {
            "instance".to_string()
        };
        entry
    }

    fn sample_instance(id: &str, name: &str) -> Instance {
        Instance {
            id: id.to_string(),
            name: name.to_string(),
            origin: "custom".to_string(),
            folder_name: None,
            mc_version: "1.20.1".to_string(),
            loader: "fabric".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            icon_path: None,
            settings: InstanceSettings::default(),
        }
    }

    fn sample_provider_match(
        source: &str,
        confidence: &str,
        reason: &str,
    ) -> LocalImportedProviderMatch {
        LocalImportedProviderMatch {
            source: source.to_string(),
            project_id: "gh:owner/repo".to_string(),
            version_id: "gh_repo_unverified".to_string(),
            name: "Sample".to_string(),
            version_number: "unverified".to_string(),
            hashes: HashMap::new(),
            confidence: confidence.to_string(),
            reason: reason.to_string(),
            verification_status: "manual_unverified".to_string(),
        }
    }

    fn sample_update(source: &str, project_id: &str, dependencies: Vec<&str>) -> ContentUpdateInfo {
        ContentUpdateInfo {
            source: source.to_string(),
            content_type: "mods".to_string(),
            project_id: project_id.to_string(),
            name: project_id.to_string(),
            current_version_id: "current".to_string(),
            current_version_number: "1.0.0".to_string(),
            latest_version_id: "latest".to_string(),
            latest_version_number: "2.0.0".to_string(),
            enabled: true,
            target_worlds: vec![],
            latest_file_name: None,
            latest_download_url: None,
            latest_hashes: HashMap::new(),
            required_dependencies: dependencies.into_iter().map(str::to_string).collect(),
            compatibility_status: Some("compatible".to_string()),
            compatibility_notes: vec![],
        }
    }

    #[test]
    fn transient_github_verification_issue_detected_from_manual_candidate_reason() {
        let matches = vec![sample_provider_match(
            "github",
            "manual",
            "GitHub local identify manual candidate: direct metadata repo hint matched, but release verification is unavailable (GitHub API rate limit reached).",
        )];
        let reason = provider_matches_have_transient_github_verification_issue(&matches);
        assert!(reason.is_some());
    }

    #[test]
    fn non_transient_manual_candidate_does_not_block_revert_logic() {
        let matches = vec![sample_provider_match(
            "github",
            "manual",
            "GitHub local identify manual candidate: direct metadata repo hint matched, but no verified release asset matched the local file.",
        )];
        let reason = provider_matches_have_transient_github_verification_issue(&matches);
        assert!(reason.is_none());
    }

    #[test]
    fn transient_github_verification_issue_detected_for_currently_unverifiable_reason() {
        let matches = vec![sample_provider_match(
            "github",
            "manual",
            "GitHub local identify manual candidate: direct metadata repo hint found, but release evidence is currently unverifiable.",
        )];
        let reason = provider_matches_have_transient_github_verification_issue(&matches);
        assert!(reason.is_some());
    }

    #[test]
    fn manual_github_attach_allowed_only_for_local_or_github_mod_entries() {
        let local_mod = sample_entry("local", "a.jar", true);
        let github_mod = sample_entry("github", "b.jar", true);
        let modrinth_mod = sample_entry("modrinth", "c.jar", true);
        let local_resourcepack =
            sample_entry_with_content_type("local", "pack.zip", true, "resourcepacks");
        assert!(lock_entry_allows_manual_github_attach(&local_mod));
        assert!(lock_entry_allows_manual_github_attach(&github_mod));
        assert!(!lock_entry_allows_manual_github_attach(&modrinth_mod));
        assert!(!lock_entry_allows_manual_github_attach(&local_resourcepack));
    }

    #[test]
    fn github_mapping_trust_check_rejects_invalid_project_ids() {
        let mut invalid_entry = sample_entry("github", "broken.jar", true);
        invalid_entry.project_id = "gh:https://meteorclient.com".to_string();
        invalid_entry.provider_candidates = vec![ProviderCandidate {
            source: "github".to_string(),
            project_id: "gh:https://meteorclient.com".to_string(),
            version_id: "gh_release:123".to_string(),
            name: "Invalid mapping".to_string(),
            version_number: "1.0.0".to_string(),
            confidence: Some("deterministic".to_string()),
            reason: Some("legacy invalid mapping".to_string()),
            verification_status: Some("verified".to_string()),
        }];
        assert!(lock_entry_has_untrusted_github_mapping(&invalid_entry));

        let mut valid_entry = sample_entry("github", "ok.jar", true);
        valid_entry.project_id = "gh:MeteorDevelopment/meteor-client".to_string();
        valid_entry.provider_candidates = vec![ProviderCandidate {
            source: "github".to_string(),
            project_id: "gh:MeteorDevelopment/meteor-client".to_string(),
            version_id: "gh_release:123".to_string(),
            name: "Valid mapping".to_string(),
            version_number: "1.0.0".to_string(),
            confidence: Some("deterministic".to_string()),
            reason: Some("verified".to_string()),
            verification_status: Some("verified".to_string()),
        }];
        assert!(!lock_entry_has_untrusted_github_mapping(&valid_entry));
    }

    #[test]
    fn validate_provider_switch_allows_explicit_local_override_for_unverified_github() {
        let mut entry = sample_entry("local", "candidate.jar", true);
        let candidate = ProviderCandidate {
            source: "github".to_string(),
            project_id: "gh:example/repo".to_string(),
            version_id: "gh_repo_unverified".to_string(),
            name: "Example Repo".to_string(),
            version_number: "unverified".to_string(),
            confidence: Some("manual".to_string()),
            reason: Some(
                "GitHub local identify manual candidate: direct metadata repo hint matched, but release evidence is currently unverified."
                    .to_string(),
            ),
            verification_status: Some("manual_unverified".to_string()),
        };
        entry.provider_candidates = vec![candidate.clone()];
        let selected = select_provider_candidate_for_source(&entry, "github")
            .expect("select github candidate");
        assert!(validate_provider_switch(&entry, "github", &selected).is_ok());
    }

    #[test]
    fn validate_provider_switch_blocks_unverified_when_not_local_override() {
        let mut entry = sample_entry("github", "candidate.jar", true);
        let candidate = ProviderCandidate {
            source: "github".to_string(),
            project_id: "gh:example/repo".to_string(),
            version_id: "gh_repo_unverified".to_string(),
            name: "Example Repo".to_string(),
            version_number: "unverified".to_string(),
            confidence: Some("manual".to_string()),
            reason: Some(
                "GitHub local identify manual candidate: direct metadata repo hint matched, but release evidence is currently unverified."
                    .to_string(),
            ),
            verification_status: Some("manual_unverified".to_string()),
        };
        entry.provider_candidates = vec![candidate.clone()];
        let selected = select_provider_candidate_for_source(&entry, "github")
            .expect("select github candidate");
        let err = validate_provider_switch(&entry, "github", &selected)
            .expect_err("non-local override should stay blocked");
        assert!(err.contains("pending verification"));
    }

    #[test]
    fn select_provider_candidate_for_source_blocks_ambiguous_local_github_projects() {
        let mut entry = sample_entry("local", "candidate.jar", true);
        entry.provider_candidates = vec![
            ProviderCandidate {
                source: "github".to_string(),
                project_id: "gh:owner/repo-a".to_string(),
                version_id: "gh_release:1".to_string(),
                name: "Repo A".to_string(),
                version_number: "1".to_string(),
                confidence: Some("high".to_string()),
                reason: Some("test".to_string()),
                verification_status: Some("verified".to_string()),
            },
            ProviderCandidate {
                source: "github".to_string(),
                project_id: "gh:owner/repo-b".to_string(),
                version_id: "gh_release:2".to_string(),
                name: "Repo B".to_string(),
                version_number: "1".to_string(),
                confidence: Some("high".to_string()),
                reason: Some("test".to_string()),
                verification_status: Some("verified".to_string()),
            },
        ];
        let err = select_provider_candidate_for_source(&entry, "github")
            .expect_err("ambiguous candidates should be blocked");
        assert!(err.contains("Multiple GitHub candidates were found"));
    }

    #[test]
    fn mod_filename_identity_key_normalizes_essential_variants() {
        assert_eq!(
            mod_filename_identity_key("Essential-1.20.1.jar"),
            mod_filename_identity_key("Essential (forge_1.20.1).jar")
        );
        assert_eq!(
            mod_filename_identity_key("geckolib-forge-1.20.1-4.8.3.jar"),
            Some("geckolib".to_string())
        );
    }

    #[test]
    fn update_dependency_order_prefers_required_dependencies_first() {
        let updates = vec![
            sample_update("modrinth", "mod-a", vec!["mod-b"]),
            sample_update("modrinth", "mod-b", vec![]),
            sample_update("modrinth", "mod-c", vec!["mod-b"]),
        ];
        let (order, warnings) = order_updates_by_required_dependencies(&updates);
        assert!(warnings.is_empty());
        let ordered_ids = order
            .into_iter()
            .filter_map(|idx| updates.get(idx))
            .map(|update| update.project_id.clone())
            .collect::<Vec<_>>();
        let pos_a = ordered_ids
            .iter()
            .position(|id| id == "mod-a")
            .expect("mod-a");
        let pos_b = ordered_ids
            .iter()
            .position(|id| id == "mod-b")
            .expect("mod-b");
        let pos_c = ordered_ids
            .iter()
            .position(|id| id == "mod-c")
            .expect("mod-c");
        assert!(pos_b < pos_a);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn update_dependency_cycle_adds_warning_and_keeps_deterministic_fallback() {
        let updates = vec![
            sample_update("modrinth", "mod-a", vec!["mod-b"]),
            sample_update("modrinth", "mod-b", vec!["mod-a"]),
        ];
        let (order, warnings) = order_updates_by_required_dependencies(&updates);
        assert_eq!(order, vec![0, 1]);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("dependency cycle"));
    }

    #[test]
    fn missing_required_dependencies_for_update_reports_missing_ids() {
        let lock = Lockfile {
            version: 2,
            entries: vec![sample_entry("modrinth", "mod-a.jar", true)],
        };
        let update = sample_update("modrinth", "mod-a", vec!["dep-one", "dep-two"]);
        let missing = missing_required_dependencies_for_update(&lock, &update);
        assert_eq!(missing, vec!["dep-one".to_string(), "dep-two".to_string()]);
    }

    #[test]
    fn known_mod_conflicts_detect_matching_rules() {
        let analyses = vec![
            (
                "OptiFine".to_string(),
                LocalModAnalysis {
                    loader_hints: vec!["forge".to_string()],
                    mod_ids: vec!["optifine".to_string()],
                    required_dependencies: vec![],
                    warnings: vec![],
                    suggestions: vec![],
                    scanned_at: now_iso(),
                },
            ),
            (
                "Sodium".to_string(),
                LocalModAnalysis {
                    loader_hints: vec!["fabric".to_string()],
                    mod_ids: vec!["sodium".to_string()],
                    required_dependencies: vec![],
                    warnings: vec![],
                    suggestions: vec![],
                    scanned_at: now_iso(),
                },
            ),
        ];
        let conflicts = detect_known_enabled_mod_conflicts(&analyses);
        assert!(!conflicts.is_empty());
        assert!(conflicts
            .iter()
            .any(|(_, _, mods)| mods.contains(&"optifine".to_string())
                && mods.contains(&"sodium".to_string())));
    }

    #[test]
    fn detect_mod_filename_key_collisions_groups_variants() {
        let collisions = detect_mod_filename_key_collisions(&[
            "Essential-1.20.1.jar".to_string(),
            "Essential (forge_1.20.1).jar".to_string(),
            "another-mod.jar".to_string(),
        ]);
        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0].0, "essential");
        assert_eq!(collisions[0].1.len(), 2);
    }

    #[test]
    fn remove_conflicting_local_mod_entries_replaces_local_duplicates() {
        let root = temp_path("local-conflict");
        let mods_dir = root.join("mods");
        fs::create_dir_all(&mods_dir).expect("create mods dir");
        fs::write(mods_dir.join("Essential-1.20.1.jar"), b"provider").expect("write provider jar");
        fs::write(mods_dir.join("Essential (forge_1.20.1).jar"), b"local")
            .expect("write local jar");
        fs::write(mods_dir.join("other.jar"), b"other").expect("write other jar");

        let mut lock = Lockfile {
            version: 2,
            entries: vec![
                sample_entry("local", "Essential (forge_1.20.1).jar", true),
                sample_entry("local", "other.jar", true),
                sample_entry("curseforge", "Essential-1.20.1.jar", true),
            ],
        };

        let removed = remove_conflicting_local_mod_entries_for_filename(
            &mut lock,
            &root,
            "Essential-1.20.1.jar",
        )
        .expect("remove conflicting local entries");

        assert_eq!(removed.len(), 1);
        assert!(!mods_dir.join("Essential (forge_1.20.1).jar").exists());
        assert!(mods_dir.join("Essential-1.20.1.jar").exists());
        assert_eq!(lock.entries.len(), 2);
        assert!(lock
            .entries
            .iter()
            .all(|entry| entry.filename != "Essential (forge_1.20.1).jar"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn requested_content_type_filter_ignores_unknown_values() {
        let requested = vec![
            "mods".to_string(),
            "shaders".to_string(),
            "unknown".to_string(),
        ];
        let filter = requested_content_type_filter(Some(&requested)).expect("filter");
        assert!(filter.contains("mods"));
        assert!(filter.contains("shaderpacks"));
        assert_eq!(filter.len(), 2);

        let unknown = vec!["???".to_string()];
        assert!(requested_content_type_filter(Some(&unknown)).is_none());
    }

    #[test]
    fn prune_missing_entries_from_lock_respects_content_filter() {
        let root = temp_path("prune-missing");
        fs::create_dir_all(root.join("mods")).expect("create mods dir");
        fs::create_dir_all(root.join("resourcepacks")).expect("create resourcepacks dir");

        fs::write(root.join("mods").join("present-mod.jar"), b"present").expect("seed present mod");

        let mut lock = Lockfile {
            version: 2,
            entries: vec![
                sample_entry_with_content_type("local", "present-mod.jar", true, "mods"),
                sample_entry_with_content_type("local", "missing-mod.jar", true, "mods"),
                sample_entry_with_content_type("local", "missing-pack.zip", true, "resourcepacks"),
            ],
        };

        let filter = HashSet::from(["mods".to_string()]);
        let removed_mods = prune_missing_entries_from_lock(&mut lock, &root, Some(&filter));
        assert_eq!(removed_mods, vec!["missing-mod".to_string()]);
        assert_eq!(lock.entries.len(), 2);
        assert!(lock
            .entries
            .iter()
            .any(|entry| entry.filename == "present-mod.jar"));
        assert!(lock
            .entries
            .iter()
            .any(|entry| entry.filename == "missing-pack.zip"));

        let removed_remaining = prune_missing_entries_from_lock(&mut lock, &root, None);
        assert_eq!(removed_remaining, vec!["missing-pack.zip".to_string()]);
        assert_eq!(lock.entries.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sync_instance_minecraft_settings_respects_target_mode() {
        let instances_dir = temp_path("sync-minecraft-settings");
        fs::create_dir_all(&instances_dir).expect("create instances dir");

        let source = sample_instance("source", "Source");
        let target_a = sample_instance("target-a", "Target A");
        let target_b = sample_instance("target-b", "Target B");
        let idx = InstanceIndex {
            instances: vec![source.clone(), target_a.clone(), target_b.clone()],
        };
        write_index(&instances_dir, &idx).expect("write index");

        let source_dir = instance_dir_for_instance(&instances_dir, &source);
        fs::create_dir_all(&source_dir).expect("create source dir");
        fs::write(source_dir.join("options.txt"), b"gamma:0.8").expect("write options");
        fs::write(source_dir.join("optionsof.txt"), b"optifine=true").expect("write optionsof");
        fs::write(source_dir.join("servers.dat"), b"servers").expect("write servers");
        fs::write(source_dir.join("notes.txt"), b"do-not-sync").expect("write unrelated");

        let target_a_dir = instance_dir_for_id(&instances_dir, &target_a.id).expect("target a dir");
        fs::create_dir_all(&target_a_dir).expect("create target a dir");
        let target_b_dir = instance_dir_for_id(&instances_dir, &target_b.id).expect("target b dir");
        fs::create_dir_all(&target_b_dir).expect("create target b dir");

        let all_targets = InstanceSettings {
            sync_minecraft_settings: true,
            sync_minecraft_settings_target: "all".to_string(),
            ..InstanceSettings::default()
        };
        let copied_all =
            sync_instance_minecraft_settings_before_launch(&instances_dir, &source, &all_targets)
                .expect("sync all");
        assert_eq!(copied_all, 6);
        assert_eq!(
            fs::read_to_string(target_a_dir.join("options.txt")).expect("read target a options"),
            "gamma:0.8"
        );
        assert_eq!(
            fs::read_to_string(target_b_dir.join("optionsof.txt"))
                .expect("read target b optionsof"),
            "optifine=true"
        );
        assert_eq!(
            fs::read_to_string(target_b_dir.join("servers.dat")).expect("read target b servers"),
            "servers"
        );
        assert!(!target_a_dir.join("notes.txt").exists());
        assert!(!target_b_dir.join("notes.txt").exists());

        fs::remove_file(target_a_dir.join("options.txt")).expect("clear target a options");
        fs::remove_file(target_a_dir.join("optionsof.txt")).expect("clear target a optionsof");
        fs::remove_file(target_a_dir.join("servers.dat")).expect("clear target a servers");
        fs::remove_file(target_b_dir.join("options.txt")).expect("clear target b options");
        fs::remove_file(target_b_dir.join("optionsof.txt")).expect("clear target b optionsof");
        fs::remove_file(target_b_dir.join("servers.dat")).expect("clear target b servers");

        let specific_target = InstanceSettings {
            sync_minecraft_settings: true,
            sync_minecraft_settings_target: target_a.id.clone(),
            ..InstanceSettings::default()
        };
        let copied_specific = sync_instance_minecraft_settings_before_launch(
            &instances_dir,
            &source,
            &specific_target,
        )
        .expect("sync specific");
        assert_eq!(copied_specific, 3);
        assert!(target_a_dir.join("options.txt").exists());
        assert!(target_a_dir.join("optionsof.txt").exists());
        assert!(target_a_dir.join("servers.dat").exists());
        assert!(!target_b_dir.join("options.txt").exists());
        assert!(!target_b_dir.join("optionsof.txt").exists());
        assert!(!target_b_dir.join("servers.dat").exists());

        let _ = fs::remove_dir_all(&instances_dir);
    }

    #[test]
    fn sync_instance_minecraft_settings_supports_dot_minecraft_layout() {
        let instances_dir = temp_path("sync-minecraft-settings-dot-minecraft");
        fs::create_dir_all(&instances_dir).expect("create instances dir");

        let source = sample_instance("source-dotmc", "Source DotMc");
        let target = sample_instance("target-dotmc", "Target DotMc");
        let idx = InstanceIndex {
            instances: vec![source.clone(), target.clone()],
        };
        write_index(&instances_dir, &idx).expect("write index");

        let source_dir = instance_dir_for_instance(&instances_dir, &source);
        fs::create_dir_all(source_dir.join(".minecraft")).expect("create source .minecraft");
        fs::write(source_dir.join("options.txt"), b"old-root").expect("write source root options");
        std::thread::sleep(std::time::Duration::from_millis(25));
        fs::write(
            source_dir.join(".minecraft").join("options.txt"),
            b"new-dot-minecraft",
        )
        .expect("write source dot minecraft options");

        let target_dir = instance_dir_for_id(&instances_dir, &target.id).expect("target dir");
        fs::create_dir_all(target_dir.join(".minecraft")).expect("create target .minecraft");

        let settings = InstanceSettings {
            sync_minecraft_settings: true,
            sync_minecraft_settings_target: "all".to_string(),
            ..InstanceSettings::default()
        };
        let copied =
            sync_instance_minecraft_settings_before_launch(&instances_dir, &source, &settings)
                .expect("sync from .minecraft source");

        assert_eq!(copied, 2);
        assert_eq!(
            fs::read_to_string(target_dir.join("options.txt")).expect("read target root options"),
            "new-dot-minecraft"
        );
        assert_eq!(
            fs::read_to_string(target_dir.join(".minecraft").join("options.txt"))
                .expect("read target dot minecraft options"),
            "new-dot-minecraft"
        );

        let _ = fs::remove_dir_all(&instances_dir);
    }

    #[test]
    fn reconcile_runtime_session_minecraft_settings_copies_back_latest_files() {
        let instances_dir = temp_path("runtime-session-settings-reconcile");
        let app_instance_dir = instances_dir.join("app-instance");
        let runtime_session_dir = instances_dir.join("runtime-session");
        fs::create_dir_all(app_instance_dir.join(".minecraft")).expect("create app .minecraft");
        fs::create_dir_all(runtime_session_dir.join(".minecraft"))
            .expect("create runtime .minecraft");

        fs::write(app_instance_dir.join("options.txt"), b"app-old").expect("write app options");
        std::thread::sleep(std::time::Duration::from_millis(25));
        fs::write(runtime_session_dir.join("options.txt"), b"runtime-new")
            .expect("write runtime options");
        fs::write(
            runtime_session_dir.join(".minecraft").join("servers.dat"),
            b"runtime-servers",
        )
        .expect("write runtime servers");

        let copied =
            reconcile_runtime_session_minecraft_settings(&runtime_session_dir, &app_instance_dir)
                .expect("reconcile runtime settings");
        assert_eq!(copied, 4);
        assert_eq!(
            fs::read_to_string(app_instance_dir.join("options.txt")).expect("read app options"),
            "runtime-new"
        );
        assert_eq!(
            fs::read_to_string(app_instance_dir.join(".minecraft").join("options.txt"))
                .expect("read mirrored app options"),
            "runtime-new"
        );
        assert_eq!(
            fs::read_to_string(app_instance_dir.join("servers.dat")).expect("read app servers"),
            "runtime-servers"
        );

        let _ = fs::remove_dir_all(&instances_dir);
    }

    #[test]
    fn isolated_native_launch_success_message_marks_runs_as_disposable() {
        let message = isolated_native_launch_success_message();
        assert!(message.contains("disposable isolated mode"));
        assert!(message.contains("temporary copy of the instance"));
        assert!(message.contains("only Minecraft settings sync back"));
    }

    fn sample_search_args(source: &str, sources: Vec<&str>) -> SearchDiscoverContentArgs {
        SearchDiscoverContentArgs {
            query: "test".to_string(),
            loaders: vec![],
            game_version: None,
            categories: vec![],
            index: "relevance".to_string(),
            limit: 20,
            offset: 0,
            sources: sources.into_iter().map(str::to_string).collect(),
            source: Some(source.to_string()),
            content_type: "mods".to_string(),
        }
    }

    #[test]
    fn normalized_discover_sources_defaults_to_all_providers() {
        let args = sample_search_args("all", vec![]);
        assert_eq!(
            normalized_discover_sources(&args),
            DISCOVER_PROVIDER_SOURCES
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn discover_source_subset_considers_full_provider_set_as_all_sources() {
        let selected = DISCOVER_PROVIDER_SOURCES
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        assert!(!discover_has_explicit_source_subset(&selected));
        assert!(discover_has_explicit_source_subset(&selected[..2]));
    }

    #[test]
    fn github_detail_install_support_stays_unknown_without_release_list() {
        assert_eq!(github_install_state(false, false), "checking");
        assert_eq!(github_install_state(false, true), "unsupported");
        assert_eq!(github_install_state(true, false), "ready");
    }
}
