use crate::*;

fn temp_file(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openjar-path-grants-{label}-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("picked.txt");
    fs::write(&path, b"picked").expect("write temp file");
    path
}

#[test]
fn path_grant_consumes_matching_purpose_once() {
    let state = AppState::default();
    let path = temp_file("consume");
    let granted = register_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT,
        path.clone(),
        false,
    )
    .expect("register grant");

    let resolved = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT,
        &granted.grant_id,
    )
    .expect("consume grant");
    assert_eq!(resolved, path);

    let err = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT,
        &granted.grant_id,
    )
    .expect_err("second consume must fail");
    assert!(err.contains("already used") || err.contains("missing"));

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir_all(path.parent().expect("parent"));
}

#[test]
fn path_grant_rejects_wrong_purpose() {
    let state = AppState::default();
    let path = temp_file("purpose");
    let granted = register_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT,
        path.clone(),
        false,
    )
    .expect("register grant");

    let err = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_MODPACK_SPEC_IMPORT,
        &granted.grant_id,
    )
    .expect_err("purpose mismatch must fail");
    assert!(err.contains("purpose mismatch"));

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir_all(path.parent().expect("parent"));
}

#[test]
fn path_grant_rejects_missing_file() {
    let state = AppState::default();
    let path = temp_file("missing");
    let granted = register_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT,
        path.clone(),
        false,
    )
    .expect("register grant");
    fs::remove_file(&path).expect("remove temp file");

    let err = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT,
        &granted.grant_id,
    )
    .expect_err("missing file must fail");
    assert!(err.contains("no longer available"));

    let _ = fs::remove_dir_all(path.parent().expect("parent"));
}

#[test]
fn path_grant_rejects_expired_grant() {
    let state = AppState::default();
    let path = temp_file("expired");
    let granted = register_external_path_grant_with_ttl(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT,
        path.clone(),
        false,
        Duration::from_millis(0),
    )
    .expect("register expiring grant");

    let err = consume_external_path_grant(
        &state,
        EXTERNAL_PATH_PURPOSE_PRESETS_IMPORT,
        &granted.grant_id,
    )
    .expect_err("expired grant must fail");
    assert!(err.contains("expired") || err.contains("missing"));

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir_all(path.parent().expect("parent"));
}
