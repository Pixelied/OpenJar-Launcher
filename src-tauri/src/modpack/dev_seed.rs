use crate::modpack::layers::{make_base_spec, normalize_entry_for_add};
use crate::modpack::types::{ModEntry, ModpackSpec, SeedDevResult};

pub fn seed_dev_data(
    app: &tauri::AppHandle,
    instance_name: Option<&str>,
) -> Result<(SeedDevResult, ModpackSpec), String> {
    if !crate::is_dev_mode_enabled() {
        return Err("Dev seed is only available when MPM_DEV_MODE=1".to_string());
    }

    let instances_dir = crate::app_instances_dir(app)?;
    let mut idx = crate::read_index(&instances_dir)?;

    let existing = idx
        .instances
        .iter()
        .find(|i| i.name.eq_ignore_ascii_case(instance_name.unwrap_or("Dev Seed Instance")))
        .cloned();

    let instance = if let Some(found) = existing {
        found
    } else {
        let name = instance_name
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "Dev Seed Instance".to_string());

        let inst = crate::Instance {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            mc_version: "1.21.1".to_string(),
            loader: "fabric".to_string(),
            created_at: crate::now_iso(),
            icon_path: None,
            settings: Default::default(),
        };

        let inst_dir = instances_dir.join(&inst.id);
        std::fs::create_dir_all(&inst_dir)
            .map_err(|e| format!("mkdir seed instance dir failed: {e}"))?;
        crate::write_instance_meta(&inst_dir, &inst)?;
        crate::write_lockfile(&instances_dir, &inst.id, &crate::Lockfile::default())?;
        idx.instances.push(inst.clone());
        crate::write_index(&instances_dir, &idx)?;
        inst
    };

    let mut spec = make_base_spec(
        format!("modpack_seed_{}", crate::now_millis()),
        "Dev Seed Modpack".to_string(),
        crate::now_iso(),
    );
    spec.description = Some("Seed pack for resolver/apply/drift testing.".to_string());
    spec.tags = vec!["dev-seed".to_string()];

    if let Some(layer) = spec.layers.iter_mut().find(|l| l.id == "layer_user") {
        layer.entries_delta.add = vec![
            normalize_entry_for_add(ModEntry {
                provider: "modrinth".to_string(),
                project_id: "AANobbMI".to_string(), // Sodium project ID
                slug: Some("Sodium".to_string()),
                content_type: "mods".to_string(),
                required: true,
                pin: None,
                channel_policy: "stable".to_string(),
                fallback_policy: "inherit".to_string(),
                replacement_group: None,
                notes: Some("Renderer optimization".to_string()),
                disabled_by_default: false,
                optional: false,
                target_scope: "instance".to_string(),
                target_worlds: vec![],
            }),
            normalize_entry_for_add(ModEntry {
                provider: "modrinth".to_string(),
                project_id: "P7dR8mSH".to_string(), // Fabric API
                slug: Some("Fabric API".to_string()),
                content_type: "mods".to_string(),
                required: true,
                pin: None,
                channel_policy: "stable".to_string(),
                fallback_policy: "inherit".to_string(),
                replacement_group: None,
                notes: Some("Common dependency".to_string()),
                disabled_by_default: false,
                optional: false,
                target_scope: "instance".to_string(),
                target_worlds: vec![],
            }),
        ];
    }

    let result = SeedDevResult {
        created_spec_id: spec.id.clone(),
        created_instance_id: instance.id,
        message: "Dev seed modpack + instance prepared.".to_string(),
    };
    Ok((result, spec))
}
