use crate::friend_link::state::{safe_join_under, SyncState};
#[cfg(not(test))]
use keyring::{Entry as KeyringEntry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

const STORE_DIR: &str = "friend_link";
const STORE_FILE: &str = "store.v1.json";
const FRIEND_LINK_SECRET_SERVICE: &str = "ModpackManager";
const FRIEND_LINK_LEGACY_SECRET_SERVICES: [&str; 3] = [
    "com.adrien.modpackmanager",
    "modpack-manager",
    "OpenJar Launcher",
];
const FRIEND_LINK_SECRET_KEY_PREFIX: &str = "friend_link_secret_v1_";
const FRIEND_LINK_SIGNING_KEY_PREFIX: &str = "friend_link_signing_v1_";

fn runtime_secret_cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn runtime_secret_cache_set(key_id: &str, secret: &str) {
    if let Ok(mut guard) = runtime_secret_cache().lock() {
        guard.insert(key_id.to_string(), secret.to_string());
    }
}

fn runtime_secret_cache_get(key_id: &str) -> Option<String> {
    runtime_secret_cache()
        .lock()
        .ok()
        .and_then(|guard| guard.get(key_id).cloned())
}

fn runtime_secret_cache_delete(key_id: &str) {
    if let Ok(mut guard) = runtime_secret_cache().lock() {
        guard.remove(key_id);
    }
}

fn store_write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

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
pub struct FriendInvitePolicyRecord {
    pub invite_version: u32,
    pub max_uses: u32,
    pub expires_at: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendInviteUsageRecord {
    #[serde(default)]
    pub used_count: u32,
    #[serde(default)]
    pub used_at: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkSessionRecord {
    pub instance_id: String,
    pub group_id: String,
    pub local_peer_id: String,
    pub display_name: String,
    #[serde(default)]
    pub shared_secret_key_id: String,
    #[serde(default, skip_serializing)]
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
    #[serde(default)]
    pub bootstrap_host_peer_id: Option<String>,
    #[serde(default)]
    pub trusted_peer_ids: Vec<String>,
    #[serde(default)]
    pub trusted_peer_ids_initialized: bool,
    #[serde(default)]
    pub guardrails_updated_at_ms: i64,
    #[serde(default)]
    pub peer_aliases: HashMap<String, String>,
    #[serde(default)]
    pub allow_loopback_endpoints: bool,
    #[serde(default)]
    pub allow_internet_endpoints: bool,
    #[serde(default = "default_friend_link_max_auto_changes")]
    pub max_auto_changes: usize,
    #[serde(default = "default_sync_mods")]
    pub sync_mods: bool,
    #[serde(default = "default_sync_resourcepacks")]
    pub sync_resourcepacks: bool,
    #[serde(default = "default_sync_shaderpacks")]
    pub sync_shaderpacks: bool,
    #[serde(default = "default_sync_datapacks")]
    pub sync_datapacks: bool,
    #[serde(default)]
    pub allow_upnp_endpoints: bool,
    #[serde(default)]
    pub public_endpoint_override: Option<String>,
    #[serde(default)]
    pub local_signing_key_id: String,
    #[serde(default, skip_serializing)]
    pub local_signing_private_b64: String,
    #[serde(default)]
    pub local_signing_public_key_b64: String,
    #[serde(default)]
    pub peer_signing_public_keys: HashMap<String, String>,
    #[serde(default)]
    pub invite_policies: HashMap<String, FriendInvitePolicyRecord>,
    #[serde(default)]
    pub invite_usage: HashMap<String, FriendInviteUsageRecord>,
}

fn default_friend_link_max_auto_changes() -> usize {
    25
}

fn default_sync_mods() -> bool {
    true
}

fn default_sync_resourcepacks() -> bool {
    false
}

fn default_sync_shaderpacks() -> bool {
    true
}

fn default_sync_datapacks() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLinkStoreV1 {
    pub version: u32,
    #[serde(default = "default_store_revision")]
    pub revision: u64,
    #[serde(default)]
    pub sessions: Vec<FriendLinkSessionRecord>,
}

fn default_store_revision() -> u64 {
    1
}

impl Default for FriendLinkStoreV1 {
    fn default() -> Self {
        Self {
            version: 1,
            revision: default_store_revision(),
            sessions: vec![],
        }
    }
}

#[cfg(not(test))]
fn friend_link_keyring_error(operation: &str, error: &KeyringError) -> String {
    match error {
        #[cfg(target_os = "linux")]
        KeyringError::NoStorageAccess(_) | KeyringError::PlatformFailure(_) => format!(
            "{operation} failed: OS keyring is unavailable. Install/unlock Secret Service (gnome-keyring or KWallet) and restart OpenJar Launcher. ({error})"
        ),
        _ => format!("{operation} failed: {error}"),
    }
}

#[cfg(test)]
fn test_friend_link_secret_store() -> &'static Mutex<HashMap<(String, String), String>> {
    use std::sync::OnceLock;
    static STORE: OnceLock<Mutex<HashMap<(String, String), String>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn secret_store_set_for_service(service: &str, username: &str, secret: &str) -> Result<(), String> {
    let mut guard = test_friend_link_secret_store()
        .lock()
        .map_err(|_| "test friend link keyring lock failed".to_string())?;
    guard.insert(
        (service.to_string(), username.to_string()),
        secret.to_string(),
    );
    Ok(())
}

#[cfg(not(test))]
fn secret_store_set_for_service(service: &str, username: &str, secret: &str) -> Result<(), String> {
    let entry = KeyringEntry::new(service, username)
        .map_err(|e| friend_link_keyring_error("friend-link keyring init", &e))?;
    entry
        .set_password(secret)
        .map_err(|e| friend_link_keyring_error("friend-link keyring write", &e))
}

#[cfg(test)]
fn secret_store_get_for_service(service: &str, username: &str) -> Result<Option<String>, String> {
    let guard = test_friend_link_secret_store()
        .lock()
        .map_err(|_| "test friend link keyring lock failed".to_string())?;
    Ok(guard
        .get(&(service.to_string(), username.to_string()))
        .cloned())
}

#[cfg(not(test))]
fn secret_store_get_for_service(service: &str, username: &str) -> Result<Option<String>, String> {
    let entry = KeyringEntry::new(service, username)
        .map_err(|e| friend_link_keyring_error("friend-link keyring init", &e))?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(friend_link_keyring_error("friend-link keyring read", &e)),
    }
}

#[cfg(test)]
fn secret_store_delete_for_service(service: &str, username: &str) -> Result<(), String> {
    let mut guard = test_friend_link_secret_store()
        .lock()
        .map_err(|_| "test friend link keyring lock failed".to_string())?;
    guard.remove(&(service.to_string(), username.to_string()));
    Ok(())
}

#[cfg(not(test))]
fn secret_store_delete_for_service(service: &str, username: &str) -> Result<(), String> {
    let entry = KeyringEntry::new(service, username)
        .map_err(|e| friend_link_keyring_error("friend-link keyring init", &e))?;
    match entry.delete_credential() {
        Ok(_) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(friend_link_keyring_error("friend-link keyring delete", &e)),
    }
}

fn secret_store_set(username: &str, secret: &str) -> Result<(), String> {
    secret_store_set_for_service(FRIEND_LINK_SECRET_SERVICE, username, secret)
}

fn secret_store_get(username: &str) -> Result<Option<String>, String> {
    if let Some(secret) = secret_store_get_for_service(FRIEND_LINK_SECRET_SERVICE, username)? {
        return Ok(Some(secret));
    }
    for service in FRIEND_LINK_LEGACY_SECRET_SERVICES {
        let Some(secret) = secret_store_get_for_service(service, username)? else {
            continue;
        };
        secret_store_set_for_service(FRIEND_LINK_SECRET_SERVICE, username, &secret)?;
        if let Err(err) = secret_store_delete_for_service(service, username) {
            eprintln!(
                "friend-link legacy keyring cleanup failed for migrated secret: {}",
                err
            );
        }
        return Ok(Some(secret));
    }
    Ok(None)
}

fn secret_store_delete(username: &str) -> Result<(), String> {
    secret_store_delete_for_service(FRIEND_LINK_SECRET_SERVICE, username)?;
    for service in FRIEND_LINK_LEGACY_SECRET_SERVICES {
        if let Err(err) = secret_store_delete_for_service(service, username) {
            eprintln!(
                "friend-link legacy keyring cleanup failed while deleting secret: {}",
                err
            );
        }
    }
    Ok(())
}

fn ensure_secret_key_id(session: &mut FriendLinkSessionRecord) {
    if !session.shared_secret_key_id.trim().is_empty() {
        return;
    }
    session.shared_secret_key_id = format!("{}{}", FRIEND_LINK_SECRET_KEY_PREFIX, Uuid::new_v4());
}

fn ensure_signing_key_id(session: &mut FriendLinkSessionRecord) {
    if !session.local_signing_key_id.trim().is_empty() {
        return;
    }
    session.local_signing_key_id = format!("{}{}", FRIEND_LINK_SIGNING_KEY_PREFIX, Uuid::new_v4());
}

#[cfg(test)]
pub fn clear_test_friend_link_secret_store() {
    if let Ok(mut guard) = test_friend_link_secret_store().lock() {
        guard.clear();
    }
    if let Ok(mut guard) = runtime_secret_cache().lock() {
        guard.clear();
    }
}

#[cfg(test)]
pub fn set_test_friend_link_secret_for_service(
    service: &str,
    username: &str,
    secret: &str,
) -> Result<(), String> {
    secret_store_set_for_service(service, username, secret)
}

#[cfg(test)]
pub fn get_test_friend_link_secret_for_service(service: &str, username: &str) -> Option<String> {
    secret_store_get_for_service(service, username)
        .ok()
        .flatten()
}

pub fn set_session_shared_secret(
    session: &mut FriendLinkSessionRecord,
    shared_secret_b64: &str,
) -> Result<(), String> {
    let secret = shared_secret_b64.trim().to_string();
    if secret.is_empty() {
        return Err("friend-link shared secret is empty".to_string());
    }
    ensure_secret_key_id(session);
    secret_store_set(&session.shared_secret_key_id, &secret)?;
    runtime_secret_cache_set(&session.shared_secret_key_id, &secret);
    session.shared_secret_b64 = secret;
    Ok(())
}

pub fn set_session_signing_private_key(
    session: &mut FriendLinkSessionRecord,
    signing_private_key_b64: &str,
) -> Result<(), String> {
    let secret = signing_private_key_b64.trim().to_string();
    if secret.is_empty() {
        return Err("friend-link signing private key is empty".to_string());
    }
    ensure_signing_key_id(session);
    secret_store_set(&session.local_signing_key_id, &secret)?;
    runtime_secret_cache_set(&session.local_signing_key_id, &secret);
    session.local_signing_private_b64 = secret;
    Ok(())
}

pub fn get_session_shared_secret(session: &mut FriendLinkSessionRecord) -> Result<String, String> {
    if !session.shared_secret_b64.trim().is_empty() {
        return Ok(session.shared_secret_b64.clone());
    }
    let key_id = session.shared_secret_key_id.trim();
    if key_id.is_empty() {
        return Err("friend-link shared secret key id is missing".to_string());
    }
    if let Some(secret) = runtime_secret_cache_get(key_id) {
        if !secret.trim().is_empty() {
            session.shared_secret_b64 = secret.clone();
            return Ok(secret);
        }
    }
    let Some(secret) = secret_store_get(key_id)? else {
        return Err("friend-link shared secret not found in secure storage".to_string());
    };
    if secret.trim().is_empty() {
        return Err("friend-link shared secret in secure storage is empty".to_string());
    }
    runtime_secret_cache_set(key_id, &secret);
    session.shared_secret_b64 = secret.clone();
    Ok(secret)
}

pub fn get_session_signing_private_key(
    session: &mut FriendLinkSessionRecord,
) -> Result<String, String> {
    if !session.local_signing_private_b64.trim().is_empty() {
        return Ok(session.local_signing_private_b64.clone());
    }
    let key_id = session.local_signing_key_id.trim();
    if key_id.is_empty() {
        return Err("friend-link signing key id is missing".to_string());
    }
    if let Some(secret) = runtime_secret_cache_get(key_id) {
        if !secret.trim().is_empty() {
            session.local_signing_private_b64 = secret.clone();
            return Ok(secret);
        }
    }
    let Some(secret) = secret_store_get(key_id)? else {
        return Err("friend-link signing private key not found in secure storage".to_string());
    };
    if secret.trim().is_empty() {
        return Err("friend-link signing private key in secure storage is empty".to_string());
    }
    runtime_secret_cache_set(key_id, &secret);
    session.local_signing_private_b64 = secret.clone();
    Ok(secret)
}

pub fn delete_session_shared_secret(session: &FriendLinkSessionRecord) -> Result<(), String> {
    let key_id = session.shared_secret_key_id.trim();
    if key_id.is_empty() {
        return Ok(());
    }
    runtime_secret_cache_delete(key_id);
    secret_store_delete(key_id)
}

pub fn delete_session_signing_private_key(session: &FriendLinkSessionRecord) -> Result<(), String> {
    let key_id = session.local_signing_key_id.trim();
    if key_id.is_empty() {
        return Ok(());
    }
    runtime_secret_cache_delete(key_id);
    secret_store_delete(key_id)
}

fn hydrate_and_migrate_session_secret(session: &mut FriendLinkSessionRecord) -> Result<(), String> {
    if !session.shared_secret_b64.trim().is_empty() {
        set_session_shared_secret(session, &session.shared_secret_b64.clone())?;
        session.shared_secret_b64.clear();
    }
    Ok(())
}

fn hydrate_and_migrate_session_signing_key(
    session: &mut FriendLinkSessionRecord,
) -> Result<(), String> {
    if !session.local_signing_private_b64.trim().is_empty() {
        set_session_signing_private_key(session, &session.local_signing_private_b64.clone())?;
        session.local_signing_private_b64.clear();
    }
    Ok(())
}

pub fn store_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path_resolver()
        .app_data_dir()
        .ok_or_else(|| "cannot resolve app data dir".to_string())?;
    let dir = safe_join_under(&base, STORE_DIR)?;
    safe_join_under(&dir, STORE_FILE)
}

pub fn store_path_from_app_data(app_data_dir: &Path) -> PathBuf {
    safe_join_under(app_data_dir, STORE_DIR)
        .and_then(|dir| safe_join_under(&dir, STORE_FILE))
        .expect("friend-link store path constants must be safe")
}

pub fn read_store(app: &tauri::AppHandle) -> Result<FriendLinkStoreV1, String> {
    let path = store_path(app)?;
    read_store_at_path(&path)
}

pub fn read_store_at_path(path: &Path) -> Result<FriendLinkStoreV1, String> {
    if !path.exists() {
        return Ok(FriendLinkStoreV1::default());
    }
    let raw =
        fs::read_to_string(path).map_err(|e| format!("read friend link store failed: {e}"))?;
    let mut store: FriendLinkStoreV1 =
        serde_json::from_str(&raw).map_err(|e| format!("parse friend link store failed: {e}"))?;
    if store.version == 0 {
        store.version = 1;
    }
    if store.revision == 0 {
        store.revision = default_store_revision();
    }
    let mut migrated_legacy_secrets = 0usize;
    for session in &mut store.sessions {
        let had_legacy_secret = !session.shared_secret_b64.trim().is_empty();
        hydrate_and_migrate_session_secret(session)?;
        hydrate_and_migrate_session_signing_key(session)?;
        if had_legacy_secret {
            migrated_legacy_secrets += 1;
        }
    }
    if migrated_legacy_secrets > 0 {
        write_store_at_path(path, &store)?;
        eprintln!(
            "migrated {} friend-link shared secret(s) to OS secure storage and rewrote store.v1.json",
            migrated_legacy_secrets
        );
    }
    Ok(store)
}

pub fn write_store(app: &tauri::AppHandle, store: &FriendLinkStoreV1) -> Result<(), String> {
    let path = store_path(app)?;
    write_store_at_path(&path, store)
}

fn merge_session_runtime_fields(
    next: &mut FriendLinkSessionRecord,
    current: &FriendLinkSessionRecord,
    stale_snapshot: bool,
) {
    if next.group_id != current.group_id || next.local_peer_id != current.local_peer_id {
        if stale_snapshot {
            *next = current.clone();
        }
        return;
    }

    if next.shared_secret_key_id.trim().is_empty() {
        next.shared_secret_key_id = current.shared_secret_key_id.clone();
    }
    if next.local_signing_key_id.trim().is_empty() {
        next.local_signing_key_id = current.local_signing_key_id.clone();
    }
    if next.local_signing_public_key_b64.trim().is_empty() {
        next.local_signing_public_key_b64 = current.local_signing_public_key_b64.clone();
    }
    if next.listener_port == 0 {
        next.listener_port = current.listener_port;
    }
    if next.listener_endpoint.is_none() {
        next.listener_endpoint = current.listener_endpoint.clone();
    }
    if next.bootstrap_host_peer_id.is_none() {
        next.bootstrap_host_peer_id = current.bootstrap_host_peer_id.clone();
    }

    for peer in &current.peers {
        if let Some(found) = next.peers.iter_mut().find(|v| v.peer_id == peer.peer_id) {
            if found.display_name.trim().is_empty() {
                found.display_name = peer.display_name.clone();
            }
            if found.endpoint.trim().is_empty() {
                found.endpoint = peer.endpoint.clone();
            }
            if found.last_seen_at.is_none() {
                found.last_seen_at = peer.last_seen_at.clone();
            }
            if !found.online && peer.online {
                found.online = true;
            }
            if found.last_state_hash.is_none() {
                found.last_state_hash = peer.last_state_hash.clone();
            }
        } else {
            next.peers.push(peer.clone());
        }
    }

    for (peer_id, last_sync_ms) in &current.last_peer_sync_at {
        let existing = next
            .last_peer_sync_at
            .get(peer_id)
            .copied()
            .unwrap_or_default();
        if *last_sync_ms > existing {
            next.last_peer_sync_at
                .insert(peer_id.clone(), *last_sync_ms);
        }
    }

    for (peer_id, state) in &current.cached_peer_state {
        next.cached_peer_state
            .entry(peer_id.clone())
            .or_insert_with(|| state.clone());
    }
    for (peer_id, public_key_b64) in &current.peer_signing_public_keys {
        next.peer_signing_public_keys
            .entry(peer_id.clone())
            .or_insert_with(|| public_key_b64.clone());
    }
    for (invite_id, policy) in &current.invite_policies {
        next.invite_policies
            .entry(invite_id.clone())
            .or_insert_with(|| policy.clone());
    }
    for (invite_id, usage) in &current.invite_usage {
        if let Some(existing) = next.invite_usage.get_mut(invite_id) {
            if usage.used_count > existing.used_count {
                *existing = usage.clone();
            }
        } else {
            next.invite_usage.insert(invite_id.clone(), usage.clone());
        }
    }

    if next.last_good_snapshot.is_none() {
        next.last_good_snapshot = current.last_good_snapshot.clone();
    }
    if next.pending_conflicts.is_empty() && !current.pending_conflicts.is_empty() {
        next.pending_conflicts = current.pending_conflicts.clone();
    }
    let next_guardrails_ts = next.guardrails_updated_at_ms.max(0);
    let current_guardrails_ts = current.guardrails_updated_at_ms.max(0);
    if current_guardrails_ts > next_guardrails_ts {
        next.trusted_peer_ids = current.trusted_peer_ids.clone();
        next.trusted_peer_ids_initialized = current.trusted_peer_ids_initialized;
        next.max_auto_changes = current.max_auto_changes;
        next.sync_mods = current.sync_mods;
        next.sync_resourcepacks = current.sync_resourcepacks;
        next.sync_shaderpacks = current.sync_shaderpacks;
        next.sync_datapacks = current.sync_datapacks;
        next.guardrails_updated_at_ms = current.guardrails_updated_at_ms;
    } else if !next.trusted_peer_ids_initialized && current.trusted_peer_ids_initialized {
        next.trusted_peer_ids_initialized = true;
        if next.trusted_peer_ids.is_empty() {
            next.trusted_peer_ids = current.trusted_peer_ids.clone();
        }
    }
    if next.peer_aliases.is_empty() && !current.peer_aliases.is_empty() {
        next.peer_aliases = current.peer_aliases.clone();
    } else {
        for (peer_id, alias) in &current.peer_aliases {
            next.peer_aliases
                .entry(peer_id.clone())
                .or_insert_with(|| alias.clone());
        }
    }
}

fn read_current_store_for_merge(path: &Path) -> Option<FriendLinkStoreV1> {
    if !path.exists() {
        return None;
    }
    let raw = fs::read_to_string(path).ok()?;
    let mut current = serde_json::from_str::<FriendLinkStoreV1>(&raw).ok()?;
    if current.version == 0 {
        current.version = 1;
    }
    if current.revision == 0 {
        current.revision = default_store_revision();
    }
    Some(current)
}

fn merge_store_with_current_disk_state(
    next: &mut FriendLinkStoreV1,
    current: &FriendLinkStoreV1,
    stale_snapshot: bool,
) {
    for session in &mut next.sessions {
        if let Some(existing) = current
            .sessions
            .iter()
            .find(|v| v.instance_id == session.instance_id)
        {
            merge_session_runtime_fields(session, existing, stale_snapshot);
        }
    }
    if stale_snapshot {
        for existing in &current.sessions {
            if !next
                .sessions
                .iter()
                .any(|session| session.instance_id == existing.instance_id)
            {
                next.sessions.push(existing.clone());
            }
        }
    }
}

pub fn write_store_at_path(path: &Path, store: &FriendLinkStoreV1) -> Result<(), String> {
    let _guard = store_write_lock()
        .lock()
        .map_err(|_| "friend-link store lock poisoned".to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("mkdir friend link store dir failed: {e}"))?;
    }
    let mut next = store.clone();
    next.version = 1;
    let current = read_current_store_for_merge(path);
    let current_revision = current
        .as_ref()
        .map(|value| value.revision)
        .unwrap_or_default();
    if next.revision == 0 {
        next.revision = current_revision;
    }
    let stale_snapshot = next.revision < current_revision;
    next.revision = current_revision.saturating_add(1);
    for session in &mut next.sessions {
        if !session.shared_secret_b64.trim().is_empty() {
            set_session_shared_secret(session, &session.shared_secret_b64.clone())?;
        }
        session.shared_secret_b64.clear();
        if !session.local_signing_private_b64.trim().is_empty() {
            set_session_signing_private_key(session, &session.local_signing_private_b64.clone())?;
        }
        session.local_signing_private_b64.clear();
    }
    if let Some(current_store) = current.as_ref() {
        merge_store_with_current_disk_state(&mut next, current_store, stale_snapshot);
    }
    let raw = serde_json::to_string_pretty(&next)
        .map_err(|e| format!("serialize friend link store failed: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("write friend link store failed: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("set friend link store permissions failed: {e}"))?;
    }
    Ok(())
}

pub fn get_session(
    store: &FriendLinkStoreV1,
    instance_id: &str,
) -> Option<FriendLinkSessionRecord> {
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
    store
        .sessions
        .iter_mut()
        .find(|s| s.instance_id == instance_id)
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
