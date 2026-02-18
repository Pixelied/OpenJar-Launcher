pub mod net;
pub mod state;
pub mod store;
#[cfg(test)]
mod tests;

use crate::friend_link::net::{endpoint_for_port, request_lock_entry_file, HelloPayload};
use crate::friend_link::state::{
    app_instances_dir, collect_sync_state, config_file_map, lock_entry_hash, lock_entry_map, preview_for_config_file,
    preview_for_lock_entry, state_manifest, CanonicalLockEntry, ConfigFileState, InstanceConfigFileEntry,
    ReadInstanceConfigFileResult, SyncState, WriteInstanceConfigFileResult,
};
use crate::friend_link::store::{
    get_session, get_session_mut, read_store, remove_session, upsert_session, write_store, FriendLastGoodSnapshot,
    FriendLinkSessionRecord, FriendManifestEntry, FriendPeerRecord, FriendSyncConflictRecord,
};
use base64::engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;

const PROTOCOL_VERSION: u32 = 1;
const MAX_PEERS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkInvite {
    pub invite_code: String,
    pub group_id: String,
    pub expires_at: String,
    pub bootstrap_peer_endpoint: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkPeer {
    pub peer_id: String,
    pub display_name: String,
    pub endpoint: String,
    pub online: bool,
    #[serde(default)]
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkStatus {
    pub instance_id: String,
    pub linked: bool,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub local_peer_id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub listener_endpoint: Option<String>,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub peers: Vec<FriendLinkPeer>,
    pub pending_conflicts_count: usize,
    pub status: String,
    #[serde(default)]
    pub last_good_hash: Option<String>,
}

pub type FriendSyncItemKind = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendSyncConflict {
    pub id: String,
    pub kind: FriendSyncItemKind,
    pub key: String,
    pub peer_id: String,
    pub mine_hash: String,
    pub theirs_hash: String,
    #[serde(default)]
    pub mine_preview: Option<String>,
    #[serde(default)]
    pub theirs_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkReconcileAction {
    pub kind: FriendSyncItemKind,
    pub key: String,
    pub peer_id: String,
    pub applied: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkReconcileResult {
    pub status: String,
    pub mode: String,
    pub actions_applied: usize,
    pub actions_pending: usize,
    #[serde(default)]
    pub actions: Vec<FriendLinkReconcileAction>,
    #[serde(default)]
    pub conflicts: Vec<FriendSyncConflict>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    pub local_state_hash: String,
    #[serde(default)]
    pub last_good_hash: Option<String>,
    pub offline_peers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkDebugBundleResult {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResolutionItem {
    pub conflict_id: String,
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResolutionPayload {
    #[serde(default)]
    pub keep_all_mine: bool,
    #[serde(default)]
    pub take_all_theirs: bool,
    #[serde(default)]
    pub items: Vec<ConflictResolutionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InvitePayload {
    group_id: String,
    bootstrap_peer_endpoint: String,
    shared_secret: String,
    expires_at: String,
    protocol_version: u32,
    host_peer_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateFriendLinkSessionArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    #[serde(alias = "displayName", default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JoinFriendLinkSessionArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    #[serde(alias = "inviteCode")]
    pub invite_code: String,
    #[serde(alias = "displayName", default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LeaveFriendLinkSessionArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
}

#[derive(Debug, Deserialize)]
pub struct GetFriendLinkStatusArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SetFriendLinkAllowlistArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    pub allowlist: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReconcileFriendLinkArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveFriendLinkConflictsArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    pub resolution: ConflictResolutionPayload,
}

#[derive(Debug, Deserialize)]
pub struct ExportFriendLinkDebugBundleArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ListInstanceConfigFilesArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadInstanceConfigFileArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct WriteInstanceConfigFileArgs {
    #[serde(alias = "instanceId")]
    pub instance_id: String,
    pub path: String,
    pub content: String,
    #[serde(alias = "expectedModifiedAt", default)]
    pub expected_modified_at: Option<i64>,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn sanitize_display_name(input: Option<String>, fallback_suffix: &str) -> String {
    input
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("Peer-{}", fallback_suffix))
}

fn random_secret_b64() -> String {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    BASE64_STANDARD.encode(bytes)
}

fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path_resolver()
        .app_data_dir()
        .ok_or_else(|| "cannot resolve app data dir".to_string())
}

fn normalize_allowlist(input: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    let base = if input.is_empty() {
        state::default_allowlist()
    } else {
        input.to_vec()
    };

    for item in base {
        let trimmed = item.trim().replace('\\', "/");
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if state::hard_excluded_prefixes()
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        {
            continue;
        }
        if seen.insert(lower.clone()) {
            out.push(trimmed);
        }
    }

    if !out.iter().any(|p| p.eq_ignore_ascii_case("options.txt")) {
        out.insert(0, "options.txt".to_string());
    }

    out
}

fn to_status(session: Option<&FriendLinkSessionRecord>, instance_id: &str) -> FriendLinkStatus {
    if let Some(session) = session {
        FriendLinkStatus {
            instance_id: instance_id.to_string(),
            linked: true,
            group_id: Some(session.group_id.clone()),
            local_peer_id: Some(session.local_peer_id.clone()),
            display_name: Some(session.display_name.clone()),
            listener_endpoint: session.listener_endpoint.clone(),
            allowlist: session.allowlist.clone(),
            peers: session
                .peers
                .iter()
                .map(|peer| FriendLinkPeer {
                    peer_id: peer.peer_id.clone(),
                    display_name: peer.display_name.clone(),
                    endpoint: peer.endpoint.clone(),
                    online: peer.online,
                    last_seen_at: peer.last_seen_at.clone(),
                })
                .collect(),
            pending_conflicts_count: session.pending_conflicts.len(),
            status: if session.pending_conflicts.is_empty() {
                "synced".to_string()
            } else {
                "conflicted".to_string()
            },
            last_good_hash: session
                .last_good_snapshot
                .as_ref()
                .map(|snap| snap.state_hash.clone()),
        }
    } else {
        FriendLinkStatus {
            instance_id: instance_id.to_string(),
            linked: false,
            group_id: None,
            local_peer_id: None,
            display_name: None,
            listener_endpoint: None,
            allowlist: vec![],
            peers: vec![],
            pending_conflicts_count: 0,
            status: "unlinked".to_string(),
            last_good_hash: None,
        }
    }
}

fn build_invite(session: &FriendLinkSessionRecord) -> Result<FriendLinkInvite, String> {
    let endpoint = session
        .listener_endpoint
        .clone()
        .unwrap_or_else(|| endpoint_for_port(session.listener_port));
    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
    let payload = InvitePayload {
        group_id: session.group_id.clone(),
        bootstrap_peer_endpoint: endpoint.clone(),
        shared_secret: session.shared_secret_b64.clone(),
        expires_at: expires_at.clone(),
        protocol_version: PROTOCOL_VERSION,
        host_peer_id: session.local_peer_id.clone(),
    };
    let raw = serde_json::to_vec(&payload).map_err(|e| format!("serialize invite payload failed: {e}"))?;
    let invite_code = URL_SAFE_NO_PAD.encode(raw);
    Ok(FriendLinkInvite {
        invite_code,
        group_id: payload.group_id,
        expires_at,
        bootstrap_peer_endpoint: endpoint,
        protocol_version: PROTOCOL_VERSION,
    })
}

fn parse_invite(code: &str) -> Result<InvitePayload, String> {
    let raw = URL_SAFE_NO_PAD
        .decode(code.trim())
        .map_err(|e| format!("decode invite code failed: {e}"))?;
    let payload: InvitePayload =
        serde_json::from_slice(&raw).map_err(|e| format!("parse invite payload failed: {e}"))?;
    if payload.group_id.trim().is_empty() {
        return Err("Invite is missing group id".to_string());
    }
    if payload.bootstrap_peer_endpoint.trim().is_empty() {
        return Err("Invite is missing bootstrap endpoint".to_string());
    }
    if payload.shared_secret.trim().is_empty() {
        return Err("Invite is missing shared secret".to_string());
    }
    let expires = chrono::DateTime::parse_from_rfc3339(&payload.expires_at)
        .map_err(|e| format!("invalid invite expiration timestamp: {e}"))?;
    if expires.with_timezone(&chrono::Utc) < chrono::Utc::now() {
        return Err("Invite code has expired".to_string());
    }
    Ok(payload)
}

fn upsert_peer(session: &mut FriendLinkSessionRecord, peer: FriendPeerRecord) {
    if peer.peer_id == session.local_peer_id {
        return;
    }
    if let Some(found) = session.peers.iter_mut().find(|p| p.peer_id == peer.peer_id) {
        *found = peer;
    } else {
        session.peers.push(peer);
    }
}

fn lock_manifest_map(snapshot: &FriendLastGoodSnapshot) -> HashMap<String, String> {
    snapshot
        .manifest
        .iter()
        .map(|entry| (entry.key.clone(), entry.hash.clone()))
        .collect()
}

fn conflict_from_lock(
    key: &str,
    peer_id: &str,
    mine: Option<&CanonicalLockEntry>,
    theirs: &CanonicalLockEntry,
) -> FriendSyncConflictRecord {
    FriendSyncConflictRecord {
        id: format!("conf_{}", Uuid::new_v4()),
        kind: "lock_entry".to_string(),
        key: key.to_string(),
        peer_id: peer_id.to_string(),
        mine_hash: mine.map(lock_entry_hash).unwrap_or_else(|| "absent".to_string()),
        theirs_hash: lock_entry_hash(theirs),
        mine_preview: mine.map(preview_for_lock_entry),
        theirs_preview: Some(preview_for_lock_entry(theirs)),
        mine_value: mine
            .cloned()
            .and_then(|v| serde_json::to_value(v).ok()),
        theirs_value: serde_json::to_value(theirs).ok(),
        created_at: now_iso(),
    }
}

fn conflict_from_config(
    key: &str,
    peer_id: &str,
    mine: Option<&ConfigFileState>,
    theirs: &ConfigFileState,
) -> FriendSyncConflictRecord {
    FriendSyncConflictRecord {
        id: format!("conf_{}", Uuid::new_v4()),
        kind: "config_file".to_string(),
        key: key.to_string(),
        peer_id: peer_id.to_string(),
        mine_hash: mine.map(|v| v.hash.clone()).unwrap_or_else(|| "absent".to_string()),
        theirs_hash: theirs.hash.clone(),
        mine_preview: mine.map(preview_for_config_file),
        theirs_preview: Some(preview_for_config_file(theirs)),
        mine_value: mine
            .cloned()
            .and_then(|v| serde_json::to_value(v).ok()),
        theirs_value: serde_json::to_value(theirs).ok(),
        created_at: now_iso(),
    }
}

fn sync_conflicts_public(conflicts: &[FriendSyncConflictRecord]) -> Vec<FriendSyncConflict> {
    conflicts
        .iter()
        .map(|c| FriendSyncConflict {
            id: c.id.clone(),
            kind: c.kind.clone(),
            key: c.key.clone(),
            peer_id: c.peer_id.clone(),
            mine_hash: c.mine_hash.clone(),
            theirs_hash: c.theirs_hash.clone(),
            mine_preview: c.mine_preview.clone(),
            theirs_preview: c.theirs_preview.clone(),
        })
        .collect()
}

fn store_last_good(session: &mut FriendLinkSessionRecord, local_state: &SyncState) {
    let manifest = state_manifest(local_state)
        .into_iter()
        .map(|(key, hash, kind)| FriendManifestEntry { key, hash, kind })
        .collect::<Vec<_>>();

    session.last_good_snapshot = Some(FriendLastGoodSnapshot {
        state_hash: local_state.state_hash.clone(),
        manifest,
        updated_at: now_iso(),
    });
}

fn apply_lock_map(instances_dir: &PathBuf, instance_id: &str, map: &HashMap<String, CanonicalLockEntry>) -> Result<(), String> {
    let mut entries = map.values().cloned().collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        format!("{}:{}:{}", a.source, a.content_type, a.project_id)
            .cmp(&format!("{}:{}:{}", b.source, b.content_type, b.project_id))
    });
    state::write_lock_entries(instances_dir, instance_id, &entries)
}

fn apply_config_file(
    instances_dir: &PathBuf,
    instance_id: &str,
    file: &ConfigFileState,
) -> Result<(), String> {
    let _ = state::write_instance_config_file(
        instances_dir,
        instance_id,
        &file.path,
        &file.content,
        None,
    )?;
    Ok(())
}

fn supports_binary_sync(entry: &CanonicalLockEntry) -> bool {
    matches!(
        entry.content_type.trim().to_lowercase().as_str(),
        "mods" | "resourcepacks" | "shaderpacks" | "datapacks"
    )
}

fn sync_lock_entry_binaries(
    instances_dir: &PathBuf,
    instance_id: &str,
    session: &FriendLinkSessionRecord,
    lock_map: &HashMap<String, CanonicalLockEntry>,
    preferred_peer_by_key: &HashMap<String, String>,
    actions: &mut Vec<FriendLinkReconcileAction>,
    warnings: &mut Vec<String>,
) -> Result<usize, String> {
    let peer_endpoint_by_id = session
        .peers
        .iter()
        .filter(|peer| peer.online)
        .map(|peer| (peer.peer_id.clone(), peer.endpoint.clone()))
        .collect::<HashMap<_, _>>();

    let mut failure_count = 0usize;
    for (key, entry) in lock_map {
        if !supports_binary_sync(entry) {
            continue;
        }
        let missing = state::lock_entry_file_missing(instances_dir, instance_id, entry);
        let should_force_refresh = preferred_peer_by_key.contains_key(key);
        if !missing && !should_force_refresh {
            continue;
        }

        let mut endpoints = Vec::new();
        if let Some(peer_id) = preferred_peer_by_key.get(key) {
            if let Some(endpoint) = peer_endpoint_by_id.get(peer_id) {
                endpoints.push(endpoint.clone());
            }
        }
        for endpoint in peer_endpoint_by_id.values() {
            if !endpoints.iter().any(|v| v == endpoint) {
                endpoints.push(endpoint.clone());
            }
        }

        let mut synced = false;
        let mut last_error: Option<String> = None;
        for endpoint in endpoints {
            match request_lock_entry_file(session, &endpoint, key) {
                Ok(response) => {
                    if !response.found {
                        last_error = Some(
                            response
                                .message
                                .unwrap_or_else(|| "peer did not return file bytes".to_string()),
                        );
                        continue;
                    }
                    let Some(raw_b64) = response.bytes_b64 else {
                        last_error = Some("peer response missing file bytes".to_string());
                        continue;
                    };
                    let bytes = BASE64_STANDARD
                        .decode(raw_b64.as_bytes())
                        .map_err(|e| format!("decode transferred content failed: {e}"))?;
                    if let Some(expected) = response.sha256.as_deref() {
                        use sha2::Digest as _;
                        let mut hasher = sha2::Sha256::new();
                        hasher.update(&bytes);
                        let actual = format!("{:x}", hasher.finalize());
                        if actual != expected {
                            last_error = Some("peer file hash verification failed".to_string());
                            continue;
                        }
                    }
                    let wrote = state::write_lock_entry_bytes(instances_dir, instance_id, entry, &bytes)?;
                    actions.push(FriendLinkReconcileAction {
                        kind: "lock_entry".to_string(),
                        key: key.clone(),
                        peer_id: preferred_peer_by_key
                            .get(key)
                            .cloned()
                            .unwrap_or_else(|| "peer".to_string()),
                        applied: true,
                        message: format!("Synced {} binary file(s) for '{}'.", wrote, entry.name),
                    });
                    synced = true;
                    break;
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        if !synced && (missing || should_force_refresh) {
            failure_count += 1;
            warnings.push(format!(
                "Could not sync binary for '{}': {}",
                entry.name,
                last_error.unwrap_or_else(|| "no reachable peer had the file".to_string())
            ));
        }
    }

    Ok(failure_count)
}

fn reconcile_internal(
    app: &tauri::AppHandle,
    instance_id: &str,
    mode: &str,
) -> Result<FriendLinkReconcileResult, String> {
    let mut store = read_store(app)?;
    let Some(session) = get_session_mut(&mut store, instance_id) else {
        return Ok(FriendLinkReconcileResult {
            status: "unlinked".to_string(),
            mode: mode.to_string(),
            actions_applied: 0,
            actions_pending: 0,
            actions: vec![],
            conflicts: vec![],
            warnings: vec![],
            blocked_reason: None,
            local_state_hash: String::new(),
            last_good_hash: None,
            offline_peers: 0,
        });
    };

    let app_data = app_data_dir(app)?;
    let _ = net::ensure_listener(app_data, session)?;

    let instances_dir = app_instances_dir(app)?;
    let local_state = collect_sync_state(&instances_dir, instance_id, &session.allowlist)?;
    let mut current_lock = lock_entry_map(&local_state.lock_entries);
    let mut current_config = config_file_map(&local_state.config_files);

    let baseline = session
        .last_good_snapshot
        .as_ref()
        .map(lock_manifest_map)
        .unwrap_or_default();

    let mut actions = Vec::<FriendLinkReconcileAction>::new();
    let mut warnings = Vec::<String>::new();
    let mut conflicts = Vec::<FriendSyncConflictRecord>::new();
    let mut offline_peers = 0usize;
    let mut binary_preferred_peer_by_key = HashMap::<String, String>::new();

    for peer in session.peers.clone() {
        let response = net::request_state(session, &peer.endpoint);
        let peer_idx = session.peers.iter().position(|p| p.peer_id == peer.peer_id);
        match response {
            Ok(payload) => {
                if let Some(idx) = peer_idx {
                    session.peers[idx].online = true;
                    session.peers[idx].last_seen_at = Some(now_iso());
                    session.peers[idx].last_state_hash = Some(payload.state.state_hash.clone());
                }
                session
                    .cached_peer_state
                    .insert(peer.peer_id.clone(), payload.state.clone());

                let remote_lock = lock_entry_map(&payload.state.lock_entries);
                for (key, remote_entry) in &remote_lock {
                    let local = current_lock.get(key);
                    let local_hash = local.map(lock_entry_hash);
                    let remote_hash = lock_entry_hash(remote_entry);
                    if local_hash.as_deref() == Some(remote_hash.as_str()) {
                        continue;
                    }
                    let baseline_hash = baseline.get(key).cloned();
                    let local_changed = baseline_hash
                        .as_ref()
                        .map(|v| local_hash.as_deref() != Some(v.as_str()))
                        .unwrap_or(local.is_some());
                    let remote_changed = baseline_hash
                        .as_ref()
                        .map(|v| v != &remote_hash)
                        .unwrap_or(true);

                    if !local_changed || local.is_none() {
                        current_lock.insert(key.clone(), remote_entry.clone());
                        binary_preferred_peer_by_key.insert(key.clone(), peer.peer_id.clone());
                        actions.push(FriendLinkReconcileAction {
                            kind: "lock_entry".to_string(),
                            key: key.clone(),
                            peer_id: peer.peer_id.clone(),
                            applied: true,
                            message: format!("Applied lock entry from {}", peer.display_name),
                        });
                    } else if remote_changed {
                        conflicts.push(conflict_from_lock(
                            key,
                            &peer.peer_id,
                            local,
                            remote_entry,
                        ));
                    }
                }

                let remote_config = config_file_map(&payload.state.config_files);
                for (key, remote_file) in &remote_config {
                    let local = current_config.get(key);
                    if local.map(|f| f.hash.as_str()) == Some(remote_file.hash.as_str()) {
                        continue;
                    }
                    let baseline_hash = baseline.get(key).cloned();
                    let local_changed = baseline_hash
                        .as_ref()
                        .map(|v| local.map(|f| f.hash.as_str()) != Some(v.as_str()))
                        .unwrap_or(local.is_some());
                    let remote_changed = baseline_hash
                        .as_ref()
                        .map(|v| v != &remote_file.hash)
                        .unwrap_or(true);

                    if !local_changed || local.is_none() {
                        current_config.insert(key.clone(), remote_file.clone());
                        actions.push(FriendLinkReconcileAction {
                            kind: "config_file".to_string(),
                            key: key.clone(),
                            peer_id: peer.peer_id.clone(),
                            applied: true,
                            message: format!("Applied config file from {}", peer.display_name),
                        });
                    } else if remote_changed {
                        conflicts.push(conflict_from_config(
                            key,
                            &peer.peer_id,
                            local,
                            remote_file,
                        ));
                    }
                }
            }
            Err(err) => {
                offline_peers += 1;
                warnings.push(format!(
                    "Peer '{}' is offline or unreachable: {}",
                    peer.display_name, err
                ));
                if let Some(idx) = peer_idx {
                    session.peers[idx].online = false;
                }
            }
        }
    }

    if !actions.is_empty() {
        apply_lock_map(&instances_dir, instance_id, &current_lock)?;
        for file in current_config.values() {
            apply_config_file(&instances_dir, instance_id, file)?;
        }
    }
    let binary_sync_failures = sync_lock_entry_binaries(
        &instances_dir,
        instance_id,
        session,
        &current_lock,
        &binary_preferred_peer_by_key,
        &mut actions,
        &mut warnings,
    )?;

    let local_after = collect_sync_state(&instances_dir, instance_id, &session.allowlist)?;
    let mut status = "synced".to_string();
    let mut blocked_reason = None;

    if !conflicts.is_empty() {
        status = "conflicted".to_string();
    } else if binary_sync_failures > 0 && mode.eq_ignore_ascii_case("prelaunch") {
        status = "error".to_string();
        blocked_reason = Some(format!(
            "Friend Link could not fetch {} required content file(s) from peers.",
            binary_sync_failures
        ));
    } else if offline_peers > 0 {
        if let Some(last_good) = session.last_good_snapshot.as_ref() {
            if local_after.state_hash == last_good.state_hash {
                status = "degraded_offline_last_good".to_string();
            } else if mode.eq_ignore_ascii_case("prelaunch") {
                status = "blocked_offline_stale".to_string();
                blocked_reason = Some(
                    "One or more peers are offline and local state differs from last fully-synced snapshot."
                        .to_string(),
                );
            } else {
                status = "error".to_string();
            }
        } else if mode.eq_ignore_ascii_case("prelaunch") {
            status = "blocked_offline_stale".to_string();
            blocked_reason = Some(
                "One or more peers are offline and no last-good snapshot is available.".to_string(),
            );
        } else {
            status = "error".to_string();
        }
    }

    session.pending_conflicts = conflicts.clone();
    if status == "synced" {
        store_last_good(session, &local_after);
        let now = now_millis();
        for peer in &session.peers {
            if peer.online {
                session
                    .last_peer_sync_at
                    .insert(peer.peer_id.clone(), now);
            }
        }
    }

    let result = FriendLinkReconcileResult {
        status,
        mode: mode.to_string(),
        actions_applied: actions.iter().filter(|a| a.applied).count(),
        actions_pending: actions.iter().filter(|a| !a.applied).count(),
        actions,
        conflicts: sync_conflicts_public(&conflicts),
        warnings,
        blocked_reason,
        local_state_hash: local_after.state_hash,
        last_good_hash: session
            .last_good_snapshot
            .as_ref()
            .map(|v| v.state_hash.clone()),
        offline_peers,
    };

    write_store(app, &store)?;
    Ok(result)
}

#[tauri::command]
pub fn create_friend_link_session(
    app: tauri::AppHandle,
    args: CreateFriendLinkSessionArgs,
) -> Result<FriendLinkInvite, String> {
    let mut store = read_store(&app)?;
    let mut session = if let Some(existing) = get_session(&store, &args.instance_id) {
        existing
    } else {
        let suffix = Uuid::new_v4().to_string();
        FriendLinkSessionRecord {
            instance_id: args.instance_id.clone(),
            group_id: format!("group_{}", Uuid::new_v4()),
            local_peer_id: format!("peer_{}", Uuid::new_v4()),
            display_name: sanitize_display_name(args.display_name.clone(), &suffix[..8]),
            shared_secret_b64: random_secret_b64(),
            protocol_version: PROTOCOL_VERSION,
            listener_port: 0,
            listener_endpoint: None,
            peers: vec![],
            allowlist: state::default_allowlist(),
            last_peer_sync_at: HashMap::new(),
            last_good_snapshot: None,
            pending_conflicts: vec![],
            cached_peer_state: HashMap::new(),
        }
    };

    let app_data = app_data_dir(&app)?;
    let endpoint = net::ensure_listener(app_data, &mut session)?;
    session.listener_endpoint = Some(endpoint);

    upsert_session(&mut store, session.clone());
    write_store(&app, &store)?;

    build_invite(&session)
}

#[tauri::command]
pub fn join_friend_link_session(
    app: tauri::AppHandle,
    args: JoinFriendLinkSessionArgs,
) -> Result<FriendLinkStatus, String> {
    let invite = parse_invite(&args.invite_code)?;

    let mut store = read_store(&app)?;
    let suffix = Uuid::new_v4().to_string();
    let mut session = FriendLinkSessionRecord {
        instance_id: args.instance_id.clone(),
        group_id: invite.group_id.clone(),
        local_peer_id: format!("peer_{}", Uuid::new_v4()),
        display_name: sanitize_display_name(args.display_name.clone(), &suffix[..8]),
        shared_secret_b64: invite.shared_secret.clone(),
        protocol_version: invite.protocol_version,
        listener_port: 0,
        listener_endpoint: None,
        peers: vec![],
        allowlist: state::default_allowlist(),
        last_peer_sync_at: HashMap::new(),
        last_good_snapshot: None,
        pending_conflicts: vec![],
        cached_peer_state: HashMap::new(),
    };

    let app_data = app_data_dir(&app)?;
    let endpoint = net::ensure_listener(app_data, &mut session)?;
    session.listener_endpoint = Some(endpoint.clone());

    let hello = HelloPayload {
        peer_id: session.local_peer_id.clone(),
        display_name: session.display_name.clone(),
        endpoint,
    };
    let ack = net::send_hello(&session, &invite.bootstrap_peer_endpoint, hello)?;

    upsert_peer(
        &mut session,
        FriendPeerRecord {
            peer_id: ack.peer_id.clone(),
            display_name: ack.display_name.clone(),
            endpoint: ack.endpoint.clone(),
            added_at: now_iso(),
            last_seen_at: Some(now_iso()),
            online: true,
            last_state_hash: None,
        },
    );

    for peer in ack.peers {
        upsert_peer(
            &mut session,
            FriendPeerRecord {
                peer_id: peer.peer_id,
                display_name: peer.display_name,
                endpoint: peer.endpoint,
                added_at: now_iso(),
                last_seen_at: Some(now_iso()),
                online: peer.online,
                last_state_hash: None,
            },
        );
    }

    if session.peers.len() + 1 > MAX_PEERS {
        return Err("Linked group is full. Maximum group size is 4 peers.".to_string());
    }

    upsert_session(&mut store, session.clone());
    write_store(&app, &store)?;
    Ok(to_status(Some(&session), &args.instance_id))
}

#[tauri::command]
pub fn leave_friend_link_session(
    app: tauri::AppHandle,
    args: LeaveFriendLinkSessionArgs,
) -> Result<FriendLinkStatus, String> {
    let mut store = read_store(&app)?;
    let removed = remove_session(&mut store, &args.instance_id);
    if removed {
        write_store(&app, &store)?;
    }
    net::stop_listener(&args.instance_id);
    Ok(to_status(None, &args.instance_id))
}

#[tauri::command]
pub fn get_friend_link_status(
    app: tauri::AppHandle,
    args: GetFriendLinkStatusArgs,
) -> Result<FriendLinkStatus, String> {
    let mut store = read_store(&app)?;
    let mut changed = false;
    if let Some(session) = get_session_mut(&mut store, &args.instance_id) {
        let app_data = app_data_dir(&app)?;
        let endpoint = net::ensure_listener(app_data, session)?;
        if session.listener_endpoint.as_deref() != Some(endpoint.as_str()) {
            session.listener_endpoint = Some(endpoint);
            changed = true;
        }
    }
    if changed {
        write_store(&app, &store)?;
    }
    let session = get_session(&store, &args.instance_id);
    Ok(to_status(session.as_ref(), &args.instance_id))
}

#[tauri::command]
pub fn set_friend_link_allowlist(
    app: tauri::AppHandle,
    args: SetFriendLinkAllowlistArgs,
) -> Result<FriendLinkStatus, String> {
    let mut store = read_store(&app)?;
    let session = get_session_mut(&mut store, &args.instance_id)
        .ok_or_else(|| "Instance is not linked".to_string())?;
    session.allowlist = normalize_allowlist(&args.allowlist);
    let session_snapshot = session.clone();
    write_store(&app, &store)?;
    Ok(to_status(Some(&session_snapshot), &args.instance_id))
}

#[tauri::command]
pub fn reconcile_friend_link(
    app: tauri::AppHandle,
    args: ReconcileFriendLinkArgs,
) -> Result<FriendLinkReconcileResult, String> {
    let mode = args.mode.unwrap_or_else(|| "manual".to_string());
    reconcile_internal(&app, &args.instance_id, &mode)
}

#[tauri::command]
pub fn resolve_friend_link_conflicts(
    app: tauri::AppHandle,
    args: ResolveFriendLinkConflictsArgs,
) -> Result<FriendLinkReconcileResult, String> {
    let mut store = read_store(&app)?;
    let session = get_session_mut(&mut store, &args.instance_id)
        .ok_or_else(|| "Instance is not linked".to_string())?;

    if session.pending_conflicts.is_empty() {
        write_store(&app, &store)?;
        return reconcile_internal(&app, &args.instance_id, "manual");
    }

    let instances_dir = app_instances_dir(&app)?;
    let local = collect_sync_state(&instances_dir, &args.instance_id, &session.allowlist)?;
    let mut lock_map = lock_entry_map(&local.lock_entries);
    let mut config_map = config_file_map(&local.config_files);

    let mut item_resolutions = HashMap::new();
    for item in args.resolution.items {
        item_resolutions.insert(item.conflict_id, item.resolution);
    }

    let mut keep_pending = Vec::new();

    for conflict in session.pending_conflicts.clone() {
        let resolution = item_resolutions
            .get(&conflict.id)
            .cloned()
            .unwrap_or_else(|| {
                if args.resolution.take_all_theirs {
                    "take_theirs".to_string()
                } else if args.resolution.keep_all_mine {
                    "keep_mine".to_string()
                } else {
                    "skip_for_now".to_string()
                }
            });

        if resolution.eq_ignore_ascii_case("skip_for_now") {
            keep_pending.push(conflict.clone());
            continue;
        }

        if resolution.eq_ignore_ascii_case("take_theirs") {
            if conflict.kind == "lock_entry" {
                if let Some(value) = conflict.theirs_value.as_ref() {
                    if let Ok(entry) = serde_json::from_value::<CanonicalLockEntry>(value.clone()) {
                        lock_map.insert(conflict.key.clone(), entry);
                    }
                }
            } else if conflict.kind == "config_file" {
                if let Some(value) = conflict.theirs_value.as_ref() {
                    if let Ok(file) = serde_json::from_value::<ConfigFileState>(value.clone()) {
                        config_map.insert(conflict.key.clone(), file);
                    }
                }
            }
        }
    }

    apply_lock_map(&instances_dir, &args.instance_id, &lock_map)?;
    for file in config_map.values() {
        apply_config_file(&instances_dir, &args.instance_id, file)?;
    }

    session.pending_conflicts = keep_pending;
    write_store(&app, &store)?;

    reconcile_internal(&app, &args.instance_id, "manual")
}

#[tauri::command]
pub fn export_friend_link_debug_bundle(
    app: tauri::AppHandle,
    args: ExportFriendLinkDebugBundleArgs,
) -> Result<FriendLinkDebugBundleResult, String> {
    let store = read_store(&app)?;
    let session = get_session(&store, &args.instance_id)
        .ok_or_else(|| "Instance is not linked".to_string())?;

    let instances_dir = app_instances_dir(&app)?;
    let state = collect_sync_state(&instances_dir, &args.instance_id, &session.allowlist)?;

    let output_dir = app
        .path_resolver()
        .app_data_dir()
        .ok_or_else(|| "Failed to resolve app data dir".to_string())?
        .join("friend_link")
        .join("debug");
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| format!("mkdir friend link debug dir failed: {e}"))?;

    let path = output_dir.join(format!("{}_{}.json", args.instance_id, Uuid::new_v4()));
    let payload = serde_json::json!({
        "instance_id": args.instance_id,
        "session": session,
        "state": state,
        "exported_at": now_iso(),
    });
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&payload)
            .map_err(|e| format!("serialize friend link debug bundle failed: {e}"))?,
    )
    .map_err(|e| format!("write friend link debug bundle failed: {e}"))?;

    Ok(FriendLinkDebugBundleResult {
        path: path.display().to_string(),
    })
}

#[tauri::command]
pub fn list_instance_config_files(
    app: tauri::AppHandle,
    args: ListInstanceConfigFilesArgs,
) -> Result<Vec<InstanceConfigFileEntry>, String> {
    let instances_dir = app_instances_dir(&app)?;
    state::list_instance_config_files(&instances_dir, &args.instance_id)
}

#[tauri::command]
pub fn read_instance_config_file(
    app: tauri::AppHandle,
    args: ReadInstanceConfigFileArgs,
) -> Result<ReadInstanceConfigFileResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    state::read_instance_config_file(&instances_dir, &args.instance_id, &args.path)
}

#[tauri::command]
pub fn write_instance_config_file(
    app: tauri::AppHandle,
    args: WriteInstanceConfigFileArgs,
) -> Result<WriteInstanceConfigFileResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    state::write_instance_config_file(
        &instances_dir,
        &args.instance_id,
        &args.path,
        &args.content,
        args.expected_modified_at,
    )
}
