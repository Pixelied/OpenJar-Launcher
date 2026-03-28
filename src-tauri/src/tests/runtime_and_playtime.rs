use crate::*;
fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("openjar-runtime-tests-{label}-{}", Uuid::new_v4()))
}

#[test]
fn runtime_reconcile_copies_missing_entries_and_keeps_non_allowlisted_conflicts() {
    let instance_dir = temp_path("runtime-reconcile");
    fs::create_dir_all(instance_dir.join("runtime")).expect("create runtime");
    fs::create_dir_all(instance_dir.join("mods")).expect("create canonical mods");
    fs::create_dir_all(instance_dir.join("runtime").join(".meteor-client"))
        .expect("create runtime meteor");
    fs::write(
        instance_dir
            .join("runtime")
            .join(".meteor-client")
            .join("config.json"),
        br#"{"ok":true}"#,
    )
    .expect("write runtime meteor config");
    fs::write(instance_dir.join("mods").join("keep.jar"), b"canonical")
        .expect("write canonical mod");
    fs::write(instance_dir.join("runtime").join("mods"), b"bad")
        .expect("write conflicting runtime file");
    fs::write(
        instance_dir.join("runtime").join("options.txt"),
        b"runtime options",
    )
    .expect("write runtime options");
    fs::write(instance_dir.join("options.txt"), b"canonical options")
        .expect("write canonical options");

    reconcile_legacy_runtime_into_instance(&instance_dir).expect("reconcile runtime");

    assert!(instance_dir
        .join(".meteor-client")
        .join("config.json")
        .exists());
    assert_eq!(
        fs::read_to_string(instance_dir.join("options.txt")).expect("read options"),
        "canonical options"
    );
    assert!(instance_dir.join("mods").join("keep.jar").exists());
    assert!(runtime_reconcile_marker_path(&instance_dir).exists());

    let _ = fs::remove_dir_all(&instance_dir);
}

#[test]
fn isolated_clone_excludes_transient_roots_and_keeps_game_content() {
    let instance_dir = temp_path("isolated-clone");
    let isolated_dir = instance_dir.join("runtime_sessions").join("launch");
    fs::create_dir_all(instance_dir.join("mods")).expect("create mods");
    fs::create_dir_all(instance_dir.join("config")).expect("create config");
    fs::create_dir_all(instance_dir.join("runtime_sessions").join("old"))
        .expect("create old session");
    fs::create_dir_all(instance_dir.join("snapshots").join("s1")).expect("create snapshot");
    fs::create_dir_all(instance_dir.join("logs").join("launches")).expect("create launch logs");
    fs::write(instance_dir.join("mods").join("a.jar"), b"jar").expect("write mod jar");
    fs::write(instance_dir.join("play_sessions.v1.json"), b"{}").expect("write play sessions");
    fs::write(
        instance_dir.join("logs").join("launches").join("x.log"),
        b"log",
    )
    .expect("write launch log");

    clone_instance_to_isolated_runtime(&instance_dir, &isolated_dir)
        .expect("clone isolated runtime");

    assert!(isolated_dir.join("mods").join("a.jar").exists());
    assert!(isolated_dir.join("config").exists());
    assert!(!isolated_dir.join("runtime_sessions").exists());
    assert!(!isolated_dir.join("snapshots").exists());
    assert!(!isolated_dir.join("play_sessions.v1.json").exists());
    assert!(!isolated_dir.join("logs").join("launches").exists());

    let _ = fs::remove_dir_all(&instance_dir);
}

#[cfg(unix)]
#[test]
fn isolated_clone_rejects_symlinked_entries() {
    use std::os::unix::fs::symlink;

    let instance_dir = temp_path("isolated-clone-symlink");
    let isolated_dir = instance_dir.join("runtime_sessions").join("launch");
    let outside_dir = temp_path("isolated-clone-symlink-outside");
    fs::create_dir_all(&instance_dir).expect("create instance dir");
    fs::create_dir_all(&outside_dir).expect("create outside dir");
    fs::write(outside_dir.join("outside.txt"), b"outside").expect("write outside file");
    symlink(&outside_dir, instance_dir.join("linked-outside")).expect("create symlink");

    let err = clone_instance_to_isolated_runtime(&instance_dir, &isolated_dir)
        .expect_err("symlinked entry must be rejected");
    assert!(err.to_ascii_lowercase().contains("symlink"));
    assert!(!isolated_dir.join("linked-outside").exists());

    let _ = fs::remove_dir_all(&instance_dir);
    let _ = fs::remove_dir_all(&outside_dir);
}

#[cfg(unix)]
#[test]
fn launcher_import_rejects_symlinked_content() {
    use std::os::unix::fs::symlink;

    let source_dir = temp_path("launcher-import-symlink-source");
    let instance_dir = temp_path("launcher-import-symlink-instance");
    let outside_file = temp_path("launcher-import-symlink-outside");
    fs::create_dir_all(source_dir.join("mods")).expect("create source mods");
    fs::create_dir_all(&instance_dir).expect("create instance dir");
    fs::write(&outside_file, b"outside").expect("write outside file");
    symlink(&outside_file, source_dir.join("mods").join("linked.jar")).expect("create linked mod");

    let err = copy_launcher_source_into_instance(&source_dir, &instance_dir)
        .expect_err("symlinked import content must be rejected");
    assert!(err.to_ascii_lowercase().contains("symlink"));
    assert!(!instance_dir.join("mods").join("linked.jar").exists());

    let _ = fs::remove_dir_all(&source_dir);
    let _ = fs::remove_dir_all(&instance_dir);
    let _ = fs::remove_file(&outside_file);
}

#[test]
fn playtime_store_tracks_native_session_duration_and_summary() {
    let instances_dir = temp_path("playtime");
    fs::create_dir_all(&instances_dir).expect("create instances root");
    let instance = Instance {
        id: "inst_playtime".to_string(),
        name: "Playtime".to_string(),
        origin: "custom".to_string(),
        folder_name: Some("Playtime".to_string()),
        mc_version: "1.20.1".to_string(),
        loader: "fabric".to_string(),
        created_at: now_iso(),
        icon_path: None,
        settings: InstanceSettings::default(),
    };
    let index = InstanceIndex {
        instances: vec![instance.clone()],
    };
    write_index(&instances_dir, &index).expect("write index");
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    fs::create_dir_all(&instance_dir).expect("create instance dir");

    register_native_play_session_start(
        &instances_dir,
        &instance.id,
        "native_test",
        std::process::id(),
        false,
    )
    .expect("register active play session");
    let mut active = read_active_play_sessions_store(&instance_dir);
    assert_eq!(active.active.len(), 1);
    active.active[0].started_at = format!("unix:{}", Utc::now().timestamp().saturating_sub(5));
    write_active_play_sessions_store(&instance_dir, active).expect("write active store");

    let finalized = finalize_native_play_session(
        &instances_dir,
        &instance.id,
        "native_test",
        "success",
        false,
    )
    .expect("finalize play session");
    assert!(finalized.is_some());

    let summary =
        instance_playtime_summary(&instances_dir, &instance.id).expect("read playtime summary");
    assert!(summary.total_seconds >= 5);
    assert_eq!(summary.sessions_count, 1);
    assert_eq!(summary.tracking_scope, "native_only");

    let _ = fs::remove_dir_all(&instances_dir);
}

#[test]
fn normalize_app_language_accepts_supported_aliases() {
    assert_eq!(normalize_app_language(""), "en-US");
    assert_eq!(normalize_app_language("en"), "en-US");
    assert_eq!(normalize_app_language("es-419"), "es-ES");
    assert_eq!(normalize_app_language("fr"), "fr-FR");
    assert_eq!(normalize_app_language("deutsch"), "de-DE");
    assert_eq!(normalize_app_language("pt"), "pt-BR");
    assert_eq!(normalize_app_language("unknown"), "en-US");
}
