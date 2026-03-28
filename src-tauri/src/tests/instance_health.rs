use crate::*;
#[test]
fn instance_last_run_metadata_serializes_camel_case_shape() {
    let payload = serde_json::to_value(InstanceLastRunMetadata {
        last_launch_at: Some("2026-02-25T20:00:00Z".to_string()),
        last_exit_kind: Some("success".to_string()),
        last_exit_at: Some("2026-02-25T20:02:00Z".to_string()),
    })
    .expect("serialize last-run metadata");

    assert_eq!(
        payload.get("lastLaunchAt").and_then(|v| v.as_str()),
        Some("2026-02-25T20:00:00Z")
    );
    assert_eq!(
        payload.get("lastExitKind").and_then(|v| v.as_str()),
        Some("success")
    );
    assert_eq!(
        payload.get("lastExitAt").and_then(|v| v.as_str()),
        Some("2026-02-25T20:02:00Z")
    );
    assert!(payload.get("last_launch_at").is_none());
    assert!(payload.get("last_exit_kind").is_none());
}

#[test]
fn disk_usage_helper_counts_instance_files() {
    let tmp = std::env::temp_dir().join(format!("openjar-disk-usage-{}", Uuid::new_v4()));
    fs::create_dir_all(&tmp).expect("create temp instance dir");
    fs::write(tmp.join("a.bin"), vec![1_u8; 64]).expect("write first file");
    fs::create_dir_all(tmp.join("nested")).expect("create nested dir");
    fs::write(tmp.join("nested").join("b.bin"), vec![2_u8; 128]).expect("write nested file");

    let size = dir_total_size_bytes(&tmp);
    assert!(size >= 192, "size should include both regular files");

    let _ = fs::remove_dir_all(&tmp);
}
