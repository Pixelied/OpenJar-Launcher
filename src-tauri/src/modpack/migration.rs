use crate::modpack::layers::{make_base_spec, normalize_entry_for_add};
use crate::modpack::types::{MigrationReport, MigrationSkippedItem, ModEntry, ModpackSpec};

pub fn migrate_legacy_payload(payload: &serde_json::Value) -> (MigrationReport, Vec<ModpackSpec>) {
    let mut created_specs = Vec::new();
    let mut skipped = Vec::new();

    let values = if let Some(array) = payload.as_array() {
        array.clone()
    } else if let Some(array) = payload.get("presets").and_then(|v| v.as_array()) {
        array.clone()
    } else {
        vec![]
    };

    for (index, raw) in values.iter().enumerate() {
        let id = raw
            .get("id")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| format!("legacy_{}", index + 1));

        let name = raw
            .get("name")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| format!("Migrated preset {}", index + 1));

        let created_at = raw
            .get("created_at")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .unwrap_or_else(crate::now_iso);

        let Some(entries) = raw.get("entries").and_then(|v| v.as_array()) else {
            skipped.push(MigrationSkippedItem {
                id,
                name,
                reason: "No entries found".to_string(),
            });
            continue;
        };

        let mut spec = make_base_spec(
            format!("modpack_{}", crate::now_millis() + index as u128),
            name.clone(),
            created_at,
        );

        let mut migrated_entries = Vec::new();
        for entry in entries {
            let provider = entry
                .get("source")
                .and_then(|v| v.as_str())
                .map(|v| v.trim().to_lowercase())
                .unwrap_or_default();
            if provider != "modrinth" && provider != "curseforge" {
                continue;
            }
            let project_id = entry
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(|v| v.trim().to_string())
                .unwrap_or_default();
            if project_id.is_empty() {
                continue;
            }

            let content_type = entry
                .get("content_type")
                .and_then(|v| v.as_str())
                .unwrap_or("mods")
                .to_string();

            let enabled = entry
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let target_worlds = entry
                .get("target_worlds")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|w| w.as_str())
                        .map(|w| w.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let pin = entry
                .get("pinned_version")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string());

            let item = ModEntry {
                provider,
                project_id,
                slug: entry
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
                content_type,
                required: true,
                pin,
                channel_policy: "stable".to_string(),
                fallback_policy: "inherit".to_string(),
                replacement_group: None,
                notes: entry
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
                disabled_by_default: !enabled,
                optional: false,
                target_scope: if target_worlds.is_empty() {
                    "instance".to_string()
                } else {
                    "world".to_string()
                },
                target_worlds,
            };
            migrated_entries.push(normalize_entry_for_add(item));
        }

        if migrated_entries.is_empty() {
            skipped.push(MigrationSkippedItem {
                id,
                name,
                reason: "No valid Modrinth/CurseForge entries to migrate".to_string(),
            });
            continue;
        }

        if let Some(layer) = spec.layers.iter_mut().find(|l| l.id == "layer_user") {
            layer.entries_delta.add = migrated_entries;
        }
        spec.updated_at = crate::now_iso();

        created_specs.push(spec);
    }

    let report = MigrationReport {
        migrated_count: created_specs.len(),
        skipped_count: skipped.len(),
        skipped_items: skipped,
        created_spec_ids: created_specs.iter().map(|s| s.id.clone()).collect(),
    };

    (report, created_specs)
}
