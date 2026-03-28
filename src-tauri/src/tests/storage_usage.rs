use crate::*;
fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("openjar-storage-tests-{label}-{}", Uuid::new_v4()))
}

#[test]
fn storage_folder_entries_include_nested_directories_by_recursive_size() {
    let root = temp_path("folders-recursive");
    fs::create_dir_all(root.join("mods").join("cache")).expect("create nested cache dir");
    fs::create_dir_all(root.join("logs")).expect("create logs dir");
    fs::write(root.join("mods").join("a.jar"), vec![0_u8; 80]).expect("write mod");
    fs::write(
        root.join("mods").join("cache").join("big.bin"),
        vec![0_u8; 240],
    )
    .expect("write nested big file");
    fs::write(root.join("logs").join("latest.log"), vec![0_u8; 32]).expect("write log");

    let rows =
        storage_collect_folder_entries("instance", &root, &root, "instance", Some("inst"), 10)
            .expect("collect folder entries");
    let rels = rows
        .iter()
        .map(|row| row.relative_path.as_str())
        .collect::<Vec<_>>();

    assert!(rels.contains(&"mods"));
    assert!(rels.contains(&"mods/cache"));
    assert!(rels.contains(&"logs"));
    assert_eq!(
        rows.first().map(|row| row.relative_path.as_str()),
        Some("mods")
    );
    assert_eq!(
        rows.iter()
            .find(|row| row.relative_path == "mods/cache")
            .map(|row| row.bytes),
        Some(240)
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn storage_app_breakdown_keeps_shared_cache_out_of_app_total() {
    let launcher_root = temp_path("app-breakdown");
    fs::create_dir_all(launcher_root.join("cache").join("assets")).expect("create cache assets");
    fs::create_dir_all(launcher_root.join("profiles")).expect("create profiles");
    fs::write(
        launcher_root.join("cache").join("assets").join("idx.bin"),
        vec![0_u8; 128],
    )
    .expect("write cache file");
    fs::write(launcher_root.join("settings.json"), vec![0_u8; 24]).expect("write settings");
    fs::write(
        launcher_root.join("profiles").join("profile.json"),
        vec![0_u8; 64],
    )
    .expect("write profile");

    let (app_bytes, shared_cache_bytes, breakdown) =
        storage_app_breakdown(&launcher_root).expect("scan app breakdown");

    assert_eq!(shared_cache_bytes, 128);
    assert_eq!(app_bytes, 88);
    assert_eq!(
        breakdown
            .iter()
            .find(|row| row.key == "launcher_metadata")
            .map(|row| row.bytes),
        Some(24)
    );
    assert_eq!(
        breakdown
            .iter()
            .find(|row| row.key == "other_launcher")
            .map(|row| row.bytes),
        Some(64)
    );

    let _ = fs::remove_dir_all(&launcher_root);
}

#[test]
fn storage_app_scope_entries_do_not_list_cache_paths() {
    let launcher_root = temp_path("app-scope");
    fs::create_dir_all(launcher_root.join("cache").join("versions"))
        .expect("create cache versions");
    fs::create_dir_all(launcher_root.join("profiles")).expect("create profiles");
    fs::write(
        launcher_root.join("cache").join("versions").join("v.json"),
        vec![0_u8; 32],
    )
    .expect("write cache version file");
    fs::write(
        launcher_root.join("profiles").join("profile.json"),
        vec![0_u8; 16],
    )
    .expect("write profile file");

    let folder_rows =
        storage_collect_folder_entries("app", &launcher_root, &launcher_root, "app", None, 10)
            .expect("collect app folder entries");
    let file_rows =
        storage_collect_file_entries("app", &launcher_root, &launcher_root, "app", None, 10)
            .expect("collect app file entries");

    assert!(folder_rows
        .iter()
        .all(|row| !row.relative_path.starts_with("cache")));
    assert!(file_rows
        .iter()
        .all(|row| !row.relative_path.starts_with("cache")));
    assert!(folder_rows
        .iter()
        .any(|row| row.relative_path == "profiles"));

    let _ = fs::remove_dir_all(&launcher_root);
}
