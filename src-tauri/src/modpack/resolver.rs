use crate::modpack::layers::{entry_key, entry_key_for, reduce_layers};
use crate::modpack::types::{
    FailedMod, ModEntry, ModpackSpec, ResolutionConflict, ResolutionPlan, ResolutionSettings,
    ResolvedMod, TargetInstanceSnapshot,
};
use reqwest::blocking::Client;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
struct ResolveCandidate {
    resolved: ResolvedMod,
    fallback_tier: u8,
    fallback_distance: u32,
    channel_rank: u8,
}

#[derive(Debug, Clone)]
struct McDistance {
    tier: u8,
    distance: u32,
}

pub fn resolve_modpack(
    client: &Client,
    instance: &crate::Instance,
    spec: &ModpackSpec,
    profile_id: Option<&str>,
    settings_override: Option<ResolutionSettings>,
) -> Result<ResolutionPlan, String> {
    let (mut computed_entries, mut conflicts, mut warnings) = reduce_layers(spec);
    let settings = settings_override.unwrap_or_else(|| spec.settings.clone());

    apply_profile(&mut computed_entries, spec, profile_id);

    let target = TargetInstanceSnapshot {
        id: instance.id.clone(),
        name: instance.name.clone(),
        mc_version: instance.mc_version.clone(),
        loader: instance.loader.clone(),
        loader_version: None,
        java_version: None,
    };

    let mut resolved_mods = Vec::new();
    let mut failed_mods = Vec::new();

    let mut fallback_hits = 0usize;
    let mut loose_hits = 0usize;

    for entry in &computed_entries {
        match resolve_single_entry(client, instance, entry, &settings) {
            Ok(candidate) => {
                if candidate.fallback_tier > 0 {
                    fallback_hits += 1;
                }
                if candidate.fallback_tier >= 2 {
                    loose_hits += 1;
                }
                resolved_mods.push(candidate.resolved);
            }
            Err(failure) => failed_mods.push(failure),
        }
    }

    let dependency_result = resolve_dependencies(
        client,
        instance,
        &settings,
        &mut resolved_mods,
        &mut failed_mods,
    )?;
    warnings.extend(dependency_result.warnings);
    fallback_hits += dependency_result.fallback_hits;
    loose_hits += dependency_result.loose_hits;

    conflicts.extend(detect_conflicts(&resolved_mods));

    let confidence_score = compute_confidence(
        fallback_hits,
        loose_hits,
        &failed_mods,
        &warnings,
        &conflicts,
    );
    let confidence_label = if confidence_score >= 80.0 {
        "High".to_string()
    } else if confidence_score >= 55.0 {
        "Medium".to_string()
    } else {
        "Risky".to_string()
    };

    Ok(ResolutionPlan {
        id: format!("plan_{}", crate::now_millis()),
        modpack_id: spec.id.clone(),
        modpack_updated_at_stamp: spec.updated_at.clone(),
        target,
        profile_id: profile_id.map(|v| v.to_string()),
        settings,
        resolved_mods,
        failed_mods,
        conflicts,
        warnings,
        confidence_score,
        confidence_label,
        created_at: crate::now_iso(),
    })
}

fn apply_profile(entries: &mut [ModEntry], spec: &ModpackSpec, profile_id: Option<&str>) {
    let profile = profile_id
        .and_then(|id| spec.profiles.iter().find(|p| p.id == id))
        .or_else(|| spec.profiles.iter().find(|p| p.id == "recommended"));

    let Some(profile) = profile else {
        return;
    };

    for entry in entries {
        if !entry.optional {
            continue;
        }
        let key = entry_key_for(entry);
        let enabled = profile.optional_entry_states.get(&key).copied().unwrap_or(true);
        entry.disabled_by_default = !enabled;
    }
}

#[derive(Default)]
struct DependencyResolutionSummary {
    warnings: Vec<String>,
    fallback_hits: usize,
    loose_hits: usize,
}

fn resolve_dependencies(
    client: &Client,
    instance: &crate::Instance,
    settings: &ResolutionSettings,
    resolved_mods: &mut Vec<ResolvedMod>,
    failed_mods: &mut Vec<FailedMod>,
) -> Result<DependencyResolutionSummary, String> {
    let mut summary = DependencyResolutionSummary::default();

    let mut resolved_keys = resolved_mods
        .iter()
        .map(|m| entry_key(&m.source, &m.project_id, &m.content_type))
        .collect::<HashSet<_>>();

    let mut pending = VecDeque::new();
    for mod_item in resolved_mods.iter() {
        if mod_item.content_type != "mods" {
            continue;
        }
        pending.push_back((
            mod_item.source.clone(),
            mod_item.project_id.clone(),
            mod_item.name.clone(),
            mod_item.version_id.clone(),
            mod_item.required,
        ));
    }

    let mut visited_parent = HashSet::new();
    while let Some((source, project_id, parent_name, version_id, required)) = pending.pop_front() {
        let parent_key = format!("{}:{}", source, version_id);
        if !visited_parent.insert(parent_key) {
            continue;
        }

        let mut required_deps: Vec<(String, String)> = Vec::new();

        if source == "modrinth" {
            let versions = crate::fetch_project_versions(client, &project_id)
                .map_err(|e| format!("dependency lookup failed for {}: {}", parent_name, e))?;
            if let Some(version) = versions.into_iter().find(|v| v.id == version_id) {
                for dep in version.dependencies {
                    if !dep.dependency_type.eq_ignore_ascii_case("required") {
                        continue;
                    }
                    let Some(dep_project_id) = dep.project_id else {
                        continue;
                    };
                    required_deps.push(("modrinth".to_string(), dep_project_id));
                }
            }
        } else if source == "curseforge" {
            let Some(api_key) = crate::curseforge_api_key() else {
                summary.warnings.push(
                    "Dependency detection for CurseForge skipped because API key is unavailable."
                        .to_string(),
                );
                continue;
            };
            let mod_id = crate::parse_curseforge_project_id(&project_id)?;
            let files = crate::fetch_curseforge_files(client, &api_key, mod_id)?;
            let file_id = parse_curseforge_file_id(&version_id);
            if let Some(file) = files.into_iter().find(|f| Some(f.id) == file_id) {
                for dep in file.dependencies {
                    if dep.mod_id <= 0 || dep.relation_type != 3 {
                        continue;
                    }
                    required_deps.push(("curseforge".to_string(), format!("cf:{}", dep.mod_id)));
                }
            }
        }

        for (dep_source, dep_project_id) in required_deps {
            let dep_key = entry_key(&dep_source, &dep_project_id, "mods");
            if resolved_keys.contains(&dep_key) {
                continue;
            }

            if settings.dependency_mode.eq_ignore_ascii_case("auto_add") {
                let dep_entry = ModEntry {
                    provider: dep_source.clone(),
                    project_id: dep_project_id.clone(),
                    slug: None,
                    content_type: "mods".to_string(),
                    required: true,
                    pin: None,
                    channel_policy: settings.channel_allowance.clone(),
                    fallback_policy: settings.global_fallback_mode.clone(),
                    replacement_group: None,
                    notes: Some(format!("Auto-added dependency for {}", parent_name)),
                    disabled_by_default: false,
                    optional: false,
                    target_scope: "instance".to_string(),
                    target_worlds: vec![],
                };
                match resolve_single_entry(client, instance, &dep_entry, settings) {
                    Ok(candidate) => {
                        if candidate.fallback_tier > 0 {
                            summary.fallback_hits += 1;
                        }
                        if candidate.fallback_tier >= 2 {
                            summary.loose_hits += 1;
                        }
                        let mut resolved = candidate.resolved;
                        resolved.added_by_dependency = true;
                        resolved.rationale_text = format!(
                            "Added because required by '{}' and dependency mode is AutoAdd.",
                            parent_name
                        );
                        pending.push_back((
                            resolved.source.clone(),
                            resolved.project_id.clone(),
                            resolved.name.clone(),
                            resolved.version_id.clone(),
                            true,
                        ));
                        resolved_keys.insert(entry_key_for_resolved(&resolved));
                        resolved_mods.push(resolved);
                    }
                    Err(mut failure) => {
                        failure.reason_code = "DependencyIncompatible".to_string();
                        failure.reason_text = format!(
                            "Required dependency '{}' for '{}' could not be resolved: {}",
                            dep_project_id, parent_name, failure.reason_text
                        );
                        failure.required = required;
                        failed_mods.push(failure);
                    }
                }
            } else {
                failed_mods.push(FailedMod {
                    source: dep_source.clone(),
                    content_type: "mods".to_string(),
                    project_id: dep_project_id.clone(),
                    name: dep_project_id.clone(),
                    reason_code: "DependencyMissing".to_string(),
                    reason_text: format!(
                        "Required dependency '{}' was not selected for '{}'.",
                        dep_project_id, parent_name
                    ),
                    actionable_hint: "Enable AutoAdd dependencies, add dependency manually, or mark parent optional."
                        .to_string(),
                    constraints_snapshot: format!(
                        "parent={} ({}) target={} {}",
                        parent_name, source, instance.loader, instance.mc_version
                    ),
                    required,
                });
            }
        }
    }

    Ok(summary)
}

fn detect_conflicts(resolved_mods: &[ResolvedMod]) -> Vec<ResolutionConflict> {
    let mut conflicts = Vec::new();
    let mut by_file = HashMap::<String, Vec<String>>::new();

    for item in resolved_mods {
        by_file
            .entry(item.filename.to_lowercase())
            .or_default()
            .push(format!("{}:{}", item.source, item.project_id));
    }

    for (filename, keys) in by_file {
        if keys.len() <= 1 {
            continue;
        }
        conflicts.push(ResolutionConflict {
            code: "FILE_COLLISION".to_string(),
            message: format!("Multiple resolved entries map to filename '{}'", filename),
            keys,
        });
    }

    conflicts
}

fn compute_confidence(
    fallback_hits: usize,
    loose_hits: usize,
    failed_mods: &[FailedMod],
    warnings: &[String],
    conflicts: &[ResolutionConflict],
) -> f64 {
    let mut score = 100.0;
    score -= (fallback_hits as f64) * 7.0;
    score -= (loose_hits as f64) * 10.0;
    score -= (warnings.len() as f64) * 2.0;
    score -= (conflicts.len() as f64) * 8.0;

    for failure in failed_mods {
        if failure.required {
            score -= 16.0;
        } else {
            score -= 6.0;
        }
    }

    score.clamp(0.0, 100.0)
}

fn resolve_single_entry(
    client: &Client,
    instance: &crate::Instance,
    entry: &ModEntry,
    settings: &ResolutionSettings,
) -> Result<ResolveCandidate, FailedMod> {
    let provider = entry.provider.trim().to_lowercase();
    let content_type = normalize_content_type(&entry.content_type);
    let resolved_name = entry
        .slug
        .clone()
        .or_else(|| entry.notes.clone())
        .unwrap_or_else(|| entry.project_id.clone());
    let enabled = !entry.disabled_by_default;

    if provider == "modrinth" {
        let versions = crate::fetch_project_versions(client, &entry.project_id).map_err(|e| FailedMod {
            source: provider.clone(),
            content_type: content_type.clone(),
            project_id: entry.project_id.clone(),
            name: resolved_name.clone(),
            reason_code: "ProviderError".to_string(),
            reason_text: format!("Failed to query Modrinth versions: {}", e),
            actionable_hint: "Retry or verify project ID/slug.".to_string(),
            constraints_snapshot: format!("{} + {}", instance.loader, instance.mc_version),
            required: entry.required,
        })?;

        let selected = select_modrinth_version(versions, instance, entry, settings).ok_or_else(|| {
            FailedMod {
                source: provider.clone(),
                content_type: content_type.clone(),
                project_id: entry.project_id.clone(),
                name: resolved_name.clone(),
                reason_code: "NoCompatibleMinecraftVersion".to_string(),
                reason_text: format!(
                    "No compatible Modrinth file found for target {} {}.",
                    instance.loader, instance.mc_version
                ),
                actionable_hint:
                    "Try smart/loose fallback, allow beta channel, or choose a compatible loader/version."
                        .to_string(),
                constraints_snapshot: format!("{} + {}", instance.loader, instance.mc_version),
                required: entry.required,
            }
        })?;

        let file = selected
            .resolved
            .download_url
            .as_ref()
            .map(|_| selected.resolved.filename.clone())
            .unwrap_or_else(|| selected.resolved.filename.clone());

        let mut resolved = selected.resolved;
        resolved.name = if resolved.name.trim().is_empty() {
            resolved_name
        } else {
            resolved.name
        };
        resolved.filename = file;
        resolved.enabled = enabled;
        resolved.required = entry.required;
        resolved.target_worlds = if content_type == "datapacks" {
            entry.target_worlds.clone()
        } else {
            vec![]
        };

        return Ok(ResolveCandidate {
            resolved,
            fallback_tier: selected.fallback_tier,
            fallback_distance: selected.fallback_distance,
            channel_rank: selected.channel_rank,
        });
    }

    if provider == "curseforge" {
        let api_key = crate::curseforge_api_key().ok_or_else(|| FailedMod {
            source: provider.clone(),
            content_type: content_type.clone(),
            project_id: entry.project_id.clone(),
            name: resolved_name.clone(),
            reason_code: "ProviderError".to_string(),
            reason_text: crate::missing_curseforge_key_message(),
            actionable_hint: "Configure CurseForge key for dev or use release-injected key.".to_string(),
            constraints_snapshot: format!("{} + {}", instance.loader, instance.mc_version),
            required: entry.required,
        })?;

        let mod_id = crate::parse_curseforge_project_id(&entry.project_id).map_err(|e| FailedMod {
            source: provider.clone(),
            content_type: content_type.clone(),
            project_id: entry.project_id.clone(),
            name: resolved_name.clone(),
            reason_code: "ProjectNotFound".to_string(),
            reason_text: e,
            actionable_hint: "Use a numeric CurseForge project id or cf:<id>.".to_string(),
            constraints_snapshot: format!("{} + {}", instance.loader, instance.mc_version),
            required: entry.required,
        })?;

        let project = crate::fetch_curseforge_project(client, &api_key, mod_id).map_err(|e| FailedMod {
            source: provider.clone(),
            content_type: content_type.clone(),
            project_id: entry.project_id.clone(),
            name: resolved_name.clone(),
            reason_code: "ProjectNotFound".to_string(),
            reason_text: e,
            actionable_hint: "Verify project id and provider.".to_string(),
            constraints_snapshot: format!("{} + {}", instance.loader, instance.mc_version),
            required: entry.required,
        })?;

        let files = crate::fetch_curseforge_files(client, &api_key, mod_id).map_err(|e| FailedMod {
            source: provider.clone(),
            content_type: content_type.clone(),
            project_id: entry.project_id.clone(),
            name: project.name.clone(),
            reason_code: "ProviderError".to_string(),
            reason_text: e,
            actionable_hint: "Retry after a short delay.".to_string(),
            constraints_snapshot: format!("{} + {}", instance.loader, instance.mc_version),
            required: entry.required,
        })?;

        let selected = select_curseforge_file(files, instance, entry, settings, mod_id).ok_or_else(|| {
            FailedMod {
                source: provider.clone(),
                content_type: content_type.clone(),
                project_id: entry.project_id.clone(),
                name: project.name.clone(),
                reason_code: "NoCompatibleLoader".to_string(),
                reason_text: format!(
                    "No compatible CurseForge file found for target {} {}.",
                    instance.loader, instance.mc_version
                ),
                actionable_hint:
                    "Try smart/loose fallback, allow prerelease channel, or choose compatible loader/version."
                        .to_string(),
                constraints_snapshot: format!("{} + {}", instance.loader, instance.mc_version),
                required: entry.required,
            }
        })?;

        let mut resolved = selected.resolved;
        resolved.name = if project.name.trim().is_empty() {
            resolved_name
        } else {
            project.name
        };
        resolved.enabled = enabled;
        resolved.required = entry.required;
        resolved.target_worlds = if content_type == "datapacks" {
            entry.target_worlds.clone()
        } else {
            vec![]
        };

        return Ok(ResolveCandidate {
            resolved,
            fallback_tier: selected.fallback_tier,
            fallback_distance: selected.fallback_distance,
            channel_rank: selected.channel_rank,
        });
    }

    Err(FailedMod {
        source: provider,
        content_type,
        project_id: entry.project_id.clone(),
        name: resolved_name,
        reason_code: "ProviderError".to_string(),
        reason_text: "Unsupported provider. Expected modrinth or curseforge.".to_string(),
        actionable_hint: "Update entry provider.".to_string(),
        constraints_snapshot: format!("{} + {}", instance.loader, instance.mc_version),
        required: entry.required,
    })
}

fn select_modrinth_version(
    versions: Vec<crate::ModrinthVersion>,
    instance: &crate::Instance,
    entry: &ModEntry,
    settings: &ResolutionSettings,
) -> Option<ResolveCandidate> {
    let content_type = normalize_content_type(&entry.content_type);
    let fallback_mode = resolved_fallback_mode(entry, settings);
    let target_parts = parse_release_parts(&instance.mc_version);
    let target_loader = instance.loader.to_lowercase();

    let pin = entry.pin.clone();
    if let Some(pin_value) = pin {
        for version in versions {
            if version.id == pin_value || version.version_number == pin_value {
                let file = pick_modrinth_file(&version)?;
                return Some(ResolveCandidate {
                    resolved: ResolvedMod {
                        source: "modrinth".to_string(),
                        content_type,
                        project_id: entry.project_id.clone(),
                        name: version
                            .name
                            .clone()
                            .unwrap_or_else(|| entry.project_id.clone()),
                        version_id: version.id.clone(),
                        version_number: version.version_number.clone(),
                        filename: crate::sanitize_filename(&file.filename),
                        download_url: Some(file.url.clone()),
                        curseforge_file_id: None,
                        hashes: file.hashes.clone(),
                        enabled: !entry.disabled_by_default,
                        target_worlds: vec![],
                        rationale_text: format!("Pinned version '{}' was selected.", pin_value),
                        added_by_dependency: false,
                        required: entry.required,
                    },
                    fallback_tier: 0,
                    fallback_distance: 0,
                    channel_rank: 0,
                });
            }
        }
        return None;
    }

    let mut candidates = Vec::new();
    for version in versions {
        if !modrinth_loader_matches(&version, &target_loader, &content_type) {
            continue;
        }
        let Some(distance) = pick_best_mc_distance(
            &version.game_versions,
            &instance.mc_version,
            target_parts,
            &fallback_mode,
            settings,
        ) else {
            continue;
        };
        let channel = infer_channel_rank(
            &format!(
                "{} {}",
                version.version_number,
                version.name.clone().unwrap_or_default()
            ),
            entry,
            settings,
        )?;
        let file = pick_modrinth_file(&version)?;
        candidates.push(ResolveCandidate {
            resolved: ResolvedMod {
                source: "modrinth".to_string(),
                content_type: content_type.clone(),
                project_id: entry.project_id.clone(),
                name: version
                    .name
                    .clone()
                    .unwrap_or_else(|| entry.project_id.clone()),
                version_id: version.id.clone(),
                version_number: version.version_number.clone(),
                filename: crate::sanitize_filename(&file.filename),
                download_url: Some(file.url.clone()),
                curseforge_file_id: None,
                hashes: file.hashes.clone(),
                enabled: !entry.disabled_by_default,
                target_worlds: vec![],
                rationale_text: rationale_text("Modrinth", distance.tier, distance.distance, channel),
                added_by_dependency: false,
                required: entry.required,
            },
            fallback_tier: distance.tier,
            fallback_distance: distance.distance,
            channel_rank: channel,
        });
    }

    candidates.sort_by(|a, b| {
        a.fallback_tier
            .cmp(&b.fallback_tier)
            .then(a.fallback_distance.cmp(&b.fallback_distance))
            .then(a.channel_rank.cmp(&b.channel_rank))
            .then(b.resolved.version_number.cmp(&a.resolved.version_number))
    });

    candidates.into_iter().next()
}

fn select_curseforge_file(
    files: Vec<crate::CurseforgeFile>,
    instance: &crate::Instance,
    entry: &ModEntry,
    settings: &ResolutionSettings,
    mod_id: i64,
) -> Option<ResolveCandidate> {
    let content_type = normalize_content_type(&entry.content_type);
    let fallback_mode = resolved_fallback_mode(entry, settings);
    let target_parts = parse_release_parts(&instance.mc_version);
    let target_loader = instance.loader.to_lowercase();

    let pin_file_id = entry
        .pin
        .as_ref()
        .and_then(|v| parse_curseforge_file_id(v));

    let mut candidates = Vec::new();
    for file in files {
        if file.file_name.trim().is_empty() {
            continue;
        }

        if let Some(id) = pin_file_id {
            if file.id != id {
                continue;
            }
        }

        if !curseforge_loader_matches(&file, &target_loader) {
            continue;
        }

        let Some(distance) = pick_best_mc_distance(
            &file.game_versions,
            &instance.mc_version,
            target_parts,
            &fallback_mode,
            settings,
        ) else {
            continue;
        };

        let channel = infer_channel_rank(
            &format!("{} {}", file.file_name, file.display_name),
            entry,
            settings,
        )?;

        let filename = crate::sanitize_filename(&file.file_name);
        if filename.is_empty() {
            continue;
        }

        candidates.push(ResolveCandidate {
            resolved: ResolvedMod {
                source: "curseforge".to_string(),
                content_type: content_type.clone(),
                project_id: format!("cf:{}", mod_id),
                name: entry.project_id.clone(),
                version_id: format!("cf_file:{}", file.id),
                version_number: if file.display_name.trim().is_empty() {
                    file.file_name.clone()
                } else {
                    file.display_name.clone()
                },
                filename,
                download_url: file.download_url.clone(),
                curseforge_file_id: Some(file.id),
                hashes: crate::parse_cf_hashes(&file),
                enabled: !entry.disabled_by_default,
                target_worlds: vec![],
                rationale_text: rationale_text("CurseForge", distance.tier, distance.distance, channel),
                added_by_dependency: false,
                required: entry.required,
            },
            fallback_tier: distance.tier,
            fallback_distance: distance.distance,
            channel_rank: channel,
        });
    }

    candidates.sort_by(|a, b| {
        a.fallback_tier
            .cmp(&b.fallback_tier)
            .then(a.fallback_distance.cmp(&b.fallback_distance))
            .then(a.channel_rank.cmp(&b.channel_rank))
            .then(b.resolved.version_number.cmp(&a.resolved.version_number))
    });

    candidates.into_iter().next()
}

fn rationale_text(provider: &str, fallback_tier: u8, distance: u32, channel_rank: u8) -> String {
    let fallback_label = match fallback_tier {
        0 => "exact match",
        1 => "smart fallback",
        _ => "loose fallback",
    };
    let channel = match channel_rank {
        0 => "stable",
        1 => "beta/rc",
        _ => "alpha",
    };
    format!(
        "Chosen from {} using {} (distance {}) with {} channel.",
        provider, fallback_label, distance, channel
    )
}

fn pick_modrinth_file(version: &crate::ModrinthVersion) -> Option<crate::ModrinthVersionFile> {
    version
        .files
        .iter()
        .find(|f| f.primary.unwrap_or(false))
        .or_else(|| version.files.first())
        .cloned()
}

fn modrinth_loader_matches(version: &crate::ModrinthVersion, target_loader: &str, content_type: &str) -> bool {
    if content_type != "mods" {
        return true;
    }
    if version.loaders.is_empty() {
        return true;
    }
    version.loaders.iter().any(|loader| {
        let lc = loader.trim().to_lowercase();
        lc == target_loader
            || (target_loader == "neoforge" && (lc == "neo forge" || lc == "neo-forge"))
            || lc == "minecraft"
    })
}

fn curseforge_loader_matches(file: &crate::CurseforgeFile, target_loader: &str) -> bool {
    let values = file
        .game_versions
        .iter()
        .map(|v| v.trim().to_lowercase())
        .collect::<Vec<_>>();
    let has_loader_tag = values.iter().any(|v| {
        v == "fabric" || v == "forge" || v == "quilt" || v == "neoforge" || v == "vanilla"
    });
    if !has_loader_tag {
        return true;
    }
    values.iter().any(|v| {
        v == target_loader
            || (target_loader == "neoforge" && (v == "neo forge" || v == "neo-forge"))
            || (target_loader == "vanilla" && v == "minecraft")
    })
}

fn resolved_fallback_mode(entry: &ModEntry, settings: &ResolutionSettings) -> String {
    let entry_mode = entry.fallback_policy.trim().to_lowercase();
    if entry_mode.is_empty() || entry_mode == "inherit" {
        return settings.global_fallback_mode.trim().to_lowercase();
    }
    entry_mode
}

fn infer_channel_rank(text: &str, entry: &ModEntry, settings: &ResolutionSettings) -> Option<u8> {
    let lower = text.to_lowercase();
    let candidate = if lower.contains("alpha") || lower.contains("snapshot") {
        2
    } else if lower.contains("beta") || lower.contains("pre") || lower.contains("rc") {
        1
    } else {
        0
    };

    let channel_policy = if entry.channel_policy.trim().is_empty() || entry.channel_policy == "inherit" {
        settings.channel_allowance.clone()
    } else {
        entry.channel_policy.clone()
    }
    .trim()
    .to_lowercase();

    let max_rank = if channel_policy.contains("alpha") {
        2
    } else if channel_policy.contains("beta") || channel_policy.contains("rc") {
        1
    } else {
        0
    };

    if candidate > max_rank {
        return None;
    }
    Some(candidate)
}

fn pick_best_mc_distance(
    advertised_versions: &[String],
    target_mc: &str,
    target_parts: Option<(i32, i32, i32)>,
    fallback_mode: &str,
    settings: &ResolutionSettings,
) -> Option<McDistance> {
    if advertised_versions.iter().any(|v| v.trim() == target_mc) {
        return Some(McDistance { tier: 0, distance: 0 });
    }

    let target = target_parts?;
    let mut best: Option<McDistance> = None;

    for advertised in advertised_versions {
        let Some(parts) = parse_release_parts(advertised) else {
            continue;
        };
        if parts.0 != target.0 && !settings.allow_cross_major {
            continue;
        }
        if parts.0 == target.0 && parts.1 != target.1 && !settings.allow_cross_minor {
            continue;
        }

        if parts.0 > target.0 {
            continue;
        }
        if parts.0 == target.0 && parts.1 > target.1 {
            continue;
        }
        if parts.0 == target.0 && parts.1 == target.1 && parts.2 > target.2 {
            continue;
        }

        let distance = ((target.0 - parts.0).unsigned_abs() * 100)
            + ((target.1 - parts.1).unsigned_abs() * 10)
            + (target.2 - parts.2).unsigned_abs();

        let tier = if parts.0 == target.0 && parts.1 == target.1 {
            1
        } else {
            2
        };

        if distance > settings.max_fallback_distance {
            continue;
        }

        let candidate = McDistance { tier, distance };
        match &best {
            Some(existing) => {
                if candidate.tier < existing.tier
                    || (candidate.tier == existing.tier && candidate.distance < existing.distance)
                {
                    best = Some(candidate);
                }
            }
            None => best = Some(candidate),
        }
    }

    let mode = fallback_mode.trim().to_lowercase();
    match mode.as_str() {
        "strict" => None,
        "smart" => best.filter(|b| b.tier <= 1),
        "loose" => best,
        _ => best.filter(|b| b.tier <= 1),
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

fn parse_release_parts(input: &str) -> Option<(i32, i32, i32)> {
    let normalized = input.trim();
    if normalized.is_empty() {
        return None;
    }
    let mut numbers = Vec::new();
    for token in normalized.split(|c: char| !c.is_ascii_digit()) {
        if token.is_empty() {
            continue;
        }
        if let Ok(value) = token.parse::<i32>() {
            numbers.push(value);
            if numbers.len() >= 3 {
                break;
            }
        }
    }
    if numbers.len() < 2 {
        return None;
    }
    let major = numbers[0];
    let minor = numbers[1];
    let patch = *numbers.get(2).unwrap_or(&0);
    Some((major, minor, patch))
}

fn parse_curseforge_file_id(raw: &str) -> Option<i64> {
    raw.trim()
        .trim_start_matches("cf_file:")
        .trim()
        .parse::<i64>()
        .ok()
}

fn entry_key_for_resolved(item: &ResolvedMod) -> String {
    entry_key(&item.source, &item.project_id, &item.content_type)
}
