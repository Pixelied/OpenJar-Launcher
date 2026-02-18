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
