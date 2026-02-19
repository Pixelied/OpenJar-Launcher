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
            folder_name: None,
            mc_version: "1.21.1".to_string(),
            loader: "fabric".to_string(),
            created_at: crate::now_iso(),
            icon_path: None,
            settings: Default::default(),
        };
        let mut inst_with_folder = inst.clone();
        let folder_name =
            crate::allocate_instance_folder_name(&instances_dir, &idx, &inst_with_folder.name, None, None);
        inst_with_folder.folder_name = Some(folder_name.clone());

        let inst_dir = instances_dir.join(folder_name);
        std::fs::create_dir_all(&inst_dir)
            .map_err(|e| format!("mkdir seed instance dir failed: {e}"))?;
        crate::write_instance_meta(&inst_dir, &inst_with_folder)?;
        idx.instances.push(inst_with_folder.clone());
        crate::write_index(&instances_dir, &idx)?;
        crate::write_lockfile(&instances_dir, &inst_with_folder.id, &crate::Lockfile::default())?;
        inst_with_folder
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
                local_file_name: None,
                local_file_path: None,
                local_sha512: None,
                local_fingerprints: vec![],
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
                local_file_name: None,
                local_file_path: None,
                local_sha512: None,
                local_fingerprints: vec![],
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
