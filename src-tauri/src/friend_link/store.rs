use crate::friend_link::state::SyncState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const STORE_DIR: &str = "friend_link";
const STORE_FILE: &str = "store.v1.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendManifestEntry {
    pub key: String,
    pub hash: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLastGoodSnapshot {
    pub state_hash: String,
    #[serde(default)]
    pub manifest: Vec<FriendManifestEntry>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendPeerRecord {
    pub peer_id: String,
    pub display_name: String,
    pub endpoint: String,
    pub added_at: String,
    #[serde(default)]
    pub last_seen_at: Option<String>,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub last_state_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendSyncConflictRecord {
    pub id: String,
    pub kind: String,
    pub key: String,
    pub peer_id: String,
    pub mine_hash: String,
    pub theirs_hash: String,
    #[serde(default)]
    pub mine_preview: Option<String>,
    #[serde(default)]
    pub theirs_preview: Option<String>,
    #[serde(default)]
    pub mine_value: Option<serde_json::Value>,
    #[serde(default)]
    pub theirs_value: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkSessionRecord {
    pub instance_id: String,
    pub group_id: String,
    pub local_peer_id: String,
    pub display_name: String,
    pub shared_secret_b64: String,
    #[serde(default)]
    pub protocol_version: u32,
    #[serde(default)]
    pub listener_port: u16,
    #[serde(default)]
    pub listener_endpoint: Option<String>,
    #[serde(default)]
    pub peers: Vec<FriendPeerRecord>,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub last_peer_sync_at: HashMap<String, i64>,
    #[serde(default)]
    pub last_good_snapshot: Option<FriendLastGoodSnapshot>,
    #[serde(default)]
    pub pending_conflicts: Vec<FriendSyncConflictRecord>,
    #[serde(default)]
    pub cached_peer_state: HashMap<String, SyncState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkStoreV1 {
    pub version: u32,
    #[serde(default)]
    pub sessions: Vec<FriendLinkSessionRecord>,
}

impl Default for FriendLinkStoreV1 {
    fn default() -> Self {
        Self {
            version: 1,
            sessions: vec![],
        }
    }
}

pub fn store_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path_resolver()
        .app_data_dir()
        .ok_or_else(|| "cannot resolve app data dir".to_string())?;
    Ok(base.join(STORE_DIR).join(STORE_FILE))
}

pub fn store_path_from_app_data(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(STORE_DIR).join(STORE_FILE)
}

pub fn read_store(app: &tauri::AppHandle) -> Result<FriendLinkStoreV1, String> {
    let path = store_path(app)?;
    read_store_at_path(&path)
}

pub fn read_store_at_path(path: &Path) -> Result<FriendLinkStoreV1, String> {
    if !path.exists() {
        return Ok(FriendLinkStoreV1::default());
    }
    let raw = fs::read_to_string(path).map_err(|e| format!("read friend link store failed: {e}"))?;
    let mut store: FriendLinkStoreV1 =
        serde_json::from_str(&raw).map_err(|e| format!("parse friend link store failed: {e}"))?;
    if store.version == 0 {
        store.version = 1;
    }
    Ok(store)
}

pub fn write_store(app: &tauri::AppHandle, store: &FriendLinkStoreV1) -> Result<(), String> {
    let path = store_path(app)?;
    write_store_at_path(&path, store)
}

pub fn write_store_at_path(path: &Path, store: &FriendLinkStoreV1) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir friend link store dir failed: {e}"))?;
    }
    let mut next = store.clone();
    next.version = 1;
    let raw = serde_json::to_string_pretty(&next)
        .map_err(|e| format!("serialize friend link store failed: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("write friend link store failed: {e}"))
}

pub fn get_session(store: &FriendLinkStoreV1, instance_id: &str) -> Option<FriendLinkSessionRecord> {
    store
        .sessions
        .iter()
        .find(|s| s.instance_id == instance_id)
        .cloned()
}

pub fn get_session_mut<'a>(
    store: &'a mut FriendLinkStoreV1,
    instance_id: &str,
) -> Option<&'a mut FriendLinkSessionRecord> {
    store.sessions.iter_mut().find(|s| s.instance_id == instance_id)
}

pub fn upsert_session(store: &mut FriendLinkStoreV1, session: FriendLinkSessionRecord) {
    if let Some(found) = store
        .sessions
        .iter_mut()
        .find(|s| s.instance_id == session.instance_id)
    {
        *found = session;
    } else {
        store.sessions.push(session);
    }
}

pub fn remove_session(store: &mut FriendLinkStoreV1, instance_id: &str) -> bool {
    let before = store.sessions.len();
    store.sessions.retain(|s| s.instance_id != instance_id);
    store.sessions.len() < before
}
