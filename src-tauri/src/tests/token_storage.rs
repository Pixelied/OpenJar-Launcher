use crate::*;
fn make_temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("openjar-token-tests-{name}-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

#[test]
fn persist_refresh_token_does_not_create_plaintext_fallback_file() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);
    let dir = make_temp_dir("persist-no-fallback");
    let fallback_path = dir.join(LAUNCHER_TOKEN_FALLBACK_FILE);
    assert!(!fallback_path.exists());

    persist_refresh_token_for_account("acct_test_a", "refresh_token_a")
        .expect("persist refresh token");

    assert!(!fallback_path.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn read_refresh_token_retrieves_from_keyring_store() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    persist_refresh_token_for_account("acct_test_b", "refresh_token_b")
        .expect("persist refresh token");
    let account = LauncherAccount {
        id: "acct_test_b".to_string(),
        username: "user_b".to_string(),
        added_at: "now".to_string(),
    };
    let token = read_refresh_token_from_keyring(&account, std::slice::from_ref(&account))
        .expect("read refresh token");
    assert_eq!(token, "refresh_token_b");
}

#[test]
fn legacy_fallback_migration_moves_to_keyring_and_deletes_file() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);
    let dir = make_temp_dir("legacy-migration");
    let fallback_path = dir.join(LAUNCHER_TOKEN_FALLBACK_FILE);

    let legacy_payload = serde_json::json!({
        "refresh_tokens": {
            "acct_test_c": "refresh_token_c"
        }
    });
    fs::write(
        &fallback_path,
        serde_json::to_string_pretty(&legacy_payload).expect("serialize legacy payload"),
    )
    .expect("write legacy fallback");
    assert!(fallback_path.exists());

    let summary =
        migrate_legacy_refresh_tokens_from_path(&fallback_path).expect("migrate fallback tokens");
    assert_eq!(summary.migrated, 1);
    assert_eq!(summary.fallback_files_removed, 1);
    assert!(!fallback_path.exists());

    let account = LauncherAccount {
        id: "acct_test_c".to_string(),
        username: "user_c".to_string(),
        added_at: "now".to_string(),
    };
    let token = read_refresh_token_from_keyring(&account, std::slice::from_ref(&account))
        .expect("read migrated refresh token");
    assert_eq!(token, "refresh_token_c");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn keyring_unavailable_returns_actionable_error() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(false);
    let err = persist_refresh_token_for_account("acct_test_d", "refresh_token_d")
        .expect_err("persist should fail when secure storage is unavailable");
    assert!(err.contains("keyring write failed"));
    assert!(err.contains("keyring"));
    set_test_token_keyring_available(true);
}

#[test]
fn persist_launcher_refresh_token_succeeds_when_post_write_verification_cannot_read() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);
    set_test_token_keyring_read_failure(KEYRING_SERVICE, true);
    for service in LEGACY_KEYRING_SERVICES {
        set_test_token_keyring_read_failure(service, true);
    }

    let account = LauncherAccount {
        id: "acct_verify_read_fail".to_string(),
        username: "player_verify_read_fail".to_string(),
        added_at: "now".to_string(),
    };
    persist_refresh_token_for_launcher_account(&account, "refresh_token_verify_read_fail")
        .expect("persist should not fail when verification read is unavailable");

    set_test_token_keyring_read_failure(KEYRING_SERVICE, false);
    for service in LEGACY_KEYRING_SERVICES {
        set_test_token_keyring_read_failure(service, false);
    }
    let canonical_alias = keyring_username_for_account(&account.id);
    let canonical = token_keyring_get_secret(KEYRING_SERVICE, &canonical_alias)
        .expect("read canonical persisted token after clearing simulated read failures");
    assert_eq!(canonical.as_deref(), Some("refresh_token_verify_read_fail"));
}

#[test]
fn read_refresh_token_recovers_single_known_token_for_selected_account() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    persist_refresh_token_for_account("acct_real", "refresh_token_real")
        .expect("persist known refresh token");
    let selected = LauncherAccount {
        id: "acct_selected_missing".to_string(),
        username: "player".to_string(),
        added_at: "now".to_string(),
    };
    let known = LauncherAccount {
        id: "acct_real".to_string(),
        username: "player".to_string(),
        added_at: "now".to_string(),
    };
    let accounts = vec![selected.clone(), known];

    let token = read_refresh_token_from_keyring(&selected, &accounts)
        .expect("recover refresh token for selected account");
    assert_eq!(token, "refresh_token_real");

    let canonical_username = keyring_username_for_account(&selected.id);
    let canonical = token_keyring_get_secret(KEYRING_SERVICE, &canonical_username)
        .expect("read canonical refreshed token");
    assert_eq!(canonical.as_deref(), Some("refresh_token_real"));
}

#[test]
fn read_refresh_token_recovery_fails_when_multiple_distinct_tokens_exist() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    persist_refresh_token_for_account("acct_a", "refresh_token_a")
        .expect("persist first refresh token");
    persist_refresh_token_for_account("acct_b", "refresh_token_b")
        .expect("persist second refresh token");

    let selected = LauncherAccount {
        id: "acct_selected_missing_2".to_string(),
        username: "player".to_string(),
        added_at: "now".to_string(),
    };
    let accounts = vec![
        selected.clone(),
        LauncherAccount {
            id: "acct_a".to_string(),
            username: "player-a".to_string(),
            added_at: "now".to_string(),
        },
        LauncherAccount {
            id: "acct_b".to_string(),
            username: "player-b".to_string(),
            added_at: "now".to_string(),
        },
    ];

    let err = read_refresh_token_from_keyring(&selected, &accounts)
        .expect_err("recovery should fail for ambiguous secure tokens");
    assert!(err.contains("Multiple secure refresh tokens were found"));
}

#[test]
fn read_refresh_token_matches_uuid_hyphen_and_simple_aliases() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    let hyphenated = "123e4567-e89b-12d3-a456-426614174000";
    let simple = "123e4567e89b12d3a456426614174000";
    persist_refresh_token_for_account(hyphenated, "refresh_token_uuid")
        .expect("persist uuid refresh token");

    let account = LauncherAccount {
        id: simple.to_string(),
        username: "uuid-user".to_string(),
        added_at: "now".to_string(),
    };
    let token = read_refresh_token_from_keyring(&account, std::slice::from_ref(&account))
        .expect("read uuid alias refresh token");
    assert_eq!(token, "refresh_token_uuid");
}

#[test]
fn read_refresh_token_recovers_from_selected_alias() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    token_keyring_set_secret(
        KEYRING_SERVICE,
        KEYRING_SELECTED_REFRESH_ALIAS,
        "refresh_token_selected_alias",
    )
    .expect("seed selected refresh alias");

    let account = LauncherAccount {
        id: "acct_selected_alias".to_string(),
        username: "player_selected".to_string(),
        added_at: "now".to_string(),
    };
    let token = read_refresh_token_from_keyring(&account, std::slice::from_ref(&account))
        .expect("recover token from selected alias");
    assert_eq!(token, "refresh_token_selected_alias");

    let canonical_username = keyring_username_for_account(&account.id);
    let canonical = token_keyring_get_secret(KEYRING_SERVICE, &canonical_username)
        .expect("read canonical token");
    assert_eq!(canonical.as_deref(), Some("refresh_token_selected_alias"));
}

#[test]
fn read_refresh_token_recovers_from_selected_alias_in_legacy_service() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    token_keyring_set_secret(
        LEGACY_KEYRING_SERVICES[0],
        KEYRING_SELECTED_REFRESH_ALIAS,
        "refresh_token_selected_legacy",
    )
    .expect("seed selected refresh alias in legacy service");

    let account = LauncherAccount {
        id: "acct_selected_alias_legacy".to_string(),
        username: "player_selected_legacy".to_string(),
        added_at: "now".to_string(),
    };
    let token = read_refresh_token_from_keyring(&account, std::slice::from_ref(&account))
        .expect("recover token from selected alias in legacy service");
    assert_eq!(token, "refresh_token_selected_legacy");

    let canonical_selected =
        token_keyring_get_secret(KEYRING_SERVICE, KEYRING_SELECTED_REFRESH_ALIAS)
            .expect("read canonical selected alias");
    assert_eq!(
        canonical_selected.as_deref(),
        Some("refresh_token_selected_legacy")
    );
}

#[test]
fn read_refresh_token_recovers_from_selected_alias_even_if_legacy_read_fails() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    token_keyring_set_secret(
        KEYRING_SERVICE,
        KEYRING_SELECTED_REFRESH_ALIAS,
        "refresh_token_selected_canonical",
    )
    .expect("seed selected refresh alias in canonical service");
    set_test_token_keyring_read_failure(LEGACY_KEYRING_SERVICES[0], true);

    let account = LauncherAccount {
        id: "acct_selected_alias_canonical".to_string(),
        username: "player_selected_canonical".to_string(),
        added_at: "now".to_string(),
    };
    let token = read_refresh_token_from_keyring(&account, std::slice::from_ref(&account))
        .expect("recover token from selected alias despite legacy read failure");
    assert_eq!(token, "refresh_token_selected_canonical");
}

#[test]
fn read_refresh_token_recovers_known_account_despite_legacy_read_failure() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    let selected = LauncherAccount {
        id: "acct_selected_legacy_read_fail".to_string(),
        username: "player_selected_legacy_read_fail".to_string(),
        added_at: "now".to_string(),
    };
    let known = LauncherAccount {
        id: "acct_known_legacy_read_fail".to_string(),
        username: "player_known_legacy_read_fail".to_string(),
        added_at: "now".to_string(),
    };

    let known_alias = keyring_username_for_account(&known.id);
    token_keyring_set_secret(
        LEGACY_KEYRING_SERVICES[1],
        &known_alias,
        "refresh_token_legacy_recover",
    )
    .expect("seed known token in secondary legacy service");
    set_test_token_keyring_read_failure(LEGACY_KEYRING_SERVICES[0], true);

    let accounts = vec![selected.clone(), known];
    let token = read_refresh_token_from_keyring(&selected, &accounts)
        .expect("recover known token despite legacy read failure");
    assert_eq!(token, "refresh_token_legacy_recover");
}

#[test]
fn read_refresh_token_survives_simulated_restart_for_launcher_account() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    let account = LauncherAccount {
        id: "acct_restart_ok".to_string(),
        username: "player_restart".to_string(),
        added_at: "now".to_string(),
    };
    persist_refresh_token_for_launcher_account(&account, "refresh_token_restart")
        .expect("persist launcher account refresh token");

    // Simulate full app restart (runtime memory cache is gone).
    runtime_refresh_token_cache_clear();

    let token = read_refresh_token_from_keyring(&account, std::slice::from_ref(&account))
        .expect("read refresh token after restart");
    assert_eq!(token, "refresh_token_restart");
}

#[test]
fn read_refresh_token_recovers_from_legacy_service_alias_after_restart() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    let account = LauncherAccount {
        id: "acct_legacy_restart".to_string(),
        username: "player_legacy_restart".to_string(),
        added_at: "now".to_string(),
    };
    let legacy_alias = keyring_username_for_account(&account.id);
    token_keyring_set_secret(
        LEGACY_KEYRING_SERVICES[1],
        &legacy_alias,
        "refresh_token_from_legacy_service",
    )
    .expect("seed legacy service refresh token");

    // Simulate full app restart (runtime memory cache is gone).
    runtime_refresh_token_cache_clear();

    let token = read_refresh_token_from_keyring(&account, std::slice::from_ref(&account))
        .expect("read refresh token migrated from legacy service");
    assert_eq!(token, "refresh_token_from_legacy_service");

    let canonical = token_keyring_get_secret(KEYRING_SERVICE, &legacy_alias)
        .expect("read canonical migrated token");
    assert_eq!(
        canonical.as_deref(),
        Some("refresh_token_from_legacy_service")
    );
}

#[test]
fn dev_curseforge_key_migrates_from_legacy_service_to_canonical_service() {
    let _guard = test_secure_storage_guard();
    clear_test_token_keyring_store();
    set_test_token_keyring_available(true);

    token_keyring_set_secret(
        LEGACY_KEYRING_SERVICES[0],
        DEV_CURSEFORGE_KEY_KEYRING_USER,
        "legacy_dev_cf_key",
    )
    .expect("seed legacy dev curseforge key");

    let key = keyring_get_dev_curseforge_key().expect("read dev curseforge key");
    assert_eq!(key.as_deref(), Some("legacy_dev_cf_key"));

    let canonical = token_keyring_get_secret(KEYRING_SERVICE, DEV_CURSEFORGE_KEY_KEYRING_USER)
        .expect("read canonical migrated dev curseforge key");
    assert_eq!(canonical.as_deref(), Some("legacy_dev_cf_key"));
}
