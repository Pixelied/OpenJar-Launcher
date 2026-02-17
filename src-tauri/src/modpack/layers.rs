use crate::modpack::types::{
    EntryKey, Layer, ModEntry, ModpackSpec, ResolutionConflict,
};
use std::collections::{HashMap, HashSet};

fn normalize_content_type(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "mods" | "mod" => "mods".to_string(),
        "resourcepacks" | "resourcepack" => "resourcepacks".to_string(),
        "shaderpacks" | "shaderpack" | "shaders" => "shaderpacks".to_string(),
        "datapacks" | "datapack" => "datapacks".to_string(),
        _ => "mods".to_string(),
    }
}

pub fn entry_key(provider: &str, project_id: &str, content_type: &str) -> String {
    format!(
        "{}:{}:{}",
        provider.trim().to_lowercase(),
        normalize_content_type(content_type),
        project_id.trim().to_lowercase()
    )
}

pub fn entry_key_for(entry: &ModEntry) -> String {
    entry_key(&entry.provider, &entry.project_id, &entry.content_type)
}

pub fn reduce_layers(spec: &ModpackSpec) -> (Vec<ModEntry>, Vec<ResolutionConflict>, Vec<String>) {
    let mut computed: HashMap<String, ModEntry> = HashMap::new();
    let mut first_layer_by_key: HashMap<String, String> = HashMap::new();
    let mut conflicts: Vec<ResolutionConflict> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for layer in &spec.layers {
        if layer.is_frozen {
            warnings.push(format!("Layer '{}' is frozen; edits are ignored until unfrozen.", layer.name));
        }

        for add in &layer.entries_delta.add {
            let mut next = add.clone();
            next.content_type = normalize_content_type(&next.content_type);
            let key = entry_key_for(&next);

            if let Some(existing_layer) = first_layer_by_key.get(&key) {
                conflicts.push(ResolutionConflict {
                    code: "LAYER_DUPLICATE".to_string(),
                    message: format!(
                        "Entry '{}' appears in multiple layers ('{}' and '{}').",
                        next.project_id, existing_layer, layer.name
                    ),
                    keys: vec![key.clone()],
                });
            }

            computed.insert(key.clone(), next);
            first_layer_by_key
                .entry(key)
                .or_insert_with(|| layer.name.clone());
        }

        for remove in &layer.entries_delta.remove {
            let key = entry_key(&remove.provider, &remove.project_id, &remove.content_type);
            computed.remove(&key);
        }

        for patch in &layer.entries_delta.override_entries {
            let mut next = patch.clone();
            next.content_type = normalize_content_type(&next.content_type);
            let key = entry_key_for(&next);

            if computed.contains_key(&key) {
                computed.insert(key, next);
            } else {
                // Explicit override with missing base becomes explicit add, and is flagged.
                conflicts.push(ResolutionConflict {
                    code: "OVERRIDE_WITHOUT_BASE".to_string(),
                    message: format!(
                        "Override for '{}' had no base entry; treated as add in layer '{}'.",
                        next.project_id, layer.name
                    ),
                    keys: vec![entry_key_for(&next)],
                });
                computed.insert(entry_key_for(&next), next);
            }
        }
    }

    let mut out = computed.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| {
        let ka = entry_key_for(a);
        let kb = entry_key_for(b);
        ka.cmp(&kb)
    });

    (out, conflicts, warnings)
}

pub fn diff_entries(current: &[ModEntry], next: &[ModEntry]) -> (Vec<ModEntry>, Vec<EntryKey>, Vec<ModEntry>) {
    let current_map = current
        .iter()
        .map(|entry| (entry_key_for(entry), entry.clone()))
        .collect::<HashMap<_, _>>();
    let next_map = next
        .iter()
        .map(|entry| (entry_key_for(entry), entry.clone()))
        .collect::<HashMap<_, _>>();

    let current_keys = current_map.keys().cloned().collect::<HashSet<_>>();
    let next_keys = next_map.keys().cloned().collect::<HashSet<_>>();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut overridden = Vec::new();

    for key in next_keys.difference(&current_keys) {
        if let Some(item) = next_map.get(key) {
            added.push(item.clone());
        }
    }

    for key in current_keys.difference(&next_keys) {
        if let Some(item) = current_map.get(key) {
            removed.push(EntryKey {
                provider: item.provider.clone(),
                project_id: item.project_id.clone(),
                content_type: item.content_type.clone(),
            });
        }
    }

    for key in next_keys.intersection(&current_keys) {
        let Some(old) = current_map.get(key) else {
            continue;
        };
        let Some(new) = next_map.get(key) else {
            continue;
        };
        if materially_different(old, new) {
            overridden.push(new.clone());
        }
    }

    added.sort_by(|a, b| entry_key_for(a).cmp(&entry_key_for(b)));
    removed.sort_by(|a, b| entry_key(&a.provider, &a.project_id, &a.content_type).cmp(&entry_key(&b.provider, &b.project_id, &b.content_type)));
    overridden.sort_by(|a, b| entry_key_for(a).cmp(&entry_key_for(b)));

    (added, removed, overridden)
}

fn materially_different(a: &ModEntry, b: &ModEntry) -> bool {
    a.required != b.required
        || a.pin != b.pin
        || a.channel_policy != b.channel_policy
        || a.fallback_policy != b.fallback_policy
        || a.replacement_group != b.replacement_group
        || a.notes != b.notes
        || a.disabled_by_default != b.disabled_by_default
        || a.optional != b.optional
        || a.target_scope != b.target_scope
        || a.target_worlds != b.target_worlds
}

pub fn ensure_default_profiles(spec: &mut ModpackSpec) {
    if !spec.profiles.is_empty() {
        return;
    }
    spec.profiles = vec![
        crate::modpack::types::Profile {
            id: "lite".to_string(),
            name: "Lite".to_string(),
            optional_entry_states: HashMap::new(),
        },
        crate::modpack::types::Profile {
            id: "recommended".to_string(),
            name: "Recommended".to_string(),
            optional_entry_states: HashMap::new(),
        },
        crate::modpack::types::Profile {
            id: "full".to_string(),
            name: "Full".to_string(),
            optional_entry_states: HashMap::new(),
        },
    ];
}

pub fn make_base_spec(id: String, name: String, created_at: String) -> ModpackSpec {
    let mut spec = ModpackSpec {
        id,
        name,
        description: None,
        tags: vec![],
        created_at: created_at.clone(),
        updated_at: created_at,
        layers: vec![
            Layer {
                id: "layer_template".to_string(),
                name: "Template".to_string(),
                source: None,
                is_frozen: false,
                entries_delta: Default::default(),
            },
            Layer {
                id: "layer_user".to_string(),
                name: "User Additions".to_string(),
                source: None,
                is_frozen: false,
                entries_delta: Default::default(),
            },
            Layer {
                id: "layer_overrides".to_string(),
                name: "Overrides".to_string(),
                source: None,
                is_frozen: false,
                entries_delta: Default::default(),
            },
        ],
        profiles: vec![],
        settings: Default::default(),
    };
    ensure_default_profiles(&mut spec);
    spec
}
