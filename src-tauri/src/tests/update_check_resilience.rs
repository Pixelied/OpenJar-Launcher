use crate::*;
use reqwest::header::{HeaderMap, HeaderValue};

fn clear_and_capture_github_env() -> Vec<(String, String)> {
    let mut captured = Vec::new();
    let keys = std::env::vars()
        .map(|(key, _)| key)
        .filter(|key| {
            key == "MPM_GITHUB_TOKENS"
                || key == "MPM_GITHUB_TOKEN"
                || key == "GITHUB_TOKEN"
                || key == "GH_TOKEN"
                || key.starts_with("MPM_GITHUB_TOKEN_")
                || key.starts_with("GITHUB_TOKEN_")
                || key.starts_with("GH_TOKEN_")
        })
        .collect::<Vec<_>>();
    for key in keys {
        if let Ok(value) = std::env::var(&key) {
            captured.push((key.clone(), value));
        }
        std::env::remove_var(&key);
    }
    captured
}

fn restore_github_env(previous: Vec<(String, String)>) {
    for (key, value) in previous {
        std::env::set_var(key, value);
    }
}

fn reset_github_rotation_state() {
    if let Ok(mut guard) = github_token_rotation_state().lock() {
        guard.next_start_index = 0;
        guard.cooldown_until.clear();
        guard.unauth_cooldown_until = None;
        guard.unauth_reset_local = None;
    }
}

fn mark_unauth_rate_limit_for_tests() {
    let mut headers = HeaderMap::new();
    let reset_epoch = (Utc::now().timestamp() + 120).to_string();
    headers.insert(
        "x-ratelimit-reset",
        HeaderValue::from_str(&reset_epoch).expect("valid reset epoch"),
    );
    headers.insert("x-ratelimit-remaining", HeaderValue::from_static("0"));
    github_mark_unauth_cooldown(&headers);
}

fn make_instance(loader: &str, mc_version: &str) -> Instance {
    Instance {
        id: "inst_resilience".to_string(),
        name: "Resilience".to_string(),
        origin: "custom".to_string(),
        folder_name: None,
        mc_version: mc_version.to_string(),
        loader: loader.to_string(),
        created_at: "now".to_string(),
        icon_path: None,
        settings: InstanceSettings::default(),
    }
}

fn make_github_lock_entry(name: &str, project_id: &str, version_id: &str) -> LockEntry {
    LockEntry {
        source: "github".to_string(),
        project_id: project_id.to_string(),
        version_id: version_id.to_string(),
        name: name.to_string(),
        version_number: "1.0.0".to_string(),
        filename: format!("{name}.jar"),
        content_type: "mods".to_string(),
        target_scope: "instance".to_string(),
        target_worlds: vec![],
        pinned_version: None,
        enabled: true,
        hashes: HashMap::new(),
        provider_candidates: vec![],
        local_analysis: None,
    }
}

#[test]
fn update_check_keeps_running_and_compacts_github_rate_limit_warnings() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);
    let _ = keyring_delete_github_token_pool();
    github_invalidate_token_pool_cache();
    reset_github_rotation_state();
    let previous_env = clear_and_capture_github_env();

    mark_unauth_rate_limit_for_tests();

    let lock = Lockfile {
        version: 2,
        entries: vec![
            make_github_lock_entry("mod-one", "gh:example/repo-one", "gh_release:1"),
            make_github_lock_entry("mod-two", "gh:example/repo-two", "gh_release:2"),
        ],
    };
    let client = build_http_client().expect("http client");
    let instance = make_instance("fabric", "1.21.1");
    let result = check_instance_content_updates_inner(
        &client,
        &instance,
        &lock,
        UpdateScope::AllContent,
        None,
    )
    .expect("update check should not fail hard");

    assert_eq!(result.checked_entries, 2);
    assert_eq!(result.update_count, 0);
    assert!(result.warnings.iter().any(|warning| {
        warning.contains("GitHub checks paused due to rate limit; skipped 2 GitHub entries")
    }));

    restore_github_env(previous_env);
    let _ = keyring_delete_github_token_pool();
    github_invalidate_token_pool_cache();
    reset_github_rotation_state();
}

#[test]
fn github_get_json_short_circuits_when_unauth_cooldown_active() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);
    let _ = keyring_delete_github_token_pool();
    github_invalidate_token_pool_cache();
    reset_github_rotation_state();
    let previous_env = clear_and_capture_github_env();

    mark_unauth_rate_limit_for_tests();
    let client = build_http_client().expect("http client");
    let err = github_get_json::<serde_json::Value>(
        &client,
        "https://api.github.com/repos/octocat/Hello-World",
    )
    .expect_err("should fail fast while unauth cooldown is active");
    assert!(err.contains("Unauthenticated GitHub requests are temporarily paused"));

    restore_github_env(previous_env);
    let _ = keyring_delete_github_token_pool();
    github_invalidate_token_pool_cache();
    reset_github_rotation_state();
}

#[test]
fn github_token_pool_status_merges_env_and_keychain_tokens() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);
    let _ = keyring_delete_github_token_pool();
    github_invalidate_token_pool_cache();
    reset_github_rotation_state();
    let previous_env = clear_and_capture_github_env();

    std::env::set_var("MPM_GITHUB_TOKENS", "envA,dup");
    keyring_set_github_token_pool("keyA\ndup").expect("store keychain pool");
    github_invalidate_token_pool_cache();

    let status = github_token_pool_status();
    assert_eq!(status.total_tokens, 3);
    assert_eq!(status.env_tokens, 2);
    assert_eq!(status.keychain_tokens, 2);
    assert!(status.configured);

    restore_github_env(previous_env);
    let _ = keyring_delete_github_token_pool();
    github_invalidate_token_pool_cache();
    reset_github_rotation_state();
}
