use crate::modpack::layers::entry_key;
use crate::modpack::types::{
    DriftItem, DriftReport, InstanceModpackLinkState, LockSnapshot, LockSnapshotEntry, ModpackApplyResult,
    ResolutionPlan,
};
use reqwest::blocking::Client;
use std::collections::{HashMap, HashSet};
use std::fs;

pub fn apply_plan_to_instance(
    app: &tauri::AppHandle,
    plan: &ResolutionPlan,
    link_mode: &str,
    partial_apply_unsafe: bool,
) -> Result<(ModpackApplyResult, LockSnapshot, InstanceModpackLinkState), String> {
    let has_blocking = plan.failed_mods.iter().any(|f| f.required);
    if has_blocking && !partial_apply_unsafe {
        let details = plan
            .failed_mods
            .iter()
            .filter(|f| f.required)
            .take(6)
            .map(|f| format!("{} ({})", f.name, f.reason_code))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Apply blocked by required failures. Enable Partial Apply (UNSAFE) to continue. {}",
            details
        ));
    }

    let instances_dir = crate::app_instances_dir(app)?;
    let instance = crate::find_instance(&instances_dir, &plan.target.id)?;
    let instance_dir = crate::instance_dir_for_instance(&instances_dir, &instance);
    let mut lock = crate::read_lockfile(&instances_dir, &instance.id)?;

    let snapshot_id = if !plan.resolved_mods.is_empty() {
        Some(
            crate::create_instance_snapshot(&instances_dir, &instance.id, "before-apply-modpack-plan")?
                .id,
        )
    } else {
        None
    };

    let client = crate::build_http_client()?;

    let mut applied = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut warnings = Vec::new();

    for item in &plan.resolved_mods {
        if !is_supported_content_type(&item.content_type) {
            skipped += 1;
            warnings.push(format!(
                "Skipped '{}': unsupported content type '{}'",
                item.name, item.content_type
            ));
            continue;
        }

        match apply_single_resolved(&client, &instance, &instance_dir, &mut lock, item) {
            Ok(_) => applied += 1,
            Err(err) => {
                failed += 1;
                warnings.push(format!("Failed to apply '{}': {}", item.name, err));
            }
        }
    }

    lock.entries
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    crate::write_lockfile(&instances_dir, &instance.id, &lock)?;

    let lock_snapshot = build_lock_snapshot(&instance.id, &plan.id, &lock, snapshot_id.clone());
    let link = InstanceModpackLinkState {
        instance_id: instance.id.clone(),
        mode: normalize_link_mode(link_mode),
        modpack_id: plan.modpack_id.clone(),
        profile_id: plan.profile_id.clone(),
        last_plan_id: Some(plan.id.clone()),
        last_lock_snapshot_id: Some(lock_snapshot.id.clone()),
        last_applied_at: Some(crate::now_iso()),
        last_confidence_label: Some(plan.confidence_label.clone()),
    };

    Ok((
        ModpackApplyResult {
            message: if failed == 0 {
                format!("Applied plan '{}' successfully.", plan.id)
            } else {
                format!("Applied plan '{}' with {} failed entries.", plan.id, failed)
            },
            applied_entries: applied,
            skipped_entries: skipped,
            failed_entries: failed,
            snapshot_id,
            plan_id: plan.id.clone(),
            lock_snapshot_id: Some(lock_snapshot.id.clone()),
            warnings,
        },
        lock_snapshot,
        link,
    ))
}

fn apply_single_resolved(
    client: &Client,
    instance: &crate::Instance,
    instance_dir: &std::path::Path,
    lock: &mut crate::Lockfile,
    item: &crate::modpack::types::ResolvedMod,
) -> Result<(), String> {
    let content_type = normalize_content_type(&item.content_type);
    let bytes = if item.source == "modrinth" {
        let download_url = item
            .download_url
            .as_ref()
            .ok_or_else(|| "missing download url in resolution plan".to_string())?;
        download_bytes(client, download_url)?
    } else if item.source == "curseforge" {
        let api_key = crate::curseforge_api_key().ok_or_else(crate::missing_curseforge_key_message)?;
        let mod_id = crate::parse_curseforge_project_id(&item.project_id)?;
        let files = crate::fetch_curseforge_files(client, &api_key, mod_id)?;
        let wanted = item
            .curseforge_file_id
            .or_else(|| parse_curseforge_file_id(&item.version_id))
            .ok_or_else(|| "missing curseforge file id in plan".to_string())?;
        let file = files
            .into_iter()
            .find(|f| f.id == wanted)
            .ok_or_else(|| format!("CurseForge file {} no longer available", wanted))?;
        let download_url = crate::resolve_curseforge_file_download_url(client, &api_key, mod_id, &file)?;
        download_bytes(client, &download_url)?
    } else {
        return Err("unsupported provider".to_string());
    };

    let target_worlds = if content_type == "datapacks" {
        crate::normalize_target_worlds_for_datapack(instance_dir, &item.target_worlds)?
    } else {
        vec![]
    };

    crate::write_download_to_content_targets(
        instance_dir,
        &content_type,
        &item.filename,
        &target_worlds,
        &bytes,
    )?;

    crate::remove_replaced_entries_for_content(
        lock,
        instance_dir,
        &item.project_id,
        &content_type,
    )?;

    let mut new_entry = crate::LockEntry {
        source: item.source.clone(),
        project_id: item.project_id.clone(),
        version_id: item.version_id.clone(),
        name: item.name.clone(),
        version_number: item.version_number.clone(),
        filename: item.filename.clone(),
        content_type: content_type.clone(),
        target_scope: if content_type == "datapacks" {
            "world".to_string()
        } else {
            "instance".to_string()
        },
        target_worlds,
        pinned_version: None,
        enabled: item.enabled,
        hashes: item.hashes.clone(),
    };

    if content_type == "mods" && !item.enabled {
        let mods_dir = instance_dir.join("mods");
        let enabled_path = mods_dir.join(&item.filename);
        let disabled_path = mods_dir.join(format!("{}.disabled", item.filename));
        if disabled_path.exists() {
            fs::remove_file(&disabled_path)
                .map_err(|e| format!("remove existing disabled file failed: {e}"))?;
        }
        if enabled_path.exists() {
            fs::rename(&enabled_path, &disabled_path)
                .map_err(|e| format!("failed to preserve disabled mod state: {e}"))?;
        }
        new_entry.enabled = false;
    }

    lock.entries.push(new_entry);

    let _ = instance;
    Ok(())
}

fn download_bytes(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| format!("download failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "download failed with status {}",
            response.status()
        ));
    }
    let mut bytes = Vec::new();
    response
        .copy_to(&mut bytes)
        .map_err(|e| format!("download read failed: {e}"))?;
    Ok(bytes)
}

pub fn build_lock_snapshot(
    instance_id: &str,
    plan_id: &str,
    lock: &crate::Lockfile,
    instance_snapshot_id: Option<String>,
) -> LockSnapshot {
    let entries = lock
        .entries
        .iter()
        .filter(|e| {
            (e.source.eq_ignore_ascii_case("modrinth") || e.source.eq_ignore_ascii_case("curseforge"))
                && is_supported_content_type(&e.content_type)
        })
        .map(|e| LockSnapshotEntry {
            source: e.source.clone(),
            content_type: normalize_content_type(&e.content_type),
            project_id: e.project_id.clone(),
            name: e.name.clone(),
            version_id: e.version_id.clone(),
            version_number: e.version_number.clone(),
            enabled: e.enabled,
            target_worlds: e.target_worlds.clone(),
        })
        .collect::<Vec<_>>();

    LockSnapshot {
        id: format!("locksnap_{}", crate::now_millis()),
        instance_id: instance_id.to_string(),
        plan_id: plan_id.to_string(),
        created_at: crate::now_iso(),
        entries,
        instance_snapshot_id,
    }
}

pub fn detect_drift(instance_id: &str, lock: &crate::Lockfile, snapshot: &LockSnapshot) -> DriftReport {
    let current_map = lock
        .entries
        .iter()
        .filter(|e| {
            (e.source.eq_ignore_ascii_case("modrinth") || e.source.eq_ignore_ascii_case("curseforge"))
                && is_supported_content_type(&e.content_type)
        })
        .map(|e| {
            (
                entry_key(&e.source, &e.project_id, &e.content_type),
                (
                    e.name.clone(),
                    e.version_number.clone(),
                    e.version_id.clone(),
                    normalize_content_type(&e.content_type),
                    e.source.clone(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    let expected_map = snapshot
        .entries
        .iter()
        .map(|e| {
            (
                entry_key(&e.source, &e.project_id, &e.content_type),
                (
                    e.name.clone(),
                    e.version_number.clone(),
                    e.version_id.clone(),
                    normalize_content_type(&e.content_type),
                    e.source.clone(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    let current_keys = current_map.keys().cloned().collect::<HashSet<_>>();
    let expected_keys = expected_map.keys().cloned().collect::<HashSet<_>>();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut version_changed = Vec::new();

    for key in current_keys.difference(&expected_keys) {
        if let Some((name, version_number, _version_id, content_type, source)) = current_map.get(key) {
            added.push(DriftItem {
                source: source.clone(),
                content_type: content_type.clone(),
                project_id: parse_project_id_from_key(key),
                name: name.clone(),
                expected_version: None,
                current_version: Some(version_number.clone()),
            });
        }
    }

    for key in expected_keys.difference(&current_keys) {
        if let Some((name, version_number, _version_id, content_type, source)) = expected_map.get(key) {
            removed.push(DriftItem {
                source: source.clone(),
                content_type: content_type.clone(),
                project_id: parse_project_id_from_key(key),
                name: name.clone(),
                expected_version: Some(version_number.clone()),
                current_version: None,
            });
        }
    }

    for key in expected_keys.intersection(&current_keys) {
        let Some((name, expected_version, expected_id, content_type, source)) = expected_map.get(key)
        else {
            continue;
        };
        let Some((_cur_name, current_version, current_id, _ct, _src)) = current_map.get(key) else {
            continue;
        };

        if expected_id != current_id {
            version_changed.push(DriftItem {
                source: source.clone(),
                content_type: content_type.clone(),
                project_id: parse_project_id_from_key(key),
                name: name.clone(),
                expected_version: Some(expected_version.clone()),
                current_version: Some(current_version.clone()),
            });
        }
    }

    let status = if added.is_empty() && removed.is_empty() && version_changed.is_empty() {
        "in_sync"
    } else {
        "drifted"
    }
    .to_string();

    DriftReport {
        instance_id: instance_id.to_string(),
        status,
        added,
        removed,
        version_changed,
        created_at: crate::now_iso(),
    }
}

fn parse_project_id_from_key(key: &str) -> String {
    let mut parts = key.split(':');
    let _provider = parts.next();
    let _content_type = parts.next();
    parts.collect::<Vec<_>>().join(":")
}

pub fn normalize_link_mode(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "unlinked" | "one_time" | "one-time" => "unlinked".to_string(),
        _ => "linked".to_string(),
    }
}

fn parse_curseforge_file_id(raw: &str) -> Option<i64> {
    raw.trim()
        .trim_start_matches("cf_file:")
        .trim()
        .parse::<i64>()
        .ok()
}

fn normalize_content_type(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "mods" | "mod" => "mods".to_string(),
        "resourcepacks" | "resourcepack" => "resourcepacks".to_string(),
        "shaderpacks" | "shaderpack" | "shaders" => "shaderpacks".to_string(),
        "datapacks" | "datapack" => "datapacks".to_string(),
        _ => "mods".to_string(),
    }
}

fn is_supported_content_type(content_type: &str) -> bool {
    matches!(
        normalize_content_type(content_type).as_str(),
        "mods" | "resourcepacks" | "shaderpacks" | "datapacks"
    )
}
