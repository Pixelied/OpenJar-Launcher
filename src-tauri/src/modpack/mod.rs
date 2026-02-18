pub mod apply;
pub mod dev_seed;
pub mod layers;
pub mod migration;
pub mod resolver;
pub mod store;
pub mod tests;
pub mod types;

use crate::modpack::apply::{apply_plan_to_instance, detect_drift, normalize_link_mode};
use crate::modpack::layers::{
    diff_entries, ensure_default_profiles, entry_key_for, make_base_spec, normalize_entry_for_add, reduce_layers,
};
use crate::modpack::migration::migrate_legacy_payload;
use crate::modpack::resolver::resolve_modpack;
use crate::modpack::store::{
    add_lock_snapshot, add_plan, get_instance_link, get_lock_snapshot, get_plan, get_spec, read_store, remove_spec,
    set_instance_link, upsert_spec, write_store,
};
use crate::modpack::types::*;
use std::fs;
use std::path::PathBuf;

#[tauri::command]
pub fn list_modpack_specs(app: tauri::AppHandle) -> Result<Vec<ModpackSpec>, String> {
    let store = read_store(&app)?;
    Ok(store.specs)
}

#[tauri::command]
pub fn get_modpack_spec(app: tauri::AppHandle, args: ModpackIdArgs) -> Result<ModpackSpec, String> {
    let store = read_store(&app)?;
    get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())
}

#[tauri::command]
pub fn upsert_modpack_spec(app: tauri::AppHandle, args: UpsertModpackSpecArgs) -> Result<ModpackSpec, String> {
    let mut store = read_store(&app)?;
    let mut spec = args.spec;
    normalize_spec_for_write(&mut spec);
    upsert_spec(&mut store, spec.clone());
    write_store(&app, &store)?;
    Ok(spec)
}

#[tauri::command]
pub fn duplicate_modpack_spec(
    app: tauri::AppHandle,
    args: DuplicateModpackSpecArgs,
) -> Result<ModpackSpec, String> {
    let mut store = read_store(&app)?;
    let source = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;

    let mut clone = source;
    clone.id = format!("modpack_{}", crate::now_millis());
    clone.name = args
        .new_name
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{} copy", clone.name));
    clone.created_at = crate::now_iso();
    clone.updated_at = clone.created_at.clone();

    upsert_spec(&mut store, clone.clone());
    write_store(&app, &store)?;
    Ok(clone)
}

#[tauri::command]
pub fn delete_modpack_spec(app: tauri::AppHandle, args: DeleteModpackSpecArgs) -> Result<bool, String> {
    let mut store = read_store(&app)?;
    let before = store.specs.len();
    remove_spec(&mut store, &args.modpack_id);
    let removed = store.specs.len() < before;
    if removed {
        write_store(&app, &store)?;
    }
    Ok(removed)
}

#[tauri::command]
pub fn import_modpack_spec_json(
    app: tauri::AppHandle,
    args: ImportModpackSpecJsonArgs,
) -> Result<SpecIoResult, String> {
    let path_text = args.input_path.trim();
    if path_text.is_empty() {
        return Err("inputPath is required".to_string());
    }
    let raw = fs::read_to_string(path_text).map_err(|e| format!("read spec import file failed: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse spec import file failed: {e}"))?;

    let mut specs: Vec<ModpackSpec> = Vec::new();
    if value.is_object() && value.get("layers").is_some() {
        let spec = serde_json::from_value::<ModpackSpec>(value)
            .map_err(|e| format!("parse modpack spec failed: {e}"))?;
        specs.push(spec);
    } else if let Some(array) = value.as_array() {
        for item in array {
            if let Ok(spec) = serde_json::from_value::<ModpackSpec>(item.clone()) {
                specs.push(spec);
            }
        }
    } else if let Some(array) = value.get("specs").and_then(|v| v.as_array()) {
        for item in array {
            if let Ok(spec) = serde_json::from_value::<ModpackSpec>(item.clone()) {
                specs.push(spec);
            }
        }
    }

    if specs.is_empty() {
        return Err("No valid ModpackSpec entries found in import payload.".to_string());
    }

    let mut store = read_store(&app)?;
    let existing_ids = store.specs.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    for mut spec in specs {
        if existing_ids.contains(&spec.id) {
            spec.id = format!("{}_{}", spec.id, crate::now_millis());
        }
        normalize_spec_for_write(&mut spec);
        upsert_spec(&mut store, spec);
    }
    write_store(&app, &store)?;

    Ok(SpecIoResult {
        path: path_text.to_string(),
        items: store.specs.len(),
    })
}

#[tauri::command]
pub fn export_modpack_spec_json(
    app: tauri::AppHandle,
    args: ExportModpackSpecJsonArgs,
) -> Result<SpecIoResult, String> {
    let path_text = args.output_path.trim();
    if path_text.is_empty() {
        return Err("outputPath is required".to_string());
    }
    let store = read_store(&app)?;
    let spec = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;

    let path = std::path::PathBuf::from(path_text);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir export dir failed: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(&spec).map_err(|e| format!("serialize spec failed: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("write export file failed: {e}"))?;

    Ok(SpecIoResult {
        path: path.display().to_string(),
        items: 1,
    })
}

#[tauri::command]
pub fn import_modpack_layer_from_provider(
    app: tauri::AppHandle,
    args: ImportLayerFromProviderArgs,
) -> Result<ModpackSpec, String> {
    let mut store = read_store(&app)?;
    let mut spec = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;

    let preset = crate::import_provider_modpack_template(crate::ImportProviderModpackArgs {
        source: args.source.clone(),
        project_id: args.project_id.clone(),
        project_title: args.project_title.clone(),
    })?;

    let entries = preset
        .entries
        .into_iter()
        .map(creator_entry_to_mod_entry)
        .map(normalize_entry_for_add)
        .collect::<Vec<_>>();

    let layer = Layer {
        id: format!("layer_{}", crate::now_millis()),
        name: args.layer_name.trim().to_string(),
        source: Some(LayerSource {
            kind: "provider_template".to_string(),
            source: Some(args.source.trim().to_lowercase()),
            project_id: Some(args.project_id.clone()),
            spec_id: None,
            imported_at: Some(crate::now_iso()),
        }),
        is_frozen: false,
        entries_delta: EntriesDelta {
            add: entries,
            remove: vec![],
            override_entries: vec![],
        },
    };

    spec.layers.push(layer);
    spec.updated_at = crate::now_iso();
    normalize_spec_for_write(&mut spec);
    upsert_spec(&mut store, spec.clone());
    write_store(&app, &store)?;

    Ok(spec)
}

#[tauri::command]
pub fn import_modpack_layer_from_spec(
    app: tauri::AppHandle,
    args: ImportLayerFromSpecArgs,
) -> Result<ModpackSpec, String> {
    let mut store = read_store(&app)?;

    let source_spec = get_spec(&store, &args.source_modpack_id)
        .ok_or_else(|| "Source modpack spec not found".to_string())?;
    let mut target_spec = get_spec(&store, &args.target_modpack_id)
        .ok_or_else(|| "Target modpack spec not found".to_string())?;

    let (entries, _, _) = reduce_layers(&source_spec);
    let layer = Layer {
        id: format!("layer_{}", crate::now_millis()),
        name: args.layer_name.trim().to_string(),
        source: Some(LayerSource {
            kind: "spec_import".to_string(),
            source: None,
            project_id: None,
            spec_id: Some(source_spec.id.clone()),
            imported_at: Some(crate::now_iso()),
        }),
        is_frozen: false,
        entries_delta: EntriesDelta {
            add: entries,
            remove: vec![],
            override_entries: vec![],
        },
    };

    target_spec.layers.push(layer);
    target_spec.updated_at = crate::now_iso();
    normalize_spec_for_write(&mut target_spec);
    upsert_spec(&mut store, target_spec.clone());
    write_store(&app, &store)?;

    Ok(target_spec)
}

#[tauri::command]
pub fn preview_template_layer_update(
    app: tauri::AppHandle,
    args: LayerRefArgs,
) -> Result<LayerDiffResult, String> {
    let store = read_store(&app)?;
    let spec = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;
    let layer = spec
        .layers
        .iter()
        .find(|l| l.id == args.layer_id)
        .ok_or_else(|| "Layer not found".to_string())?;
    let source = layer
        .source
        .clone()
        .ok_or_else(|| "Layer has no source metadata".to_string())?;
    if source.kind != "provider_template" {
        return Err("Only provider template layers can be refreshed.".to_string());
    }

    let source_provider = source
        .source
        .clone()
        .ok_or_else(|| "Layer source provider missing".to_string())?;
    let source_project_id = source
        .project_id
        .clone()
        .ok_or_else(|| "Layer source project id missing".to_string())?;

    let preset = crate::import_provider_modpack_template(crate::ImportProviderModpackArgs {
        source: source_provider,
        project_id: source_project_id,
        project_title: None,
    })?;

    let latest_entries = preset
        .entries
        .into_iter()
        .map(creator_entry_to_mod_entry)
        .map(normalize_entry_for_add)
        .collect::<Vec<_>>();

    let current_entries = layer
        .entries_delta
        .add
        .iter()
        .cloned()
        .map(normalize_entry_for_add)
        .collect::<Vec<_>>();

    let (added, removed, overridden) = diff_entries(&current_entries, &latest_entries);

    Ok(LayerDiffResult {
        layer_id: Some(layer.id.clone()),
        added,
        removed,
        overridden,
        conflicts: vec![],
        warnings: vec![],
    })
}

#[tauri::command]
pub fn apply_template_layer_update(
    app: tauri::AppHandle,
    args: LayerRefArgs,
) -> Result<ModpackSpec, String> {
    let mut store = read_store(&app)?;
    let mut spec = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;

    let layer_idx = spec
        .layers
        .iter()
        .position(|l| l.id == args.layer_id)
        .ok_or_else(|| "Layer not found".to_string())?;

    if spec.layers[layer_idx].is_frozen {
        return Err("Layer is frozen. Unfreeze before applying template update.".to_string());
    }

    let source = spec.layers[layer_idx]
        .source
        .clone()
        .ok_or_else(|| "Layer has no source metadata".to_string())?;
    if source.kind != "provider_template" {
        return Err("Only provider template layers can be refreshed.".to_string());
    }

    let source_provider = source
        .source
        .clone()
        .ok_or_else(|| "Layer source provider missing".to_string())?;
    let source_project_id = source
        .project_id
        .clone()
        .ok_or_else(|| "Layer source project id missing".to_string())?;

    let preset = crate::import_provider_modpack_template(crate::ImportProviderModpackArgs {
        source: source_provider,
        project_id: source_project_id,
        project_title: None,
    })?;

    spec.layers[layer_idx].entries_delta.add = preset
        .entries
        .into_iter()
        .map(creator_entry_to_mod_entry)
        .map(normalize_entry_for_add)
        .collect::<Vec<_>>();
    spec.layers[layer_idx].entries_delta.override_entries.clear();
    spec.layers[layer_idx].entries_delta.remove.clear();
    spec.layers[layer_idx].source.as_mut().map(|s| s.imported_at = Some(crate::now_iso()));

    spec.updated_at = crate::now_iso();
    normalize_spec_for_write(&mut spec);
    upsert_spec(&mut store, spec.clone());
    write_store(&app, &store)?;

    Ok(spec)
}

#[derive(Debug, Clone)]
struct ProviderResolution {
    source: String,
    project_id: String,
    version_id: String,
    name: String,
    version_number: String,
    confidence: String,
    reason: String,
}

fn provider_resolution_from_import(found: crate::LocalImportedProviderMatch) -> ProviderResolution {
    ProviderResolution {
        source: found.source,
        project_id: found.project_id,
        version_id: found.version_id,
        name: found.name,
        version_number: found.version_number,
        confidence: found.confidence,
        reason: found.reason,
    }
}

fn local_file_name_from_entry(entry: &ModEntry) -> String {
    if let Some(name) = entry
        .local_file_name
        .as_ref()
        .map(|v| crate::sanitize_filename(v))
        .filter(|v| !v.trim().is_empty())
    {
        return name;
    }

    if let Some(path_text) = entry.local_file_path.as_ref() {
        let path = PathBuf::from(path_text);
        if let Some(name) = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(crate::sanitize_filename)
            .filter(|value| !value.trim().is_empty())
        {
            return name;
        }
    }

    let local_id = entry.project_id.trim().trim_start_matches("local:");
    let candidate = if local_id.to_ascii_lowercase().ends_with(".jar") {
        local_id.to_string()
    } else if local_id.is_empty() {
        "local-mod.jar".to_string()
    } else {
        format!("{local_id}.jar")
    };
    let safe = crate::sanitize_filename(&candidate);
    if safe.trim().is_empty() {
        "local-mod.jar".to_string()
    } else {
        safe
    }
}

fn local_entry_display_name(entry: &ModEntry, safe_file_name: &str) -> String {
    entry
        .notes
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| crate::infer_local_name(safe_file_name))
}

fn detect_provider_from_entry_metadata(
    client: &reqwest::blocking::Client,
    entry: &ModEntry,
) -> Option<ProviderResolution> {
    let safe_file_name = local_file_name_from_entry(entry);
    let sha512 = entry
        .local_sha512
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    if let Some(api_key) = crate::curseforge_api_key() {
        if !entry.local_fingerprints.is_empty() {
            if let Ok(Some((project, file))) =
                crate::fetch_curseforge_match_by_fingerprints(client, &api_key, &entry.local_fingerprints)
            {
                let name = if project.name.trim().is_empty() {
                    local_entry_display_name(entry, &safe_file_name)
                } else {
                    project.name.clone()
                };
                let version_number = if file.display_name.trim().is_empty() {
                    if file.file_name.trim().is_empty() {
                        "unknown".to_string()
                    } else {
                        file.file_name.clone()
                    }
                } else {
                    file.display_name.clone()
                };
                return Some(ProviderResolution {
                    source: "curseforge".to_string(),
                    project_id: format!("cf:{}", project.id),
                    version_id: format!("cf_file:{}", file.id),
                    name,
                    version_number,
                    confidence: "deterministic".to_string(),
                    reason: "Exact CurseForge fingerprint match from local metadata.".to_string(),
                });
            }
        }
    }

    if let Some(sha) = sha512 {
        if let Ok(Some(version)) = crate::fetch_modrinth_version_by_sha512(client, &sha) {
            let project_id = version.project_id.trim().to_string();
            if !project_id.is_empty() {
                let matched_file = version
                    .files
                    .iter()
                    .find(|f| {
                        f.hashes
                            .get("sha512")
                            .map(|value| value.eq_ignore_ascii_case(&sha))
                            .unwrap_or(false)
                    })
                    .or_else(|| {
                        version
                            .files
                            .iter()
                            .find(|f| crate::sanitize_filename(&f.filename).eq_ignore_ascii_case(&safe_file_name))
                    })
                    .or_else(|| version.files.first());
                let version_number = if version.version_number.trim().is_empty() {
                    matched_file
                        .map(|f| f.filename.clone())
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "unknown".to_string())
                } else {
                    version.version_number.clone()
                };
                let name = version
                    .name
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| crate::fetch_project_title(client, &project_id))
                    .unwrap_or_else(|| local_entry_display_name(entry, &safe_file_name));
                return Some(ProviderResolution {
                    source: "modrinth".to_string(),
                    project_id,
                    version_id: version.id,
                    name,
                    version_number,
                    confidence: "deterministic".to_string(),
                    reason: "Exact Modrinth SHA-512 match from local metadata.".to_string(),
                });
            }
        }
    }

    None
}

fn is_local_mod_entry(entry: &ModEntry) -> bool {
    entry.provider.trim().eq_ignore_ascii_case("local") && normalize_content_type(&entry.content_type) == "mods"
}

fn count_remaining_local_entries(spec: &ModpackSpec) -> usize {
    spec.layers
        .iter()
        .flat_map(|layer| layer.entries_delta.add.iter())
        .filter(|entry| is_local_mod_entry(entry))
        .count()
}

#[tauri::command]
pub fn import_local_jars_to_modpack_layer(
    app: tauri::AppHandle,
    args: ImportLocalJarsToLayerArgs,
) -> Result<ModpackImportLocalJarsResult, String> {
    if args.file_paths.is_empty() {
        return Err("Pick at least one .jar file.".to_string());
    }

    let mut store = read_store(&app)?;
    let mut spec = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;
    let layer_idx = spec
        .layers
        .iter()
        .position(|layer| layer.id == args.layer_id)
        .ok_or_else(|| "Layer not found".to_string())?;

    if spec.layers[layer_idx].is_frozen {
        return Err("Layer is frozen. Unfreeze before adding local jars.".to_string());
    }

    let auto_identify = args.auto_identify.unwrap_or(false);
    let client = if auto_identify {
        Some(crate::build_http_client()?)
    } else {
        None
    };

    let mut added_entries = 0usize;
    let mut updated_entries = 0usize;
    let mut resolved_entries = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    for path_text in args.file_paths {
        let trimmed = path_text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = PathBuf::from(trimmed);
        if !path.exists() {
            warnings.push(format!("Skipped '{}': file was not found.", trimmed));
            continue;
        }

        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        if ext != "jar" {
            warnings.push(format!("Skipped '{}': only .jar files are supported.", path.display()));
            continue;
        }

        let file_name_raw = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("local-mod.jar");
        let safe_file_name = crate::sanitize_filename(file_name_raw);
        if safe_file_name.trim().is_empty() {
            warnings.push(format!("Skipped '{}': filename could not be sanitized.", path.display()));
            continue;
        }

        let bytes = match fs::read(&path) {
            Ok(value) => value,
            Err(err) => {
                warnings.push(format!("Skipped '{}': read failed ({err}).", path.display()));
                continue;
            }
        };

        let sha512 = crate::sha512_hex(&bytes);
        let fingerprints = crate::curseforge_fingerprint_candidates(&bytes);
        let display_name = crate::infer_local_name(&safe_file_name);
        let detected = client
            .as_ref()
            .and_then(|http| crate::detect_provider_for_local_mod(http, &bytes, &safe_file_name, true))
            .map(provider_resolution_from_import);

        let mut entry = normalize_entry_for_add(ModEntry {
            provider: detected
                .as_ref()
                .map(|value| value.source.clone())
                .unwrap_or_else(|| "local".to_string()),
            project_id: detected
                .as_ref()
                .map(|value| value.project_id.clone())
                .unwrap_or_else(|| format!("local:{}", safe_file_name.to_ascii_lowercase())),
            slug: Some(display_name.clone()),
            content_type: "mods".to_string(),
            required: true,
            pin: detected.as_ref().map(|value| value.version_id.clone()),
            channel_policy: "stable".to_string(),
            fallback_policy: "inherit".to_string(),
            replacement_group: None,
            notes: Some(
                detected
                    .as_ref()
                    .map(|value| value.name.clone())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(display_name),
            ),
            disabled_by_default: false,
            optional: false,
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            local_file_name: Some(safe_file_name.clone()),
            local_file_path: Some(path.display().to_string()),
            local_sha512: Some(sha512.clone()),
            local_fingerprints: fingerprints,
        });

        if let Some(found) = detected {
            resolved_entries += 1;
            if entry.notes.as_ref().map(|value| value.trim().is_empty()).unwrap_or(true) {
                entry.notes = Some(found.name);
            }
        } else if auto_identify {
            warnings.push(format!(
                "Added '{}' as local entry (provider match not found yet).",
                safe_file_name
            ));
        }

        let key = entry_key_for(&entry);
        let layer = &mut spec.layers[layer_idx];
        if let Some(existing_idx) = layer
            .entries_delta
            .add
            .iter()
            .position(|current| entry_key_for(current) == key)
        {
            layer.entries_delta.add[existing_idx] = entry;
            updated_entries += 1;
        } else {
            layer.entries_delta.add.push(entry);
            added_entries += 1;
        }
    }

    if added_entries == 0 && updated_entries == 0 {
        return Err("No local jars were imported.".to_string());
    }

    spec.updated_at = crate::now_iso();
    normalize_spec_for_write(&mut spec);
    upsert_spec(&mut store, spec.clone());
    write_store(&app, &store)?;

    Ok(ModpackImportLocalJarsResult {
        remaining_local_entries: count_remaining_local_entries(&spec),
        spec,
        added_entries,
        updated_entries,
        resolved_entries,
        warnings,
    })
}

#[tauri::command]
pub fn resolve_local_modpack_entries(
    app: tauri::AppHandle,
    args: ResolveLocalModpackEntriesArgs,
) -> Result<ModpackLocalResolverResult, String> {
    let mut store = read_store(&app)?;
    let mut spec = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;
    let mode = args.mode.unwrap_or_else(|| "missing_only".to_string());
    let scan_all_candidates = mode.trim().eq_ignore_ascii_case("all");
    let layer_scope = args
        .layer_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let client = crate::build_http_client()?;
    let mut scanned_entries = 0usize;
    let mut resolved_entries = 0usize;
    let mut warnings: Vec<String> = Vec::new();
    let mut matches: Vec<ModpackLocalResolverMatch> = Vec::new();
    let mut changed = false;

    for layer in &mut spec.layers {
        if let Some(scope) = layer_scope.as_ref() {
            if &layer.id != scope {
                continue;
            }
        }

        for entry in &mut layer.entries_delta.add {
            if normalize_content_type(&entry.content_type) != "mods" {
                continue;
            }
            let provider = entry.provider.trim().to_ascii_lowercase();
            let project_looks_local = entry
                .project_id
                .trim()
                .to_ascii_lowercase()
                .starts_with("local:");
            let has_local_metadata = entry
                .local_sha512
                .as_ref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
                || !entry.local_fingerprints.is_empty()
                || entry
                    .local_file_path
                    .as_ref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false);

            if !scan_all_candidates {
                if provider != "local" {
                    continue;
                }
            } else if provider != "local" && !project_looks_local {
                continue;
            }
            if provider == "local" && !has_local_metadata {
                warnings.push(format!(
                    "Skipped '{}': no local jar metadata. Re-add from computer first.",
                    entry.project_id
                ));
                continue;
            }

            scanned_entries += 1;
            let key_before = entry_key_for(entry);
            let from_source = provider.clone();
            let safe_file_name = local_file_name_from_entry(entry);
            let mut detected: Option<ProviderResolution> = None;

            if let Some(path_text) = entry
                .local_file_path
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            {
                let path = PathBuf::from(&path_text);
                if path.exists() {
                    match fs::read(&path) {
                        Ok(bytes) => {
                            let refreshed_name = path
                                .file_name()
                                .and_then(|value| value.to_str())
                                .map(crate::sanitize_filename)
                                .filter(|value| !value.trim().is_empty())
                                .unwrap_or_else(|| safe_file_name.clone());
                            entry.local_file_name = Some(refreshed_name.clone());
                            entry.local_sha512 = Some(crate::sha512_hex(&bytes));
                            entry.local_fingerprints = crate::curseforge_fingerprint_candidates(&bytes);
                            changed = true;
                            detected = crate::detect_provider_for_local_mod(
                                &client,
                                &bytes,
                                &refreshed_name,
                                true,
                            )
                            .map(provider_resolution_from_import);
                        }
                        Err(err) => {
                            warnings.push(format!("Failed to read '{}': {err}", path.display()));
                        }
                    }
                } else {
                    warnings.push(format!(
                        "Local jar path missing for '{}': {}",
                        entry.project_id, path_text
                    ));
                }
            }

            if detected.is_none() {
                detected = detect_provider_from_entry_metadata(&client, entry);
            }

            let Some(found) = detected else {
                continue;
            };

            entry.provider = found.source.clone();
            entry.project_id = found.project_id.clone();
            entry.pin = Some(found.version_id.clone());
            if entry.slug.as_ref().map(|value| value.trim().is_empty()).unwrap_or(true) {
                entry.slug = Some(found.name.clone());
            }
            if from_source == "local"
                || entry
                    .notes
                    .as_ref()
                    .map(|value| value.trim().is_empty())
                    .unwrap_or(true)
            {
                entry.notes = Some(found.name.clone());
            }
            changed = true;
            resolved_entries += 1;
            matches.push(ModpackLocalResolverMatch {
                key: key_before,
                from_source,
                to_source: found.source,
                project_id: found.project_id,
                version_id: found.version_id,
                name: found.name,
                version_number: found.version_number,
                confidence: found.confidence,
                reason: found.reason,
            });
        }
    }

    if changed {
        spec.updated_at = crate::now_iso();
        normalize_spec_for_write(&mut spec);
        upsert_spec(&mut store, spec.clone());
        write_store(&app, &store)?;
    }

    Ok(ModpackLocalResolverResult {
        remaining_local_entries: count_remaining_local_entries(&spec),
        spec,
        scanned_entries,
        resolved_entries,
        matches,
        warnings,
    })
}

#[tauri::command]
pub fn resolve_modpack_for_instance(
    app: tauri::AppHandle,
    args: ResolveModpackArgs,
) -> Result<ResolutionPlan, String> {
    let mut store = read_store(&app)?;
    let spec = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;

    let instances_dir = crate::app_instances_dir(&app)?;
    let instance = crate::find_instance(&instances_dir, &args.instance_id)?;
    let client = crate::build_http_client()?;

    let plan = resolve_modpack(
        &client,
        &instance,
        &spec,
        args.profile_id.as_deref(),
        args.settings,
    )?;

    add_plan(&mut store, plan.clone());
    write_store(&app, &store)?;

    Ok(plan)
}

#[tauri::command]
pub fn apply_modpack_plan(
    app: tauri::AppHandle,
    args: ApplyModpackPlanArgs,
) -> Result<ModpackApplyResult, String> {
    let mut store = read_store(&app)?;
    let plan = get_plan(&store, &args.plan_id).ok_or_else(|| "Resolution plan not found".to_string())?;

    let allow_partial = args
        .partial_apply_unsafe
        .unwrap_or(plan.settings.partial_apply_unsafe);
    let (result, lock_snapshot, link) = apply_plan_to_instance(
        &app,
        &plan,
        args.link_mode.as_deref().unwrap_or("linked"),
        allow_partial,
    )?;

    add_lock_snapshot(&mut store, lock_snapshot);
    set_instance_link(&mut store, link);
    write_store(&app, &store)?;

    Ok(result)
}

#[tauri::command]
pub fn get_instance_modpack_status(
    app: tauri::AppHandle,
    args: InstanceArgs,
) -> Result<InstanceModpackStatus, String> {
    let store = read_store(&app)?;
    let link = get_instance_link(&store, &args.instance_id);
    let last_plan = link
        .as_ref()
        .and_then(|l| l.last_plan_id.as_deref())
        .and_then(|id| get_plan(&store, id));

    let drift = if let Some(link_state) = link.as_ref() {
        if let Some(lock_snapshot_id) = link_state.last_lock_snapshot_id.as_deref() {
            if let Some(snapshot) = get_lock_snapshot(&store, lock_snapshot_id) {
                let instances_dir = crate::app_instances_dir(&app)?;
                let lock = crate::read_lockfile(&instances_dir, &args.instance_id)?;
                Some(detect_drift(&args.instance_id, &lock, &snapshot))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    Ok(InstanceModpackStatus {
        instance_id: args.instance_id,
        link,
        last_plan,
        drift,
    })
}

#[tauri::command]
pub fn detect_instance_modpack_drift(
    app: tauri::AppHandle,
    args: InstanceArgs,
) -> Result<DriftReport, String> {
    let store = read_store(&app)?;
    let Some(link) = get_instance_link(&store, &args.instance_id) else {
        return Ok(DriftReport {
            instance_id: args.instance_id,
            status: "unlinked".to_string(),
            added: vec![],
            removed: vec![],
            version_changed: vec![],
            created_at: crate::now_iso(),
        });
    };

    let Some(snapshot_id) = link.last_lock_snapshot_id.as_deref() else {
        return Ok(DriftReport {
            instance_id: args.instance_id,
            status: "no_snapshot".to_string(),
            added: vec![],
            removed: vec![],
            version_changed: vec![],
            created_at: crate::now_iso(),
        });
    };

    let snapshot = get_lock_snapshot(&store, snapshot_id)
        .ok_or_else(|| "Linked lock snapshot not found".to_string())?;
    let instances_dir = crate::app_instances_dir(&app)?;
    let lock = crate::read_lockfile(&instances_dir, &args.instance_id)?;
    Ok(detect_drift(&args.instance_id, &lock, &snapshot))
}

#[tauri::command]
pub fn realign_instance_to_modpack(
    app: tauri::AppHandle,
    args: InstanceArgs,
) -> Result<ModpackApplyResult, String> {
    let mut store = read_store(&app)?;
    let link = get_instance_link(&store, &args.instance_id)
        .ok_or_else(|| "Instance is not linked to a modpack".to_string())?;
    if normalize_link_mode(&link.mode) != "linked" {
        return Err("Instance is unlinked. Re-align is only available for linked instances.".to_string());
    }

    let spec = get_spec(&store, &link.modpack_id)
        .ok_or_else(|| "Linked modpack not found".to_string())?;
    let instances_dir = crate::app_instances_dir(&app)?;
    let instance = crate::find_instance(&instances_dir, &args.instance_id)?;
    let client = crate::build_http_client()?;

    let plan = resolve_modpack(
        &client,
        &instance,
        &spec,
        link.profile_id.as_deref(),
        Some(spec.settings.clone()),
    )?;
    add_plan(&mut store, plan.clone());

    let (result, lock_snapshot, new_link) =
        apply_plan_to_instance(&app, &plan, "linked", plan.settings.partial_apply_unsafe)?;

    add_lock_snapshot(&mut store, lock_snapshot);
    set_instance_link(&mut store, new_link);
    write_store(&app, &store)?;

    Ok(result)
}

#[tauri::command]
pub fn preview_update_modpack_from_instance(
    app: tauri::AppHandle,
    args: PreviewUpdateFromInstanceArgs,
) -> Result<LayerDiffResult, String> {
    let store = read_store(&app)?;
    let spec = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;

    let instances_dir = crate::app_instances_dir(&app)?;
    let lock = crate::read_lockfile(&instances_dir, &args.instance_id)?;

    let (spec_entries, _, _) = reduce_layers(&spec);
    let instance_entries = lock
        .entries
        .iter()
        .filter(|e| {
            (e.source == "modrinth" || e.source == "curseforge")
                && matches!(
                    normalize_content_type(&e.content_type).as_str(),
                    "mods" | "resourcepacks" | "shaderpacks" | "datapacks"
                )
        })
        .map(lock_entry_to_mod_entry)
        .collect::<Vec<_>>();

    let (added, removed, overridden) = diff_entries(&spec_entries, &instance_entries);
    Ok(LayerDiffResult {
        layer_id: None,
        added,
        removed,
        overridden,
        conflicts: vec![],
        warnings: vec!["Preview only. Apply will create explicit overrides after confirmation.".to_string()],
    })
}

#[tauri::command]
pub fn apply_update_modpack_from_instance(
    app: tauri::AppHandle,
    args: ApplyUpdateFromInstanceArgs,
) -> Result<ModpackSpec, String> {
    let mut store = read_store(&app)?;
    let mut spec = get_spec(&store, &args.modpack_id).ok_or_else(|| "Modpack spec not found".to_string())?;

    let preview = preview_update_modpack_from_instance(
        app.clone(),
        PreviewUpdateFromInstanceArgs {
            instance_id: args.instance_id,
            modpack_id: args.modpack_id.clone(),
        },
    )?;

    let layer_name = args
        .layer_name
        .as_deref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "Instance Overrides".to_string());

    let layer_idx = spec
        .layers
        .iter()
        .position(|l| l.name.eq_ignore_ascii_case(&layer_name))
        .unwrap_or_else(|| {
            spec.layers.push(Layer {
                id: format!("layer_{}", crate::now_millis()),
                name: layer_name.clone(),
                source: Some(LayerSource {
                    kind: "instance_sync".to_string(),
                    source: None,
                    project_id: None,
                    spec_id: None,
                    imported_at: Some(crate::now_iso()),
                }),
                is_frozen: false,
                entries_delta: EntriesDelta::default(),
            });
            spec.layers.len() - 1
        });

    if spec.layers[layer_idx].is_frozen {
        return Err("Target overrides layer is frozen. Unfreeze before applying updates.".to_string());
    }

    let layer = &mut spec.layers[layer_idx];
    append_unique_entries(&mut layer.entries_delta.add, &preview.added);
    append_unique_entries(&mut layer.entries_delta.override_entries, &preview.overridden);
    append_unique_remove_keys(&mut layer.entries_delta.remove, &preview.removed);

    spec.updated_at = crate::now_iso();
    normalize_spec_for_write(&mut spec);
    upsert_spec(&mut store, spec.clone());
    write_store(&app, &store)?;

    Ok(spec)
}

#[tauri::command]
pub fn rollback_instance_to_last_modpack_snapshot(
    app: tauri::AppHandle,
    args: InstanceArgs,
) -> Result<crate::RollbackResult, String> {
    let store = read_store(&app)?;
    let link = get_instance_link(&store, &args.instance_id)
        .ok_or_else(|| "Instance has no modpack link".to_string())?;
    let lock_snapshot_id = link
        .last_lock_snapshot_id
        .ok_or_else(|| "No modpack lock snapshot recorded for this instance".to_string())?;
    let lock_snapshot = get_lock_snapshot(&store, &lock_snapshot_id)
        .ok_or_else(|| "Recorded lock snapshot not found".to_string())?;

    let instance_snapshot_id = lock_snapshot
        .instance_snapshot_id
        .ok_or_else(|| "No instance snapshot id recorded for rollback".to_string())?;

    let instances_dir = crate::app_instances_dir(&app)?;
    let _ = crate::find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instances_dir.join(&args.instance_id);
    let snapshots = crate::list_snapshots(&instance_dir)?;
    let selected = snapshots
        .into_iter()
        .find(|s| s.id == instance_snapshot_id)
        .ok_or_else(|| "Snapshot not found".to_string())?;

    let snapshot_dir = crate::snapshots_dir(&instance_dir).join(&selected.id);
    let lock_raw = std::fs::read_to_string(crate::snapshot_lock_path(&snapshot_dir))
        .map_err(|e| format!("read snapshot lock failed: {e}"))?;
    let lock: crate::Lockfile =
        serde_json::from_str(&lock_raw).map_err(|e| format!("parse snapshot lock failed: {e}"))?;

    let restored_files = crate::restore_instance_content_zip(
        &crate::snapshot_content_zip_path(&snapshot_dir),
        &instance_dir,
    )?;
    crate::write_lockfile(&instances_dir, &args.instance_id, &lock)?;

    Ok(crate::RollbackResult {
        snapshot_id: selected.id,
        created_at: selected.created_at,
        restored_files,
        message: "Rollback complete.".to_string(),
    })
}

#[tauri::command]
pub fn migrate_legacy_creator_presets(
    app: tauri::AppHandle,
    args: MigrateLegacyCreatorPresetsArgs,
) -> Result<MigrationReport, String> {
    let (report, specs) = migrate_legacy_payload(&args.payload);
    let mut store = read_store(&app)?;
    for mut spec in specs {
        normalize_spec_for_write(&mut spec);
        upsert_spec(&mut store, spec);
    }
    write_store(&app, &store)?;
    Ok(report)
}

#[tauri::command]
pub fn seed_dev_modpack_data(
    app: tauri::AppHandle,
    args: SeedDevModpackDataArgs,
) -> Result<SeedDevResult, String> {
    let (result, mut spec) = crate::modpack::dev_seed::seed_dev_data(&app, args.instance_name.as_deref())?;
    let mut store = read_store(&app)?;
    normalize_spec_for_write(&mut spec);
    upsert_spec(&mut store, spec);
    write_store(&app, &store)?;
    Ok(result)
}

pub fn legacy_creator_preset_to_spec(preset: &crate::CreatorPreset) -> ModpackSpec {
    let mut spec = make_base_spec(
        format!("modpack_{}", crate::now_millis()),
        preset.name.clone(),
        preset.created_at.clone(),
    );

    let entries = preset
        .entries
        .iter()
        .cloned()
        .map(creator_entry_to_mod_entry)
        .map(normalize_entry_for_add)
        .collect::<Vec<_>>();
    if let Some(layer) = spec.layers.iter_mut().find(|l| l.id == "layer_user") {
        layer.entries_delta.add = entries;
    }
    spec.updated_at = crate::now_iso();
    spec
}

fn normalize_spec_for_write(spec: &mut ModpackSpec) {
    if spec.id.trim().is_empty() {
        spec.id = format!("modpack_{}", crate::now_millis());
    }
    if spec.name.trim().is_empty() {
        spec.name = "Untitled modpack".to_string();
    }
    if spec.created_at.trim().is_empty() {
        spec.created_at = crate::now_iso();
    }
    spec.updated_at = crate::now_iso();

    for layer in &mut spec.layers {
        if layer.id.trim().is_empty() {
            layer.id = format!("layer_{}", crate::now_millis());
        }
        if layer.name.trim().is_empty() {
            layer.name = "Layer".to_string();
        }
        layer.entries_delta.add = layer
            .entries_delta
            .add
            .drain(..)
            .map(normalize_entry_for_add)
            .collect::<Vec<_>>();
        layer.entries_delta.override_entries = layer
            .entries_delta
            .override_entries
            .drain(..)
            .map(normalize_entry_for_add)
            .collect::<Vec<_>>();
    }

    ensure_default_profiles(spec);
}

fn lock_entry_to_mod_entry(entry: &crate::LockEntry) -> ModEntry {
    normalize_entry_for_add(ModEntry {
        provider: entry.source.clone(),
        project_id: entry.project_id.clone(),
        slug: Some(entry.name.clone()),
        content_type: normalize_content_type(&entry.content_type),
        required: true,
        pin: Some(entry.version_id.clone()),
        channel_policy: "stable".to_string(),
        fallback_policy: "inherit".to_string(),
        replacement_group: None,
        notes: Some(entry.name.clone()),
        disabled_by_default: !entry.enabled,
        optional: false,
        target_scope: if entry.target_worlds.is_empty() {
            "instance".to_string()
        } else {
            "world".to_string()
        },
        target_worlds: entry.target_worlds.clone(),
        local_file_name: None,
        local_file_path: None,
        local_sha512: None,
        local_fingerprints: vec![],
    })
}

fn creator_entry_to_mod_entry(entry: crate::CreatorPresetEntry) -> ModEntry {
    normalize_entry_for_add(ModEntry {
        provider: entry.source,
        project_id: entry.project_id,
        slug: Some(entry.title.clone()),
        content_type: normalize_content_type(&entry.content_type),
        required: true,
        pin: entry.pinned_version,
        channel_policy: "stable".to_string(),
        fallback_policy: "inherit".to_string(),
        replacement_group: None,
        notes: Some(entry.title),
        disabled_by_default: !entry.enabled,
        optional: false,
        target_scope: if entry.target_worlds.is_empty() {
            "instance".to_string()
        } else {
            "world".to_string()
        },
        target_worlds: entry.target_worlds,
        local_file_name: None,
        local_file_path: None,
        local_sha512: None,
        local_fingerprints: vec![],
    })
}

fn append_unique_entries(target: &mut Vec<ModEntry>, source: &[ModEntry]) {
    let mut seen = target.iter().map(entry_key_for).collect::<std::collections::HashSet<_>>();
    for item in source {
        let key = entry_key_for(item);
        if seen.insert(key) {
            target.push(item.clone());
        }
    }
}

fn append_unique_remove_keys(target: &mut Vec<EntryKey>, source: &[EntryKey]) {
    let mut seen = target
        .iter()
        .map(|k| format!("{}:{}:{}", k.provider, k.content_type, k.project_id))
        .collect::<std::collections::HashSet<_>>();

    for item in source {
        let key = format!("{}:{}:{}", item.provider, item.content_type, item.project_id);
        if seen.insert(key) {
            target.push(item.clone());
        }
    }
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
