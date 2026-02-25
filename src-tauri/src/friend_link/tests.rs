use super::*;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn sample_session() -> store::FriendLinkSessionRecord {
    let signing_key = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
    store::FriendLinkSessionRecord {
        instance_id: "inst_1".to_string(),
        group_id: "group_1".to_string(),
        local_peer_id: "peer_1".to_string(),
        display_name: "Host".to_string(),
        shared_secret_key_id: String::new(),
        shared_secret_b64: random_secret_b64(),
        protocol_version: PROTOCOL_VERSION,
        listener_port: 45001,
        listener_endpoint: Some("127.0.0.1:45001".to_string()),
        peers: vec![],
        allowlist: state::default_allowlist(),
        last_peer_sync_at: HashMap::new(),
        last_good_snapshot: None,
        pending_conflicts: vec![],
        cached_peer_state: HashMap::new(),
        bootstrap_host_peer_id: None,
        trusted_peer_ids: vec![],
        trusted_peer_ids_initialized: false,
        guardrails_updated_at_ms: 0,
        peer_aliases: HashMap::new(),
        allow_loopback_endpoints: true,
        allow_internet_endpoints: false,
        max_auto_changes: 25,
        sync_mods: true,
        sync_resourcepacks: false,
        sync_shaderpacks: true,
        sync_datapacks: true,
        allow_upnp_endpoints: false,
        public_endpoint_override: None,
        local_signing_key_id: String::new(),
        local_signing_private_b64: BASE64_STANDARD.encode(signing_key.to_bytes()),
        local_signing_public_key_b64: BASE64_STANDARD
            .encode(signing_key.verifying_key().to_bytes()),
        peer_signing_public_keys: HashMap::new(),
        invite_policies: HashMap::new(),
        invite_usage: HashMap::new(),
    }
}

#[test]
fn invite_roundtrip() {
    let mut session = sample_session();
    let invite = build_invite(&mut session).expect("build invite");
    let payload = parse_invite(&invite.invite_code, true, false).expect("parse invite");
    assert_eq!(payload.group_id, session.group_id);
    assert_eq!(payload.bootstrap_peer_endpoint, "127.0.0.1:45001");
    assert_eq!(payload.bootstrap_peer_endpoints, vec!["127.0.0.1:45001"]);
    assert_eq!(invite.bootstrap_peer_endpoints, vec!["127.0.0.1:45001"]);
    assert_eq!(payload.protocol_version, PROTOCOL_VERSION);
    assert_eq!(payload.invite_version, INVITE_VERSION_V2);
    assert!(payload.invite_id.is_some());
    assert_eq!(payload.max_uses, Some(3));
}

#[test]
fn invite_parse_blocks_loopback_when_internet_mode_without_opt_in() {
    let mut session = sample_session();
    let invite = build_invite(&mut session).expect("build invite");
    let err = parse_invite(&invite.invite_code, false, true)
        .expect_err("loopback should be blocked in internet mode");
    assert!(err.to_ascii_lowercase().contains("loopback"));
}

#[test]
fn invite_parse_ignores_whitespace() {
    let mut session = sample_session();
    let invite = build_invite(&mut session).expect("build invite");
    let mut spaced = String::new();
    for (idx, ch) in invite.invite_code.chars().enumerate() {
        spaced.push(ch);
        if idx > 0 && idx % 12 == 0 {
            spaced.push('\n');
            spaced.push(' ');
        }
    }
    let payload = parse_invite(&spaced, true, false).expect("parse invite with whitespace");
    assert_eq!(payload.group_id, session.group_id);
}

#[test]
fn invite_parse_accepts_legacy_payload_without_endpoints_array() {
    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let legacy_payload = serde_json::json!({
        "group_id": "group_legacy",
        "bootstrap_peer_endpoint": "127.0.0.1:45001",
        "shared_secret": random_secret_b64(),
        "expires_at": expires_at,
        "protocol_version": PROTOCOL_VERSION,
        "host_peer_id": "peer_host"
    });
    let raw = serde_json::to_vec(&legacy_payload).expect("serialize legacy invite");
    let invite_code = URL_SAFE_NO_PAD.encode(raw);
    let parsed = parse_invite(&invite_code, true, false).expect("parse legacy invite");
    assert_eq!(parsed.bootstrap_peer_endpoint, "127.0.0.1:45001");
    assert_eq!(parsed.bootstrap_peer_endpoints, vec!["127.0.0.1:45001"]);
    assert_eq!(parsed.invite_version, INVITE_VERSION_LEGACY);
    assert!(parsed.invite_id.is_none());
}

#[test]
fn internet_invite_defaults_to_short_lived_single_use() {
    let mut session = sample_session();
    session.allow_internet_endpoints = true;
    let invite = build_invite(&mut session).expect("build internet invite");
    let parsed = parse_invite(&invite.invite_code, true, true).expect("parse internet invite");
    assert_eq!(parsed.invite_version, INVITE_VERSION_V2);
    assert_eq!(parsed.max_uses, Some(1));
    let expires = chrono::DateTime::parse_from_rfc3339(&parsed.expires_at)
        .expect("parse expires")
        .with_timezone(&chrono::Utc);
    let minutes = (expires - chrono::Utc::now()).num_minutes();
    assert!(minutes <= 10 && minutes >= 0);
}

#[test]
fn allowlist_excludes_disallowed_prefixes() {
    let normalized = normalize_allowlist(&vec![
        "mods/**/*.jar".to_string(),
        "saves/**".to_string(),
        "config/**/*.json".to_string(),
    ]);
    assert!(normalized.iter().any(|v| v == "options.txt"));
    assert!(normalized.iter().any(|v| v == "config/**/*.json"));
    assert!(!normalized.iter().any(|v| v.starts_with("mods/")));
    assert!(!normalized.iter().any(|v| v.starts_with("saves/")));
}

#[test]
fn lock_conflict_builds_expected_fields() {
    let mine = state::CanonicalLockEntry {
        source: "modrinth".to_string(),
        project_id: "abc".to_string(),
        version_id: "v1".to_string(),
        name: "ABC".to_string(),
        version_number: "1.0.0".to_string(),
        filename: "abc.jar".to_string(),
        content_type: "mods".to_string(),
        target_scope: "instance".to_string(),
        target_worlds: vec![],
        enabled: true,
        hashes: HashMap::new(),
    };
    let theirs = state::CanonicalLockEntry {
        version_id: "v2".to_string(),
        version_number: "2.0.0".to_string(),
        ..mine.clone()
    };

    let conflict = conflict_from_lock("lock::modrinth::mods::abc", "peer_2", Some(&mine), &theirs);
    assert_eq!(conflict.kind, "lock_entry");
    assert_eq!(conflict.peer_id, "peer_2");
    assert_ne!(conflict.mine_hash, conflict.theirs_hash);
}

#[test]
fn lock_entry_hash_is_stable_across_hashmap_order() {
    let mut hashes_a = HashMap::new();
    hashes_a.insert("sha512".to_string(), "AA".to_string());
    hashes_a.insert("sha1".to_string(), "bb".to_string());

    let mut hashes_b = HashMap::new();
    hashes_b.insert("sha1".to_string(), "BB".to_string());
    hashes_b.insert("sha512".to_string(), "aa".to_string());

    let base = state::CanonicalLockEntry {
        source: "modrinth".to_string(),
        project_id: "abc".to_string(),
        version_id: "v1".to_string(),
        name: "ABC".to_string(),
        version_number: "1.0.0".to_string(),
        filename: "abc.jar".to_string(),
        content_type: "mods".to_string(),
        target_scope: "instance".to_string(),
        target_worlds: vec![],
        enabled: true,
        hashes: hashes_a,
    };
    let other = state::CanonicalLockEntry {
        hashes: hashes_b,
        ..base.clone()
    };

    assert_eq!(
        state::lock_entry_hash(&base),
        state::lock_entry_hash(&other)
    );
}

#[test]
fn drift_preview_honors_content_type_sync_toggles() {
    let mut session = sample_session();
    session.sync_mods = true;
    session.sync_resourcepacks = false;

    let local_state = state::SyncState {
        state_hash: "local".to_string(),
        lock_entries: vec![],
        config_files: vec![],
    };
    let remote_state = state::SyncState {
        state_hash: "remote".to_string(),
        lock_entries: vec![
            state::CanonicalLockEntry {
                source: "modrinth".to_string(),
                project_id: "mod-a".to_string(),
                version_id: "v1".to_string(),
                name: "Mod A".to_string(),
                version_number: "1.0.0".to_string(),
                filename: "mod-a.jar".to_string(),
                content_type: "mods".to_string(),
                target_scope: "instance".to_string(),
                target_worlds: vec![],
                enabled: true,
                hashes: HashMap::new(),
            },
            state::CanonicalLockEntry {
                source: "modrinth".to_string(),
                project_id: "tex-a".to_string(),
                version_id: "v1".to_string(),
                name: "Texture A".to_string(),
                version_number: "1.0.0".to_string(),
                filename: "tex-a.zip".to_string(),
                content_type: "texturepacks".to_string(),
                target_scope: "instance".to_string(),
                target_worlds: vec![],
                enabled: true,
                hashes: HashMap::new(),
            },
        ],
        config_files: vec![],
    };

    let preview = build_friend_link_drift_preview(
        std::path::Path::new("."),
        "inst_1",
        &session,
        &local_state,
        &[PeerStateSnapshot {
            peer_id: "peer_2".to_string(),
            display_name: "Peer".to_string(),
            state: remote_state,
        }],
        1,
    );

    assert_eq!(preview.total_changes, 1);
    assert_eq!(preview.added, 1);
    assert!(preview
        .items
        .iter()
        .all(|item| item.key.contains("::mods::")));
}

#[test]
fn drift_change_summary_flags_untrusted_risk() {
    let message = summarize_drift_changes(2, 1, 3, 6, true);
    assert!(message.contains("Will add 2, remove 1, and update 3 items."));
    assert!(message.to_lowercase().contains("untrusted"));
}

#[test]
fn trusted_peers_do_not_auto_fill_without_init_flag() {
    let mut session = sample_session();
    session.peers = vec![
        store::FriendPeerRecord {
            peer_id: "peer_a".to_string(),
            display_name: "A".to_string(),
            endpoint: "127.0.0.1:1".to_string(),
            added_at: "now".to_string(),
            last_seen_at: None,
            online: true,
            last_state_hash: None,
        },
        store::FriendPeerRecord {
            peer_id: "peer_b".to_string(),
            display_name: "B".to_string(),
            endpoint: "127.0.0.1:2".to_string(),
            added_at: "now".to_string(),
            last_seen_at: None,
            online: true,
            last_state_hash: None,
        },
    ];
    let trusted = normalize_trusted_peer_ids(&session, &[]);
    assert!(trusted.is_empty());
}

#[test]
fn trusted_peers_initialize_to_empty_for_legacy_sessions_without_bootstrap_host() {
    let mut session = sample_session();
    session.peers = vec![
        store::FriendPeerRecord {
            peer_id: "peer_a".to_string(),
            display_name: "A".to_string(),
            endpoint: "127.0.0.1:1".to_string(),
            added_at: "now".to_string(),
            last_seen_at: None,
            online: true,
            last_state_hash: None,
        },
        store::FriendPeerRecord {
            peer_id: "peer_b".to_string(),
            display_name: "B".to_string(),
            endpoint: "127.0.0.1:2".to_string(),
            added_at: "now".to_string(),
            last_seen_at: None,
            online: true,
            last_state_hash: None,
        },
    ];
    ensure_trusted_peer_ids_initialized(&mut session);
    assert!(session.trusted_peer_ids.is_empty());
    assert!(session.trusted_peer_ids_initialized);
}

#[test]
fn trusted_peers_default_to_bootstrap_host_when_present() {
    let mut session = sample_session();
    session.bootstrap_host_peer_id = Some("peer_host".to_string());
    session.peers = vec![
        store::FriendPeerRecord {
            peer_id: "peer_host".to_string(),
            display_name: "Host".to_string(),
            endpoint: "192.168.1.20:25565".to_string(),
            added_at: "now".to_string(),
            last_seen_at: None,
            online: true,
            last_state_hash: None,
        },
        store::FriendPeerRecord {
            peer_id: "peer_other".to_string(),
            display_name: "Other".to_string(),
            endpoint: "192.168.1.21:25565".to_string(),
            added_at: "now".to_string(),
            last_seen_at: None,
            online: true,
            last_state_hash: None,
        },
    ];
    ensure_trusted_peer_ids_initialized(&mut session);
    assert_eq!(session.trusted_peer_ids, vec!["peer_host".to_string()]);
}

#[test]
fn upsert_peer_transfers_trust_when_peer_id_rotates_on_same_host() {
    let mut session = sample_session();
    session.peers.push(store::FriendPeerRecord {
        peer_id: "peer_old".to_string(),
        display_name: "Buddy".to_string(),
        endpoint: "192.168.1.50:41000".to_string(),
        added_at: "now".to_string(),
        last_seen_at: Some("now".to_string()),
        online: true,
        last_state_hash: Some("old".to_string()),
    });
    session.trusted_peer_ids = vec!["peer_old".to_string()];
    session.trusted_peer_ids_initialized = true;
    session
        .peer_aliases
        .insert("peer_old".to_string(), "Bestie".to_string());

    upsert_peer(
        &mut session,
        store::FriendPeerRecord {
            peer_id: "peer_new".to_string(),
            display_name: "Buddy".to_string(),
            endpoint: "192.168.1.50:42000".to_string(),
            added_at: "now".to_string(),
            last_seen_at: Some("now".to_string()),
            online: true,
            last_state_hash: Some("new".to_string()),
        },
    );

    assert_eq!(session.peers.len(), 1);
    assert_eq!(session.peers[0].peer_id, "peer_new");
    assert!(session
        .trusted_peer_ids
        .iter()
        .any(|peer_id| peer_id == "peer_new"));
    assert!(!session
        .trusted_peer_ids
        .iter()
        .any(|peer_id| peer_id == "peer_old"));
    assert_eq!(
        session.peer_aliases.get("peer_new").map(String::as_str),
        Some("Bestie")
    );
}

#[test]
fn sanitize_filename_and_world_name_reject_traversal() {
    assert!(state::sanitize_lock_entry_filename("../bad.jar").is_err());
    assert!(state::sanitize_lock_entry_filename("mods/bad.jar").is_err());
    assert!(state::sanitize_world_name("world/../../escape").is_err());
    assert!(state::sanitize_world_name("..").is_err());
}

#[test]
fn safe_join_under_refuses_escape_attempts() {
    let root = std::env::temp_dir().join(format!("openjar-safe-join-{}", uuid::Uuid::new_v4()));
    assert!(state::safe_join_under(&root, "../escape.txt").is_err());
    assert!(state::safe_join_under(&root, "/absolute/path").is_err());
    assert!(state::safe_join_under(&root, "config/ok.toml").is_ok());
}

#[cfg(unix)]
#[test]
fn write_instance_config_file_refuses_symlink_parent_path() {
    use std::os::unix::fs::symlink;

    let temp = std::env::temp_dir().join(format!("openjar-symlink-write-{}", Uuid::new_v4()));
    let instances_dir = temp.join("instances");
    let instance_dir = instances_dir.join("inst_1");
    let config_dir = instance_dir.join("config");
    let outside_dir = temp.join("outside");

    fs::create_dir_all(&config_dir).expect("create config dir");
    fs::create_dir_all(&outside_dir).expect("create outside dir");
    symlink(&outside_dir, config_dir.join("linked")).expect("create symlinked parent");

    let err = state::write_instance_config_file(
        &instances_dir,
        "inst_1",
        "config/linked/unsafe.toml",
        "safe = false",
        None,
    )
    .expect_err("symlink parent path must be rejected");
    assert!(err.to_ascii_lowercase().contains("symlink"));

    let _ = fs::remove_dir_all(&temp);
}

#[cfg(unix)]
#[test]
fn read_lock_entry_bytes_refuses_symlinked_target_file() {
    use std::os::unix::fs::symlink;

    let temp = std::env::temp_dir().join(format!("openjar-symlink-read-{}", Uuid::new_v4()));
    let instances_dir = temp.join("instances");
    let instance_dir = instances_dir.join("inst_1");
    let mods_dir = instance_dir.join("mods");
    let outside_dir = temp.join("outside");
    let outside_file = outside_dir.join("outside.jar");

    fs::create_dir_all(&mods_dir).expect("create mods dir");
    fs::create_dir_all(&outside_dir).expect("create outside dir");
    fs::write(&outside_file, b"outside").expect("write outside file");
    symlink(&outside_file, mods_dir.join("linked.jar")).expect("create symlinked file");

    let entry = state::CanonicalLockEntry {
        source: "modrinth".to_string(),
        project_id: "proj".to_string(),
        version_id: "ver".to_string(),
        name: "Linked".to_string(),
        version_number: "1.0.0".to_string(),
        filename: "linked.jar".to_string(),
        content_type: "mods".to_string(),
        target_scope: "instance".to_string(),
        target_worlds: vec![],
        enabled: true,
        hashes: HashMap::new(),
    };
    let err = state::read_lock_entry_bytes(&instances_dir, "inst_1", &entry)
        .expect_err("symlinked content file must be rejected");
    assert!(err.to_ascii_lowercase().contains("symlink"));

    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn lock_entry_paths_never_escape_instance_dir() {
    let instances_dir =
        std::env::temp_dir().join(format!("openjar-lock-paths-{}", uuid::Uuid::new_v4()));
    let instance_path = PathBuf::from(&instances_dir).join("inst_a");
    let entry = state::CanonicalLockEntry {
        source: "modrinth".to_string(),
        project_id: "project".to_string(),
        version_id: "ver".to_string(),
        name: "Valid".to_string(),
        version_number: "1.0.0".to_string(),
        filename: "valid.jar".to_string(),
        content_type: "mods".to_string(),
        target_scope: "instance".to_string(),
        target_worlds: vec![],
        enabled: true,
        hashes: HashMap::new(),
    };
    let paths = state::lock_entry_paths(&instances_dir, "inst_a", &entry).expect("valid paths");
    assert_eq!(paths.len(), 1);
    assert!(paths[0].starts_with(&instance_path));

    let bad = state::CanonicalLockEntry {
        filename: "../evil.jar".to_string(),
        ..entry
    };
    assert!(state::lock_entry_paths(&instances_dir, "inst_a", &bad).is_err());
}

#[test]
fn write_store_moves_secret_to_keyring_and_omits_plaintext_secret_field() {
    store::clear_test_friend_link_secret_store();
    let path = std::env::temp_dir().join(format!(
        "openjar-friend-store-write-{}.json",
        Uuid::new_v4()
    ));
    let mut store = store::FriendLinkStoreV1::default();
    store.sessions.push(sample_session());

    store::write_store_at_path(&path, &store).expect("write friend-link store");
    let raw = fs::read_to_string(&path).expect("read written store");
    assert!(
        !raw.contains("\"shared_secret_b64\""),
        "friend-link store must not contain plaintext shared secret"
    );
    assert!(
        raw.contains("\"shared_secret_key_id\""),
        "friend-link store must include key-id reference"
    );

    let _ = fs::remove_file(path);
}

#[test]
fn read_store_migrates_legacy_plaintext_secret_and_rewrites_store_file() {
    store::clear_test_friend_link_secret_store();
    let path = std::env::temp_dir().join(format!(
        "openjar-friend-store-migrate-{}.json",
        Uuid::new_v4()
    ));
    let legacy_raw = serde_json::json!({
      "version": 1,
      "sessions": [{
        "instance_id": "inst_legacy",
        "group_id": "group_legacy",
        "local_peer_id": "peer_legacy",
        "display_name": "Legacy",
        "shared_secret_b64": "aGVsbG8=",
        "protocol_version": 1,
        "listener_port": 0,
        "listener_endpoint": null,
        "peers": [],
        "allowlist": state::default_allowlist(),
        "last_peer_sync_at": {},
        "last_good_snapshot": null,
        "pending_conflicts": [],
        "cached_peer_state": {},
        "bootstrap_host_peer_id": null,
        "trusted_peer_ids": [],
        "trusted_peer_ids_initialized": false,
        "guardrails_updated_at_ms": 0,
        "peer_aliases": {},
        "allow_loopback_endpoints": false,
        "allow_internet_endpoints": false,
        "max_auto_changes": 25,
        "sync_mods": true,
        "sync_resourcepacks": false,
        "sync_shaderpacks": true,
        "sync_datapacks": true
      }]
    });
    fs::write(
        &path,
        serde_json::to_string_pretty(&legacy_raw).expect("serialize legacy store"),
    )
    .expect("write legacy store");

    let loaded = store::read_store_at_path(&path).expect("read and migrate legacy store");
    assert_eq!(loaded.sessions.len(), 1);
    assert!(loaded.sessions[0].shared_secret_b64.is_empty());
    assert!(!loaded.sessions[0].shared_secret_key_id.trim().is_empty());

    let rewritten = fs::read_to_string(&path).expect("read rewritten store");
    assert!(!rewritten.contains("\"shared_secret_b64\""));
    assert!(rewritten.contains("\"shared_secret_key_id\""));

    let _ = fs::remove_file(path);
}

#[test]
fn missing_secure_secret_can_be_recovered_for_host_session() {
    store::clear_test_friend_link_secret_store();
    let mut session = sample_session();
    session.shared_secret_b64.clear();
    session.shared_secret_key_id = "friend_link_secret_missing_test".to_string();

    let err =
        ensure_session_secret_loaded(&mut session).expect_err("missing keyring secret must error");
    assert!(session_secret_missing_or_broken(&err));

    reset_session_for_new_host_secret(&mut session);
    ensure_session_secret_loaded(&mut session).expect("recovered host session secret should load");
    assert!(!session.group_id.trim().is_empty());
    assert!(!session.local_peer_id.trim().is_empty());
    assert!(!session.shared_secret_key_id.trim().is_empty());
}

#[test]
fn get_session_secret_migrates_from_legacy_keyring_service_alias() {
    store::clear_test_friend_link_secret_store();
    let mut session = sample_session();
    session.shared_secret_b64.clear();
    session.shared_secret_key_id = format!("friend_link_secret_v1_{}", Uuid::new_v4());

    store::set_test_friend_link_secret_for_service(
        "modpack-manager",
        &session.shared_secret_key_id,
        "legacy_secret_b64",
    )
    .expect("seed legacy keyring service secret");

    let loaded =
        store::get_session_shared_secret(&mut session).expect("load migrated legacy secret");
    assert_eq!(loaded, "legacy_secret_b64");
    assert_eq!(session.shared_secret_b64, "legacy_secret_b64");

    let canonical = store::get_test_friend_link_secret_for_service(
        "OpenJar Launcher",
        &session.shared_secret_key_id,
    );
    assert_eq!(canonical.as_deref(), Some("legacy_secret_b64"));
}

#[test]
fn write_store_preserves_peers_from_newer_snapshot_on_disk() {
    store::clear_test_friend_link_secret_store();
    let path = std::env::temp_dir().join(format!(
        "openjar-friend-store-merge-{}.json",
        Uuid::new_v4()
    ));

    let mut base = store::FriendLinkStoreV1::default();
    base.sessions.push(sample_session());
    store::write_store_at_path(&path, &base).expect("write base store");

    let mut stale = base.clone();
    stale.sessions[0].display_name = "Host (stale writer)".to_string();

    let mut fresh = base.clone();
    fresh.sessions[0].peers.push(store::FriendPeerRecord {
        peer_id: "peer_2".to_string(),
        display_name: "Peer Two".to_string(),
        endpoint: "127.0.0.1:45100".to_string(),
        added_at: "now".to_string(),
        last_seen_at: Some("now".to_string()),
        online: true,
        last_state_hash: Some("hash-1".to_string()),
    });
    store::write_store_at_path(&path, &fresh).expect("write fresh peer update");
    store::write_store_at_path(&path, &stale).expect("stale write should not drop peers");

    let loaded = store::read_store_at_path(&path).expect("read merged store");
    let session = loaded
        .sessions
        .iter()
        .find(|v| v.instance_id == "inst_1")
        .expect("session exists");
    assert!(
        session.peers.iter().any(|peer| peer.peer_id == "peer_2"),
        "stale writer must not remove newly added peers"
    );

    let _ = fs::remove_file(path);
}

#[test]
fn write_store_preserves_newer_guardrails_against_stale_snapshot() {
    store::clear_test_friend_link_secret_store();
    let path = std::env::temp_dir().join(format!(
        "openjar-friend-store-guardrails-{}.json",
        Uuid::new_v4()
    ));

    let mut base = store::FriendLinkStoreV1::default();
    let mut session = sample_session();
    session.peers.push(store::FriendPeerRecord {
        peer_id: "peer_2".to_string(),
        display_name: "Peer Two".to_string(),
        endpoint: "127.0.0.1:45100".to_string(),
        added_at: "now".to_string(),
        last_seen_at: Some("now".to_string()),
        online: true,
        last_state_hash: None,
    });
    session.trusted_peer_ids_initialized = true;
    session.guardrails_updated_at_ms = 100;
    base.sessions.push(session);
    store::write_store_at_path(&path, &base).expect("write base store");

    let mut fresh = base.clone();
    fresh.sessions[0].trusted_peer_ids = vec!["peer_2".to_string()];
    fresh.sessions[0].max_auto_changes = 42;
    fresh.sessions[0].guardrails_updated_at_ms = 200;
    store::write_store_at_path(&path, &fresh).expect("write fresh guardrails");

    let mut stale = base.clone();
    stale.sessions[0].trusted_peer_ids = vec![];
    stale.sessions[0].max_auto_changes = 5;
    stale.sessions[0].guardrails_updated_at_ms = 150;
    store::write_store_at_path(&path, &stale).expect("write stale guardrails");

    let loaded = store::read_store_at_path(&path).expect("read merged guardrails");
    let session = loaded
        .sessions
        .iter()
        .find(|v| v.instance_id == "inst_1")
        .expect("session exists");
    assert_eq!(session.trusted_peer_ids, vec!["peer_2".to_string()]);
    assert_eq!(session.max_auto_changes, 42);
    assert_eq!(session.guardrails_updated_at_ms, 200);

    let _ = fs::remove_file(path);
}
