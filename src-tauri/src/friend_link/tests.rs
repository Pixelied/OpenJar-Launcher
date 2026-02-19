use super::*;
use std::collections::HashMap;

fn sample_session() -> store::FriendLinkSessionRecord {
    store::FriendLinkSessionRecord {
        instance_id: "inst_1".to_string(),
        group_id: "group_1".to_string(),
        local_peer_id: "peer_1".to_string(),
        display_name: "Host".to_string(),
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
        peer_aliases: HashMap::new(),
        max_auto_changes: 25,
        sync_mods: true,
        sync_resourcepacks: false,
        sync_shaderpacks: true,
        sync_datapacks: true,
    }
}

#[test]
fn invite_roundtrip() {
    let session = sample_session();
    let invite = build_invite(&session).expect("build invite");
    let payload = parse_invite(&invite.invite_code).expect("parse invite");
    assert_eq!(payload.group_id, session.group_id);
    assert_eq!(payload.bootstrap_peer_endpoint, "127.0.0.1:45001");
    assert_eq!(payload.protocol_version, PROTOCOL_VERSION);
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

    assert_eq!(state::lock_entry_hash(&base), state::lock_entry_hash(&other));
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
fn trusted_peers_initialize_to_all_once_for_legacy_sessions() {
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
    assert_eq!(session.trusted_peer_ids.len(), 2);
    assert!(session.trusted_peer_ids.contains(&"peer_a".to_string()));
    assert!(session.trusted_peer_ids.contains(&"peer_b".to_string()));
    assert!(session.trusted_peer_ids_initialized);
}
