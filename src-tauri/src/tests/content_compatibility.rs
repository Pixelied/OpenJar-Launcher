use crate::*;
fn make_instance(loader: &str, mc_version: &str) -> Instance {
    Instance {
        id: "inst_test".to_string(),
        name: "Test".to_string(),
        origin: "custom".to_string(),
        folder_name: None,
        mc_version: mc_version.to_string(),
        loader: loader.to_string(),
        created_at: "now".to_string(),
        icon_path: None,
        settings: InstanceSettings::default(),
    }
}

fn make_cf_file(game_versions: Vec<&str>) -> CurseforgeFile {
    CurseforgeFile {
        id: 1,
        mod_id: 1,
        display_name: "file".to_string(),
        file_name: "file.zip".to_string(),
        file_date: "2026-01-01T00:00:00Z".to_string(),
        download_url: None,
        game_versions: game_versions.into_iter().map(str::to_string).collect(),
        hashes: vec![],
        dependencies: vec![],
    }
}

fn make_modrinth_version(game_versions: Vec<&str>, loaders: Vec<&str>) -> ModrinthVersion {
    ModrinthVersion {
        project_id: "project".to_string(),
        id: "ver".to_string(),
        version_number: "1.0.0".to_string(),
        name: None,
        game_versions: game_versions.into_iter().map(str::to_string).collect(),
        loaders: loaders.into_iter().map(str::to_string).collect(),
        date_published: "2026-01-01T00:00:00Z".to_string(),
        dependencies: vec![],
        files: vec![ModrinthVersionFile {
            url: "https://example.com/file.zip".to_string(),
            filename: "file.zip".to_string(),
            primary: Some(true),
            hashes: HashMap::new(),
        }],
    }
}

fn make_cf_project(
    class_id: i64,
    slug: &str,
    categories: Vec<CurseforgeCategory>,
) -> CurseforgeMod {
    CurseforgeMod {
        id: 12345,
        class_id,
        name: "Project".to_string(),
        slug: Some(slug.to_string()),
        summary: String::new(),
        download_count: 0.0,
        date_modified: String::new(),
        authors: vec![],
        categories,
        logo: None,
    }
}

#[test]
fn non_mod_curseforge_compatibility_ignores_loader_tags() {
    let instance = make_instance("fabric", "1.20.1");
    let file = make_cf_file(vec!["1.20.1", "forge"]);
    assert!(file_looks_compatible_with_instance(
        &file,
        &instance,
        "resourcepacks"
    ));
    assert!(!file_looks_compatible_with_instance(
        &file, &instance, "mods"
    ));
}

#[test]
fn non_mod_curseforge_compatibility_allows_patch_level_fallback() {
    let instance = make_instance("fabric", "1.21.11");
    let file = make_cf_file(vec!["1.21.1", "forge"]);
    assert!(file_looks_compatible_with_instance(
        &file,
        &instance,
        "resourcepacks"
    ));
}

#[test]
fn mod_curseforge_compatibility_keeps_patch_strict() {
    let instance = make_instance("fabric", "1.21.11");
    let file = make_cf_file(vec!["1.21.1", "fabric"]);
    assert!(!file_looks_compatible_with_instance(
        &file, &instance, "mods"
    ));
}

#[test]
fn non_mod_modrinth_selection_ignores_loader_mismatch() {
    let instance = make_instance("fabric", "1.20.1");
    let versions = vec![make_modrinth_version(vec!["1.20.1"], vec!["forge"])];
    assert!(pick_compatible_version_for_content(versions, &instance, "shaderpacks").is_some());
}

#[test]
fn non_mod_modrinth_selection_allows_patch_level_fallback() {
    let instance = make_instance("fabric", "1.21.11");
    let versions = vec![make_modrinth_version(vec!["1.21.1"], vec!["forge"])];
    assert!(pick_compatible_version_for_content(versions, &instance, "shaderpacks").is_some());
}

#[test]
fn mod_modrinth_selection_still_requires_loader_match() {
    let instance = make_instance("fabric", "1.20.1");
    let versions = vec![make_modrinth_version(vec!["1.20.1"], vec!["forge"])];
    assert!(pick_compatible_version_for_content(versions, &instance, "mods").is_none());
}

#[test]
fn normalize_update_content_type_filter_accepts_supported_aliases() {
    let requested = vec![
        "shaders".to_string(),
        "resourcepacks".to_string(),
        "mods".to_string(),
    ];
    let filter = normalize_update_content_type_filter(Some(&requested))
        .expect("filter should include supported content types");
    assert!(filter.contains("shaderpacks"));
    assert!(filter.contains("resourcepacks"));
    assert!(filter.contains("mods"));
}

#[test]
fn normalize_update_content_type_filter_ignores_unsupported_values() {
    let requested = vec!["modpacks".to_string(), "unknown".to_string()];
    assert!(normalize_update_content_type_filter(Some(&requested)).is_none());
}

#[test]
fn curseforge_resourcepack_slug_uses_texture_packs_url_path() {
    let url = curseforge_external_project_url("12345", Some("fresh-animations"), "resourcepacks");
    assert_eq!(
        url,
        "https://www.curseforge.com/minecraft/texture-packs/fresh-animations"
    );
}

#[test]
fn infer_curseforge_class_12_without_shader_category_defaults_to_resourcepacks() {
    let project = make_cf_project(
        12,
        "fresh-animations",
        vec![CurseforgeCategory {
            name: "Resource Packs".to_string(),
            slug: Some("resource-packs".to_string()),
        }],
    );
    let inferred = infer_curseforge_project_content_type(&project, None);
    assert_eq!(inferred, "resourcepacks");
    let url = curseforge_external_project_url("12345", project.slug.as_deref(), &inferred);
    assert_eq!(
        url,
        "https://www.curseforge.com/minecraft/texture-packs/fresh-animations"
    );
}

#[test]
fn infer_curseforge_class_12_shader_category_uses_shaders_path() {
    let project = make_cf_project(
        12,
        "complementary-shaders",
        vec![CurseforgeCategory {
            name: "Shaders".to_string(),
            slug: Some("shaders".to_string()),
        }],
    );
    let inferred = infer_curseforge_project_content_type(&project, None);
    assert_eq!(inferred, "shaderpacks");
    let url = curseforge_external_project_url("12345", project.slug.as_deref(), &inferred);
    assert_eq!(
        url,
        "https://www.curseforge.com/minecraft/shaders/complementary-shaders"
    );
}

#[test]
fn local_loader_guard_blocks_fabric_and_forge_mismatch_both_directions() {
    assert!(!instance_loader_accepts_mod_loader("fabric", "forge"));
    assert!(!instance_loader_accepts_mod_loader("forge", "fabric"));
}

#[test]
fn local_loader_guard_allows_quilt_instance_to_accept_fabric_mods() {
    assert!(instance_loader_accepts_mod_loader("quilt", "fabric"));
    assert!(!instance_loader_accepts_mod_loader("fabric", "quilt"));
}

#[test]
fn local_loader_guard_keeps_neoforge_and_forge_distinct() {
    assert!(!instance_loader_accepts_mod_loader("neoforge", "forge"));
    assert!(!instance_loader_accepts_mod_loader("forge", "neoforge"));
    assert!(instance_loader_accepts_mod_loader("neoforge", "neoforge"));
}

#[test]
fn local_loader_guard_allows_forge_family_hint_for_forge_variants() {
    assert!(instance_loader_accepts_mod_loader("forge", "forge_family"));
    assert!(instance_loader_accepts_mod_loader(
        "neoforge",
        "forge_family"
    ));
    assert!(!instance_loader_accepts_mod_loader(
        "fabric",
        "forge_family"
    ));
}
