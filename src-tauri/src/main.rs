#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine as _;
use chrono::{DateTime, Local, Utc};
#[cfg(not(test))]
use keyring::{Entry as KeyringEntry, Error as KeyringError};
use open_launcher::{auth as ol_auth, version as ol_version, Launcher as OpenLauncher};
use reqwest::blocking::{multipart, Client, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use tauri::Manager;
use tauri::{CustomMenuItem, Menu, MenuItem, Submenu};
use uuid::Uuid;
use zip::write::FileOptions;
use zip::ZipArchive;

mod commands;
mod friend_link;
mod modpack;
mod permissions;
pub(crate) mod run_reports;
pub(crate) use commands::import_provider_modpack_template;

const USER_AGENT: &str = "OpenJarLauncher/0.1.6 (Tauri)";
const KEYRING_SERVICE: &str = "OpenJar Launcher";
const KEYRING_SELECTED_REFRESH_ALIAS: &str = "msa_refresh_selected";
const LEGACY_KEYRING_SERVICES: [&str; 5] = [
    "ModpackManager",
    "com.adrien.modpackmanager",
    "modpack-manager",
    "openjar-launcher",
    "OpenJar",
];
const DEV_CURSEFORGE_KEY_KEYRING_USER: &str = "dev_curseforge_api_key";
const GITHUB_TOKEN_POOL_KEYRING_USER: &str = "github_api_tokens";
const LAUNCHER_TOKEN_FALLBACK_FILE: &str = "tokens_fallback.json";
const LAUNCHER_TOKEN_RECOVERY_FALLBACK_FILE: &str = "tokens_recovery_fallback.json";
#[cfg(debug_assertions)]
const LAUNCHER_TOKEN_DEBUG_FALLBACK_FILE: &str = "tokens_debug_fallback.json";
const MS_TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const MS_DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const GITHUB_API_BASE: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2022-11-28";
const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_AUTH_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_LAUNCHER_AUTH_URL: &str = "https://api.minecraftservices.com/launcher/login";
const MC_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const DEFAULT_MS_PUBLIC_CLIENT_ID: &str = "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb";
const CURSEFORGE_API_BASE: &str = "https://api.curseforge.com/v1";
const CURSEFORGE_GAME_ID_MINECRAFT: i64 = 432;
const BUILT_IN_CURSEFORGE_API_KEY_ENV: &str = "MPM_CURSEFORGE_API_KEY_BUILTIN";
const BUILT_IN_CURSEFORGE_API_KEY: Option<&str> = option_env!("MPM_CURSEFORGE_API_KEY_BUILTIN");
const DEV_RUNTIME_CURSEFORGE_API_KEY_ENV: &str = "MPM_CURSEFORGE_API_KEY_DEV_RUNTIME";
const MAX_LOCAL_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_WORLD_BACKUP_INTERVAL_MINUTES: u32 = 10;
const DEFAULT_WORLD_BACKUP_RETENTION_COUNT: u32 = 1;
const DEFAULT_SNAPSHOT_RETENTION_COUNT: u32 = 5;
const DEFAULT_SNAPSHOT_MAX_AGE_DAYS: u32 = 14;
const MENU_CHECK_FOR_UPDATES_ID: &str = "menu_check_for_updates";
const APP_MENU_CHECK_FOR_UPDATES_EVENT: &str = "app_menu_check_for_updates";
const UPDATE_ENTRY_WORKERS_MAX_ENV: &str = "MPM_UPDATE_ENTRY_WORKERS_MAX";
const UPDATE_PREFETCH_WORKERS_MAX_ENV: &str = "MPM_UPDATE_PREFETCH_WORKERS_MAX";
const CURSEFORGE_RESOLVE_WORKERS_MAX_ENV: &str = "MPM_CURSEFORGE_RESOLVE_WORKERS_MAX";
pub(crate) const LOCAL_JAR_IMPORT_WORKERS_MAX_ENV: &str = "MPM_LOCAL_JAR_IMPORT_WORKERS_MAX";
const INSTANCE_LAST_RUN_METADATA_FILE: &str = "last_run_metadata.v1.json";
const PLAY_SESSIONS_STORE_FILE: &str = "play_sessions.v1.json";
const PLAY_SESSIONS_ACTIVE_STORE_FILE: &str = "play_sessions_active.v1.json";
const MAX_PLAY_SESSION_HISTORY: usize = 500;
const QUICK_PLAY_SERVERS_FILE: &str = "quick_play_servers.v1.json";
const RUNTIME_RECONCILE_MARKER_FILE: &str = ".runtime_reconcile.v1.done";
const RUNTIME_SESSION_ACTIVE_MARKER_FILE: &str = ".active_session.v1";
const STALE_RUNTIME_SESSION_MAX_AGE_HOURS: u64 = 24;
const GITHUB_DISCOVER_LOW_HITS_THRESHOLD: usize = 4;
const GITHUB_DISCOVER_MAX_REPO_CANDIDATES: usize = 12;
const GITHUB_DISCOVER_SOURCE_MAX_REPO_CANDIDATES: usize = 180;
const GITHUB_DISCOVER_RESULTS_BUFFER: usize = 8;
const GITHUB_DISCOVER_MIN_RESULT_POOL: usize = 60;
const GITHUB_DISCOVER_MAX_RELEASE_FETCHES: usize = 36;
const GITHUB_DISCOVER_NONSTRICT_RELEASE_FETCHES: usize = 18;
const GITHUB_REPO_SEARCH_MAX_PAGES_PER_QUERY: usize = 4;
const GITHUB_REPO_SEARCH_PER_PAGE_MAX: usize = 30;
const GITHUB_RELEASES_PER_PAGE: usize = 25;
const GITHUB_REPO_TREE_PATH_SCAN_LIMIT: usize = 4500;
const GITHUB_LOCAL_IDENTIFY_MAX_QUERY_HINTS: usize = 6;
const GITHUB_LOCAL_IDENTIFY_MAX_REPO_CANDIDATES: usize = 28;
const GITHUB_LOCAL_IDENTIFY_MAX_RELEASE_FETCHES: usize = 10;
const GITHUB_LOCAL_IDENTIFY_UNAUTH_MAX_RELEASE_FETCHES: usize = 4;
const GITHUB_API_CACHE_TTL_SECS: u64 = 120;
const GITHUB_API_CACHE_MAX_ENTRIES: usize = 256;
const GITHUB_TOKEN_UNAUTHORIZED_COOLDOWN_SECS: u64 = 30 * 60;
const GITHUB_TOKEN_RATE_LIMIT_FALLBACK_COOLDOWN_SECS: u64 = 90;
const GITHUB_TOKEN_POOL_CACHE_TTL_SECS: u64 = 60;
const GITHUB_DISCOVER_MIN_SIMILARITY_WITHOUT_SIGNAL: i64 = 24;
const GITHUB_UNAUTH_MAX_SEARCH_QUERIES: usize = 3;
const GITHUB_UNAUTH_MAX_PAGES_PER_QUERY: usize = 1;
const GITHUB_UNAUTH_NONSTRICT_RELEASE_FETCH_BUDGET: usize = 0;
const GITHUB_UNAUTH_STRICT_RELEASE_FETCH_BUDGET: usize = 2;
// Hard cap is intentionally high enough for team token pools, but bounded to avoid
// unbounded env parsing/rotation overhead and accidental gigantic env payloads.
const GITHUB_API_TOKENS_MAX: usize = 200;
const GITHUB_LOW_SIGNAL_HIGH_SIMILARITY_THRESHOLD: i64 = 34;

fn runtime_refresh_token_cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone)]
struct GithubApiCacheEntry {
    body: String,
    fetched_at: Instant,
}

fn github_api_response_cache() -> &'static Mutex<HashMap<String, GithubApiCacheEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<String, GithubApiCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn github_api_cache_get(url: &str) -> Option<String> {
    let ttl = Duration::from_secs(GITHUB_API_CACHE_TTL_SECS);
    let mut guard = github_api_response_cache().lock().ok()?;
    let entry = guard.get(url)?.clone();
    if entry.fetched_at.elapsed() > ttl {
        guard.remove(url);
        return None;
    }
    Some(entry.body)
}

fn github_api_cache_put(url: &str, body: String) {
    if body.trim().is_empty() {
        return;
    }
    if let Ok(mut guard) = github_api_response_cache().lock() {
        if guard.len() >= GITHUB_API_CACHE_MAX_ENTRIES {
            if let Some(oldest_key) = guard
                .iter()
                .min_by_key(|(_, entry)| entry.fetched_at)
                .map(|(key, _)| key.clone())
            {
                guard.remove(&oldest_key);
            }
        }
        guard.insert(
            url.to_string(),
            GithubApiCacheEntry {
                body,
                fetched_at: Instant::now(),
            },
        );
    }
}

#[derive(Debug, Default)]
struct GithubTokenRotationState {
    next_start_index: usize,
    cooldown_until: HashMap<String, Instant>,
    unauth_cooldown_until: Option<Instant>,
    unauth_reset_local: Option<String>,
}

fn github_token_rotation_state() -> &'static Mutex<GithubTokenRotationState> {
    static STATE: OnceLock<Mutex<GithubTokenRotationState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(GithubTokenRotationState::default()))
}

#[derive(Debug, Clone)]
struct GithubTokenPoolSnapshot {
    tokens: Vec<String>,
    env_tokens: usize,
    keychain_tokens: usize,
    keychain_error: Option<String>,
    fetched_at: Instant,
}

fn github_token_pool_cache() -> &'static Mutex<Option<GithubTokenPoolSnapshot>> {
    static CACHE: OnceLock<Mutex<Option<GithubTokenPoolSnapshot>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn runtime_refresh_token_cache_set(account_id: &str, refresh_token: &str) {
    if let Ok(mut guard) = runtime_refresh_token_cache().lock() {
        guard.insert(account_id.trim().to_string(), refresh_token.to_string());
    }
}

fn runtime_refresh_token_cache_get(account_id: &str) -> Option<String> {
    runtime_refresh_token_cache()
        .lock()
        .ok()
        .and_then(|guard| guard.get(account_id.trim()).cloned())
}

fn runtime_refresh_token_cache_delete(account_id: &str) {
    if let Ok(mut guard) = runtime_refresh_token_cache().lock() {
        guard.remove(account_id.trim());
    }
}

#[cfg(test)]
fn runtime_refresh_token_cache_clear() {
    if let Ok(mut guard) = runtime_refresh_token_cache().lock() {
        guard.clear();
    }
}

async fn run_blocking_task<T, F>(label: &str, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| format!("{label} task join failed: {e}"))?
}

fn updater_check_menu_item() -> CustomMenuItem {
    CustomMenuItem::new(
        MENU_CHECK_FOR_UPDATES_ID.to_string(),
        "Check for Updates...",
    )
}

#[cfg(target_os = "macos")]
fn build_main_menu(app_name: &str) -> Menu {
    let app_submenu = Submenu::new(
        app_name,
        Menu::new()
            .add_native_item(MenuItem::About(
                app_name.to_string(),
                tauri::AboutMetadata::default(),
            ))
            .add_native_item(MenuItem::Separator)
            .add_item(updater_check_menu_item())
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::Services)
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::Hide)
            .add_native_item(MenuItem::HideOthers)
            .add_native_item(MenuItem::ShowAll)
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::Quit),
    );

    let file_submenu = Submenu::new("File", Menu::new().add_native_item(MenuItem::CloseWindow));
    let edit_submenu = Submenu::new(
        "Edit",
        Menu::new()
            .add_native_item(MenuItem::Undo)
            .add_native_item(MenuItem::Redo)
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::Cut)
            .add_native_item(MenuItem::Copy)
            .add_native_item(MenuItem::Paste)
            .add_native_item(MenuItem::SelectAll),
    );
    let view_submenu = Submenu::new(
        "View",
        Menu::new().add_native_item(MenuItem::EnterFullScreen),
    );
    let window_submenu = Submenu::new(
        "Window",
        Menu::new()
            .add_native_item(MenuItem::Minimize)
            .add_native_item(MenuItem::Zoom)
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::CloseWindow),
    );
    Menu::new()
        .add_submenu(app_submenu)
        .add_submenu(file_submenu)
        .add_submenu(edit_submenu)
        .add_submenu(view_submenu)
        .add_submenu(window_submenu)
}

#[cfg(not(target_os = "macos"))]
fn build_main_menu(app_name: &str) -> Menu {
    Menu::os_default(app_name)
}

fn modrinth_api_base() -> String {
    std::env::var("MPM_MODRINTH_API_BASE")
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "https://api.modrinth.com/v2".to_string())
}

fn github_append_token_candidate(tokens: &mut Vec<String>, candidate: &str) {
    if tokens.len() >= GITHUB_API_TOKENS_MAX {
        return;
    }
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return;
    }
    if !tokens.iter().any(|existing| existing == trimmed) {
        tokens.push(trimmed.to_string());
    }
}

fn github_parse_token_pool(raw: &str, tokens: &mut Vec<String>) {
    for candidate in raw
        .split(|ch| ch == ',' || ch == ';' || ch == '\n')
        .map(|part| part.trim())
    {
        github_append_token_candidate(tokens, candidate);
    }
}

fn github_api_tokens_from_env_entries(entries: &[(String, String)]) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    if let Some((_, raw)) = entries.iter().find(|(key, _)| key == "MPM_GITHUB_TOKENS") {
        github_parse_token_pool(raw, &mut tokens);
    }

    let mut numbered_tokens: Vec<(u32, String, String)> = Vec::new();
    let numbered_prefixes = ["MPM_GITHUB_TOKEN_", "GITHUB_TOKEN_", "GH_TOKEN_"];
    for (key, value) in entries.iter() {
        for prefix in numbered_prefixes {
            if let Some(suffix) = key.strip_prefix(prefix) {
                let order = suffix.trim().parse::<u32>().unwrap_or(u32::MAX);
                numbered_tokens.push((order, key.clone(), value.clone()));
                break;
            }
        }
    }
    numbered_tokens.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    for (_, _, value) in numbered_tokens.into_iter() {
        github_append_token_candidate(&mut tokens, &value);
    }

    for key in ["MPM_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
        if let Some((_, value)) = entries.iter().find(|(candidate, _)| candidate == key) {
            github_append_token_candidate(&mut tokens, value);
        }
    }
    tokens
}

fn github_keyring_error_is_unavailable(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("secure credential storage is unavailable")
        || lower.contains("no os keyring provider is available")
}

fn github_invalidate_token_pool_cache() {
    if let Ok(mut guard) = github_token_pool_cache().lock() {
        *guard = None;
    }
}

fn github_token_pool_snapshot_from_runtime() -> GithubTokenPoolSnapshot {
    let entries: Vec<(String, String)> = std::env::vars().collect();
    let env_tokens = github_api_tokens_from_env_entries(&entries);
    let (keychain_tokens, keychain_error) = match keyring_get_github_token_pool() {
        Ok(tokens) => (tokens, None),
        Err(err) => (vec![], Some(err)),
    };

    let mut merged = Vec::new();
    for token in &env_tokens {
        github_append_token_candidate(&mut merged, token);
    }
    for token in &keychain_tokens {
        github_append_token_candidate(&mut merged, token);
    }

    GithubTokenPoolSnapshot {
        tokens: merged,
        env_tokens: env_tokens.len(),
        keychain_tokens: keychain_tokens.len(),
        keychain_error,
        fetched_at: Instant::now(),
    }
}

fn github_token_pool_snapshot() -> GithubTokenPoolSnapshot {
    let ttl = Duration::from_secs(GITHUB_TOKEN_POOL_CACHE_TTL_SECS);
    if let Ok(mut guard) = github_token_pool_cache().lock() {
        if let Some(snapshot) = guard.as_ref() {
            if snapshot.fetched_at.elapsed() <= ttl {
                return snapshot.clone();
            }
        }
        let next = github_token_pool_snapshot_from_runtime();
        *guard = Some(next.clone());
        return next;
    }
    github_token_pool_snapshot_from_runtime()
}

fn github_api_tokens() -> Vec<String> {
    github_token_pool_snapshot().tokens
}

pub(crate) fn github_has_configured_tokens() -> bool {
    !github_token_pool_snapshot().tokens.is_empty()
}

fn github_configured_token_count() -> usize {
    github_token_pool_snapshot().tokens.len()
}

pub(crate) fn github_token_pool_status() -> GithubTokenPoolStatus {
    let snapshot = github_token_pool_snapshot();
    let total_tokens = snapshot.tokens.len();
    let keychain_available = snapshot
        .keychain_error
        .as_ref()
        .map(|err| !github_keyring_error_is_unavailable(err))
        .unwrap_or(true);
    let (unauth_rate_limited, unauth_rate_limit_reset_at) = github_unauth_cooldown_state();

    let message = if total_tokens > 0 {
        format!(
            "Loaded {total_tokens} GitHub token(s): {} from environment, {} from secure keychain.",
            snapshot.env_tokens, snapshot.keychain_tokens
        )
    } else if let Some(err) = snapshot.keychain_error.as_ref() {
        if github_keyring_error_is_unavailable(err) {
            "No GitHub tokens configured. Secure keychain is unavailable; configure MPM_GITHUB_TOKENS / MPM_GITHUB_TOKEN / GITHUB_TOKEN / GH_TOKEN."
                .to_string()
        } else {
            format!(
                "No GitHub tokens configured. Secure keychain read warning: {err}. Configure env fallback tokens or retry keychain setup."
            )
        }
    } else {
        "No GitHub tokens configured. Add tokens in Settings > Advanced > GitHub API, or via MPM_GITHUB_TOKENS / MPM_GITHUB_TOKEN / GITHUB_TOKEN / GH_TOKEN."
            .to_string()
    };

    GithubTokenPoolStatus {
        configured: total_tokens > 0,
        total_tokens,
        env_tokens: snapshot.env_tokens,
        keychain_tokens: snapshot.keychain_tokens,
        keychain_available,
        unauth_rate_limited,
        unauth_rate_limit_reset_at,
        message,
    }
}

fn curseforge_api_key() -> Option<String> {
    curseforge_api_key_with_source().map(|(key, _)| key)
}

fn curseforge_api_key_with_source() -> Option<(String, String)> {
    for key in [
        DEV_RUNTIME_CURSEFORGE_API_KEY_ENV,
        "MPM_CURSEFORGE_API_KEY",
        "CURSEFORGE_API_KEY",
    ] {
        if let Ok(v) = std::env::var(key) {
            let trimmed = v.trim().to_string();
            if !trimmed.is_empty() {
                return Some((trimmed, key.to_string()));
            }
        }
    }
    if is_dev_mode_enabled() {
        match keyring_get_dev_curseforge_key() {
            Ok(Some(v)) => return Some((v, "dev:keyring".to_string())),
            Ok(None) => {}
            Err(e) => {
                eprintln!("dev curseforge key read failed: {e}");
            }
        }
    }
    if let Some(v) = BUILT_IN_CURSEFORGE_API_KEY {
        let trimmed = v.trim().to_string();
        if !trimmed.is_empty() {
            return Some((trimmed, format!("build:{BUILT_IN_CURSEFORGE_API_KEY_ENV}")));
        }
    }
    None
}

fn missing_curseforge_key_message() -> String {
    "CurseForge API key is not configured for this build.".to_string()
}

fn discover_missing_curseforge_key_message() -> String {
    "CurseForge is not configured for this build. Release builds can use an injected key; for local dev set MPM_CURSEFORGE_API_KEY and restart."
        .to_string()
}

fn is_dev_mode_enabled() -> bool {
    let raw = std::env::var("MPM_DEV_MODE").unwrap_or_default();
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn load_dev_curseforge_key_into_runtime_env(app: &tauri::AppHandle) {
    if !is_dev_mode_enabled() {
        return;
    }
    if let Ok(v) = std::env::var(DEV_RUNTIME_CURSEFORGE_API_KEY_ENV) {
        if !v.trim().is_empty() {
            return;
        }
    }
    match keyring_get_dev_curseforge_key() {
        Ok(Some(v)) => {
            std::env::set_var(DEV_RUNTIME_CURSEFORGE_API_KEY_ENV, v);
            return;
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("dev curseforge keyring preload failed: {e}");
        }
    }
    match read_dev_curseforge_key_file(app) {
        Ok(Some(v)) => {
            std::env::set_var(DEV_RUNTIME_CURSEFORGE_API_KEY_ENV, v);
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("dev curseforge file preload failed: {e}");
        }
    }
}

fn mask_secret(secret: &str) -> String {
    if secret.len() <= 8 {
        return "********".to_string();
    }
    let head = &secret[..4];
    let tail = &secret[secret.len().saturating_sub(4)..];
    format!("{head}…{tail}")
}

fn parse_curseforge_project_id(raw: &str) -> Result<i64, String> {
    let normalized = raw
        .trim()
        .trim_start_matches("cf:")
        .trim_start_matches("curseforge:")
        .trim();
    normalized
        .parse::<i64>()
        .map_err(|_| format!("Invalid CurseForge project ID: {}", raw))
}

fn github_owner_segment_is_valid(owner: &str) -> bool {
    let trimmed = owner.trim();
    if trimmed.is_empty() || trimmed.len() > 39 {
        return false;
    }
    let bytes = trimmed.as_bytes();
    if !bytes
        .first()
        .map(|value| value.is_ascii_alphanumeric())
        .unwrap_or(false)
        || !bytes
            .last()
            .map(|value| value.is_ascii_alphanumeric())
            .unwrap_or(false)
    {
        return false;
    }
    let mut previous_hyphen = false;
    for byte in bytes {
        if byte.is_ascii_alphanumeric() {
            previous_hyphen = false;
            continue;
        }
        if *byte == b'-' {
            if previous_hyphen {
                return false;
            }
            previous_hyphen = true;
            continue;
        }
        return false;
    }
    true
}

fn github_repo_segment_is_valid(repo: &str) -> bool {
    let trimmed = repo.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || trimmed.len() > 100 {
        return false;
    }
    trimmed
        .as_bytes()
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_' || *byte == b'.')
}

fn parse_github_project_id(raw: &str) -> Result<(String, String), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("GitHub project ID is required".to_string());
    }
    let mut normalized = trimmed
        .trim_start_matches("gh:")
        .trim_start_matches("github:")
        .trim()
        .to_string();
    let mut is_github_url = false;
    let normalized_lower = normalized.to_ascii_lowercase();
    if normalized_lower.starts_with("https://") || normalized_lower.starts_with("http://") {
        let scheme_len = if normalized_lower.starts_with("https://") {
            "https://".len()
        } else {
            "http://".len()
        };
        let after_scheme = &normalized[scheme_len..];
        let host_end = after_scheme
            .find(['/', '?', '#'])
            .unwrap_or(after_scheme.len());
        let host_port = after_scheme[..host_end].trim();
        let host = host_port
            .rsplit('@')
            .next()
            .unwrap_or(host_port)
            .split(':')
            .next()
            .unwrap_or(host_port)
            .trim()
            .to_ascii_lowercase();
        if host != "github.com" && host != "www.github.com" {
            return Err(format!(
                "Invalid GitHub project ID '{}'. Expected 'owner/repo'.",
                raw
            ));
        }
        is_github_url = true;
        normalized = after_scheme[host_end..].to_string();
    } else {
        let lowered = normalized.to_ascii_lowercase();
        for prefix in ["github.com/", "www.github.com/"] {
            if lowered.starts_with(prefix) {
                normalized = normalized[prefix.len()..].to_string();
                is_github_url = true;
                break;
            }
        }
        if lowered.contains("://") {
            return Err(format!(
                "Invalid GitHub project ID '{}'. Expected 'owner/repo'.",
                raw
            ));
        }
    }
    normalized = normalized
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_string();
    normalized = normalized
        .trim_matches('/')
        .trim_end_matches(".git")
        .to_string();
    let parts = normalized
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().to_string())
        .collect::<Vec<_>>();
    if parts.len() < 2 || (!is_github_url && parts.len() != 2) {
        return Err(format!(
            "Invalid GitHub project ID '{}'. Expected 'owner/repo'.",
            raw
        ));
    }
    let owner = parts[0].trim().to_string();
    let repo = parts[1].trim().to_string();
    if !github_owner_segment_is_valid(&owner) || !github_repo_segment_is_valid(&repo) {
        return Err(format!(
            "Invalid GitHub project ID '{}'. Expected 'owner/repo'.",
            raw
        ));
    }
    Ok((owner, repo))
}

fn github_project_key(owner: &str, repo: &str) -> String {
    format!("gh:{}/{}", owner.trim(), repo.trim())
}

fn parse_github_release_id(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("gh_release:") {
        return rest.trim().parse::<u64>().ok().filter(|value| *value > 0);
    }
    trimmed.parse::<u64>().ok().filter(|value| *value > 0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Instance {
    id: String,
    name: String,
    #[serde(default)]
    folder_name: Option<String>,
    mc_version: String,
    loader: String, // "fabric" | "forge"
    created_at: String,
    #[serde(default)]
    icon_path: Option<String>,
    #[serde(default)]
    settings: InstanceSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstanceSettings {
    #[serde(default = "default_true")]
    keep_launcher_open_while_playing: bool,
    #[serde(default)]
    close_launcher_on_game_exit: bool,
    #[serde(default)]
    notes: String,
    #[serde(default = "default_true")]
    sync_minecraft_settings: bool,
    #[serde(default = "default_sync_minecraft_settings_target")]
    sync_minecraft_settings_target: String,
    #[serde(default)]
    auto_update_installed_content: bool,
    #[serde(default = "default_true")]
    prefer_release_builds: bool,
    #[serde(default)]
    java_path: String,
    #[serde(default = "default_memory_mb")]
    memory_mb: u32,
    #[serde(default)]
    jvm_args: String,
    #[serde(default = "default_graphics_preset")]
    graphics_preset: String,
    #[serde(default)]
    enable_shaders: bool,
    #[serde(default)]
    force_vsync: bool,
    #[serde(default = "default_world_backup_interval_minutes")]
    world_backup_interval_minutes: u32,
    #[serde(default = "default_world_backup_retention_count")]
    world_backup_retention_count: u32,
    #[serde(default = "default_snapshot_retention_count")]
    snapshot_retention_count: u32,
    #[serde(default = "default_snapshot_max_age_days")]
    snapshot_max_age_days: u32,
}

impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            keep_launcher_open_while_playing: true,
            close_launcher_on_game_exit: false,
            notes: String::new(),
            sync_minecraft_settings: true,
            sync_minecraft_settings_target: default_sync_minecraft_settings_target(),
            auto_update_installed_content: false,
            prefer_release_builds: true,
            java_path: String::new(),
            memory_mb: default_memory_mb(),
            jvm_args: String::new(),
            graphics_preset: default_graphics_preset(),
            enable_shaders: false,
            force_vsync: false,
            world_backup_interval_minutes: default_world_backup_interval_minutes(),
            world_backup_retention_count: default_world_backup_retention_count(),
            snapshot_retention_count: default_snapshot_retention_count(),
            snapshot_max_age_days: default_snapshot_max_age_days(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct InstanceIndex {
    instances: Vec<Instance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderCandidate {
    source: String,
    project_id: String,
    version_id: String,
    name: String,
    version_number: String,
    #[serde(default)]
    confidence: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalModAnalysis {
    #[serde(default)]
    loader_hints: Vec<String>,
    #[serde(default)]
    mod_ids: Vec<String>,
    #[serde(default)]
    required_dependencies: Vec<String>,
    #[serde(default)]
    warnings: Vec<String>,
    #[serde(default)]
    suggestions: Vec<String>,
    scanned_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockEntry {
    source: String,
    project_id: String,
    version_id: String,
    name: String,
    version_number: String,
    filename: String,
    #[serde(default = "default_content_type_mods")]
    content_type: String,
    #[serde(default = "default_target_scope_instance")]
    target_scope: String,
    #[serde(default)]
    target_worlds: Vec<String>,
    #[serde(default)]
    pinned_version: Option<String>,
    enabled: bool,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    provider_candidates: Vec<ProviderCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    local_analysis: Option<LocalModAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Lockfile {
    version: u32,
    entries: Vec<LockEntry>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: 2,
            entries: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledMod {
    source: String,
    project_id: String,
    version_id: String,
    name: String,
    version_number: String,
    filename: String,
    content_type: String,
    target_scope: String,
    #[serde(default)]
    target_worlds: Vec<String>,
    #[serde(default)]
    pinned_version: Option<String>,
    enabled: bool,
    file_exists: bool,
    #[serde(default)]
    added_at: i64,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    provider_candidates: Vec<ProviderCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    local_analysis: Option<LocalModAnalysis>,
}

#[derive(Debug, Clone, Serialize)]
struct InstallProgressEvent {
    instance_id: String,
    project_id: String,
    stage: String, // resolving | downloading | completed | error
    downloaded: u64,
    total: Option<u64>,
    percent: Option<f64>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateInstanceArgs {
    name: String,
    #[serde(alias = "mcVersion", alias = "mc_version")]
    mc_version: String,
    loader: String,
    #[serde(alias = "iconPath", alias = "icon_path", default)]
    icon_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeleteInstanceArgs {
    id: String,
}

#[derive(Debug, Deserialize)]
struct SetInstanceIconArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "iconPath", alias = "icon_path", default)]
    icon_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReadLocalImageDataUrlArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct UpdateInstanceArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(alias = "mcVersion", alias = "mc_version", default)]
    mc_version: Option<String>,
    #[serde(default)]
    loader: Option<String>,
    #[serde(default)]
    settings: Option<InstanceSettings>,
}

#[derive(Debug, Clone, Serialize)]
struct JavaRuntimeCandidate {
    path: String,
    major: u32,
    version_line: String,
}

#[derive(Debug, Clone, Serialize)]
struct CurseforgeApiStatus {
    configured: bool,
    env_var: Option<String>,
    key_hint: Option<String>,
    validated: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct GithubTokenPoolStatus {
    configured: bool,
    total_tokens: usize,
    env_tokens: usize,
    keychain_tokens: usize,
    keychain_available: bool,
    unauth_rate_limited: bool,
    unauth_rate_limit_reset_at: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct InstallModrinthModArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "projectId")]
    project_id: String,
    #[serde(alias = "projectTitle", default)]
    project_title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListInstalledModsArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
}

#[derive(Debug, Deserialize)]
struct SetInstalledModEnabledArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "versionId")]
    version_id: String,
    #[serde(alias = "contentType", default)]
    content_type: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct SetInstalledModProviderArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "versionId")]
    version_id: String,
    #[serde(alias = "contentType", default)]
    content_type: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    source: String,
}

#[derive(Debug, Deserialize)]
struct AttachInstalledModGithubRepoArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "versionId")]
    version_id: String,
    #[serde(alias = "contentType", default)]
    content_type: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(alias = "githubRepo", alias = "repo", alias = "projectId")]
    github_repo: String,
    #[serde(default)]
    activate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RemoveInstalledModArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "versionId")]
    version_id: String,
    #[serde(alias = "contentType", default)]
    content_type: Option<String>,
    #[serde(default)]
    filename: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImportLocalModFileArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "filePath")]
    file_path: String,
    #[serde(alias = "contentType", default)]
    content_type: Option<String>,
    #[serde(alias = "targetWorlds", default)]
    target_worlds: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CheckUpdatesArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "contentTypes", default)]
    content_types: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct LaunchInstanceArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(default)]
    method: Option<String>,
    #[serde(alias = "quickPlayHost", default)]
    quick_play_host: Option<String>,
    #[serde(alias = "quickPlayPort", default)]
    quick_play_port: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct ExportInstanceModsZipArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "outputPath", default)]
    output_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PreflightLaunchCompatibilityArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "launchMethod", default)]
    method: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResolveLocalModSourcesArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(default)]
    mode: Option<String>, // missing_only | all
    #[serde(alias = "contentTypes", default)]
    content_types: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct SetInstalledModPinArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "versionId")]
    version_id: String,
    #[serde(alias = "contentType", default)]
    content_type: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    pin: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListInstanceHistoryEventsArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(alias = "beforeAt", default)]
    before_at: Option<String>,
    #[serde(default)]
    kinds: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct UpsertQuickPlayServerArgs {
    #[serde(default)]
    id: Option<String>,
    name: String,
    host: String,
    #[serde(default)]
    port: Option<u16>,
    #[serde(alias = "boundInstanceId", default)]
    bound_instance_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoveQuickPlayServerArgs {
    id: String,
}

#[derive(Debug, Deserialize)]
struct LaunchQuickPlayServerArgs {
    #[serde(alias = "serverId")]
    server_id: String,
    #[serde(default)]
    method: Option<String>,
    #[serde(alias = "instanceId", default)]
    instance_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PruneMissingInstalledEntriesArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "contentTypes", default)]
    content_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SupportPerfAction {
    id: String,
    name: String,
    #[serde(default)]
    detail: Option<String>,
    status: String,
    duration_ms: f64,
    finished_at: i64,
}

#[derive(Debug, Deserialize)]
struct ExportInstanceSupportBundleArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "outputPath", default)]
    output_path: Option<String>,
    #[serde(alias = "includeRawLogs", default)]
    include_raw_logs: Option<bool>,
    #[serde(alias = "perfActions", default)]
    perf_actions: Vec<SupportPerfAction>,
}

#[derive(Debug, Deserialize)]
struct PollMicrosoftLoginArgs {
    #[serde(alias = "sessionId")]
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct SelectLauncherAccountArgs {
    #[serde(alias = "accountId")]
    account_id: String,
}

#[derive(Debug, Deserialize)]
struct LogoutMicrosoftAccountArgs {
    #[serde(alias = "accountId")]
    account_id: String,
}

#[derive(Debug, Deserialize)]
struct SetLauncherSettingsArgs {
    #[serde(alias = "defaultLaunchMethod", default)]
    default_launch_method: Option<String>,
    #[serde(alias = "javaPath", default)]
    java_path: Option<String>,
    #[serde(alias = "oauthClientId", default)]
    oauth_client_id: Option<String>,
    #[serde(alias = "updateCheckCadence", default)]
    update_check_cadence: Option<String>,
    #[serde(alias = "updateAutoApplyMode", default)]
    update_auto_apply_mode: Option<String>,
    #[serde(alias = "updateApplyScope", default)]
    update_apply_scope: Option<String>,
    #[serde(alias = "autoIdentifyLocalJars", default)]
    auto_identify_local_jars: Option<bool>,
    #[serde(alias = "autoTriggerMicPermissionPrompt", default)]
    auto_trigger_mic_permission_prompt: Option<bool>,
    #[serde(alias = "discordPresenceEnabled", default)]
    discord_presence_enabled: Option<bool>,
    #[serde(alias = "discordPresenceDetailLevel", default)]
    discord_presence_detail_level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SetDevCurseforgeApiKeyArgs {
    key: String,
}

#[derive(Debug, Deserialize)]
struct SetGithubTokenPoolArgs {
    #[serde(alias = "tokens", alias = "tokenPool", alias = "pool")]
    tokens: String,
}

#[derive(Debug, Deserialize)]
struct ApplySelectedAccountAppearanceArgs {
    #[serde(alias = "applySkin", default)]
    apply_skin: bool,
    #[serde(alias = "skinSource", default)]
    skin_source: Option<String>,
    #[serde(alias = "skinVariant", default)]
    skin_variant: Option<String>,
    #[serde(alias = "applyCape", default)]
    apply_cape: bool,
    #[serde(alias = "capeId", default)]
    cape_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StopRunningInstanceArgs {
    #[serde(alias = "launchId")]
    launch_id: String,
}

#[derive(Debug, Deserialize)]
struct CancelInstanceLaunchArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
}

#[derive(Debug, Deserialize)]
struct OpenInstancePathArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    target: String, // instance | mods
}

#[derive(Debug, Deserialize)]
struct ReadInstanceLogsArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    source: String, // live | latest_launch | latest_crash
    #[serde(alias = "maxLines", default)]
    max_lines: Option<usize>,
    #[serde(alias = "beforeLine", default)]
    before_line: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RevealConfigEditorFileArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    scope: String, // instance | world
    #[serde(alias = "worldId", default)]
    world_id: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateInstanceFromModpackFileArgs {
    #[serde(alias = "filePath")]
    file_path: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(alias = "iconPath", alias = "icon_path", default)]
    icon_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CreateInstanceFromModpackFileResult {
    instance: Instance,
    imported_files: usize,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LauncherImportSource {
    id: String,
    source_kind: String, // vanilla | prism
    label: String,
    mc_version: String,
    loader: String,
    source_path: String,
}

#[derive(Debug, Deserialize)]
struct ImportInstanceFromLauncherArgs {
    #[serde(alias = "sourceId")]
    source_id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(alias = "iconPath", alias = "icon_path", default)]
    icon_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ImportInstanceFromLauncherResult {
    instance: Instance,
    imported_files: usize,
}

#[derive(Debug, Deserialize)]
struct RollbackInstanceArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "snapshotId", default)]
    snapshot_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListInstanceSnapshotsArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
}

#[derive(Debug, Deserialize)]
struct ListInstanceWorldsArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
}

#[derive(Debug, Deserialize)]
struct GetInstanceDiskUsageArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
}

#[derive(Debug, Deserialize)]
struct GetInstanceLastRunMetadataArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
}

#[derive(Debug, Deserialize)]
struct GetInstancePlaytimeArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
}

#[derive(Debug, Deserialize)]
struct ListWorldConfigFilesArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "worldId")]
    world_id: String,
}

#[derive(Debug, Deserialize)]
struct ReadWorldConfigFileArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "worldId")]
    world_id: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct WriteWorldConfigFileArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "worldId")]
    world_id: String,
    path: String,
    content: String,
    #[serde(alias = "expectedModifiedAt", default)]
    expected_modified_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RollbackInstanceWorldBackupArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "worldId")]
    world_id: String,
    #[serde(alias = "backupId", default)]
    backup_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InstallDiscoverContentArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    source: String,
    #[serde(alias = "projectId")]
    project_id: String,
    #[serde(alias = "projectTitle", default)]
    project_title: Option<String>,
    #[serde(alias = "contentType")]
    content_type: String,
    #[serde(alias = "targetWorlds", default)]
    target_worlds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreatorPresetSettings {
    #[serde(default)]
    dependency_policy: String,
    #[serde(default)]
    conflict_strategy: String,
    #[serde(default)]
    provider_priority: Vec<String>,
    #[serde(default = "default_true")]
    snapshot_before_apply: bool,
    #[serde(default)]
    apply_order: Vec<String>,
    #[serde(default)]
    datapack_target_policy: String,
}

impl Default for CreatorPresetSettings {
    fn default() -> Self {
        default_preset_settings()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreatorPresetEntry {
    source: String,
    project_id: String,
    title: String,
    content_type: String,
    #[serde(default)]
    pinned_version: Option<String>,
    #[serde(default = "default_target_scope_instance")]
    target_scope: String,
    #[serde(default)]
    target_worlds: Vec<String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreatorPreset {
    id: String,
    name: String,
    created_at: String,
    source_instance_id: String,
    source_instance_name: String,
    entries: Vec<CreatorPresetEntry>,
    #[serde(default)]
    settings: CreatorPresetSettings,
}

#[derive(Debug, Deserialize)]
struct ApplyPresetToInstanceArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    preset: CreatorPreset,
}

#[derive(Debug, Deserialize)]
struct PreviewPresetApplyArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    preset: CreatorPreset,
}

#[derive(Debug, Deserialize)]
struct ImportProviderModpackArgs {
    source: String,
    #[serde(alias = "projectId")]
    project_id: String,
    #[serde(alias = "projectTitle", default)]
    project_title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetCurseforgeProjectArgs {
    #[serde(alias = "projectId")]
    project_id: String,
    #[serde(alias = "contentType", default)]
    content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetGithubProjectArgs {
    #[serde(alias = "projectId")]
    project_id: String,
}

#[derive(Debug, Deserialize)]
struct ExportPresetsJsonArgs {
    #[serde(alias = "outputPath")]
    output_path: String,
    payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ImportPresetsJsonArgs {
    #[serde(alias = "inputPath")]
    input_path: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SearchDiscoverContentArgs {
    query: String,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(alias = "gameVersion", default)]
    game_version: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    index: String, // relevance | downloads | follows | updated | newest
    limit: usize,
    offset: usize,
    source: String, // modrinth | curseforge | all
    #[serde(alias = "contentType")]
    content_type: String, // mods | modpacks | resourcepacks | datapacks | shaders
}

#[derive(Debug, Deserialize)]
struct InstallCurseforgeModArgs {
    #[serde(alias = "instanceId")]
    instance_id: String,
    #[serde(alias = "projectId")]
    project_id: String,
    #[serde(alias = "projectTitle", default)]
    project_title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthProjectResponse {
    title: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ModrinthVersionFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: Option<bool>,
    #[serde(default)]
    hashes: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModrinthDependency {
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    dependency_type: String, // required | optional | incompatible | embedded
}

#[derive(Debug, Clone, Deserialize)]
struct ModrinthVersion {
    #[serde(default)]
    project_id: String,
    id: String,
    version_number: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    date_published: String,
    #[serde(default)]
    dependencies: Vec<ModrinthDependency>,
    #[serde(default)]
    files: Vec<ModrinthVersionFile>,
}

#[derive(Debug, Clone)]
struct ResolvedInstallMod {
    project_id: String,
    version: ModrinthVersion,
    file: ModrinthVersionFile,
}

#[derive(Debug, Deserialize)]
struct CurseforgePagination {
    #[serde(default)]
    #[serde(rename = "totalCount")]
    total_count: usize,
}

#[derive(Debug, Deserialize)]
struct CurseforgeSearchResponse {
    data: Vec<CurseforgeMod>,
    #[serde(default)]
    pagination: Option<CurseforgePagination>,
}

#[derive(Debug, Deserialize)]
struct CurseforgeModResponse {
    data: CurseforgeMod,
}

#[derive(Debug, Deserialize)]
struct CurseforgeFilesResponse {
    data: Vec<CurseforgeFile>,
}

#[derive(Debug, Deserialize)]
struct CurseforgeFileResponse {
    data: CurseforgeFile,
}

#[derive(Debug, Deserialize, Default)]
struct CurseforgeFingerprintData {
    #[serde(default)]
    #[serde(rename = "exactMatches")]
    exact_matches: Vec<CurseforgeFingerprintMatch>,
}

#[derive(Debug, Deserialize)]
struct CurseforgeFingerprintResponse {
    data: CurseforgeFingerprintData,
}

#[derive(Debug, Deserialize, Default)]
struct CurseforgeFingerprintMatch {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    #[serde(rename = "modId")]
    mod_id: i64,
    #[serde(default)]
    file: Option<CurseforgeFile>,
}

#[derive(Debug, Deserialize)]
struct CurseforgeDownloadUrlResponse {
    data: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeAuthor {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeLogo {
    url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeCategory {
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeFileHash {
    #[serde(default)]
    value: String,
    #[serde(default)]
    algo: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeMod {
    id: i64,
    #[serde(default)]
    #[serde(rename = "classId")]
    class_id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    #[serde(rename = "downloadCount")]
    download_count: f64,
    #[serde(default)]
    #[serde(rename = "dateModified")]
    date_modified: String,
    #[serde(default)]
    authors: Vec<CurseforgeAuthor>,
    #[serde(default)]
    categories: Vec<CurseforgeCategory>,
    #[serde(default)]
    logo: Option<CurseforgeLogo>,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeFile {
    id: i64,
    #[serde(default)]
    #[serde(rename = "modId")]
    mod_id: i64,
    #[serde(default)]
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(default)]
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(default)]
    #[serde(rename = "fileDate")]
    file_date: String,
    #[serde(default)]
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
    #[serde(default)]
    #[serde(rename = "gameVersions")]
    game_versions: Vec<String>,
    #[serde(default)]
    hashes: Vec<CurseforgeFileHash>,
    #[serde(default)]
    dependencies: Vec<CurseforgeFileDependency>,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeFileDependency {
    #[serde(default)]
    #[serde(rename = "modId")]
    mod_id: i64,
    #[serde(default)]
    #[serde(rename = "relationType")]
    relation_type: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct GithubOwner {
    #[serde(default)]
    login: String,
    #[serde(default)]
    #[serde(rename = "type")]
    owner_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRepository {
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    stargazers_count: u64,
    #[serde(default)]
    forks_count: u64,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    fork: bool,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    watchers_count: u64,
    #[serde(default)]
    open_issues_count: u64,
    #[serde(default)]
    pushed_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    default_branch: String,
    #[serde(default)]
    owner: GithubOwner,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRepoSearchResponse {
    #[serde(default)]
    items: Vec<GithubRepository>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubTreeNode {
    #[serde(default)]
    path: String,
    #[serde(default)]
    #[serde(rename = "type")]
    node_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubTreeResponse {
    #[serde(default)]
    tree: Vec<GithubTreeNode>,
    #[serde(default, rename = "truncated")]
    _truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubReleaseAsset {
    #[serde(default)]
    name: String,
    #[serde(default)]
    browser_download_url: String,
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    id: u64,
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubReadmeResponse {
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    download_url: String,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Clone)]
struct GithubReleaseSelection {
    release: GithubRelease,
    asset: GithubReleaseAsset,
    has_checksum_sidecar: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ModrinthModpackIndex {
    #[serde(default)]
    files: Vec<ModrinthModpackIndexFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModrinthModpackIndexFile {
    #[serde(default)]
    path: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeModpackManifest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    files: Vec<CurseforgeModpackManifestFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeModpackManifestFile {
    #[serde(rename = "projectID")]
    project_id: i64,
    #[serde(rename = "fileID")]
    file_id: i64,
}

#[derive(Debug, Clone, Serialize)]
struct InstallPlanPreview {
    total_mods: usize,
    dependency_mods: usize,
    will_install_mods: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ModUpdateInfo {
    project_id: String,
    name: String,
    current_version_id: String,
    current_version_number: String,
    latest_version_id: String,
    latest_version_number: String,
}

#[derive(Debug, Clone, Serialize)]
struct ModUpdateCheckResult {
    checked_mods: usize,
    update_count: usize,
    updates: Vec<ModUpdateInfo>,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateAllResult {
    checked_mods: usize,
    updated_mods: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ContentUpdateInfo {
    source: String,
    content_type: String,
    project_id: String,
    name: String,
    current_version_id: String,
    current_version_number: String,
    latest_version_id: String,
    latest_version_number: String,
    enabled: bool,
    target_worlds: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_download_url: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    latest_hashes: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    required_dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    compatibility_status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    compatibility_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ContentUpdateCheckResult {
    checked_entries: usize,
    update_count: usize,
    updates: Vec<ContentUpdateInfo>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateAllContentResult {
    checked_entries: usize,
    updated_entries: usize,
    warnings: Vec<String>,
    by_source: HashMap<String, usize>,
    by_content_type: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
struct LaunchResult {
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    launch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prism_instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prism_root: Option<String>,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct LaunchCompatibilityItem {
    code: String,
    title: String,
    message: String,
    severity: String, // blocker | warning | info
    blocking: bool,
}

#[derive(Debug, Clone, Serialize)]
struct LaunchCompatibilityReport {
    instance_id: String,
    status: String, // ok | warning | blocked
    checked_at: String,
    blocking_count: usize,
    warning_count: usize,
    unresolved_local_entries: usize,
    items: Vec<LaunchCompatibilityItem>,
    #[serde(default)]
    permissions: Vec<permissions::LaunchPermissionChecklistItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mic_requirement: Option<permissions::LaunchMicRequirementSummary>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalResolverMatch {
    key: String,
    from_source: String,
    to_source: String,
    project_id: String,
    version_id: String,
    name: String,
    version_number: String,
    confidence: String, // deterministic | high
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
struct LocalResolverResult {
    instance_id: String,
    scanned_entries: usize,
    resolved_entries: usize,
    remaining_local_entries: usize,
    matches: Vec<LocalResolverMatch>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PruneMissingInstalledEntriesResult {
    instance_id: String,
    removed_count: usize,
    remaining_count: usize,
    removed_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SupportBundleResult {
    output_path: String,
    files_count: usize,
    redactions_applied: usize,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum LaunchMethod {
    Prism,
    Native,
}

impl Default for LaunchMethod {
    fn default() -> Self {
        LaunchMethod::Native
    }
}

impl LaunchMethod {
    fn as_str(&self) -> &'static str {
        match self {
            LaunchMethod::Prism => "prism",
            LaunchMethod::Native => "native",
        }
    }

    fn parse(v: &str) -> Option<Self> {
        let x = v.trim().to_lowercase();
        match x.as_str() {
            "prism" => Some(LaunchMethod::Prism),
            "native" => Some(LaunchMethod::Native),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct LauncherSettings {
    default_launch_method: LaunchMethod,
    java_path: String,
    oauth_client_id: String,
    #[serde(default = "default_update_check_cadence")]
    update_check_cadence: String,
    #[serde(default = "default_update_auto_apply_mode")]
    update_auto_apply_mode: String,
    #[serde(default = "default_update_apply_scope")]
    update_apply_scope: String,
    selected_account_id: Option<String>,
    auto_identify_local_jars: bool,
    #[serde(default = "default_auto_trigger_mic_permission_prompt")]
    auto_trigger_mic_permission_prompt: bool,
    #[serde(default = "default_true")]
    discord_presence_enabled: bool,
    #[serde(default = "default_discord_presence_detail_level")]
    discord_presence_detail_level: String,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            default_launch_method: LaunchMethod::Native,
            java_path: String::new(),
            oauth_client_id: String::new(),
            update_check_cadence: default_update_check_cadence(),
            update_auto_apply_mode: default_update_auto_apply_mode(),
            update_apply_scope: default_update_apply_scope(),
            selected_account_id: None,
            auto_identify_local_jars: false,
            auto_trigger_mic_permission_prompt: default_auto_trigger_mic_permission_prompt(),
            discord_presence_enabled: true,
            discord_presence_detail_level: default_discord_presence_detail_level(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LauncherAccount {
    id: String,
    username: String,
    added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuickPlayServerEntry {
    id: String,
    name: String,
    host: String,
    port: u16,
    #[serde(default)]
    bound_instance_id: Option<String>,
    #[serde(default)]
    last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct QuickPlayServersStore {
    version: u32,
    servers: Vec<QuickPlayServerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct LauncherTokenFallbackStore {
    refresh_tokens: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
struct BeginMicrosoftLoginResult {
    session_id: String,
    auth_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verification_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MicrosoftLoginState {
    status: String, // pending | success | error
    message: Option<String>,
    account: Option<LauncherAccount>,
}

#[derive(Debug, Clone, Serialize)]
struct RunningInstance {
    launch_id: String,
    instance_id: String,
    instance_name: String,
    method: String,
    pid: u32,
    started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ExportModsResult {
    output_path: String,
    files_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct RollbackResult {
    snapshot_id: String,
    created_at: String,
    restored_files: usize,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct PresetsJsonIoResult {
    path: String,
    items: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotMeta {
    id: String,
    created_at: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
struct DiscoverSearchHit {
    source: String, // modrinth | curseforge | github
    project_id: String,
    title: String,
    description: String,
    author: String,
    downloads: u64,
    follows: u64,
    icon_url: Option<String>,
    categories: Vec<String>,
    versions: Vec<String>,
    date_modified: String,
    content_type: String, // mods | shaderpacks | resourcepacks | datapacks | modpacks
    slug: Option<String>,
    external_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    install_supported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    install_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DiscoverSearchResult {
    hits: Vec<DiscoverSearchHit>,
    offset: usize,
    limit: usize,
    total_hits: usize,
}

#[derive(Debug, Clone, Serialize)]
struct CurseforgeProjectFileDetail {
    file_id: String,
    display_name: String,
    file_name: String,
    file_date: String,
    game_versions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CurseforgeProjectDetail {
    source: String, // curseforge
    project_id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    slug: Option<String>,
    summary: String,
    description: String,
    author_names: Vec<String>,
    downloads: u64,
    categories: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_url: Option<String>,
    date_modified: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_url: Option<String>,
    files: Vec<CurseforgeProjectFileDetail>,
}

#[derive(Debug, Clone, Serialize)]
struct GithubProjectReleaseAssetDetail {
    name: String,
    download_url: String,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct GithubProjectReleaseDetail {
    id: String,
    tag_name: String,
    name: String,
    published_at: String,
    prerelease: bool,
    draft: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_url: Option<String>,
    assets: Vec<GithubProjectReleaseAssetDetail>,
}

#[derive(Debug, Clone, Serialize)]
struct GithubProjectDetail {
    source: String, // github
    project_id: String,
    title: String,
    owner: String,
    summary: String,
    description: String,
    stars: u64,
    forks: u64,
    watchers: u64,
    open_issues: u64,
    categories: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_url: Option<String>,
    date_modified: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    releases_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    issues_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readme_markdown: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readme_html_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readme_source_url: Option<String>,
    releases: Vec<GithubProjectReleaseDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct OpenInstancePathResult {
    target: String,
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct RevealConfigEditorFileResult {
    opened_path: String,
    revealed_file: bool,
    virtual_file: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct LogLineDto {
    raw: String,
    line_no: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    severity: Option<String>,
    source: String,
}

#[derive(Debug, Clone, Serialize)]
struct ReadInstanceLogsResult {
    source: String,
    path: String,
    available: bool,
    total_lines: usize,
    returned_lines: usize,
    truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_line_no: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line_no: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_before_line: Option<u64>,
    lines: Vec<LogLineDto>,
    updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct InstanceWorld {
    id: String,
    name: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_backup_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_backup_at: Option<String>,
    #[serde(default)]
    backup_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct InstanceLastRunMetadata {
    #[serde(default)]
    last_launch_at: Option<String>,
    #[serde(default)]
    last_exit_kind: Option<String>,
    #[serde(default)]
    last_exit_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaySessionRecord {
    id: String,
    launch_id: String,
    instance_id: String,
    method: String,
    isolated: bool,
    pid: u32,
    started_at: String,
    ended_at: String,
    duration_seconds: u64,
    exit_kind: String,
    recovered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct PlaySessionsStoreV1 {
    version: u32,
    total_seconds: u64,
    sessions: Vec<PlaySessionRecord>,
}

impl Default for PlaySessionsStoreV1 {
    fn default() -> Self {
        Self {
            version: 1,
            total_seconds: 0,
            sessions: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivePlaySessionRecord {
    launch_id: String,
    instance_id: String,
    method: String,
    isolated: bool,
    pid: u32,
    started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct ActivePlaySessionsStoreV1 {
    version: u32,
    active: Vec<ActivePlaySessionRecord>,
}

impl Default for ActivePlaySessionsStoreV1 {
    fn default() -> Self {
        Self {
            version: 1,
            active: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstancePlaytimeSummary {
    total_seconds: u64,
    sessions_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_session_seconds: Option<u64>,
    currently_running: bool,
    tracking_scope: String,
}

#[derive(Debug, Clone, Serialize)]
struct WorldConfigFileEntry {
    path: String,
    size_bytes: u64,
    modified_at: i64,
    editable: bool,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    readonly_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ReadWorldConfigFileResult {
    path: String,
    editable: bool,
    kind: String,
    size_bytes: u64,
    modified_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    readonly_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct WriteWorldConfigFileResult {
    path: String,
    size_bytes: u64,
    modified_at: i64,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct WorldRollbackResult {
    world_id: String,
    backup_id: String,
    created_at: String,
    restored_files: usize,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorldBackupMeta {
    id: String,
    world_id: String,
    created_at: String,
    reason: String,
    files_count: usize,
    total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
struct PresetApplyPreview {
    valid: bool,
    installable_entries: usize,
    skipped_disabled_entries: usize,
    missing_world_targets: Vec<String>,
    provider_warnings: Vec<String>,
    duplicate_entries: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PresetApplyResult {
    message: String,
    installed_entries: usize,
    skipped_entries: usize,
    failed_entries: usize,
    snapshot_id: Option<String>,
    by_content_type: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
struct AccountCosmeticSummary {
    id: String,
    state: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variant: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AccountDiagnostics {
    status: String, // connected | not_connected | error
    last_refreshed_at: String,
    selected_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account: Option<LauncherAccount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minecraft_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minecraft_username: Option<String>,
    entitlements_ok: bool,
    token_exchange_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skin_url: Option<String>,
    cape_count: usize,
    skins: Vec<AccountCosmeticSummary>,
    capes: Vec<AccountCosmeticSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    client_id_source: String,
}

struct RunningProcess {
    meta: RunningInstance,
    child: Arc<Mutex<Child>>,
    log_path: Option<PathBuf>,
}

#[derive(Clone, Default)]
struct AppState {
    login_sessions: Arc<Mutex<HashMap<String, MicrosoftLoginState>>>,
    running: Arc<Mutex<HashMap<String, RunningProcess>>>,
    launch_cancelled: Arc<Mutex<HashSet<String>>>,
    stop_requested_launches: Arc<Mutex<HashSet<String>>>,
}

fn app_instances_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path_resolver()
        .app_data_dir()
        .ok_or("Failed to resolve app data dir")?;
    Ok(base.join("instances"))
}

fn index_path(instances_dir: &Path) -> PathBuf {
    instances_dir.join("instances.json")
}

fn normalize_instance_folder_name(raw: &str) -> String {
    let cleaned = sanitize_name(raw);
    let collapsed = cleaned
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = collapsed.trim().trim_matches('.').to_string();
    if normalized.is_empty() {
        "Instance".to_string()
    } else {
        normalized
    }
}

fn instance_folder_name_or_legacy(inst: &Instance) -> String {
    if let Some(raw) = inst
        .folder_name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return normalize_instance_folder_name(raw);
    }
    if !inst.id.trim().is_empty() {
        return inst.id.trim().to_string();
    }
    normalize_instance_folder_name(&inst.name)
}

pub(crate) fn instance_dir_for_instance(instances_dir: &Path, inst: &Instance) -> PathBuf {
    let preferred = instances_dir.join(instance_folder_name_or_legacy(inst));
    if preferred.exists() {
        return preferred;
    }
    let legacy = instances_dir.join(&inst.id);
    if legacy.exists() {
        legacy
    } else {
        preferred
    }
}

pub(crate) fn instance_dir_for_id(
    instances_dir: &Path,
    instance_id: &str,
) -> Result<PathBuf, String> {
    let inst = find_instance(instances_dir, instance_id)?;
    Ok(instance_dir_for_instance(instances_dir, &inst))
}

fn instance_last_run_metadata_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(INSTANCE_LAST_RUN_METADATA_FILE)
}

fn read_instance_last_run_metadata_from_dir(instance_dir: &Path) -> InstanceLastRunMetadata {
    let path = instance_last_run_metadata_path(instance_dir);
    if !path.exists() {
        return InstanceLastRunMetadata::default();
    }
    match fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<InstanceLastRunMetadata>(&raw) {
            Ok(parsed) => parsed,
            Err(err) => {
                eprintln!(
                    "parse instance last-run metadata failed for '{}': {}",
                    path.display(),
                    err
                );
                InstanceLastRunMetadata::default()
            }
        },
        Err(err) => {
            eprintln!(
                "read instance last-run metadata failed for '{}': {}",
                path.display(),
                err
            );
            InstanceLastRunMetadata::default()
        }
    }
}

fn write_instance_last_run_metadata_to_dir(
    instance_dir: &Path,
    meta: &InstanceLastRunMetadata,
) -> Result<(), String> {
    let path = instance_last_run_metadata_path(instance_dir);
    let raw = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("serialize instance last-run metadata failed: {e}"))?;
    fs::write(&path, raw).map_err(|e| {
        format!(
            "write instance last-run metadata failed for '{}': {e}",
            path.display()
        )
    })
}

fn read_instance_last_run_metadata(
    instances_dir: &Path,
    instance_id: &str,
) -> Result<InstanceLastRunMetadata, String> {
    let instance_dir = instance_dir_for_id(instances_dir, instance_id)?;
    Ok(read_instance_last_run_metadata_from_dir(&instance_dir))
}

fn mark_instance_launch_triggered(instances_dir: &Path, instance_id: &str) -> Result<(), String> {
    let instance_dir = instance_dir_for_id(instances_dir, instance_id)?;
    let mut meta = read_instance_last_run_metadata_from_dir(&instance_dir);
    meta.last_launch_at = Some(now_iso());
    meta.last_exit_kind = Some("unknown".to_string());
    meta.last_exit_at = None;
    write_instance_last_run_metadata_to_dir(&instance_dir, &meta)
}

fn mark_instance_launch_exit(
    instances_dir: &Path,
    instance_id: &str,
    exit_kind: &str,
) -> Result<(), String> {
    let kind = exit_kind.trim().to_lowercase();
    let next_kind = match kind.as_str() {
        "success" | "crashed" | "stopped" => kind,
        _ => "unknown".to_string(),
    };
    let instance_dir = instance_dir_for_id(instances_dir, instance_id)?;
    let mut meta = read_instance_last_run_metadata_from_dir(&instance_dir);
    if meta.last_launch_at.is_none() {
        meta.last_launch_at = Some(now_iso());
    }
    meta.last_exit_kind = Some(next_kind);
    meta.last_exit_at = Some(now_iso());
    write_instance_last_run_metadata_to_dir(&instance_dir, &meta)
}

fn play_sessions_store_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(PLAY_SESSIONS_STORE_FILE)
}

fn play_sessions_active_store_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(PLAY_SESSIONS_ACTIVE_STORE_FILE)
}

fn read_play_sessions_store(instance_dir: &Path) -> PlaySessionsStoreV1 {
    let path = play_sessions_store_path(instance_dir);
    if !path.exists() {
        return PlaySessionsStoreV1::default();
    }
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<PlaySessionsStoreV1>(&raw).unwrap_or_default(),
        Err(_) => PlaySessionsStoreV1::default(),
    }
}

fn write_play_sessions_store(
    instance_dir: &Path,
    mut store: PlaySessionsStoreV1,
) -> Result<(), String> {
    store.version = 1;
    if store.sessions.len() > MAX_PLAY_SESSION_HISTORY {
        store.sessions.truncate(MAX_PLAY_SESSION_HISTORY);
    }
    let raw = serde_json::to_string_pretty(&store)
        .map_err(|e| format!("serialize play sessions store failed: {e}"))?;
    fs::write(play_sessions_store_path(instance_dir), raw)
        .map_err(|e| format!("write play sessions store failed: {e}"))
}

fn read_active_play_sessions_store(instance_dir: &Path) -> ActivePlaySessionsStoreV1 {
    let path = play_sessions_active_store_path(instance_dir);
    if !path.exists() {
        return ActivePlaySessionsStoreV1::default();
    }
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<ActivePlaySessionsStoreV1>(&raw).unwrap_or_default(),
        Err(_) => ActivePlaySessionsStoreV1::default(),
    }
}

fn write_active_play_sessions_store(
    instance_dir: &Path,
    mut store: ActivePlaySessionsStoreV1,
) -> Result<(), String> {
    store.version = 1;
    let raw = serde_json::to_string_pretty(&store)
        .map_err(|e| format!("serialize active play sessions store failed: {e}"))?;
    fs::write(play_sessions_active_store_path(instance_dir), raw)
        .map_err(|e| format!("write active play sessions store failed: {e}"))
}

#[cfg(target_os = "windows")]
fn process_pid_is_running(pid: u32) -> bool {
    let filter = format!("PID eq {}", pid);
    let output = Command::new("tasklist")
        .args(["/FI", &filter, "/FO", "CSV", "/NH"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            text.contains(&format!(",\"{}\"", pid))
                || text.contains(&format!(",{}", pid))
                || text.contains(&pid.to_string())
        }
        _ => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn process_pid_is_running(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn register_native_play_session_start(
    instances_dir: &Path,
    instance_id: &str,
    launch_id: &str,
    pid: u32,
    isolated: bool,
) -> Result<(), String> {
    let instance_dir = instance_dir_for_id(instances_dir, instance_id)?;
    let mut active = read_active_play_sessions_store(&instance_dir);
    active.active.retain(|entry| entry.launch_id != launch_id);
    active.active.push(ActivePlaySessionRecord {
        launch_id: launch_id.to_string(),
        instance_id: instance_id.to_string(),
        method: "native".to_string(),
        isolated,
        pid,
        started_at: now_iso(),
    });
    write_active_play_sessions_store(&instance_dir, active)
}

fn finalize_native_play_session(
    instances_dir: &Path,
    instance_id: &str,
    launch_id: &str,
    exit_kind: &str,
    recovered: bool,
) -> Result<Option<PlaySessionRecord>, String> {
    let instance_dir = instance_dir_for_id(instances_dir, instance_id)?;
    let mut active = read_active_play_sessions_store(&instance_dir);
    let Some(pos) = active
        .active
        .iter()
        .position(|entry| entry.launch_id == launch_id)
    else {
        return Ok(None);
    };
    let entry = active.active.remove(pos);
    write_active_play_sessions_store(&instance_dir, active)?;

    let ended_at = now_iso();
    let start_secs = created_at_sort_key(&entry.started_at);
    let end_secs = created_at_sort_key(&ended_at);
    let duration_seconds = if start_secs > 0 && end_secs >= start_secs {
        (end_secs - start_secs) as u64
    } else {
        0
    };
    let record = PlaySessionRecord {
        id: format!("ps_{}_{}", now_millis(), launch_id.replace(':', "_")),
        launch_id: entry.launch_id,
        instance_id: entry.instance_id,
        method: entry.method,
        isolated: entry.isolated,
        pid: entry.pid,
        started_at: entry.started_at,
        ended_at,
        duration_seconds,
        exit_kind: exit_kind.trim().to_lowercase(),
        recovered,
    };

    let mut store = read_play_sessions_store(&instance_dir);
    store.total_seconds = store.total_seconds.saturating_add(record.duration_seconds);
    store.sessions.insert(0, record.clone());
    write_play_sessions_store(&instance_dir, store)?;
    Ok(Some(record))
}

fn instance_playtime_summary(
    instances_dir: &Path,
    instance_id: &str,
) -> Result<InstancePlaytimeSummary, String> {
    let instance_dir = instance_dir_for_id(instances_dir, instance_id)?;
    let store = read_play_sessions_store(&instance_dir);
    let active = read_active_play_sessions_store(&instance_dir);
    let currently_running = active.active.iter().any(|entry| {
        entry.method.eq_ignore_ascii_case("native") && process_pid_is_running(entry.pid)
    });
    Ok(InstancePlaytimeSummary {
        total_seconds: store.total_seconds,
        sessions_count: store.sessions.len(),
        last_session_seconds: store.sessions.first().map(|item| item.duration_seconds),
        currently_running,
        tracking_scope: "native_only".to_string(),
    })
}

fn recover_native_play_sessions_startup(app: &tauri::AppHandle) -> Result<(), String> {
    let instances_dir = app_instances_dir(app)?;
    let idx = read_index(&instances_dir)?;
    for instance in idx.instances {
        let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
        if !instance_dir.exists() {
            continue;
        }
        let active_store = read_active_play_sessions_store(&instance_dir);
        for active in active_store.active {
            if !active.method.eq_ignore_ascii_case("native") {
                let _ = finalize_native_play_session(
                    &instances_dir,
                    &active.instance_id,
                    &active.launch_id,
                    "unknown",
                    true,
                );
                continue;
            }
            if process_pid_is_running(active.pid) {
                let instances_dir_for_thread = instances_dir.clone();
                let instance_id_for_thread = active.instance_id.clone();
                let launch_id_for_thread = active.launch_id.clone();
                thread::spawn(move || loop {
                    if !process_pid_is_running(active.pid) {
                        let _ = finalize_native_play_session(
                            &instances_dir_for_thread,
                            &instance_id_for_thread,
                            &launch_id_for_thread,
                            "unknown",
                            true,
                        );
                        break;
                    }
                    thread::sleep(Duration::from_secs(3));
                });
            } else {
                let _ = finalize_native_play_session(
                    &instances_dir,
                    &active.instance_id,
                    &active.launch_id,
                    "unknown",
                    true,
                );
            }
        }
    }
    Ok(())
}

fn dir_total_size_bytes(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let meta = match fs::symlink_metadata(&current) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_file() {
            total = total.saturating_add(meta.len());
            continue;
        }
        if !meta.is_dir() {
            continue;
        }
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(err) => {
                eprintln!("read dir failed for '{}': {}", current.display(), err);
                continue;
            }
        };
        for entry in entries.flatten() {
            stack.push(entry.path());
        }
    }
    total
}

fn lock_path(instances_dir: &Path, instance_id: &str) -> PathBuf {
    instance_dir_for_id(instances_dir, instance_id)
        .unwrap_or_else(|_| instances_dir.join(instance_id))
        .join("lock.json")
}

fn launcher_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path_resolver()
        .app_data_dir()
        .ok_or("Failed to resolve app data dir")?;
    Ok(base.join("launcher"))
}

fn launcher_settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(launcher_dir(app)?.join("settings.json"))
}

fn launcher_quick_play_servers_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(launcher_dir(app)?.join(QUICK_PLAY_SERVERS_FILE))
}

fn launcher_accounts_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(launcher_dir(app)?.join("accounts.json"))
}

fn launcher_token_fallback_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(launcher_dir(app)?.join(LAUNCHER_TOKEN_FALLBACK_FILE))
}

fn launcher_token_recovery_fallback_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(launcher_dir(app)?.join(LAUNCHER_TOKEN_RECOVERY_FALLBACK_FILE))
}

#[cfg(debug_assertions)]
fn launcher_token_debug_fallback_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(launcher_dir(app)?.join(LAUNCHER_TOKEN_DEBUG_FALLBACK_FILE))
}

fn launcher_cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(launcher_dir(app)?.join("cache"))
}

fn launcher_dev_curseforge_key_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(launcher_dir(app)?.join("dev_curseforge_api_key.txt"))
}

fn read_dev_curseforge_key_file(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    let p = launcher_dev_curseforge_key_path(app)?;
    if !p.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&p).map_err(|e| format!("read dev curseforge key file failed: {e}"))?;
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed))
}

fn write_dev_curseforge_key_file(app: &tauri::AppHandle, key: &str) -> Result<(), String> {
    let p = launcher_dev_curseforge_key_path(app)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir launcher dir failed: {e}"))?;
    }
    fs::write(&p, key).map_err(|e| format!("write dev curseforge key file failed: {e}"))
}

fn clear_dev_curseforge_key_file(app: &tauri::AppHandle) -> Result<(), String> {
    let p = launcher_dev_curseforge_key_path(app)?;
    if p.exists() {
        fs::remove_file(&p).map_err(|e| format!("clear dev curseforge key file failed: {e}"))?;
    }
    Ok(())
}

fn keyring_username_for_account(account_id: &str) -> String {
    format!("msa_refresh_{account_id}")
}

fn keyring_alias_usernames_for_key(key: &str) -> Vec<String> {
    fn push_unique(out: &mut Vec<String>, value: String) {
        if value.trim().is_empty() {
            return;
        }
        if !out.iter().any(|item| item == &value) {
            out.push(value);
        }
    }

    fn add_variants(out: &mut Vec<String>, raw: &str) {
        let key = raw.trim();
        if key.is_empty() {
            return;
        }
        push_unique(out, keyring_username_for_account(key));
        push_unique(out, format!("msa_refresh_token_{key}"));
        push_unique(out, key.to_string());
    }

    let mut out = Vec::new();
    let trimmed = key.trim();
    add_variants(&mut out, trimmed);
    add_variants(&mut out, &trimmed.to_lowercase());
    if let Ok(uuid) = Uuid::parse_str(trimmed) {
        add_variants(&mut out, &uuid.simple().to_string());
        add_variants(&mut out, &uuid.hyphenated().to_string());
    }
    out
}

fn read_launcher_settings(app: &tauri::AppHandle) -> Result<LauncherSettings, String> {
    let p = launcher_settings_path(app)?;
    if !p.exists() {
        return Ok(LauncherSettings::default());
    }
    let raw = fs::read_to_string(&p).map_err(|e| format!("read launcher settings failed: {e}"))?;
    let mut settings: LauncherSettings =
        serde_json::from_str(&raw).map_err(|e| format!("parse launcher settings failed: {e}"))?;
    settings.update_check_cadence = normalize_update_check_cadence(&settings.update_check_cadence);
    settings.update_auto_apply_mode =
        normalize_update_auto_apply_mode(&settings.update_auto_apply_mode);
    settings.update_apply_scope = normalize_update_apply_scope(&settings.update_apply_scope);
    settings.discord_presence_detail_level =
        normalize_discord_presence_detail_level(&settings.discord_presence_detail_level);
    Ok(settings)
}

fn write_launcher_settings(
    app: &tauri::AppHandle,
    settings: &LauncherSettings,
) -> Result<(), String> {
    let p = launcher_settings_path(app)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir launcher dir failed: {e}"))?;
    }
    let s = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("serialize launcher settings failed: {e}"))?;
    fs::write(&p, s).map_err(|e| format!("write launcher settings failed: {e}"))
}

fn read_launcher_accounts(app: &tauri::AppHandle) -> Result<Vec<LauncherAccount>, String> {
    let p = launcher_accounts_path(app)?;
    if !p.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(&p).map_err(|e| format!("read launcher accounts failed: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse launcher accounts failed: {e}"))
}

fn write_launcher_accounts(
    app: &tauri::AppHandle,
    accounts: &[LauncherAccount],
) -> Result<(), String> {
    let p = launcher_accounts_path(app)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir launcher dir failed: {e}"))?;
    }
    let s = serde_json::to_string_pretty(accounts)
        .map_err(|e| format!("serialize launcher accounts failed: {e}"))?;
    fs::write(&p, s).map_err(|e| format!("write launcher accounts failed: {e}"))
}

fn normalize_quick_play_host(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let stripped = trimmed
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let host_part = stripped
        .split('/')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('.');
    if host_part.is_empty() || host_part.contains(' ') {
        return None;
    }
    Some(host_part.to_ascii_lowercase())
}

fn normalize_quick_play_port(input: Option<u16>) -> u16 {
    let value = input.unwrap_or(25565);
    value.clamp(1, u16::MAX)
}

fn read_quick_play_servers(app: &tauri::AppHandle) -> Result<QuickPlayServersStore, String> {
    let p = launcher_quick_play_servers_path(app)?;
    if !p.exists() {
        return Ok(QuickPlayServersStore {
            version: 1,
            servers: vec![],
        });
    }
    let raw = fs::read_to_string(&p).map_err(|e| format!("read quick-play servers failed: {e}"))?;
    let mut store: QuickPlayServersStore =
        serde_json::from_str(&raw).map_err(|e| format!("parse quick-play servers failed: {e}"))?;
    if store.version == 0 {
        store.version = 1;
    }
    store.servers.retain(|entry| !entry.id.trim().is_empty());
    for entry in &mut store.servers {
        entry.host = normalize_quick_play_host(&entry.host).unwrap_or_default();
        entry.port = normalize_quick_play_port(Some(entry.port));
        entry.name = entry.name.trim().to_string();
        entry.bound_instance_id = entry
            .bound_instance_id
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        entry.last_used_at = entry
            .last_used_at
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
    }
    store.servers.retain(|entry| !entry.host.is_empty());
    store
        .servers
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(store)
}

fn write_quick_play_servers(
    app: &tauri::AppHandle,
    store: &QuickPlayServersStore,
) -> Result<(), String> {
    let p = launcher_quick_play_servers_path(app)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir launcher dir failed: {e}"))?;
    }
    let mut normalized = store.clone();
    normalized.version = 1;
    normalized
        .servers
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    let s = serde_json::to_string_pretty(&normalized)
        .map_err(|e| format!("serialize quick-play servers failed: {e}"))?;
    fs::write(&p, s).map_err(|e| format!("write quick-play servers failed: {e}"))
}

fn read_token_fallback_store_at_path(path: &Path) -> Result<LauncherTokenFallbackStore, String> {
    if !path.exists() {
        return Ok(LauncherTokenFallbackStore::default());
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("read launcher token fallback failed: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse launcher token fallback failed: {e}"))
}

fn write_token_fallback_store_at_path(
    path: &Path,
    store: &LauncherTokenFallbackStore,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir launcher dir failed: {e}"))?;
    }
    let payload = serde_json::to_string_pretty(store)
        .map_err(|e| format!("serialize launcher token fallback failed: {e}"))?;
    fs::write(path, payload).map_err(|e| format!("write launcher token fallback failed: {e}"))?;
    #[cfg(unix)]
    {
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions)
            .map_err(|e| format!("set launcher token fallback permissions failed: {e}"))?;
    }
    Ok(())
}

fn refresh_token_lookup_keys(
    account: &LauncherAccount,
    accounts: &[LauncherAccount],
) -> Vec<String> {
    let mut keys = vec![account.id.clone(), account.username.clone()];
    for candidate in accounts
        .iter()
        .filter(|x| x.username.eq_ignore_ascii_case(&account.username))
    {
        if !keys.iter().any(|k| k == &candidate.id) {
            keys.push(candidate.id.clone());
        }
    }
    keys
}

#[cfg(debug_assertions)]
fn persist_refresh_token_debug_fallback(
    app: &tauri::AppHandle,
    account: &LauncherAccount,
    refresh_token: &str,
) -> Result<(), String> {
    let path = launcher_token_debug_fallback_path(app)?;
    let mut store = read_token_fallback_store_at_path(&path)?;
    for key in refresh_token_lookup_keys(account, std::slice::from_ref(account)) {
        store.refresh_tokens.insert(key, refresh_token.to_string());
    }
    write_token_fallback_store_at_path(&path, &store)
}

#[cfg(debug_assertions)]
fn persist_refresh_token_debug_fallback_for_key(
    app: &tauri::AppHandle,
    account_key: &str,
    refresh_token: &str,
) -> Result<(), String> {
    let key = account_key.trim();
    if key.is_empty() {
        return Ok(());
    }
    let path = launcher_token_debug_fallback_path(app)?;
    let mut store = read_token_fallback_store_at_path(&path)?;
    store
        .refresh_tokens
        .insert(key.to_string(), refresh_token.to_string());
    write_token_fallback_store_at_path(&path, &store)
}

#[cfg(debug_assertions)]
fn read_refresh_token_debug_fallback(
    app: &tauri::AppHandle,
    account: &LauncherAccount,
    accounts: &[LauncherAccount],
) -> Result<Option<String>, String> {
    let path = launcher_token_debug_fallback_path(app)?;
    let store = read_token_fallback_store_at_path(&path)?;
    for key in refresh_token_lookup_keys(account, accounts) {
        if let Some(token) = store.refresh_tokens.get(&key) {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                continue;
            }
            return Ok(Some(trimmed.to_string()));
        }
    }
    Ok(None)
}

#[cfg(debug_assertions)]
fn remove_refresh_token_debug_fallback(
    app: &tauri::AppHandle,
    account: &LauncherAccount,
) -> Result<(), String> {
    let path = launcher_token_debug_fallback_path(app)?;
    if !path.exists() {
        return Ok(());
    }
    let mut store = read_token_fallback_store_at_path(&path)?;
    let mut changed = false;
    for key in refresh_token_lookup_keys(account, std::slice::from_ref(account)) {
        if store.refresh_tokens.remove(&key).is_some() {
            changed = true;
        }
    }
    if changed {
        write_token_fallback_store_at_path(&path, &store)?;
    }
    Ok(())
}

fn persist_refresh_token_recovery_fallback(
    app: &tauri::AppHandle,
    account: &LauncherAccount,
    refresh_token: &str,
) -> Result<(), String> {
    let path = launcher_token_recovery_fallback_path(app)?;
    let mut store = read_token_fallback_store_at_path(&path)?;
    for key in refresh_token_lookup_keys(account, std::slice::from_ref(account)) {
        store.refresh_tokens.insert(key, refresh_token.to_string());
    }
    write_token_fallback_store_at_path(&path, &store)
}

fn read_refresh_token_recovery_fallback(
    app: &tauri::AppHandle,
    account: &LauncherAccount,
    accounts: &[LauncherAccount],
) -> Result<Option<String>, String> {
    let path = launcher_token_recovery_fallback_path(app)?;
    let store = read_token_fallback_store_at_path(&path)?;
    for key in refresh_token_lookup_keys(account, accounts) {
        if let Some(token) = store.refresh_tokens.get(&key) {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                continue;
            }
            return Ok(Some(trimmed.to_string()));
        }
    }
    Ok(None)
}

fn remove_refresh_token_recovery_fallback(
    app: &tauri::AppHandle,
    account: &LauncherAccount,
) -> Result<(), String> {
    let path = launcher_token_recovery_fallback_path(app)?;
    if !path.exists() {
        return Ok(());
    }
    let mut store = read_token_fallback_store_at_path(&path)?;
    let mut changed = false;
    for key in refresh_token_lookup_keys(account, std::slice::from_ref(account)) {
        if store.refresh_tokens.remove(&key).is_some() {
            changed = true;
        }
    }
    if changed {
        write_token_fallback_store_at_path(&path, &store)?;
    }
    Ok(())
}

fn remove_refresh_token_recovery_fallback_for_key(
    app: &tauri::AppHandle,
    account_key: &str,
) -> Result<(), String> {
    let key = account_key.trim();
    if key.is_empty() {
        return Ok(());
    }
    let path = launcher_token_recovery_fallback_path(app)?;
    if !path.exists() {
        return Ok(());
    }
    let mut store = read_token_fallback_store_at_path(&path)?;
    if store.refresh_tokens.remove(key).is_some() {
        write_token_fallback_store_at_path(&path, &store)?;
    }
    Ok(())
}

fn read_index(instances_dir: &Path) -> Result<InstanceIndex, String> {
    let p = index_path(instances_dir);
    if !p.exists() {
        return Ok(InstanceIndex::default());
    }
    let s = fs::read_to_string(&p).map_err(|e| format!("read index failed: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("parse index failed: {e}"))
}

fn default_true() -> bool {
    true
}

fn default_memory_mb() -> u32 {
    4096
}

fn default_graphics_preset() -> String {
    "Balanced".to_string()
}

fn default_sync_minecraft_settings_target() -> String {
    "none".to_string()
}

fn default_discord_presence_detail_level() -> String {
    "minimal".to_string()
}

fn default_world_backup_interval_minutes() -> u32 {
    DEFAULT_WORLD_BACKUP_INTERVAL_MINUTES
}

fn default_world_backup_retention_count() -> u32 {
    DEFAULT_WORLD_BACKUP_RETENTION_COUNT
}

fn default_snapshot_retention_count() -> u32 {
    DEFAULT_SNAPSHOT_RETENTION_COUNT
}

fn default_snapshot_max_age_days() -> u32 {
    DEFAULT_SNAPSHOT_MAX_AGE_DAYS
}

fn default_update_check_cadence() -> String {
    "daily".to_string()
}

fn normalize_update_check_cadence(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "off" => "off".to_string(),
        "hourly" => "hourly".to_string(),
        "every_3_hours" | "3h" => "every_3_hours".to_string(),
        "every_6_hours" | "6h" => "every_6_hours".to_string(),
        "every_12_hours" | "12h" => "every_12_hours".to_string(),
        "weekly" => "weekly".to_string(),
        _ => "daily".to_string(),
    }
}

fn default_update_auto_apply_mode() -> String {
    "never".to_string()
}

fn normalize_update_auto_apply_mode(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "opt_in_instances" | "opt-in" | "instance_opt_in" => "opt_in_instances".to_string(),
        "all_instances" | "all" => "all_instances".to_string(),
        _ => "never".to_string(),
    }
}

fn default_update_apply_scope() -> String {
    "scheduled_only".to_string()
}

fn default_auto_trigger_mic_permission_prompt() -> bool {
    true
}

fn normalize_update_apply_scope(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "scheduled_and_manual" | "scheduled+manual" | "scheduled_and_check_now" => {
            "scheduled_and_manual".to_string()
        }
        _ => "scheduled_only".to_string(),
    }
}

fn normalize_discord_presence_detail_level(input: &str) -> String {
    match input.trim().to_ascii_lowercase().as_str() {
        "expanded" | "full" | "instance" => "expanded".to_string(),
        _ => "minimal".to_string(),
    }
}

fn default_content_type_mods() -> String {
    "mods".to_string()
}

fn default_target_scope_instance() -> String {
    "instance".to_string()
}

fn normalize_lock_content_type(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "mods" | "mod" => "mods".to_string(),
        "resourcepacks" | "resourcepack" => "resourcepacks".to_string(),
        "shaderpacks" | "shaderpack" | "shaders" | "shader" => "shaderpacks".to_string(),
        "datapacks" | "datapack" => "datapacks".to_string(),
        "modpacks" | "modpack" => "modpacks".to_string(),
        _ => "mods".to_string(),
    }
}

fn core_mod_name_from_filename(filename: &str) -> Option<String> {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_disabled = trimmed.strip_suffix(".disabled").unwrap_or(trimmed);
    let stem = Path::new(without_disabled).file_stem()?.to_str()?.trim();
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}

fn canonical_lock_entry_name(content_type: &str, filename: &str, fallback_name: &str) -> String {
    let normalized = normalize_lock_content_type(content_type);
    if normalized == "mods" {
        if let Some(core_name) = core_mod_name_from_filename(filename) {
            return core_name;
        }
        let trimmed_filename = filename.trim();
        if !trimmed_filename.is_empty() {
            return trimmed_filename.to_string();
        }
    }
    let fallback = fallback_name.trim();
    if !fallback.is_empty() {
        return fallback.to_string();
    }
    if normalized == "mods" {
        "mod".to_string()
    } else {
        "content".to_string()
    }
}

fn content_type_display_name(content_type: &str) -> &'static str {
    match normalize_lock_content_type(content_type).as_str() {
        "resourcepacks" => "resourcepack",
        "shaderpacks" => "shaderpack",
        "datapacks" => "datapack",
        "modpacks" => "modpack",
        _ => "mod",
    }
}

fn normalize_target_scope(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "world" => "world".to_string(),
        _ => "instance".to_string(),
    }
}

fn write_index(instances_dir: &Path, idx: &InstanceIndex) -> Result<(), String> {
    fs::create_dir_all(instances_dir).map_err(|e| format!("mkdir instances dir failed: {e}"))?;
    let p = index_path(instances_dir);
    let s =
        serde_json::to_string_pretty(idx).map_err(|e| format!("serialize index failed: {e}"))?;
    fs::write(&p, s).map_err(|e| format!("write index failed: {e}"))
}

fn read_lockfile(instances_dir: &Path, instance_id: &str) -> Result<Lockfile, String> {
    let p = lock_path(instances_dir, instance_id);
    if !p.exists() {
        return Ok(Lockfile::default());
    }
    let s = fs::read_to_string(&p).map_err(|e| format!("read lockfile failed: {e}"))?;
    let mut lock: Lockfile =
        serde_json::from_str(&s).map_err(|e| format!("parse lockfile failed: {e}"))?;
    if lock.version < 2 {
        lock.version = 2;
    }
    for entry in &mut lock.entries {
        entry.content_type = normalize_lock_content_type(&entry.content_type);
        entry.target_scope = normalize_target_scope(&entry.target_scope);
        entry.name = canonical_lock_entry_name(&entry.content_type, &entry.filename, &entry.name);
        if entry.content_type != "datapacks" {
            entry.target_worlds.clear();
            if entry.target_scope == "world" {
                entry.target_scope = "instance".to_string();
            }
        } else if entry.target_scope != "world" {
            entry.target_scope = "world".to_string();
        }
    }
    Ok(lock)
}

fn write_lockfile(instances_dir: &Path, instance_id: &str, lock: &Lockfile) -> Result<(), String> {
    let p = lock_path(instances_dir, instance_id);
    let parent = p.parent().ok_or("invalid lockfile path")?.to_path_buf();
    fs::create_dir_all(parent).map_err(|e| format!("mkdir instance dir failed: {e}"))?;
    let mut normalized = lock.clone();
    normalized.version = 2;
    for entry in &mut normalized.entries {
        entry.content_type = normalize_lock_content_type(&entry.content_type);
        entry.target_scope = normalize_target_scope(&entry.target_scope);
        entry.name = canonical_lock_entry_name(&entry.content_type, &entry.filename, &entry.name);
        if entry.content_type != "datapacks" {
            entry.target_worlds.clear();
            if entry.target_scope == "world" {
                entry.target_scope = "instance".to_string();
            }
        }
    }
    let s = serde_json::to_string_pretty(&normalized)
        .map_err(|e| format!("serialize lockfile failed: {e}"))?;
    fs::write(&p, s).map_err(|e| format!("write lockfile failed: {e}"))
}

fn snapshots_dir(instance_dir: &Path) -> PathBuf {
    instance_dir.join("snapshots")
}

fn snapshot_content_zip_path(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join("content.zip")
}

fn snapshot_lock_path(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join("lock.json")
}

fn snapshot_meta_path(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join("meta.json")
}

fn snapshot_allowed_root(name: &str) -> bool {
    matches!(name, "mods" | "resourcepacks" | "shaderpacks" | "saves")
}

fn add_dir_recursive_to_zip(
    zip: &mut zip::ZipWriter<File>,
    root: &Path,
    current: &Path,
    opts: FileOptions,
    count: &mut usize,
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(current)
        .map_err(|e| format!("read dir '{}' failed: {e}", current.display()))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read dir entry failed: {e}"))?;
        let path = ent.path();
        let meta = ent
            .metadata()
            .map_err(|e| format!("read metadata '{}' failed: {e}", path.display()))?;
        if meta.is_dir() {
            add_dir_recursive_to_zip(zip, root, &path, opts, count)?;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|_| "failed to compute relative snapshot path".to_string())?;
        let rel_text = rel
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        if rel_text.is_empty() {
            continue;
        }
        zip.start_file(rel_text, opts)
            .map_err(|e| format!("zip start file failed: {e}"))?;
        let data = fs::read(&path).map_err(|e| format!("read snapshot source file failed: {e}"))?;
        zip.write_all(&data)
            .map_err(|e| format!("zip write failed: {e}"))?;
        *count += 1;
    }
    Ok(())
}

fn create_instance_content_zip(instance_dir: &Path, zip_path: &Path) -> Result<usize, String> {
    let parent = zip_path
        .parent()
        .ok_or_else(|| "invalid snapshot zip path".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("mkdir snapshot dir failed: {e}"))?;
    let file = File::create(zip_path).map_err(|e| format!("create snapshot zip failed: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0usize;

    for dir_name in ["mods", "resourcepacks", "shaderpacks"] {
        let dir = instance_dir.join(dir_name);
        add_dir_recursive_to_zip(&mut zip, instance_dir, &dir, opts, &mut count)?;
    }
    let saves = instance_dir.join("saves");
    if saves.exists() {
        let worlds = fs::read_dir(&saves).map_err(|e| format!("read saves dir failed: {e}"))?;
        for world in worlds {
            let world = world.map_err(|e| format!("read saves entry failed: {e}"))?;
            let world_path = world.path();
            if !world_path.is_dir() {
                continue;
            }
            let dp_dir = world_path.join("datapacks");
            add_dir_recursive_to_zip(&mut zip, instance_dir, &dp_dir, opts, &mut count)?;
        }
    }

    zip.finish()
        .map_err(|e| format!("finalize snapshot zip failed: {e}"))?;
    Ok(count)
}

fn restore_instance_content_zip(zip_path: &Path, instance_dir: &Path) -> Result<usize, String> {
    for dir_name in ["mods", "resourcepacks", "shaderpacks"] {
        let dir = instance_dir.join(dir_name);
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .map_err(|e| format!("clear '{}' failed: {e}", dir.display()))?;
        }
        fs::create_dir_all(&dir).map_err(|e| format!("mkdir '{}' failed: {e}", dir.display()))?;
    }
    let saves = instance_dir.join("saves");
    if saves.exists() {
        let worlds = fs::read_dir(&saves).map_err(|e| format!("read saves dir failed: {e}"))?;
        for world in worlds {
            let world = world.map_err(|e| format!("read saves entry failed: {e}"))?;
            let world_path = world.path();
            if !world_path.is_dir() {
                continue;
            }
            let dp_dir = world_path.join("datapacks");
            if dp_dir.exists() {
                fs::remove_dir_all(&dp_dir).map_err(|e| format!("clear datapacks failed: {e}"))?;
            }
            fs::create_dir_all(&dp_dir).map_err(|e| format!("mkdir datapacks failed: {e}"))?;
        }
    }
    if !zip_path.exists() {
        return Ok(0);
    }

    let file = File::open(zip_path).map_err(|e| format!("open snapshot zip failed: {e}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("read snapshot zip failed: {e}"))?;
    let mut count = 0usize;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("read snapshot zip entry failed: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let rel = name.to_string_lossy().replace('\\', "/");
        let parts: Vec<&str> = rel.split('/').filter(|p| !p.trim().is_empty()).collect();
        if parts.is_empty() || !snapshot_allowed_root(parts[0]) {
            continue;
        }
        if parts[0] == "saves" && (parts.len() < 4 || parts[2] != "datapacks") {
            continue;
        }
        let out_path = instance_dir.join(parts.join("/"));
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir restore parent failed: {e}"))?;
        }
        let mut out =
            File::create(&out_path).map_err(|e| format!("restore mods file failed: {e}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("restore copy failed: {e}"))?;
        count += 1;
    }

    Ok(count)
}

fn read_snapshot_meta(snapshot_dir: &Path) -> Result<SnapshotMeta, String> {
    let raw = fs::read_to_string(snapshot_meta_path(snapshot_dir))
        .map_err(|e| format!("read snapshot metadata failed: {e}"))?;
    serde_json::from_str::<SnapshotMeta>(&raw)
        .map_err(|e| format!("parse snapshot metadata failed: {e}"))
}

fn write_snapshot_meta(snapshot_dir: &Path, meta: &SnapshotMeta) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("serialize snapshot metadata failed: {e}"))?;
    fs::write(snapshot_meta_path(snapshot_dir), raw)
        .map_err(|e| format!("write snapshot metadata failed: {e}"))
}

fn list_snapshots(instance_dir: &Path) -> Result<Vec<SnapshotMeta>, String> {
    let root = snapshots_dir(instance_dir);
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    let entries = fs::read_dir(&root).map_err(|e| format!("read snapshots dir failed: {e}"))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read snapshot dir entry failed: {e}"))?;
        let path = ent.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(meta) = read_snapshot_meta(&path) {
            out.push(meta);
        }
    }
    out.sort_by(|a, b| created_at_sort_key(&b.created_at).cmp(&created_at_sort_key(&a.created_at)));
    Ok(out)
}

fn prune_old_snapshots(instance_dir: &Path, keep: usize, max_age_days: i64) -> Result<(), String> {
    let metas = list_snapshots(instance_dir)?;
    let root = snapshots_dir(instance_dir);
    let cutoff = Utc::now()
        .timestamp()
        .saturating_sub(max_age_days.max(0).saturating_mul(86_400));
    for (idx, meta) in metas.iter().enumerate() {
        let created_at = created_at_sort_key(&meta.created_at);
        let over_count = keep > 0 && idx >= keep;
        let over_age = created_at > 0 && created_at < cutoff;
        if !over_count && !over_age {
            continue;
        }
        let dir = root.join(&meta.id);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| format!("remove old snapshot failed: {e}"))?;
        }
    }
    Ok(())
}

fn snapshot_reason_slug(reason: &str) -> String {
    let mut out = String::with_capacity(reason.len());
    let mut last_dash = false;
    for ch in reason.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let cleaned = out.trim_matches('-');
    if cleaned.is_empty() {
        return String::new();
    }
    cleaned.chars().take(32).collect()
}

fn snapshot_install_subject(project_title: Option<&str>, project_id: &str) -> String {
    let title = project_title
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| project_id.trim());
    let mut cleaned = String::with_capacity(title.len());
    for ch in title.chars() {
        if ch == ':' || ch == '\n' || ch == '\r' || ch == '\t' {
            cleaned.push(' ');
        } else {
            cleaned.push(ch);
        }
    }
    let normalized = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        project_id.trim().to_string()
    } else {
        normalized
    }
}

fn create_instance_snapshot(
    instances_dir: &Path,
    instance_id: &str,
    reason: &str,
) -> Result<SnapshotMeta, String> {
    let instance_dir = instance_dir_for_id(instances_dir, instance_id)?;
    let lock = read_lockfile(instances_dir, instance_id)?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let reason_slug = snapshot_reason_slug(reason);
    let entropy = now_millis() % 1000;
    let snapshot_id = if reason_slug.is_empty() {
        format!("snapshot-{stamp}-{entropy:03}")
    } else {
        format!("snapshot-{stamp}-{reason_slug}-{entropy:03}")
    };
    let snapshot_dir = snapshots_dir(&instance_dir).join(&snapshot_id);
    fs::create_dir_all(&snapshot_dir).map_err(|e| format!("mkdir snapshot failed: {e}"))?;

    let lock_raw = serde_json::to_string_pretty(&lock)
        .map_err(|e| format!("serialize snapshot lock failed: {e}"))?;
    fs::write(snapshot_lock_path(&snapshot_dir), lock_raw)
        .map_err(|e| format!("write snapshot lock failed: {e}"))?;

    let _ = create_instance_content_zip(&instance_dir, &snapshot_content_zip_path(&snapshot_dir))?;
    let meta = SnapshotMeta {
        id: snapshot_id,
        created_at: now_iso(),
        reason: reason.to_string(),
    };
    write_snapshot_meta(&snapshot_dir, &meta)?;
    let instance_settings = read_index(instances_dir)
        .ok()
        .and_then(|index| {
            index
                .instances
                .into_iter()
                .find(|inst| inst.id == instance_id)
                .map(|inst| normalize_instance_settings(inst.settings))
        })
        .unwrap_or_default();
    prune_old_snapshots(
        &instance_dir,
        instance_settings.snapshot_retention_count as usize,
        instance_settings.snapshot_max_age_days as i64,
    )?;
    Ok(meta)
}

fn world_backups_dir(instance_dir: &Path) -> PathBuf {
    instance_dir.join("world_backups")
}

fn world_backup_meta_path(backup_dir: &Path) -> PathBuf {
    backup_dir.join("meta.json")
}

fn world_backup_zip_path(backup_dir: &Path) -> PathBuf {
    backup_dir.join("world.zip")
}

fn read_world_backup_meta(backup_dir: &Path) -> Result<WorldBackupMeta, String> {
    let raw = fs::read_to_string(world_backup_meta_path(backup_dir))
        .map_err(|e| format!("read world backup metadata failed: {e}"))?;
    serde_json::from_str::<WorldBackupMeta>(&raw)
        .map_err(|e| format!("parse world backup metadata failed: {e}"))
}

fn write_world_backup_meta(backup_dir: &Path, meta: &WorldBackupMeta) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("serialize world backup metadata failed: {e}"))?;
    fs::write(world_backup_meta_path(backup_dir), raw)
        .map_err(|e| format!("write world backup metadata failed: {e}"))
}

fn list_world_backups(instance_dir: &Path) -> Result<Vec<WorldBackupMeta>, String> {
    let root = world_backups_dir(instance_dir);
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    let entries = fs::read_dir(&root).map_err(|e| format!("read world backups dir failed: {e}"))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read world backup dir entry failed: {e}"))?;
        let path = ent.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(meta) = read_world_backup_meta(&path) {
            out.push(meta);
        }
    }
    out.sort_by(|a, b| created_at_sort_key(&b.created_at).cmp(&created_at_sort_key(&a.created_at)));
    Ok(out)
}

fn prune_old_world_backups(instance_dir: &Path, world_id: &str, keep: usize) -> Result<(), String> {
    if keep == 0 {
        return Ok(());
    }
    let metas = list_world_backups(instance_dir)?;
    let root = world_backups_dir(instance_dir);
    let mut seen = 0usize;
    for meta in metas {
        if meta.world_id != world_id {
            continue;
        }
        seen += 1;
        if seen <= keep {
            continue;
        }
        let dir = root.join(&meta.id);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| format!("remove old world backup failed: {e}"))?;
        }
    }
    Ok(())
}

fn add_world_dir_recursive_to_zip(
    zip: &mut zip::ZipWriter<File>,
    root: &Path,
    current: &Path,
    opts: FileOptions,
    file_count: &mut usize,
    total_bytes: &mut u64,
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(current)
        .map_err(|e| format!("read dir '{}' failed: {e}", current.display()))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read dir entry failed: {e}"))?;
        let path = ent.path();
        let meta = match ent.metadata() {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.is_dir() {
            add_world_dir_recursive_to_zip(zip, root, &path, opts, file_count, total_bytes)?;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case("session.lock"))
            .unwrap_or(false)
        {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|_| "failed to compute relative world backup path".to_string())?;
        let rel_text = rel
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        if rel_text.is_empty() {
            continue;
        }
        let data = match fs::read(&path) {
            Ok(data) => data,
            Err(_) => continue,
        };
        zip.start_file(rel_text, opts)
            .map_err(|e| format!("world backup zip start file failed: {e}"))?;
        zip.write_all(&data)
            .map_err(|e| format!("world backup zip write failed: {e}"))?;
        *file_count += 1;
        *total_bytes += data.len() as u64;
    }
    Ok(())
}

fn create_world_backup_zip(world_dir: &Path, zip_path: &Path) -> Result<(usize, u64), String> {
    let parent = zip_path
        .parent()
        .ok_or_else(|| "invalid world backup zip path".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("mkdir world backup dir failed: {e}"))?;
    let file =
        File::create(zip_path).map_err(|e| format!("create world backup zip failed: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut file_count = 0usize;
    let mut total_bytes = 0u64;
    add_world_dir_recursive_to_zip(
        &mut zip,
        world_dir,
        world_dir,
        opts,
        &mut file_count,
        &mut total_bytes,
    )?;
    zip.finish()
        .map_err(|e| format!("finalize world backup zip failed: {e}"))?;
    Ok((file_count, total_bytes))
}

fn restore_world_backup_zip(zip_path: &Path, world_dir: &Path) -> Result<usize, String> {
    if !zip_path.exists() {
        return Err("World backup archive is missing".to_string());
    }
    if world_dir.exists() {
        fs::remove_dir_all(world_dir).map_err(|e| format!("clear world dir failed: {e}"))?;
    }
    fs::create_dir_all(world_dir).map_err(|e| format!("mkdir world dir failed: {e}"))?;

    let file = File::open(zip_path).map_err(|e| format!("open world backup zip failed: {e}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("read world backup zip failed: {e}"))?;
    let mut count = 0usize;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("read world backup zip entry failed: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let rel = name.to_string_lossy().replace('\\', "/");
        let parts: Vec<&str> = rel.split('/').filter(|p| !p.trim().is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let out_path = world_dir.join(parts.join("/"));
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir world restore parent failed: {e}"))?;
        }
        let mut out =
            File::create(&out_path).map_err(|e| format!("restore world file failed: {e}"))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| format!("restore world copy failed: {e}"))?;
        count += 1;
    }

    Ok(count)
}

fn create_world_backup_for_world(
    instance_dir: &Path,
    world_id: &str,
    reason: &str,
    keep_per_world: usize,
) -> Result<WorldBackupMeta, String> {
    let world_name = world_id.trim();
    if world_name.is_empty() {
        return Err("World name is empty".to_string());
    }
    let world_dir = instance_dir.join("saves").join(world_name);
    if !world_dir.exists() || !world_dir.is_dir() {
        return Err(format!("World '{}' not found", world_name));
    }

    let slug_base = sanitize_name(world_name).replace(' ', "_");
    let slug = if slug_base.is_empty() {
        "world".to_string()
    } else {
        slug_base
    };
    let backup_id = format!("wb_{}_{}", slug, now_millis());
    let backup_dir = world_backups_dir(instance_dir).join(&backup_id);
    fs::create_dir_all(&backup_dir).map_err(|e| format!("mkdir world backup failed: {e}"))?;
    let (files_count, total_bytes) =
        create_world_backup_zip(&world_dir, &world_backup_zip_path(&backup_dir))?;
    let meta = WorldBackupMeta {
        id: backup_id,
        world_id: world_name.to_string(),
        created_at: now_iso(),
        reason: reason.to_string(),
        files_count,
        total_bytes,
    };
    write_world_backup_meta(&backup_dir, &meta)?;
    prune_old_world_backups(instance_dir, world_name, keep_per_world)?;
    Ok(meta)
}

fn create_world_backups_for_instance(
    instances_dir: &Path,
    instance_id: &str,
    reason: &str,
    keep_per_world: usize,
) -> Result<usize, String> {
    let instance_dir = instance_dir_for_id(instances_dir, instance_id)?;
    let worlds = list_instance_world_names(&instance_dir)?;
    if worlds.is_empty() {
        return Ok(0);
    }
    let mut created = 0usize;
    let mut last_error: Option<String> = None;
    for world in worlds {
        match create_world_backup_for_world(&instance_dir, &world, reason, keep_per_world) {
            Ok(_) => created += 1,
            Err(e) => last_error = Some(e),
        }
    }
    if created == 0 {
        if let Some(err) = last_error {
            return Err(err);
        }
    }
    Ok(created)
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
            out.push(c);
        }
    }
    out.trim().to_string()
}

pub(crate) fn allocate_instance_folder_name(
    instances_dir: &Path,
    idx: &InstanceIndex,
    requested_name: &str,
    skip_instance_id: Option<&str>,
    allow_existing: Option<&str>,
) -> String {
    let base = normalize_instance_folder_name(requested_name);
    let mut used: HashSet<String> = HashSet::new();
    for inst in &idx.instances {
        if skip_instance_id
            .map(|skip| skip == inst.id)
            .unwrap_or(false)
        {
            continue;
        }
        used.insert(instance_folder_name_or_legacy(inst).to_ascii_lowercase());
    }
    let allow_existing_lc = allow_existing.map(|value| value.to_ascii_lowercase());
    let mut candidate = base.clone();
    let mut suffix = 2usize;
    loop {
        let key = candidate.to_ascii_lowercase();
        let candidate_exists = instances_dir.join(&candidate).exists();
        let allow_this_existing = allow_existing_lc
            .as_ref()
            .map(|value| *value == key)
            .unwrap_or(false);
        if !used.contains(&key) && (!candidate_exists || allow_this_existing) {
            return candidate;
        }
        candidate = format!("{base} ({suffix})");
        suffix += 1;
    }
}

fn migrate_instance_folder_names(
    instances_dir: &Path,
    idx: &mut InstanceIndex,
) -> Result<bool, String> {
    let mut changed = false;
    for pos in 0..idx.instances.len() {
        let current = idx.instances[pos]
            .folder_name
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if current.is_some() {
            continue;
        }
        let instance_id = idx.instances[pos].id.clone();
        let instance_name = idx.instances[pos].name.clone();
        let legacy_dir = instances_dir.join(&instance_id);
        let next_folder = allocate_instance_folder_name(
            instances_dir,
            idx,
            &instance_name,
            Some(&instance_id),
            Some(&instance_id),
        );
        if legacy_dir.exists() && legacy_dir.is_dir() && next_folder != instance_id {
            let target_dir = instances_dir.join(&next_folder);
            if !target_dir.exists() {
                if let Err(err) = fs::rename(&legacy_dir, &target_dir) {
                    eprintln!(
                        "instance folder migration skipped ({} -> {}): {}",
                        legacy_dir.display(),
                        target_dir.display(),
                        err
                    );
                    idx.instances[pos].folder_name = Some(instance_id.clone());
                    changed = true;
                    continue;
                }
            } else {
                idx.instances[pos].folder_name = Some(instance_id.clone());
                changed = true;
                continue;
            }
        }
        idx.instances[pos].folder_name = Some(next_folder);
        changed = true;
    }
    Ok(changed)
}

fn normalize_instance_settings(mut settings: InstanceSettings) -> InstanceSettings {
    settings.notes = settings.notes.trim().to_string();
    settings.sync_minecraft_settings_target =
        settings.sync_minecraft_settings_target.trim().to_string();
    if settings.sync_minecraft_settings_target.is_empty() {
        settings.sync_minecraft_settings_target = default_sync_minecraft_settings_target();
    }
    settings.java_path = settings.java_path.trim().to_string();
    settings.jvm_args = settings.jvm_args.trim().to_string();
    settings.graphics_preset = match settings.graphics_preset.trim() {
        "Performance" | "Balanced" | "Quality" => settings.graphics_preset.trim().to_string(),
        _ => default_graphics_preset(),
    };
    settings.memory_mb = settings.memory_mb.clamp(512, 65536);
    settings.world_backup_interval_minutes = settings.world_backup_interval_minutes.clamp(5, 15);
    settings.world_backup_retention_count = settings.world_backup_retention_count.clamp(1, 2);
    settings.snapshot_retention_count = settings.snapshot_retention_count.clamp(1, 20);
    settings.snapshot_max_age_days = settings.snapshot_max_age_days.clamp(1, 90);
    settings
}

fn parse_loader_for_instance(input: &str) -> Option<String> {
    match input.trim().to_lowercase().as_str() {
        "vanilla" => Some("vanilla".to_string()),
        "fabric" => Some("fabric".to_string()),
        "forge" => Some("forge".to_string()),
        "neoforge" => Some("neoforge".to_string()),
        "quilt" => Some("quilt".to_string()),
        _ => None,
    }
}

fn sanitize_filename(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c == '/' || c == '\\' || c.is_control() {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

fn github_release_query_hint(filename: &str, display_name: &str, repo: &GithubRepository) -> String {
    let sanitized_filename = sanitize_filename(filename);
    let filename_hint = sanitized_filename
        .trim()
        .trim_end_matches(".disabled")
        .trim_end_matches(".jar")
        .trim();
    if !filename_hint.is_empty() {
        return filename_hint.to_string();
    }
    if !display_name.trim().is_empty() {
        return display_name.trim().to_string();
    }
    if !repo.name.trim().is_empty() {
        return repo.name.trim().to_string();
    }
    if !repo.full_name.trim().is_empty() {
        return repo.full_name.trim().to_string();
    }
    "github".to_string()
}

fn allowed_icon_extension(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif")
}

fn image_mime_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

fn write_instance_meta(instance_dir: &Path, inst: &Instance) -> Result<(), String> {
    let meta_path = instance_dir.join("meta.json");
    let meta =
        serde_json::to_string_pretty(inst).map_err(|e| format!("serialize meta failed: {e}"))?;
    fs::write(meta_path, meta).map_err(|e| format!("write meta failed: {e}"))
}

fn clear_instance_icon_files(instance_dir: &Path) -> Result<(), String> {
    if !instance_dir.exists() {
        return Ok(());
    }
    let entries =
        fs::read_dir(instance_dir).map_err(|e| format!("read instance dir failed: {e}"))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read instance entry failed: {e}"))?;
        let path = ent.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if !lower.starts_with("icon.") {
            continue;
        }
        fs::remove_file(&path).map_err(|e| format!("remove old icon failed: {e}"))?;
    }
    Ok(())
}

fn copy_instance_icon_to_dir(icon_source: &Path, instance_dir: &Path) -> Result<String, String> {
    if !icon_source.exists() || !icon_source.is_file() {
        return Err("selected icon file does not exist".to_string());
    }

    let ext = icon_source
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.trim().to_ascii_lowercase())
        .ok_or_else(|| "icon file must have an extension".to_string())?;
    if !allowed_icon_extension(&ext) {
        return Err("icon must be png/jpg/jpeg/webp/bmp/gif".to_string());
    }

    clear_instance_icon_files(instance_dir)?;
    let target = instance_dir.join(format!("icon.{ext}"));
    fs::copy(icon_source, &target).map_err(|e| format!("copy icon failed: {e}"))?;
    Ok(target.display().to_string())
}

fn now_iso() -> String {
    Local::now().to_rfc3339()
}

fn created_at_sort_key(raw: &str) -> i64 {
    let text = raw.trim();
    if let Some(rest) = text.strip_prefix("unix:") {
        if let Ok(secs) = rest.trim().parse::<i64>() {
            return secs;
        }
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return dt.timestamp();
    }
    0
}

fn gen_id() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("inst_{n}")
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn modified_millis(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|stamp| stamp.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|dur| dur.as_millis() as i64)
        .unwrap_or(0)
}

fn normalize_relative_file_path(input: &str) -> Result<String, String> {
    let trimmed = input.trim().replace('\\', "/");
    let mut parts: Vec<String> = Vec::new();
    for part in trimmed.split('/') {
        let clean = part.trim();
        if clean.is_empty() || clean == "." {
            continue;
        }
        if clean == ".." {
            return Err("Path traversal is not allowed".to_string());
        }
        parts.push(clean.to_string());
    }
    if parts.is_empty() {
        return Err("File path is required".to_string());
    }
    Ok(parts.join("/"))
}

fn world_root_dir(
    instances_dir: &Path,
    instance_id: &str,
    world_id: &str,
) -> Result<PathBuf, String> {
    let _ = find_instance(instances_dir, instance_id)?;
    let world_name = world_id.trim();
    if world_name.is_empty() {
        return Err("World ID is required".to_string());
    }
    if world_name.contains('/')
        || world_name.contains('\\')
        || world_name == "."
        || world_name == ".."
    {
        return Err("Invalid world ID".to_string());
    }
    let saves_dir = instance_dir_for_id(instances_dir, instance_id)?.join("saves");
    let world_dir = saves_dir.join(world_name);
    if !world_dir.exists() || !world_dir.is_dir() {
        return Err(format!(
            "World '{}' was not found in this instance.",
            world_name
        ));
    }
    let world_meta = fs::symlink_metadata(&world_dir)
        .map_err(|e| format!("read world path metadata failed: {e}"))?;
    if world_meta.file_type().is_symlink() {
        return Err(
            "Symlinked world folders are not supported for live config editing.".to_string(),
        );
    }
    let resolved_world =
        fs::canonicalize(&world_dir).map_err(|e| format!("resolve world path failed: {e}"))?;
    let resolved_saves =
        fs::canonicalize(&saves_dir).map_err(|e| format!("resolve saves path failed: {e}"))?;
    let _ = resolved_world
        .strip_prefix(&resolved_saves)
        .map_err(|_| "World path escapes instance saves directory".to_string())?;
    Ok(resolved_world)
}

fn resolve_world_file_path(
    world_root: &Path,
    relative_path: &str,
    must_exist: bool,
) -> Result<(PathBuf, String), String> {
    let normalized = normalize_relative_file_path(relative_path)?;
    let candidate = world_root.join(&normalized);
    if must_exist {
        if !candidate.exists() || !candidate.is_file() {
            return Err("World file was not found".to_string());
        }
        let resolved =
            fs::canonicalize(&candidate).map_err(|e| format!("resolve world file failed: {e}"))?;
        if resolved
            .strip_prefix(world_root)
            .map_err(|_| "World file path escapes selected world".to_string())?
            .as_os_str()
            .is_empty()
        {
            return Err("Invalid world file path".to_string());
        }
        return Ok((resolved, normalized));
    }
    let parent = candidate
        .parent()
        .ok_or_else(|| "Invalid world file path".to_string())?;
    let resolved_parent =
        fs::canonicalize(parent).map_err(|e| format!("resolve world file parent failed: {e}"))?;
    let _ = resolved_parent
        .strip_prefix(world_root)
        .map_err(|_| "World file path escapes selected world".to_string())?;
    Ok((candidate, normalized))
}

fn infer_world_file_kind(path: &Path, text_like: bool) -> String {
    let lower = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match lower.as_str() {
        "json" => "json".to_string(),
        "toml" => "toml".to_string(),
        "properties" => "properties".to_string(),
        "txt" => "txt".to_string(),
        "cfg" | "conf" | "ini" | "yaml" | "yml" | "mcmeta" | "lang" | "log" => "text".to_string(),
        "dat" | "nbt" | "mca" | "png" | "jpg" | "jpeg" | "webp" | "gif" | "ogg" | "mp3" | "mp4" => {
            "binary".to_string()
        }
        _ => {
            if text_like {
                "text".to_string()
            } else {
                "binary".to_string()
            }
        }
    }
}

fn file_is_text_like(path: &Path, sample: &[u8]) -> bool {
    if sample.is_empty() {
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        return matches!(
            ext.as_str(),
            "json"
                | "toml"
                | "properties"
                | "txt"
                | "cfg"
                | "conf"
                | "ini"
                | "yaml"
                | "yml"
                | "mcmeta"
                | "lang"
                | "log"
        );
    }
    if sample.iter().any(|b| *b == 0) {
        return false;
    }
    std::str::from_utf8(sample).is_ok()
}

fn describe_non_editable_reason(kind: &str, text_like: bool) -> Option<String> {
    if kind == "binary" || !text_like {
        Some("Binary or unsupported file type.".to_string())
    } else {
        None
    }
}

fn format_binary_preview(sample: &[u8], total_bytes: u64, kind: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Read-only {kind} file\nSize: {total_bytes} bytes\nShowing first {} byte(s)\n\n",
        sample.len()
    ));
    for (line_idx, chunk) in sample.chunks(16).enumerate() {
        let offset = line_idx * 16;
        let mut hex = String::new();
        let mut ascii = String::new();
        for byte in chunk {
            hex.push_str(&format!("{byte:02x} "));
            let ch = if (32..=126).contains(byte) {
                char::from(*byte)
            } else {
                '.'
            };
            ascii.push(ch);
        }
        out.push_str(&format!(
            "{offset:08x}  {:<48} |{}|\n",
            hex.trim_end(),
            ascii
        ));
    }
    if (sample.len() as u64) < total_bytes {
        out.push_str("\n... truncated ...");
    }
    out
}

fn infer_local_name(filename: &str) -> String {
    let base = filename.strip_suffix(".jar").unwrap_or(filename);
    let mut out = String::with_capacity(base.len());
    let mut prev_space = false;
    for c in base.chars() {
        let mapped = if c == '_' || c == '-' || c == '.' {
            ' '
        } else {
            c
        };
        if mapped.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(mapped);
            prev_space = false;
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "Local mod".to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Debug, Clone)]
struct LocalImportedProviderMatch {
    source: String,
    project_id: String,
    version_id: String,
    name: String,
    version_number: String,
    hashes: HashMap<String, String>,
    confidence: String,
    reason: String,
}

impl LocalImportedProviderMatch {
    fn to_provider_candidate(&self) -> ProviderCandidate {
        ProviderCandidate {
            source: self.source.clone(),
            project_id: self.project_id.clone(),
            version_id: self.version_id.clone(),
            name: self.name.clone(),
            version_number: self.version_number.clone(),
            confidence: Some(self.confidence.clone()),
            reason: Some(self.reason.clone()),
        }
    }
}

fn github_manual_unverified_hint_is_promotable(
    confidence: &str,
    version_id: &str,
    reason: &str,
) -> bool {
    if !confidence.trim().eq_ignore_ascii_case("manual") {
        return false;
    }
    if !version_id.trim().eq_ignore_ascii_case("gh_repo_unverified") {
        return false;
    }
    let lower = reason.trim().to_ascii_lowercase();
    if !lower.contains("direct metadata repo hint") {
        return false;
    }
    lower.contains("verification is unavailable")
        || lower.contains("verification unavailable")
        || lower.contains("temporarily unavailable")
        || lower.contains("currently unverifiable")
        || lower.contains("rate limit")
}

pub(crate) fn provider_match_is_auto_activatable(found: &LocalImportedProviderMatch) -> bool {
    let source = found.source.trim().to_ascii_lowercase();
    if source != "github" {
        return true;
    }
    if parse_github_project_id(&found.project_id).is_err() {
        return false;
    }
    matches!(
        found.confidence.trim().to_ascii_lowercase().as_str(),
        "deterministic" | "high"
    ) || github_manual_unverified_hint_is_promotable(
        &found.confidence,
        &found.version_id,
        &found.reason,
    )
}

pub(crate) fn provider_candidate_is_auto_activatable(candidate: &ProviderCandidate) -> bool {
    let source = candidate.source.trim().to_ascii_lowercase();
    if source != "github" {
        return true;
    }
    if parse_github_project_id(&candidate.project_id).is_err() {
        return false;
    }
    let confidence = candidate
        .confidence
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    matches!(confidence.as_str(), "deterministic" | "high")
        || github_manual_unverified_hint_is_promotable(
            confidence.as_str(),
            &candidate.version_id,
            candidate.reason.as_deref().unwrap_or_default(),
        )
}

fn provider_candidate_confidence_rank(candidate: &ProviderCandidate) -> i32 {
    match candidate
        .confidence
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "deterministic" => 5,
        "high" => 4,
        "medium" => 3,
        "manual" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn provider_candidate_version_rank(candidate: &ProviderCandidate) -> i32 {
    let version_id = candidate.version_id.trim().to_ascii_lowercase();
    if version_id.starts_with("gh_release:") || version_id.starts_with("cf_file:") {
        return 3;
    }
    if version_id.starts_with("gh_repo_unverified") {
        return 1;
    }
    if !version_id.is_empty() {
        return 2;
    }
    0
}

fn provider_candidate_dedup_key(candidate: &ProviderCandidate) -> Option<String> {
    let source = candidate.source.trim().to_ascii_lowercase();
    let project_id = candidate.project_id.trim().to_ascii_lowercase();
    if source.is_empty() || project_id.is_empty() {
        return None;
    }
    Some(format!("{source}:{project_id}"))
}

fn provider_candidate_is_better(
    candidate: &ProviderCandidate,
    existing: &ProviderCandidate,
) -> bool {
    let candidate_activation = provider_candidate_is_auto_activatable(candidate) as i32;
    let existing_activation = provider_candidate_is_auto_activatable(existing) as i32;
    if candidate_activation != existing_activation {
        return candidate_activation > existing_activation;
    }
    let candidate_confidence = provider_candidate_confidence_rank(candidate);
    let existing_confidence = provider_candidate_confidence_rank(existing);
    if candidate_confidence != existing_confidence {
        return candidate_confidence > existing_confidence;
    }
    let candidate_version = provider_candidate_version_rank(candidate);
    let existing_version = provider_candidate_version_rank(existing);
    if candidate_version != existing_version {
        return candidate_version > existing_version;
    }
    let candidate_reason_len = candidate.reason.as_deref().unwrap_or_default().trim().len();
    let existing_reason_len = existing.reason.as_deref().unwrap_or_default().trim().len();
    if candidate_reason_len != existing_reason_len {
        return candidate_reason_len > existing_reason_len;
    }
    candidate.version_id.len() > existing.version_id.len()
}

pub(crate) fn compact_provider_candidates(
    candidates: impl IntoIterator<Item = ProviderCandidate>,
) -> Vec<ProviderCandidate> {
    let mut dedup: HashMap<String, ProviderCandidate> = HashMap::new();
    for candidate in candidates {
        let Some(key) = provider_candidate_dedup_key(&candidate) else {
            continue;
        };
        if let Some(existing) = dedup.get(&key) {
            if provider_candidate_is_better(&candidate, existing) {
                dedup.insert(key, candidate);
            }
        } else {
            dedup.insert(key, candidate);
        }
    }
    let mut out = dedup.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| {
        provider_source_priority(&b.source)
            .cmp(&provider_source_priority(&a.source))
            .then_with(|| {
                provider_candidate_confidence_rank(b).cmp(&provider_candidate_confidence_rank(a))
            })
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.project_id.cmp(&b.project_id))
    });
    out
}

#[derive(Debug, Clone, Default)]
struct LocalMetadataHint {
    project_hint: Option<String>,
    display_name_hint: Option<String>,
    github_repo_hint: Option<String>,
}

fn extract_github_repo_slug(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok((owner, repo)) = parse_github_project_id(trimmed) {
        return Some(format!("{owner}/{repo}"));
    }
    let lower = trimmed.to_ascii_lowercase();
    let marker = "github.com/";
    let marker_idx = lower.find(marker)?;
    let tail = &trimmed[marker_idx + marker.len()..];
    let mut slug = String::new();
    for ch in tail.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '/' || ch == '.' {
            slug.push(ch);
            continue;
        }
        break;
    }
    if slug.is_empty() {
        return None;
    }
    let cleaned = slug.trim_matches('/').trim_end_matches(".git").to_string();
    let mut parts = cleaned
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().to_string());
    let owner = parts.next()?;
    let repo = parts.next()?;
    parse_github_project_id(&format!("{owner}/{repo}"))
        .ok()
        .map(|(normalized_owner, normalized_repo)| format!("{normalized_owner}/{normalized_repo}"))
}

fn normalize_loader_hint_token(raw: &str) -> Option<String> {
    let lower = raw.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    let compact = lower.replace(['-', '_', ' '], "");
    if compact == "vanilla" || compact == "minecraft" {
        return Some("vanilla".to_string());
    }
    if compact.contains("neoforge") || compact.contains("neoforged") {
        return Some("neoforge".to_string());
    }
    if compact.contains("fabric") {
        return Some("fabric".to_string());
    }
    if compact.contains("quilt") {
        return Some("quilt".to_string());
    }
    if compact.contains("forge") {
        return Some("forge".to_string());
    }
    None
}

fn loader_label_for_user(raw: &str) -> &'static str {
    let raw_lower = raw.trim().to_ascii_lowercase();
    let normalized = if raw_lower == "forge_family" {
        raw_lower
    } else {
        normalize_loader_hint_token(raw).unwrap_or(raw_lower)
    };
    match normalized.as_str() {
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "forge_family" => "Forge/NeoForge",
        "quilt" => "Quilt",
        "vanilla" => "Vanilla",
        _ => "unknown loader",
    }
}

fn detect_mod_loader_hints_from_filename(safe_filename: &str) -> HashSet<String> {
    let mut hints = HashSet::new();
    let lower = safe_filename.to_ascii_lowercase();
    if let Some(token) = normalize_loader_hint_token(&lower) {
        hints.insert(token);
    } else {
        let has_neoforge = lower.contains("neoforge")
            || lower.contains("neo-forge")
            || lower.contains("neo_forge")
            || lower.contains("neo forge");
        if has_neoforge {
            hints.insert("neoforge".to_string());
        }
        if lower.contains("fabric") {
            hints.insert("fabric".to_string());
        }
        if lower.contains("quilt") {
            hints.insert("quilt".to_string());
        }
        if lower.contains("forge") && !has_neoforge {
            hints.insert("forge".to_string());
        }
    }
    hints
}

fn detect_mod_loader_hints_from_jar(file_bytes: &[u8]) -> HashSet<String> {
    let mut hints = HashSet::new();
    let mut archive = match ZipArchive::new(Cursor::new(file_bytes)) {
        Ok(value) => value,
        Err(_) => return hints,
    };

    if archive.by_name("fabric.mod.json").is_ok() {
        hints.insert("fabric".to_string());
    }
    if archive.by_name("quilt.mod.json").is_ok() {
        hints.insert("quilt".to_string());
    }
    if archive.by_name("META-INF/neoforge.mods.toml").is_ok() {
        hints.insert("neoforge".to_string());
    }
    if archive.by_name("mcmod.info").is_ok() {
        hints.insert("forge".to_string());
    }

    if let Ok(mut mods_toml) = archive.by_name("META-INF/mods.toml") {
        let mut raw = String::new();
        if mods_toml.read_to_string(&mut raw).is_ok() {
            let lower = raw.to_ascii_lowercase();
            let has_neoforge_markers =
                lower.contains("neoforge") || lower.contains("net.neoforged");
            let has_forge_markers = lower.contains("javafml") || lower.contains("forge");
            if has_neoforge_markers {
                hints.insert("neoforge".to_string());
            }
            if has_forge_markers {
                // mods.toml/javafml is used by Forge and can also appear in NeoForge ecosystems.
                hints.insert("forge_family".to_string());
            }
            if !has_neoforge_markers && !has_forge_markers {
                // Conservative fallback for unknown mods.toml variants.
                hints.insert("forge_family".to_string());
            }
        } else {
            hints.insert("forge_family".to_string());
        }
    }

    hints
}

fn instance_loader_accepts_mod_loader(instance_loader: &str, mod_loader_hint: &str) -> bool {
    let instance_loader = parse_loader_for_instance(instance_loader)
        .unwrap_or_else(|| instance_loader.trim().to_ascii_lowercase());
    let mod_loader_raw = mod_loader_hint.trim().to_ascii_lowercase();
    let mod_loader = if mod_loader_raw == "forge_family" {
        mod_loader_raw
    } else {
        normalize_loader_hint_token(mod_loader_hint).unwrap_or(mod_loader_raw)
    };
    if instance_loader == mod_loader {
        return true;
    }
    matches!(
        (instance_loader.as_str(), mod_loader.as_str()),
        ("quilt", "fabric") | ("forge", "forge_family") | ("neoforge", "forge_family")
    )
}

fn supported_mod_loader_labels_for_instance(instance_loader: &str) -> Vec<&'static str> {
    let normalized = parse_loader_for_instance(instance_loader)
        .unwrap_or_else(|| instance_loader.trim().to_ascii_lowercase());
    match normalized.as_str() {
        "quilt" => vec!["Quilt", "Fabric"],
        "fabric" => vec!["Fabric"],
        "forge" => vec!["Forge"],
        "neoforge" => vec!["NeoForge"],
        "vanilla" => vec!["Vanilla"],
        _ => vec![loader_label_for_user(&normalized)],
    }
}

fn ensure_local_mod_loader_compatible(
    instance: &Instance,
    safe_filename: &str,
    file_bytes: &[u8],
) -> Result<(), String> {
    let mut hints = detect_mod_loader_hints_from_jar(file_bytes);
    let source_label = if hints.is_empty() {
        hints = detect_mod_loader_hints_from_filename(safe_filename);
        "filename"
    } else {
        "mod metadata"
    };
    if hints.is_empty() {
        return Ok(());
    }

    let instance_loader =
        parse_loader_for_instance(&instance.loader).unwrap_or_else(|| instance.loader.clone());
    let instance_loader = instance_loader.trim().to_ascii_lowercase();
    if hints
        .iter()
        .any(|hint| instance_loader_accepts_mod_loader(&instance_loader, hint))
    {
        return Ok(());
    }

    let mut hinted = hints
        .into_iter()
        .map(|value| loader_label_for_user(&value).to_string())
        .collect::<Vec<_>>();
    hinted.sort();
    hinted.dedup();
    let hinted_label = hinted.join(", ");
    let accepted_label = supported_mod_loader_labels_for_instance(&instance_loader).join(", ");

    Err(format!(
        "This local mod appears to target {} ({source_label}), but this instance uses {}. Supported local mod loader(s) for this instance: {}.",
        hinted_label,
        loader_label_for_user(&instance_loader),
        accepted_label
    ))
}

fn normalize_local_mod_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            out.push(ch);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn is_builtin_dependency_id(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "minecraft"
            | "java"
            | "forge"
            | "neoforge"
            | "fabricloader"
            | "fabric_loader"
            | "quilt_loader"
            | "quilted_fabric_api"
            | "fabric-api"
            | "fabric_api"
            | "fml"
    )
}

fn push_unique_string(out: &mut Vec<String>, value: Option<String>) {
    let Some(v) = value else {
        return;
    };
    if v.trim().is_empty() {
        return;
    }
    if !out.iter().any(|existing| existing.eq_ignore_ascii_case(&v)) {
        out.push(v);
    }
}

fn collect_json_mod_metadata(
    value: &serde_json::Value,
    mod_ids: &mut Vec<String>,
    required_deps: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, node) in map {
                let key_lower = key.trim().to_ascii_lowercase();
                if matches!(key_lower.as_str(), "id" | "modid" | "mod_id") {
                    if let Some(raw) = node.as_str() {
                        push_unique_string(mod_ids, normalize_local_mod_id(raw));
                    }
                }
                if matches!(
                    key_lower.as_str(),
                    "depends" | "dependencies" | "requires" | "requiredmods" | "required_mods"
                ) {
                    match node {
                        serde_json::Value::Object(dep_map) => {
                            for (dep_key, dep_value) in dep_map {
                                let include = dep_value.as_bool().unwrap_or(true)
                                    || dep_value.is_object()
                                    || dep_value.is_string()
                                    || dep_value.is_array();
                                if !include {
                                    continue;
                                }
                                let normalized = normalize_local_mod_id(dep_key);
                                if let Some(dep_id) = normalized {
                                    if !is_builtin_dependency_id(&dep_id) {
                                        push_unique_string(required_deps, Some(dep_id));
                                    }
                                }
                            }
                        }
                        serde_json::Value::Array(items) => {
                            for item in items {
                                if let Some(dep_raw) = item.as_str() {
                                    let normalized = normalize_local_mod_id(dep_raw);
                                    if let Some(dep_id) = normalized {
                                        if !is_builtin_dependency_id(&dep_id) {
                                            push_unique_string(required_deps, Some(dep_id));
                                        }
                                    }
                                }
                            }
                        }
                        serde_json::Value::String(dep_raw) => {
                            let normalized = normalize_local_mod_id(dep_raw);
                            if let Some(dep_id) = normalized {
                                if !is_builtin_dependency_id(&dep_id) {
                                    push_unique_string(required_deps, Some(dep_id));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                collect_json_mod_metadata(node, mod_ids, required_deps);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json_mod_metadata(item, mod_ids, required_deps);
            }
        }
        _ => {}
    }
}

fn parse_mod_id_from_assignment_line(raw: &str) -> Option<String> {
    let line = raw.trim();
    let lower = line.to_ascii_lowercase();
    if !(lower.starts_with("modid") || lower.starts_with("mod_id")) {
        return None;
    }
    let idx = line.find('=')?;
    normalize_local_mod_id(
        line[idx + 1..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim(),
    )
}

fn parse_mods_toml_dependency_ids(raw: &str) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for line in raw.lines() {
        if let Some(mod_id) = parse_mod_id_from_assignment_line(line) {
            if is_builtin_dependency_id(&mod_id) {
                continue;
            }
            push_unique_string(&mut out, Some(mod_id));
        }
    }
    out
}

fn parse_local_mod_analysis_from_jar(
    safe_filename: &str,
    file_bytes: &[u8],
    instance_loader: Option<&str>,
    installed_mod_ids: Option<&HashSet<String>>,
) -> LocalModAnalysis {
    let mut loader_hints: Vec<String> = detect_mod_loader_hints_from_jar(file_bytes)
        .into_iter()
        .collect();
    if loader_hints.is_empty() {
        loader_hints = detect_mod_loader_hints_from_filename(safe_filename)
            .into_iter()
            .collect();
    }
    loader_hints.sort();
    loader_hints.dedup();

    let mut mod_ids: Vec<String> = Vec::new();
    let mut required_dependencies: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut suggestions: Vec<String> = Vec::new();

    if let Ok(mut archive) = ZipArchive::new(Cursor::new(file_bytes)) {
        for path in [
            "fabric.mod.json",
            "quilt.mod.json",
            "META-INF/mods.toml",
            "META-INF/neoforge.mods.toml",
            "mcmod.info",
        ] {
            let mut file = match archive.by_name(path) {
                Ok(file) => file,
                Err(_) => continue,
            };
            let mut raw = String::new();
            if file.read_to_string(&mut raw).is_err() || raw.trim().is_empty() {
                continue;
            }
            if path.ends_with(".json") || path.ends_with("mcmod.info") {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                    collect_json_mod_metadata(&value, &mut mod_ids, &mut required_dependencies);
                }
            } else {
                for dep in parse_mods_toml_dependency_ids(&raw) {
                    push_unique_string(&mut required_dependencies, Some(dep));
                }
                if path.ends_with("mods.toml") {
                    push_unique_string(
                        &mut mod_ids,
                        parse_toml_assignment(&raw, "modid")
                            .and_then(|value| normalize_local_mod_id(&value)),
                    );
                }
            }
        }
    }

    mod_ids.sort();
    mod_ids.dedup();
    required_dependencies.sort();
    required_dependencies.dedup();

    if let Some(instance_loader_value) = instance_loader {
        if !loader_hints.is_empty()
            && !loader_hints
                .iter()
                .any(|hint| instance_loader_accepts_mod_loader(instance_loader_value, hint))
        {
            let hinted = loader_hints
                .iter()
                .map(|value| loader_label_for_user(value))
                .collect::<Vec<_>>()
                .join(", ");
            let expected =
                supported_mod_loader_labels_for_instance(instance_loader_value).join(", ");
            warnings.push(format!(
                "Loader mismatch: this file hints {} but instance loader supports {}.",
                hinted, expected
            ));
            suggestions
                .push("Move to disabled or install in a matching-loader instance.".to_string());
        }
    }

    if let Some(installed) = installed_mod_ids {
        let mut missing = required_dependencies
            .iter()
            .filter(|dep| !installed.contains(*dep))
            .cloned()
            .collect::<Vec<_>>();
        missing.sort();
        missing.dedup();
        if !missing.is_empty() {
            warnings.push(format!(
                "Missing required dependencies: {}.",
                missing.join(", ")
            ));
            suggestions.push(format!(
                "Install required dependencies: {}.",
                missing.join(", ")
            ));
        }
    }

    LocalModAnalysis {
        loader_hints,
        mod_ids,
        required_dependencies,
        warnings,
        suggestions,
        scanned_at: now_iso(),
    }
}

pub(crate) fn analyze_local_mod_file(
    safe_filename: &str,
    file_bytes: &[u8],
    instance_loader: Option<&str>,
    installed_mod_ids: Option<&HashSet<String>>,
) -> LocalModAnalysis {
    parse_local_mod_analysis_from_jar(
        safe_filename,
        file_bytes,
        instance_loader,
        installed_mod_ids,
    )
}

fn sha512_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha512::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn curseforge_murmur2_fingerprint(bytes: &[u8]) -> u32 {
    let m: u32 = 0x5bd1e995;
    let r: u32 = 24;
    let len = bytes.len() as u32;
    let mut h: u32 = 1 ^ len;
    let mut index = 0usize;

    while index + 4 <= bytes.len() {
        let mut k = (bytes[index] as u32)
            | ((bytes[index + 1] as u32) << 8)
            | ((bytes[index + 2] as u32) << 16)
            | ((bytes[index + 3] as u32) << 24);
        k = k.wrapping_mul(m);
        k ^= k >> r;
        k = k.wrapping_mul(m);
        h = h.wrapping_mul(m);
        h ^= k;
        index += 4;
    }

    match bytes.len().saturating_sub(index) {
        3 => {
            h ^= (bytes[index + 2] as u32) << 16;
            h ^= (bytes[index + 1] as u32) << 8;
            h ^= bytes[index] as u32;
            h = h.wrapping_mul(m);
        }
        2 => {
            h ^= (bytes[index + 1] as u32) << 8;
            h ^= bytes[index] as u32;
            h = h.wrapping_mul(m);
        }
        1 => {
            h ^= bytes[index] as u32;
            h = h.wrapping_mul(m);
        }
        _ => {}
    }

    h ^= h >> 13;
    h = h.wrapping_mul(m);
    h ^= h >> 15;
    h
}

fn fetch_modrinth_version_by_sha512(
    client: &Client,
    sha512: &str,
) -> Result<Option<ModrinthVersion>, String> {
    let url = format!(
        "{}/version_file/{}?algorithm=sha512",
        modrinth_api_base(),
        sha512.trim()
    );
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("modrinth hash lookup failed: {e}"))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(format!(
            "modrinth hash lookup failed with status {}",
            resp.status()
        ));
    }
    let version = resp
        .json::<ModrinthVersion>()
        .map_err(|e| format!("parse modrinth hash lookup failed: {e}"))?;
    Ok(Some(version))
}

fn curseforge_fingerprint_candidates(bytes: &[u8]) -> Vec<u32> {
    // CurseForge fingerprinting commonly hashes a whitespace-stripped byte stream.
    let filtered: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|b| !matches!(*b, 9 | 10 | 13 | 32))
        .collect();
    let mut out = Vec::with_capacity(2);
    if !filtered.is_empty() {
        out.push(curseforge_murmur2_fingerprint(&filtered));
    }
    let raw = curseforge_murmur2_fingerprint(bytes);
    if !out.iter().any(|existing| *existing == raw) {
        out.push(raw);
    }
    out
}

fn fetch_curseforge_match_by_fingerprints(
    client: &Client,
    api_key: &str,
    fingerprints: &[u32],
) -> Result<Option<(CurseforgeMod, CurseforgeFile)>, String> {
    let mut unique: Vec<u32> = Vec::with_capacity(fingerprints.len());
    for fingerprint in fingerprints {
        if !unique.iter().any(|existing| existing == fingerprint) {
            unique.push(*fingerprint);
        }
    }
    if unique.is_empty() {
        return Ok(None);
    }
    let url = format!(
        "{}/fingerprints/{}",
        CURSEFORGE_API_BASE, CURSEFORGE_GAME_ID_MINECRAFT
    );
    let body = serde_json::json!({ "fingerprints": unique });
    let resp = client
        .post(&url)
        .header("Accept", "application/json")
        .header("x-api-key", api_key)
        .json(&body)
        .send()
        .map_err(|e| format!("curseforge fingerprint lookup failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "curseforge fingerprint lookup failed with status {}",
            resp.status()
        ));
    }
    let payload = resp
        .json::<CurseforgeFingerprintResponse>()
        .map_err(|e| format!("parse curseforge fingerprint response failed: {e}"))?;
    let Some(matched) = payload.data.exact_matches.into_iter().next() else {
        return Ok(None);
    };
    let mut file = match matched.file {
        Some(file) => file,
        None => return Ok(None),
    };
    let mod_id = if matched.mod_id > 0 {
        matched.mod_id
    } else if matched.id > 0 {
        matched.id
    } else if file.mod_id > 0 {
        file.mod_id
    } else {
        0
    };
    if mod_id <= 0 {
        return Ok(None);
    }
    if file.mod_id <= 0 {
        file.mod_id = mod_id;
    }
    let project = fetch_curseforge_project(client, api_key, mod_id)?;
    Ok(Some((project, file)))
}

fn normalize_hint_token(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let lc = ch.to_ascii_lowercase();
        if lc.is_ascii_alphanumeric() || lc == '_' || lc == '-' {
            out.push(lc);
        } else if lc.is_whitespace() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn parse_toml_assignment(text: &str, key: &str) -> Option<String> {
    let key_lower = key.trim().to_ascii_lowercase();
    if key_lower.is_empty() {
        return None;
    }
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if !lower.starts_with(&format!("{key_lower}="))
            && !lower.starts_with(&format!("{key_lower} ="))
        {
            continue;
        }
        let idx = line.find('=')?;
        let value = line[idx + 1..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn parse_json_hint(raw: &str, id_keys: &[&str], name_keys: &[&str]) -> LocalMetadataHint {
    let mut hint = LocalMetadataHint::default();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return hint;
    };

    let mut stack: Vec<&serde_json::Value> = vec![&value];
    while let Some(node) = stack.pop() {
        match node {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let key = k.trim().to_ascii_lowercase();
                    if hint.project_hint.is_none() && id_keys.iter().any(|c| *c == key) {
                        if let Some(s) = v.as_str() {
                            let normalized = normalize_hint_token(s);
                            if !normalized.is_empty() {
                                hint.project_hint = Some(normalized);
                            }
                        }
                    }
                    if hint.display_name_hint.is_none() && name_keys.iter().any(|c| *c == key) {
                        if let Some(s) = v.as_str() {
                            let trimmed = s.trim();
                            if !trimmed.is_empty() {
                                hint.display_name_hint = Some(trimmed.to_string());
                            }
                        }
                    }
                    if hint.github_repo_hint.is_none() {
                        if let Some(s) = v.as_str() {
                            if key == "source"
                                || key == "repository"
                                || key == "repo"
                                || key == "homepage"
                                || key == "url"
                                || key == "issues"
                                || key == "github"
                                || s.to_ascii_lowercase().contains("github.com/")
                            {
                                hint.github_repo_hint = extract_github_repo_slug(s);
                            }
                        }
                    }
                    stack.push(v);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    stack.push(item);
                }
            }
            serde_json::Value::String(value) => {
                if hint.github_repo_hint.is_none() {
                    hint.github_repo_hint = extract_github_repo_slug(value);
                }
            }
            _ => {}
        }
        if hint.project_hint.is_some()
            && hint.display_name_hint.is_some()
            && hint.github_repo_hint.is_some()
        {
            break;
        }
    }

    hint
}

fn parse_mod_metadata_hint_from_jar(file_bytes: &[u8]) -> Option<LocalMetadataHint> {
    let mut archive = ZipArchive::new(Cursor::new(file_bytes)).ok()?;
    let candidates = [
        "fabric.mod.json",
        "quilt.mod.json",
        "META-INF/mods.toml",
        "mcmod.info",
    ];
    let mut merged = LocalMetadataHint::default();

    for path in candidates {
        let mut file = match archive.by_name(path) {
            Ok(file) => file,
            Err(_) => continue,
        };
        let mut raw = String::new();
        if file.read_to_string(&mut raw).is_err() || raw.trim().is_empty() {
            continue;
        }
        let hint = if path.ends_with(".json") {
            parse_json_hint(
                &raw,
                &["id", "modid", "slug", "project_id", "projectid"],
                &["name", "displayname", "title"],
            )
        } else if path.ends_with("mcmod.info") {
            parse_json_hint(
                &raw,
                &["modid", "id", "slug", "project_id", "projectid"],
                &["name", "displayname", "title"],
            )
        } else if path.ends_with("mods.toml") {
            LocalMetadataHint {
                project_hint: parse_toml_assignment(&raw, "modid")
                    .map(|v| normalize_hint_token(&v))
                    .filter(|v| !v.is_empty()),
                display_name_hint: parse_toml_assignment(&raw, "displayName"),
                github_repo_hint: parse_toml_assignment(&raw, "displayurl")
                    .and_then(|value| extract_github_repo_slug(&value))
                    .or_else(|| extract_github_repo_slug(&raw)),
            }
        } else {
            LocalMetadataHint::default()
        };
        if merged.project_hint.is_none() {
            merged.project_hint = hint.project_hint;
        }
        if merged.display_name_hint.is_none() {
            merged.display_name_hint = hint.display_name_hint;
        }
        if merged.github_repo_hint.is_none() {
            merged.github_repo_hint = hint.github_repo_hint;
        }
        if merged.project_hint.is_some()
            && merged.display_name_hint.is_some()
            && merged.github_repo_hint.is_some()
        {
            break;
        }
    }

    if merged.project_hint.is_none()
        && merged.display_name_hint.is_none()
        && merged.github_repo_hint.is_none()
    {
        None
    } else {
        Some(merged)
    }
}

fn detect_provider_from_metadata_hint(
    client: &Client,
    safe_filename: &str,
    metadata: &LocalMetadataHint,
    sha512: &str,
) -> Vec<LocalImportedProviderMatch> {
    let Some(project_hint) = metadata.project_hint.as_ref() else {
        return vec![];
    };
    let versions = match fetch_project_versions(client, project_hint) {
        Ok(value) => value,
        Err(_) => return vec![],
    };
    let mut exact_matches: Vec<(String, ModrinthVersion, ModrinthVersionFile)> = Vec::new();

    for version in versions {
        for file in &version.files {
            if sanitize_filename(&file.filename).eq_ignore_ascii_case(safe_filename) {
                exact_matches.push((project_hint.clone(), version.clone(), file.clone()));
            }
        }
    }

    if exact_matches.len() == 1 {
        let Some((_, version, file)) = exact_matches.into_iter().next() else {
            return vec![];
        };
        let mut hashes = file.hashes.clone();
        hashes
            .entry("sha512".to_string())
            .or_insert_with(|| sha512.to_string());
        let name = metadata
            .display_name_hint
            .clone()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| version.name.clone().filter(|v| !v.trim().is_empty()))
            .or_else(|| fetch_project_title(client, project_hint))
            .unwrap_or_else(|| infer_local_name(safe_filename));
        return vec![LocalImportedProviderMatch {
            source: "modrinth".to_string(),
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            name,
            version_number: if version.version_number.trim().is_empty() {
                file.filename.clone()
            } else {
                version.version_number.clone()
            },
            hashes,
            confidence: "high".to_string(),
            reason: "Metadata hint + exact filename match on Modrinth.".to_string(),
        }];
    }

    vec![]
}

fn normalize_github_lookup_hint(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_ext = trimmed
        .trim_end_matches(".jar")
        .trim_end_matches(".disabled")
        .trim();
    if without_ext.is_empty() {
        return None;
    }
    let normalized = without_ext
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn github_lookup_token_is_version_noise(token: &str) -> bool {
    let normalized = token.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return true;
    }
    if normalized.chars().all(|ch| ch.is_ascii_digit()) {
        return true;
    }
    if matches!(
        normalized.as_str(),
        "alpha" | "beta" | "snapshot" | "release" | "final"
    ) {
        return true;
    }
    if let Some(rest) = normalized.strip_prefix("rc") {
        if !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()) {
            return true;
        }
    }
    if let Some(rest) = normalized.strip_prefix('v') {
        if !rest.is_empty()
            && rest
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '_')
        {
            return true;
        }
    }
    if let Some(rest) = normalized.strip_prefix("mc") {
        if !rest.is_empty()
            && rest
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '_')
        {
            return true;
        }
    }
    let dotted = normalized.replace('_', ".");
    if dotted.contains('.')
        && dotted.chars().any(|ch| ch.is_ascii_digit())
        && dotted.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
    {
        return true;
    }
    false
}

fn normalize_github_lookup_hint_without_versions(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_ext = trimmed
        .trim_end_matches(".jar")
        .trim_end_matches(".disabled")
        .trim();
    if without_ext.is_empty() {
        return None;
    }
    let mut tokens: Vec<String> = without_ext
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| token.len() > 1)
        .filter(|token| !github_lookup_token_is_version_noise(token))
        .collect();
    tokens.dedup();
    if tokens.is_empty() {
        return None;
    }
    Some(tokens.join(" "))
}

fn push_github_lookup_query(
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
    candidate: Option<String>,
) {
    let Some(value) = candidate else {
        return;
    };
    let key = value.trim().to_ascii_lowercase();
    if !key.is_empty() && seen.insert(key) {
        out.push(value);
    }
}

fn append_github_lookup_query_variants(
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
    raw: &str,
) {
    push_github_lookup_query(
        out,
        seen,
        normalize_github_lookup_hint_without_versions(raw),
    );
    push_github_lookup_query(out, seen, normalize_github_lookup_hint(raw));
}

fn github_lookup_queries_for_local_mod(
    safe_filename: &str,
    metadata: Option<&LocalMetadataHint>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(meta) = metadata {
        if let Some(candidate) = meta.github_repo_hint.clone() {
            if let Ok((owner, repo)) = parse_github_project_id(&candidate) {
                let normalized = format!("{owner}/{repo}");
                let repo_key = format!("repo:{}", normalized.to_ascii_lowercase());
                if seen.insert(repo_key) {
                    out.push(normalized);
                }
            }
        }
        append_github_lookup_query_variants(
            &mut out,
            &mut seen,
            meta.display_name_hint.as_deref().unwrap_or_default(),
        );
        append_github_lookup_query_variants(
            &mut out,
            &mut seen,
            meta.project_hint.as_deref().unwrap_or_default(),
        );
    }
    append_github_lookup_query_variants(
        &mut out,
        &mut seen,
        &core_mod_name_from_filename(safe_filename).unwrap_or_default(),
    );
    append_github_lookup_query_variants(&mut out, &mut seen, &infer_local_name(safe_filename));
    if out.len() > GITHUB_LOCAL_IDENTIFY_MAX_QUERY_HINTS {
        out.truncate(GITHUB_LOCAL_IDENTIFY_MAX_QUERY_HINTS);
    }
    out
}

fn github_local_asset_identity_tokens(raw: &str) -> Vec<String> {
    const NOISE: &[&str] = &[
        "mc",
        "minecraft",
        "forge",
        "fabric",
        "quilt",
        "neoforge",
        "neo",
        "loader",
        "mod",
        "mods",
        "jar",
        "server",
        "api",
        "release",
        "build",
        "snapshot",
        "beta",
        "alpha",
    ];
    let without_ext = raw
        .trim()
        .trim_end_matches(".jar")
        .trim_end_matches(".disabled")
        .trim();
    if without_ext.is_empty() {
        return vec![];
    }
    without_ext
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| {
            if token.is_empty() {
                return false;
            }
            if NOISE.contains(&token.as_str()) {
                return false;
            }
            if token.chars().all(|ch| ch.is_ascii_digit()) {
                return false;
            }
            token.len() > 1
        })
        .take(6)
        .collect()
}

fn github_local_asset_identity_key(raw: &str) -> String {
    github_local_asset_identity_tokens(raw).join("_")
}

fn github_local_asset_token_overlap_count(left: &str, right: &str) -> usize {
    let left_tokens = github_local_asset_identity_tokens(left);
    let right_tokens = github_local_asset_identity_tokens(right);
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0;
    }
    let right_set: HashSet<&str> = right_tokens.iter().map(String::as_str).collect();
    left_tokens
        .iter()
        .filter(|token| right_set.contains(token.as_str()))
        .count()
}

fn github_local_asset_key_overlap(left: &str, right: &str) -> bool {
    github_local_asset_token_overlap_count(left, right) >= 2
}

fn github_metadata_project_hint_matches_repo(
    metadata: Option<&LocalMetadataHint>,
    repo: &GithubRepository,
) -> bool {
    let Some(project_hint) = metadata.and_then(|value| value.project_hint.as_ref()) else {
        return false;
    };
    let normalized_hint = normalize_provider_match_key(project_hint);
    if normalized_hint.is_empty() {
        return false;
    }
    let repo_name = normalize_provider_match_key(&repo.name);
    let repo_full_name = normalize_provider_match_key(&repo.full_name);
    (!repo_name.is_empty()
        && (repo_name == normalized_hint || repo_name.contains(&normalized_hint)))
        || (!repo_full_name.is_empty()
            && (repo_full_name == normalized_hint || repo_full_name.contains(&normalized_hint)))
}

fn github_local_hint_texts(
    safe_filename: &str,
    query_hint: &str,
    metadata: Option<&LocalMetadataHint>,
) -> Vec<String> {
    let mut out = Vec::new();
    let push = |raw: &str, out: &mut Vec<String>| {
        let normalized = normalize_provider_match_key(raw);
        if normalized.is_empty() {
            return;
        }
        if !out.iter().any(|existing| existing == &normalized) {
            out.push(normalized);
        }
    };

    push(safe_filename, &mut out);
    push(
        &core_mod_name_from_filename(safe_filename).unwrap_or_default(),
        &mut out,
    );
    push(&infer_local_name(safe_filename), &mut out);
    push(query_hint, &mut out);
    if let Some(meta) = metadata {
        push(meta.project_hint.as_deref().unwrap_or_default(), &mut out);
        push(
            meta.display_name_hint.as_deref().unwrap_or_default(),
            &mut out,
        );
    }
    out
}

fn github_local_hint_contains_any(hints: &[String], aliases: &[&str]) -> bool {
    aliases.iter().any(|alias| {
        let normalized_alias = normalize_provider_match_key(alias);
        if normalized_alias.is_empty() {
            return false;
        }
        let compact_alias = normalized_alias.replace(' ', "");
        hints.iter().any(|hint| {
            if hint.contains(&normalized_alias) {
                return true;
            }
            let compact_hint = hint.replace(' ', "");
            !compact_alias.is_empty() && compact_hint.contains(&compact_alias)
        })
    })
}

fn github_local_known_repo_boost(
    repo: &GithubRepository,
    safe_filename: &str,
    query_hint: &str,
    metadata: Option<&LocalMetadataHint>,
) -> (i64, Option<&'static str>) {
    let repo_key = if !repo.full_name.trim().is_empty() {
        repo.full_name.trim().to_ascii_lowercase()
    } else {
        format!("{}/{}", repo.owner.login.trim(), repo.name.trim()).to_ascii_lowercase()
    };
    let hints = github_local_hint_texts(safe_filename, query_hint, metadata);
    if github_local_hint_contains_any(&hints, &["meteor", "meteor client", "meteorclient"])
        && repo_key == "meteordevelopment/meteor-client"
    {
        return (58, Some("Meteor Client ecosystem canonical repo"));
    }
    if github_local_hint_contains_any(&hints, &["baritone"]) && repo_key == "cabaletta/baritone" {
        return (56, Some("Baritone ecosystem canonical repo"));
    }
    if github_local_hint_contains_any(
        &hints,
        &["trouser streak", "trouserstreak", "trouser", "streak"],
    ) && (repo_key == "etianl/trouser-streak" || repo_key == "babbaj/trouser-streak")
    {
        return (54, Some("Trouser Streak ecosystem canonical repo"));
    }
    (0, None)
}

fn github_local_match_confidence_and_reason(
    repo: &GithubRepository,
    selection: &GithubReleaseSelection,
    safe_filename: &str,
    query_hint: &str,
    metadata: Option<&LocalMetadataHint>,
    has_direct_repo_hint: bool,
    canonical_repo_boost: i64,
    canonical_repo_reason: Option<&'static str>,
    digest_match: Option<bool>,
) -> Result<(String, String), String> {
    if digest_match == Some(false) {
        return Err("GitHub release digest mismatched local file checksum.".to_string());
    }

    let expected = sanitize_filename(safe_filename);
    if expected.is_empty() {
        return Err("Local filename is empty after sanitization.".to_string());
    }
    let sanitized_asset = sanitize_filename(&selection.asset.name);
    let expected_key = github_local_asset_identity_key(&expected);
    let asset_key = github_local_asset_identity_key(&selection.asset.name);
    let expected_tokens = github_local_asset_identity_tokens(&expected);
    let exact_filename_match = sanitized_asset.eq_ignore_ascii_case(&expected);
    let key_match = !expected_key.is_empty() && !asset_key.is_empty() && asset_key == expected_key;
    let overlap_count = github_local_asset_token_overlap_count(&selection.asset.name, &expected);
    let metadata_project_hint_match = github_metadata_project_hint_matches_repo(metadata, repo);
    let minecraft_signal = github_repo_minecraft_signal_score(repo);
    let ecosystem_signal = github_repo_mod_ecosystem_signal_score(repo);
    let repo_similarity = github_repo_query_similarity(repo, query_hint);
    let asset_similarity = github_name_similarity_score(&selection.asset.name, query_hint).max(
        github_name_similarity_score(&selection.asset.name, &expected),
    );
    let ambiguous_single_token = expected_tokens.len() <= 1;
    let strong_repo_signal = minecraft_signal >= 2
        || ecosystem_signal >= 2
        || repo.stargazers_count >= 500
        || canonical_repo_boost >= 40;

    if minecraft_signal <= 0
        && ecosystem_signal <= 0
        && !has_direct_repo_hint
        && canonical_repo_boost <= 0
    {
        return Err("Repository lacks strong Minecraft/mod ecosystem signals.".to_string());
    }
    if repo_similarity < 10
        && asset_similarity < 18
        && !has_direct_repo_hint
        && !metadata_project_hint_match
        && canonical_repo_boost <= 0
    {
        return Err(
            "Repository/name similarity is too weak for safe local identification.".to_string(),
        );
    }

    let mut hard_evidence = digest_match == Some(true)
        || exact_filename_match
        || key_match
        || has_direct_repo_hint
        || metadata_project_hint_match;
    if !hard_evidence && overlap_count >= 2 && strong_repo_signal {
        hard_evidence = true;
    }
    if !hard_evidence {
        return Err(
            "Only weak query/name similarity evidence was found (hard evidence required)."
                .to_string(),
        );
    }

    if ambiguous_single_token && digest_match != Some(true) {
        let trusted_ambiguous = has_direct_repo_hint
            || canonical_repo_boost >= 40
            || (repo.stargazers_count >= 1500
                && minecraft_signal >= 3
                && repo_similarity >= 34
                && exact_filename_match);
        if !trusted_ambiguous {
            return Err(
                "Blocked ambiguous one-token local filename match without trusted repo evidence."
                    .to_string(),
            );
        }
    }

    if repo.stargazers_count < 40
        && digest_match != Some(true)
        && !has_direct_repo_hint
        && canonical_repo_boost <= 0
    {
        let low_star_is_strong =
            exact_filename_match && repo_similarity >= 28 && minecraft_signal >= 2;
        if !low_star_is_strong {
            return Err(
                "Low-star repository requires stronger filename/metadata evidence.".to_string(),
            );
        }
    }

    let confidence = if digest_match == Some(true) {
        "deterministic".to_string()
    } else if exact_filename_match
        && (has_direct_repo_hint
            || metadata_project_hint_match
            || canonical_repo_boost > 0
            || (strong_repo_signal && repo_similarity >= 22)
            || key_match)
    {
        "high".to_string()
    } else if key_match
        && (has_direct_repo_hint
            || metadata_project_hint_match
            || canonical_repo_boost > 0
            || (overlap_count >= 2 && strong_repo_signal && repo_similarity >= 28))
    {
        "high".to_string()
    } else if has_direct_repo_hint && (exact_filename_match || key_match || overlap_count >= 2) {
        "high".to_string()
    } else {
        "medium".to_string()
    };

    let mut evidence: Vec<String> = Vec::new();
    if digest_match == Some(true) {
        evidence.push("asset digest match".to_string());
    }
    if exact_filename_match {
        evidence.push("exact asset filename match".to_string());
    } else if key_match {
        evidence.push("asset identity-key match".to_string());
    } else if overlap_count >= 2 {
        evidence.push(format!("{overlap_count} shared filename tokens"));
    }
    if has_direct_repo_hint {
        evidence.push("direct GitHub repo hint from jar metadata".to_string());
    } else if metadata_project_hint_match {
        evidence.push("jar metadata project hint matches repository".to_string());
    }
    if canonical_repo_boost > 0 {
        if let Some(label) = canonical_repo_reason {
            evidence.push(label.to_string());
        } else {
            evidence.push("known canonical ecosystem repo".to_string());
        }
    }
    if evidence.is_empty() {
        evidence.push("strict safety gate passed".to_string());
    }
    Ok((
        confidence.clone(),
        format!(
            "GitHub local identify {} confidence: {}.",
            confidence,
            evidence.join("; ")
        ),
    ))
}

fn select_github_release_for_local_file(
    repo: &GithubRepository,
    releases: &[GithubRelease],
    safe_filename: &str,
    query_hint: &str,
) -> Option<GithubReleaseSelection> {
    let expected = sanitize_filename(safe_filename);
    if expected.is_empty() {
        return None;
    }
    let expected_key = github_local_asset_identity_key(&expected);

    let mut best: Option<(bool, i64, i64, GithubReleaseSelection)> = None;
    for release in releases {
        if release.draft {
            continue;
        }
        let release_sort = github_release_sort_key(release);
        for asset in &release.assets {
            if github_release_asset_is_checksum_sidecar(&asset.name)
                || !github_release_asset_looks_like_mod_jar(&asset.name)
            {
                continue;
            }
            let sanitized_asset = sanitize_filename(&asset.name);
            let exact_filename_match = sanitized_asset.eq_ignore_ascii_case(&expected);
            let asset_key = github_local_asset_identity_key(&asset.name);
            let key_match =
                !expected_key.is_empty() && !asset_key.is_empty() && asset_key == expected_key;
            let key_overlap = github_local_asset_key_overlap(&asset.name, &expected);
            let query_similarity = github_name_similarity_score(&asset.name, query_hint)
                .max(github_name_similarity_score(&asset.name, &expected));
            if !exact_filename_match && !key_match && !key_overlap && query_similarity < 18 {
                continue;
            }
            let mut score = query_similarity
                + github_name_similarity_score(&repo.full_name, query_hint) / 2
                + (repo.stargazers_count.min(5000) / 125) as i64;
            if exact_filename_match {
                score += 220;
            } else if key_match {
                score += 120;
            } else if key_overlap {
                score += 55;
            }
            let candidate = (
                release.prerelease,
                score,
                release_sort,
                GithubReleaseSelection {
                    release: release.clone(),
                    asset: asset.clone(),
                    has_checksum_sidecar: release
                        .assets
                        .iter()
                        .any(|value| github_release_asset_is_checksum_sidecar(&value.name)),
                },
            );
            let replace = if let Some(existing) = best.as_ref() {
                if existing.0 != candidate.0 {
                    !candidate.0
                } else if existing.1 != candidate.1 {
                    candidate.1 > existing.1
                } else {
                    candidate.2 > existing.2
                }
            } else {
                true
            };
            if replace {
                best = Some(candidate);
            }
        }
    }

    best.map(|item| item.3)
}

fn github_asset_digest_matches_local_hashes(
    digests: &HashMap<String, String>,
    sha256: &str,
    sha512: &str,
) -> Option<bool> {
    if digests.is_empty() {
        return None;
    }
    let mut matched_any = false;
    if let Some(remote) = digests.get("sha256").map(|value| value.trim()) {
        if remote.is_empty() || !remote.eq_ignore_ascii_case(sha256) {
            return Some(false);
        }
        matched_any = true;
    }
    if let Some(remote) = digests.get("sha512").map(|value| value.trim()) {
        if remote.is_empty() || !remote.eq_ignore_ascii_case(sha512) {
            return Some(false);
        }
        matched_any = true;
    }
    Some(matched_any)
}

fn github_unverified_manual_candidate(
    owner: &str,
    repo_name: &str,
    name: &str,
    sha256: &str,
    sha512: &str,
    reason: String,
) -> LocalImportedProviderMatch {
    let mut hashes = HashMap::new();
    hashes.insert("sha256".to_string(), sha256.to_string());
    hashes.insert("sha512".to_string(), sha512.to_string());
    LocalImportedProviderMatch {
        source: "github".to_string(),
        project_id: github_project_key(owner, repo_name),
        version_id: "gh_repo_unverified".to_string(),
        name: name.to_string(),
        version_number: "unverified".to_string(),
        hashes,
        confidence: "manual".to_string(),
        reason,
    }
}

fn detect_provider_from_github_release_assets(
    client: &Client,
    safe_filename: &str,
    sha256: &str,
    sha512: &str,
    metadata: Option<&LocalMetadataHint>,
) -> Vec<LocalImportedProviderMatch> {
    let lookup_queries = github_lookup_queries_for_local_mod(safe_filename, metadata);
    if lookup_queries.is_empty() {
        return vec![];
    }
    let query_hint = lookup_queries
        .first()
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            core_mod_name_from_filename(safe_filename)
                .unwrap_or_else(|| infer_local_name(safe_filename))
        });
    let direct_repo_hint_key = metadata
        .and_then(|hint| hint.github_repo_hint.as_ref())
        .and_then(|value| parse_github_project_id(value).ok())
        .map(|(owner, repo)| format!("{owner}/{repo}").to_ascii_lowercase());
    let mut github_transient_issue: Option<String> = None;

    let push_repo = |repo: GithubRepository,
                     repos: &mut Vec<GithubRepository>,
                     seen_repo_keys: &mut HashSet<String>| {
        let key = repo.full_name.trim().to_ascii_lowercase();
        if key.is_empty() || !seen_repo_keys.insert(key) {
            return;
        }
        repos.push(repo);
    };
    let mut repos: Vec<GithubRepository> = Vec::new();
    let mut seen_repo_keys: HashSet<String> = HashSet::new();

    for query in &lookup_queries {
        if repos.len() >= GITHUB_LOCAL_IDENTIFY_MAX_REPO_CANDIDATES {
            break;
        }
        if let Some((owner, repo_name)) = parse_github_repo_query_candidate(query) {
            match fetch_github_repo(client, &owner, &repo_name) {
                Ok(direct_repo) => {
                    push_repo(direct_repo, &mut repos, &mut seen_repo_keys);
                }
                Err(err) => {
                    if github_error_is_auth_or_rate_limit(&err) && github_transient_issue.is_none()
                    {
                        github_transient_issue = Some(err);
                    }
                }
            }
        }
        let remaining = GITHUB_LOCAL_IDENTIFY_MAX_REPO_CANDIDATES.saturating_sub(repos.len());
        if remaining == 0 {
            break;
        }
        match search_github_repositories(client, query, remaining) {
            Ok(mut found) => {
                found.truncate(remaining);
                for repo in found {
                    push_repo(repo, &mut repos, &mut seen_repo_keys);
                }
            }
            Err(err) => {
                if github_error_is_auth_or_rate_limit(&err) && github_transient_issue.is_none() {
                    github_transient_issue = Some(err);
                }
            }
        }
    }

    repos.sort_by(|a, b| {
        let a_key = if !a.full_name.trim().is_empty() {
            a.full_name.trim().to_ascii_lowercase()
        } else {
            format!("{}/{}", a.owner.login.trim(), a.name.trim()).to_ascii_lowercase()
        };
        let b_key = if !b.full_name.trim().is_empty() {
            b.full_name.trim().to_ascii_lowercase()
        } else {
            format!("{}/{}", b.owner.login.trim(), b.name.trim()).to_ascii_lowercase()
        };
        let a_direct = direct_repo_hint_key
            .as_ref()
            .map(|value| value == &a_key)
            .unwrap_or(false);
        let b_direct = direct_repo_hint_key
            .as_ref()
            .map(|value| value == &b_key)
            .unwrap_or(false);
        let (a_boost, _) = github_local_known_repo_boost(a, safe_filename, &query_hint, metadata);
        let (b_boost, _) = github_local_known_repo_boost(b, safe_filename, &query_hint, metadata);
        b_direct
            .cmp(&a_direct)
            .then_with(|| b_boost.cmp(&a_boost))
            .then_with(|| {
                github_repo_minecraft_signal_score(b).cmp(&github_repo_minecraft_signal_score(a))
            })
            .then_with(|| {
                github_repo_query_similarity(b, &query_hint)
                    .cmp(&github_repo_query_similarity(a, &query_hint))
            })
            .then_with(|| b.stargazers_count.cmp(&a.stargazers_count))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });

    let has_configured_auth_tokens = github_has_configured_tokens();
    let release_fetch_budget = if has_configured_auth_tokens {
        GITHUB_LOCAL_IDENTIFY_MAX_RELEASE_FETCHES
    } else {
        GITHUB_LOCAL_IDENTIFY_UNAUTH_MAX_RELEASE_FETCHES
    };
    let mut release_fetches = 0usize;
    let mut matches: Vec<LocalImportedProviderMatch> = Vec::new();
    let mut direct_hint_repo_seen = false;
    for repo in repos {
        if github_repo_policy_rejection_reason(&repo).is_some() {
            continue;
        }
        let repo_identity_key = if !repo.full_name.trim().is_empty() {
            repo.full_name.trim().to_ascii_lowercase()
        } else {
            format!("{}/{}", repo.owner.login.trim(), repo.name.trim()).to_ascii_lowercase()
        };
        let has_direct_repo_hint = direct_repo_hint_key
            .as_ref()
            .map(|value| value == &repo_identity_key)
            .unwrap_or(false);
        if has_direct_repo_hint {
            direct_hint_repo_seen = true;
        }
        let (canonical_repo_boost, canonical_repo_reason) =
            github_local_known_repo_boost(&repo, safe_filename, &query_hint, metadata);
        let similarity = github_repo_query_similarity(&repo, &query_hint);
        let minecraft_signal = github_repo_minecraft_signal_score(&repo);
        let ecosystem_signal = github_repo_mod_ecosystem_signal_score(&repo);
        let ambiguous_filename = github_local_asset_identity_tokens(safe_filename).len() <= 1;
        if !has_direct_repo_hint && canonical_repo_boost <= 0 {
            if minecraft_signal <= 0 && ecosystem_signal <= 0 {
                continue;
            }
            if similarity < 12 && repo.stargazers_count < 120 {
                continue;
            }
        }
        if ambiguous_filename
            && !has_direct_repo_hint
            && canonical_repo_boost <= 0
            && (repo.stargazers_count < 1500 || minecraft_signal < 3 || similarity < 34)
        {
            continue;
        }
        let (owner, repo_name) = if !repo.full_name.trim().is_empty() {
            match parse_github_project_id(&repo.full_name) {
                Ok(value) => value,
                Err(_) => continue,
            }
        } else {
            (repo.owner.login.clone(), repo.name.clone())
        };
        if release_fetches >= release_fetch_budget && !has_direct_repo_hint {
            continue;
        }
        release_fetches = release_fetches.saturating_add(1);
        let releases = match fetch_github_releases(client, &owner, &repo_name) {
            Ok(value) => value,
            Err(err) => {
                if github_error_is_auth_or_rate_limit(&err) {
                    if github_transient_issue.is_none() {
                        github_transient_issue = Some(err.clone());
                    }
                    if has_direct_repo_hint {
                        matches.push(github_unverified_manual_candidate(
                            &owner,
                            &repo_name,
                            &github_repo_title(&repo),
                            sha256,
                            sha512,
                            format!(
                                "GitHub local identify manual candidate: direct metadata repo hint matched, but release verification is unavailable ({err})."
                            ),
                        ));
                    }
                    break;
                }
                continue;
            }
        };
        let Some(selection) =
            select_github_release_for_local_file(&repo, &releases, safe_filename, &query_hint)
        else {
            if has_direct_repo_hint {
                matches.push(github_unverified_manual_candidate(
                    &owner,
                    &repo_name,
                    &github_repo_title(&repo),
                    sha256,
                    sha512,
                    "GitHub local identify manual candidate: direct metadata repo hint matched, but no verified release asset matched the local file."
                        .to_string(),
                ));
            }
            continue;
        };

        let mut hashes = extract_github_asset_digest(&selection.asset);
        let digest_match = github_asset_digest_matches_local_hashes(&hashes, sha256, sha512);
        let (confidence, reason) = match github_local_match_confidence_and_reason(
            &repo,
            &selection,
            safe_filename,
            &query_hint,
            metadata,
            has_direct_repo_hint,
            canonical_repo_boost,
            canonical_repo_reason,
            digest_match,
        ) {
            Ok(value) => value,
            Err(_) => continue,
        };
        hashes
            .entry("sha256".to_string())
            .or_insert_with(|| sha256.to_string());
        hashes
            .entry("sha512".to_string())
            .or_insert_with(|| sha512.to_string());
        matches.push(LocalImportedProviderMatch {
            source: "github".to_string(),
            project_id: github_project_key(&owner, &repo_name),
            version_id: format!("gh_release:{}", selection.release.id),
            name: github_repo_title(&repo),
            version_number: github_release_version_label(&selection.release),
            hashes,
            confidence,
            reason,
        });
    }

    if let Some(repo_key) = direct_repo_hint_key.as_ref() {
        let hinted_project = format!("gh:{repo_key}");
        let already_has_direct_hint_candidate = matches
            .iter()
            .any(|item| item.project_id.trim().eq_ignore_ascii_case(&hinted_project));
        if !already_has_direct_hint_candidate {
            if let Ok((owner, repo_name)) = parse_github_project_id(repo_key) {
                if let Some(issue) = github_transient_issue.as_ref() {
                    matches.push(github_unverified_manual_candidate(
                        &owner,
                        &repo_name,
                        &format!("{owner}/{repo_name}"),
                        sha256,
                        sha512,
                        format!(
                            "GitHub local identify manual candidate: direct metadata repo hint found, but repository verification is unavailable ({issue})."
                        ),
                    ));
                } else if direct_hint_repo_seen {
                    matches.push(github_unverified_manual_candidate(
                        &owner,
                        &repo_name,
                        &format!("{owner}/{repo_name}"),
                        sha256,
                        sha512,
                        "GitHub local identify manual candidate: direct metadata repo hint found, but release evidence is currently unverifiable."
                            .to_string(),
                    ));
                }
            }
        }
    }

    matches
}

fn provider_match_priority(value: &LocalImportedProviderMatch) -> i32 {
    match value.confidence.trim().to_ascii_lowercase().as_str() {
        "deterministic" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

fn provider_source_priority(source: &str) -> i32 {
    match source.trim().to_ascii_lowercase().as_str() {
        "modrinth" => 3,
        "curseforge" => 2,
        "github" => 1,
        _ => 0,
    }
}

fn dedupe_provider_matches(
    mut matches: Vec<LocalImportedProviderMatch>,
) -> Vec<LocalImportedProviderMatch> {
    if matches.is_empty() {
        return matches;
    }
    matches.sort_by(|a, b| {
        provider_match_priority(b)
            .cmp(&provider_match_priority(a))
            .then_with(|| {
                provider_source_priority(&b.source).cmp(&provider_source_priority(&a.source))
            })
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.project_id.cmp(&b.project_id))
    });
    let mut out: Vec<LocalImportedProviderMatch> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for item in matches {
        let key = format!(
            "{}:{}:{}",
            item.source.trim().to_ascii_lowercase(),
            item.project_id.trim().to_ascii_lowercase(),
            item.version_id.trim().to_ascii_lowercase()
        );
        if seen.insert(key) {
            out.push(item);
        }
    }
    out
}

fn detect_provider_matches_for_local_mod(
    client: &Client,
    file_bytes: &[u8],
    safe_filename: &str,
    include_metadata_fallback: bool,
    forced_github_repo_hint: Option<&str>,
) -> Vec<LocalImportedProviderMatch> {
    let sha512 = sha512_hex(file_bytes);
    let sha256 = sha256_bytes_hex(file_bytes);
    let mut metadata_hint = if include_metadata_fallback {
        parse_mod_metadata_hint_from_jar(file_bytes)
    } else {
        None
    };
    if include_metadata_fallback {
        if let Some(forced_hint) = forced_github_repo_hint {
            if let Some(slug) = extract_github_repo_slug(forced_hint).or_else(|| {
                parse_github_project_id(forced_hint)
                    .ok()
                    .map(|(owner, repo)| format!("{owner}/{repo}"))
            }) {
                if metadata_hint.is_none() {
                    metadata_hint = Some(LocalMetadataHint::default());
                }
                if let Some(metadata) = metadata_hint.as_mut() {
                    if metadata.github_repo_hint.is_none() {
                        metadata.github_repo_hint = Some(slug);
                    }
                }
            }
        }
    }
    let mut matches: Vec<LocalImportedProviderMatch> = Vec::new();
    if let Some(api_key) = curseforge_api_key() {
        let fingerprints = curseforge_fingerprint_candidates(file_bytes);
        if let Ok(Some((project, file))) =
            fetch_curseforge_match_by_fingerprints(client, &api_key, &fingerprints)
        {
            let mut hashes = parse_cf_hashes(&file);
            hashes
                .entry("sha512".to_string())
                .or_insert_with(|| sha512.clone());
            let version_number = if file.display_name.trim().is_empty() {
                if file.file_name.trim().is_empty() {
                    "unknown".to_string()
                } else {
                    file.file_name.clone()
                }
            } else {
                file.display_name.clone()
            };
            let name = if project.name.trim().is_empty() {
                infer_local_name(safe_filename)
            } else {
                project.name.clone()
            };
            matches.push(LocalImportedProviderMatch {
                source: "curseforge".to_string(),
                project_id: format!("cf:{}", project.id),
                version_id: format!("cf_file:{}", file.id),
                name,
                version_number,
                hashes,
                confidence: "deterministic".to_string(),
                reason: "Exact CurseForge fingerprint match.".to_string(),
            });
        }
    }

    if let Ok(Some(version)) = fetch_modrinth_version_by_sha512(client, &sha512) {
        let project_id = version.project_id.trim().to_string();
        if !project_id.is_empty() {
            let matched_file = version
                .files
                .iter()
                .find(|f| {
                    f.hashes
                        .get("sha512")
                        .map(|h| h.eq_ignore_ascii_case(&sha512))
                        .unwrap_or(false)
                })
                .or_else(|| {
                    version.files.iter().find(|f| {
                        sanitize_filename(&f.filename).eq_ignore_ascii_case(safe_filename)
                    })
                })
                .or_else(|| version.files.first());
            let mut hashes = matched_file.map(|f| f.hashes.clone()).unwrap_or_default();
            hashes
                .entry("sha512".to_string())
                .or_insert_with(|| sha512.clone());
            let version_number = if version.version_number.trim().is_empty() {
                matched_file
                    .map(|f| f.filename.clone())
                    .filter(|v| !v.trim().is_empty())
                    .unwrap_or_else(|| "unknown".to_string())
            } else {
                version.version_number.clone()
            };
            let name = version
                .name
                .clone()
                .filter(|v| !v.trim().is_empty())
                .or_else(|| fetch_project_title(client, &project_id))
                .unwrap_or_else(|| infer_local_name(safe_filename));
            matches.push(LocalImportedProviderMatch {
                source: "modrinth".to_string(),
                project_id,
                version_id: version.id,
                name,
                version_number,
                hashes,
                confidence: "deterministic".to_string(),
                reason: "Exact Modrinth SHA-512 match.".to_string(),
            });
        }
    }

    if include_metadata_fallback {
        matches.extend(detect_provider_from_github_release_assets(
            client,
            safe_filename,
            &sha256,
            &sha512,
            metadata_hint.as_ref(),
        ));
    }

    if include_metadata_fallback {
        if let Some(metadata) = metadata_hint.as_ref() {
            matches.extend(detect_provider_from_metadata_hint(
                client,
                safe_filename,
                metadata,
                &sha512,
            ));
        }
    }

    dedupe_provider_matches(matches)
}

fn select_preferred_provider_match<'a>(
    matches: &'a [LocalImportedProviderMatch],
    preferred_source: Option<&str>,
) -> Option<&'a LocalImportedProviderMatch> {
    if matches.is_empty() {
        return None;
    }
    if let Some(preferred) = preferred_source
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    {
        if let Some(found) = matches
            .iter()
            .find(|item| item.source.trim().eq_ignore_ascii_case(&preferred))
        {
            return Some(found);
        }
    }
    matches.iter().max_by(|a, b| {
        provider_match_priority(a)
            .cmp(&provider_match_priority(b))
            .then_with(|| {
                provider_source_priority(&a.source).cmp(&provider_source_priority(&b.source))
            })
            .then_with(|| b.source.cmp(&a.source))
    })
}

#[cfg(test)]
mod local_provider_preference_tests {
    use super::*;

    fn sample_match(source: &str, confidence: &str, project: &str) -> LocalImportedProviderMatch {
        LocalImportedProviderMatch {
            source: source.to_string(),
            project_id: project.to_string(),
            version_id: "v1".to_string(),
            name: "Sample".to_string(),
            version_number: "1.0.0".to_string(),
            hashes: HashMap::new(),
            confidence: confidence.to_string(),
            reason: "test".to_string(),
        }
    }

    #[test]
    fn select_preferred_provider_match_prefers_modrinth_on_tie() {
        let matches = vec![
            sample_match("curseforge", "high", "cf:123"),
            sample_match("modrinth", "high", "mr:abc"),
        ];
        let selected = select_preferred_provider_match(&matches, None).expect("selected match");
        assert_eq!(selected.source, "modrinth");
    }

    #[test]
    fn to_provider_candidates_keeps_modrinth_first_on_tie() {
        let deduped = dedupe_provider_matches(vec![
            sample_match("curseforge", "high", "cf:123"),
            sample_match("modrinth", "high", "mr:abc"),
        ]);
        let candidates = to_provider_candidates(&deduped);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].source, "modrinth");
        assert_eq!(candidates[1].source, "curseforge");
    }

    #[test]
    fn provider_match_auto_activation_blocks_medium_github() {
        let github_medium = sample_match("github", "medium", "owner/repo");
        let github_high = sample_match("github", "high", "owner/repo");
        let modrinth_high = sample_match("modrinth", "high", "mr:test");
        assert!(!provider_match_is_auto_activatable(&github_medium));
        assert!(provider_match_is_auto_activatable(&github_high));
        assert!(provider_match_is_auto_activatable(&modrinth_high));
    }

    #[test]
    fn provider_match_auto_activation_allows_manual_unverified_direct_repo_hint() {
        let github_manual = LocalImportedProviderMatch {
            source: "github".to_string(),
            project_id: "gh:example/repo".to_string(),
            version_id: "gh_repo_unverified".to_string(),
            name: "Example".to_string(),
            version_number: "unverified".to_string(),
            hashes: HashMap::new(),
            confidence: "manual".to_string(),
            reason: "GitHub local identify manual candidate: direct metadata repo hint matched, but release verification is unavailable (GitHub API rate limit reached).".to_string(),
        };
        assert!(provider_match_is_auto_activatable(&github_manual));
    }

    #[test]
    fn compact_provider_candidates_dedupes_same_source_project() {
        let compacted = compact_provider_candidates(vec![
            ProviderCandidate {
                source: "github".to_string(),
                project_id: "gh:example/repo".to_string(),
                version_id: "gh_repo_unverified".to_string(),
                name: "Example Repo".to_string(),
                version_number: "unverified".to_string(),
                confidence: Some("manual".to_string()),
                reason: Some("manual".to_string()),
            },
            ProviderCandidate {
                source: "github".to_string(),
                project_id: "gh:example/repo".to_string(),
                version_id: "gh_release:42".to_string(),
                name: "Example Repo".to_string(),
                version_number: "v1.2.3".to_string(),
                confidence: Some("high".to_string()),
                reason: Some("verified".to_string()),
            },
        ]);
        assert_eq!(compacted.len(), 1);
        assert_eq!(compacted[0].version_id, "gh_release:42");
    }

    #[test]
    fn effective_updatable_provider_allows_safe_local_github_candidate() {
        let entry = LockEntry {
            source: "local".to_string(),
            project_id: "local:mods:test.jar".to_string(),
            version_id: "local_1".to_string(),
            name: "Test".to_string(),
            version_number: "local-file".to_string(),
            filename: "test.jar".to_string(),
            content_type: "mods".to_string(),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            pinned_version: None,
            enabled: true,
            hashes: HashMap::new(),
            provider_candidates: vec![ProviderCandidate {
                source: "github".to_string(),
                project_id: "gh:example/repo".to_string(),
                version_id: "gh_release:7".to_string(),
                name: "Example Repo".to_string(),
                version_number: "v1.0.0".to_string(),
                confidence: Some("high".to_string()),
                reason: Some("verified".to_string()),
            }],
            local_analysis: None,
        };
        let effective = effective_updatable_provider_for_entry(&entry, UpdateScope::AllContent)
            .expect("effective provider");
        assert_eq!(effective.source.to_ascii_lowercase(), "github");
        assert_eq!(effective.project_id, "gh:example/repo");
    }

    #[test]
    fn effective_updatable_provider_blocks_weak_local_github_candidate() {
        let entry = LockEntry {
            source: "local".to_string(),
            project_id: "local:mods:test.jar".to_string(),
            version_id: "local_1".to_string(),
            name: "Test".to_string(),
            version_number: "local-file".to_string(),
            filename: "test.jar".to_string(),
            content_type: "mods".to_string(),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            pinned_version: None,
            enabled: true,
            hashes: HashMap::new(),
            provider_candidates: vec![ProviderCandidate {
                source: "github".to_string(),
                project_id: "gh:example/repo".to_string(),
                version_id: "gh_repo_unverified".to_string(),
                name: "Example Repo".to_string(),
                version_number: "unverified".to_string(),
                confidence: Some("manual".to_string()),
                reason: Some(
                    "GitHub local identify manual candidate: direct metadata repo hint matched, but no verified release asset matched the local file."
                        .to_string(),
                ),
            }],
            local_analysis: None,
        };
        assert!(effective_updatable_provider_for_entry(&entry, UpdateScope::AllContent).is_none());
    }
}

fn to_provider_candidates(matches: &[LocalImportedProviderMatch]) -> Vec<ProviderCandidate> {
    compact_provider_candidates(matches.iter().map(|item| item.to_provider_candidate()))
}

fn detect_provider_for_local_mod(
    client: &Client,
    file_bytes: &[u8],
    safe_filename: &str,
    include_metadata_fallback: bool,
) -> Option<LocalImportedProviderMatch> {
    let matches = detect_provider_matches_for_local_mod(
        client,
        file_bytes,
        safe_filename,
        include_metadata_fallback,
        None,
    );
    let preferred = select_preferred_provider_match(&matches, None)?;
    if provider_match_is_auto_activatable(preferred) {
        Some(preferred.clone())
    } else {
        None
    }
}

fn local_entry_key(entry: &LockEntry) -> String {
    format!(
        "{}:{}:{}",
        entry.source.trim().to_lowercase(),
        normalize_lock_content_type(&entry.content_type),
        entry.project_id.trim().to_lowercase()
    )
}

fn apply_provider_match_to_lock_entry(entry: &mut LockEntry, found: &LocalImportedProviderMatch) {
    entry.source = found.source.clone();
    entry.project_id = found.project_id.clone();
    entry.version_id = found.version_id.clone();
    entry.name = canonical_lock_entry_name(&entry.content_type, &entry.filename, &found.name);
    entry.version_number = found.version_number.clone();
    entry.hashes = found.hashes.clone();
    if entry.provider_candidates.is_empty() {
        entry.provider_candidates = vec![found.to_provider_candidate()];
    }
}

fn apply_provider_candidate_to_lock_entry(entry: &mut LockEntry, candidate: &ProviderCandidate) {
    entry.source = candidate.source.clone();
    entry.project_id = candidate.project_id.clone();
    entry.version_id = candidate.version_id.clone();
    entry.name = canonical_lock_entry_name(&entry.content_type, &entry.filename, &candidate.name);
    entry.version_number = candidate.version_number.clone();
}

fn mod_paths(instance_dir: &Path, filename: &str) -> (PathBuf, PathBuf) {
    let mods_dir = instance_dir.join("mods");
    let enabled = mods_dir.join(filename);
    let disabled = mods_dir.join(format!("{filename}.disabled"));
    (enabled, disabled)
}

fn content_paths_for_type(
    instance_dir: &Path,
    content_type: &str,
    filename: &str,
) -> (PathBuf, PathBuf) {
    let dir = content_dir_for_type(instance_dir, content_type);
    let enabled = dir.join(filename);
    let disabled = dir.join(format!("{filename}.disabled"));
    (enabled, disabled)
}

fn datapack_world_paths(instance_dir: &Path, world: &str, filename: &str) -> (PathBuf, PathBuf) {
    let dir = instance_dir.join("saves").join(world).join("datapacks");
    let enabled = dir.join(filename);
    let disabled = dir.join(format!("{filename}.disabled"));
    (enabled, disabled)
}

fn content_dir_for_type(instance_dir: &Path, content_type: &str) -> PathBuf {
    match normalize_lock_content_type(content_type).as_str() {
        "resourcepacks" => instance_dir.join("resourcepacks"),
        "shaderpacks" => instance_dir.join("shaderpacks"),
        _ => instance_dir.join("mods"),
    }
}

fn supported_local_content_types() -> &'static [&'static str] {
    &["mods", "resourcepacks", "shaderpacks", "datapacks"]
}

fn is_supported_local_content_type(content_type: &str) -> bool {
    matches!(
        normalize_lock_content_type(content_type).as_str(),
        "mods" | "resourcepacks" | "shaderpacks" | "datapacks"
    )
}

fn local_file_extension_allowed(content_type: &str, ext: &str) -> bool {
    let normalized = normalize_lock_content_type(content_type);
    let ext_lc = ext.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "mods" => ext_lc == "jar",
        "resourcepacks" | "datapacks" => ext_lc == "zip",
        "shaderpacks" => ext_lc == "zip" || ext_lc == "jar",
        _ => false,
    }
}

fn local_file_extension_hint(content_type: &str) -> &'static str {
    match normalize_lock_content_type(content_type).as_str() {
        "mods" => ".jar",
        "resourcepacks" | "datapacks" => ".zip",
        "shaderpacks" => ".zip or .jar",
        _ => "supported archive",
    }
}

fn local_entry_file_read_path(
    instance_dir: &Path,
    entry: &LockEntry,
) -> Result<Option<PathBuf>, String> {
    let content_type = normalize_lock_content_type(&entry.content_type);
    match content_type.as_str() {
        "mods" => {
            let (enabled_path, disabled_path) = mod_paths(instance_dir, &entry.filename);
            if enabled_path.exists() {
                Ok(Some(enabled_path))
            } else if disabled_path.exists() {
                Ok(Some(disabled_path))
            } else {
                Ok(None)
            }
        }
        "resourcepacks" | "shaderpacks" => {
            let path = content_dir_for_type(instance_dir, &content_type).join(&entry.filename);
            if path.exists() {
                Ok(Some(path))
            } else {
                Ok(None)
            }
        }
        "datapacks" => {
            let mut worlds = entry.target_worlds.clone();
            if worlds.is_empty() {
                worlds = list_instance_world_names(instance_dir)?;
            }
            for world in worlds {
                let path = instance_dir
                    .join("saves")
                    .join(world)
                    .join("datapacks")
                    .join(&entry.filename);
                if path.exists() {
                    return Ok(Some(path));
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn entry_file_exists(instance_dir: &Path, entry: &LockEntry) -> bool {
    match normalize_lock_content_type(&entry.content_type).as_str() {
        "mods" => {
            let (enabled_path, disabled_path) = mod_paths(instance_dir, &entry.filename);
            if entry.enabled {
                enabled_path.exists() || disabled_path.exists()
            } else {
                disabled_path.exists() || enabled_path.exists()
            }
        }
        "resourcepacks" | "shaderpacks" => {
            let (enabled_path, disabled_path) =
                content_paths_for_type(instance_dir, &entry.content_type, &entry.filename);
            if entry.enabled {
                enabled_path.exists() || disabled_path.exists()
            } else {
                disabled_path.exists() || enabled_path.exists()
            }
        }
        "datapacks" => {
            if entry.target_worlds.is_empty() {
                return false;
            }
            entry.target_worlds.iter().all(|world| {
                let (enabled_path, disabled_path) =
                    datapack_world_paths(instance_dir, world, &entry.filename);
                if entry.enabled {
                    enabled_path.exists() || disabled_path.exists()
                } else {
                    disabled_path.exists() || enabled_path.exists()
                }
            })
        }
        _ => {
            let dir = content_dir_for_type(instance_dir, "mods");
            dir.join(&entry.filename).exists()
        }
    }
}

fn lock_entry_provider_candidates(entry: &LockEntry) -> Vec<ProviderCandidate> {
    let mut candidates = entry.provider_candidates.clone();
    let source = entry.source.trim().to_ascii_lowercase();
    if source == "modrinth" || source == "curseforge" || source == "github" {
        candidates.push(ProviderCandidate {
            source: entry.source.clone(),
            project_id: entry.project_id.clone(),
            version_id: entry.version_id.clone(),
            name: entry.name.clone(),
            version_number: entry.version_number.clone(),
            confidence: None,
            reason: None,
        });
    }
    compact_provider_candidates(candidates)
}

fn lock_entry_to_installed(instance_dir: &Path, entry: &LockEntry) -> InstalledMod {
    let file_exists = entry_file_exists(instance_dir, entry);
    let added_at = local_entry_file_read_path(instance_dir, entry)
        .ok()
        .flatten()
        .and_then(|path| fs::metadata(path).ok())
        .map(|meta| modified_millis(&meta))
        .unwrap_or(0);

    InstalledMod {
        source: entry.source.clone(),
        project_id: entry.project_id.clone(),
        version_id: entry.version_id.clone(),
        name: canonical_lock_entry_name(&entry.content_type, &entry.filename, &entry.name),
        version_number: entry.version_number.clone(),
        filename: entry.filename.clone(),
        content_type: normalize_lock_content_type(&entry.content_type),
        target_scope: normalize_target_scope(&entry.target_scope),
        target_worlds: entry.target_worlds.clone(),
        pinned_version: entry.pinned_version.clone(),
        enabled: entry.enabled,
        file_exists,
        added_at,
        hashes: entry.hashes.clone(),
        provider_candidates: lock_entry_provider_candidates(entry),
        local_analysis: entry.local_analysis.clone(),
    }
}

fn find_instance(instances_dir: &Path, instance_id: &str) -> Result<Instance, String> {
    let idx = read_index(instances_dir)?;
    idx.instances
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| "instance not found".to_string())
}

fn emit_install_progress(app: &tauri::AppHandle, payload: InstallProgressEvent) {
    let _ = app.emit_all("mod_install_progress", payload);
}

fn emit_launch_state(
    app: &tauri::AppHandle,
    instance_id: &str,
    launch_id: Option<&str>,
    method: &str,
    status: &str,
    message: &str,
) {
    let payload = serde_json::json!({
        "instance_id": instance_id,
        "launch_id": launch_id,
        "method": method,
        "status": status,
        "message": message
    });
    let _ = app.emit_all("instance_launch_state", payload);
}

fn clear_launch_cancel_request(
    state: &tauri::State<'_, AppState>,
    instance_id: &str,
) -> Result<(), String> {
    let mut guard = state
        .launch_cancelled
        .lock()
        .map_err(|_| "lock launch cancellation state failed".to_string())?;
    guard.remove(instance_id);
    Ok(())
}

fn mark_launch_cancel_request(
    state: &tauri::State<'_, AppState>,
    instance_id: &str,
) -> Result<(), String> {
    let mut guard = state
        .launch_cancelled
        .lock()
        .map_err(|_| "lock launch cancellation state failed".to_string())?;
    guard.insert(instance_id.to_string());
    Ok(())
}

fn is_launch_cancel_requested(
    state: &tauri::State<'_, AppState>,
    instance_id: &str,
) -> Result<bool, String> {
    let guard = state
        .launch_cancelled
        .lock()
        .map_err(|_| "lock launch cancellation state failed".to_string())?;
    Ok(guard.contains(instance_id))
}

async fn await_launch_stage_with_cancel<T, F>(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
    instance_id: &str,
    method: &str,
    stage_label: &str,
    timeout_secs: u64,
    future: F,
) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    let mut fut = Box::pin(future);
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        tokio::select! {
            result = &mut fut => return result,
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                if is_launch_cancel_requested(state, instance_id)? {
                    emit_launch_state(
                        app,
                        instance_id,
                        None,
                        method,
                        "stopped",
                        "Launch cancelled by user.",
                    );
                    clear_launch_cancel_request(state, instance_id)?;
                    return Err("Launch cancelled by user.".to_string());
                }
                if Instant::now() >= deadline {
                    let timeout_msg = format!(
                        "{} timed out after {}s. Check network/firewall and try again.",
                        stage_label, timeout_secs
                    );
                    emit_launch_state(
                        app,
                        instance_id,
                        None,
                        method,
                        "stopped",
                        &timeout_msg,
                    );
                    return Err(timeout_msg);
                }
            }
        }
    }
}

fn resolve_oauth_client_id_with_source(app: &tauri::AppHandle) -> Result<(String, String), String> {
    let settings = read_launcher_settings(app)?;
    if !settings.oauth_client_id.trim().is_empty() {
        return Ok((
            settings.oauth_client_id.trim().to_string(),
            "settings".to_string(),
        ));
    }

    if let Some(v) = std::env::var("MPM_MS_CLIENT_ID_DEFAULT")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return Ok((v, "app_default_env".to_string()));
    }

    if !DEFAULT_MS_PUBLIC_CLIENT_ID.trim().is_empty() {
        return Ok((
            DEFAULT_MS_PUBLIC_CLIENT_ID.trim().to_string(),
            "bundled_default".to_string(),
        ));
    }

    if let Some(v) = std::env::var("MPM_MS_CLIENT_ID")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return Ok((v, "legacy_env".to_string()));
    }

    Err(
        "Microsoft public client ID is missing. This is not a secret key. Configure it in Settings > Launcher > Advanced."
            .to_string(),
    )
}

fn resolve_oauth_client_id(app: &tauri::AppHandle) -> Result<String, String> {
    resolve_oauth_client_id_with_source(app).map(|v| v.0)
}

fn env_usize_override(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
}

pub(crate) fn env_worker_cap_or_default(
    key: &str,
    default: usize,
    min: usize,
    max: usize,
) -> usize {
    env_usize_override(key).unwrap_or(default).clamp(min, max)
}

fn retry_backoff_ms(attempt: usize) -> u64 {
    let attempt = attempt.max(1).min(6) as u32;
    let exp = 1_u64 << (attempt - 1);
    let base = 200_u64.saturating_mul(exp);
    let jitter = 35_u64 + ((attempt as u64 * 97) % 140);
    base.saturating_add(jitter).min(6_000)
}

fn build_http_client_once() -> Result<Client, String> {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(12))
        .timeout(Duration::from_secs(150))
        .pool_max_idle_per_host(24)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_nodelay(true)
        .build()
        .map_err(|e| format!("build http client failed: {e}"))
}

fn build_http_client() -> Result<Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    match CLIENT.get_or_init(build_http_client_once) {
        Ok(client) => Ok(client.clone()),
        Err(err) => Err(err.clone()),
    }
}

fn is_transient_network_error(err: &reqwest::Error) -> bool {
    if err.is_timeout() || err.is_connect() || err.is_request() {
        return true;
    }
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("timed out")
        || msg.contains("dns")
        || msg.contains("connection reset")
        || msg.contains("connection refused")
        || msg.contains("connection closed")
        || msg.contains("network is unreachable")
}

fn should_retry_http_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn error_mentions_forbidden(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("403") || lower.contains("forbidden")
}

pub(crate) fn github_error_is_auth_or_rate_limit(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    error_mentions_forbidden(text)
        || lower.contains("401")
        || lower.contains("unauthorized")
        || lower.contains("rate limit")
}

fn github_error_is_rate_limit(text: &str) -> bool {
    text.to_ascii_lowercase().contains("rate limit")
}

fn github_rate_limit_reset_hint_from_error(text: &str) -> Option<String> {
    let marker = "resets around ";
    let lower = text.to_ascii_lowercase();
    let start = lower.find(marker)?;
    let rest = &text[start + marker.len()..];
    let hint = rest
        .split('.')
        .next()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    Some(hint)
}

pub(crate) fn github_reason_is_transient_verification_failure(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    github_error_is_auth_or_rate_limit(text)
        || lower.contains("verification is unavailable")
        || lower.contains("verification unavailable")
        || lower.contains("temporarily unavailable")
        || lower.contains("currently unverifiable")
}

pub(crate) fn download_bytes_with_retry(
    client: &Client,
    url: &str,
    label: &str,
) -> Result<Vec<u8>, String> {
    let max_attempts = 3usize;
    let mut attempt = 0usize;
    loop {
        attempt += 1;
        match client
            .get(url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send()
        {
            Ok(mut response) => {
                let status = response.status();
                if status.is_success() {
                    let mut bytes = Vec::new();
                    response
                        .copy_to(&mut bytes)
                        .map_err(|e| format!("download read failed for {label}: {e}"))?;
                    return Ok(bytes);
                }
                if attempt < max_attempts && should_retry_http_status(status) {
                    thread::sleep(Duration::from_millis(retry_backoff_ms(attempt)));
                    continue;
                }
                return Err(format!("download failed for {label} with status {status}"));
            }
            Err(err) => {
                if attempt < max_attempts && is_transient_network_error(&err) {
                    thread::sleep(Duration::from_millis(retry_backoff_ms(attempt)));
                    continue;
                }
                return Err(format!("download failed for {label}: {err}"));
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct StreamDownloadProfile {
    attempts: usize,
    retries: usize,
    connect_tls_ms: u128,
    time_to_first_byte_ms: u128,
    transfer_ms: u128,
    disk_commit_ms: u128,
    post_process_ms: u128,
    bytes_downloaded: u64,
    content_length: Option<u64>,
}

#[derive(Debug, Clone)]
struct StreamDownloadResult {
    sha512: String,
    profile: StreamDownloadProfile,
}

fn download_profile_enabled() -> bool {
    matches!(
        std::env::var("MPM_DOWNLOAD_PROFILE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn is_transient_io_error(err: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        err.kind(),
        ErrorKind::Interrupted
            | ErrorKind::TimedOut
            | ErrorKind::UnexpectedEof
            | ErrorKind::WouldBlock
            | ErrorKind::ConnectionAborted
            | ErrorKind::ConnectionReset
            | ErrorKind::BrokenPipe
            | ErrorKind::NotConnected
    )
}

fn unknown_progress_ratio(downloaded_bytes: u64) -> f64 {
    if downloaded_bytes == 0 {
        return 0.0;
    }
    // Smoothly advance when content-length is unavailable so progress doesn't stick at 0.
    let mb = downloaded_bytes as f64 / (1024.0 * 1024.0);
    let ratio = 0.05 + (mb / (mb + 24.0)) * 0.88;
    ratio.clamp(0.05, 0.93)
}

fn format_download_meter(downloaded_bytes: u64, total_bytes: Option<u64>) -> String {
    let downloaded_mb = downloaded_bytes as f64 / (1024.0 * 1024.0);
    if let Some(total) = total_bytes {
        if total > 0 {
            let total_mb = total as f64 / (1024.0 * 1024.0);
            return format!("{downloaded_mb:.1} / {total_mb:.1} MB");
        }
    }
    format!("{downloaded_mb:.1} MB")
}

fn download_stream_to_temp_with_retry<F>(
    client: &Client,
    url: &str,
    label: &str,
    temp_path: &Path,
    mut on_progress: F,
) -> Result<StreamDownloadResult, String>
where
    F: FnMut(u64, Option<u64>),
{
    let max_attempts = 3usize;
    let mut attempt = 0usize;
    'attempt_loop: loop {
        attempt += 1;
        if temp_path.exists() {
            let _ = fs::remove_file(temp_path);
        }
        let request_started = Instant::now();
        let mut response = match client
            .get(url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send()
        {
            Ok(response) => response,
            Err(err) => {
                if attempt < max_attempts && is_transient_network_error(&err) {
                    thread::sleep(Duration::from_millis(retry_backoff_ms(attempt)));
                    continue;
                }
                return Err(format!("download failed for {label}: {err}"));
            }
        };
        let connect_tls_ms = request_started.elapsed().as_millis();
        let status = response.status();
        if !status.is_success() {
            if attempt < max_attempts && should_retry_http_status(status) {
                thread::sleep(Duration::from_millis(retry_backoff_ms(attempt)));
                continue;
            }
            return Err(format!("download failed for {label} with status {status}"));
        }

        let total_bytes = response.content_length();
        on_progress(0, total_bytes);
        let mut out = File::create(temp_path)
            .map_err(|e| format!("create temp file failed for {label}: {e}"))?;
        let mut hasher = Sha512::new();
        let mut downloaded_bytes: u64 = 0;
        let mut buf = vec![0_u8; 1024 * 1024];
        let mut first_byte_at: Option<Instant> = None;
        loop {
            let n = match response.read(&mut buf) {
                Ok(n) => n,
                Err(err) => {
                    if attempt < max_attempts && is_transient_io_error(&err) {
                        let _ = fs::remove_file(temp_path);
                        thread::sleep(Duration::from_millis(retry_backoff_ms(attempt)));
                        continue 'attempt_loop;
                    }
                    let _ = fs::remove_file(temp_path);
                    return Err(format!("read download stream failed for {label}: {err}"));
                }
            };
            if n == 0 {
                break;
            }
            if first_byte_at.is_none() {
                first_byte_at = Some(Instant::now());
            }
            out.write_all(&buf[..n])
                .map_err(|e| format!("write download stream failed for {label}: {e}"))?;
            hasher.update(&buf[..n]);
            downloaded_bytes += n as u64;
            on_progress(downloaded_bytes, total_bytes);
        }
        let disk_commit_started = Instant::now();
        out.flush()
            .map_err(|e| format!("flush download stream failed for {label}: {e}"))?;
        let disk_commit_ms = disk_commit_started.elapsed().as_millis();

        let time_to_first_byte_ms = first_byte_at
            .map(|ts| {
                ts.duration_since(request_started)
                    .as_millis()
                    .saturating_sub(connect_tls_ms)
            })
            .unwrap_or(0);
        let transfer_ms = first_byte_at
            .map(|ts| Instant::now().duration_since(ts).as_millis())
            .unwrap_or(0);
        let digest = hasher.finalize();
        let mut sha512 = String::with_capacity(digest.len() * 2);
        for byte in digest {
            sha512.push_str(&format!("{byte:02x}"));
        }
        let profile = StreamDownloadProfile {
            attempts: attempt,
            retries: attempt.saturating_sub(1),
            connect_tls_ms,
            time_to_first_byte_ms,
            transfer_ms,
            disk_commit_ms,
            post_process_ms: 0,
            bytes_downloaded: downloaded_bytes,
            content_length: total_bytes,
        };
        return Ok(StreamDownloadResult { sha512, profile });
    }
}

fn maybe_log_download_profile(label: &str, profile: &StreamDownloadProfile) {
    if !download_profile_enabled() {
        return;
    }
    let size = profile
        .content_length
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".to_string());
    eprintln!(
        "[download_profile] {label} attempts={} retries={} connect_tls={}ms ttfb={}ms transfer={}ms disk_commit={}ms post_process={}ms bytes={} content_length={}",
        profile.attempts,
        profile.retries,
        profile.connect_tls_ms,
        profile.time_to_first_byte_ms,
        profile.transfer_ms,
        profile.disk_commit_ms,
        profile.post_process_ms,
        profile.bytes_downloaded,
        size
    );
}

fn network_block_hint(url: &str) -> Option<&'static str> {
    if url.contains("xboxlive.com") {
        return Some(
            "Your network may be blocking Xbox endpoints (user.auth.xboxlive.com / xsts.auth.xboxlive.com). Native Minecraft sign-in requires these. This is common on school/work networks.",
        );
    }
    if url.contains("minecraftservices.com") {
        return Some(
            "Your network may be blocking Minecraft services endpoints. Try another network or hotspot and retry sign-in.",
        );
    }
    None
}

fn endpoint_send_error(stage: &str, url: &str, err: &reqwest::Error) -> String {
    let mut out = format!(
        "{stage} failed while calling {url}: {}",
        reqwest_error_with_causes(err)
    );
    if is_transient_network_error(err) {
        if let Some(hint) = network_block_hint(url) {
            out.push(' ');
            out.push_str(hint);
            if url.contains("xboxlive.com") {
                out.push_str(" You can still use `Launch: Prism` for this instance while native auth is blocked.");
            }
        }
    }
    out
}

fn reqwest_error_with_causes(err: &reqwest::Error) -> String {
    let mut out = err.to_string();
    let mut cur = std::error::Error::source(err);
    while let Some(next) = cur {
        out.push_str(" | caused by: ");
        out.push_str(&next.to_string());
        cur = next.source();
    }
    out
}

fn trim_error_body(raw: &str) -> String {
    let one_line = raw.replace('\n', " ").replace('\r', " ").trim().to_string();
    if one_line.len() > 280 {
        format!("{}…", &one_line[..280])
    } else {
        one_line
    }
}

fn post_json_with_retry(
    client: &Client,
    url: &str,
    body: &serde_json::Value,
    stage: &str,
    headers: &[(&str, &str)],
) -> Result<Response, String> {
    let max_attempts = 3usize;
    let mut attempt = 0usize;
    loop {
        attempt += 1;
        let mut req = client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        for (k, v) in headers {
            req = req.header(*k, *v);
        }

        match req.json(body).send() {
            Ok(resp) => return Ok(resp),
            Err(err) => {
                if attempt < max_attempts && is_transient_network_error(&err) {
                    thread::sleep(Duration::from_millis(260 * attempt as u64));
                    continue;
                }
                return Err(endpoint_send_error(stage, url, &err));
            }
        }
    }
}

fn retry_after_delay(resp: &Response, attempt: usize) -> Duration {
    let retry_after_secs = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or((attempt as u64).saturating_mul(2));
    Duration::from_secs(retry_after_secs.min(20))
}

fn post_json_with_status_retry(
    client: &Client,
    url: &str,
    body: &serde_json::Value,
    stage: &str,
    headers: &[(&str, &str)],
) -> Result<Response, String> {
    let max_attempts = 3usize;
    let mut attempt = 0usize;
    loop {
        attempt += 1;
        let resp = post_json_with_retry(client, url, body, stage, headers)?;
        if attempt < max_attempts && should_retry_http_status(resp.status()) {
            let delay = retry_after_delay(&resp, attempt);
            thread::sleep(delay);
            continue;
        }
        return Ok(resp);
    }
}

fn parse_xerr_code(body: &str) -> Option<i64> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("XErr").cloned())
        .and_then(|v| v.as_i64())
}

fn explain_xerr_code(xerr: i64) -> Option<&'static str> {
    match xerr {
        2148916233 => {
            Some("This Microsoft account does not have an Xbox profile or Minecraft entitlement.")
        }
        2148916235 => Some("Xbox Live is unavailable in your current country/region."),
        2148916238 => {
            Some("This Microsoft account is underaged and not linked to a family account.")
        }
        2148916236 => {
            Some("This Microsoft account requires proof of age before it can sign into Xbox.")
        }
        2148916237 => Some("This Microsoft account has reached its allowed playtime limit."),
        2148916227 => Some("This Microsoft account is banned from Xbox services."),
        2148916229 => {
            Some("Guardian/parental controls currently block online play for this account.")
        }
        2148916234 => Some("This Microsoft account must accept Xbox terms first."),
        _ => None,
    }
}

fn normalize_microsoft_login_error(
    error_code: &str,
    error_desc: &str,
    client_id_source: &str,
) -> String {
    let code = error_code.to_ascii_lowercase();
    let desc = error_desc.to_ascii_lowercase();
    let client_hint = if client_id_source == "settings" {
        "This usually means your current OAuth client ID is not allowed for Minecraft auth."
    } else {
        "This can happen when the bundled client ID is restricted by Microsoft tenant policy."
    };

    if desc.contains("not permitted to consent")
        || desc.contains("first party application")
        || desc.contains("pre-authorization")
        || desc.contains("user does not have consent")
        || code.contains("invalid_request")
    {
        return format!(
            "Microsoft sign-in was blocked by consent policy. {client_hint} This commonly happens with school/work accounts. Use a personal Microsoft account, or set your own Azure Public Client ID in Settings > Launcher > Advanced > OAuth client ID."
        );
    }

    if desc.contains("application with identifier")
        || desc.contains("unauthorized_client")
        || desc.contains("aadsts700016")
    {
        return "Microsoft sign-in failed: OAuth client ID is invalid for this tenant. Set your own Azure Public Client ID in Settings > Launcher > Advanced.".to_string();
    }

    if code.contains("access_denied") || desc.contains("access denied") {
        return "Microsoft sign-in was denied. Please complete consent in browser, then try again."
            .to_string();
    }

    format!("Microsoft device token polling failed: {error_desc}")
}

fn summarize_cosmetics(items: &[McProfileCosmetic]) -> Vec<AccountCosmeticSummary> {
    items
        .iter()
        .filter(|x| !x.url.trim().is_empty())
        .map(|x| AccountCosmeticSummary {
            id: x.id.clone(),
            state: x.state.clone(),
            url: x.url.clone(),
            alias: x.alias.clone(),
            variant: x.variant.clone(),
        })
        .collect()
}

fn make_account_diagnostics_base(settings: &LauncherSettings) -> AccountDiagnostics {
    AccountDiagnostics {
        status: if settings.selected_account_id.is_some() {
            "connected".to_string()
        } else {
            "not_connected".to_string()
        },
        last_refreshed_at: now_iso(),
        selected_account_id: settings.selected_account_id.clone(),
        account: None,
        minecraft_uuid: None,
        minecraft_username: None,
        entitlements_ok: false,
        token_exchange_status: "idle".to_string(),
        skin_url: None,
        cape_count: 0,
        skins: vec![],
        capes: vec![],
        last_error: None,
        client_id_source: "none".to_string(),
    }
}

fn fail_account_diag(mut diag: AccountDiagnostics, stage: &str, msg: String) -> AccountDiagnostics {
    diag.status = "error".to_string();
    diag.token_exchange_status = stage.to_string();
    diag.last_error = Some(msg);
    diag
}

fn latest_crash_report_path(instance_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        instance_dir.join("runtime").join("crash-reports"),
        instance_dir.join("crash-reports"),
    ];
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for root in candidates {
        if !root.exists() || !root.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for ent in entries.flatten() {
            let path = ent.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !name.ends_with(".txt") {
                continue;
            }
            let Ok(meta) = ent.metadata() else {
                continue;
            };
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match &best {
                Some((prev, _)) if *prev >= modified => {}
                _ => {
                    best = Some((modified, path));
                }
            }
        }
    }
    best.map(|(_, path)| path)
}

fn launch_logs_dir(instance_dir: &Path) -> PathBuf {
    instance_dir.join("logs").join("launches")
}

fn latest_launch_log_path(instance_dir: &Path) -> Option<PathBuf> {
    let root = launch_logs_dir(instance_dir);
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(entries) = fs::read_dir(&root) {
        for ent in entries.flatten() {
            let path = ent.path();
            if !path.is_file() {
                continue;
            }
            let Ok(meta) = ent.metadata() else {
                continue;
            };
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match &best {
                Some((prev, _)) if *prev >= modified => {}
                _ => {
                    best = Some((modified, path));
                }
            }
        }
    }
    if let Some((_, path)) = best {
        return Some(path);
    }
    let legacy = instance_dir.join("runtime").join("native-launch.log");
    if legacy.exists() && legacy.is_file() {
        Some(legacy)
    } else {
        None
    }
}

fn classify_log_severity(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if lower.contains(" fatal ")
        || lower.contains(" exception")
        || lower.contains(" crashed")
        || lower.contains(" crash ")
        || lower.contains(" error")
        || lower.starts_with("error")
        || lower.starts_with("[error")
    {
        return Some("error".to_string());
    }
    if lower.contains(" warning")
        || lower.contains(" warn ")
        || lower.starts_with("warn")
        || lower.starts_with("[warn")
    {
        return Some("warn".to_string());
    }
    if lower.contains(" debug ") || lower.starts_with("debug") || lower.starts_with("[debug") {
        return Some("debug".to_string());
    }
    if lower.contains(" trace ") || lower.starts_with("trace") || lower.starts_with("[trace") {
        return Some("trace".to_string());
    }
    if lower.contains(" info ") || lower.starts_with("info") || lower.starts_with("[info") {
        return Some("info".to_string());
    }
    None
}

fn extract_log_timestamp(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(end) = trimmed.find(']') {
        if trimmed.starts_with('[') && end > 1 {
            let ts = trimmed[1..end].trim();
            if !ts.is_empty() {
                return Some(ts.to_string());
            }
        }
    }
    if trimmed.len() >= 19 {
        let candidate = &trimmed[..19];
        let bytes = candidate.as_bytes();
        if bytes.get(4) == Some(&b'-')
            && bytes.get(7) == Some(&b'-')
            && (bytes.get(10) == Some(&b' ') || bytes.get(10) == Some(&b'T'))
        {
            return Some(candidate.to_string());
        }
    }
    None
}

fn read_windowed_log_lines(
    path: &Path,
    source: &str,
    max_lines: usize,
    before_line: Option<u64>,
) -> Result<
    (
        Vec<LogLineDto>,
        usize,
        bool,
        Option<u64>,
        Option<u64>,
        Option<u64>,
    ),
    String,
> {
    let bytes = fs::read(path).map_err(|e| format!("read log file failed: {e}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let all_lines: Vec<&str> = text.lines().collect();
    let total_lines = all_lines.len();
    let end_exclusive = before_line
        .map(|line| line.saturating_sub(1) as usize)
        .unwrap_or(total_lines)
        .min(total_lines);
    let start = end_exclusive.saturating_sub(max_lines);
    let truncated = start > 0;
    let lines = all_lines[start..end_exclusive]
        .iter()
        .enumerate()
        .map(|(offset, line)| {
            let raw = line.trim_end().to_string();
            let line_no = (start + offset + 1) as u64;
            LogLineDto {
                raw: raw.clone(),
                line_no,
                timestamp: extract_log_timestamp(&raw),
                severity: classify_log_severity(&raw),
                source: source.to_string(),
            }
        })
        .collect::<Vec<_>>();
    let start_line_no = if end_exclusive > start {
        Some((start + 1) as u64)
    } else {
        None
    };
    let end_line_no = if end_exclusive > start {
        Some(end_exclusive as u64)
    } else {
        None
    };
    let next_before_line = if truncated { start_line_no } else { None };
    Ok((
        lines,
        total_lines,
        truncated,
        start_line_no,
        end_line_no,
        next_before_line,
    ))
}

fn resolve_target_instance_path(
    instance_dir: &Path,
    target: &str,
) -> Result<(String, PathBuf, bool), String> {
    match target.trim().to_lowercase().as_str() {
        "instance" => Ok(("instance".to_string(), instance_dir.to_path_buf(), true)),
        "mods" => Ok(("mods".to_string(), instance_dir.join("mods"), true)),
        "resourcepacks" => Ok(("resourcepacks".to_string(), instance_dir.join("resourcepacks"), true)),
        "shaderpacks" => Ok(("shaderpacks".to_string(), instance_dir.join("shaderpacks"), true)),
        "saves" => Ok(("saves".to_string(), instance_dir.join("saves"), true)),
        "launch-log" | "launch_log" | "log" => Ok((
            "launch-log".to_string(),
            latest_launch_log_path(instance_dir)
                .unwrap_or_else(|| instance_dir.join("runtime").join("native-launch.log")),
            false,
        )),
        "crash-log" | "crash_log" | "latest-crash" | "latest_crash" => {
            if let Some(path) = latest_crash_report_path(instance_dir) {
                Ok(("crash-log".to_string(), path, false))
            } else {
                Err("No crash report found yet for this instance.".to_string())
            }
        }
        _ => Err(
            "target must be 'instance', 'mods', 'resourcepacks', 'shaderpacks', 'saves', 'launch-log', or 'crash-log'"
                .to_string(),
        ),
    }
}

fn open_path_in_shell(path: &Path, create_if_missing: bool) -> Result<(), String> {
    if !path.exists() {
        if create_if_missing {
            fs::create_dir_all(path)
                .map_err(|e| format!("create path '{}' failed: {e}", path.display()))?;
        } else {
            return Err(format!(
                "Path '{}' does not exist yet. Launch once first to generate it.",
                path.display()
            ));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg(path)
            .status()
            .map_err(|e| format!("open path '{}' failed: {e}", path.display()))?;
        if !status.success() {
            return Err(format!(
                "open path '{}' failed: open exited with status {}",
                path.display(),
                status
            ));
        }
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let status = Command::new("explorer")
            .arg(path)
            .status()
            .map_err(|e| format!("open path '{}' failed: {e}", path.display()))?;
        if !status.success() {
            return Err(format!(
                "open path '{}' failed: explorer exited with status {}",
                path.display(),
                status
            ));
        }
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = Command::new("xdg-open")
            .arg(path)
            .status()
            .map_err(|e| format!("open path '{}' failed: {e}", path.display()))?;
        if !status.success() {
            return Err(format!(
                "open path '{}' failed: xdg-open exited with status {}",
                path.display(),
                status
            ));
        }
        return Ok(());
    }
}

fn reveal_path_in_shell(
    path: &Path,
    allow_parent_fallback: bool,
) -> Result<(PathBuf, bool), String> {
    let mut target = path.to_path_buf();
    if !target.exists() {
        if allow_parent_fallback {
            if let Some(parent) = target.parent() {
                target = parent.to_path_buf();
            }
        }
    }
    if !target.exists() {
        return Err(format!(
            "Path '{}' does not exist yet. Launch once first to generate it.",
            target.display()
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("open");
        let reveal_exact = target.is_file();
        if reveal_exact {
            cmd.arg("-R");
        }
        let status = cmd
            .arg(&target)
            .status()
            .map_err(|e| format!("reveal path '{}' failed: {e}", target.display()))?;
        if !status.success() {
            return Err(format!(
                "reveal path '{}' failed: open exited with status {}",
                target.display(),
                status
            ));
        }
        return Ok((target, reveal_exact));
    }

    #[cfg(target_os = "windows")]
    {
        if target.is_file() {
            let arg = format!("/select,{}", target.display());
            let status = Command::new("explorer")
                .arg(arg)
                .status()
                .map_err(|e| format!("reveal path '{}' failed: {e}", target.display()))?;
            if !status.success() {
                return Err(format!(
                    "reveal path '{}' failed: explorer exited with status {}",
                    target.display(),
                    status
                ));
            }
            return Ok((target, true));
        }
        let status = Command::new("explorer")
            .arg(&target)
            .status()
            .map_err(|e| format!("open path '{}' failed: {e}", target.display()))?;
        if !status.success() {
            return Err(format!(
                "open path '{}' failed: explorer exited with status {}",
                target.display(),
                status
            ));
        }
        return Ok((target, false));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let open_target = if target.is_file() {
            target.parent().unwrap_or(&target).to_path_buf()
        } else {
            target.clone()
        };
        let status = Command::new("xdg-open")
            .arg(&open_target)
            .status()
            .map_err(|e| format!("open path '{}' failed: {e}", open_target.display()))?;
        if !status.success() {
            return Err(format!(
                "open path '{}' failed: xdg-open exited with status {}",
                open_target.display(),
                status
            ));
        }
        return Ok((open_target, false));
    }
}

fn set_login_session_state(
    state: &Arc<Mutex<HashMap<String, MicrosoftLoginState>>>,
    session_id: &str,
    status: &str,
    message: Option<String>,
    account: Option<LauncherAccount>,
) {
    if let Ok(mut guard) = state.lock() {
        guard.insert(
            session_id.to_string(),
            MicrosoftLoginState {
                status: status.to_string(),
                message,
                account,
            },
        );
    }
}

fn keyring_unavailable_hint() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "No OS keyring provider is available. Install and unlock Secret Service (for example gnome-keyring or KWallet), then restart OpenJar Launcher."
    }
    #[cfg(not(target_os = "linux"))]
    {
        "OS secure credential storage is unavailable. Ensure your keychain/credential manager is enabled and unlocked, then restart OpenJar Launcher."
    }
}

#[cfg(not(test))]
fn keyring_error_with_action(operation: &str, error: &KeyringError) -> String {
    match error {
        KeyringError::NoStorageAccess(_) | KeyringError::PlatformFailure(_) => {
            format!(
                "{operation} failed: {} ({error})",
                keyring_unavailable_hint()
            )
        }
        _ => format!("{operation} failed: {error}"),
    }
}

#[cfg(test)]
fn test_token_keyring_store() -> &'static Mutex<HashMap<(String, String), String>> {
    use std::sync::OnceLock;
    static STORE: OnceLock<Mutex<HashMap<(String, String), String>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn test_token_keyring_available_flag() -> &'static std::sync::atomic::AtomicBool {
    use std::sync::atomic::AtomicBool;
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<AtomicBool> = OnceLock::new();
    AVAILABLE.get_or_init(|| AtomicBool::new(true))
}

#[cfg(test)]
fn test_token_keyring_read_fail_services() -> &'static Mutex<HashSet<String>> {
    use std::sync::OnceLock;
    static SERVICES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SERVICES.get_or_init(|| Mutex::new(HashSet::new()))
}

#[cfg(test)]
fn test_token_keyring_available() -> bool {
    use std::sync::atomic::Ordering;
    test_token_keyring_available_flag().load(Ordering::SeqCst)
}

#[cfg(test)]
fn set_test_token_keyring_available(value: bool) {
    use std::sync::atomic::Ordering;
    test_token_keyring_available_flag().store(value, Ordering::SeqCst);
}

#[cfg(test)]
fn set_test_token_keyring_read_failure(service: &str, should_fail: bool) {
    if let Ok(mut guard) = test_token_keyring_read_fail_services().lock() {
        if should_fail {
            guard.insert(service.to_string());
        } else {
            guard.remove(service);
        }
    }
}

#[cfg(test)]
fn clear_test_token_keyring_store() {
    if let Ok(mut guard) = test_token_keyring_store().lock() {
        guard.clear();
    }
    if let Ok(mut guard) = test_token_keyring_read_fail_services().lock() {
        guard.clear();
    }
    runtime_refresh_token_cache_clear();
}

#[cfg(test)]
fn token_keyring_set_secret(service: &str, username: &str, secret: &str) -> Result<(), String> {
    if !test_token_keyring_available() {
        return Err(format!(
            "keyring write failed: {}",
            keyring_unavailable_hint()
        ));
    }
    let mut guard = test_token_keyring_store()
        .lock()
        .map_err(|_| "test keyring lock failed".to_string())?;
    guard.insert(
        (service.to_string(), username.to_string()),
        secret.to_string(),
    );
    Ok(())
}

#[cfg(not(test))]
fn token_keyring_set_secret(service: &str, username: &str, secret: &str) -> Result<(), String> {
    let entry = KeyringEntry::new(service, username)
        .map_err(|e| keyring_error_with_action("keyring init", &e))?;
    entry
        .set_password(secret)
        .map_err(|e| keyring_error_with_action("keyring write", &e))
}

#[cfg(test)]
fn token_keyring_get_secret(service: &str, username: &str) -> Result<Option<String>, String> {
    if !test_token_keyring_available() {
        return Err(format!(
            "keyring read failed: {}",
            keyring_unavailable_hint()
        ));
    }
    let should_fail = test_token_keyring_read_fail_services()
        .lock()
        .map_err(|_| "test keyring lock failed".to_string())?
        .contains(service);
    if should_fail {
        return Err(format!(
            "keyring read failed: simulated read failure for service '{service}'"
        ));
    }
    let guard = test_token_keyring_store()
        .lock()
        .map_err(|_| "test keyring lock failed".to_string())?;
    Ok(guard
        .get(&(service.to_string(), username.to_string()))
        .cloned())
}

#[cfg(not(test))]
fn token_keyring_get_secret(service: &str, username: &str) -> Result<Option<String>, String> {
    let entry = KeyringEntry::new(service, username)
        .map_err(|e| keyring_error_with_action("keyring init", &e))?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(keyring_error_with_action("keyring read", &e)),
    }
}

#[cfg(test)]
fn token_keyring_delete_secret(service: &str, username: &str) -> Result<(), String> {
    if !test_token_keyring_available() {
        return Err(format!(
            "keyring delete failed: {}",
            keyring_unavailable_hint()
        ));
    }
    let mut guard = test_token_keyring_store()
        .lock()
        .map_err(|_| "test keyring lock failed".to_string())?;
    guard.remove(&(service.to_string(), username.to_string()));
    Ok(())
}

#[cfg(not(test))]
fn token_keyring_delete_secret(service: &str, username: &str) -> Result<(), String> {
    let entry = KeyringEntry::new(service, username)
        .map_err(|e| keyring_error_with_action("keyring init", &e))?;
    match entry.delete_credential() {
        Ok(_) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(keyring_error_with_action("keyring delete", &e)),
    }
}

fn keyring_set_refresh_token(account_id: &str, refresh_token: &str) -> Result<(), String> {
    let keys = vec![account_id.to_string()];
    for key in keys {
        for username in keyring_alias_usernames_for_key(&key) {
            token_keyring_set_secret(KEYRING_SERVICE, &username, refresh_token)?;
            for legacy_service in LEGACY_KEYRING_SERVICES {
                if legacy_service == KEYRING_SERVICE {
                    continue;
                }
                if let Err(err) = token_keyring_set_secret(legacy_service, &username, refresh_token)
                {
                    eprintln!(
                        "legacy secure-storage mirror write failed for alias '{}' in service '{}': {}",
                        username, legacy_service, err
                    );
                }
            }
        }
        runtime_refresh_token_cache_set(&key, refresh_token);
    }
    Ok(())
}

fn keyring_set_refresh_token_for_account(
    account: &LauncherAccount,
    refresh_token: &str,
) -> Result<(), String> {
    let keys = vec![account.id.clone(), account.username.clone()];
    for key in keys {
        for username in keyring_alias_usernames_for_key(&key) {
            token_keyring_set_secret(KEYRING_SERVICE, &username, refresh_token)?;
            for legacy_service in LEGACY_KEYRING_SERVICES {
                if legacy_service == KEYRING_SERVICE {
                    continue;
                }
                if let Err(err) = token_keyring_set_secret(legacy_service, &username, refresh_token)
                {
                    eprintln!(
                        "legacy secure-storage mirror write failed for alias '{}' in service '{}': {}",
                        username, legacy_service, err
                    );
                }
            }
        }
        runtime_refresh_token_cache_set(&key, refresh_token);
    }
    Ok(())
}

fn keyring_set_selected_refresh_token(refresh_token: &str) -> Result<(), String> {
    token_keyring_set_secret(
        KEYRING_SERVICE,
        KEYRING_SELECTED_REFRESH_ALIAS,
        refresh_token,
    )?;
    for legacy_service in LEGACY_KEYRING_SERVICES {
        if legacy_service == KEYRING_SERVICE {
            continue;
        }
        if let Err(err) = token_keyring_set_secret(
            legacy_service,
            KEYRING_SELECTED_REFRESH_ALIAS,
            refresh_token,
        ) {
            eprintln!(
                "legacy selected refresh-token alias mirror write failed in service '{}': {}",
                legacy_service, err
            );
        }
    }
    Ok(())
}

fn keyring_get_selected_refresh_token() -> Result<Option<String>, String> {
    let services = keyring_service_candidates();
    let mut canonical_read_err: Option<String> = None;
    for service in services {
        let token = match token_keyring_get_secret(service, KEYRING_SELECTED_REFRESH_ALIAS) {
            Ok(token) => token,
            Err(err) => {
                if service == KEYRING_SERVICE {
                    if canonical_read_err.is_none() {
                        canonical_read_err = Some(err);
                    }
                } else {
                    eprintln!(
                        "legacy selected refresh-token alias read failed in service '{}': {}",
                        service, err
                    );
                }
                continue;
            }
        };
        let Some(token) = token else {
            continue;
        };
        if token.trim().is_empty() {
            continue;
        }
        if service != KEYRING_SERVICE {
            if let Err(err) =
                token_keyring_set_secret(KEYRING_SERVICE, KEYRING_SELECTED_REFRESH_ALIAS, &token)
            {
                eprintln!(
                    "selected refresh-token alias canonical mirror write failed from service '{}': {}",
                    service, err
                );
            }
        }
        return Ok(Some(token));
    }
    if let Some(err) = canonical_read_err {
        return Err(err);
    }
    Ok(None)
}

fn keyring_get_dev_curseforge_key() -> Result<Option<String>, String> {
    let services = keyring_service_candidates();
    let mut canonical_read_err: Option<String> = None;
    for service in services {
        let value = match token_keyring_get_secret(service, DEV_CURSEFORGE_KEY_KEYRING_USER) {
            Ok(value) => value,
            Err(err) => {
                if service == KEYRING_SERVICE {
                    if canonical_read_err.is_none() {
                        canonical_read_err = Some(err);
                    }
                } else {
                    eprintln!(
                        "legacy dev curseforge key read failed in service '{}': {}",
                        service, err
                    );
                }
                continue;
            }
        };
        let Some(value) = value else { continue };
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if service != KEYRING_SERVICE {
            if let Err(err) =
                token_keyring_set_secret(KEYRING_SERVICE, DEV_CURSEFORGE_KEY_KEYRING_USER, &trimmed)
            {
                eprintln!(
                    "dev curseforge key canonical mirror write failed from service '{}': {}",
                    service, err
                );
            }
        }
        return Ok(Some(trimmed));
    }
    if let Some(err) = canonical_read_err {
        return Err(err);
    }
    Ok(None)
}

fn keyring_set_dev_curseforge_key(value: &str) -> Result<(), String> {
    token_keyring_set_secret(KEYRING_SERVICE, DEV_CURSEFORGE_KEY_KEYRING_USER, value)?;
    for legacy_service in LEGACY_KEYRING_SERVICES {
        if legacy_service == KEYRING_SERVICE {
            continue;
        }
        if let Err(err) =
            token_keyring_set_secret(legacy_service, DEV_CURSEFORGE_KEY_KEYRING_USER, value)
        {
            eprintln!(
                "legacy dev curseforge key mirror write failed in service '{}': {}",
                legacy_service, err
            );
        }
    }
    Ok(())
}

fn keyring_delete_dev_curseforge_key() -> Result<(), String> {
    token_keyring_delete_secret(KEYRING_SERVICE, DEV_CURSEFORGE_KEY_KEYRING_USER)?;
    for legacy_service in LEGACY_KEYRING_SERVICES {
        if legacy_service == KEYRING_SERVICE {
            continue;
        }
        if let Err(err) =
            token_keyring_delete_secret(legacy_service, DEV_CURSEFORGE_KEY_KEYRING_USER)
        {
            eprintln!(
                "legacy dev curseforge key mirror delete failed in service '{}': {}",
                legacy_service, err
            );
        }
    }
    Ok(())
}

fn keyring_get_github_token_pool() -> Result<Vec<String>, String> {
    let services = keyring_service_candidates();
    let mut canonical_read_err: Option<String> = None;
    for service in services {
        let value = match token_keyring_get_secret(service, GITHUB_TOKEN_POOL_KEYRING_USER) {
            Ok(value) => value,
            Err(err) => {
                if service == KEYRING_SERVICE {
                    if canonical_read_err.is_none() {
                        canonical_read_err = Some(err);
                    }
                } else {
                    eprintln!(
                        "legacy github token pool read failed in service '{}': {}",
                        service, err
                    );
                }
                continue;
            }
        };
        let Some(value) = value else {
            continue;
        };
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let mut parsed = Vec::new();
        github_parse_token_pool(&trimmed, &mut parsed);
        if parsed.is_empty() {
            continue;
        }
        if service != KEYRING_SERVICE {
            let canonical = parsed.join("\n");
            if let Err(err) = token_keyring_set_secret(
                KEYRING_SERVICE,
                GITHUB_TOKEN_POOL_KEYRING_USER,
                &canonical,
            ) {
                eprintln!(
                    "github token pool canonical mirror write failed from service '{}': {}",
                    service, err
                );
            }
        }
        return Ok(parsed);
    }
    if let Some(err) = canonical_read_err {
        return Err(err);
    }
    Ok(vec![])
}

fn keyring_set_github_token_pool(raw: &str) -> Result<usize, String> {
    let mut parsed = Vec::new();
    github_parse_token_pool(raw, &mut parsed);
    if parsed.is_empty() {
        return Err(
            "GitHub token pool is empty. Paste one or more tokens separated by comma, semicolon, or newline."
                .to_string(),
        );
    }
    let canonical = parsed.join("\n");
    token_keyring_set_secret(KEYRING_SERVICE, GITHUB_TOKEN_POOL_KEYRING_USER, &canonical)?;
    for legacy_service in LEGACY_KEYRING_SERVICES {
        if legacy_service == KEYRING_SERVICE {
            continue;
        }
        if let Err(err) =
            token_keyring_set_secret(legacy_service, GITHUB_TOKEN_POOL_KEYRING_USER, &canonical)
        {
            eprintln!(
                "legacy github token pool mirror write failed in service '{}': {}",
                legacy_service, err
            );
        }
    }
    Ok(parsed.len())
}

fn keyring_delete_github_token_pool() -> Result<(), String> {
    token_keyring_delete_secret(KEYRING_SERVICE, GITHUB_TOKEN_POOL_KEYRING_USER)?;
    for legacy_service in LEGACY_KEYRING_SERVICES {
        if legacy_service == KEYRING_SERVICE {
            continue;
        }
        if let Err(err) =
            token_keyring_delete_secret(legacy_service, GITHUB_TOKEN_POOL_KEYRING_USER)
        {
            eprintln!(
                "legacy github token pool mirror delete failed in service '{}': {}",
                legacy_service, err
            );
        }
    }
    Ok(())
}

fn persist_refresh_token_for_account(account_id: &str, refresh_token: &str) -> Result<(), String> {
    keyring_set_refresh_token(account_id, refresh_token)
}

fn persist_refresh_token_for_account_with_app(
    app: &tauri::AppHandle,
    account_id: &str,
    refresh_token: &str,
) -> Result<(), String> {
    #[cfg(not(debug_assertions))]
    let _ = app;
    persist_refresh_token_for_account(account_id, refresh_token)?;
    #[cfg(debug_assertions)]
    {
        if let Err(err) =
            persist_refresh_token_debug_fallback_for_key(app, account_id, refresh_token)
        {
            eprintln!(
                "debug refresh-token fallback write failed for account '{account_id}': {err}"
            );
        }
    }
    Ok(())
}

fn persist_refresh_token_for_launcher_account(
    account: &LauncherAccount,
    refresh_token: &str,
) -> Result<(), String> {
    keyring_set_refresh_token_for_account(account, refresh_token)?;
    keyring_set_selected_refresh_token(refresh_token)
}

fn persist_refresh_token_for_launcher_account_with_app(
    app: &tauri::AppHandle,
    account: &LauncherAccount,
    refresh_token: &str,
) -> Result<(), String> {
    persist_refresh_token_for_launcher_account(account, refresh_token)?;
    if let Err(err) = persist_refresh_token_recovery_fallback(app, account, refresh_token) {
        eprintln!(
            "refresh-token recovery fallback write failed for selected account '{}': {}",
            account.id, err
        );
    }
    if let Err(err) = verify_refresh_token_secure_storage_write(account, refresh_token) {
        eprintln!(
            "refresh-token secure-storage verification warning for account '{}': {}",
            account.id, err
        );
    }
    #[cfg(debug_assertions)]
    {
        if let Err(err) = persist_refresh_token_debug_fallback(app, account, refresh_token) {
            eprintln!(
                "debug refresh-token fallback write failed for selected account '{}': {}",
                account.id, err
            );
        }
    }
    Ok(())
}

fn persist_refresh_token(
    app: &tauri::AppHandle,
    account_id: &str,
    refresh_token: &str,
) -> Result<(), String> {
    persist_refresh_token_for_account_with_app(app, account_id, refresh_token)
}

fn keyring_username_candidates(
    account: &LauncherAccount,
    accounts: &[LauncherAccount],
) -> Vec<String> {
    fn push_unique(out: &mut Vec<String>, value: String) {
        if value.trim().is_empty() {
            return;
        }
        if !out.iter().any(|x| x == &value) {
            out.push(value);
        }
    }

    fn add_aliases(out: &mut Vec<String>, key: &str) {
        for alias in keyring_alias_usernames_for_key(key) {
            push_unique(out, alias);
        }
    }

    let mut out = Vec::new();
    add_aliases(&mut out, &account.id);
    add_aliases(&mut out, &account.username);
    add_aliases(&mut out, &account.id.to_lowercase());
    for candidate in accounts
        .iter()
        .filter(|x| x.username.eq_ignore_ascii_case(&account.username))
    {
        add_aliases(&mut out, &candidate.id);
    }
    out
}

fn keyring_try_read(service: &str, username: &str) -> Result<Option<String>, String> {
    token_keyring_get_secret(service, username)
}

fn keyring_service_candidates() -> Vec<&'static str> {
    let mut candidates = Vec::with_capacity(1 + LEGACY_KEYRING_SERVICES.len());
    candidates.push(KEYRING_SERVICE);
    for legacy in LEGACY_KEYRING_SERVICES {
        if legacy != KEYRING_SERVICE {
            candidates.push(legacy);
        }
    }
    candidates
}

fn secure_storage_contains_refresh_token_for_aliases(
    aliases: &[String],
    expected_refresh_token: &str,
) -> Result<bool, String> {
    if expected_refresh_token.trim().is_empty() {
        return Ok(false);
    }
    let services = keyring_service_candidates();
    let mut canonical_read_err: Option<String> = None;
    for service in services {
        for alias in aliases {
            let token = match keyring_try_read(service, alias) {
                Ok(token) => token,
                Err(err) => {
                    if service == KEYRING_SERVICE {
                        if canonical_read_err.is_none() {
                            canonical_read_err = Some(err);
                        }
                    } else {
                        eprintln!(
                            "legacy secure-storage read failed for alias '{}' in service '{}': {}",
                            alias, service, err
                        );
                    }
                    continue;
                }
            };
            let Some(token) = token else {
                continue;
            };
            if token == expected_refresh_token {
                return Ok(true);
            }
        }
    }
    if let Some(selected) = keyring_get_selected_refresh_token()? {
        if selected == expected_refresh_token {
            return Ok(true);
        }
    }
    if let Some(err) = canonical_read_err {
        return Err(err);
    }
    Ok(false)
}

fn verify_refresh_token_secure_storage_write(
    account: &LauncherAccount,
    refresh_token: &str,
) -> Result<(), String> {
    fn push_unique(out: &mut Vec<String>, value: String) {
        if value.trim().is_empty() {
            return;
        }
        if !out.iter().any(|item| item == &value) {
            out.push(value);
        }
    }

    let mut aliases = Vec::new();
    for alias in keyring_alias_usernames_for_key(&account.id) {
        push_unique(&mut aliases, alias);
    }
    for alias in keyring_alias_usernames_for_key(&account.username) {
        push_unique(&mut aliases, alias);
    }

    if secure_storage_contains_refresh_token_for_aliases(&aliases, refresh_token)? {
        return Ok(());
    }

    Err("Secure storage verification failed after writing refresh token. Reconnect Microsoft account and ensure your OS keychain is unlocked.".to_string())
}

fn recover_refresh_token_from_known_accounts(
    selected_account: &LauncherAccount,
    accounts: &[LauncherAccount],
    services: &[&str],
) -> Result<Option<String>, String> {
    fn push_unique(out: &mut Vec<String>, value: String) {
        if value.trim().is_empty() {
            return;
        }
        if !out.iter().any(|item| item == &value) {
            out.push(value);
        }
    }

    let mut usernames = Vec::new();
    for candidate in accounts {
        for username in keyring_username_candidates(candidate, accounts) {
            push_unique(&mut usernames, username);
        }
    }
    if usernames.is_empty() {
        return Ok(None);
    }

    let mut unique_tokens = Vec::<String>::new();
    let mut canonical_read_err: Option<String> = None;
    for service in services {
        for username in &usernames {
            let token = match keyring_try_read(service, username) {
                Ok(token) => token,
                Err(err) => {
                    if *service == KEYRING_SERVICE {
                        if canonical_read_err.is_none() {
                            canonical_read_err = Some(err);
                        }
                    } else {
                        eprintln!(
                            "legacy secure-storage read failed for alias '{}' in service '{}': {}",
                            username, service, err
                        );
                    }
                    continue;
                }
            };
            let Some(token) = token else {
                continue;
            };
            if token.trim().is_empty() {
                continue;
            }
            if !unique_tokens.iter().any(|item| item == &token) {
                unique_tokens.push(token);
            }
        }
    }

    match unique_tokens.len() {
        0 => {
            if let Some(err) = canonical_read_err {
                return Err(err);
            }
            Ok(None)
        }
        1 => {
            let token = unique_tokens.pop().expect("token list length checked");
            persist_refresh_token_for_launcher_account(selected_account, &token)?;
            eprintln!(
                "recovered refresh token in secure storage for selected account {} from legacy alias",
                selected_account.id
            );
            Ok(Some(token))
        }
        _ => Err(
            "Multiple secure refresh tokens were found but none matched the selected account. Select the correct account or reconnect to repair credentials."
                .to_string(),
        ),
    }
}

fn read_refresh_token_from_keyring_aliases_only(
    account: &LauncherAccount,
    accounts: &[LauncherAccount],
) -> Result<Option<String>, String> {
    let canonical_username = keyring_username_for_account(&account.id);
    let usernames = keyring_username_candidates(account, accounts);
    let services = keyring_service_candidates();
    let mut canonical_read_err: Option<String> = None;

    for service in &services {
        for username in &usernames {
            let token = match keyring_try_read(service, username) {
                Ok(token) => token,
                Err(err) => {
                    if *service == KEYRING_SERVICE {
                        if canonical_read_err.is_none() {
                            canonical_read_err = Some(err);
                        }
                    } else {
                        eprintln!(
                            "legacy secure-storage read failed for alias '{}' in service '{}': {}",
                            username, service, err
                        );
                    }
                    continue;
                }
            };
            let Some(token) = token else {
                continue;
            };

            let is_canonical = *service == KEYRING_SERVICE && username == &canonical_username;
            if !is_canonical {
                if let Err(e) = persist_refresh_token_for_launcher_account(account, &token) {
                    eprintln!(
                        "refresh token migration to canonical key failed for account {}: {}",
                        account.id, e
                    );
                }
            } else if let Err(e) = keyring_set_selected_refresh_token(&token) {
                eprintln!(
                    "refresh token selected-alias write failed for account {}: {}",
                    account.id, e
                );
            }
            runtime_refresh_token_cache_set(&account.id, &token);
            runtime_refresh_token_cache_set(&account.username, &token);
            return Ok(Some(token));
        }
    }

    if let Some(token) = runtime_refresh_token_cache_get(&account.id) {
        if !token.trim().is_empty() {
            return Ok(Some(token));
        }
    }

    if let Some(err) = canonical_read_err {
        return Err(err);
    }
    Ok(None)
}

fn maybe_repair_selected_account_with_available_token(
    app: &tauri::AppHandle,
    selected_account: &LauncherAccount,
    accounts: &[LauncherAccount],
) -> Result<Option<LauncherAccount>, String> {
    if read_refresh_token_from_keyring_aliases_only(selected_account, accounts)?.is_some() {
        return Ok(Some(selected_account.clone()));
    }

    let mut candidates = Vec::<LauncherAccount>::new();
    for candidate in accounts {
        if read_refresh_token_from_keyring_aliases_only(candidate, accounts)?.is_some()
            && !candidates.iter().any(|item| item.id == candidate.id)
        {
            candidates.push(candidate.clone());
        }
    }

    if candidates.len() != 1 {
        return Ok(None);
    }

    let repaired = candidates.pop().expect("candidate count checked");
    if repaired.id != selected_account.id {
        let mut settings = read_launcher_settings(app)?;
        settings.selected_account_id = Some(repaired.id.clone());
        write_launcher_settings(app, &settings)?;
        eprintln!(
            "repaired selected Microsoft account to '{}' because the previous selection had no secure refresh token",
            repaired.id
        );
    }
    Ok(Some(repaired))
}

fn read_refresh_token_from_keyring(
    account: &LauncherAccount,
    accounts: &[LauncherAccount],
) -> Result<String, String> {
    if let Some(token) = read_refresh_token_from_keyring_aliases_only(account, accounts)? {
        return Ok(token);
    }

    let services = keyring_service_candidates();
    if let Some(token) = recover_refresh_token_from_known_accounts(account, accounts, &services)? {
        return Ok(token);
    }
    if let Some(token) = keyring_get_selected_refresh_token()? {
        if !token.trim().is_empty() {
            let _ = keyring_set_refresh_token_for_account(account, &token);
            runtime_refresh_token_cache_set(&account.id, &token);
            runtime_refresh_token_cache_set(&account.username, &token);
            return Ok(token);
        }
    }

    Err("No refresh token found in secure storage for the selected account. Click Connect / Reconnect to repair account credentials.".to_string())
}

fn keyring_get_refresh_token_for_account(
    app: &tauri::AppHandle,
    account: &LauncherAccount,
    accounts: &[LauncherAccount],
) -> Result<String, String> {
    match read_refresh_token_from_keyring(account, accounts) {
        Ok(token) => Ok(token),
        Err(err) => {
            if err.starts_with("No refresh token found in secure storage")
                || err.starts_with("Multiple secure refresh tokens were found")
            {
                #[cfg(debug_assertions)]
                {
                    if let Some(token) = read_refresh_token_debug_fallback(app, account, accounts)?
                    {
                        if let Err(write_err) = persist_refresh_token_for_launcher_account_with_app(
                            app, account, &token,
                        ) {
                            eprintln!(
                                "debug refresh-token recovery write-back failed for account '{}': {}",
                                account.id, write_err
                            );
                        }
                        return Ok(token);
                    }
                }
                if let Some(token) = read_refresh_token_recovery_fallback(app, account, accounts)? {
                    if let Err(write_err) =
                        persist_refresh_token_for_launcher_account_with_app(app, account, &token)
                    {
                        eprintln!(
                            "refresh-token recovery fallback write-back failed for account '{}': {}",
                            account.id, write_err
                        );
                    }
                    return Ok(token);
                }
            }
            Err(err)
        }
    }
}

fn keyring_delete_refresh_token(account_id: &str) -> Result<(), String> {
    for username in keyring_alias_usernames_for_key(account_id) {
        token_keyring_delete_secret(KEYRING_SERVICE, &username)?;
        for legacy_service in LEGACY_KEYRING_SERVICES {
            if legacy_service == KEYRING_SERVICE {
                continue;
            }
            if let Err(err) = token_keyring_delete_secret(legacy_service, &username) {
                eprintln!(
                    "legacy secure-storage mirror delete failed for alias '{}' in service '{}': {}",
                    username, legacy_service, err
                );
            }
        }
    }
    runtime_refresh_token_cache_delete(account_id);
    Ok(())
}

fn keyring_delete_refresh_token_for_account(account: &LauncherAccount) -> Result<(), String> {
    for key in [&account.id, &account.username] {
        for username in keyring_alias_usernames_for_key(key) {
            token_keyring_delete_secret(KEYRING_SERVICE, &username)?;
            for legacy_service in LEGACY_KEYRING_SERVICES {
                if legacy_service == KEYRING_SERVICE {
                    continue;
                }
                if let Err(err) = token_keyring_delete_secret(legacy_service, &username) {
                    eprintln!(
                        "legacy secure-storage mirror delete failed for alias '{}' in service '{}': {}",
                        username, legacy_service, err
                    );
                }
            }
        }
        runtime_refresh_token_cache_delete(key);
    }
    Ok(())
}

fn keyring_delete_selected_refresh_token() -> Result<(), String> {
    token_keyring_delete_secret(KEYRING_SERVICE, KEYRING_SELECTED_REFRESH_ALIAS)?;
    for legacy_service in LEGACY_KEYRING_SERVICES {
        if legacy_service == KEYRING_SERVICE {
            continue;
        }
        if let Err(err) =
            token_keyring_delete_secret(legacy_service, KEYRING_SELECTED_REFRESH_ALIAS)
        {
            eprintln!(
                "legacy selected refresh-token alias mirror delete failed in service '{}': {}",
                legacy_service, err
            );
        }
    }
    Ok(())
}

fn delete_refresh_token_everywhere(_app: &tauri::AppHandle, account_id: &str) {
    if let Err(e) = keyring_delete_refresh_token(account_id) {
        eprintln!("keyring delete failed for account {}: {}", account_id, e);
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct LegacyTokenMigrationSummary {
    migrated: usize,
    skipped: usize,
    failed: usize,
    fallback_files_removed: usize,
}

fn migrate_legacy_refresh_tokens_from_path(
    path: &Path,
) -> Result<LegacyTokenMigrationSummary, String> {
    if !path.exists() {
        return Ok(LegacyTokenMigrationSummary::default());
    }
    let legacy_store = read_token_fallback_store_at_path(path)?;
    let mut summary = LegacyTokenMigrationSummary::default();

    for (raw_account_id, raw_token) in legacy_store.refresh_tokens {
        let account_id = raw_account_id.trim();
        let refresh_token = raw_token.trim();
        if account_id.is_empty() || refresh_token.is_empty() {
            summary.skipped += 1;
            continue;
        }
        match keyring_set_refresh_token(account_id, refresh_token) {
            Ok(()) => summary.migrated += 1,
            Err(e) => {
                summary.failed += 1;
                eprintln!(
                    "legacy refresh-token migration failed for account '{}': {}",
                    account_id, e
                );
            }
        }
    }

    fs::remove_file(path).map_err(|e| format!("remove launcher token fallback failed: {e}"))?;
    summary.fallback_files_removed = 1;
    Ok(summary)
}

fn legacy_token_fallback_paths(app: &tauri::AppHandle) -> Vec<PathBuf> {
    fn push_unique(out: &mut Vec<PathBuf>, path: PathBuf) {
        if !out.iter().any(|existing| existing == &path) {
            out.push(path);
        }
    }

    let mut out = Vec::new();
    if let Ok(current) = launcher_token_fallback_path(app) {
        push_unique(&mut out, current);
    }
    if let Some(data_dir) = tauri::api::path::data_dir() {
        for legacy_app_id in [
            "com.adrien.modpackmanager",
            "io.github.pixelied.openjarlauncher",
            "openjar-launcher",
            "modpack-manager",
        ] {
            push_unique(
                &mut out,
                data_dir
                    .join(legacy_app_id)
                    .join("launcher")
                    .join(LAUNCHER_TOKEN_FALLBACK_FILE),
            );
        }
    }
    out
}

fn migrate_legacy_refresh_tokens_to_keyring(app: &tauri::AppHandle) -> Result<(), String> {
    let mut summary = LegacyTokenMigrationSummary::default();
    for path in legacy_token_fallback_paths(app) {
        let path_summary = migrate_legacy_refresh_tokens_from_path(&path)?;
        summary.migrated += path_summary.migrated;
        summary.skipped += path_summary.skipped;
        summary.failed += path_summary.failed;
        summary.fallback_files_removed += path_summary.fallback_files_removed;
    }
    if summary.migrated == 0
        && summary.skipped == 0
        && summary.failed == 0
        && summary.fallback_files_removed == 0
    {
        return Ok(());
    }
    eprintln!(
        "legacy refresh-token fallback migration complete: migrated={}, skipped={}, failed={}, fallback_files_removed={}",
        summary.migrated, summary.skipped, summary.failed, summary.fallback_files_removed
    );
    if summary.failed > 0 {
        return Err(format!(
            "Failed to migrate {} legacy refresh token(s) to OS secure storage. {}",
            summary.failed,
            keyring_unavailable_hint()
        ));
    }
    Ok(())
}

fn migrate_selected_refresh_alias(app: &tauri::AppHandle) -> Result<(), String> {
    let settings = read_launcher_settings(app)?;
    let accounts = read_launcher_accounts(app)?;
    if let Some(existing) = keyring_get_selected_refresh_token()? {
        if !existing.trim().is_empty() {
            return Ok(());
        }
    }

    if let Some(selected_account) = settings
        .selected_account_id
        .as_deref()
        .and_then(|id| accounts.iter().find(|account| account.id == id))
        .cloned()
    {
        if let Some(token) =
            read_refresh_token_from_keyring_aliases_only(&selected_account, &accounts)?
        {
            keyring_set_selected_refresh_token(&token)?;
            runtime_refresh_token_cache_set(&selected_account.id, &token);
            runtime_refresh_token_cache_set(&selected_account.username, &token);
            return Ok(());
        }
    }

    let mut found_owner: Option<LauncherAccount> = None;
    let mut found_token: Option<String> = None;
    for account in &accounts {
        let Some(token) = read_refresh_token_from_keyring_aliases_only(account, &accounts)? else {
            continue;
        };
        if token.trim().is_empty() {
            continue;
        }
        match found_token.as_ref() {
            None => {
                found_owner = Some(account.clone());
                found_token = Some(token);
            }
            Some(existing) if existing == &token => {}
            Some(_) => {
                return Ok(());
            }
        }
    }

    if let (Some(owner), Some(token)) = (found_owner, found_token) {
        keyring_set_selected_refresh_token(&token)?;
        runtime_refresh_token_cache_set(&owner.id, &token);
        runtime_refresh_token_cache_set(&owner.username, &token);
        if settings.selected_account_id.as_deref() != Some(owner.id.as_str()) {
            let mut updated = settings.clone();
            updated.selected_account_id = Some(owner.id);
            write_launcher_settings(app, &updated)?;
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct MsoTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MsoDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    interval: u64,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XboxAuthResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XboxDisplayClaims,
}

#[derive(Debug, Deserialize)]
struct XboxDisplayClaims {
    xui: Vec<XboxUserClaim>,
}

#[derive(Debug, Deserialize)]
struct XboxUserClaim {
    uhs: String,
}

#[derive(Debug, Deserialize)]
struct McAuthResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct McEntitlementsResponse {
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct McProfileCosmetic {
    #[serde(default)]
    id: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    variant: Option<String>,
}

#[derive(Debug, Deserialize)]
struct McProfileResponse {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<McProfileCosmetic>,
    #[serde(default)]
    capes: Vec<McProfileCosmetic>,
}

fn microsoft_refresh_access_token(
    client: &Client,
    client_id: &str,
    refresh_token: &str,
) -> Result<MsoTokenResponse, String> {
    let params = [
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("scope", "XboxLive.signin XboxLive.offline_access"),
    ];
    let res = client
        .post(MS_TOKEN_URL)
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .map_err(|e| format!("Microsoft refresh failed: {e}"))?;
    if !res.status().is_success() {
        return Err(format!(
            "Microsoft refresh failed with status {}",
            res.status()
        ));
    }
    res.json::<MsoTokenResponse>()
        .map_err(|e| format!("parse Microsoft refresh failed: {e}"))
}

fn microsoft_begin_device_code(
    client: &Client,
    client_id: &str,
) -> Result<MsoDeviceCodeResponse, String> {
    let params = [
        ("client_id", client_id),
        ("scope", "XboxLive.signin XboxLive.offline_access"),
    ];
    let res = client
        .post(MS_DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .map_err(|e| format!("Microsoft device code start failed: {e}"))?;
    if !res.status().is_success() {
        return Err(format!(
            "Microsoft device code start failed with status {}",
            res.status()
        ));
    }
    res.json::<MsoDeviceCodeResponse>()
        .map_err(|e| format!("parse Microsoft device code response failed: {e}"))
}

fn microsoft_access_to_mc_token(client: &Client, msa_access_token: &str) -> Result<String, String> {
    let xbl_req_with_prefix = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={}", msa_access_token),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let mut xbl = post_json_with_status_retry(
        client,
        XBL_AUTH_URL,
        &xbl_req_with_prefix,
        "Xbox Live auth",
        &[("x-xbl-contract-version", "1")],
    )?;

    if !xbl.status().is_success() {
        // Some environments are picky about the ticket prefix. Retry once with raw token.
        if xbl.status().as_u16() == 400 || xbl.status().as_u16() == 401 {
            let xbl_req_plain = serde_json::json!({
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": msa_access_token,
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT"
            });
            xbl = post_json_with_status_retry(
                client,
                XBL_AUTH_URL,
                &xbl_req_plain,
                "Xbox Live auth",
                &[("x-xbl-contract-version", "1")],
            )?;
        }
    }

    if !xbl.status().is_success() {
        let status = xbl.status();
        let body = xbl.text().unwrap_or_default();
        return Err(format!(
            "Xbox Live auth failed with status {}{}",
            status,
            if body.trim().is_empty() {
                "".to_string()
            } else {
                format!(" ({})", trim_error_body(&body))
            }
        ));
    }
    let xbl_data = xbl
        .json::<XboxAuthResponse>()
        .map_err(|e| format!("parse Xbox Live auth failed: {e}"))?;
    let uhs = xbl_data
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .ok_or_else(|| "Xbox auth response missing uhs".to_string())?;

    let xsts_req = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_data.token],
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let xsts = post_json_with_status_retry(
        client,
        XSTS_AUTH_URL,
        &xsts_req,
        "XSTS auth",
        &[("x-xbl-contract-version", "1")],
    )?;
    if !xsts.status().is_success() {
        let status = xsts.status();
        let body = xsts.text().unwrap_or_default();
        if let Some(xerr) = parse_xerr_code(&body) {
            if let Some(explained) = explain_xerr_code(xerr) {
                return Err(format!("XSTS auth failed ({xerr}): {explained}"));
            }
            return Err(format!("XSTS auth failed with XErr {xerr}."));
        }
        return Err(format!(
            "XSTS auth failed with status {}{}",
            status,
            if body.trim().is_empty() {
                "".to_string()
            } else {
                format!(" ({})", trim_error_body(&body))
            }
        ));
    }
    let xsts_data = xsts
        .json::<XboxAuthResponse>()
        .map_err(|e| format!("parse XSTS auth failed: {e}"))?;

    let identity_token = format!("XBL3.0 x={};{}", uhs, xsts_data.token);

    let launcher_req = serde_json::json!({
        "xtoken": identity_token,
        "platform": "PC_LAUNCHER",
    });
    let launcher_resp = post_json_with_status_retry(
        client,
        MC_LAUNCHER_AUTH_URL,
        &launcher_req,
        "Minecraft launcher login",
        &[],
    )?;
    if launcher_resp.status().is_success() {
        let mc_data = launcher_resp
            .json::<McAuthResponse>()
            .map_err(|e| format!("parse Minecraft launcher login failed: {e}"))?;
        return Ok(mc_data.access_token);
    }

    // Fallback for older response shapes.
    let mc_req = serde_json::json!({
        "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_data.token),
    });
    let mc = post_json_with_status_retry(client, MC_AUTH_URL, &mc_req, "Minecraft login", &[])?;
    if !mc.status().is_success() {
        let status = mc.status();
        let body = mc.text().unwrap_or_default();
        return Err(format!(
            "Minecraft login failed with status {}{}",
            status,
            if body.trim().is_empty() {
                "".to_string()
            } else {
                format!(" ({})", trim_error_body(&body))
            }
        ));
    }
    mc.json::<McAuthResponse>()
        .map(|v| v.access_token)
        .map_err(|e| format!("parse Minecraft login failed: {e}"))
}

fn ensure_minecraft_entitlement(client: &Client, mc_access_token: &str) -> Result<(), String> {
    let resp = client
        .get(MC_ENTITLEMENTS_URL)
        .header("Accept", "application/json")
        .bearer_auth(mc_access_token)
        .send()
        .map_err(|e| format!("Minecraft entitlements check failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Minecraft entitlements check failed with status {}",
            resp.status()
        ));
    }
    let payload = resp
        .json::<McEntitlementsResponse>()
        .map_err(|e| format!("parse Minecraft entitlements failed: {e}"))?;
    if payload.items.is_empty() {
        return Err("No Minecraft entitlement found for this Microsoft account.".to_string());
    }
    Ok(())
}

fn fetch_minecraft_profile(
    client: &Client,
    mc_access_token: &str,
) -> Result<McProfileResponse, String> {
    let resp = client
        .get(MC_PROFILE_URL)
        .header("Accept", "application/json")
        .bearer_auth(mc_access_token)
        .send()
        .map_err(|e| format!("Minecraft profile fetch failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Minecraft profile fetch failed with status {}",
            resp.status()
        ));
    }
    resp.json::<McProfileResponse>()
        .map_err(|e| format!("parse Minecraft profile failed: {e}"))
}

fn normalize_skin_variant(input: Option<&str>) -> &'static str {
    match input.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "slim" | "alex" => "slim",
        _ => "classic",
    }
}

fn parse_http_error_with_body(resp: Response) -> String {
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    let snippet = body.chars().take(280).collect::<String>();
    if snippet.is_empty() {
        format!("status {status}")
    } else {
        format!("status {status}: {snippet}")
    }
}

fn upload_minecraft_skin_png_bytes(
    client: &Client,
    mc_access_token: &str,
    variant: &str,
    bytes: Vec<u8>,
    file_name: String,
) -> Result<(), String> {
    let url = format!("{MC_PROFILE_URL}/skins");
    let part = multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str("image/png")
        .map_err(|e| format!("prepare skin upload failed: {e}"))?;
    let form = multipart::Form::new()
        .text("variant", variant.to_string())
        .part("file", part);
    let resp = client
        .post(&url)
        .bearer_auth(mc_access_token)
        .multipart(form)
        .send()
        .map_err(|e| format!("upload skin failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Minecraft skin upload failed ({})",
            parse_http_error_with_body(resp)
        ));
    }
    Ok(())
}

fn apply_minecraft_skin(
    client: &Client,
    mc_access_token: &str,
    skin_source: &str,
    skin_variant: Option<&str>,
) -> Result<(), String> {
    let source = skin_source.trim();
    if source.is_empty() {
        return Err("Skin source is empty.".to_string());
    }
    let variant = normalize_skin_variant(skin_variant);
    let url = format!("{MC_PROFILE_URL}/skins");
    if source.starts_with("http://") || source.starts_with("https://") {
        let json = serde_json::json!({
            "variant": variant,
            "url": source,
        });
        let by_url = client
            .post(&url)
            .bearer_auth(mc_access_token)
            .json(&json)
            .send()
            .map_err(|e| format!("set skin via URL failed: {e}"))?;
        if by_url.status().is_success() {
            return Ok(());
        }
        let by_url_err = parse_http_error_with_body(by_url);
        let downloaded = client
            .get(source)
            .header("Accept", "image/png,image/*;q=0.9,*/*;q=0.2")
            .send()
            .map_err(|e| {
                format!("download skin URL failed after URL apply failed ({by_url_err}): {e}")
            })?;
        if !downloaded.status().is_success() {
            return Err(format!(
                "Skin URL apply failed ({by_url_err}) and fallback download failed ({})",
                parse_http_error_with_body(downloaded)
            ));
        }
        let bytes = downloaded
            .bytes()
            .map_err(|e| format!("read downloaded skin bytes failed: {e}"))?
            .to_vec();
        if bytes.is_empty() {
            return Err(format!(
                "Skin URL apply failed ({by_url_err}) and downloaded image was empty."
            ));
        }
        let file_name = source
            .split('/')
            .next_back()
            .map(|s| s.split('?').next().unwrap_or(s))
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("skin.png")
            .to_string();
        upload_minecraft_skin_png_bytes(client, mc_access_token, variant, bytes, file_name).map_err(
            |e| format!("Skin URL apply failed ({by_url_err}); fallback upload failed: {e}"),
        )
    } else {
        let path = PathBuf::from(source);
        if !path.exists() || !path.is_file() {
            return Err("Selected skin file does not exist.".to_string());
        }
        let bytes = fs::read(&path).map_err(|e| format!("read skin file failed: {e}"))?;
        if bytes.is_empty() {
            return Err("Skin file is empty.".to_string());
        }
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("skin.png")
            .to_string();
        upload_minecraft_skin_png_bytes(client, mc_access_token, variant, bytes, file_name)
    }
}

fn apply_minecraft_cape(
    client: &Client,
    mc_access_token: &str,
    cape_id: Option<&str>,
) -> Result<(), String> {
    let url = format!("{MC_PROFILE_URL}/capes/active");
    let trimmed = cape_id.unwrap_or("").trim();

    let resp = if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        client
            .delete(&url)
            .bearer_auth(mc_access_token)
            .send()
            .map_err(|e| format!("clear active cape failed: {e}"))?
    } else {
        let json = serde_json::json!({ "capeId": trimmed });
        client
            .put(&url)
            .bearer_auth(mc_access_token)
            .json(&json)
            .send()
            .map_err(|e| format!("set active cape failed: {e}"))?
    };

    if !resp.status().is_success() {
        return Err(format!(
            "Minecraft cape update failed ({})",
            parse_http_error_with_body(resp)
        ));
    }
    Ok(())
}

fn resolve_fabric_loader_version(client: &Client, mc_version: &str) -> Result<String, String> {
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc_version}");
    let resp = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("Fabric loader lookup failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Fabric loader lookup failed with status {}",
            resp.status()
        ));
    }
    let items = resp
        .json::<Vec<serde_json::Value>>()
        .map_err(|e| format!("parse Fabric loader lookup failed: {e}"))?;
    for it in &items {
        if let Some(v) = it
            .get("loader")
            .and_then(|x| x.get("version"))
            .and_then(|x| x.as_str())
        {
            return Ok(v.to_string());
        }
    }
    Err(format!(
        "No compatible Fabric loader version found for Minecraft {}",
        mc_version
    ))
}

fn resolve_forge_loader_version(client: &Client, mc_version: &str) -> Result<String, String> {
    let url = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
    let resp = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("Forge loader lookup failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Forge loader lookup failed with status {}",
            resp.status()
        ));
    }
    let payload = resp
        .json::<serde_json::Value>()
        .map_err(|e| format!("parse Forge loader lookup failed: {e}"))?;
    let promos = payload
        .get("promos")
        .and_then(|x| x.as_object())
        .ok_or_else(|| "Forge promotions payload missing promos".to_string())?;
    let rec_key = format!("{mc_version}-recommended");
    if let Some(v) = promos.get(&rec_key).and_then(|x| x.as_str()) {
        return Ok(v.to_string());
    }
    let latest_key = format!("{mc_version}-latest");
    if let Some(v) = promos.get(&latest_key).and_then(|x| x.as_str()) {
        return Ok(v.to_string());
    }
    let mut candidates: Vec<String> = promos
        .iter()
        .filter_map(|(k, v)| {
            if !k.starts_with(&format!("{mc_version}-")) {
                return None;
            }
            v.as_str().map(|s| s.to_string())
        })
        .collect();
    candidates.sort();
    candidates.pop().ok_or_else(|| {
        format!(
            "No compatible Forge version found for Minecraft {}",
            mc_version
        )
    })
}

fn safe_mod_filename(project_id: &str, version_id: &str, source_filename: &str) -> String {
    let cleaned = sanitize_filename(source_filename);
    if cleaned.is_empty() {
        format!("{project_id}-{version_id}.jar")
    } else {
        cleaned
    }
}

fn pick_compatible_version(
    versions: Vec<ModrinthVersion>,
    instance: &Instance,
) -> Option<ModrinthVersion> {
    let mut compatible: Vec<ModrinthVersion> = versions
        .into_iter()
        .filter(|v| {
            v.game_versions.iter().any(|gv| gv == &instance.mc_version)
                && v.loaders.iter().any(|l| l == &instance.loader)
        })
        .collect();
    compatible.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    compatible.into_iter().next()
}

fn fetch_project_versions(
    client: &Client,
    project_id: &str,
) -> Result<Vec<ModrinthVersion>, String> {
    let versions_url = format!("{}/project/{project_id}/version", modrinth_api_base());
    let versions_resp = client
        .get(&versions_url)
        .send()
        .map_err(|e| format!("fetch versions failed for {project_id}: {e}"))?;
    if !versions_resp.status().is_success() {
        return Err(format!(
            "fetch versions failed for {project_id} with status {}",
            versions_resp.status()
        ));
    }

    let mut versions: Vec<ModrinthVersion> = versions_resp
        .json()
        .map_err(|e| format!("parse versions failed for {project_id}: {e}"))?;
    for v in &mut versions {
        if v.project_id.trim().is_empty() {
            v.project_id = project_id.to_string();
        }
    }
    Ok(versions)
}

fn fetch_version_by_id(client: &Client, version_id: &str) -> Result<ModrinthVersion, String> {
    let url = format!("{}/version/{version_id}", modrinth_api_base());
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("fetch dependency version {version_id} failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "fetch dependency version {version_id} failed with status {}",
            resp.status()
        ));
    }
    resp.json::<ModrinthVersion>()
        .map_err(|e| format!("parse dependency version {version_id} failed: {e}"))
}

fn resolve_modrinth_install_plan(
    client: &Client,
    instance: &Instance,
    root_project_id: &str,
) -> Result<Vec<ResolvedInstallMod>, String> {
    let mut project_versions_cache: HashMap<String, Vec<ModrinthVersion>> = HashMap::new();
    let mut version_by_id_cache: HashMap<String, ModrinthVersion> = HashMap::new();
    let mut resolved: Vec<ResolvedInstallMod> = Vec::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();

    queue.push_back(root_project_id.to_string());

    while let Some(project_id) = queue.pop_front() {
        if !visited.insert(project_id.clone()) {
            continue;
        }

        let versions = if let Some(cached) = project_versions_cache.get(&project_id) {
            cached.clone()
        } else {
            let fetched = fetch_project_versions(client, &project_id)?;
            project_versions_cache.insert(project_id.clone(), fetched.clone());
            fetched
        };

        let version = pick_compatible_version(versions, instance).ok_or_else(|| {
            format!(
                "No compatible Modrinth version found for project {} ({} + {})",
                project_id, instance.loader, instance.mc_version
            )
        })?;

        for dep in &version.dependencies {
            if !dep.dependency_type.eq_ignore_ascii_case("required") {
                continue;
            }

            let dep_project_id = if let Some(pid) = dep.project_id.as_ref() {
                Some(pid.clone())
            } else if let Some(version_id) = dep.version_id.as_ref() {
                let dep_version = if let Some(cached) = version_by_id_cache.get(version_id) {
                    cached.clone()
                } else {
                    let fetched = fetch_version_by_id(client, version_id)?;
                    version_by_id_cache.insert(version_id.clone(), fetched.clone());
                    fetched
                };
                if dep_version.project_id.trim().is_empty() {
                    None
                } else {
                    Some(dep_version.project_id)
                }
            } else {
                None
            };

            if let Some(dep_pid) = dep_project_id {
                if dep_pid != project_id && !visited.contains(&dep_pid) {
                    queue.push_back(dep_pid);
                }
            }
        }

        let file = version
            .files
            .iter()
            .find(|f| f.primary.unwrap_or(false))
            .or_else(|| version.files.first())
            .cloned()
            .ok_or_else(|| format!("Version {} has no downloadable files", version.id))?;

        resolved.push(ResolvedInstallMod {
            project_id,
            version,
            file,
        });
    }

    Ok(resolved)
}

fn is_plan_entry_up_to_date(
    instance_dir: &Path,
    lock: &Lockfile,
    item: &ResolvedInstallMod,
) -> bool {
    let safe_filename = safe_mod_filename(&item.project_id, &item.version.id, &item.file.filename);
    let Some(existing) = lock
        .entries
        .iter()
        .find(|e| e.project_id == item.project_id)
    else {
        return false;
    };
    if existing.version_id != item.version.id
        || existing.filename != safe_filename
        || !existing.enabled
    {
        return false;
    }
    let (enabled_path, _) = mod_paths(instance_dir, &existing.filename);
    enabled_path.exists()
}

fn count_plan_install_actions(
    instance_dir: &Path,
    lock: &Lockfile,
    plan: &[ResolvedInstallMod],
) -> usize {
    plan.iter()
        .filter(|item| !is_plan_entry_up_to_date(instance_dir, lock, item))
        .count()
}

fn remove_replaced_entries_for_project(
    lock: &mut Lockfile,
    instance_dir: &Path,
    project_id: &str,
    keep_enabled_filename: Option<&str>,
) -> Result<(), String> {
    let keep = keep_enabled_filename.unwrap_or("");
    let replaced: Vec<LockEntry> = lock
        .entries
        .iter()
        .filter(|e| e.project_id == project_id)
        .cloned()
        .collect();
    lock.entries.retain(|e| e.project_id != project_id);

    for old in replaced {
        let (old_enabled, old_disabled) = mod_paths(instance_dir, &old.filename);
        if old.filename != keep && old_enabled.exists() {
            fs::remove_file(&old_enabled)
                .map_err(|e| format!("remove old mod file '{}' failed: {e}", old.filename))?;
        }
        if old_disabled.exists() {
            fs::remove_file(&old_disabled).map_err(|e| {
                format!(
                    "remove old disabled mod file '{}' failed: {e}",
                    old.filename
                )
            })?;
        }
    }
    Ok(())
}

fn remove_replaced_entries_for_content(
    lock: &mut Lockfile,
    instance_dir: &Path,
    project_id: &str,
    content_type: &str,
) -> Result<(), String> {
    let normalized = normalize_lock_content_type(content_type);
    let replaced: Vec<LockEntry> = lock
        .entries
        .iter()
        .filter(|e| {
            e.project_id == project_id && normalize_lock_content_type(&e.content_type) == normalized
        })
        .cloned()
        .collect();
    lock.entries.retain(|e| {
        !(e.project_id == project_id && normalize_lock_content_type(&e.content_type) == normalized)
    });

    for old in replaced {
        match normalized.as_str() {
            "mods" => {
                let (old_enabled, old_disabled) = mod_paths(instance_dir, &old.filename);
                if old_enabled.exists() {
                    fs::remove_file(&old_enabled).map_err(|e| {
                        format!("remove old mod file '{}' failed: {e}", old.filename)
                    })?;
                }
                if old_disabled.exists() {
                    fs::remove_file(&old_disabled).map_err(|e| {
                        format!(
                            "remove old disabled mod file '{}' failed: {e}",
                            old.filename
                        )
                    })?;
                }
            }
            "resourcepacks" | "shaderpacks" => {
                let (enabled_path, disabled_path) =
                    content_paths_for_type(instance_dir, &normalized, &old.filename);
                if enabled_path.exists() {
                    fs::remove_file(&enabled_path).map_err(|e| {
                        format!("remove old file '{}' failed: {e}", enabled_path.display())
                    })?;
                }
                if disabled_path.exists() {
                    fs::remove_file(&disabled_path).map_err(|e| {
                        format!(
                            "remove old disabled file '{}' failed: {e}",
                            disabled_path.display()
                        )
                    })?;
                }
            }
            "datapacks" => {
                for world in old.target_worlds {
                    let (enabled_path, disabled_path) =
                        datapack_world_paths(instance_dir, &world, &old.filename);
                    if enabled_path.exists() {
                        fs::remove_file(&enabled_path).map_err(|e| {
                            format!(
                                "remove old datapack '{}' failed: {e}",
                                enabled_path.display()
                            )
                        })?;
                    }
                    if disabled_path.exists() {
                        fs::remove_file(&disabled_path).map_err(|e| {
                            format!(
                                "remove old disabled datapack '{}' failed: {e}",
                                disabled_path.display()
                            )
                        })?;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn fetch_project_title(client: &Client, project_id: &str) -> Option<String> {
    let project_url = format!("{}/project/{project_id}", modrinth_api_base());
    match client.get(&project_url).send() {
        Ok(resp) if resp.status().is_success() => match resp.json::<ModrinthProjectResponse>() {
            Ok(project) => Some(project.title),
            Err(_) => None,
        },
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateScope {
    AllContent,
    ModrinthModsOnly,
}

fn is_updatable_content_type(content_type: &str) -> bool {
    matches!(
        normalize_lock_content_type(content_type).as_str(),
        "mods" | "resourcepacks" | "shaderpacks" | "datapacks"
    )
}

fn parse_update_content_type_filter_value(input: &str) -> Option<String> {
    match input.trim().to_lowercase().as_str() {
        "mods" | "mod" => Some("mods".to_string()),
        "resourcepacks" | "resourcepack" => Some("resourcepacks".to_string()),
        "shaderpacks" | "shaderpack" | "shaders" | "shader" => Some("shaderpacks".to_string()),
        "datapacks" | "datapack" => Some("datapacks".to_string()),
        _ => None,
    }
}

fn normalize_update_content_type_filter(requested: Option<&[String]>) -> Option<HashSet<String>> {
    let values = requested?;
    let mut out = HashSet::new();
    for value in values {
        if let Some(normalized) = parse_update_content_type_filter_value(value) {
            out.insert(normalized);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn update_scope_allows_source(scope: UpdateScope, source: &str, content_type: &str) -> bool {
    match scope {
        UpdateScope::AllContent => {
            source == "modrinth" || source == "curseforge" || source == "github"
        }
        UpdateScope::ModrinthModsOnly => source == "modrinth" && content_type == "mods",
    }
}

fn effective_updatable_provider_for_entry(
    entry: &LockEntry,
    scope: UpdateScope,
) -> Option<ProviderCandidate> {
    let content_type = normalize_lock_content_type(&entry.content_type);
    if !is_updatable_content_type(&content_type) {
        return None;
    }

    let active_source = entry.source.trim().to_ascii_lowercase();
    if update_scope_allows_source(scope, &active_source, &content_type) {
        let active_candidate = ProviderCandidate {
            source: entry.source.clone(),
            project_id: entry.project_id.clone(),
            version_id: entry.version_id.clone(),
            name: entry.name.clone(),
            version_number: entry.version_number.clone(),
            confidence: None,
            reason: None,
        };
        if active_source != "github"
            || parse_github_project_id(&active_candidate.project_id).is_ok()
        {
            return Some(active_candidate);
        }
    }

    let mut candidates = lock_entry_provider_candidates(entry)
        .into_iter()
        .filter(|candidate| {
            let source = candidate.source.trim().to_ascii_lowercase();
            if !update_scope_allows_source(scope, &source, &content_type) {
                return false;
            }
            if source == "github" {
                return parse_github_project_id(&candidate.project_id).is_ok()
                    && provider_candidate_is_auto_activatable(candidate);
            }
            !candidate.project_id.trim().is_empty()
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by(|a, b| {
        provider_candidate_confidence_rank(b)
            .cmp(&provider_candidate_confidence_rank(a))
            .then_with(|| {
                provider_candidate_version_rank(b).cmp(&provider_candidate_version_rank(a))
            })
            .then_with(|| {
                provider_source_priority(&b.source).cmp(&provider_source_priority(&a.source))
            })
            .then_with(|| a.project_id.cmp(&b.project_id))
    });
    candidates.into_iter().next()
}

fn entry_allowed_in_update_scope(entry: &LockEntry, scope: UpdateScope) -> bool {
    let content_type = normalize_lock_content_type(&entry.content_type);
    if !is_updatable_content_type(&content_type) {
        return false;
    }
    effective_updatable_provider_for_entry(entry, scope).is_some()
}

fn entry_allowed_in_content_type_filter(
    entry: &LockEntry,
    content_type_filter: Option<&HashSet<String>>,
) -> bool {
    let Some(filter) = content_type_filter else {
        return true;
    };
    if filter.is_empty() {
        return true;
    }
    let content_type = normalize_lock_content_type(&entry.content_type);
    filter.contains(&content_type)
}

fn check_single_content_update_entry(
    client: &Client,
    instance: &Instance,
    entry: &LockEntry,
    cf_key: Option<&str>,
    scope: UpdateScope,
) -> Result<(Option<ContentUpdateInfo>, Vec<String>), String> {
    let mut warnings: Vec<String> = Vec::new();

    if entry
        .pinned_version
        .as_ref()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        warnings.push(format!(
            "Skipped pinned entry '{}' ({})",
            entry.name, entry.project_id
        ));
        return Ok((None, warnings));
    }

    let Some(effective_provider) = effective_updatable_provider_for_entry(entry, scope) else {
        return Ok((None, warnings));
    };
    let source = effective_provider.source.trim().to_ascii_lowercase();
    let content_type = normalize_lock_content_type(&entry.content_type);
    let project_id = effective_provider.project_id.trim().to_string();
    let current_version_id = if effective_provider.version_id.trim().is_empty() {
        entry.version_id.clone()
    } else {
        effective_provider.version_id.clone()
    };
    let current_version_number = if effective_provider.version_number.trim().is_empty() {
        entry.version_number.clone()
    } else {
        effective_provider.version_number.clone()
    };
    let display_name = if effective_provider.name.trim().is_empty() {
        entry.name.clone()
    } else {
        effective_provider.name.clone()
    };

    if source == "modrinth" {
        let versions = fetch_project_versions(client, &project_id)?;
        let latest_any = versions
            .iter()
            .max_by(|a, b| a.date_published.cmp(&b.date_published))
            .cloned();
        let Some(latest) = pick_compatible_version_for_content(versions, instance, &content_type)
        else {
            if let Some(incompatible_latest) = latest_any {
                warnings.push(format!(
                    "No compatible Modrinth update found for '{}' ({}). Latest release '{}' targets MC [{}] loaders [{}].",
                    display_name,
                    project_id,
                    incompatible_latest.version_number,
                    incompatible_latest.game_versions.join(", "),
                    incompatible_latest.loaders.join(", "),
                ));
            } else {
                warnings.push(format!(
                    "No compatible Modrinth update found for '{}' ({})",
                    display_name, project_id
                ));
            }
            return Ok((None, warnings));
        };
        if latest.id == current_version_id {
            return Ok((None, warnings));
        }
        let latest_file = latest
            .files
            .iter()
            .find(|f| f.primary.unwrap_or(false))
            .or_else(|| latest.files.first());
        let required_dependencies = latest
            .dependencies
            .iter()
            .filter(|dep| dep.dependency_type.eq_ignore_ascii_case("required"))
            .filter_map(|dep| dep.project_id.as_ref())
            .map(|project_id| project_id.trim().to_string())
            .filter(|dependency_project_id| {
                !dependency_project_id.is_empty() && dependency_project_id != &project_id
            })
            .collect::<Vec<_>>();
        let mut compatibility_notes: Vec<String> = Vec::new();
        if let Some(incompatible_latest) = latest_any {
            if incompatible_latest.id != latest.id {
                compatibility_notes.push(format!(
                    "Newest release '{}' is not compatible with this instance ({} / {}); selected latest compatible '{}'.",
                    incompatible_latest.version_number,
                    instance.loader,
                    instance.mc_version,
                    latest.version_number
                ));
            }
        }
        return Ok((
            Some(ContentUpdateInfo {
                source: "modrinth".to_string(),
                content_type,
                project_id: project_id.clone(),
                name: display_name.clone(),
                current_version_id,
                current_version_number,
                latest_version_id: latest.id,
                latest_version_number: latest.version_number,
                enabled: entry.enabled,
                target_worlds: entry.target_worlds.clone(),
                latest_file_name: latest_file.map(|f| f.filename.clone()),
                latest_download_url: latest_file.map(|f| f.url.clone()),
                latest_hashes: latest_file.map(|f| f.hashes.clone()).unwrap_or_default(),
                required_dependencies,
                compatibility_status: Some("compatible".to_string()),
                compatibility_notes,
            }),
            warnings,
        ));
    }

    if source == "curseforge" {
        let Some(api_key) = cf_key else {
            return Ok((None, warnings));
        };
        let mod_id = parse_curseforge_project_id(&project_id)?;
        let mut files = fetch_curseforge_files(client, api_key, mod_id)?;
        let latest_any = files
            .iter()
            .max_by(|a, b| a.file_date.cmp(&b.file_date))
            .cloned();
        files.retain(|f| {
            !f.file_name.trim().is_empty()
                && file_looks_compatible_with_instance(f, instance, &content_type)
        });
        files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
        let Some(latest) = files.into_iter().next() else {
            if let Some(incompatible_latest) = latest_any {
                warnings.push(format!(
                    "No compatible CurseForge update found for '{}' ({}). Latest file '{}' supports [{}].",
                    display_name,
                    project_id,
                    if incompatible_latest.display_name.trim().is_empty() {
                        incompatible_latest.file_name.clone()
                    } else {
                        incompatible_latest.display_name.clone()
                    },
                    incompatible_latest.game_versions.join(", "),
                ));
            } else {
                warnings.push(format!(
                    "No compatible CurseForge update found for '{}' ({})",
                    display_name, project_id
                ));
            }
            return Ok((None, warnings));
        };
        let latest_version_id = format!("cf_file:{}", latest.id);
        if latest_version_id == current_version_id {
            return Ok((None, warnings));
        }
        let latest_version_number = if latest.display_name.trim().is_empty() {
            latest.file_name.clone()
        } else {
            latest.display_name.clone()
        };
        let mut latest_download_url = latest
            .download_url
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        if latest_download_url.is_none() {
            match resolve_curseforge_file_download_url(client, api_key, mod_id, &latest) {
                Ok(url) => latest_download_url = Some(url),
                Err(err) => {
                    if error_mentions_forbidden(&err) {
                        warnings.push(format!(
                            "Skipped CurseForge update '{}' ({}): provider blocked automated download URL (403).",
                            display_name, project_id
                        ));
                        return Ok((None, warnings));
                    }
                    warnings.push(format!(
                        "Could not resolve download url for CurseForge update '{}' ({}): {}",
                        display_name, project_id, err
                    ));
                }
            }
        }
        let mut compatibility_notes: Vec<String> = Vec::new();
        if let Some(incompatible_latest) = latest_any {
            if incompatible_latest.id != latest.id {
                let latest_label = if incompatible_latest.display_name.trim().is_empty() {
                    incompatible_latest.file_name
                } else {
                    incompatible_latest.display_name
                };
                compatibility_notes.push(format!(
                    "Newest file '{}' is not compatible with this instance ({} / {}).",
                    latest_label, instance.loader, instance.mc_version
                ));
            }
        }
        return Ok((
            Some(ContentUpdateInfo {
                source: "curseforge".to_string(),
                content_type,
                project_id: project_id.clone(),
                name: display_name.clone(),
                current_version_id,
                current_version_number,
                latest_version_id,
                latest_version_number,
                enabled: entry.enabled,
                target_worlds: entry.target_worlds.clone(),
                latest_file_name: Some(latest.file_name.clone()),
                latest_download_url,
                latest_hashes: parse_cf_hashes(&latest),
                required_dependencies: latest
                    .dependencies
                    .iter()
                    .filter(|dep| {
                        dep.mod_id > 0 && curseforge_relation_is_required(dep.relation_type)
                    })
                    .map(|dep| format!("cf:{}", dep.mod_id))
                    .filter(|dependency_project_id| dependency_project_id != &project_id)
                    .collect(),
                compatibility_status: Some("compatible".to_string()),
                compatibility_notes,
            }),
            warnings,
        ));
    }

    if source == "github" {
        if content_type != "mods" {
            return Ok((None, warnings));
        }
        let (owner, repo_name) = parse_github_project_id(&project_id)?;
        let repo = fetch_github_repo(client, &owner, &repo_name)?;
        if let Some(reason) = github_repo_policy_rejection_reason(&repo) {
            warnings.push(format!(
                "Skipped GitHub update '{}' ({}): {}.",
                display_name, project_id, reason
            ));
            return Ok((None, warnings));
        }
        let releases = fetch_github_releases(client, &owner, &repo_name)?;
        let query_hint = github_release_query_hint(&entry.filename, &display_name, &repo);
        let mut repo_loader_hints: HashSet<String> = HashSet::new();
        let mut selection = select_github_release_with_asset(
            &repo,
            &releases,
            &query_hint,
            Some(&instance.mc_version),
            Some(&instance.loader),
            None,
            None,
        );
        if selection.is_none() {
            repo_loader_hints = fetch_github_repo_loader_hints(client, &repo);
            let repo_loader_hints_opt = if repo_loader_hints.is_empty() {
                None
            } else {
                Some(&repo_loader_hints)
            };
            selection = select_github_release_with_asset(
                &repo,
                &releases,
                &query_hint,
                Some(&instance.mc_version),
                Some(&instance.loader),
                None,
                repo_loader_hints_opt,
            );
        }
        let Some(selection) = selection else {
            let repo_loader_hints_opt = if repo_loader_hints.is_empty() {
                None
            } else {
                Some(&repo_loader_hints)
            };
            let has_any_release = select_github_release_with_asset(
                &repo,
                &releases,
                &query_hint,
                None,
                None,
                None,
                repo_loader_hints_opt,
            )
            .is_some();
            if has_any_release {
                warnings.push(format!(
                    "No compatible GitHub update found for '{}' ({}) on {} + {}.",
                    display_name, project_id, instance.loader, instance.mc_version
                ));
            } else {
                warnings.push(format!(
                    "No acceptable GitHub release with .jar asset found for '{}' ({})",
                    display_name, project_id
                ));
            }
            return Ok((None, warnings));
        };

        let latest_version_id = format!("gh_release:{}", selection.release.id);
        if github_release_selection_matches_current(
            &selection,
            &current_version_id,
            &current_version_number,
            &entry.hashes,
        ) || latest_version_id == current_version_id.trim()
        {
            return Ok((None, warnings));
        }
        let mut latest_hashes = extract_github_asset_digest(&selection.asset);
        if selection.has_checksum_sidecar {
            latest_hashes
                .entry("checksum_sidecar".to_string())
                .or_insert_with(|| "present".to_string());
        }
        return Ok((
            Some(ContentUpdateInfo {
                source: "github".to_string(),
                content_type,
                project_id: github_project_key(&owner, &repo_name),
                name: if display_name.trim().is_empty() {
                    if repo.full_name.trim().is_empty() {
                        format!("{owner}/{repo_name}")
                    } else {
                        repo.full_name.clone()
                    }
                } else {
                    display_name
                },
                current_version_id,
                current_version_number,
                latest_version_id,
                latest_version_number: github_release_version_label(&selection.release),
                enabled: entry.enabled,
                target_worlds: entry.target_worlds.clone(),
                latest_file_name: Some(selection.asset.name.clone()),
                latest_download_url: Some(selection.asset.browser_download_url.clone()),
                latest_hashes,
                required_dependencies: vec![],
                compatibility_status: Some("compatible".to_string()),
                compatibility_notes: vec![],
            }),
            warnings,
        ));
    }

    Ok((None, warnings))
}

fn update_check_entry_context(entry: &LockEntry, scope: UpdateScope) -> (String, String, String) {
    if let Some(provider) = effective_updatable_provider_for_entry(entry, scope) {
        let source = provider.source.trim().to_ascii_lowercase();
        let project_id = if provider.project_id.trim().is_empty() {
            entry.project_id.trim().to_string()
        } else {
            provider.project_id.trim().to_string()
        };
        let name = if provider.name.trim().is_empty() {
            entry.name.trim().to_string()
        } else {
            provider.name.trim().to_string()
        };
        return (source, project_id, name);
    }
    (
        entry.source.trim().to_ascii_lowercase(),
        entry.project_id.trim().to_string(),
        entry.name.trim().to_string(),
    )
}

fn update_check_entry_failure_warning(
    entry: &LockEntry,
    scope: UpdateScope,
    err: &str,
) -> (String, String, String, String) {
    let (source, project_id, name) = update_check_entry_context(entry, scope);
    let label = if name.trim().is_empty() {
        entry.filename.clone()
    } else {
        name
    };
    let warning = format!(
        "Skipped update check for '{}' [{}:{}]: {}",
        label, source, project_id, err
    );
    (warning, source, project_id, label)
}

fn check_instance_content_updates_inner(
    client: &Client,
    instance: &Instance,
    lock: &Lockfile,
    scope: UpdateScope,
    content_type_filter: Option<&HashSet<String>>,
) -> Result<ContentUpdateCheckResult, String> {
    let mut updates: Vec<ContentUpdateInfo> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut github_rate_limit_skipped_entries = 0usize;
    let mut github_rate_limit_reset_hint: Option<String> = None;
    let candidate_entries: Vec<LockEntry> = lock
        .entries
        .iter()
        .filter(|e| {
            entry_allowed_in_update_scope(e, scope)
                && entry_allowed_in_content_type_filter(e, content_type_filter)
        })
        .cloned()
        .collect();
    let checked_entries = candidate_entries.len();

    let has_cf_entries = candidate_entries.iter().any(|entry| {
        effective_updatable_provider_for_entry(entry, scope)
            .map(|provider| provider.source.trim().eq_ignore_ascii_case("curseforge"))
            .unwrap_or(false)
    });
    let cf_key = curseforge_api_key();
    if has_cf_entries && cf_key.is_none() {
        warnings.push(
            "CurseForge key unavailable, skipped CurseForge update checks for this instance."
                .to_string(),
        );
    }

    if candidate_entries.is_empty() {
        return Ok(ContentUpdateCheckResult {
            checked_entries,
            update_count: 0,
            updates,
            warnings,
        });
    }

    let update_entry_worker_max =
        env_worker_cap_or_default(UPDATE_ENTRY_WORKERS_MAX_ENV, 12, 1, 24);
    let parallelism = std::thread::available_parallelism()
        .map(|v| v.get())
        .unwrap_or(4)
        .clamp(1, 24)
        .min(update_entry_worker_max)
        .min(candidate_entries.len());

    if parallelism <= 1 {
        for entry in &candidate_entries {
            match check_single_content_update_entry(
                client,
                instance,
                entry,
                cf_key.as_deref(),
                scope,
            ) {
                Ok((maybe_update, mut local_warnings)) => {
                    if let Some(update) = maybe_update {
                        updates.push(update);
                    }
                    warnings.append(&mut local_warnings);
                }
                Err(err) => {
                    let (warning, source, _, _) =
                        update_check_entry_failure_warning(entry, scope, &err);
                    if source.eq_ignore_ascii_case("github") && github_error_is_rate_limit(&err) {
                        github_rate_limit_skipped_entries += 1;
                        if github_rate_limit_reset_hint.is_none() {
                            github_rate_limit_reset_hint =
                                github_rate_limit_reset_hint_from_error(&err);
                        }
                    } else {
                        warnings.push(warning);
                    }
                }
            }
        }
    } else {
        let mut partitions: Vec<Vec<LockEntry>> = vec![Vec::new(); parallelism];
        for (idx, entry) in candidate_entries.into_iter().enumerate() {
            partitions[idx % parallelism].push(entry);
        }

        let instance_snapshot = instance.clone();
        let cf_key_snapshot = cf_key.clone();
        std::thread::scope(|scope_ctx| -> Result<(), String> {
            let mut handles = Vec::new();
            for chunk in partitions {
                let client_local = client.clone();
                let instance_local = instance_snapshot.clone();
                let cf_key_local = cf_key_snapshot.clone();
                handles.push(scope_ctx.spawn(
                    move || -> Result<(Vec<ContentUpdateInfo>, Vec<String>, usize, Option<String>), String> {
                        let mut local_updates = Vec::new();
                        let mut local_warnings = Vec::new();
                        let mut local_github_rate_limit_skips = 0usize;
                        let mut local_github_rate_limit_reset: Option<String> = None;
                        for entry in &chunk {
                            match check_single_content_update_entry(
                                &client_local,
                                &instance_local,
                                entry,
                                cf_key_local.as_deref(),
                                scope,
                            ) {
                                Ok((maybe_update, mut warnings_for_entry)) => {
                                    if let Some(update) = maybe_update {
                                        local_updates.push(update);
                                    }
                                    local_warnings.append(&mut warnings_for_entry);
                                }
                                Err(err) => {
                                    let (warning, source, _, _) =
                                        update_check_entry_failure_warning(entry, scope, &err);
                                    if source.eq_ignore_ascii_case("github")
                                        && github_error_is_rate_limit(&err)
                                    {
                                        local_github_rate_limit_skips += 1;
                                        if local_github_rate_limit_reset.is_none() {
                                            local_github_rate_limit_reset =
                                                github_rate_limit_reset_hint_from_error(&err);
                                        }
                                    } else {
                                        local_warnings.push(warning);
                                    }
                                }
                            }
                        }
                        Ok((
                            local_updates,
                            local_warnings,
                            local_github_rate_limit_skips,
                            local_github_rate_limit_reset,
                        ))
                    },
                ));
            }

            for handle in handles {
                let joined = handle
                    .join()
                    .map_err(|_| "update-check worker thread panicked".to_string())?;
                let (
                    mut local_updates,
                    mut local_warnings,
                    local_github_rate_limit_skips,
                    local_github_rate_limit_reset,
                ) = joined?;
                updates.append(&mut local_updates);
                warnings.append(&mut local_warnings);
                github_rate_limit_skipped_entries += local_github_rate_limit_skips;
                if github_rate_limit_reset_hint.is_none() {
                    github_rate_limit_reset_hint = local_github_rate_limit_reset;
                }
            }
            Ok(())
        })?;
    }

    if github_rate_limit_skipped_entries > 0 {
        warnings.push(format!(
            "GitHub checks paused due to rate limit; skipped {} GitHub entr{} this run.{}",
            github_rate_limit_skipped_entries,
            if github_rate_limit_skipped_entries == 1 {
                "y"
            } else {
                "ies"
            },
            github_rate_limit_reset_hint
                .map(|value| format!(" Resets around {value}."))
                .unwrap_or_default()
        ));
    }

    updates.sort_by(|a, b| {
        let by_name = a.name.to_lowercase().cmp(&b.name.to_lowercase());
        if by_name != std::cmp::Ordering::Equal {
            return by_name;
        }
        let by_source = a.source.cmp(&b.source);
        if by_source != std::cmp::Ordering::Equal {
            return by_source;
        }
        a.project_id.cmp(&b.project_id)
    });
    Ok(ContentUpdateCheckResult {
        checked_entries,
        update_count: updates.len(),
        updates,
        warnings,
    })
}

fn lock_has_enabled_modrinth_mod(lock: &Lockfile, project_id: &str) -> bool {
    lock.entries.iter().any(|entry| {
        entry.source.eq_ignore_ascii_case("modrinth")
            && normalize_lock_content_type(&entry.content_type) == "mods"
            && entry.enabled
            && entry.project_id == project_id
    })
}

fn lock_has_enabled_curseforge_mod(lock: &Lockfile, mod_id: i64) -> bool {
    if mod_id <= 0 {
        return false;
    }
    lock.entries.iter().any(|entry| {
        if !entry.source.eq_ignore_ascii_case("curseforge")
            || normalize_lock_content_type(&entry.content_type) != "mods"
            || !entry.enabled
        {
            return false;
        }
        parse_curseforge_project_id(&entry.project_id)
            .map(|entry_mod_id| entry_mod_id == mod_id)
            .unwrap_or(false)
    })
}

fn modrinth_required_dependencies_satisfied(lock: &Lockfile, version: &ModrinthVersion) -> bool {
    let root_project_id = version.project_id.trim();
    for dep in &version.dependencies {
        if !dep.dependency_type.eq_ignore_ascii_case("required") {
            continue;
        }
        let Some(project_id) = dep.project_id.as_ref() else {
            return false;
        };
        let project_id = project_id.trim();
        if project_id.is_empty() || project_id == root_project_id {
            continue;
        }
        if !lock_has_enabled_modrinth_mod(lock, project_id) {
            return false;
        }
    }
    true
}

fn modrinth_required_dependency_list_satisfied(lock: &Lockfile, dependencies: &[String]) -> bool {
    for dependency in dependencies {
        let project_id = dependency.trim();
        if project_id.is_empty() {
            continue;
        }
        if !lock_has_enabled_modrinth_mod(lock, project_id) {
            return false;
        }
    }
    true
}

fn curseforge_required_dependencies_satisfied(
    lock: &Lockfile,
    file: &CurseforgeFile,
    root_mod_id: i64,
) -> bool {
    for dep in &file.dependencies {
        if dep.mod_id <= 0
            || !curseforge_relation_is_required(dep.relation_type)
            || dep.mod_id == root_mod_id
        {
            continue;
        }
        if !lock_has_enabled_curseforge_mod(lock, dep.mod_id) {
            return false;
        }
    }
    true
}

fn curseforge_required_dependency_list_satisfied(lock: &Lockfile, dependencies: &[String]) -> bool {
    for dependency in dependencies {
        let Some(mod_id) = parse_curseforge_project_id(dependency).ok() else {
            continue;
        };
        if !lock_has_enabled_curseforge_mod(lock, mod_id) {
            return false;
        }
    }
    true
}

fn parse_curseforge_file_id(version_id: &str) -> Option<i64> {
    let trimmed = version_id.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(raw) = trimmed.strip_prefix("cf_file:") {
        return raw.trim().parse::<i64>().ok().filter(|id| *id > 0);
    }
    trimmed.parse::<i64>().ok().filter(|id| *id > 0)
}

fn carried_pinned_version_for_update(
    lock: &Lockfile,
    update: &ContentUpdateInfo,
) -> Option<String> {
    let update_source = update.source.trim().to_ascii_lowercase();
    let update_content_type = normalize_lock_content_type(&update.content_type);
    let update_project_id = update.project_id.trim();
    for entry in &lock.entries {
        if normalize_lock_content_type(&entry.content_type) != update_content_type {
            continue;
        }
        if !entry.source.trim().eq_ignore_ascii_case(&update_source) {
            continue;
        }
        let entry_pin = entry
            .pinned_version
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if entry_pin.is_none() {
            continue;
        }
        if !update.current_version_id.trim().is_empty()
            && entry.version_id.trim() == update.current_version_id.trim()
        {
            return entry_pin;
        }
        if update_source == "modrinth"
            && entry
                .project_id
                .trim()
                .eq_ignore_ascii_case(update_project_id)
        {
            return entry_pin;
        }
        if update_source == "curseforge" {
            let update_mod_id = parse_curseforge_project_id(update_project_id).ok();
            let entry_mod_id = parse_curseforge_project_id(&entry.project_id).ok();
            if update_mod_id.is_some() && entry_mod_id.is_some() && update_mod_id == entry_mod_id {
                return entry_pin;
            }
        }
        if update_source == "github" {
            let update_repo = parse_github_project_id(update_project_id).ok();
            let entry_repo = parse_github_project_id(&entry.project_id).ok();
            if update_repo.is_some() && entry_repo.is_some() && update_repo == entry_repo {
                return entry_pin;
            }
        }
    }
    None
}

fn disable_mod_file(instance_dir: &Path, filename: &str) -> Result<(), String> {
    let (enabled_path, disabled_path) = mod_paths(instance_dir, filename);
    if disabled_path.exists() {
        return Ok(());
    }
    if !enabled_path.exists() {
        return Err("mod file not found on disk".to_string());
    }
    fs::rename(&enabled_path, &disabled_path).map_err(|e| format!("disable mod failed: {e}"))?;
    Ok(())
}

#[derive(Debug, Clone)]
enum PrefetchedDownload {
    Ready(Vec<u8>),
    Failed(String),
}

#[derive(Debug, Clone)]
struct PrefetchJob {
    index: usize,
    update: ContentUpdateInfo,
}

fn adaptive_update_prefetch_worker_cap(updates: &[ContentUpdateInfo]) -> usize {
    let mut eligible = 0usize;
    let mut curseforge_jobs = 0usize;
    for update in updates {
        let normalized = normalize_lock_content_type(&update.content_type);
        if !is_updatable_content_type(&normalized) {
            continue;
        }
        let has_url = update
            .latest_download_url
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        if !has_url {
            continue;
        }
        eligible += 1;
        if update.source.trim().eq_ignore_ascii_case("curseforge") {
            curseforge_jobs += 1;
        }
    }
    if eligible <= 1 {
        return eligible.max(1);
    }

    let prefetch_worker_max = env_worker_cap_or_default(UPDATE_PREFETCH_WORKERS_MAX_ENV, 24, 1, 32);
    let cpu = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 24);
    let target_by_workload = if eligible >= 32 {
        14
    } else if eligible >= 20 {
        12
    } else if eligible >= 12 {
        10
    } else if eligible >= 6 {
        8
    } else {
        6
    };
    let mut cap = target_by_workload
        .min(cpu.saturating_mul(2))
        .min(prefetch_worker_max)
        .min(eligible)
        .max(1);
    if curseforge_jobs > 0 {
        // CF can throttle aggressively; keep mixed-source prefetch stable.
        cap = if curseforge_jobs * 2 >= eligible {
            cap.min(6)
        } else {
            cap.min(8)
        };
    }
    cap.max(1)
}

fn prefetch_update_downloads(
    client: &Client,
    updates: &[ContentUpdateInfo],
    worker_cap: usize,
) -> HashMap<usize, PrefetchedDownload> {
    let mut queue: VecDeque<PrefetchJob> = VecDeque::new();
    for (index, update) in updates.iter().enumerate() {
        let normalized = normalize_lock_content_type(&update.content_type);
        if !is_updatable_content_type(&normalized) {
            continue;
        }
        let source = update.source.trim().to_lowercase();
        if source != "modrinth" && source != "curseforge" && source != "github" {
            continue;
        }
        let Some(download_url) = update
            .latest_download_url
            .as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        else {
            continue;
        };
        let mut prepared = update.clone();
        prepared.latest_download_url = Some(download_url.to_string());
        queue.push_back(PrefetchJob {
            index,
            update: prepared,
        });
    }
    if queue.is_empty() {
        return HashMap::new();
    }

    let jobs = Arc::new(Mutex::new(queue));
    let out: Arc<Mutex<HashMap<usize, PrefetchedDownload>>> = Arc::new(Mutex::new(HashMap::new()));
    let prefetch_worker_max = env_worker_cap_or_default(UPDATE_PREFETCH_WORKERS_MAX_ENV, 24, 1, 32);
    let worker_count = worker_cap.max(1).min(prefetch_worker_max).min({
        let guard = jobs.lock();
        match guard {
            Ok(items) => items.len(),
            Err(_) => 1,
        }
    });

    let mut handles = Vec::new();
    for _ in 0..worker_count {
        let jobs_ref = jobs.clone();
        let out_ref = out.clone();
        let client_ref = client.clone();
        handles.push(thread::spawn(move || loop {
            let next = {
                let mut guard = match jobs_ref.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                guard.pop_front()
            };
            let Some(job) = next else {
                return;
            };
            let download_url = job
                .update
                .latest_download_url
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            let Some(download_url) = download_url else {
                continue;
            };
            let result =
                download_bytes_with_retry(&client_ref, &download_url, &job.update.project_id)
                    .map(PrefetchedDownload::Ready)
                    .unwrap_or_else(PrefetchedDownload::Failed);
            if let Ok(mut guard) = out_ref.lock() {
                guard.insert(job.index, result);
            }
        }));
    }
    for handle in handles {
        let _ = handle.join();
    }
    match Arc::try_unwrap(out) {
        Ok(mutex) => mutex.into_inner().unwrap_or_default(),
        Err(shared) => shared.lock().map(|v| v.clone()).unwrap_or_default(),
    }
}

fn try_fast_install_content_update(
    instances_dir: &Path,
    instance: &Instance,
    args: &CheckUpdatesArgs,
    client: &Client,
    cf_key: Option<&str>,
    update: &ContentUpdateInfo,
    prefetched_download: Option<&PrefetchedDownload>,
) -> Result<Option<InstalledMod>, String> {
    let source = update.source.trim().to_lowercase();
    let normalized = normalize_lock_content_type(&update.content_type);
    if !is_updatable_content_type(&normalized) {
        return Ok(None);
    }

    let instance_dir = instance_dir_for_id(instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(instances_dir, &args.instance_id)?;
    let carried_pin = carried_pinned_version_for_update(&lock, update);

    if source == "modrinth" {
        let latest_version_id = update.latest_version_id.trim();
        if latest_version_id.is_empty() {
            return Ok(None);
        }
        let mut download_url = update
            .latest_download_url
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mut latest_file_name = update
            .latest_file_name
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mut latest_hashes = update.latest_hashes.clone();
        let mut latest_version_number = update.latest_version_number.clone();

        if normalized == "mods"
            && !update.required_dependencies.is_empty()
            && !modrinth_required_dependency_list_satisfied(&lock, &update.required_dependencies)
        {
            return Ok(None);
        }

        if download_url.is_none() || latest_file_name.is_none() || latest_hashes.is_empty() {
            let mut version = fetch_version_by_id(client, latest_version_id)?;
            if version.project_id.trim().is_empty() {
                version.project_id = update.project_id.clone();
            }
            if normalized == "mods" && !modrinth_required_dependencies_satisfied(&lock, &version) {
                return Ok(None);
            }
            let file = version
                .files
                .iter()
                .find(|f| f.primary.unwrap_or(false))
                .or_else(|| version.files.first())
                .cloned()
                .ok_or_else(|| format!("Version {} has no downloadable files", version.id))?;
            download_url = Some(file.url.clone());
            latest_file_name = Some(file.filename.clone());
            latest_hashes = file.hashes.clone();
            latest_version_number = version.version_number.clone();
        }

        let download_url =
            download_url.ok_or_else(|| "Missing Modrinth download URL".to_string())?;
        let latest_file_name =
            latest_file_name.ok_or_else(|| "Missing Modrinth filename".to_string())?;
        let safe_filename = sanitize_filename(&latest_file_name);
        if safe_filename.is_empty() {
            return Err("Resolved filename is invalid".to_string());
        }

        let bytes = if let Some(prefetched) = prefetched_download {
            match prefetched {
                PrefetchedDownload::Ready(bytes) => bytes.clone(),
                PrefetchedDownload::Failed(err) => {
                    return Err(format!("prefetch download failed: {err}"));
                }
            }
        } else {
            download_bytes_with_retry(client, &download_url, &update.project_id)?
        };

        let worlds = if normalized == "datapacks" {
            normalize_target_worlds_for_datapack(&instance_dir, &update.target_worlds)?
        } else {
            vec![]
        };
        write_download_to_content_targets(
            &instance_dir,
            &normalized,
            &safe_filename,
            &worlds,
            &bytes,
        )?;
        remove_replaced_entries_for_content(
            &mut lock,
            &instance_dir,
            &update.project_id,
            &normalized,
        )?;

        let new_entry = LockEntry {
            source: "modrinth".to_string(),
            project_id: update.project_id.clone(),
            version_id: latest_version_id.to_string(),
            name: canonical_lock_entry_name(
                &normalized,
                &safe_filename,
                if update.name.trim().is_empty() {
                    &update.project_id
                } else {
                    &update.name
                },
            ),
            version_number: latest_version_number.clone(),
            filename: safe_filename,
            content_type: normalized.clone(),
            target_scope: if normalized == "datapacks" {
                "world".to_string()
            } else {
                "instance".to_string()
            },
            target_worlds: worlds,
            pinned_version: carried_pin.clone(),
            enabled: update.enabled,
            hashes: latest_hashes,
            provider_candidates: vec![ProviderCandidate {
                source: "modrinth".to_string(),
                project_id: update.project_id.clone(),
                version_id: latest_version_id.to_string(),
                name: if update.name.trim().is_empty() {
                    update.project_id.clone()
                } else {
                    update.name.clone()
                },
                version_number: latest_version_number.clone(),
                confidence: None,
                reason: None,
            }],
            local_analysis: None,
        };
        if normalized == "mods" && !new_entry.enabled {
            disable_mod_file(&instance_dir, &new_entry.filename)?;
        }
        lock.entries.push(new_entry.clone());
        lock.entries
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        write_lockfile(instances_dir, &args.instance_id, &lock)?;
        return Ok(Some(lock_entry_to_installed(&instance_dir, &new_entry)));
    }

    if source == "curseforge" {
        let Some(api_key) = cf_key else {
            return Ok(None);
        };
        let mod_id = parse_curseforge_project_id(&update.project_id)?;
        let Some(latest_file_id) = parse_curseforge_file_id(&update.latest_version_id) else {
            return Ok(None);
        };
        if normalized == "mods"
            && !update.required_dependencies.is_empty()
            && !curseforge_required_dependency_list_satisfied(&lock, &update.required_dependencies)
        {
            return Ok(None);
        }

        let mut latest_file_name = update
            .latest_file_name
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mut download_url = update
            .latest_download_url
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mut latest_hashes = update.latest_hashes.clone();
        let mut latest_version_number = update.latest_version_number.clone();

        if latest_file_name.is_none() || download_url.is_none() || latest_hashes.is_empty() {
            let file = fetch_curseforge_file(client, api_key, mod_id, latest_file_id)?;
            if file.file_name.trim().is_empty()
                || !file_looks_compatible_with_instance(&file, instance, &normalized)
            {
                return Ok(None);
            }
            if normalized == "mods"
                && !curseforge_required_dependencies_satisfied(&lock, &file, mod_id)
            {
                return Ok(None);
            }
            latest_file_name = Some(file.file_name.clone());
            download_url = Some(resolve_curseforge_file_download_url(
                client, api_key, mod_id, &file,
            )?);
            latest_hashes = parse_cf_hashes(&file);
            latest_version_number = if file.display_name.trim().is_empty() {
                file.file_name.clone()
            } else {
                file.display_name.clone()
            };
        }

        let latest_file_name =
            latest_file_name.ok_or_else(|| "Missing CurseForge filename".to_string())?;
        let download_url =
            download_url.ok_or_else(|| "Missing CurseForge download URL".to_string())?;
        let safe_filename = sanitize_filename(&latest_file_name);
        if safe_filename.is_empty() {
            return Err("Resolved CurseForge filename is invalid".to_string());
        }
        let bytes = if let Some(prefetched) = prefetched_download {
            match prefetched {
                PrefetchedDownload::Ready(bytes) => bytes.clone(),
                PrefetchedDownload::Failed(err) => {
                    return Err(format!("prefetch download failed: {err}"));
                }
            }
        } else {
            download_bytes_with_retry(
                client,
                &download_url,
                &format!("cf:{mod_id}:{latest_file_id}"),
            )?
        };

        let worlds = if normalized == "datapacks" {
            normalize_target_worlds_for_datapack(&instance_dir, &update.target_worlds)?
        } else {
            vec![]
        };
        write_download_to_content_targets(
            &instance_dir,
            &normalized,
            &safe_filename,
            &worlds,
            &bytes,
        )?;
        let project_key = format!("cf:{mod_id}");
        remove_replaced_entries_for_content(&mut lock, &instance_dir, &project_key, &normalized)?;
        let fallback_name = if update.name.trim().is_empty() {
            format!("CurseForge {mod_id}")
        } else {
            update.name.clone()
        };

        let new_entry = LockEntry {
            source: "curseforge".to_string(),
            project_id: project_key,
            version_id: format!("cf_file:{}", latest_file_id),
            name: canonical_lock_entry_name(&normalized, &safe_filename, &fallback_name),
            version_number: latest_version_number.clone(),
            filename: safe_filename,
            content_type: normalized.clone(),
            target_scope: if normalized == "datapacks" {
                "world".to_string()
            } else {
                "instance".to_string()
            },
            target_worlds: worlds,
            pinned_version: carried_pin.clone(),
            enabled: update.enabled,
            hashes: latest_hashes,
            provider_candidates: vec![ProviderCandidate {
                source: "curseforge".to_string(),
                project_id: format!("cf:{mod_id}"),
                version_id: format!("cf_file:{}", latest_file_id),
                name: if update.name.trim().is_empty() {
                    format!("CurseForge {mod_id}")
                } else {
                    update.name.clone()
                },
                version_number: latest_version_number.clone(),
                confidence: None,
                reason: None,
            }],
            local_analysis: None,
        };
        if normalized == "mods" && !new_entry.enabled {
            disable_mod_file(&instance_dir, &new_entry.filename)?;
        }
        lock.entries.push(new_entry.clone());
        lock.entries
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        write_lockfile(instances_dir, &args.instance_id, &lock)?;
        return Ok(Some(lock_entry_to_installed(&instance_dir, &new_entry)));
    }

    if source == "github" {
        if normalized != "mods" {
            return Ok(None);
        }
        let (owner, repo_name) = parse_github_project_id(&update.project_id)?;
        let mut repo_full_name_hint: Option<String> = None;
        let mut latest_release_id = parse_github_release_id(&update.latest_version_id);
        let mut latest_file_name = update
            .latest_file_name
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mut download_url = update
            .latest_download_url
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mut latest_hashes = update.latest_hashes.clone();
        let mut latest_version_number = update.latest_version_number.clone();

        if latest_release_id.is_none() || latest_file_name.is_none() || download_url.is_none() {
            let repo = fetch_github_repo(client, &owner, &repo_name)?;
            if github_repo_policy_rejection_reason(&repo).is_some() {
                return Ok(None);
            }
            repo_full_name_hint = Some(repo.full_name.clone());
            let releases = fetch_github_releases(client, &owner, &repo_name)?;
            let current_filename_hint = lock
                .entries
                .iter()
                .find(|entry| {
                    if normalize_lock_content_type(&entry.content_type) != normalized {
                        return false;
                    }
                    if !entry.source.trim().eq_ignore_ascii_case("github") {
                        return false;
                    }
                    if !update.current_version_id.trim().is_empty()
                        && entry.version_id.trim() == update.current_version_id.trim()
                    {
                        return true;
                    }
                    let entry_repo = parse_github_project_id(&entry.project_id).ok();
                    let update_repo = parse_github_project_id(&update.project_id).ok();
                    entry_repo.is_some() && entry_repo == update_repo
                })
                .map(|entry| entry.filename.clone())
                .unwrap_or_default();
            let query_hint = github_release_query_hint(&current_filename_hint, &update.name, &repo);
            let mut selection = select_github_release_with_asset(
                &repo,
                &releases,
                &query_hint,
                Some(&instance.mc_version),
                Some(&instance.loader),
                None,
                None,
            );
            if selection.is_none() {
                let repo_loader_hints = fetch_github_repo_loader_hints(client, &repo);
                let repo_loader_hints_opt = if repo_loader_hints.is_empty() {
                    None
                } else {
                    Some(&repo_loader_hints)
                };
                selection = select_github_release_with_asset(
                    &repo,
                    &releases,
                    &query_hint,
                    Some(&instance.mc_version),
                    Some(&instance.loader),
                    None,
                    repo_loader_hints_opt,
                );
            }
            let Some(selection) = selection else {
                return Ok(None);
            };
            latest_release_id = Some(selection.release.id);
            latest_file_name = Some(selection.asset.name.clone());
            download_url = Some(selection.asset.browser_download_url.clone());
            latest_version_number = github_release_version_label(&selection.release);
            let digests = extract_github_asset_digest(&selection.asset);
            for (algo, value) in digests {
                latest_hashes.insert(algo, value);
            }
            if selection.has_checksum_sidecar {
                latest_hashes
                    .entry("checksum_sidecar".to_string())
                    .or_insert_with(|| "present".to_string());
            }
        }

        let latest_release_id = match latest_release_id {
            Some(id) if id > 0 => id,
            _ => return Ok(None),
        };
        let latest_file_name =
            latest_file_name.ok_or_else(|| "Missing GitHub release filename".to_string())?;
        let download_url =
            download_url.ok_or_else(|| "Missing GitHub release download URL".to_string())?;
        let safe_filename = sanitize_filename(&latest_file_name);
        if safe_filename.is_empty() {
            return Err("Resolved GitHub filename is invalid".to_string());
        }

        let bytes = if let Some(prefetched) = prefetched_download {
            match prefetched {
                PrefetchedDownload::Ready(bytes) => bytes.clone(),
                PrefetchedDownload::Failed(err) => {
                    return Err(format!("prefetch download failed: {err}"));
                }
            }
        } else {
            download_bytes_with_retry(
                client,
                &download_url,
                &format!("gh:{owner}/{repo_name}:{latest_release_id}"),
            )?
        };

        latest_hashes
            .entry("sha256".to_string())
            .or_insert_with(|| sha256_bytes_hex(&bytes));

        write_download_to_content_targets(&instance_dir, &normalized, &safe_filename, &[], &bytes)?;
        let project_key = github_project_key(&owner, &repo_name);
        remove_replaced_entries_for_content(&mut lock, &instance_dir, &project_key, &normalized)?;
        if update.project_id.trim() != project_key {
            remove_replaced_entries_for_content(
                &mut lock,
                &instance_dir,
                &update.project_id,
                &normalized,
            )?;
        }
        let fallback_name = if update.name.trim().is_empty() {
            if repo_full_name_hint
                .as_ref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                format!("{owner}/{repo_name}")
            } else {
                repo_full_name_hint
                    .clone()
                    .unwrap_or_else(|| format!("{owner}/{repo_name}"))
            }
        } else {
            update.name.clone()
        };

        let new_entry = LockEntry {
            source: "github".to_string(),
            project_id: project_key.clone(),
            version_id: format!("gh_release:{latest_release_id}"),
            name: canonical_lock_entry_name(&normalized, &safe_filename, &fallback_name),
            version_number: if latest_version_number.trim().is_empty() {
                format!("release-{latest_release_id}")
            } else {
                latest_version_number.clone()
            },
            filename: safe_filename,
            content_type: normalized.clone(),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            pinned_version: carried_pin,
            enabled: update.enabled,
            hashes: latest_hashes,
            provider_candidates: vec![ProviderCandidate {
                source: "github".to_string(),
                project_id: project_key,
                version_id: format!("gh_release:{latest_release_id}"),
                name: if update.name.trim().is_empty() {
                    format!("{owner}/{repo_name}")
                } else {
                    update.name.clone()
                },
                version_number: if latest_version_number.trim().is_empty() {
                    format!("release-{latest_release_id}")
                } else {
                    latest_version_number
                },
                confidence: None,
                reason: None,
            }],
            local_analysis: None,
        };
        if !new_entry.enabled {
            disable_mod_file(&instance_dir, &new_entry.filename)?;
        }
        lock.entries.push(new_entry.clone());
        lock.entries
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        write_lockfile(instances_dir, &args.instance_id, &lock)?;
        return Ok(Some(lock_entry_to_installed(&instance_dir, &new_entry)));
    }

    Ok(None)
}

fn content_updates_to_modrinth_result(content: ContentUpdateCheckResult) -> ModUpdateCheckResult {
    let mut updates: Vec<ModUpdateInfo> = content
        .updates
        .into_iter()
        .filter(|u| u.source == "modrinth" && u.content_type == "mods")
        .map(|u| ModUpdateInfo {
            project_id: u.project_id,
            name: u.name,
            current_version_id: u.current_version_id,
            current_version_number: u.current_version_number,
            latest_version_id: u.latest_version_id,
            latest_version_number: u.latest_version_number,
        })
        .collect();
    updates.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    ModUpdateCheckResult {
        checked_mods: content.checked_entries,
        update_count: updates.len(),
        updates,
    }
}

fn normalize_discover_content_type(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "mods" | "mod" => "mods".to_string(),
        "resourcepacks" | "resourcepack" | "texturepacks" | "texturepack" => {
            "resourcepacks".to_string()
        }
        "shaders" | "shaderpacks" | "shaderpack" | "shader" => "shaderpacks".to_string(),
        "datapacks" | "datapack" => "datapacks".to_string(),
        "modpacks" | "modpack" => "modpacks".to_string(),
        _ => "mods".to_string(),
    }
}

fn modrinth_project_type_facets(content_type: &str) -> Vec<String> {
    match content_type {
        "resourcepacks" => vec!["project_type:resourcepack".to_string()],
        "shaderpacks" => vec!["project_type:shader".to_string()],
        "datapacks" => vec!["project_type:datapack".to_string()],
        "modpacks" => vec!["project_type:modpack".to_string()],
        _ => vec!["project_type:mod".to_string()],
    }
}

fn curseforge_class_ids_for_content_type(content_type: &str) -> Vec<i64> {
    match content_type {
        "resourcepacks" => vec![12],
        // CurseForge does not have a first-class "shaderpacks" class in all metadata variants.
        // We use texture packs + query hints as best effort.
        "shaderpacks" => vec![12],
        "datapacks" => vec![6945],
        "modpacks" => vec![4471],
        _ => vec![6],
    }
}

fn discover_index_sort_field(index: &str) -> i64 {
    match index.trim().to_lowercase().as_str() {
        "downloads" => 6,
        "updated" => 3,
        "newest" => 11,
        "follows" => 2,
        _ => 1,
    }
}

fn github_request(client: &Client, url: &str, token: Option<&str>) -> Result<Response, String> {
    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", GITHUB_API_VERSION);
    if let Some(value) = token {
        req = req.bearer_auth(value);
    }
    req.send()
        .map_err(|e| format!("GitHub request failed: {e}"))
}

fn github_error_message_from_body(body: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;
    let message = value.get("message").and_then(|v| v.as_str())?.trim();
    if message.is_empty() {
        None
    } else {
        Some(message.to_string())
    }
}

fn github_rate_limit_reset_local(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let raw = headers.get("x-ratelimit-reset")?.to_str().ok()?;
    let epoch = raw.trim().parse::<i64>().ok()?;
    let dt_utc = DateTime::<Utc>::from_timestamp(epoch, 0)?;
    let dt_local = dt_utc.with_timezone(&Local);
    Some(dt_local.format("%Y-%m-%d %H:%M:%S %Z").to_string())
}

fn github_rate_limit_reset_instant(headers: &reqwest::header::HeaderMap) -> Option<Instant> {
    let raw = headers.get("x-ratelimit-reset")?.to_str().ok()?;
    let epoch = raw.trim().parse::<i64>().ok()?;
    let now_epoch = Utc::now().timestamp();
    let wait_secs = if epoch > now_epoch {
        (epoch - now_epoch) as u64
    } else {
        1
    };
    Some(Instant::now() + Duration::from_secs(wait_secs.saturating_add(1)))
}

fn github_is_rate_limited_from_headers(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
) -> bool {
    if status != reqwest::StatusCode::FORBIDDEN {
        return false;
    }
    headers
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim() == "0")
        .unwrap_or(false)
}

fn github_mark_unauth_cooldown(headers: &reqwest::header::HeaderMap) {
    let until = github_rate_limit_reset_instant(headers).unwrap_or_else(|| {
        Instant::now() + Duration::from_secs(GITHUB_TOKEN_RATE_LIMIT_FALLBACK_COOLDOWN_SECS)
    });
    if let Ok(mut guard) = github_token_rotation_state().lock() {
        guard.unauth_cooldown_until = Some(until);
        guard.unauth_reset_local = github_rate_limit_reset_local(headers);
    }
}

fn github_unauth_cooldown_state() -> (bool, Option<String>) {
    let now = Instant::now();
    if let Ok(mut guard) = github_token_rotation_state().lock() {
        if let Some(until) = guard.unauth_cooldown_until {
            if until > now {
                return (true, guard.unauth_reset_local.clone());
            }
            guard.unauth_cooldown_until = None;
            guard.unauth_reset_local = None;
        }
    }
    (false, None)
}

fn github_clear_unauth_cooldown() {
    if let Ok(mut guard) = github_token_rotation_state().lock() {
        guard.unauth_cooldown_until = None;
        guard.unauth_reset_local = None;
    }
}

fn github_unauth_rate_limit_message(
    configured_token_count: usize,
    reset_local: Option<String>,
) -> String {
    let reset = reset_local
        .map(|value| format!(" Resets around {value}."))
        .unwrap_or_default();
    let token_diag = if configured_token_count == 0 {
        " No GitHub tokens are configured.".to_string()
    } else {
        format!(" Detected {configured_token_count} configured GitHub token(s).")
    };
    format!(
        "GitHub API rate limit reached (403 Forbidden). Unauthenticated GitHub requests are temporarily paused.{}{} Configure GitHub API auth in Settings > Advanced > GitHub API, or via MPM_GITHUB_TOKENS / MPM_GITHUB_TOKEN / GITHUB_TOKEN / GH_TOKEN (including numbered variants like *_TOKEN_1).{}",
        token_diag,
        if configured_token_count > 0 {
            " Authenticated requests will keep rotating configured token(s)."
        } else {
            ""
        },
        reset
    )
}

fn github_mark_token_cooldown(
    token: &str,
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
) {
    let cooldown_until = if status == reqwest::StatusCode::UNAUTHORIZED {
        Some(Instant::now() + Duration::from_secs(GITHUB_TOKEN_UNAUTHORIZED_COOLDOWN_SECS))
    } else if github_is_rate_limited_from_headers(status, headers) {
        github_rate_limit_reset_instant(headers).or_else(|| {
            Some(
                Instant::now()
                    + Duration::from_secs(GITHUB_TOKEN_RATE_LIMIT_FALLBACK_COOLDOWN_SECS),
            )
        })
    } else {
        None
    };
    if let Some(until) = cooldown_until {
        if let Ok(mut guard) = github_token_rotation_state().lock() {
            guard.cooldown_until.insert(token.to_string(), until);
        }
    }
}

fn github_clear_token_cooldown(token: &str) {
    if let Ok(mut guard) = github_token_rotation_state().lock() {
        guard.cooldown_until.remove(token);
    }
}

fn github_tokens_in_request_order(all_tokens: &[String]) -> Vec<String> {
    if all_tokens.is_empty() {
        return vec![];
    }
    let now = Instant::now();
    let mut guard = match github_token_rotation_state().lock() {
        Ok(value) => value,
        Err(_) => return all_tokens.to_vec(),
    };
    guard.cooldown_until.retain(|_, until| *until > now);
    let len = all_tokens.len();
    let start = guard.next_start_index % len;
    guard.next_start_index = (start + 1) % len;

    let mut ordered = Vec::with_capacity(len);
    for offset in 0..len {
        ordered.push(all_tokens[(start + offset) % len].clone());
    }
    let available: Vec<String> = ordered
        .iter()
        .filter(|token| !guard.cooldown_until.contains_key((*token).as_str()))
        .cloned()
        .collect();
    if !available.is_empty() {
        return available;
    }
    // If every token is cooling down, probe one token so we recover quickly after reset.
    ordered.into_iter().take(1).collect()
}

fn github_http_error_message(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &str,
    token_attempts: usize,
    configured_token_count: usize,
    retried_without_token: bool,
) -> String {
    if status == reqwest::StatusCode::FORBIDDEN {
        let remaining = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .trim()
            .to_string();
        let message = github_error_message_from_body(body).unwrap_or_else(|| {
            "GitHub denied this request (likely rate-limited or blocked temporarily).".to_string()
        });
        if remaining == "0" || message.to_ascii_lowercase().contains("rate limit") {
            let reset = github_rate_limit_reset_local(headers)
                .map(|value| format!(" Resets around {value}."))
                .unwrap_or_default();
            let token_diag = if configured_token_count == 0 {
                " No GitHub tokens are configured.".to_string()
            } else {
                format!(" Detected {configured_token_count} configured GitHub token(s).")
            };
            return format!(
                "GitHub API rate limit reached (403 Forbidden).{}{} Configure GitHub API auth in Settings > Advanced > GitHub API, or use MPM_GITHUB_TOKENS / MPM_GITHUB_TOKEN / GITHUB_TOKEN / GH_TOKEN (including numbered variants like *_TOKEN_1). {}",
                if token_attempts > 1 {
                    " All configured GitHub tokens are rate-limited."
                } else if token_attempts == 1 {
                    " The configured token is still rate-limited."
                } else {
                    ""
                },
                token_diag,
                reset
            );
        }
        if token_attempts > 0 && !retried_without_token {
            return format!(
                "GitHub request failed with status 403 Forbidden using the configured token(s). {message}"
            );
        }
        return format!("GitHub request failed with status 403 Forbidden. {message}");
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return if token_attempts > 0 {
            "GitHub request failed with status 401 Unauthorized for the configured token(s). Check GitHub API auth in Settings > Advanced > GitHub API, or MPM_GITHUB_TOKENS / MPM_GITHUB_TOKEN / GITHUB_TOKEN / GH_TOKEN."
                .to_string()
        } else {
            "GitHub request failed with status 401 Unauthorized.".to_string()
        };
    }
    let message = github_error_message_from_body(body).unwrap_or_default();
    if message.is_empty() {
        format!("GitHub request failed with status {status}")
    } else {
        format!("GitHub request failed with status {status}: {message}")
    }
}

fn github_get_json<T: for<'de> Deserialize<'de>>(client: &Client, url: &str) -> Result<T, String> {
    if let Some(cached) = github_api_cache_get(url) {
        if let Ok(parsed) = serde_json::from_str::<T>(&cached) {
            return Ok(parsed);
        }
    }

    let all_tokens = github_api_tokens();
    if all_tokens.is_empty() {
        let (rate_limited, reset_local) = github_unauth_cooldown_state();
        if rate_limited {
            return Err(github_unauth_rate_limit_message(
                github_configured_token_count(),
                reset_local,
            ));
        }
    }
    let tokens = github_tokens_in_request_order(&all_tokens);
    let mut token_attempts = 0usize;
    let mut retried_without_token = false;
    let mut response = if tokens.is_empty() {
        let resp = github_request(client, url, None)?;
        if resp.status().is_success() {
            github_clear_unauth_cooldown();
        } else if github_is_rate_limited_from_headers(resp.status(), resp.headers()) {
            github_mark_unauth_cooldown(resp.headers());
        }
        resp
    } else {
        let mut selected: Option<Response> = None;
        for token in tokens.iter() {
            token_attempts += 1;
            let attempt = github_request(client, url, Some(token.as_str()))?;
            let status = attempt.status();
            if status.is_success() {
                github_clear_token_cooldown(token);
                selected = Some(attempt);
                break;
            }
            let should_try_next_token = status == reqwest::StatusCode::FORBIDDEN
                || status == reqwest::StatusCode::UNAUTHORIZED;
            if should_try_next_token {
                github_mark_token_cooldown(token, status, attempt.headers());
            }
            selected = Some(attempt);
            if !should_try_next_token {
                break;
            }
        }
        selected
            .ok_or_else(|| "GitHub request failed: no request attempts were executed".to_string())?
    };

    if !response.status().is_success()
        && token_attempts > 0
        && (response.status() == reqwest::StatusCode::FORBIDDEN
            || response.status() == reqwest::StatusCode::UNAUTHORIZED)
    {
        let (unauth_rate_limited, reset_local) = github_unauth_cooldown_state();
        if unauth_rate_limited {
            return Err(github_unauth_rate_limit_message(
                github_configured_token_count(),
                reset_local,
            ));
        }
        retried_without_token = true;
        response = github_request(client, url, None)?;
        if response.status().is_success() {
            github_clear_unauth_cooldown();
        } else if github_is_rate_limited_from_headers(response.status(), response.headers()) {
            github_mark_unauth_cooldown(response.headers());
        }
    }

    if !response.status().is_success() {
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.text().unwrap_or_default();
        return Err(github_http_error_message(
            status,
            &headers,
            &body,
            token_attempts,
            all_tokens.len(),
            retried_without_token,
        ));
    }

    let body = response
        .text()
        .map_err(|e| format!("read GitHub response body failed: {e}"))?;
    github_api_cache_put(url, body.clone());
    serde_json::from_str::<T>(&body).map_err(|e| format!("parse GitHub response failed: {e}"))
}

fn github_release_version_label(release: &GithubRelease) -> String {
    if !release.tag_name.trim().is_empty() {
        return release.tag_name.trim().to_string();
    }
    if let Some(name) = release.name.as_ref() {
        if !name.trim().is_empty() {
            return name.trim().to_string();
        }
    }
    format!("release-{}", release.id)
}

fn github_release_selection_matches_current(
    selection: &GithubReleaseSelection,
    current_version_id: &str,
    current_version_number: &str,
    current_hashes: &HashMap<String, String>,
) -> bool {
    let latest_version_id = format!("gh_release:{}", selection.release.id);
    if latest_version_id.eq_ignore_ascii_case(current_version_id.trim()) {
        return true;
    }

    let latest_version_label = github_release_version_label(&selection.release);
    if !latest_version_label.trim().is_empty()
        && !current_version_number.trim().is_empty()
        && latest_version_label
            .trim()
            .eq_ignore_ascii_case(current_version_number.trim())
    {
        return true;
    }

    let latest_hashes = extract_github_asset_digest(&selection.asset);
    for key in ["sha512", "sha256"] {
        let current = current_hashes
            .get(key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        let latest = latest_hashes
            .get(key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        if let (Some(current), Some(latest)) = (current, latest) {
            if current.eq_ignore_ascii_case(latest) {
                return true;
            }
        }
    }

    false
}

fn github_release_sort_key(release: &GithubRelease) -> i64 {
    if let Some(value) = release.published_at.as_ref() {
        let key = created_at_sort_key(value);
        if key > 0 {
            return key;
        }
    }
    if let Some(value) = release.created_at.as_ref() {
        let key = created_at_sort_key(value);
        if key > 0 {
            return key;
        }
    }
    0
}

fn github_release_asset_is_checksum_sidecar(asset_name: &str) -> bool {
    let lower = asset_name.trim().to_ascii_lowercase();
    lower.ends_with(".sha256")
        || lower.ends_with(".sha512")
        || lower.ends_with(".md5")
        || lower.ends_with(".sha1")
        || lower.contains("checksum")
}

fn github_release_asset_looks_like_mod_jar(asset_name: &str) -> bool {
    let lower = asset_name.trim().to_ascii_lowercase();
    if !lower.ends_with(".jar") {
        return false;
    }
    !(lower.contains("sources")
        || lower.contains("source")
        || lower.contains("javadoc")
        || lower.contains("deobf"))
}

fn bounded_levenshtein_distance(left: &str, right: &str, max_distance: usize) -> usize {
    if left == right {
        return 0;
    }
    if left.is_empty() {
        return right.chars().count().min(max_distance.saturating_add(1));
    }
    if right.is_empty() {
        return left.chars().count().min(max_distance.saturating_add(1));
    }

    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    let left_len = left_chars.len();
    let right_len = right_chars.len();
    let length_delta = left_len.abs_diff(right_len);
    if length_delta > max_distance {
        return max_distance.saturating_add(1);
    }

    let mut prev: Vec<usize> = (0..=right_len).collect();
    let mut curr: Vec<usize> = vec![0; right_len + 1];
    for (i, left_ch) in left_chars.iter().enumerate() {
        curr[0] = i + 1;
        let mut row_best = curr[0];
        for (j, right_ch) in right_chars.iter().enumerate() {
            let cost = if left_ch == right_ch { 0 } else { 1 };
            let deletion = prev[j + 1].saturating_add(1);
            let insertion = curr[j].saturating_add(1);
            let substitution = prev[j].saturating_add(cost);
            let value = deletion.min(insertion).min(substitution);
            curr[j + 1] = value;
            row_best = row_best.min(value);
        }
        if row_best > max_distance {
            return max_distance.saturating_add(1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[right_len]
}

fn normalized_discover_tokens(input: &str) -> Vec<String> {
    normalize_provider_match_key(input)
        .split_whitespace()
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| token.len() > 1)
        .collect()
}

fn github_name_similarity_score(name: &str, query: &str) -> i64 {
    let normalized_name = normalize_provider_match_key(name);
    let normalized_query = normalize_provider_match_key(query);
    if normalized_name.is_empty() || normalized_query.is_empty() {
        return 0;
    }
    if normalized_name.contains(&normalized_query) {
        return 72;
    }

    let query_terms = normalized_discover_tokens(&normalized_query);
    let name_terms = normalized_discover_tokens(&normalized_name);
    if query_terms.is_empty() || name_terms.is_empty() {
        return 0;
    }

    let mut score = 0_i64;
    for query_token in &query_terms {
        let mut best = 0_i64;
        for name_token in &name_terms {
            if name_token.contains(query_token) || query_token.contains(name_token) {
                let overlap = query_token.len().min(name_token.len()) as i64;
                best = best.max(16 + overlap.min(20));
                continue;
            }
            let max_edit = if query_token.len() <= 4 { 1 } else { 2 };
            let distance = bounded_levenshtein_distance(query_token, name_token, max_edit);
            if distance <= max_edit {
                let proximity = (max_edit.saturating_sub(distance)) as i64;
                let token_weight = query_token.len().min(12) as i64;
                best = best.max(6 + proximity * 6 + token_weight / 2);
            }
            if query_token.starts_with(name_token) || name_token.starts_with(query_token) {
                best = best.max(10);
            }
        }
        score += best;
    }
    if query_terms.len() > 1 {
        let joined = query_terms.join(" ");
        if normalized_name.contains(&joined) {
            score += 16;
        }
    }
    score.clamp(0, 100)
}

fn discover_hit_query_score(hit: &DiscoverSearchHit, query: &str) -> i64 {
    let q = query.trim();
    if q.is_empty() {
        return 0;
    }
    let categories_text = hit.categories.join(" ");
    github_name_similarity_score(&hit.title, q)
        .max(github_name_similarity_score(&hit.project_id, q))
        .max(github_name_similarity_score(
            hit.slug.as_deref().unwrap_or_default(),
            q,
        ))
        .max(github_name_similarity_score(&categories_text, q))
        .max(github_name_similarity_score(&hit.description, q) / 2)
        .max(github_name_similarity_score(&hit.author, q) / 2)
}

fn discover_query_variants(query: &str) -> Vec<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return vec![];
    }
    let tokens = normalized_discover_tokens(trimmed);
    if tokens.is_empty() {
        return vec![];
    }
    let normalized_query = normalize_provider_match_key(trimmed);
    let mut variants: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let push = |value: String, variants: &mut Vec<String>, seen: &mut HashSet<String>| {
        let normalized = normalize_provider_match_key(&value);
        if normalized.is_empty() || normalized == normalized_query {
            return;
        }
        if seen.insert(normalized.clone()) {
            variants.push(normalized);
        }
    };

    if tokens.len() >= 2 {
        let mut longest = tokens.clone();
        longest.sort_by(|a, b| b.len().cmp(&a.len()));
        for token in longest.into_iter().take(3) {
            push(token, &mut variants, &mut seen);
        }
        let joined = tokens.join(" ");
        push(joined, &mut variants, &mut seen);
        if tokens.len() >= 3 {
            push(
                format!("{} {}", tokens[0], tokens[1]),
                &mut variants,
                &mut seen,
            );
        }
    } else if let Some(token) = tokens.first() {
        if token.len() >= 8 {
            let prefix = token
                .chars()
                .take(token.len().saturating_sub(2))
                .collect::<String>();
            push(prefix, &mut variants, &mut seen);
        }
    }

    variants.truncate(4);
    variants
}

fn github_repo_policy_rejection_reason(repo: &GithubRepository) -> Option<&'static str> {
    if repo.archived {
        return Some("repository is archived");
    }
    if repo.fork {
        return Some("repository is a fork");
    }
    if repo.disabled {
        return Some("repository is disabled");
    }
    None
}

fn extract_github_asset_digest(asset: &GithubReleaseAsset) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Some(digest) = asset.digest.as_ref() {
        let trimmed = digest.trim();
        if let Some(value) = trimmed.strip_prefix("sha256:") {
            if !value.trim().is_empty() {
                out.insert("sha256".to_string(), value.trim().to_string());
            }
        } else if let Some(value) = trimmed.strip_prefix("sha512:") {
            if !value.trim().is_empty() {
                out.insert("sha512".to_string(), value.trim().to_string());
            }
        }
    }
    out
}

fn select_github_release_with_asset(
    repo: &GithubRepository,
    releases: &[GithubRelease],
    query: &str,
    required_game_version: Option<&str>,
    required_loader: Option<&str>,
    discover_filters: Option<&SearchDiscoverContentArgs>,
    repo_loader_hints: Option<&HashSet<String>>,
) -> Option<GithubReleaseSelection> {
    let repo_hint = if repo.name.trim().is_empty() {
        repo.full_name.trim().to_string()
    } else {
        repo.name.trim().to_string()
    };
    let query_hint = if query.trim().is_empty() {
        repo_hint.as_str()
    } else {
        query.trim()
    };

    let collect_candidates = |allow_prerelease: bool| -> Vec<(i64, i64, GithubReleaseSelection)> {
        let mut candidates: Vec<(i64, i64, GithubReleaseSelection)> = Vec::new();
        for release in releases {
            if release.draft || (release.prerelease && !allow_prerelease) {
                continue;
            }
            let mut checksum_present = false;
            for asset in &release.assets {
                if github_release_asset_is_checksum_sidecar(&asset.name) {
                    checksum_present = true;
                    continue;
                }
                if !github_release_asset_looks_like_mod_jar(&asset.name) {
                    continue;
                }
                if let Some(filters) = discover_filters {
                    if !github_release_asset_matches_discover_filters(
                        repo,
                        release,
                        asset,
                        filters,
                        repo_loader_hints,
                    ) {
                        continue;
                    }
                }
                if !github_release_asset_matches_install_requirements(
                    repo,
                    release,
                    asset,
                    required_game_version,
                    required_loader,
                    repo_loader_hints,
                ) {
                    continue;
                }
                let mut asset_score = github_name_similarity_score(&asset.name, query_hint)
                    + github_name_similarity_score(&asset.name, &repo_hint);
                if let Some(content_type) = asset.content_type.as_ref() {
                    if content_type.eq_ignore_ascii_case("application/java-archive") {
                        asset_score += 16;
                    }
                }
                if release.prerelease {
                    asset_score -= 5;
                }
                asset_score += ((asset.size / (1024 * 1024)).min(40)) as i64;
                let selection = GithubReleaseSelection {
                    release: release.clone(),
                    asset: asset.clone(),
                    has_checksum_sidecar: checksum_present,
                };
                candidates.push((github_release_sort_key(release), asset_score, selection));
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        candidates
    };

    let stable_candidates = collect_candidates(false);
    if let Some(best) = stable_candidates.into_iter().next() {
        return Some(best.2);
    }
    collect_candidates(true)
        .into_iter()
        .next()
        .map(|item| item.2)
}

fn github_discover_confidence_score(
    repo: &GithubRepository,
    selection: &GithubReleaseSelection,
    query: &str,
) -> i64 {
    let mut score = (repo.stargazers_count.min(15_000) / 120) as i64;
    if repo.owner.owner_type.eq_ignore_ascii_case("organization") {
        score += 24;
    }
    if selection.has_checksum_sidecar {
        score += 18;
    }
    score += github_name_similarity_score(&selection.asset.name, query);
    score += github_name_similarity_score(&repo.full_name, query) / 2;
    let age_days = Utc::now()
        .timestamp()
        .saturating_sub(github_release_sort_key(&selection.release));
    if age_days > 0 {
        let days = age_days / 86_400;
        if days <= 30 {
            score += 24;
        } else if days <= 120 {
            score += 12;
        } else if days > 720 {
            score -= 10;
        }
    }
    score.clamp(0, 100)
}

fn github_confidence_label(score: i64) -> String {
    if score >= 70 {
        "high".to_string()
    } else if score >= 45 {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

fn github_discover_reason(
    repo: &GithubRepository,
    selection: &GithubReleaseSelection,
    score: i64,
) -> String {
    let mut parts = Vec::new();
    parts.push(format!("{}★", repo.stargazers_count));
    if repo.owner.owner_type.eq_ignore_ascii_case("organization") {
        parts.push("organization-owned".to_string());
    }
    if selection.has_checksum_sidecar {
        parts.push("checksum sidecar detected".to_string());
    }
    if !selection.release.tag_name.trim().is_empty() {
        parts.push(format!("tag {}", selection.release.tag_name.trim()));
    }
    parts.push(format!("confidence {}", github_confidence_label(score)));
    parts.join(" · ")
}

fn fetch_github_repo(client: &Client, owner: &str, repo: &str) -> Result<GithubRepository, String> {
    let url = format!("{}/repos/{}/{}", GITHUB_API_BASE, owner, repo);
    github_get_json::<GithubRepository>(client, &url)
}

fn fetch_github_releases(
    client: &Client,
    owner: &str,
    repo: &str,
) -> Result<Vec<GithubRelease>, String> {
    let url = format!(
        "{}/repos/{}/{}/releases?per_page={}",
        GITHUB_API_BASE, owner, repo, GITHUB_RELEASES_PER_PAGE
    );
    github_get_json::<Vec<GithubRelease>>(client, &url)
}

fn fetch_github_readme(
    client: &Client,
    owner: &str,
    repo: &str,
) -> Result<GithubReadmeResponse, String> {
    let url = format!("{}/repos/{}/{}/readme", GITHUB_API_BASE, owner, repo);
    github_get_json::<GithubReadmeResponse>(client, &url)
}

fn decode_github_readme_markdown(payload: &GithubReadmeResponse) -> Option<String> {
    let encoding = payload.encoding.trim().to_ascii_lowercase();
    if encoding != "base64" {
        let text = payload.content.trim();
        if text.is_empty() {
            return None;
        }
        return Some(text.to_string());
    }
    let compact = payload
        .content
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return None;
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(compact.as_bytes())
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn github_repo_project_id(repo: &GithubRepository) -> String {
    if !repo.full_name.trim().is_empty() {
        format!("gh:{}", repo.full_name.trim())
    } else {
        github_project_key(&repo.owner.login, &repo.name)
    }
}

fn github_repo_title(repo: &GithubRepository) -> String {
    if repo.name.trim().is_empty() {
        repo.full_name.clone()
    } else {
        repo.name.clone()
    }
}

fn github_repo_external_url(repo: &GithubRepository) -> String {
    if !repo.html_url.trim().is_empty() {
        repo.html_url.clone()
    } else {
        format!(
            "https://github.com/{}/{}",
            repo.owner.login.trim(),
            repo.name.trim()
        )
    }
}

fn github_owner_avatar_url(repo: &GithubRepository) -> Option<String> {
    let owner = repo.owner.login.trim();
    if owner.is_empty() {
        None
    } else {
        Some(format!("https://github.com/{owner}.png?size=96"))
    }
}

fn github_repo_identity(repo: &GithubRepository) -> Option<(String, String)> {
    if !repo.full_name.trim().is_empty() {
        if let Ok((owner, repo_name)) = parse_github_project_id(&repo.full_name) {
            return Some((owner, repo_name));
        }
    }
    let owner = repo.owner.login.trim();
    let repo_name = repo.name.trim();
    if owner.is_empty() || repo_name.is_empty() {
        None
    } else {
        Some((owner.to_string(), repo_name.to_string()))
    }
}

fn fetch_github_repo_tree_paths(
    client: &Client,
    owner: &str,
    repo: &str,
    reference: &str,
) -> Result<Vec<String>, String> {
    let encoded_reference =
        url::form_urlencoded::byte_serialize(reference.trim().as_bytes()).collect::<String>();
    let url = format!(
        "{}/repos/{}/{}/git/trees/{}?recursive=1",
        GITHUB_API_BASE, owner, repo, encoded_reference
    );
    let payload = github_get_json::<GithubTreeResponse>(client, &url)?;
    let mut out = Vec::new();
    for node in payload.tree {
        if node.node_type != "blob" {
            continue;
        }
        let normalized = node.path.trim();
        if normalized.is_empty() {
            continue;
        }
        out.push(normalized.to_ascii_lowercase());
        if out.len() >= GITHUB_REPO_TREE_PATH_SCAN_LIMIT {
            break;
        }
    }
    Ok(out)
}

fn github_loader_hints_from_repo_tree_paths(paths: &[String]) -> HashSet<String> {
    let mut out = HashSet::new();
    for path in paths {
        if path.ends_with("fabric.mod.json")
            || path.contains("/fabric.mod.json")
            || path.contains("/src/fabric/")
            || path.starts_with("fabric/src/")
        {
            out.insert("fabric".to_string());
        }
        if path.ends_with("quilt.mod.json")
            || path.contains("/quilt.mod.json")
            || path.contains("/src/quilt/")
            || path.starts_with("quilt/src/")
        {
            out.insert("quilt".to_string());
        }
        if path.ends_with("meta-inf/neoforge.mods.toml")
            || path.ends_with("neoforge.mods.toml")
            || path.contains("/src/neoforge/")
            || path.starts_with("neoforge/src/")
            || path.contains("/neo-forge/")
        {
            out.insert("neoforge".to_string());
        }
        if path.ends_with("meta-inf/mods.toml")
            || path.ends_with("mcmod.info")
            || path.contains("/src/forge/")
            || path.starts_with("forge/src/")
        {
            out.insert("forge_family".to_string());
        }
    }
    out
}

fn fetch_github_repo_loader_hints(client: &Client, repo: &GithubRepository) -> HashSet<String> {
    let Some((owner, repo_name)) = github_repo_identity(repo) else {
        return HashSet::new();
    };
    let mut refs: Vec<String> = vec![];
    let mut seen_refs: HashSet<String> = HashSet::new();
    for candidate in [repo.default_branch.trim(), "HEAD", "main", "master"] {
        if candidate.is_empty() {
            continue;
        }
        let key = candidate.to_ascii_lowercase();
        if !seen_refs.insert(key) {
            continue;
        }
        refs.push(candidate.to_string());
    }
    for reference in refs {
        match fetch_github_repo_tree_paths(client, &owner, &repo_name, &reference) {
            Ok(paths) => {
                let hints = github_loader_hints_from_repo_tree_paths(&paths);
                if !hints.is_empty() {
                    return hints;
                }
            }
            Err(_) => continue,
        }
    }
    HashSet::new()
}

fn github_loader_hints_to_labels(hints: &HashSet<String>) -> Vec<String> {
    let mut labels = Vec::new();
    if hints.contains("fabric") {
        labels.push("loader:fabric".to_string());
    }
    if hints.contains("quilt") {
        labels.push("loader:quilt".to_string());
    }
    if hints.contains("neoforge") {
        labels.push("loader:neoforge".to_string());
    }
    if hints.contains("forge") || hints.contains("forge_family") {
        labels.push("loader:forge".to_string());
    }
    labels
}

fn normalize_provider_match_key(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fetch_modrinth_icon_hints_for_query(client: &Client, query: &str) -> HashMap<String, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return HashMap::new();
    }
    let facets_json = serde_json::to_string(&vec![vec!["project_type:mod"]]).unwrap_or_default();
    let encoded_query =
        url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect::<String>();
    let encoded_facets =
        url::form_urlencoded::byte_serialize(facets_json.as_bytes()).collect::<String>();
    let url = format!(
        "{}/search?query={encoded_query}&index=relevance&limit=20&offset=0&facets={encoded_facets}",
        modrinth_api_base()
    );
    let resp = match client.get(url).header("Accept", "application/json").send() {
        Ok(value) => value,
        Err(_) => return HashMap::new(),
    };
    if !resp.status().is_success() {
        return HashMap::new();
    }
    let payload = match resp.json::<serde_json::Value>() {
        Ok(value) => value,
        Err(_) => return HashMap::new(),
    };
    let mut out = HashMap::new();
    let Some(items) = payload.get("hits").and_then(|v| v.as_array()) else {
        return out;
    };
    for item in items {
        let icon = item
            .get("icon_url")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let Some(icon_url) = icon else {
            continue;
        };
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .map(normalize_provider_match_key);
        if let Some(key) = title.filter(|v| !v.is_empty()) {
            out.entry(key).or_insert_with(|| icon_url.clone());
        }
        let slug = item
            .get("slug")
            .and_then(|v| v.as_str())
            .map(normalize_provider_match_key);
        if let Some(key) = slug.filter(|v| !v.is_empty()) {
            out.entry(key).or_insert_with(|| icon_url.clone());
        }
    }
    out
}

fn github_best_discover_icon_url(
    repo: &GithubRepository,
    modrinth_icon_hints: &HashMap<String, String>,
) -> Option<String> {
    let mut candidates = vec![
        normalize_provider_match_key(&repo.name),
        normalize_provider_match_key(repo.full_name.split('/').last().unwrap_or_default().trim()),
        normalize_provider_match_key(&github_repo_title(repo)),
    ];
    candidates.retain(|value| !value.is_empty());
    candidates.sort();
    candidates.dedup();
    for key in candidates {
        if let Some(icon) = modrinth_icon_hints.get(&key) {
            return Some(icon.clone());
        }
    }
    github_owner_avatar_url(repo)
}

fn github_loader_hints_from_text(text: &str) -> HashSet<String> {
    let mut out = detect_mod_loader_hints_from_filename(text);
    for token in text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')) {
        if let Some(loader) = normalize_loader_hint_token(token) {
            out.insert(loader);
        }
    }
    out
}

fn github_repo_minecraft_signal_score(repo: &GithubRepository) -> i64 {
    let text = normalize_provider_match_key(&format!(
        "{} {} {} {}",
        repo.name,
        repo.full_name,
        repo.description.clone().unwrap_or_default(),
        repo.topics.join(" ")
    ));
    let topics = repo
        .topics
        .iter()
        .map(|value| normalize_provider_match_key(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let mut score = 0_i64;
    if topics
        .iter()
        .any(|topic| topic == "minecraft" || topic.contains("minecraft"))
    {
        score += 3;
    }
    if topics.iter().any(|topic| {
        topic.contains("fabric")
            || topic.contains("forge")
            || topic.contains("neoforge")
            || topic.contains("neo forge")
            || topic.contains("quilt")
    }) {
        score += 2;
    }
    if topics
        .iter()
        .any(|topic| topic == "mod" || topic == "mods" || topic.contains("minecraft mod"))
    {
        score += 2;
    }
    if text.contains("minecraft") {
        score += 2;
    }
    for token in ["fabric", "forge", "neoforge", "neo forge", "quilt"] {
        if text.contains(token) {
            score += 1;
        }
    }
    for token in [
        "plugin",
        "bukkit",
        "spigot",
        "paper",
        "velocity",
        "bungeecord",
    ] {
        if text.contains(token) {
            score -= 2;
        }
    }
    score
}

fn github_repo_mod_ecosystem_signal_score(repo: &GithubRepository) -> i64 {
    let text = normalize_provider_match_key(&format!(
        "{} {} {} {}",
        repo.name,
        repo.full_name,
        repo.description.clone().unwrap_or_default(),
        repo.topics.join(" ")
    ));
    let token_set = normalized_discover_tokens(&text)
        .into_iter()
        .collect::<HashSet<String>>();
    let has_token = |token: &str| token_set.contains(token);
    let has_pair = |left: &str, right: &str| has_token(left) && has_token(right);

    let mut score = 0_i64;
    for token in [
        "mod", "mods", "addon", "addons", "meteor", "fabric", "forge", "neoforge", "quilt",
    ] {
        if has_token(token) {
            score += 1;
        }
    }
    if has_pair("minecraft", "client")
        || has_pair("minecraft", "mod")
        || has_pair("minecraft", "mods")
    {
        score += 2;
    }
    if has_pair("add", "on") {
        score += 1;
    }
    for token in [
        "dataset",
        "tensor",
        "tensorflow",
        "pytorch",
        "model",
        "gan",
        "llm",
        "image generation",
        "fashion",
        "mnist",
    ] {
        if has_token(token) {
            score -= 2;
        }
    }
    score
}

fn github_loader_filter_matches(
    requested_loaders: &[String],
    detected_loaders: &HashSet<String>,
    allow_when_unknown: bool,
) -> bool {
    if requested_loaders.is_empty() {
        return true;
    }
    if detected_loaders.is_empty() {
        return allow_when_unknown;
    }
    requested_loaders.iter().any(|requested| {
        let normalized = parse_loader_for_instance(requested)
            .or_else(|| normalize_loader_hint_token(requested))
            .unwrap_or_else(|| requested.trim().to_ascii_lowercase());
        detected_loaders
            .iter()
            .any(|detected| instance_loader_accepts_mod_loader(&normalized, detected))
    })
}

fn github_game_version_filter_matches(required_game_version: Option<&str>, text: &str) -> bool {
    let Some(required) = required_game_version.map(|value| value.trim().to_ascii_lowercase())
    else {
        return true;
    };
    if required.is_empty() {
        return true;
    }
    for token in text.split(|ch: char| !(ch.is_ascii_digit() || ch == '.')) {
        let normalized = token.trim_matches('.');
        if normalized.len() < 3 || !normalized.starts_with('1') {
            continue;
        }
        if parse_mc_version_parts_loose(normalized).is_none() {
            continue;
        }
        if minecraft_version_matches_advertised(normalized, &required, true) {
            return true;
        }
    }
    false
}

fn github_category_filter_matches(
    requested_categories: &[String],
    repo: &GithubRepository,
    text: &str,
) -> bool {
    if requested_categories.is_empty() {
        return true;
    }
    let topics = repo
        .topics
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let normalized_text = normalize_provider_match_key(text);
    for category in requested_categories {
        let normalized = normalize_provider_match_key(category);
        if normalized.is_empty() {
            continue;
        }
        if topics
            .iter()
            .any(|topic| topic == &normalized || topic.contains(&normalized))
        {
            return true;
        }
        if normalized_text.contains(&normalized) {
            return true;
        }
    }
    false
}

fn github_repo_matches_discover_light_filters(
    repo: &GithubRepository,
    args: &SearchDiscoverContentArgs,
    repo_loader_hints: Option<&HashSet<String>>,
) -> bool {
    let repo_text = format!(
        "{} {} {} {}",
        repo.name,
        repo.full_name,
        repo.description.clone().unwrap_or_default(),
        repo.topics.join(" ")
    );
    let mut loader_hints = github_loader_hints_from_text(&repo_text);
    if let Some(extra) = repo_loader_hints {
        loader_hints.extend(extra.iter().cloned());
    }
    github_loader_filter_matches(&args.loaders, &loader_hints, true)
        && github_category_filter_matches(&args.categories, repo, &repo_text)
}

fn github_repo_passes_signal_gate(
    repo: &GithubRepository,
    minecraft_signal: i64,
    similarity: i64,
    query: &str,
) -> bool {
    if minecraft_signal > 0 {
        return true;
    }
    !query.trim().is_empty()
        && similarity >= GITHUB_DISCOVER_MIN_SIMILARITY_WITHOUT_SIGNAL
        && github_repo_mod_ecosystem_signal_score(repo) > 0
}

fn github_repo_can_skip_release_validation(
    repo: &GithubRepository,
    minecraft_signal: i64,
    similarity: i64,
) -> bool {
    minecraft_signal > 0
        || (similarity >= GITHUB_LOW_SIGNAL_HIGH_SIMILARITY_THRESHOLD
            && github_repo_mod_ecosystem_signal_score(repo) > 0)
}

fn github_release_asset_matches_discover_filters(
    repo: &GithubRepository,
    release: &GithubRelease,
    asset: &GithubReleaseAsset,
    args: &SearchDiscoverContentArgs,
    repo_loader_hints: Option<&HashSet<String>>,
) -> bool {
    let combined_text = format!(
        "{} {} {} {} {} {} {}",
        repo.name,
        repo.full_name,
        repo.description.clone().unwrap_or_default(),
        repo.topics.join(" "),
        release.tag_name,
        release.name.clone().unwrap_or_default(),
        asset.name
    );
    let mut loader_hints = github_loader_hints_from_text(&combined_text);
    if let Some(extra) = repo_loader_hints {
        loader_hints.extend(extra.iter().cloned());
    }
    github_loader_filter_matches(&args.loaders, &loader_hints, false)
        && github_game_version_filter_matches(args.game_version.as_deref(), &combined_text)
        && github_category_filter_matches(&args.categories, repo, &combined_text)
}

fn github_release_asset_matches_install_requirements(
    repo: &GithubRepository,
    release: &GithubRelease,
    asset: &GithubReleaseAsset,
    required_game_version: Option<&str>,
    required_loader: Option<&str>,
    repo_loader_hints: Option<&HashSet<String>>,
) -> bool {
    let combined_text = format!(
        "{} {} {} {} {} {} {}",
        repo.name,
        repo.full_name,
        repo.description.clone().unwrap_or_default(),
        repo.topics.join(" "),
        release.tag_name,
        release.name.clone().unwrap_or_default(),
        asset.name
    );
    if !github_game_version_filter_matches(required_game_version, &combined_text) {
        return false;
    }
    let Some(loader) = required_loader
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return true;
    };
    let mut loader_hints = github_loader_hints_from_text(&combined_text);
    if let Some(extra) = repo_loader_hints {
        loader_hints.extend(extra.iter().cloned());
    }
    if loader_hints.is_empty() {
        return true;
    }
    loader_hints
        .iter()
        .any(|detected| instance_loader_accepts_mod_loader(loader, detected))
}

fn github_repo_query_similarity(repo: &GithubRepository, query: &str) -> i64 {
    let query = query.trim();
    if query.is_empty() {
        return 0;
    }
    let repo_name = github_repo_title(repo);
    github_name_similarity_score(&repo_name, query)
        .max(github_name_similarity_score(&repo.full_name, query))
        .max(
            github_name_similarity_score(repo.description.as_deref().unwrap_or_default(), query)
                / 2,
        )
}

fn parse_github_repo_query_candidate(query: &str) -> Option<(String, String)> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    parse_github_project_id(trimmed).ok()
}

fn search_github_repositories_once(
    client: &Client,
    query: &str,
    per_page: usize,
    page: usize,
) -> Result<Vec<GithubRepository>, String> {
    let raw_query = query.trim();
    let query_with_qualifiers = if raw_query.is_empty() {
        "minecraft mod archived:false fork:false".to_string()
    } else {
        format!("{raw_query} archived:false fork:false")
    };
    let encoded_query =
        url::form_urlencoded::byte_serialize(query_with_qualifiers.as_bytes()).collect::<String>();
    let url = format!(
        "{}/search/repositories?q={encoded_query}&per_page={}&page={}",
        GITHUB_API_BASE,
        per_page.clamp(1, GITHUB_REPO_SEARCH_PER_PAGE_MAX),
        page.max(1)
    );
    let payload = github_get_json::<GithubRepoSearchResponse>(client, &url)?;
    Ok(payload.items)
}

fn search_github_repositories(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<GithubRepository>, String> {
    let has_configured_auth_tokens = github_has_configured_tokens();
    let effective_limit = limit.max(1);
    let collection_limit = effective_limit
        .saturating_mul(2)
        .min(GITHUB_DISCOVER_SOURCE_MAX_REPO_CANDIDATES.saturating_mul(2))
        .max(effective_limit);
    let mut out: Vec<GithubRepository> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut per_page = effective_limit
        .clamp(1, GITHUB_REPO_SEARCH_PER_PAGE_MAX)
        .max(10);
    if !has_configured_auth_tokens {
        per_page = per_page.min(20);
    }
    let max_pages_cap = if has_configured_auth_tokens {
        GITHUB_REPO_SEARCH_MAX_PAGES_PER_QUERY
    } else {
        GITHUB_UNAUTH_MAX_PAGES_PER_QUERY
    };
    let max_pages = ((collection_limit + per_page - 1) / per_page).clamp(1, max_pages_cap);
    let trimmed_query = query.trim();
    let search_queries = github_discover_search_queries(trimmed_query, has_configured_auth_tokens);

    for (query_index, search_query) in search_queries.into_iter().enumerate() {
        let page_limit = if query_index == 0 { max_pages } else { 1 };
        for page in 1..=page_limit {
            let repos = search_github_repositories_once(client, &search_query, per_page, page)?;
            if repos.is_empty() {
                break;
            }
            let fetched_count = repos.len();
            for repo in repos {
                let key = repo.full_name.trim().to_ascii_lowercase();
                if key.is_empty() || !seen.insert(key) {
                    continue;
                }
                out.push(repo);
                if out.len() >= collection_limit {
                    break;
                }
            }
            if out.len() >= collection_limit {
                break;
            }
            if fetched_count < per_page {
                break;
            }
        }
        if out.len() >= collection_limit {
            break;
        }
    }

    if !trimmed_query.is_empty() {
        out.sort_by(|a, b| {
            let left = github_repo_query_similarity(a, trimmed_query);
            let right = github_repo_query_similarity(b, trimmed_query);
            right
                .cmp(&left)
                .then_with(|| b.stargazers_count.cmp(&a.stargazers_count))
                .then_with(|| b.forks_count.cmp(&a.forks_count))
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        });
    }
    out.truncate(effective_limit);
    Ok(out)
}

fn github_discover_search_queries(
    trimmed_query: &str,
    has_configured_auth_tokens: bool,
) -> Vec<String> {
    if trimmed_query.trim().is_empty() {
        return vec!["minecraft mod language:java".to_string()];
    }
    let mut variants: Vec<String> = Vec::new();
    let mut seen_variants: HashSet<String> = HashSet::new();
    for candidate in std::iter::once(trimmed_query.trim().to_string())
        .chain(discover_query_variants(trimmed_query).into_iter().take(3))
    {
        let normalized = normalize_provider_match_key(&candidate);
        if normalized.is_empty() || !seen_variants.insert(normalized) {
            continue;
        }
        variants.push(candidate.trim().to_string());
    }
    if variants.is_empty() {
        return vec![];
    }
    let primary = variants[0].clone();
    let fallback = variants
        .iter()
        .skip(1)
        .find(|value| !value.trim().is_empty())
        .cloned();
    let mut queries: Vec<String> = Vec::new();
    let mut seen_queries: HashSet<String> = HashSet::new();
    let push = |value: String, queries: &mut Vec<String>, seen_queries: &mut HashSet<String>| {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() || !seen_queries.insert(normalized) {
            return;
        }
        queries.push(value);
    };

    push(
        format!("{primary} in:name,description"),
        &mut queries,
        &mut seen_queries,
    );
    push(
        format!("{primary} minecraft mod in:name,description"),
        &mut queries,
        &mut seen_queries,
    );
    if let Some(fallback_query) = fallback {
        push(
            format!("{fallback_query} in:name,description"),
            &mut queries,
            &mut seen_queries,
        );
        push(
            format!("{fallback_query} minecraft mod in:name,description"),
            &mut queries,
            &mut seen_queries,
        );
    }
    push(
        format!("{primary} topic:minecraft"),
        &mut queries,
        &mut seen_queries,
    );
    push(
        format!("{primary} minecraft fabric forge neoforge quilt"),
        &mut queries,
        &mut seen_queries,
    );

    queries.truncate(if has_configured_auth_tokens {
        8
    } else {
        GITHUB_UNAUTH_MAX_SEARCH_QUERIES
    });
    queries
}

fn github_release_to_discover_hit(
    repo: &GithubRepository,
    selection: &GithubReleaseSelection,
    query: &str,
    icon_url: Option<String>,
    detected_loader_hints: Option<&HashSet<String>>,
) -> DiscoverSearchHit {
    let confidence_score = github_discover_confidence_score(repo, selection, query);
    let project_id = github_repo_project_id(repo);
    let title = github_repo_title(repo);
    let description = repo
        .description
        .clone()
        .unwrap_or_else(|| "GitHub release suggestion".to_string());
    DiscoverSearchHit {
        source: "github".to_string(),
        project_id,
        title,
        description,
        author: repo.owner.login.clone(),
        downloads: repo.stargazers_count,
        follows: repo.forks_count,
        icon_url,
        categories: {
            let mut tags = repo.topics.clone();
            if let Some(hints) = detected_loader_hints {
                tags.extend(github_loader_hints_to_labels(hints));
            }
            tags.retain(|topic| !topic.trim().is_empty());
            tags.sort();
            tags.dedup();
            tags
        },
        versions: vec![github_release_version_label(&selection.release)],
        date_modified: selection
            .release
            .published_at
            .clone()
            .or_else(|| selection.release.created_at.clone())
            .or_else(|| repo.pushed_at.clone())
            .or_else(|| repo.updated_at.clone())
            .unwrap_or_default(),
        content_type: "mods".to_string(),
        slug: Some(repo.name.clone()),
        external_url: Some(github_repo_external_url(repo)),
        confidence: Some(github_confidence_label(confidence_score)),
        reason: Some(github_discover_reason(repo, selection, confidence_score)),
        install_supported: Some(true),
        install_note: None,
    }
}

fn github_repo_without_release_hit(
    repo: &GithubRepository,
    query: &str,
    install_note: &str,
    icon_url: Option<String>,
    detected_loader_hints: Option<&HashSet<String>>,
) -> DiscoverSearchHit {
    let similarity = github_repo_query_similarity(repo, query);
    let confidence_score =
        ((repo.stargazers_count.min(4000) / 80) as i64 + similarity).clamp(0, 100);
    DiscoverSearchHit {
        source: "github".to_string(),
        project_id: github_repo_project_id(repo),
        title: github_repo_title(repo),
        description: repo
            .description
            .clone()
            .unwrap_or_else(|| "GitHub repository".to_string()),
        author: repo.owner.login.clone(),
        downloads: repo.stargazers_count,
        follows: repo.forks_count,
        icon_url,
        categories: {
            let mut tags = repo.topics.clone();
            if let Some(hints) = detected_loader_hints {
                tags.extend(github_loader_hints_to_labels(hints));
            }
            tags.retain(|topic| !topic.trim().is_empty());
            tags.sort();
            tags.dedup();
            tags
        },
        versions: vec![],
        date_modified: repo
            .pushed_at
            .clone()
            .or_else(|| repo.updated_at.clone())
            .unwrap_or_default(),
        content_type: "mods".to_string(),
        slug: Some(repo.name.clone()),
        external_url: Some(github_repo_external_url(repo)),
        confidence: Some(github_confidence_label(confidence_score)),
        reason: Some(format!(
            "{}★ · repository match · {}",
            repo.stargazers_count,
            github_confidence_label(confidence_score)
        )),
        install_supported: Some(false),
        install_note: Some(install_note.to_string()),
    }
}

fn github_repo_deferred_release_hit(
    repo: &GithubRepository,
    query: &str,
    install_note: &str,
    icon_url: Option<String>,
    detected_loader_hints: Option<&HashSet<String>>,
) -> DiscoverSearchHit {
    let mut hit =
        github_repo_without_release_hit(repo, query, install_note, icon_url, detected_loader_hints);
    hit.install_supported = Some(true);
    hit
}

fn search_github_discover(
    client: &Client,
    args: &SearchDiscoverContentArgs,
) -> Result<DiscoverSearchResult, String> {
    if normalize_discover_content_type(&args.content_type) != "mods" {
        return Ok(DiscoverSearchResult {
            hits: vec![],
            offset: args.offset,
            limit: args.limit,
            total_hits: 0,
        });
    }
    let query = args.query.trim();
    let required_hits = args
        .offset
        .saturating_add(args.limit)
        .saturating_add(GITHUB_DISCOVER_RESULTS_BUFFER);
    let desired_pool = required_hits
        .max(args.limit.saturating_mul(3))
        .max(GITHUB_DISCOVER_MIN_RESULT_POOL)
        .min(GITHUB_DISCOVER_SOURCE_MAX_REPO_CANDIDATES);
    let requested_limit = desired_pool;
    let mut repos = search_github_repositories(client, query, requested_limit)?;
    let modrinth_icon_hints = fetch_modrinth_icon_hints_for_query(client, query);
    if let Some((owner, repo_name)) = parse_github_repo_query_candidate(query) {
        if let Ok(direct_repo) = fetch_github_repo(client, &owner, &repo_name) {
            let key = direct_repo.full_name.trim().to_ascii_lowercase();
            let already_present = repos
                .iter()
                .any(|repo| repo.full_name.trim().eq_ignore_ascii_case(&key));
            if !already_present {
                repos.insert(0, direct_repo);
            }
        }
    }

    let mut hits: Vec<DiscoverSearchHit> = Vec::new();
    let mut seen_hits: HashSet<String> = HashSet::new();
    let has_configured_auth_tokens = github_has_configured_tokens();
    let strict_asset_filters = !args.loaders.is_empty()
        || args
            .game_version
            .as_ref()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
    let mut release_fetch_budget = if strict_asset_filters {
        GITHUB_DISCOVER_MAX_RELEASE_FETCHES
    } else {
        GITHUB_DISCOVER_NONSTRICT_RELEASE_FETCHES
    };
    if !has_configured_auth_tokens {
        release_fetch_budget = if strict_asset_filters {
            GITHUB_UNAUTH_STRICT_RELEASE_FETCH_BUDGET
        } else {
            GITHUB_UNAUTH_NONSTRICT_RELEASE_FETCH_BUDGET
        };
    }
    let mut release_fetches = 0usize;
    for repo in repos {
        if github_repo_policy_rejection_reason(&repo).is_some() {
            continue;
        }
        let repo_tree_loader_hints = if args.loaders.is_empty() {
            HashSet::new()
        } else {
            fetch_github_repo_loader_hints(client, &repo)
        };
        let repo_tree_loader_hints_opt = if repo_tree_loader_hints.is_empty() {
            None
        } else {
            Some(&repo_tree_loader_hints)
        };
        if !github_repo_matches_discover_light_filters(&repo, args, repo_tree_loader_hints_opt) {
            continue;
        }
        let similarity = github_repo_query_similarity(&repo, query);
        let minecraft_signal = github_repo_minecraft_signal_score(&repo);
        if !github_repo_passes_signal_gate(&repo, minecraft_signal, similarity, query) {
            continue;
        }
        if query.is_empty() && repo.stargazers_count < 250 {
            continue;
        }
        if !query.is_empty() && similarity < 8 && repo.stargazers_count < 120 {
            continue;
        }
        let (owner, repo_name) = if !repo.full_name.trim().is_empty() {
            parse_github_project_id(&repo.full_name)?
        } else {
            (repo.owner.login.clone(), repo.name.clone())
        };
        let icon_url = github_best_discover_icon_url(&repo, &modrinth_icon_hints);
        let requires_release_validation = minecraft_signal <= 0
            && !github_repo_can_skip_release_validation(&repo, minecraft_signal, similarity);
        if requires_release_validation && release_fetches >= release_fetch_budget {
            continue;
        }
        if !strict_asset_filters && release_fetches >= release_fetch_budget {
            let hit = github_repo_deferred_release_hit(
                &repo,
                query,
                "Fast mode: release metadata is checked when you open/install this result.",
                icon_url,
                repo_tree_loader_hints_opt,
            );
            let dedupe_key = hit.project_id.trim().to_ascii_lowercase();
            if seen_hits.insert(dedupe_key) {
                hits.push(hit);
            }
            if hits.len() >= desired_pool {
                break;
            }
            continue;
        }
        if strict_asset_filters && release_fetches >= release_fetch_budget {
            break;
        }
        release_fetches = release_fetches.saturating_add(1);
        let releases = match fetch_github_releases(client, &owner, &repo_name) {
            Ok(value) => value,
            Err(_) => {
                if strict_asset_filters || requires_release_validation {
                    continue;
                }
                let hit = github_repo_without_release_hit(
                    &repo,
                    query,
                    "Repository found, but GitHub release metadata is unavailable right now.",
                    icon_url,
                    repo_tree_loader_hints_opt,
                );
                let dedupe_key = hit.project_id.trim().to_ascii_lowercase();
                if seen_hits.insert(dedupe_key) {
                    hits.push(hit);
                }
                continue;
            }
        };
        if let Some(selection) = select_github_release_with_asset(
            &repo,
            &releases,
            query,
            None,
            None,
            Some(args),
            repo_tree_loader_hints_opt,
        ) {
            let combined_text = format!(
                "{} {} {} {} {} {} {}",
                repo.name,
                repo.full_name,
                repo.description.clone().unwrap_or_default(),
                repo.topics.join(" "),
                selection.release.tag_name,
                selection.release.name.clone().unwrap_or_default(),
                selection.asset.name
            );
            let mut detected_loader_hints = github_loader_hints_from_text(&combined_text);
            if let Some(extra) = repo_tree_loader_hints_opt {
                detected_loader_hints.extend(extra.iter().cloned());
            }
            let loader_hints_opt = if detected_loader_hints.is_empty() {
                None
            } else {
                Some(&detected_loader_hints)
            };
            let hit = github_release_to_discover_hit(
                &repo,
                &selection,
                query,
                icon_url,
                loader_hints_opt,
            );
            let dedupe_key = hit.project_id.trim().to_ascii_lowercase();
            if seen_hits.insert(dedupe_key) {
                hits.push(hit);
            }
            if hits.len() >= desired_pool {
                break;
            }
            continue;
        }

        if strict_asset_filters || requires_release_validation {
            continue;
        }
        let hit = github_repo_deferred_release_hit(
            &repo,
            query,
            "Repository match found. Release compatibility is checked when you open/install.",
            icon_url,
            repo_tree_loader_hints_opt,
        );
        let dedupe_key = hit.project_id.trim().to_ascii_lowercase();
        if seen_hits.insert(dedupe_key) {
            hits.push(hit);
        }
        if hits.len() >= desired_pool {
            break;
        }
    }
    sort_discover_hits(&mut hits, &args.index, Some(query));
    let total_hits = hits.len();
    let sliced = hits
        .into_iter()
        .skip(args.offset)
        .take(args.limit)
        .collect::<Vec<_>>();
    Ok(DiscoverSearchResult {
        hits: sliced,
        offset: args.offset,
        limit: args.limit,
        total_hits,
    })
}

fn search_github_discover_fallback(
    client: &Client,
    args: &SearchDiscoverContentArgs,
    existing_hits: &[DiscoverSearchHit],
) -> Vec<DiscoverSearchHit> {
    let normalized = normalize_discover_content_type(&args.content_type);
    if normalized != "mods" {
        return vec![];
    }
    let query = args.query.trim();
    if query.is_empty() {
        return vec![];
    }
    let repos = match search_github_repositories(client, query, GITHUB_DISCOVER_MAX_REPO_CANDIDATES)
    {
        Ok(value) => value,
        Err(err) => {
            eprintln!("github fallback discover search failed: {err}");
            return vec![];
        }
    };
    let modrinth_icon_hints = fetch_modrinth_icon_hints_for_query(client, query);
    let mut dedupe: HashSet<String> = existing_hits
        .iter()
        .map(|hit| hit.project_id.trim().to_ascii_lowercase())
        .collect();
    let mut out: Vec<DiscoverSearchHit> = Vec::new();
    let has_configured_auth_tokens = github_has_configured_tokens();
    let strict_asset_filters = !args.loaders.is_empty()
        || args
            .game_version
            .as_ref()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
    let mut release_fetch_budget = if strict_asset_filters {
        GITHUB_DISCOVER_MAX_RELEASE_FETCHES
    } else {
        (GITHUB_DISCOVER_NONSTRICT_RELEASE_FETCHES / 2).max(6)
    };
    if !has_configured_auth_tokens {
        release_fetch_budget = if strict_asset_filters {
            GITHUB_UNAUTH_STRICT_RELEASE_FETCH_BUDGET
        } else {
            GITHUB_UNAUTH_NONSTRICT_RELEASE_FETCH_BUDGET
        };
    }
    let mut release_fetches = 0usize;
    for repo in repos {
        if github_repo_policy_rejection_reason(&repo).is_some() {
            continue;
        }
        let key = github_repo_project_id(&repo).to_ascii_lowercase();
        if !key.is_empty() && dedupe.contains(&key) {
            continue;
        }
        let repo_tree_loader_hints = if args.loaders.is_empty() {
            HashSet::new()
        } else {
            fetch_github_repo_loader_hints(client, &repo)
        };
        let repo_tree_loader_hints_opt = if repo_tree_loader_hints.is_empty() {
            None
        } else {
            Some(&repo_tree_loader_hints)
        };
        if !github_repo_matches_discover_light_filters(&repo, args, repo_tree_loader_hints_opt) {
            continue;
        }
        let similarity = github_repo_query_similarity(&repo, query);
        let minecraft_signal = github_repo_minecraft_signal_score(&repo);
        if !github_repo_passes_signal_gate(&repo, minecraft_signal, similarity, query) {
            continue;
        }
        if similarity < 8 && repo.stargazers_count < 120 {
            continue;
        }
        let icon_url = github_best_discover_icon_url(&repo, &modrinth_icon_hints);
        let requires_release_validation = minecraft_signal <= 0
            && !github_repo_can_skip_release_validation(&repo, minecraft_signal, similarity);
        if requires_release_validation && release_fetches >= release_fetch_budget {
            continue;
        }
        if !strict_asset_filters && release_fetches >= release_fetch_budget {
            let hit = github_repo_deferred_release_hit(
                &repo,
                query,
                "Fast fallback: release metadata is checked when you open/install this result.",
                icon_url,
                repo_tree_loader_hints_opt,
            );
            if dedupe.insert(hit.project_id.trim().to_ascii_lowercase()) {
                out.push(hit);
            }
            continue;
        }
        let (owner, repo_name) = if !repo.full_name.trim().is_empty() {
            match parse_github_project_id(&repo.full_name) {
                Ok(value) => value,
                Err(_) => continue,
            }
        } else {
            (repo.owner.login.clone(), repo.name.clone())
        };
        release_fetches = release_fetches.saturating_add(1);
        let releases = match fetch_github_releases(client, &owner, &repo_name) {
            Ok(value) => value,
            Err(_) => {
                if strict_asset_filters || requires_release_validation {
                    continue;
                }
                let hit = github_repo_without_release_hit(
                    &repo,
                    query,
                    "Repository found, but release metadata is currently unavailable.",
                    icon_url,
                    repo_tree_loader_hints_opt,
                );
                if dedupe.insert(hit.project_id.trim().to_ascii_lowercase()) {
                    out.push(hit);
                }
                continue;
            }
        };
        let hit = if let Some(selection) = select_github_release_with_asset(
            &repo,
            &releases,
            query,
            None,
            None,
            Some(args),
            repo_tree_loader_hints_opt,
        ) {
            let combined_text = format!(
                "{} {} {} {} {} {} {}",
                repo.name,
                repo.full_name,
                repo.description.clone().unwrap_or_default(),
                repo.topics.join(" "),
                selection.release.tag_name,
                selection.release.name.clone().unwrap_or_default(),
                selection.asset.name
            );
            let mut detected_loader_hints = github_loader_hints_from_text(&combined_text);
            if let Some(extra) = repo_tree_loader_hints_opt {
                detected_loader_hints.extend(extra.iter().cloned());
            }
            let loader_hints_opt = if detected_loader_hints.is_empty() {
                None
            } else {
                Some(&detected_loader_hints)
            };
            github_release_to_discover_hit(&repo, &selection, query, icon_url, loader_hints_opt)
        } else {
            if strict_asset_filters || requires_release_validation {
                continue;
            }
            github_repo_deferred_release_hit(
                &repo,
                query,
                "Repository match found. Release compatibility is checked when you open/install.",
                icon_url,
                repo_tree_loader_hints_opt,
            )
        };
        if dedupe.insert(hit.project_id.trim().to_ascii_lowercase()) {
            out.push(hit);
        }
    }
    out
}

fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

fn sha256_file_hex(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|e| format!("open file '{}' for sha256 failed: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("read file '{}' for sha256 failed: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(format!("{:x}", digest))
}

fn parse_cf_hashes(file: &CurseforgeFile) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for h in &file.hashes {
        let key = match h.algo {
            1 => "sha1",
            2 => "md5",
            _ => continue,
        };
        if !h.value.trim().is_empty() {
            out.insert(key.to_string(), h.value.clone());
        }
    }
    out
}

fn fetch_curseforge_project(
    client: &Client,
    api_key: &str,
    mod_id: i64,
) -> Result<CurseforgeMod, String> {
    let mod_resp = client
        .get(format!("{}/mods/{}", CURSEFORGE_API_BASE, mod_id))
        .header("Accept", "application/json")
        .header("x-api-key", api_key)
        .send()
        .map_err(|e| format!("CurseForge project lookup failed: {e}"))?;
    if !mod_resp.status().is_success() {
        return Err(format!(
            "CurseForge project lookup failed with status {}",
            mod_resp.status()
        ));
    }
    Ok(mod_resp
        .json::<CurseforgeModResponse>()
        .map_err(|e| format!("parse CurseForge project failed: {e}"))?
        .data)
}

fn fetch_curseforge_files(
    client: &Client,
    api_key: &str,
    mod_id: i64,
) -> Result<Vec<CurseforgeFile>, String> {
    let files_resp = client
        .get(format!(
            "{}/mods/{}/files?pageSize=80&index=0",
            CURSEFORGE_API_BASE, mod_id
        ))
        .header("Accept", "application/json")
        .header("x-api-key", api_key)
        .send()
        .map_err(|e| format!("CurseForge files lookup failed: {e}"))?;
    if !files_resp.status().is_success() {
        return Err(format!(
            "CurseForge files lookup failed with status {}",
            files_resp.status()
        ));
    }
    Ok(files_resp
        .json::<CurseforgeFilesResponse>()
        .map_err(|e| format!("parse CurseForge files failed: {e}"))?
        .data)
}

fn fetch_curseforge_file(
    client: &Client,
    api_key: &str,
    mod_id: i64,
    file_id: i64,
) -> Result<CurseforgeFile, String> {
    let resp = client
        .get(format!(
            "{}/mods/{}/files/{}",
            CURSEFORGE_API_BASE, mod_id, file_id
        ))
        .header("Accept", "application/json")
        .header("x-api-key", api_key)
        .send()
        .map_err(|e| format!("CurseForge file lookup failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "CurseForge file lookup failed with status {}",
            resp.status()
        ));
    }
    Ok(resp
        .json::<CurseforgeFileResponse>()
        .map_err(|e| format!("parse CurseForge file failed: {e}"))?
        .data)
}

fn curseforge_relation_is_required(relation_type: i64) -> bool {
    relation_type == 3
}

#[derive(Debug, Clone)]
struct ResolvedCurseforgeInstallMod {
    mod_id: i64,
    file: CurseforgeFile,
}

fn is_curseforge_resolve_transient_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("429")
        || lower.contains("403")
        || lower.contains("forbidden")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection")
        || lower.contains("network")
}

fn dedupe_i64_ordered(values: Vec<i64>) -> Vec<i64> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        if seen.insert(value) {
            out.push(value);
        }
    }
    out
}

fn adaptive_curseforge_resolve_worker_cap(frontier_len: usize, max_cap: usize) -> usize {
    if frontier_len <= 1 {
        return 1;
    }
    let cpu = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 16);
    let target = if frontier_len >= 12 {
        8
    } else if frontier_len >= 6 {
        6
    } else if frontier_len >= 3 {
        4
    } else {
        2
    };
    target
        .min(cpu)
        .min(max_cap.max(1))
        .min(frontier_len.max(1))
        .max(1)
}

fn resolve_curseforge_selected_file_with_cache(
    client: &Client,
    api_key: &str,
    instance: &Instance,
    mod_id: i64,
    files_cache: &Arc<Mutex<HashMap<i64, Vec<CurseforgeFile>>>>,
    selected_files: &Arc<Mutex<HashMap<i64, CurseforgeFile>>>,
) -> Result<CurseforgeFile, String> {
    if let Ok(guard) = selected_files.lock() {
        if let Some(cached) = guard.get(&mod_id) {
            return Ok(cached.clone());
        }
    }

    let mut files = if let Ok(guard) = files_cache.lock() {
        guard.get(&mod_id).cloned().unwrap_or_default()
    } else {
        vec![]
    };

    if files.is_empty() {
        files = fetch_curseforge_files(client, api_key, mod_id)?;
        if let Ok(mut guard) = files_cache.lock() {
            guard.insert(mod_id, files.clone());
        }
    }

    files.retain(|f| {
        !f.file_name.trim().is_empty() && file_looks_compatible_with_instance(f, instance, "mods")
    });
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    let file = files.into_iter().next().ok_or_else(|| {
        format!(
            "No compatible CurseForge file found for dependency project {} ({} + {})",
            mod_id, instance.loader, instance.mc_version
        )
    })?;
    if let Ok(mut guard) = selected_files.lock() {
        guard.insert(mod_id, file.clone());
    }
    Ok(file)
}

fn resolve_curseforge_dependency_chain_with_worker_cap(
    client: &Client,
    api_key: &str,
    instance: &Instance,
    root_mod_id: i64,
    max_worker_cap: usize,
    on_progress: &mut dyn FnMut(usize, usize),
) -> Result<Vec<ResolvedCurseforgeInstallMod>, String> {
    let mut ordered: Vec<i64> = Vec::new();
    let mut visited: HashSet<i64> = HashSet::new();
    let selected_files: Arc<Mutex<HashMap<i64, CurseforgeFile>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let files_cache: Arc<Mutex<HashMap<i64, Vec<CurseforgeFile>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let mut frontier = vec![root_mod_id];
    let mut resolved_count = 0usize;

    while !frontier.is_empty() {
        let level_ids: Vec<i64> = dedupe_i64_ordered(frontier)
            .into_iter()
            .filter(|mod_id| visited.insert(*mod_id))
            .collect();
        if level_ids.is_empty() {
            break;
        }

        ordered.extend(level_ids.iter().copied());

        let queue = Arc::new(Mutex::new(VecDeque::from(level_ids.clone())));
        let results: Arc<Mutex<Vec<(i64, CurseforgeFile, Vec<i64>)>>> =
            Arc::new(Mutex::new(vec![]));
        let first_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let worker_count = adaptive_curseforge_resolve_worker_cap(level_ids.len(), max_worker_cap);
        let mut handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let queue_ref = queue.clone();
            let results_ref = results.clone();
            let error_ref = first_error.clone();
            let selected_ref = selected_files.clone();
            let files_cache_ref = files_cache.clone();
            let client_ref = client.clone();
            let api_key_ref = api_key.to_string();
            let instance_ref = instance.clone();
            handles.push(thread::spawn(move || loop {
                if let Ok(guard) = error_ref.lock() {
                    if guard.is_some() {
                        return;
                    }
                }
                let next = {
                    let mut guard = match queue_ref.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    guard.pop_front()
                };
                let Some(mod_id) = next else {
                    return;
                };
                let file = match resolve_curseforge_selected_file_with_cache(
                    &client_ref,
                    &api_key_ref,
                    &instance_ref,
                    mod_id,
                    &files_cache_ref,
                    &selected_ref,
                ) {
                    Ok(file) => file,
                    Err(err) => {
                        if let Ok(mut guard) = error_ref.lock() {
                            if guard.is_none() {
                                *guard = Some(err);
                            }
                        }
                        return;
                    }
                };
                let deps = file
                    .dependencies
                    .iter()
                    .filter(|dep| {
                        dep.mod_id > 0 && curseforge_relation_is_required(dep.relation_type)
                    })
                    .map(|dep| dep.mod_id)
                    .collect::<Vec<_>>();
                if let Ok(mut guard) = results_ref.lock() {
                    guard.push((mod_id, file, deps));
                }
            }));
        }

        for handle in handles {
            let _ = handle.join();
        }

        if let Ok(mut guard) = first_error.lock() {
            if let Some(err) = guard.take() {
                return Err(err);
            }
        }

        let mut resolved_level = match Arc::try_unwrap(results) {
            Ok(mutex) => mutex.into_inner().unwrap_or_default(),
            Err(shared) => shared.lock().map(|v| v.clone()).unwrap_or_default(),
        };
        resolved_level.sort_by(|a, b| a.0.cmp(&b.0));

        let mut next_frontier: Vec<i64> = Vec::new();
        for (_, _, deps) in &resolved_level {
            for dep_id in deps {
                if !visited.contains(dep_id) {
                    next_frontier.push(*dep_id);
                }
            }
        }

        resolved_count += resolved_level.len();
        on_progress(resolved_count, next_frontier.len());

        for (mod_id, file, _) in resolved_level {
            if let Ok(mut guard) = selected_files.lock() {
                guard.insert(mod_id, file);
            }
        }
        frontier = dedupe_i64_ordered(next_frontier);
    }

    ordered.reverse();
    let mut plan: Vec<ResolvedCurseforgeInstallMod> = Vec::with_capacity(ordered.len());
    for mod_id in ordered {
        let file = selected_files
            .lock()
            .ok()
            .and_then(|guard| guard.get(&mod_id).cloned())
            .ok_or_else(|| {
                format!("Missing selected CurseForge file for dependency project {mod_id}")
            })?;
        plan.push(ResolvedCurseforgeInstallMod { mod_id, file });
    }
    Ok(plan)
}

fn resolve_curseforge_dependency_chain<F>(
    client: &Client,
    api_key: &str,
    instance: &Instance,
    root_mod_id: i64,
    mut on_progress: F,
) -> Result<Vec<ResolvedCurseforgeInstallMod>, String>
where
    F: FnMut(usize, usize),
{
    let max_worker_cap = env_worker_cap_or_default(CURSEFORGE_RESOLVE_WORKERS_MAX_ENV, 8, 1, 16);
    match resolve_curseforge_dependency_chain_with_worker_cap(
        client,
        api_key,
        instance,
        root_mod_id,
        max_worker_cap,
        &mut on_progress,
    ) {
        Ok(plan) => Ok(plan),
        Err(err) => {
            if !is_curseforge_resolve_transient_error(&err) || max_worker_cap <= 1 {
                return Err(err);
            }
            let fallback_caps = [3usize, 1usize];
            for cap in fallback_caps {
                let reduced_cap = cap.min(max_worker_cap);
                if reduced_cap >= max_worker_cap {
                    continue;
                }
                match resolve_curseforge_dependency_chain_with_worker_cap(
                    client,
                    api_key,
                    instance,
                    root_mod_id,
                    reduced_cap,
                    &mut on_progress,
                ) {
                    Ok(plan) => return Ok(plan),
                    Err(fallback_err) => {
                        if !is_curseforge_resolve_transient_error(&fallback_err) {
                            return Err(fallback_err);
                        }
                    }
                }
            }
            Err(err)
        }
    }
}

fn discover_content_type_from_modrinth_project_type(project_type: &str) -> String {
    match project_type.trim().to_lowercase().as_str() {
        "resourcepack" => "resourcepacks".to_string(),
        "shader" => "shaderpacks".to_string(),
        "datapack" => "datapacks".to_string(),
        "modpack" => "modpacks".to_string(),
        _ => "mods".to_string(),
    }
}

fn discover_content_type_from_curseforge_class_id(
    class_id: i64,
    requested_content_type: &str,
) -> String {
    match class_id {
        4471 => "modpacks".to_string(),
        6945 => "datapacks".to_string(),
        12 => {
            if normalize_discover_content_type(requested_content_type) == "shaderpacks" {
                "shaderpacks".to_string()
            } else {
                "resourcepacks".to_string()
            }
        }
        _ => "mods".to_string(),
    }
}

fn curseforge_web_category_path_for_content_type(content_type: &str) -> &'static str {
    match normalize_discover_content_type(content_type).as_str() {
        "resourcepacks" => "texture-packs",
        "shaderpacks" => "shaders",
        "datapacks" => "data-packs",
        "modpacks" => "modpacks",
        _ => "mc-mods",
    }
}

fn curseforge_external_project_url(
    project_id: &str,
    slug: Option<&str>,
    content_type: &str,
) -> String {
    let category = curseforge_web_category_path_for_content_type(content_type);
    let project_slug = slug
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(project_id.trim());
    format!(
        "https://www.curseforge.com/minecraft/{}/{}",
        category, project_slug
    )
}

fn infer_curseforge_project_content_type(
    project: &CurseforgeMod,
    requested_content_type: Option<&str>,
) -> String {
    if let Some(requested) = requested_content_type {
        let normalized = normalize_discover_content_type(requested);
        if !normalized.is_empty() {
            return normalized;
        }
    }

    if project.class_id == 12 {
        let has_shader_category = project.categories.iter().any(|category| {
            category
                .slug
                .as_ref()
                .map(|slug| slug.to_ascii_lowercase().contains("shader"))
                .unwrap_or(false)
                || category.name.to_ascii_lowercase().contains("shader")
        });
        if has_shader_category {
            return "shaderpacks".to_string();
        }
        return "resourcepacks".to_string();
    }

    discover_content_type_from_curseforge_class_id(project.class_id, "mods")
}

fn parse_mc_version_parts_loose(input: &str) -> Option<(u32, u32, Option<u32>)> {
    let mut numbers = Vec::new();
    for token in input.split(|c: char| !c.is_ascii_digit()) {
        if token.is_empty() {
            continue;
        }
        if let Ok(value) = token.parse::<u32>() {
            numbers.push(value);
            if numbers.len() >= 3 {
                break;
            }
        }
    }
    if numbers.len() < 2 {
        return None;
    }
    let patch = if numbers.len() >= 3 {
        Some(numbers[2])
    } else {
        None
    };
    Some((numbers[0], numbers[1], patch))
}

fn minecraft_version_matches_advertised(
    advertised: &str,
    target: &str,
    allow_patch_fallback: bool,
) -> bool {
    let advertised_trimmed = advertised.trim().to_ascii_lowercase();
    let target_trimmed = target.trim().to_ascii_lowercase();
    if advertised_trimmed.is_empty() || target_trimmed.is_empty() {
        return false;
    }
    if advertised_trimmed == target_trimmed {
        return true;
    }
    if !allow_patch_fallback {
        return false;
    }

    let Some((adv_major, adv_minor, _)) = parse_mc_version_parts_loose(&advertised_trimmed) else {
        return false;
    };
    let Some((target_major, target_minor, _)) = parse_mc_version_parts_loose(&target_trimmed)
    else {
        return false;
    };

    adv_major == target_major && adv_minor == target_minor
}

fn file_looks_compatible_with_instance(
    file: &CurseforgeFile,
    instance: &Instance,
    content_type: &str,
) -> bool {
    let normalized = normalize_lock_content_type(content_type);
    let values: Vec<String> = file
        .game_versions
        .iter()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .collect();

    if values.is_empty() {
        return false;
    }
    let target_mc = instance.mc_version.to_lowercase();
    if !values
        .iter()
        .any(|v| minecraft_version_matches_advertised(v, &target_mc, normalized != "mods"))
    {
        return false;
    }
    if normalized != "mods" {
        return true;
    }

    let has_loader_tokens = values.iter().any(|v| {
        v == "fabric" || v == "forge" || v == "quilt" || v == "neoforge" || v == "vanilla"
    });
    if !has_loader_tokens {
        return true;
    }

    let loader = instance.loader.to_lowercase();
    values.iter().any(|v| {
        v == &loader
            || (loader == "neoforge" && (v == "neo forge" || v == "neo-forge"))
            || (loader == "vanilla" && v == "minecraft")
    })
}

fn pick_compatible_version_for_content(
    versions: Vec<ModrinthVersion>,
    instance: &Instance,
    content_type: &str,
) -> Option<ModrinthVersion> {
    let normalized = normalize_lock_content_type(content_type);
    let target_mc = instance.mc_version.to_lowercase();
    let mut compatible: Vec<ModrinthVersion> = versions
        .into_iter()
        .filter(|v| {
            v.game_versions.iter().any(|gv| {
                minecraft_version_matches_advertised(gv, &target_mc, normalized != "mods")
            })
        })
        .filter(|v| {
            if normalized == "mods" {
                return v.loaders.iter().any(|l| l == &instance.loader);
            }
            true
        })
        .collect();
    compatible.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    compatible.into_iter().next()
}

fn list_instance_world_names(instance_dir: &Path) -> Result<Vec<String>, String> {
    let saves_dir = instance_dir.join("saves");
    if !saves_dir.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    let entries = fs::read_dir(&saves_dir).map_err(|e| format!("read saves dir failed: {e}"))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read save entry failed: {e}"))?;
        let path = ent.path();
        if !path.is_dir() {
            continue;
        }
        let name = ent.file_name().to_string_lossy().to_string();
        if !name.trim().is_empty() {
            out.push(name);
        }
    }
    out.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    Ok(out)
}

fn normalize_target_worlds_for_datapack(
    instance_dir: &Path,
    target_worlds: &[String],
) -> Result<Vec<String>, String> {
    let world_set: HashSet<String> = list_instance_world_names(instance_dir)?
        .into_iter()
        .collect();
    if world_set.is_empty() {
        return Err(
            "This instance has no worlds yet. Create a world first to install datapacks."
                .to_string(),
        );
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for world in target_worlds {
        let clean = world.trim().to_string();
        if clean.is_empty() {
            continue;
        }
        if !world_set.contains(&clean) {
            return Err(format!("World '{}' was not found in this instance.", clean));
        }
        if seen.insert(clean.clone()) {
            out.push(clean);
        }
    }
    if out.is_empty() {
        return Err("Select at least one world for datapack installation.".to_string());
    }
    Ok(out)
}

fn write_download_to_content_targets(
    instance_dir: &Path,
    content_type: &str,
    filename: &str,
    target_worlds: &[String],
    bytes: &[u8],
) -> Result<(), String> {
    let normalized = normalize_lock_content_type(content_type);
    match normalized.as_str() {
        "mods" | "resourcepacks" | "shaderpacks" => {
            let dir = content_dir_for_type(instance_dir, &normalized);
            fs::create_dir_all(&dir)
                .map_err(|e| format!("mkdir '{}' failed: {e}", dir.display()))?;
            let out_path = dir.join(filename);
            fs::write(&out_path, bytes)
                .map_err(|e| format!("write '{}' failed: {e}", out_path.display()))?;
        }
        "datapacks" => {
            for world in target_worlds {
                let dir = instance_dir.join("saves").join(world).join("datapacks");
                fs::create_dir_all(&dir)
                    .map_err(|e| format!("mkdir '{}' failed: {e}", dir.display()))?;
                let out_path = dir.join(filename);
                fs::write(&out_path, bytes)
                    .map_err(|e| format!("write '{}' failed: {e}", out_path.display()))?;
            }
        }
        _ => {
            return Err("Unsupported content type for direct install".to_string());
        }
    }
    Ok(())
}

fn write_staged_download_to_content_targets(
    instance_dir: &Path,
    content_type: &str,
    filename: &str,
    target_worlds: &[String],
    staged_path: &Path,
) -> Result<(), String> {
    let normalized = normalize_lock_content_type(content_type);
    match normalized.as_str() {
        "mods" | "resourcepacks" | "shaderpacks" => {
            let dir = content_dir_for_type(instance_dir, &normalized);
            fs::create_dir_all(&dir)
                .map_err(|e| format!("mkdir '{}' failed: {e}", dir.display()))?;
            let out_path = dir.join(filename);
            if out_path.exists() {
                fs::remove_file(&out_path)
                    .map_err(|e| format!("remove '{}' failed: {e}", out_path.display()))?;
            }
            match fs::rename(staged_path, &out_path) {
                Ok(()) => {}
                Err(_) => {
                    fs::copy(staged_path, &out_path).map_err(|e| {
                        format!(
                            "copy '{}' -> '{}' failed: {e}",
                            staged_path.display(),
                            out_path.display()
                        )
                    })?;
                    let _ = fs::remove_file(staged_path);
                }
            }
        }
        "datapacks" => {
            for world in target_worlds {
                let dir = instance_dir.join("saves").join(world).join("datapacks");
                fs::create_dir_all(&dir)
                    .map_err(|e| format!("mkdir '{}' failed: {e}", dir.display()))?;
                let out_path = dir.join(filename);
                let part_path = dir.join(format!("{filename}.part"));
                if part_path.exists() {
                    let _ = fs::remove_file(&part_path);
                }
                fs::copy(staged_path, &part_path).map_err(|e| {
                    format!(
                        "copy staged datapack '{}' -> '{}' failed: {e}",
                        staged_path.display(),
                        part_path.display()
                    )
                })?;
                if out_path.exists() {
                    fs::remove_file(&out_path)
                        .map_err(|e| format!("remove '{}' failed: {e}", out_path.display()))?;
                }
                fs::rename(&part_path, &out_path).map_err(|e| {
                    format!(
                        "move staged datapack '{}' -> '{}' failed: {e}",
                        part_path.display(),
                        out_path.display()
                    )
                })?;
            }
            let _ = fs::remove_file(staged_path);
        }
        _ => {
            return Err("Unsupported content type for direct install".to_string());
        }
    }
    Ok(())
}

fn install_modrinth_content_inner<F>(
    instance: &Instance,
    instance_dir: &Path,
    lock: &mut Lockfile,
    client: &Client,
    project_id: &str,
    project_title: Option<&str>,
    content_type: &str,
    target_worlds: &[String],
    mut on_progress: F,
) -> Result<LockEntry, String>
where
    F: FnMut(u64, Option<u64>),
{
    let normalized = normalize_lock_content_type(content_type);
    if normalized == "modpacks" {
        return Err(
            "Modpack entries are template-only. Import as template in Modpacks & Presets."
                .to_string(),
        );
    }

    let versions = fetch_project_versions(client, project_id)?;
    let version =
        pick_compatible_version_for_content(versions, instance, &normalized).ok_or_else(|| {
            if normalized == "mods" {
                format!(
                    "No compatible Modrinth version found for {} ({} + {})",
                    project_id, instance.loader, instance.mc_version
                )
            } else {
                format!(
                    "No compatible Modrinth {} version found for {} (Minecraft {})",
                    content_type_display_name(&normalized),
                    project_id,
                    instance.mc_version
                )
            }
        })?;
    let file = version
        .files
        .iter()
        .find(|f| f.primary.unwrap_or(false))
        .or_else(|| version.files.first())
        .cloned()
        .ok_or_else(|| format!("Version {} has no downloadable files", version.id))?;

    let safe_filename = sanitize_filename(&file.filename);
    if safe_filename.is_empty() {
        return Err("Resolved filename is invalid".to_string());
    }

    let resolved_title = project_title
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| fetch_project_title(client, project_id))
        .unwrap_or_else(|| project_id.to_string());

    let tmp_dir = instance_dir.join(".openjar_downloads");
    fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("mkdir '{}' failed: {e}", tmp_dir.display()))?;
    let tmp_path = tmp_dir.join(format!("{safe_filename}.{}.part", version.id));
    let mut stream_result = download_stream_to_temp_with_retry(
        client,
        &file.url,
        project_id,
        &tmp_path,
        |downloaded_bytes, total_bytes| on_progress(downloaded_bytes, total_bytes),
    )?;

    let worlds = if normalized == "datapacks" {
        normalize_target_worlds_for_datapack(instance_dir, target_worlds)?
    } else {
        vec![]
    };
    let post_process_started = Instant::now();
    write_staged_download_to_content_targets(
        instance_dir,
        &normalized,
        &safe_filename,
        &worlds,
        &tmp_path,
    )?;
    stream_result.profile.post_process_ms = post_process_started.elapsed().as_millis();
    maybe_log_download_profile(project_id, &stream_result.profile);

    remove_replaced_entries_for_content(lock, instance_dir, project_id, &normalized)?;

    let new_entry = LockEntry {
        source: "modrinth".to_string(),
        project_id: project_id.to_string(),
        version_id: version.id.clone(),
        name: canonical_lock_entry_name(&normalized, &safe_filename, &resolved_title),
        version_number: version.version_number.clone(),
        filename: safe_filename,
        content_type: normalized.clone(),
        target_scope: if normalized == "datapacks" {
            "world".to_string()
        } else {
            "instance".to_string()
        },
        target_worlds: worlds,
        pinned_version: None,
        enabled: true,
        hashes: {
            let mut hashes = file.hashes.clone();
            if !stream_result.sha512.trim().is_empty() {
                hashes
                    .entry("sha512".to_string())
                    .or_insert_with(|| stream_result.sha512.clone());
            }
            hashes
        },
        provider_candidates: vec![ProviderCandidate {
            source: "modrinth".to_string(),
            project_id: project_id.to_string(),
            version_id: version.id.clone(),
            name: resolved_title.clone(),
            version_number: version.version_number.clone(),
            confidence: None,
            reason: None,
        }],
        local_analysis: None,
    };
    lock.entries.push(new_entry.clone());
    Ok(new_entry)
}

fn install_curseforge_content_inner<F>(
    instance: &Instance,
    instance_dir: &Path,
    lock: &mut Lockfile,
    client: &Client,
    api_key: &str,
    project_id: &str,
    project_title: Option<&str>,
    content_type: &str,
    target_worlds: &[String],
    resolved_file: Option<&CurseforgeFile>,
    mut on_progress: F,
) -> Result<LockEntry, String>
where
    F: FnMut(u64, Option<u64>),
{
    let normalized = normalize_lock_content_type(content_type);
    if normalized == "modpacks" {
        return Err(
            "Modpack entries are template-only. Import as template in Modpacks & Presets."
                .to_string(),
        );
    }
    let mod_id = parse_curseforge_project_id(project_id)?;
    let project_key = format!("cf:{mod_id}");
    let project = fetch_curseforge_project(client, api_key, mod_id)?;
    let file = if let Some(file) = resolved_file {
        file.clone()
    } else {
        let mut files = fetch_curseforge_files(client, api_key, mod_id)?;
        files.retain(|f| {
            !f.file_name.trim().is_empty()
                && file_looks_compatible_with_instance(f, instance, &normalized)
        });
        files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
        files.into_iter().next().ok_or_else(|| {
            if normalized == "mods" {
                format!(
                    "No compatible CurseForge file found for {} + {}",
                    instance.loader, instance.mc_version
                )
            } else {
                format!(
                    "No compatible CurseForge {} file found for Minecraft {}",
                    content_type_display_name(&normalized),
                    instance.mc_version
                )
            }
        })?
    };

    let safe_filename = sanitize_filename(&file.file_name);
    if safe_filename.is_empty() {
        return Err("Resolved CurseForge filename is invalid".to_string());
    }
    let download_url = resolve_curseforge_file_download_url(client, api_key, mod_id, &file)?;
    let tmp_dir = instance_dir.join(".openjar_downloads");
    fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("mkdir '{}' failed: {e}", tmp_dir.display()))?;
    let tmp_path = tmp_dir.join(format!("{safe_filename}.{}.part", file.id));
    let mut stream_result = download_stream_to_temp_with_retry(
        client,
        &download_url,
        &format!("cf:{mod_id}:{}", file.id),
        &tmp_path,
        |downloaded_bytes, total_bytes| on_progress(downloaded_bytes, total_bytes),
    )?;

    let worlds = if normalized == "datapacks" {
        normalize_target_worlds_for_datapack(instance_dir, target_worlds)?
    } else {
        vec![]
    };
    let post_process_started = Instant::now();
    write_staged_download_to_content_targets(
        instance_dir,
        &normalized,
        &safe_filename,
        &worlds,
        &tmp_path,
    )?;
    stream_result.profile.post_process_ms = post_process_started.elapsed().as_millis();
    maybe_log_download_profile(&format!("cf:{mod_id}:{}", file.id), &stream_result.profile);

    remove_replaced_entries_for_content(lock, instance_dir, &project_key, &normalized)?;

    let new_entry = LockEntry {
        source: "curseforge".to_string(),
        project_id: project_key,
        version_id: format!("cf_file:{}", file.id),
        name: canonical_lock_entry_name(
            &normalized,
            &safe_filename,
            project_title
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| project.name.clone())
                .as_str(),
        ),
        version_number: if file.display_name.trim().is_empty() {
            file.file_name.clone()
        } else {
            file.display_name.clone()
        },
        filename: safe_filename,
        content_type: normalized.clone(),
        target_scope: if normalized == "datapacks" {
            "world".to_string()
        } else {
            "instance".to_string()
        },
        target_worlds: worlds,
        pinned_version: None,
        enabled: true,
        hashes: parse_cf_hashes(&file),
        provider_candidates: vec![ProviderCandidate {
            source: "curseforge".to_string(),
            project_id: format!("cf:{mod_id}"),
            version_id: format!("cf_file:{}", file.id),
            name: project_title
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| project.name.clone()),
            version_number: if file.display_name.trim().is_empty() {
                file.file_name.clone()
            } else {
                file.display_name.clone()
            },
            confidence: None,
            reason: None,
        }],
        local_analysis: None,
    };
    lock.entries.push(new_entry.clone());
    Ok(new_entry)
}

fn install_github_content_inner<F>(
    instance: &Instance,
    instance_dir: &Path,
    lock: &mut Lockfile,
    client: &Client,
    project_id: &str,
    project_title: Option<&str>,
    content_type: &str,
    target_worlds: &[String],
    mut on_progress: F,
) -> Result<LockEntry, String>
where
    F: FnMut(u64, Option<u64>),
{
    let normalized = normalize_lock_content_type(content_type);
    if normalized != "mods" {
        return Err("GitHub provider currently supports mods only.".to_string());
    }
    if !target_worlds.is_empty() {
        return Err("GitHub mods install at instance scope only.".to_string());
    }

    let (owner, repo_name) = parse_github_project_id(project_id)?;
    let repo = fetch_github_repo(client, &owner, &repo_name)?;
    if let Some(reason) = github_repo_policy_rejection_reason(&repo) {
        return Err(format!(
            "GitHub repository rejected by safety policy: {reason}."
        ));
    }
    let releases = fetch_github_releases(client, &owner, &repo_name)?;
    let repo_loader_hints = fetch_github_repo_loader_hints(client, &repo);
    let repo_loader_hints_opt = if repo_loader_hints.is_empty() {
        None
    } else {
        Some(&repo_loader_hints)
    };
    let query_hint = project_title
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| repo.name.trim());
    let selection = if let Some(selection) = select_github_release_with_asset(
        &repo,
        &releases,
        query_hint,
        Some(&instance.mc_version),
        Some(&instance.loader),
        None,
        repo_loader_hints_opt,
    ) {
        selection
    } else if select_github_release_with_asset(
        &repo,
        &releases,
        query_hint,
        None,
        None,
        None,
        repo_loader_hints_opt,
    )
    .is_some()
    {
        return Err(format!(
            "No compatible GitHub release .jar found for {} + {}. This launcher now requires explicit game-version hints in release assets/tags for GitHub installs.",
            instance.loader, instance.mc_version
        ));
    } else {
        return Err("No acceptable GitHub release with a .jar asset was found.".to_string());
    };

    let safe_filename = sanitize_filename(&selection.asset.name);
    if safe_filename.is_empty() {
        return Err("Resolved GitHub release filename is invalid".to_string());
    }

    let tmp_dir = instance_dir.join(".openjar_downloads");
    fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("mkdir '{}' failed: {e}", tmp_dir.display()))?;
    let tmp_path = tmp_dir.join(format!("{safe_filename}.{}.part", selection.release.id));
    let mut stream_result = download_stream_to_temp_with_retry(
        client,
        &selection.asset.browser_download_url,
        &format!("gh:{owner}/{repo_name}:{}", selection.release.id),
        &tmp_path,
        |downloaded_bytes, total_bytes| on_progress(downloaded_bytes, total_bytes),
    )?;
    let sha256 = sha256_file_hex(&tmp_path)?;

    let post_process_started = Instant::now();
    write_staged_download_to_content_targets(
        instance_dir,
        &normalized,
        &safe_filename,
        &[],
        &tmp_path,
    )?;
    stream_result.profile.post_process_ms = post_process_started.elapsed().as_millis();
    maybe_log_download_profile(
        &format!("gh:{owner}/{repo_name}:{}", selection.release.id),
        &stream_result.profile,
    );

    let project_key = github_project_key(&owner, &repo_name);
    remove_replaced_entries_for_content(lock, instance_dir, &project_key, &normalized)?;
    if project_id.trim() != project_key {
        remove_replaced_entries_for_content(lock, instance_dir, project_id, &normalized)?;
    }

    let mut hashes = extract_github_asset_digest(&selection.asset);
    hashes.insert("sha256".to_string(), sha256);
    if !stream_result.sha512.trim().is_empty() {
        hashes
            .entry("sha512".to_string())
            .or_insert_with(|| stream_result.sha512.clone());
    }
    if selection.has_checksum_sidecar {
        hashes
            .entry("checksum_sidecar".to_string())
            .or_insert_with(|| "present".to_string());
    }

    let resolved_name = project_title
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if repo.full_name.trim().is_empty() {
                None
            } else {
                Some(repo.full_name.clone())
            }
        })
        .unwrap_or_else(|| format!("{owner}/{repo_name}"));
    let version_number = github_release_version_label(&selection.release);
    let version_id = format!("gh_release:{}", selection.release.id);

    let new_entry = LockEntry {
        source: "github".to_string(),
        project_id: project_key.clone(),
        version_id: version_id.clone(),
        name: canonical_lock_entry_name(&normalized, &safe_filename, &resolved_name),
        version_number: version_number.clone(),
        filename: safe_filename,
        content_type: normalized,
        target_scope: "instance".to_string(),
        target_worlds: vec![],
        pinned_version: None,
        enabled: true,
        hashes,
        provider_candidates: vec![ProviderCandidate {
            source: "github".to_string(),
            project_id: project_key,
            version_id,
            name: resolved_name,
            version_number,
            confidence: None,
            reason: Some("GitHub release asset (policy-filtered)".to_string()),
        }],
        local_analysis: None,
    };
    lock.entries.push(new_entry.clone());
    Ok(new_entry)
}

fn default_preset_settings() -> CreatorPresetSettings {
    CreatorPresetSettings {
        dependency_policy: "required".to_string(),
        conflict_strategy: "replace".to_string(),
        provider_priority: vec!["modrinth".to_string(), "curseforge".to_string()],
        snapshot_before_apply: true,
        apply_order: vec![
            "mods".to_string(),
            "resourcepacks".to_string(),
            "shaderpacks".to_string(),
            "datapacks".to_string(),
        ],
        datapack_target_policy: "choose_worlds".to_string(),
    }
}

fn classify_pack_path_content_type(path: &str) -> Option<String> {
    let lower = path.trim().replace('\\', "/").to_lowercase();
    if lower.starts_with("mods/") {
        return Some("mods".to_string());
    }
    if lower.starts_with("resourcepacks/") {
        return Some("resourcepacks".to_string());
    }
    if lower.starts_with("shaderpacks/") {
        return Some("shaderpacks".to_string());
    }
    if lower.contains("/datapacks/") || lower.starts_with("datapacks/") {
        return Some("datapacks".to_string());
    }
    None
}

fn import_modrinth_modpack_template_inner(
    client: &Client,
    project_id: &str,
    project_title: Option<&str>,
) -> Result<CreatorPreset, String> {
    let project_name = project_title
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| fetch_project_title(client, project_id))
        .unwrap_or_else(|| format!("Imported Modrinth pack {}", project_id));

    let mut versions = fetch_project_versions(client, project_id)?;
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    let version = versions
        .first()
        .cloned()
        .ok_or_else(|| "No versions found for this Modrinth project".to_string())?;
    let file = version
        .files
        .iter()
        .find(|f| f.primary.unwrap_or(false))
        .or_else(|| version.files.first())
        .cloned()
        .ok_or_else(|| "Selected Modrinth version has no downloadable files".to_string())?;

    let mut entries: Vec<CreatorPresetEntry> = Vec::new();
    for dep in &version.dependencies {
        if !dep.dependency_type.eq_ignore_ascii_case("required") {
            continue;
        }
        let Some(dep_project_id) = dep.project_id.as_ref() else {
            continue;
        };
        entries.push(CreatorPresetEntry {
            source: "modrinth".to_string(),
            project_id: dep_project_id.clone(),
            title: dep_project_id.clone(),
            content_type: "mods".to_string(),
            pinned_version: dep.version_id.clone(),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            enabled: true,
        });
    }

    let mut resp = client
        .get(&file.url)
        .send()
        .map_err(|e| format!("download modpack failed: {e}"))?;
    if resp.status().is_success() {
        let mut bytes = Vec::new();
        let _ = resp.copy_to(&mut bytes);
        if !bytes.is_empty() {
            if let Ok(mut archive) = ZipArchive::new(Cursor::new(bytes)) {
                if let Ok(mut idx_file) = archive.by_name("modrinth.index.json") {
                    let mut raw = String::new();
                    if idx_file.read_to_string(&mut raw).is_ok() {
                        if let Ok(index) = serde_json::from_str::<ModrinthModpackIndex>(&raw) {
                            if entries.is_empty() {
                                for file in index.files {
                                    let Some(content_type) =
                                        classify_pack_path_content_type(&file.path)
                                    else {
                                        continue;
                                    };
                                    entries.push(CreatorPresetEntry {
                                        source: "modrinth".to_string(),
                                        project_id: format!("packfile:{}", file.path),
                                        title: file.path,
                                        content_type,
                                        pinned_version: None,
                                        target_scope: "instance".to_string(),
                                        target_worlds: vec![],
                                        enabled: false,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if entries.is_empty() {
        return Err("Could not derive installable entries from this Modrinth modpack.".to_string());
    }
    entries.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    Ok(CreatorPreset {
        id: format!("preset_{}", now_millis()),
        name: project_name,
        created_at: now_iso(),
        source_instance_id: "template".to_string(),
        source_instance_name: "Modrinth template".to_string(),
        entries,
        settings: default_preset_settings(),
    })
}

fn import_curseforge_modpack_template_inner(
    client: &Client,
    api_key: &str,
    project_id: &str,
    project_title: Option<&str>,
) -> Result<CreatorPreset, String> {
    let mod_id = parse_curseforge_project_id(project_id)?;
    let mod_resp = client
        .get(format!("{}/mods/{}", CURSEFORGE_API_BASE, mod_id))
        .header("Accept", "application/json")
        .header("x-api-key", api_key)
        .send()
        .map_err(|e| format!("CurseForge project lookup failed: {e}"))?;
    if !mod_resp.status().is_success() {
        return Err(format!(
            "CurseForge project lookup failed with status {}",
            mod_resp.status()
        ));
    }
    let project = mod_resp
        .json::<CurseforgeModResponse>()
        .map_err(|e| format!("parse CurseForge project failed: {e}"))?
        .data;

    let files_resp = client
        .get(format!(
            "{}/mods/{}/files?pageSize=40&index=0",
            CURSEFORGE_API_BASE, mod_id
        ))
        .header("Accept", "application/json")
        .header("x-api-key", api_key)
        .send()
        .map_err(|e| format!("CurseForge files lookup failed: {e}"))?;
    if !files_resp.status().is_success() {
        return Err(format!(
            "CurseForge files lookup failed with status {}",
            files_resp.status()
        ));
    }
    let mut files = files_resp
        .json::<CurseforgeFilesResponse>()
        .map_err(|e| format!("parse CurseForge files failed: {e}"))?
        .data;
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    let file = files
        .first()
        .cloned()
        .ok_or_else(|| "No files found for this CurseForge modpack".to_string())?;
    let download_url = resolve_curseforge_file_download_url(client, api_key, mod_id, &file)?;
    let mut resp = client
        .get(&download_url)
        .send()
        .map_err(|e| format!("download CurseForge modpack failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "download CurseForge modpack failed with status {}",
            resp.status()
        ));
    }
    let mut bytes = Vec::new();
    resp.copy_to(&mut bytes)
        .map_err(|e| format!("read CurseForge modpack failed: {e}"))?;

    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("read CurseForge modpack archive failed: {e}"))?;
    let mut manifest_file = archive
        .by_name("manifest.json")
        .map_err(|_| "manifest.json was not found in the CurseForge modpack archive".to_string())?;
    let mut manifest_raw = String::new();
    manifest_file
        .read_to_string(&mut manifest_raw)
        .map_err(|e| format!("read manifest.json failed: {e}"))?;
    let manifest = serde_json::from_str::<CurseforgeModpackManifest>(&manifest_raw)
        .map_err(|e| format!("parse manifest.json failed: {e}"))?;

    let mut entries = Vec::new();
    for file_ref in manifest.files {
        entries.push(CreatorPresetEntry {
            source: "curseforge".to_string(),
            project_id: format!("cf:{}", file_ref.project_id),
            title: format!("CurseForge {}", file_ref.project_id),
            content_type: "mods".to_string(),
            pinned_version: Some(format!("cf_file:{}", file_ref.file_id)),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            enabled: true,
        });
    }
    if entries.is_empty() {
        return Err(
            "This CurseForge modpack manifest does not contain installable files.".to_string(),
        );
    }

    let preset_name = manifest
        .name
        .or_else(|| project_title.map(|v| v.to_string()))
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| project.name);

    Ok(CreatorPreset {
        id: format!("preset_{}", now_millis()),
        name: preset_name,
        created_at: now_iso(),
        source_instance_id: "template".to_string(),
        source_instance_name: "CurseForge template".to_string(),
        entries,
        settings: default_preset_settings(),
    })
}

fn search_modrinth_discover(
    client: &Client,
    args: &SearchDiscoverContentArgs,
) -> Result<DiscoverSearchResult, String> {
    let content_type = normalize_discover_content_type(&args.content_type);
    let project_type_facets = modrinth_project_type_facets(&content_type);

    let mut params = vec![
        ("query".to_string(), args.query.clone()),
        ("index".to_string(), args.index.clone()),
        ("limit".to_string(), args.limit.to_string()),
        ("offset".to_string(), args.offset.to_string()),
    ];

    let mut groups: Vec<Vec<String>> = vec![project_type_facets];
    if !args.loaders.is_empty() {
        groups.push(
            args.loaders
                .iter()
                .map(|l| format!("categories:{}", l.trim().to_lowercase()))
                .collect(),
        );
    }
    if let Some(game_version) = args.game_version.as_ref() {
        if !game_version.trim().is_empty() {
            groups.push(vec![format!("versions:{}", game_version.trim())]);
        }
    }
    if !args.categories.is_empty() {
        groups.push(
            args.categories
                .iter()
                .map(|c| format!("categories:{}", c.trim().to_lowercase()))
                .collect(),
        );
    }
    let facets =
        serde_json::to_string(&groups).map_err(|e| format!("serialize facets failed: {e}"))?;
    params.push(("facets".to_string(), facets));

    let query = params
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                url::form_urlencoded::byte_serialize(k.as_bytes()).collect::<String>(),
                url::form_urlencoded::byte_serialize(v.as_bytes()).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    let url = format!("{}/search?{}", modrinth_api_base(), query);
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("Modrinth discover search failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Modrinth discover search failed with status {}",
            resp.status()
        ));
    }
    let payload = resp
        .json::<serde_json::Value>()
        .map_err(|e| format!("parse Modrinth discover search failed: {e}"))?;
    let offset = payload
        .get("offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(args.offset as u64) as usize;
    let limit = payload
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(args.limit as u64) as usize;
    let total_hits = payload
        .get("total_hits")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let mut hits = Vec::new();
    if let Some(arr) = payload.get("hits").and_then(|v| v.as_array()) {
        for it in arr {
            let project_id = it
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if project_id.is_empty() {
                continue;
            }
            let title = it
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();
            let description = it
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let author = it
                .get("author")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let downloads = it.get("downloads").and_then(|v| v.as_u64()).unwrap_or(0);
            let follows = it.get("follows").and_then(|v| v.as_u64()).unwrap_or(0);
            let icon_url = it
                .get("icon_url")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string());
            let categories = it
                .get("categories")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| c.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let versions = it
                .get("versions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| c.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let date_modified = it
                .get("date_modified")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let slug = it
                .get("slug")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string());
            let hit_content_type = discover_content_type_from_modrinth_project_type(
                it.get("project_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default(),
            );

            hits.push(DiscoverSearchHit {
                source: "modrinth".to_string(),
                project_id,
                title,
                description,
                author,
                downloads,
                follows,
                icon_url,
                categories,
                versions,
                date_modified,
                content_type: hit_content_type,
                slug: slug.clone(),
                external_url: slug.map(|s| format!("https://modrinth.com/project/{s}")),
                confidence: None,
                reason: None,
                install_supported: None,
                install_note: None,
            });
        }
    }

    Ok(DiscoverSearchResult {
        hits,
        offset,
        limit,
        total_hits,
    })
}

fn search_curseforge_discover(
    client: &Client,
    args: &SearchDiscoverContentArgs,
) -> Result<DiscoverSearchResult, String> {
    let api_key = curseforge_api_key().ok_or_else(discover_missing_curseforge_key_message)?;
    let content_type = normalize_discover_content_type(&args.content_type);
    let class_ids = curseforge_class_ids_for_content_type(&content_type);
    let sort_field = discover_index_sort_field(&args.index);
    let mut all_hits: Vec<DiscoverSearchHit> = Vec::new();
    let mut aggregate_total = 0usize;

    for class_id in class_ids {
        let mut query_pairs: Vec<(String, String)> = vec![
            (
                "gameId".to_string(),
                CURSEFORGE_GAME_ID_MINECRAFT.to_string(),
            ),
            ("classId".to_string(), class_id.to_string()),
            ("sortField".to_string(), sort_field.to_string()),
            ("sortOrder".to_string(), "desc".to_string()),
            (
                "pageSize".to_string(),
                (args.limit + args.offset).max(20).to_string(),
            ),
            ("index".to_string(), "0".to_string()),
        ];

        let q_trim = args.query.trim();
        if !q_trim.is_empty() {
            query_pairs.push(("searchFilter".to_string(), q_trim.to_string()));
        } else if content_type == "shaderpacks" {
            query_pairs.push(("searchFilter".to_string(), "shader".to_string()));
        }

        if let Some(game_version) = args.game_version.as_ref() {
            let gv = game_version.trim();
            if !gv.is_empty() {
                query_pairs.push(("gameVersion".to_string(), gv.to_string()));
            }
        }

        let query = query_pairs
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}={}",
                    url::form_urlencoded::byte_serialize(k.as_bytes()).collect::<String>(),
                    url::form_urlencoded::byte_serialize(v.as_bytes()).collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("&");
        let url = format!("{}/mods/search?{}", CURSEFORGE_API_BASE, query);
        let resp = client
            .get(&url)
            .header("Accept", "application/json")
            .header("x-api-key", api_key.clone())
            .send()
            .map_err(|e| format!("CurseForge search failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!(
                "CurseForge search failed with status {} (classId={})",
                resp.status(),
                class_id
            ));
        }
        let payload = resp
            .json::<CurseforgeSearchResponse>()
            .map_err(|e| format!("parse CurseForge search failed: {e}"))?;
        aggregate_total += payload
            .pagination
            .as_ref()
            .map(|p| p.total_count)
            .unwrap_or(payload.data.len());

        for item in payload.data {
            let project_id = item.id.to_string();
            let title = if item.name.trim().is_empty() {
                format!("CurseForge #{}", item.id)
            } else {
                item.name.clone()
            };
            let author = item
                .authors
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let categories = item
                .categories
                .iter()
                .filter_map(|c| c.slug.clone().or_else(|| Some(c.name.clone())))
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>();
            let hit_content_type =
                discover_content_type_from_curseforge_class_id(class_id, &content_type);
            let follows = 0_u64;
            all_hits.push(DiscoverSearchHit {
                source: "curseforge".to_string(),
                project_id: project_id.clone(),
                title,
                description: item.summary.clone(),
                author,
                downloads: item.download_count.max(0.0) as u64,
                follows,
                icon_url: item.logo.as_ref().map(|l| l.url.clone()),
                categories,
                versions: Vec::new(),
                date_modified: item.date_modified.clone(),
                content_type: hit_content_type.clone(),
                slug: item.slug.clone(),
                external_url: Some(curseforge_external_project_url(
                    &project_id,
                    item.slug.as_deref(),
                    &hit_content_type,
                )),
                confidence: None,
                reason: None,
                install_supported: None,
                install_note: None,
            });
        }
    }

    if !args.categories.is_empty() {
        let requested = args
            .categories
            .iter()
            .map(|value| normalize_provider_match_key(value))
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if !requested.is_empty() {
            all_hits.retain(|hit| {
                let categories_text = hit.categories.join(" ");
                let normalized_hit_text = normalize_provider_match_key(&categories_text);
                requested.iter().any(|category| {
                    normalized_hit_text.contains(category)
                        || hit
                            .categories
                            .iter()
                            .any(|value| normalize_provider_match_key(value).contains(category))
                })
            });
        }
    }

    let filtered_total = all_hits.len();
    all_hits.sort_by(|a, b| b.date_modified.cmp(&a.date_modified));
    let sliced = all_hits
        .into_iter()
        .skip(args.offset)
        .take(args.limit)
        .collect::<Vec<_>>();
    let total_hits = if args.categories.is_empty() {
        aggregate_total.max(filtered_total)
    } else {
        filtered_total
    };

    Ok(DiscoverSearchResult {
        hits: sliced,
        offset: args.offset,
        limit: args.limit,
        total_hits,
    })
}

fn resolve_curseforge_file_download_url(
    client: &Client,
    api_key: &str,
    mod_id: i64,
    file: &CurseforgeFile,
) -> Result<String, String> {
    if let Some(url) = file.download_url.as_ref() {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let fallback = format!(
        "{}/mods/{}/files/{}/download-url",
        CURSEFORGE_API_BASE, mod_id, file.id
    );
    let response = client
        .get(&fallback)
        .header("Accept", "application/json")
        .header("x-api-key", api_key)
        .send()
        .map_err(|e| format!("CurseForge download-url lookup failed: {e}"));

    let prior_error = match response {
        Ok(resp) if resp.status().is_success() => {
            let payload = resp
                .json::<CurseforgeDownloadUrlResponse>()
                .map_err(|e| format!("parse CurseForge download-url response failed: {e}"))?;
            let url = payload.data.trim().to_string();
            if !url.is_empty() {
                return Ok(url);
            }
            "CurseForge file has no download url".to_string()
        }
        Ok(resp) if resp.status() == reqwest::StatusCode::FORBIDDEN => {
            return Err(
                "CurseForge blocked automated download URL access for this file (HTTP 403). \
This file may disallow third-party downloads. Try another file/provider or import the file manually."
                    .to_string(),
            );
        }
        Ok(resp) => format!(
            "CurseForge download-url lookup failed with status {}",
            resp.status()
        ),
        Err(err) => err,
    };

    if let Ok(fresh_file) = fetch_curseforge_file(client, api_key, mod_id, file.id) {
        if let Some(url) = fresh_file.download_url.as_ref() {
            let trimmed = url.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    Err(prior_error)
}

fn sort_discover_hits(hits: &mut [DiscoverSearchHit], index: &str, query: Option<&str>) {
    let relevance_cmp = |left: &DiscoverSearchHit, right: &DiscoverSearchHit| {
        let left_score = query
            .map(|value| discover_hit_query_score(left, value))
            .unwrap_or(0);
        let right_score = query
            .map(|value| discover_hit_query_score(right, value))
            .unwrap_or(0);
        right_score
            .cmp(&left_score)
            .then_with(|| right.downloads.cmp(&left.downloads))
            .then_with(|| right.follows.cmp(&left.follows))
            .then_with(|| right.date_modified.cmp(&left.date_modified))
    };

    match index.trim().to_lowercase().as_str() {
        "downloads" => hits.sort_by(|a, b| {
            b.downloads
                .cmp(&a.downloads)
                .then_with(|| relevance_cmp(a, b))
        }),
        "follows" => {
            hits.sort_by(|a, b| b.follows.cmp(&a.follows).then_with(|| relevance_cmp(a, b)))
        }
        "updated" | "newest" => hits.sort_by(|a, b| {
            b.date_modified
                .cmp(&a.date_modified)
                .then_with(|| relevance_cmp(a, b))
        }),
        _ => hits.sort_by(relevance_cmp),
    }
}

fn blend_discover_hits_prefer_modrinth(hits: Vec<DiscoverSearchHit>) -> Vec<DiscoverSearchHit> {
    let mut modrinth_hits = VecDeque::<DiscoverSearchHit>::new();
    let mut other_hits = VecDeque::<DiscoverSearchHit>::new();
    for hit in hits {
        if hit.source.eq_ignore_ascii_case("modrinth") {
            modrinth_hits.push_back(hit);
        } else {
            other_hits.push_back(hit);
        }
    }

    if modrinth_hits.is_empty() || other_hits.is_empty() {
        let mut passthrough = Vec::with_capacity(modrinth_hits.len() + other_hits.len());
        passthrough.extend(modrinth_hits);
        passthrough.extend(other_hits);
        return passthrough;
    }

    // Favor Modrinth while keeping mixed-provider visibility (2:1 cadence).
    let mut blended = Vec::with_capacity(modrinth_hits.len() + other_hits.len());
    while !modrinth_hits.is_empty() || !other_hits.is_empty() {
        for _ in 0..2 {
            if let Some(hit) = modrinth_hits.pop_front() {
                blended.push(hit);
            }
        }
        if let Some(hit) = other_hits.pop_front() {
            blended.push(hit);
        }
        if modrinth_hits.is_empty() {
            blended.extend(other_hits.drain(..));
            break;
        }
        if other_hits.is_empty() {
            blended.extend(modrinth_hits.drain(..));
            break;
        }
    }
    blended
}

fn home_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            let candidate = PathBuf::from(profile);
            if !candidate.as_os_str().is_empty() {
                return Some(candidate);
            }
        }
        let drive = std::env::var_os("HOMEDRIVE");
        let path = std::env::var_os("HOMEPATH");
        if let (Some(drive), Some(path)) = (drive, path) {
            let joined = format!("{}{}", drive.to_string_lossy(), path.to_string_lossy());
            let candidate = PathBuf::from(joined);
            if !candidate.as_os_str().is_empty() {
                return Some(candidate);
            }
        }
        None
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn prism_root_dir() -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("MPM_PRISM_ROOT") {
        let p = PathBuf::from(custom.trim());
        if !p.as_os_str().is_empty() {
            return Ok(p);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        if let Some(home) = home_dir() {
            candidates.push(
                home.join("Library")
                    .join("Application Support")
                    .join("PrismLauncher"),
            );
        }
    } else if cfg!(target_os = "windows") {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            candidates.push(PathBuf::from(appdata).join("PrismLauncher"));
        }
        if let Some(home) = home_dir() {
            candidates.push(home.join("AppData").join("Roaming").join("PrismLauncher"));
        }
    } else if let Some(home) = home_dir() {
        candidates.push(home.join(".local").join("share").join("PrismLauncher"));
        candidates.push(
            home.join(".var")
                .join("app")
                .join("org.prismlauncher.PrismLauncher")
                .join("data")
                .join("PrismLauncher"),
        );
    }

    if let Some(existing) = candidates
        .iter()
        .find(|p| p.exists() && p.is_dir())
        .cloned()
    {
        return Ok(existing);
    }
    if let Some(default_candidate) = candidates.into_iter().next() {
        return Ok(default_candidate);
    }

    Err("Failed to resolve Prism Launcher root. Set MPM_PRISM_ROOT.".into())
}

fn parse_instance_cfg_name(cfg_path: &Path) -> Option<String> {
    let content = fs::read_to_string(cfg_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(v) = trimmed.strip_prefix("name=") {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn find_prism_instance_id(prism_root: &Path, instance: &Instance) -> Result<String, String> {
    let instances_dir = prism_root.join("instances");
    if !instances_dir.exists() {
        return Err(format!(
            "Prism instances folder not found at '{}'",
            instances_dir.display()
        ));
    }

    if instances_dir.join(&instance.id).is_dir() {
        return Ok(instance.id.clone());
    }

    let mut by_name: Option<String> = None;
    let target_name = instance.name.trim().to_lowercase();
    if !target_name.is_empty() {
        let read = fs::read_dir(&instances_dir)
            .map_err(|e| format!("read Prism instances failed: {e}"))?;
        for ent in read {
            let ent = ent.map_err(|e| format!("read Prism instance entry failed: {e}"))?;
            if !ent.path().is_dir() {
                continue;
            }
            let cfg = ent.path().join("instance.cfg");
            let Some(name) = parse_instance_cfg_name(&cfg) else {
                continue;
            };
            if name.trim().to_lowercase() == target_name {
                by_name = Some(ent.file_name().to_string_lossy().to_string());
                break;
            }
        }
    }

    by_name.ok_or_else(|| {
        format!(
            "No Prism instance matched '{}'. Create one in Prism first (same visible name), or set folder ID to '{}'.",
            instance.name, instance.id
        )
    })
}

fn parse_instance_cfg_value(cfg_path: &Path, key: &str) -> Option<String> {
    let content = fs::read_to_string(cfg_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(v) = trimmed.strip_prefix(&format!("{key}=")) {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn parse_loader_from_hint(input: &str) -> String {
    let lower = input.trim().to_lowercase();
    if lower.contains("neoforge") {
        return "neoforge".to_string();
    }
    if lower.contains("fabric") {
        return "fabric".to_string();
    }
    if lower.contains("quilt") {
        return "quilt".to_string();
    }
    if lower.contains("forge") {
        return "forge".to_string();
    }
    "vanilla".to_string()
}

fn vanilla_minecraft_dir() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        if let Some(home) = home_dir() {
            candidates.push(
                home.join("Library")
                    .join("Application Support")
                    .join("minecraft"),
            );
        }
    } else if cfg!(target_os = "windows") {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            candidates.push(PathBuf::from(appdata).join(".minecraft"));
        }
        if let Some(home) = home_dir() {
            candidates.push(home.join("AppData").join("Roaming").join(".minecraft"));
        }
    } else if let Some(home) = home_dir() {
        candidates.push(home.join(".minecraft"));
        candidates.push(
            home.join(".var")
                .join("app")
                .join("com.mojang.Minecraft")
                .join(".minecraft"),
        );
    }

    candidates
        .iter()
        .find(|p| p.exists() && p.is_dir())
        .cloned()
        .or_else(|| candidates.into_iter().next())
}

fn detect_latest_release_version_from_dir(mc_dir: &Path) -> Option<String> {
    let versions_dir = mc_dir.join("versions");
    if !versions_dir.exists() {
        return None;
    }
    let mut candidates: Vec<String> = Vec::new();
    let entries = fs::read_dir(&versions_dir).ok()?;
    for ent in entries.flatten() {
        let path = ent.path();
        if !path.is_dir() {
            continue;
        }
        let id = ent.file_name().to_string_lossy().to_string();
        if id
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
            && id.chars().any(|c| c == '.')
        {
            candidates.push(id);
        }
    }
    candidates.sort();
    candidates.pop()
}

fn detect_prism_instance_meta(prism_instance_dir: &Path) -> (String, String) {
    let mut mc_version = "1.20.1".to_string();
    let mut loader = "vanilla".to_string();
    let mmc_pack = prism_instance_dir.join("mmc-pack.json");
    if let Ok(raw) = fs::read_to_string(&mmc_pack) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(components) = value.get("components").and_then(|v| v.as_array()) {
                for comp in components {
                    let uid = comp
                        .get("uid")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_lowercase();
                    let version = comp
                        .get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if uid == "net.minecraft" && !version.trim().is_empty() {
                        mc_version = version;
                    }
                    if uid.contains("fabric-loader") {
                        loader = "fabric".to_string();
                    } else if uid.contains("neoforge") {
                        loader = "neoforge".to_string();
                    } else if uid.contains("quilt") {
                        loader = "quilt".to_string();
                    } else if uid.contains("forge") {
                        loader = "forge".to_string();
                    }
                }
            }
        }
    }
    (mc_version, loader)
}

fn list_launcher_import_sources_inner() -> Vec<LauncherImportSource> {
    let mut out: Vec<LauncherImportSource> = Vec::new();
    if let Some(mc_dir) = vanilla_minecraft_dir() {
        if mc_dir.exists() && mc_dir.is_dir() {
            out.push(LauncherImportSource {
                id: "vanilla:default".to_string(),
                source_kind: "vanilla".to_string(),
                label: "Vanilla Minecraft".to_string(),
                mc_version: detect_latest_release_version_from_dir(&mc_dir)
                    .unwrap_or_else(|| "1.20.1".to_string()),
                loader: "vanilla".to_string(),
                source_path: mc_dir.display().to_string(),
            });
        }
    }

    if let Ok(prism_root) = prism_root_dir() {
        let instances_dir = prism_root.join("instances");
        if instances_dir.exists() && instances_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&instances_dir) {
                for ent in entries.flatten() {
                    let path = ent.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let folder_id = ent.file_name().to_string_lossy().to_string();
                    let cfg_path = path.join("instance.cfg");
                    let mc_dir = path.join(".minecraft");
                    if !cfg_path.exists() || !mc_dir.exists() {
                        continue;
                    }
                    let label = parse_instance_cfg_value(&cfg_path, "name")
                        .filter(|v| !v.trim().is_empty())
                        .unwrap_or_else(|| folder_id.clone());
                    let (mc_version, loader) = detect_prism_instance_meta(&path);
                    out.push(LauncherImportSource {
                        id: format!("prism:{folder_id}"),
                        source_kind: "prism".to_string(),
                        label,
                        mc_version,
                        loader,
                        source_path: mc_dir.display().to_string(),
                    });
                }
            }
        }
    }

    out.sort_by(|a, b| {
        a.source_kind
            .cmp(&b.source_kind)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    out
}

fn copy_dir_recursive_count(src: &Path, dst: &Path) -> Result<usize, String> {
    if !src.exists() {
        return Ok(0);
    }
    fs::create_dir_all(dst).map_err(|e| format!("mkdir '{}' failed: {e}", dst.display()))?;
    let entries = fs::read_dir(src).map_err(|e| format!("read '{}' failed: {e}", src.display()))?;
    let mut copied = 0usize;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read dir entry failed: {e}"))?;
        let src_path = ent.path();
        let dst_path = dst.join(ent.file_name());
        let meta = ent
            .metadata()
            .map_err(|e| format!("read metadata '{}' failed: {e}", src_path.display()))?;
        if meta.is_dir() {
            copied += copy_dir_recursive_count(&src_path, &dst_path)?;
        } else if meta.is_file() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir '{}' failed: {e}", parent.display()))?;
            }
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!(
                    "copy '{}' -> '{}' failed: {e}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn copy_file_if_exists(src: &Path, dst: &Path) -> Result<usize, String> {
    if !src.exists() || !src.is_file() {
        return Ok(0);
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("mkdir '{}' failed: {e}", parent.display()))?;
    }
    fs::copy(src, dst).map_err(|e| {
        format!(
            "copy '{}' -> '{}' failed: {e}",
            src.display(),
            dst.display()
        )
    })?;
    Ok(1)
}

fn copy_launcher_source_into_instance(
    source_mc_dir: &Path,
    instance_dir: &Path,
) -> Result<usize, String> {
    let mut copied = 0usize;
    copied += copy_dir_recursive_count(&source_mc_dir.join("mods"), &instance_dir.join("mods"))?;
    copied +=
        copy_dir_recursive_count(&source_mc_dir.join("config"), &instance_dir.join("config"))?;
    copied += copy_dir_recursive_count(
        &source_mc_dir.join("resourcepacks"),
        &instance_dir.join("resourcepacks"),
    )?;
    copied += copy_dir_recursive_count(
        &source_mc_dir.join("shaderpacks"),
        &instance_dir.join("shaderpacks"),
    )?;
    copied += copy_dir_recursive_count(&source_mc_dir.join("saves"), &instance_dir.join("saves"))?;
    copied += copy_file_if_exists(
        &source_mc_dir.join("options.txt"),
        &instance_dir.join("options.txt"),
    )?;
    copied += copy_file_if_exists(
        &source_mc_dir.join("servers.dat"),
        &instance_dir.join("servers.dat"),
    )?;
    Ok(copied)
}

fn parse_modpack_file_info(
    file_path: &Path,
) -> Result<(String, String, String, Vec<String>, Vec<String>), String> {
    let file = File::open(file_path).map_err(|e| format!("open modpack archive failed: {e}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("read modpack archive failed: {e}"))?;

    if let Ok(mut idx_file) = archive.by_name("modrinth.index.json") {
        let mut raw = String::new();
        idx_file
            .read_to_string(&mut raw)
            .map_err(|e| format!("read modrinth.index.json failed: {e}"))?;
        let value = serde_json::from_str::<serde_json::Value>(&raw)
            .map_err(|e| format!("parse modrinth.index.json failed: {e}"))?;
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "Imported Modrinth Pack".to_string());
        let deps = value
            .get("dependencies")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let mc_version = deps
            .get("minecraft")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "1.20.1".to_string());

        let mut loader = "vanilla".to_string();
        for key in deps.keys() {
            let parsed = parse_loader_from_hint(key);
            if parsed != "vanilla" {
                loader = parsed;
                break;
            }
        }

        return Ok((
            name,
            mc_version,
            loader,
            vec!["overrides".to_string(), "client-overrides".to_string()],
            vec![],
        ));
    }

    if let Ok(mut manifest_file) = archive.by_name("manifest.json") {
        let mut raw = String::new();
        manifest_file
            .read_to_string(&mut raw)
            .map_err(|e| format!("read manifest.json failed: {e}"))?;
        let value = serde_json::from_str::<serde_json::Value>(&raw)
            .map_err(|e| format!("parse manifest.json failed: {e}"))?;
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "Imported CurseForge Pack".to_string());

        let mc_version = value
            .get("minecraft")
            .and_then(|v| v.get("version"))
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "1.20.1".to_string());

        let mut loader = "vanilla".to_string();
        if let Some(loaders) = value
            .get("minecraft")
            .and_then(|v| v.get("modLoaders"))
            .and_then(|v| v.as_array())
        {
            for row in loaders {
                let id = row.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                let parsed = parse_loader_from_hint(id);
                if parsed != "vanilla" {
                    loader = parsed;
                    break;
                }
            }
        }

        let override_dir = value
            .get("overrides")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "overrides".to_string());

        return Ok((name, mc_version, loader, vec![override_dir], vec![]));
    }

    Err("Unsupported modpack archive. Expected modrinth.index.json or manifest.json.".to_string())
}

fn extract_overrides_from_modpack(
    file_path: &Path,
    instance_dir: &Path,
    override_roots: &[String],
) -> Result<usize, String> {
    if override_roots.is_empty() {
        return Ok(0);
    }
    let file = File::open(file_path).map_err(|e| format!("open modpack archive failed: {e}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("read modpack archive failed: {e}"))?;

    let mut extracted = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("read modpack archive entry failed: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        let rel = enclosed.to_string_lossy().replace('\\', "/");
        let mut matched_rel: Option<String> = None;
        for root in override_roots {
            let root_norm = root.trim().trim_matches('/').to_string();
            if root_norm.is_empty() {
                continue;
            }
            let prefix = format!("{root_norm}/");
            if rel == root_norm {
                matched_rel = Some(String::new());
                break;
            }
            if let Some(rest) = rel.strip_prefix(&prefix) {
                matched_rel = Some(rest.to_string());
                break;
            }
        }
        let Some(out_rel) = matched_rel else {
            continue;
        };
        if out_rel.is_empty() {
            continue;
        }
        if out_rel.starts_with("snapshots/") || out_rel.starts_with("runtime/") {
            continue;
        }
        let out_path = instance_dir.join(&out_rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir override parent failed: {e}"))?;
        }
        let mut out =
            File::create(&out_path).map_err(|e| format!("write override file failed: {e}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("extract override failed: {e}"))?;
        extracted += 1;
    }
    Ok(extracted)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|e| format!("mkdir '{}' failed: {e}", dst.display()))?;
    let entries = fs::read_dir(src).map_err(|e| format!("read '{}' failed: {e}", src.display()))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read dir entry failed: {e}"))?;
        let src_path = ent.path();
        let dst_path = dst.join(ent.file_name());
        let meta = ent
            .metadata()
            .map_err(|e| format!("read metadata '{}' failed: {e}", src_path.display()))?;
        if meta.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if meta.is_file() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir '{}' failed: {e}", parent.display()))?;
            }
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!(
                    "copy '{}' -> '{}' failed: {e}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn copy_entry_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    let meta = fs::metadata(src)
        .map_err(|e| format!("read source metadata '{}' failed: {e}", src.display()))?;
    if meta.is_dir() {
        copy_dir_recursive(src, dst)
    } else if meta.is_file() {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir '{}' failed: {e}", parent.display()))?;
        }
        fs::copy(src, dst).map_err(|e| {
            format!(
                "copy '{}' -> '{}' failed: {e}",
                src.display(),
                dst.display()
            )
        })?;
        Ok(())
    } else {
        Ok(())
    }
}

fn ensure_instance_content_dirs(instance_dir: &Path) -> Result<(), String> {
    for seg in ["mods", "config", "resourcepacks", "shaderpacks", "saves"] {
        let dir = instance_dir.join(seg);
        fs::create_dir_all(&dir)
            .map_err(|e| format!("mkdir instance content '{}' failed: {e}", dir.display()))?;
    }
    Ok(())
}

fn runtime_reconcile_marker_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(RUNTIME_RECONCILE_MARKER_FILE)
}

fn runtime_reconcile_skip_entry(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "assets" | "libraries" | "versions" | "logs" | "crash-reports" | "runtime_sessions"
    )
}

fn runtime_reconcile_newest_wins(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "options.txt" | "servers.dat" | "usercache.json"
    )
}

fn write_runtime_reconcile_marker(instance_dir: &Path) -> Result<(), String> {
    let marker = runtime_reconcile_marker_path(instance_dir);
    if marker.exists() {
        return Ok(());
    }
    fs::write(&marker, now_iso()).map_err(|e| {
        format!(
            "write runtime reconcile marker '{}' failed: {e}",
            marker.display()
        )
    })
}

fn is_source_newer_than_destination(src: &Path, dst: &Path) -> bool {
    let src_modified = fs::metadata(src).and_then(|m| m.modified()).ok();
    let dst_modified = fs::metadata(dst).and_then(|m| m.modified()).ok();
    match (src_modified, dst_modified) {
        (Some(a), Some(b)) => a > b,
        (Some(_), None) => true,
        _ => false,
    }
}

fn reconcile_legacy_runtime_into_instance(instance_dir: &Path) -> Result<(), String> {
    let marker = runtime_reconcile_marker_path(instance_dir);
    if marker.exists() {
        return Ok(());
    }

    let runtime_root = instance_dir.join("runtime");
    if !runtime_root.exists() || !runtime_root.is_dir() {
        return write_runtime_reconcile_marker(instance_dir);
    }

    let mut copied = 0usize;
    let mut replaced = 0usize;
    let mut skipped = 0usize;
    let entries = fs::read_dir(&runtime_root).map_err(|e| {
        format!(
            "read legacy runtime '{}' failed: {e}",
            runtime_root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read legacy runtime entry failed: {e}"))?;
        let src_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.trim().is_empty() || runtime_reconcile_skip_entry(&name) {
            continue;
        }
        let dst_path = instance_dir.join(&name);
        if !dst_path.exists() {
            copy_entry_recursive(&src_path, &dst_path)?;
            copied += 1;
            continue;
        }
        if runtime_reconcile_newest_wins(&name)
            && src_path.is_file()
            && dst_path.is_file()
            && is_source_newer_than_destination(&src_path, &dst_path)
        {
            copy_entry_recursive(&src_path, &dst_path)?;
            replaced += 1;
            continue;
        }
        skipped += 1;
        eprintln!(
            "runtime reconcile skipped conflicting entry '{}': destination '{}' already exists",
            name,
            dst_path.display()
        );
    }
    eprintln!(
        "runtime reconcile complete for '{}': copied={}, replaced={}, skipped={}",
        instance_dir.display(),
        copied,
        replaced,
        skipped
    );
    write_runtime_reconcile_marker(instance_dir)
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let md = fs::symlink_metadata(path)
        .map_err(|e| format!("read metadata '{}' failed: {e}", path.display()))?;
    if md.is_dir() {
        fs::remove_dir_all(path).map_err(|e| format!("remove '{}' failed: {e}", path.display()))?;
    } else {
        fs::remove_file(path).map_err(|e| format!("remove '{}' failed: {e}", path.display()))?;
    }
    Ok(())
}

fn create_dir_symlink(src: &Path, dst: &Path) -> Result<(), String> {
    remove_path_if_exists(dst)?;
    #[cfg(target_os = "windows")]
    {
        std::os::windows::fs::symlink_dir(src, dst).map_err(|e| {
            format!(
                "symlink '{}' -> '{}' failed: {e}",
                dst.display(),
                src.display()
            )
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::os::unix::fs::symlink(src, dst).map_err(|e| {
            format!(
                "symlink '{}' -> '{}' failed: {e}",
                dst.display(),
                src.display()
            )
        })
    }
}

fn sync_dir_link_first(src: &Path, dst: &Path, label: &str) -> Result<(), String> {
    if !src.exists() {
        fs::create_dir_all(src)
            .map_err(|e| format!("mkdir source '{}' for {} failed: {e}", src.display(), label))?;
    }
    match create_dir_symlink(src, dst) {
        Ok(()) => Ok(()),
        Err(link_err) => copy_dir_recursive(src, dst).map_err(|copy_err| {
            format!(
                "sync {} failed. symlink error: {}; copy fallback error: {}",
                label, link_err, copy_err
            )
        }),
    }
}

fn isolated_runtime_clone_excluded(rel_path: &str) -> bool {
    let normalized = rel_path.trim_matches('/').replace('\\', "/");
    if normalized.is_empty() {
        return false;
    }
    let lower = normalized.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "play_sessions.v1.json" | "play_sessions_active.v1.json"
    ) {
        return true;
    }
    if lower == "runtime" || lower.starts_with("runtime/") {
        return true;
    }
    if lower == "runtime_sessions" || lower.starts_with("runtime_sessions/") {
        return true;
    }
    if lower == "snapshots" || lower.starts_with("snapshots/") {
        return true;
    }
    if lower == "world_backups" || lower.starts_with("world_backups/") {
        return true;
    }
    if lower == "assets" || lower.starts_with("assets/") {
        return true;
    }
    if lower == "libraries" || lower.starts_with("libraries/") {
        return true;
    }
    if lower == "versions" || lower.starts_with("versions/") {
        return true;
    }
    if lower == "logs/launches" || lower.starts_with("logs/launches/") {
        return true;
    }
    false
}

fn clone_tree_recursive_with_exclusions(
    src_root: &Path,
    dst_root: &Path,
    rel_prefix: &str,
) -> Result<(), String> {
    let current = if rel_prefix.is_empty() {
        src_root.to_path_buf()
    } else {
        src_root.join(rel_prefix)
    };
    if !current.exists() {
        return Ok(());
    }
    let entries =
        fs::read_dir(&current).map_err(|e| format!("read '{}' failed: {e}", current.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read dir entry failed: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.trim().is_empty() {
            continue;
        }
        let rel = if rel_prefix.is_empty() {
            name.clone()
        } else {
            format!("{rel_prefix}/{name}")
        };
        if isolated_runtime_clone_excluded(&rel) {
            continue;
        }
        let src_path = src_root.join(&rel);
        let dst_path = dst_root.join(&rel);
        let meta = entry
            .metadata()
            .map_err(|e| format!("read metadata '{}' failed: {e}", src_path.display()))?;
        if meta.is_dir() {
            fs::create_dir_all(&dst_path)
                .map_err(|e| format!("mkdir '{}' failed: {e}", dst_path.display()))?;
            clone_tree_recursive_with_exclusions(src_root, dst_root, &rel)?;
        } else if meta.is_file() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir '{}' failed: {e}", parent.display()))?;
            }
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!(
                    "copy '{}' -> '{}' failed: {e}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn clone_instance_to_isolated_runtime(
    app_instance_dir: &Path,
    runtime_dir: &Path,
) -> Result<(), String> {
    remove_path_if_exists(runtime_dir)?;
    fs::create_dir_all(runtime_dir).map_err(|e| {
        format!(
            "mkdir isolated runtime '{}' failed: {e}",
            runtime_dir.display()
        )
    })?;
    clone_tree_recursive_with_exclusions(app_instance_dir, runtime_dir, "")?;
    ensure_instance_content_dirs(runtime_dir)?;
    Ok(())
}

fn runtime_session_active_marker_path(runtime_session_dir: &Path) -> PathBuf {
    runtime_session_dir.join(RUNTIME_SESSION_ACTIVE_MARKER_FILE)
}

fn write_runtime_session_active_marker(runtime_session_dir: &Path) -> Result<(), String> {
    let marker = runtime_session_active_marker_path(runtime_session_dir);
    fs::write(&marker, now_iso()).map_err(|e| {
        format!(
            "write runtime session active marker '{}' failed: {e}",
            marker.display()
        )
    })
}

fn cleanup_stale_runtime_sessions_for_instance(
    instance_dir: &Path,
    max_age: Duration,
) -> Result<usize, String> {
    let sessions_root = instance_dir.join("runtime_sessions");
    if !sessions_root.exists() || !sessions_root.is_dir() {
        return Ok(0);
    }
    let mut removed = 0usize;
    let now = std::time::SystemTime::now();
    let entries = fs::read_dir(&sessions_root).map_err(|e| {
        format!(
            "read runtime sessions '{}' failed: {e}",
            sessions_root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read runtime session entry failed: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let marker = runtime_session_active_marker_path(&path);
        if marker.exists() {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let age = now
            .duration_since(modified)
            .unwrap_or_else(|_| Duration::from_secs(0));
        if age < max_age {
            continue;
        }
        remove_path_if_exists(&path)?;
        removed += 1;
    }
    Ok(removed)
}

fn cleanup_stale_runtime_sessions_startup(app: &tauri::AppHandle) -> Result<usize, String> {
    let instances_dir = app_instances_dir(app)?;
    let idx = read_index(&instances_dir)?;
    let mut removed = 0usize;
    let max_age = Duration::from_secs(STALE_RUNTIME_SESSION_MAX_AGE_HOURS * 3600);
    for instance in idx.instances {
        let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
        if !instance_dir.exists() {
            continue;
        }
        removed += cleanup_stale_runtime_sessions_for_instance(&instance_dir, max_age)?;
    }
    Ok(removed)
}

fn sync_instance_runtime_content_isolated(
    app_instance_dir: &Path,
    runtime_dir: &Path,
) -> Result<(), String> {
    // Concurrent sessions are disposable full clones of the instance at launch time.
    clone_instance_to_isolated_runtime(app_instance_dir, runtime_dir)?;
    write_runtime_session_active_marker(runtime_dir)?;
    Ok(())
}

fn sync_prism_instance_content(app_instance_dir: &Path, prism_mc_dir: &Path) -> Result<(), String> {
    let source_mods = app_instance_dir.join("mods");
    let source_config = app_instance_dir.join("config");
    let source_resourcepacks = app_instance_dir.join("resourcepacks");
    let source_shaderpacks = app_instance_dir.join("shaderpacks");
    let source_saves = app_instance_dir.join("saves");
    let target_mods = prism_mc_dir.join("mods");
    let target_config = prism_mc_dir.join("config");
    let target_resourcepacks = prism_mc_dir.join("resourcepacks");
    let target_shaderpacks = prism_mc_dir.join("shaderpacks");
    let target_saves = prism_mc_dir.join("saves");

    sync_dir_link_first(&source_mods, &target_mods, "prism mods")?;
    sync_dir_link_first(&source_config, &target_config, "prism config")?;
    sync_dir_link_first(
        &source_resourcepacks,
        &target_resourcepacks,
        "prism resourcepacks",
    )?;
    sync_dir_link_first(
        &source_shaderpacks,
        &target_shaderpacks,
        "prism shaderpacks",
    )?;
    sync_dir_link_first(&source_saves, &target_saves, "prism saves")?;
    for filename in [
        "options.txt",
        "optionsof.txt",
        "optionsshaders.txt",
        "servers.dat",
    ] {
        let root = app_instance_dir.join(filename);
        let dot_mc = app_instance_dir.join(".minecraft").join(filename);
        let source_file = match (root.is_file(), dot_mc.is_file()) {
            (true, true) => {
                let root_time = fs::metadata(&root)
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                let dot_time = fs::metadata(&dot_mc)
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                if dot_time > root_time {
                    dot_mc
                } else {
                    root
                }
            }
            (true, false) => root,
            (false, true) => dot_mc,
            (false, false) => continue,
        };
        let _ = copy_file_if_exists(&source_file, &prism_mc_dir.join(filename))?;
    }
    Ok(())
}

fn effective_jvm_args(raw: &str) -> Vec<String> {
    let explicit: Vec<String> = raw
        .split_whitespace()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .collect();
    if !explicit.is_empty() {
        return explicit;
    }
    vec![
        "-XX:+UseG1GC".to_string(),
        "-XX:+ParallelRefProcEnabled".to_string(),
        "-XX:+UseStringDeduplication".to_string(),
        "-XX:MaxGCPauseMillis=200".to_string(),
    ]
}

fn wire_shared_cache(cache_dir: &Path, runtime_dir: &Path) -> Result<(), String> {
    for seg in ["assets", "libraries", "versions"] {
        let shared = cache_dir.join(seg);
        let local = runtime_dir.join(seg);
        fs::create_dir_all(&shared)
            .map_err(|e| format!("mkdir shared cache '{}' failed: {e}", shared.display()))?;
        if local.exists() {
            continue;
        }
        if create_dir_symlink(&shared, &local).is_err() {
            // Best-effort fallback where symlinks are unavailable.
            copy_dir_recursive(&shared, &local)?;
        }
    }
    Ok(())
}

fn resolve_java_executable(settings: &LauncherSettings) -> Result<String, String> {
    if !settings.java_path.trim().is_empty() {
        let p = PathBuf::from(settings.java_path.trim());
        if !p.exists() {
            return Err(format!(
                "Configured Java path does not exist: {}",
                settings.java_path
            ));
        }
        return Ok(p.display().to_string());
    }

    if let Ok(env_java) = std::env::var("MPM_JAVA_PATH") {
        let p = PathBuf::from(env_java.trim());
        if p.exists() {
            return Ok(p.display().to_string());
        }
    }

    match Command::new("java").arg("-version").output() {
        Ok(_) => Ok("java".to_string()),
        Err(e) => Err(format!(
            "Java not found. Set Java path in Settings > Launcher. ({e})"
        )),
    }
}

fn parse_java_major(version_text: &str) -> Option<u32> {
    let mut candidate = String::new();
    if let Some(start) = version_text.find('"') {
        let rest = &version_text[start + 1..];
        if let Some(end) = rest.find('"') {
            candidate = rest[..end].trim().to_string();
        }
    }
    if candidate.is_empty() {
        candidate = version_text.trim().to_string();
    }
    if candidate.is_empty() {
        return None;
    }
    let parts: Vec<&str> = candidate.split('.').collect();
    if parts.first().copied() == Some("1") {
        return parts.get(1).and_then(|p| p.parse::<u32>().ok());
    }
    parts.first().and_then(|p| p.parse::<u32>().ok())
}

fn detect_java_major(java_executable: &str) -> Result<(u32, String), String> {
    let output = Command::new(java_executable)
        .arg("-version")
        .output()
        .map_err(|e| format!("failed to run `{java_executable} -version`: {e}"))?;
    let stderr_text = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let combined = if !stderr_text.is_empty() {
        stderr_text
    } else {
        stdout_text
    };
    if combined.is_empty() {
        return Err(format!(
            "`{java_executable} -version` returned no output. Set a valid Java path in Settings."
        ));
    }
    let first_line = combined
        .lines()
        .next()
        .unwrap_or(&combined)
        .trim()
        .to_string();
    let major = parse_java_major(&first_line)
        .ok_or_else(|| format!("could not parse Java version from: {first_line}"))?;
    Ok((major, first_line))
}

fn maybe_add_java_candidate(path: PathBuf, out: &mut HashMap<String, JavaRuntimeCandidate>) {
    if !path.exists() || !path.is_file() {
        return;
    }
    let resolved = fs::canonicalize(&path).unwrap_or(path);
    let key = resolved.display().to_string();
    if out.contains_key(&key) {
        return;
    }
    if let Ok((major, version_line)) = detect_java_major(&key) {
        out.insert(
            key.clone(),
            JavaRuntimeCandidate {
                path: key,
                major,
                version_line,
            },
        );
    }
}

fn detect_java_runtimes_inner() -> Vec<JavaRuntimeCandidate> {
    let mut map: HashMap<String, JavaRuntimeCandidate> = HashMap::new();

    if let Ok(v) = std::env::var("MPM_JAVA_PATH") {
        maybe_add_java_candidate(PathBuf::from(v.trim()), &mut map);
    }
    if let Ok(v) = std::env::var("JAVA_HOME") {
        let home = PathBuf::from(v.trim());
        if cfg!(target_os = "windows") {
            maybe_add_java_candidate(home.join("bin").join("java.exe"), &mut map);
        } else {
            maybe_add_java_candidate(home.join("bin").join("java"), &mut map);
        }
    }

    if cfg!(target_os = "windows") {
        if let Ok(output) = Command::new("where").arg("java").output() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let p = PathBuf::from(line.trim());
                maybe_add_java_candidate(p, &mut map);
            }
        }
    } else {
        if let Ok(output) = Command::new("which").arg("java").output() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let p = PathBuf::from(line.trim());
                maybe_add_java_candidate(p, &mut map);
            }
        }
        for path in [
            "/usr/local/bin/java",
            "/usr/bin/java",
            "/opt/homebrew/bin/java",
            "/usr/local/opt/openjdk/bin/java",
            "/opt/homebrew/opt/openjdk/bin/java",
            "/usr/local/opt/openjdk@21/bin/java",
            "/opt/homebrew/opt/openjdk@21/bin/java",
            "/usr/local/opt/openjdk@17/bin/java",
            "/opt/homebrew/opt/openjdk@17/bin/java",
        ] {
            maybe_add_java_candidate(PathBuf::from(path), &mut map);
        }

        if let Some(home) = home_dir() {
            let sdkman_root = home.join(".sdkman").join("candidates").join("java");
            if let Ok(entries) = fs::read_dir(sdkman_root) {
                for ent in entries.flatten() {
                    maybe_add_java_candidate(ent.path().join("bin").join("java"), &mut map);
                }
            }
            let asdf_root = home.join(".asdf").join("installs").join("java");
            if let Ok(entries) = fs::read_dir(asdf_root) {
                for ent in entries.flatten() {
                    maybe_add_java_candidate(ent.path().join("bin").join("java"), &mut map);
                }
            }
        }
    }

    if cfg!(target_os = "macos") {
        maybe_add_java_candidate(PathBuf::from("/usr/bin/java"), &mut map);
        let vm_root = PathBuf::from("/Library/Java/JavaVirtualMachines");
        if let Ok(entries) = fs::read_dir(vm_root) {
            for ent in entries.flatten() {
                let p = ent
                    .path()
                    .join("Contents")
                    .join("Home")
                    .join("bin")
                    .join("java");
                maybe_add_java_candidate(p, &mut map);
            }
        }
        let user_vm_root =
            home_dir().map(|h| h.join("Library").join("Java").join("JavaVirtualMachines"));
        if let Some(vm_root) = user_vm_root {
            if let Ok(entries) = fs::read_dir(vm_root) {
                for ent in entries.flatten() {
                    let p = ent
                        .path()
                        .join("Contents")
                        .join("Home")
                        .join("bin")
                        .join("java");
                    maybe_add_java_candidate(p, &mut map);
                }
            }
        }

        if let Ok(output) = Command::new("/usr/libexec/java_home").arg("-V").output() {
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            for line in text.lines() {
                if let Some(start) = line.find('/') {
                    let candidate_home = line[start..].trim();
                    if !candidate_home.is_empty() {
                        let java_bin = PathBuf::from(candidate_home).join("bin").join("java");
                        maybe_add_java_candidate(java_bin, &mut map);
                    }
                }
            }
        }

        for version_hint in ["21", "17", "8"] {
            if let Ok(output) = Command::new("/usr/libexec/java_home")
                .arg("-v")
                .arg(version_hint)
                .output()
            {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !text.is_empty() {
                    maybe_add_java_candidate(
                        PathBuf::from(text).join("bin").join("java"),
                        &mut map,
                    );
                }
            }
        }

        for brew_opt in ["/opt/homebrew/opt", "/usr/local/opt"] {
            if let Ok(entries) = fs::read_dir(brew_opt) {
                for ent in entries.flatten() {
                    let name = ent.file_name().to_string_lossy().to_lowercase();
                    if !name.starts_with("openjdk") {
                        continue;
                    }
                    maybe_add_java_candidate(ent.path().join("bin").join("java"), &mut map);
                }
            }
        }
    }

    let mut out: Vec<JavaRuntimeCandidate> = map.into_values().collect();
    out.sort_by(|a, b| {
        b.major
            .cmp(&a.major)
            .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
    });
    out
}

fn parse_mc_release_triplet(version: &str) -> Option<(u32, u32, u32)> {
    let trimmed = version.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let mut parts = trimmed.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = parts
        .next()
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);
    Some((major, minor, patch))
}

fn required_java_major_for_mc(mc_version: &str) -> u32 {
    if let Some((major, minor, patch)) = parse_mc_release_triplet(mc_version) {
        if major > 1 {
            return 21;
        }
        if minor > 20 || (minor == 20 && patch >= 5) {
            return 21;
        }
        if minor >= 18 {
            return 17;
        }
        if minor >= 17 {
            return 16;
        }
        return 8;
    }
    // Unknown/non-release version format: choose a safe modern baseline.
    17
}

fn tail_lines_from_file(path: &Path, max_lines: usize) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len().saturating_sub(max_lines));
    }
    let joined = lines.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn resolve_native_loader(
    client: &Client,
    instance: &Instance,
) -> Result<(Option<String>, Option<String>), String> {
    let loader = instance.loader.to_lowercase();
    match loader.as_str() {
        "vanilla" => Ok((None, None)),
        "fabric" => {
            let version = resolve_fabric_loader_version(client, &instance.mc_version)?;
            Ok((Some("fabric".to_string()), Some(version)))
        }
        "forge" => {
            let version = resolve_forge_loader_version(client, &instance.mc_version)?;
            Ok((Some("forge".to_string()), Some(version)))
        }
        other => Err(format!(
            "Native launch currently supports vanilla/fabric/forge. '{}' is not supported yet.",
            other
        )),
    }
}

fn upsert_launcher_account(
    app: &tauri::AppHandle,
    account: &LauncherAccount,
) -> Result<(), String> {
    let mut accounts = read_launcher_accounts(app)?;
    accounts.retain(|a| a.id != account.id);
    accounts.push(account.clone());
    accounts.sort_by(|a, b| a.username.to_lowercase().cmp(&b.username.to_lowercase()));
    write_launcher_accounts(app, &accounts)
}

fn launch_prism_instance(
    prism_root: &Path,
    prism_instance_id: &str,
    quick_play_host: Option<&str>,
    quick_play_port: Option<u16>,
) -> Result<(), String> {
    let mut attempts: Vec<(OsString, Vec<OsString>)> = Vec::new();
    let root = OsString::from(prism_root.as_os_str());
    let launch_arg = OsString::from(prism_instance_id);
    let quick_play_args = if let Some(host) = quick_play_host {
        let mut args = vec![OsString::from("--server"), OsString::from(host)];
        if let Some(port) = quick_play_port {
            args.push(OsString::from("--port"));
            args.push(OsString::from(port.to_string()));
        }
        args
    } else {
        vec![]
    };

    let build_prism_args = || {
        let mut args = vec![
            OsString::from("--dir"),
            root.clone(),
            OsString::from("--launch"),
            launch_arg.clone(),
        ];
        args.extend(quick_play_args.clone());
        args
    };

    if let Ok(bin) = std::env::var("MPM_PRISM_BIN") {
        let trimmed = bin.trim();
        if !trimmed.is_empty() {
            attempts.push((OsString::from(trimmed), build_prism_args()));
        }
    }

    if cfg!(target_os = "macos") {
        attempts.push((
            OsString::from("/Applications/Prism Launcher.app/Contents/MacOS/prismlauncher"),
            build_prism_args(),
        ));
        attempts.push((
            OsString::from("/Applications/Prism Launcher.app/Contents/MacOS/PrismLauncher"),
            build_prism_args(),
        ));
        let mut open_args = vec![
            OsString::from("-a"),
            OsString::from("Prism Launcher"),
            OsString::from("--args"),
        ];
        open_args.extend(build_prism_args());
        attempts.push((OsString::from("open"), open_args));
    } else if cfg!(target_os = "windows") {
        attempts.push((OsString::from("prismlauncher.exe"), build_prism_args()));
        attempts.push((OsString::from("PrismLauncher.exe"), build_prism_args()));
        attempts.push((OsString::from("prismlauncher"), build_prism_args()));
        for env_key in ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"] {
            if let Some(base) = std::env::var_os(env_key) {
                let base = PathBuf::from(base);
                for exe in ["prismlauncher.exe", "PrismLauncher.exe"] {
                    attempts.push((
                        base.join("Programs")
                            .join("PrismLauncher")
                            .join(exe)
                            .into_os_string(),
                        build_prism_args(),
                    ));
                    attempts.push((
                        base.join("PrismLauncher").join(exe).into_os_string(),
                        build_prism_args(),
                    ));
                }
            }
        }
    } else {
        attempts.push((OsString::from("prismlauncher"), build_prism_args()));
        attempts.push((OsString::from("PrismLauncher"), build_prism_args()));
        let mut flatpak_args = vec![
            OsString::from("run"),
            OsString::from("org.prismlauncher.PrismLauncher"),
        ];
        flatpak_args.extend(build_prism_args());
        attempts.push((OsString::from("flatpak"), flatpak_args));
    }

    let mut errs: Vec<String> = Vec::new();
    for (bin, args) in attempts {
        let mut cmd = Command::new(&bin);
        cmd.args(&args);
        match cmd.spawn() {
            Ok(_) => return Ok(()),
            Err(e) => errs.push(format!("{}: {e}", PathBuf::from(&bin).display())),
        }
    }

    Err(format!(
        "Failed to launch Prism Launcher. {}",
        errs.join(" | ")
    ))
}

fn default_export_filename(instance_name: &str) -> String {
    let date = Local::now().format("%Y-%m-%d").to_string();
    let base = sanitize_filename(&instance_name.replace(' ', "-"));
    let clean = if base.is_empty() {
        "instance".to_string()
    } else {
        base
    };
    format!("{clean}-mods-{date}.zip")
}

fn build_selected_microsoft_auth(
    app: &tauri::AppHandle,
    client: &Client,
    settings: &LauncherSettings,
) -> Result<(LauncherAccount, String), String> {
    let selected_id = settings.selected_account_id.clone().ok_or_else(|| {
        "No Microsoft account selected. Connect one in Settings > Launcher.".to_string()
    })?;
    let mut accounts = read_launcher_accounts(app)?;
    let mut account = accounts
        .iter()
        .find(|a| a.id == selected_id)
        .cloned()
        .ok_or_else(|| {
            "Selected Microsoft account no longer exists. Reconnect account.".to_string()
        })?;
    let client_id = resolve_oauth_client_id(app)?;
    let mut refresh = keyring_get_refresh_token_for_account(app, &account, &accounts);
    if let Err(err) = &refresh {
        if err.starts_with("No refresh token found in secure storage")
            || err.starts_with("Multiple secure refresh tokens were found")
        {
            if let Some(repaired) =
                maybe_repair_selected_account_with_available_token(app, &account, &accounts)?
            {
                account = repaired;
                refresh = keyring_get_refresh_token_for_account(app, &account, &accounts);
            }
        }
    }
    let refresh = refresh?;
    let old_account_id = account.id.clone();
    let refreshed = microsoft_refresh_access_token(client, &client_id, &refresh)?;
    if let Some(new_refresh) = refreshed.refresh_token.as_ref() {
        persist_refresh_token_for_launcher_account_with_app(app, &account, new_refresh)?;
        persist_refresh_token(app, &old_account_id, new_refresh)?;
    }
    let mc_access = microsoft_access_to_mc_token(client, &refreshed.access_token)?;
    ensure_minecraft_entitlement(client, &mc_access)?;
    let profile = fetch_minecraft_profile(client, &mc_access)?;
    let token_for_new_id = refreshed.refresh_token.as_ref().unwrap_or(&refresh);
    account.id = profile.id;
    if account.id != old_account_id {
        if let Err(e) = persist_refresh_token(app, &account.id, token_for_new_id) {
            eprintln!(
                "refresh token copy to updated account id failed ({} -> {}): {}",
                old_account_id, account.id, e
            );
        }
        let mut settings = read_launcher_settings(app)?;
        settings.selected_account_id = Some(account.id.clone());
        write_launcher_settings(app, &settings)?;
    }
    account.username = profile.name;
    persist_refresh_token_for_launcher_account_with_app(app, &account, token_for_new_id)?;
    upsert_launcher_account(app, &account)?;
    accounts.retain(|a| a.id != old_account_id && a.id != account.id);
    accounts.push(account.clone());
    write_launcher_accounts(app, &accounts)?;
    Ok((account, mc_access))
}

fn resolve_native_auth_and_loader(
    app: &tauri::AppHandle,
    settings: &LauncherSettings,
    instance: &Instance,
) -> Result<(LauncherAccount, String, Option<String>, Option<String>), String> {
    let client = build_http_client()?;
    let (account, mc_access_token) = build_selected_microsoft_auth(app, &client, settings)?;
    let (loader, loader_version) = resolve_native_loader(&client, instance)?;
    Ok((account, mc_access_token, loader, loader_version))
}

fn main() {
    tauri::Builder::default()
        .menu(build_main_menu("OpenJar Launcher"))
        .on_menu_event(|event| {
            if event.menu_item_id() != MENU_CHECK_FOR_UPDATES_ID {
                return;
            }
            let app = event.window().app_handle();
            if let Some(window) = app.get_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            let _ = app.emit_all(APP_MENU_CHECK_FOR_UPDATES_EVENT, ());
        })
        .manage(AppState::default())
        .setup(|app| {
            load_dev_curseforge_key_into_runtime_env(&app.handle());
            if let Err(err) = migrate_legacy_refresh_tokens_to_keyring(&app.handle()) {
                eprintln!("legacy refresh-token migration warning: {err}");
            }
            if let Err(err) = migrate_selected_refresh_alias(&app.handle()) {
                eprintln!("selected refresh-token alias migration warning: {err}");
            }
            match cleanup_stale_runtime_sessions_startup(&app.handle()) {
                Ok(removed) if removed > 0 => {
                    eprintln!("startup cleanup removed {removed} stale runtime session folder(s)");
                }
                Ok(_) => {}
                Err(err) => {
                    eprintln!("startup runtime session cleanup warning: {err}");
                }
            }
            if let Err(err) = recover_native_play_sessions_startup(&app.handle()) {
                eprintln!("startup play session recovery warning: {err}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_instances,
            commands::create_instance,
            commands::create_instance_from_modpack_file,
            commands::list_launcher_import_sources,
            commands::import_instance_from_launcher,
            commands::update_instance,
            commands::set_instance_icon,
            commands::read_local_image_data_url,
            commands::detect_java_runtimes,
            commands::delete_instance,
            commands::search_discover_content,
            commands::install_modrinth_mod,
            commands::install_curseforge_mod,
            commands::preview_modrinth_install,
            commands::check_instance_content_updates,
            commands::update_all_instance_content,
            commands::check_modrinth_updates,
            commands::update_all_modrinth_mods,
            commands::import_local_mod_file,
            commands::resolve_local_mod_sources,
            commands::prune_missing_installed_entries,
            commands::list_installed_mods,
            commands::set_installed_mod_enabled,
            commands::set_installed_mod_pin,
            commands::set_installed_mod_provider,
            commands::attach_installed_mod_github_repo,
            commands::remove_installed_mod,
            commands::trigger_instance_microphone_permission_prompt,
            commands::open_microphone_system_settings,
            commands::preflight_launch_compatibility,
            commands::launch_instance,
            commands::get_launcher_settings,
            commands::get_dev_mode_state,
            commands::set_dev_curseforge_api_key,
            commands::clear_dev_curseforge_api_key,
            commands::get_curseforge_api_status,
            commands::get_github_token_pool_status,
            commands::set_github_token_pool,
            commands::clear_github_token_pool,
            commands::set_launcher_settings,
            commands::list_launcher_accounts,
            commands::select_launcher_account,
            commands::logout_microsoft_account,
            commands::begin_microsoft_login,
            commands::poll_microsoft_login,
            commands::list_running_instances,
            commands::stop_running_instance,
            commands::cancel_instance_launch,
            commands::list_instance_snapshots,
            commands::list_instance_worlds,
            commands::get_instance_disk_usage,
            commands::get_instance_playtime,
            commands::get_instance_last_run_metadata,
            commands::get_instance_last_run_report,
            commands::list_instance_run_reports,
            commands::list_instance_history_events,
            commands::reset_instance_config_files_with_backup,
            commands::list_world_config_files,
            commands::read_world_config_file,
            commands::write_world_config_file,
            commands::rollback_instance,
            commands::rollback_instance_world_backup,
            commands::read_instance_logs,
            commands::install_discover_content,
            commands::preview_preset_apply,
            commands::apply_preset_to_instance,
            modpack::list_modpack_specs,
            modpack::get_modpack_spec,
            modpack::upsert_modpack_spec,
            modpack::duplicate_modpack_spec,
            modpack::delete_modpack_spec,
            modpack::import_modpack_spec_json,
            modpack::export_modpack_spec_json,
            modpack::import_modpack_layer_from_provider,
            modpack::import_modpack_layer_from_spec,
            modpack::import_local_jars_to_modpack_layer,
            modpack::preview_template_layer_update,
            modpack::apply_template_layer_update,
            modpack::resolve_local_modpack_entries,
            modpack::resolve_modpack_for_instance,
            modpack::apply_modpack_plan,
            modpack::get_instance_modpack_status,
            modpack::detect_instance_modpack_drift,
            modpack::realign_instance_to_modpack,
            modpack::preview_update_modpack_from_instance,
            modpack::apply_update_modpack_from_instance,
            modpack::rollback_instance_to_last_modpack_snapshot,
            modpack::migrate_legacy_creator_presets,
            modpack::seed_dev_modpack_data,
            friend_link::create_friend_link_session,
            friend_link::join_friend_link_session,
            friend_link::leave_friend_link_session,
            friend_link::get_friend_link_status,
            friend_link::set_friend_link_allowlist,
            friend_link::set_friend_link_guardrails,
            friend_link::set_friend_link_peer_alias,
            friend_link::preview_friend_link_drift,
            friend_link::sync_friend_link_selected,
            friend_link::reconcile_friend_link,
            friend_link::resolve_friend_link_conflicts,
            friend_link::export_friend_link_debug_bundle,
            friend_link::list_instance_config_files,
            friend_link::read_instance_config_file,
            friend_link::write_instance_config_file,
            friend_link::list_instance_config_file_backups,
            friend_link::restore_instance_config_file_backup,
            commands::get_curseforge_project_detail,
            commands::get_github_project_detail,
            commands::import_provider_modpack_template,
            commands::export_presets_json,
            commands::import_presets_json,
            commands::get_selected_account_diagnostics,
            commands::apply_selected_account_appearance,
            commands::open_instance_path,
            commands::reveal_config_editor_file,
            commands::list_quick_play_servers,
            commands::upsert_quick_play_server,
            commands::remove_quick_play_server,
            commands::launch_quick_play_server,
            commands::export_instance_mods_zip,
            commands::export_instance_support_bundle
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod content_compatibility_tests {
    use super::*;

    fn make_instance(loader: &str, mc_version: &str) -> Instance {
        Instance {
            id: "inst_test".to_string(),
            name: "Test".to_string(),
            folder_name: None,
            mc_version: mc_version.to_string(),
            loader: loader.to_string(),
            created_at: "now".to_string(),
            icon_path: None,
            settings: InstanceSettings::default(),
        }
    }

    fn make_cf_file(game_versions: Vec<&str>) -> CurseforgeFile {
        CurseforgeFile {
            id: 1,
            mod_id: 1,
            display_name: "file".to_string(),
            file_name: "file.zip".to_string(),
            file_date: "2026-01-01T00:00:00Z".to_string(),
            download_url: None,
            game_versions: game_versions.into_iter().map(str::to_string).collect(),
            hashes: vec![],
            dependencies: vec![],
        }
    }

    fn make_modrinth_version(game_versions: Vec<&str>, loaders: Vec<&str>) -> ModrinthVersion {
        ModrinthVersion {
            project_id: "project".to_string(),
            id: "ver".to_string(),
            version_number: "1.0.0".to_string(),
            name: None,
            game_versions: game_versions.into_iter().map(str::to_string).collect(),
            loaders: loaders.into_iter().map(str::to_string).collect(),
            date_published: "2026-01-01T00:00:00Z".to_string(),
            dependencies: vec![],
            files: vec![ModrinthVersionFile {
                url: "https://example.com/file.zip".to_string(),
                filename: "file.zip".to_string(),
                primary: Some(true),
                hashes: HashMap::new(),
            }],
        }
    }

    fn make_cf_project(
        class_id: i64,
        slug: &str,
        categories: Vec<CurseforgeCategory>,
    ) -> CurseforgeMod {
        CurseforgeMod {
            id: 12345,
            class_id,
            name: "Project".to_string(),
            slug: Some(slug.to_string()),
            summary: String::new(),
            download_count: 0.0,
            date_modified: String::new(),
            authors: vec![],
            categories,
            logo: None,
        }
    }

    #[test]
    fn non_mod_curseforge_compatibility_ignores_loader_tags() {
        let instance = make_instance("fabric", "1.20.1");
        let file = make_cf_file(vec!["1.20.1", "forge"]);
        assert!(file_looks_compatible_with_instance(
            &file,
            &instance,
            "resourcepacks"
        ));
        assert!(!file_looks_compatible_with_instance(
            &file, &instance, "mods"
        ));
    }

    #[test]
    fn non_mod_curseforge_compatibility_allows_patch_level_fallback() {
        let instance = make_instance("fabric", "1.21.11");
        let file = make_cf_file(vec!["1.21.1", "forge"]);
        assert!(file_looks_compatible_with_instance(
            &file,
            &instance,
            "resourcepacks"
        ));
    }

    #[test]
    fn mod_curseforge_compatibility_keeps_patch_strict() {
        let instance = make_instance("fabric", "1.21.11");
        let file = make_cf_file(vec!["1.21.1", "fabric"]);
        assert!(!file_looks_compatible_with_instance(
            &file, &instance, "mods"
        ));
    }

    #[test]
    fn non_mod_modrinth_selection_ignores_loader_mismatch() {
        let instance = make_instance("fabric", "1.20.1");
        let versions = vec![make_modrinth_version(vec!["1.20.1"], vec!["forge"])];
        assert!(pick_compatible_version_for_content(versions, &instance, "shaderpacks").is_some());
    }

    #[test]
    fn non_mod_modrinth_selection_allows_patch_level_fallback() {
        let instance = make_instance("fabric", "1.21.11");
        let versions = vec![make_modrinth_version(vec!["1.21.1"], vec!["forge"])];
        assert!(pick_compatible_version_for_content(versions, &instance, "shaderpacks").is_some());
    }

    #[test]
    fn mod_modrinth_selection_still_requires_loader_match() {
        let instance = make_instance("fabric", "1.20.1");
        let versions = vec![make_modrinth_version(vec!["1.20.1"], vec!["forge"])];
        assert!(pick_compatible_version_for_content(versions, &instance, "mods").is_none());
    }

    #[test]
    fn normalize_update_content_type_filter_accepts_supported_aliases() {
        let requested = vec![
            "shaders".to_string(),
            "resourcepacks".to_string(),
            "mods".to_string(),
        ];
        let filter = normalize_update_content_type_filter(Some(&requested))
            .expect("filter should include supported content types");
        assert!(filter.contains("shaderpacks"));
        assert!(filter.contains("resourcepacks"));
        assert!(filter.contains("mods"));
    }

    #[test]
    fn normalize_update_content_type_filter_ignores_unsupported_values() {
        let requested = vec!["modpacks".to_string(), "unknown".to_string()];
        assert!(normalize_update_content_type_filter(Some(&requested)).is_none());
    }

    #[test]
    fn curseforge_resourcepack_slug_uses_texture_packs_url_path() {
        let url =
            curseforge_external_project_url("12345", Some("fresh-animations"), "resourcepacks");
        assert_eq!(
            url,
            "https://www.curseforge.com/minecraft/texture-packs/fresh-animations"
        );
    }

    #[test]
    fn infer_curseforge_class_12_without_shader_category_defaults_to_resourcepacks() {
        let project = make_cf_project(
            12,
            "fresh-animations",
            vec![CurseforgeCategory {
                name: "Resource Packs".to_string(),
                slug: Some("resource-packs".to_string()),
            }],
        );
        let inferred = infer_curseforge_project_content_type(&project, None);
        assert_eq!(inferred, "resourcepacks");
        let url = curseforge_external_project_url("12345", project.slug.as_deref(), &inferred);
        assert_eq!(
            url,
            "https://www.curseforge.com/minecraft/texture-packs/fresh-animations"
        );
    }

    #[test]
    fn infer_curseforge_class_12_shader_category_uses_shaders_path() {
        let project = make_cf_project(
            12,
            "complementary-shaders",
            vec![CurseforgeCategory {
                name: "Shaders".to_string(),
                slug: Some("shaders".to_string()),
            }],
        );
        let inferred = infer_curseforge_project_content_type(&project, None);
        assert_eq!(inferred, "shaderpacks");
        let url = curseforge_external_project_url("12345", project.slug.as_deref(), &inferred);
        assert_eq!(
            url,
            "https://www.curseforge.com/minecraft/shaders/complementary-shaders"
        );
    }

    #[test]
    fn local_loader_guard_blocks_fabric_and_forge_mismatch_both_directions() {
        assert!(!instance_loader_accepts_mod_loader("fabric", "forge"));
        assert!(!instance_loader_accepts_mod_loader("forge", "fabric"));
    }

    #[test]
    fn local_loader_guard_allows_quilt_instance_to_accept_fabric_mods() {
        assert!(instance_loader_accepts_mod_loader("quilt", "fabric"));
        assert!(!instance_loader_accepts_mod_loader("fabric", "quilt"));
    }

    #[test]
    fn local_loader_guard_keeps_neoforge_and_forge_distinct() {
        assert!(!instance_loader_accepts_mod_loader("neoforge", "forge"));
        assert!(!instance_loader_accepts_mod_loader("forge", "neoforge"));
        assert!(instance_loader_accepts_mod_loader("neoforge", "neoforge"));
    }

    #[test]
    fn local_loader_guard_allows_forge_family_hint_for_forge_variants() {
        assert!(instance_loader_accepts_mod_loader("forge", "forge_family"));
        assert!(instance_loader_accepts_mod_loader(
            "neoforge",
            "forge_family"
        ));
        assert!(!instance_loader_accepts_mod_loader(
            "fabric",
            "forge_family"
        ));
    }
}

#[cfg(test)]
mod discover_ranking_tests {
    use super::*;

    fn make_hit(source: &str, project_id: &str) -> DiscoverSearchHit {
        DiscoverSearchHit {
            source: source.to_string(),
            project_id: project_id.to_string(),
            title: project_id.to_string(),
            description: "".to_string(),
            author: "".to_string(),
            downloads: 0,
            follows: 0,
            icon_url: None,
            categories: vec![],
            versions: vec![],
            date_modified: "".to_string(),
            content_type: "mods".to_string(),
            slug: None,
            external_url: None,
            confidence: None,
            reason: None,
            install_supported: None,
            install_note: None,
        }
    }

    fn sample_discover_repo() -> GithubRepository {
        GithubRepository {
            full_name: "etianl/Trouser-Streak".to_string(),
            name: "Trouser-Streak".to_string(),
            description: Some("Meteor addon with mods for chunk tracing.".to_string()),
            stargazers_count: 500,
            forks_count: 12,
            archived: false,
            fork: false,
            disabled: false,
            html_url: "https://github.com/etianl/Trouser-Streak".to_string(),
            homepage: None,
            watchers_count: 500,
            open_issues_count: 1,
            pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
            updated_at: Some("2026-03-01T00:00:00Z".to_string()),
            topics: vec![],
            default_branch: "main".to_string(),
            owner: GithubOwner {
                login: "etianl".to_string(),
                owner_type: "User".to_string(),
            },
        }
    }

    fn sample_non_minecraft_ml_repo() -> GithubRepository {
        GithubRepository {
            full_name: "hwaluskle/tensorflow-generative-model-collections".to_string(),
            name: "tensorflow-generative-model-collections".to_string(),
            description: Some("Collection of generative models in Tensorflow.".to_string()),
            stargazers_count: 3900,
            forks_count: 840,
            archived: false,
            fork: false,
            disabled: false,
            html_url: "https://github.com/hwaluskle/tensorflow-generative-model-collections"
                .to_string(),
            homepage: None,
            watchers_count: 3900,
            open_issues_count: 1,
            pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
            updated_at: Some("2026-03-01T00:00:00Z".to_string()),
            topics: vec![
                "tensorflow".to_string(),
                "model".to_string(),
                "gan".to_string(),
            ],
            default_branch: "main".to_string(),
            owner: GithubOwner {
                login: "hwaluskle".to_string(),
                owner_type: "User".to_string(),
            },
        }
    }

    #[test]
    fn blend_discover_hits_prefers_modrinth_but_keeps_other_provider_visible() {
        let input = vec![
            make_hit("curseforge", "cf_1"),
            make_hit("modrinth", "mr_1"),
            make_hit("curseforge", "cf_2"),
            make_hit("modrinth", "mr_2"),
            make_hit("modrinth", "mr_3"),
        ];
        let blended = blend_discover_hits_prefer_modrinth(input);
        let order = blended
            .iter()
            .map(|hit| hit.project_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["mr_1", "mr_2", "cf_1", "mr_3", "cf_2"]);
    }

    #[test]
    fn blend_discover_hits_passthrough_when_single_provider_present() {
        let input = vec![make_hit("modrinth", "mr_1"), make_hit("modrinth", "mr_2")];
        let blended = blend_discover_hits_prefer_modrinth(input);
        let order = blended
            .iter()
            .map(|hit| hit.project_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["mr_1", "mr_2"]);
    }

    #[test]
    fn github_similarity_score_tolerates_common_typos() {
        let exact = github_name_similarity_score("meteor client", "meteor client");
        let typo = github_name_similarity_score("meteor client", "metor clint");
        assert!(exact >= typo);
        assert!(typo >= 20);
    }

    #[test]
    fn discover_query_variants_include_shortened_form() {
        let variants = discover_query_variants("meteor client hacks");
        assert!(!variants.is_empty());
        assert!(variants.iter().any(|value| value.contains("meteor")));
    }

    #[test]
    fn sort_discover_hits_prefers_relevance_by_default() {
        let mut hits = vec![
            DiscoverSearchHit {
                project_id: "a".to_string(),
                title: "random utility".to_string(),
                downloads: 9000,
                ..make_hit("modrinth", "a")
            },
            DiscoverSearchHit {
                project_id: "b".to_string(),
                title: "meteor client".to_string(),
                downloads: 100,
                ..make_hit("modrinth", "b")
            },
        ];
        sort_discover_hits(&mut hits, "relevance", Some("metor"));
        assert_eq!(hits.first().map(|hit| hit.project_id.as_str()), Some("b"));
    }

    #[test]
    fn github_signal_gate_rejects_low_similarity_without_minecraft_signal() {
        let repo = sample_discover_repo();
        assert!(!github_repo_passes_signal_gate(
            &repo,
            0,
            12,
            "trouser treaks"
        ));
    }

    #[test]
    fn github_signal_gate_allows_high_similarity_without_minecraft_signal() {
        let repo = sample_discover_repo();
        assert!(github_repo_passes_signal_gate(
            &repo,
            0,
            40,
            "trouser treaks"
        ));
    }

    #[test]
    fn github_signal_gate_allows_positive_minecraft_signal() {
        let repo = sample_discover_repo();
        assert!(github_repo_passes_signal_gate(&repo, 2, 0, "any"));
    }

    #[test]
    fn github_mod_ecosystem_signal_does_not_confuse_model_with_mod() {
        let repo = sample_non_minecraft_ml_repo();
        assert!(github_repo_mod_ecosystem_signal_score(&repo) <= 0);
    }

    #[test]
    fn github_lookup_queries_strip_local_version_noise() {
        let queries =
            github_lookup_queries_for_local_mod("Trouser-Streak-v1.5.8-fabric-1.21.1.jar", None);
        assert!(!queries.is_empty());
        assert!(queries
            .iter()
            .any(|query| query.contains("trouser") && query.contains("streak")));
    }

    #[test]
    fn github_discover_search_queries_prioritize_typo_fallback_without_tokens() {
        let queries = github_discover_search_queries("Trouser Treaks", false);
        assert!(!queries.is_empty());
        assert!(queries
            .iter()
            .any(|q| q.contains("trouser in:name,description")));
        assert!(queries.len() <= GITHUB_UNAUTH_MAX_SEARCH_QUERIES);
    }
}

#[cfg(test)]
mod lock_entry_name_tests {
    use super::*;

    #[test]
    fn mod_entries_use_core_jar_filename_as_name() {
        let name = canonical_lock_entry_name("mods", "meteor-client-1.21.1-0.5.8.jar", "Meteor");
        assert_eq!(name, "meteor-client-1.21.1-0.5.8");
    }

    #[test]
    fn mod_entries_strip_disabled_suffix_before_naming() {
        let name = canonical_lock_entry_name(
            "mods",
            "trouser-streak-1.21.1.jar.disabled",
            "Trouser Streak",
        );
        assert_eq!(name, "trouser-streak-1.21.1");
    }

    #[test]
    fn non_mod_entries_keep_existing_name() {
        let name = canonical_lock_entry_name(
            "resourcepacks",
            "fresh-animations-1.0.0.zip",
            "Fresh Animations",
        );
        assert_eq!(name, "Fresh Animations");
    }
}

#[cfg(test)]
mod github_provider_tests {
    use super::*;

    fn sample_repo() -> GithubRepository {
        GithubRepository {
            full_name: "OpenJar/test-mod".to_string(),
            name: "test-mod".to_string(),
            description: Some("A test Minecraft mod".to_string()),
            stargazers_count: 4200,
            forks_count: 120,
            archived: false,
            fork: false,
            disabled: false,
            html_url: "https://github.com/OpenJar/test-mod".to_string(),
            homepage: None,
            watchers_count: 4200,
            open_issues_count: 12,
            pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
            updated_at: Some("2026-03-01T00:00:00Z".to_string()),
            topics: vec!["minecraft".to_string(), "fabric".to_string()],
            default_branch: "main".to_string(),
            owner: GithubOwner {
                login: "OpenJar".to_string(),
                owner_type: "Organization".to_string(),
            },
        }
    }

    fn release_with_assets(id: u64, published_at: &str, assets: Vec<&str>) -> GithubRelease {
        GithubRelease {
            id,
            tag_name: format!("v{id}"),
            html_url: format!("https://github.com/OpenJar/test-mod/releases/tag/v{id}"),
            name: None,
            draft: false,
            prerelease: false,
            created_at: Some(published_at.to_string()),
            published_at: Some(published_at.to_string()),
            assets: assets
                .into_iter()
                .map(|name| GithubReleaseAsset {
                    name: name.to_string(),
                    browser_download_url: format!(
                        "https://github.com/OpenJar/test-mod/releases/download/v{id}/{name}"
                    ),
                    content_type: Some("application/java-archive".to_string()),
                    size: 2 * 1024 * 1024,
                    digest: None,
                })
                .collect(),
        }
    }

    #[test]
    fn github_repo_policy_rejects_unsafe_repository_states() {
        let mut repo = sample_repo();
        repo.archived = true;
        assert_eq!(
            github_repo_policy_rejection_reason(&repo),
            Some("repository is archived")
        );
        repo.archived = false;
        repo.fork = true;
        assert_eq!(
            github_repo_policy_rejection_reason(&repo),
            Some("repository is a fork")
        );
        repo.fork = false;
        repo.disabled = true;
        assert_eq!(
            github_repo_policy_rejection_reason(&repo),
            Some("repository is disabled")
        );
    }

    #[test]
    fn github_error_classification_detects_auth_or_rate_limit() {
        assert!(github_error_is_auth_or_rate_limit(
            "GitHub API rate limit reached (403 Forbidden)."
        ));
        assert!(github_error_is_auth_or_rate_limit(
            "GitHub request failed with status 401 Unauthorized."
        ));
        assert!(!github_error_is_auth_or_rate_limit(
            "GitHub request failed with status 404 Not Found."
        ));
    }

    #[test]
    fn github_reason_transient_detection_covers_verification_unavailable_messages() {
        assert!(github_reason_is_transient_verification_failure(
            "GitHub local identify manual candidate: direct metadata repo hint matched, but release verification is unavailable (GitHub API rate limit reached)."
        ));
        assert!(github_reason_is_transient_verification_failure(
            "GitHub local identify manual candidate: direct metadata repo hint found, but release evidence is currently unverifiable."
        ));
        assert!(!github_reason_is_transient_verification_failure(
            "GitHub local identify manual candidate: direct metadata repo hint matched, but no verified release asset matched the local file."
        ));
    }

    #[test]
    fn github_release_selector_picks_latest_real_jar_asset() {
        let repo = sample_repo();
        let releases = vec![
            release_with_assets(
                1,
                "2026-01-01T00:00:00Z",
                vec!["test-mod-1.0.0.jar", "checksums.sha256"],
            ),
            release_with_assets(
                2,
                "2026-02-01T00:00:00Z",
                vec!["test-mod-1.1.0-sources.jar", "test-mod-1.1.0.jar"],
            ),
        ];
        let selected =
            select_github_release_with_asset(&repo, &releases, "test mod", None, None, None, None)
                .expect("expected a selected github release");
        assert_eq!(selected.release.id, 2);
        assert_eq!(selected.asset.name, "test-mod-1.1.0.jar");
        assert!(!selected.asset.name.contains("sources"));
    }

    #[test]
    fn github_release_query_hint_prefers_installed_filename() {
        let repo = sample_repo();
        let hint = github_release_query_hint("test-mod-1.2.3.jar.disabled", "Pretty Mod Name", &repo);
        assert_eq!(hint, "test-mod-1.2.3");
    }

    #[test]
    fn github_release_selection_match_detects_same_release_label() {
        let repo = sample_repo();
        let releases = vec![release_with_assets(
            7,
            "2026-03-01T00:00:00Z",
            vec!["test-mod-1.2.0.jar"],
        )];
        let selection = select_github_release_with_asset(
            &repo,
            &releases,
            "test-mod-1.2.0",
            None,
            None,
            None,
            None,
        )
        .expect("expected selected release");
        assert!(github_release_selection_matches_current(
            &selection,
            "gh_release:999",
            "v7",
            &HashMap::new(),
        ));
    }

    #[test]
    fn github_discover_hit_contains_confidence_metadata() {
        let repo = sample_repo();
        let releases = vec![release_with_assets(
            3,
            "2026-03-01T00:00:00Z",
            vec!["test-mod-1.2.0.jar", "checksums.sha256"],
        )];
        let selected =
            select_github_release_with_asset(&repo, &releases, "test mod", None, None, None, None)
                .expect("expected selected release");
        let hit = github_release_to_discover_hit(&repo, &selected, "test mod", None, None);
        assert_eq!(hit.source, "github");
        assert_eq!(hit.content_type, "mods");
        assert!(hit.confidence.is_some());
        assert!(hit.reason.is_some());
    }

    #[test]
    fn github_release_selector_enforces_instance_compatibility() {
        let repo = sample_repo();
        let releases = vec![
            release_with_assets(
                1,
                "2026-01-01T00:00:00Z",
                vec!["test-mod-fabric-1.20.4.jar"],
            ),
            release_with_assets(
                2,
                "2026-02-01T00:00:00Z",
                vec!["test-mod-fabric-1.21.1.jar"],
            ),
        ];
        let selected = select_github_release_with_asset(
            &repo,
            &releases,
            "test mod",
            Some("1.21.1"),
            Some("fabric"),
            None,
            None,
        )
        .expect("expected a compatible github selection");
        assert_eq!(selected.release.id, 2);

        let incompatible = select_github_release_with_asset(
            &repo,
            &releases,
            "test mod",
            Some("1.21.1"),
            Some("forge"),
            None,
            None,
        );
        assert!(incompatible.is_none());
    }

    #[test]
    fn github_asset_digest_matching_rejects_mismatch() {
        let mut digests = HashMap::new();
        digests.insert("sha256".to_string(), "abc123".to_string());
        assert_eq!(
            github_asset_digest_matches_local_hashes(&digests, "abc123", "zzz"),
            Some(true)
        );
        assert_eq!(
            github_asset_digest_matches_local_hashes(&digests, "nope", "zzz"),
            Some(false)
        );
    }

    #[test]
    fn github_local_release_selector_uses_exact_asset_filename() {
        let repo = sample_repo();
        let releases = vec![
            release_with_assets(1, "2026-01-01T00:00:00Z", vec!["test-mod-1.0.0.jar"]),
            release_with_assets(
                2,
                "2026-02-01T00:00:00Z",
                vec!["test-mod-1.2.0.jar", "test-mod-1.2.0-sources.jar"],
            ),
        ];
        let selected = select_github_release_for_local_file(
            &repo,
            &releases,
            "test-mod-1.2.0.jar",
            "test mod",
        )
        .expect("expected exact filename match");
        assert_eq!(selected.release.id, 2);
        assert_eq!(selected.asset.name, "test-mod-1.2.0.jar");
    }

    #[test]
    fn github_local_release_selector_accepts_strong_name_pattern_match() {
        let repo = sample_repo();
        let releases = vec![
            release_with_assets(
                1,
                "2026-01-01T00:00:00Z",
                vec!["meteor-client-fabric-1.21.1-0.5.8.jar"],
            ),
            release_with_assets(2, "2026-02-01T00:00:00Z", vec!["something-else-1.0.0.jar"]),
        ];
        let selected = select_github_release_for_local_file(
            &repo,
            &releases,
            "meteor-client-0.5.8.jar",
            "meteor client",
        )
        .expect("expected strong fuzzy filename pattern match");
        assert_eq!(selected.release.id, 1);
        assert_eq!(selected.asset.name, "meteor-client-fabric-1.21.1-0.5.8.jar");
    }

    #[test]
    fn github_local_match_rejects_similarity_only_without_hard_evidence() {
        let repo = sample_repo();
        let release = release_with_assets(1, "2026-01-01T00:00:00Z", vec!["totally-different.jar"]);
        let selection = GithubReleaseSelection {
            release: release.clone(),
            asset: release.assets[0].clone(),
            has_checksum_sidecar: false,
        };
        let result = github_local_match_confidence_and_reason(
            &repo,
            &selection,
            "meteor-client-1.0.0.jar",
            "meteor client",
            None,
            false,
            0,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn github_local_match_rejects_ambiguous_baritone_on_weak_repo() {
        let weak_repo = GithubRepository {
            full_name: "kaushikkumarbora/forager".to_string(),
            name: "forager".to_string(),
            description: Some("A random utility project".to_string()),
            stargazers_count: 12,
            forks_count: 1,
            archived: false,
            fork: false,
            disabled: false,
            html_url: "https://github.com/kaushikkumarbora/forager".to_string(),
            homepage: None,
            watchers_count: 12,
            open_issues_count: 0,
            pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
            updated_at: Some("2026-03-01T00:00:00Z".to_string()),
            topics: vec![],
            default_branch: "main".to_string(),
            owner: GithubOwner {
                login: "kaushikkumarbora".to_string(),
                owner_type: "User".to_string(),
            },
        };
        let release = release_with_assets(2, "2026-01-01T00:00:00Z", vec!["baritone-1.0.0.jar"]);
        let selection = GithubReleaseSelection {
            release: release.clone(),
            asset: release.assets[0].clone(),
            has_checksum_sidecar: false,
        };
        let result = github_local_match_confidence_and_reason(
            &weak_repo,
            &selection,
            "baritone-1.0.0.jar",
            "baritone",
            None,
            false,
            0,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn github_local_known_repo_boost_enables_canonical_baritone_match() {
        let canonical_repo = GithubRepository {
            full_name: "cabaletta/baritone".to_string(),
            name: "baritone".to_string(),
            description: Some("Minecraft pathfinding bot".to_string()),
            stargazers_count: 8_000,
            forks_count: 900,
            archived: false,
            fork: false,
            disabled: false,
            html_url: "https://github.com/cabaletta/baritone".to_string(),
            homepage: None,
            watchers_count: 8_000,
            open_issues_count: 12,
            pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
            updated_at: Some("2026-03-01T00:00:00Z".to_string()),
            topics: vec!["minecraft".to_string()],
            default_branch: "main".to_string(),
            owner: GithubOwner {
                login: "cabaletta".to_string(),
                owner_type: "Organization".to_string(),
            },
        };
        let (boost, reason) =
            github_local_known_repo_boost(&canonical_repo, "baritone-1.0.0.jar", "baritone", None);
        assert!(boost >= 40);
        assert!(reason.is_some());

        let release = release_with_assets(3, "2026-01-01T00:00:00Z", vec!["baritone-1.0.0.jar"]);
        let selection = GithubReleaseSelection {
            release: release.clone(),
            asset: release.assets[0].clone(),
            has_checksum_sidecar: false,
        };
        let evaluated = github_local_match_confidence_and_reason(
            &canonical_repo,
            &selection,
            "baritone-1.0.0.jar",
            "baritone",
            None,
            false,
            boost,
            reason,
            None,
        )
        .expect("canonical match accepted");
        assert!(matches!(evaluated.0.as_str(), "high" | "deterministic"));
    }

    #[test]
    fn extract_github_repo_slug_parses_owner_repo_urls() {
        let parsed = extract_github_repo_slug("https://github.com/MeteorDevelopment/meteor-client");
        assert_eq!(parsed.as_deref(), Some("MeteorDevelopment/meteor-client"));
    }

    #[test]
    fn extract_github_repo_slug_rejects_non_github_urls() {
        assert!(extract_github_repo_slug("https://meteorclient.com").is_none());
        assert!(extract_github_repo_slug("https://jfronny.gitlab.io").is_none());
    }

    #[test]
    fn parse_github_project_id_rejects_non_github_urls() {
        assert!(parse_github_project_id("https://meteorclient.com").is_err());
        assert!(parse_github_project_id("gh:https://meteorclient.com").is_err());
        assert!(parse_github_project_id("https://jfronny.gitlab.io").is_err());
        assert!(parse_github_project_id("gh:https://jfronny.gitlab.io").is_err());
    }

    #[test]
    fn parse_github_project_id_accepts_github_urls_with_extra_path_segments() {
        let parsed = parse_github_project_id(
            "https://github.com/MeteorDevelopment/meteor-client/releases/tag/v1.0.0",
        )
        .expect("github release URL should parse");
        assert_eq!(parsed.0, "MeteorDevelopment");
        assert_eq!(parsed.1, "meteor-client");
    }

    #[test]
    fn parse_toml_assignment_is_case_insensitive() {
        let toml = r#"
            modId = "examplemod"
            displayName = "Example Mod"
            displayURL = "https://github.com/example/mod-repo"
        "#;
        assert_eq!(
            parse_toml_assignment(toml, "modid").as_deref(),
            Some("examplemod")
        );
        assert_eq!(
            parse_toml_assignment(toml, "displayname").as_deref(),
            Some("Example Mod")
        );
        assert_eq!(
            parse_toml_assignment(toml, "displayurl").as_deref(),
            Some("https://github.com/example/mod-repo")
        );
    }

    #[test]
    fn github_api_tokens_from_env_entries_supports_pool_and_numbered_tokens() {
        let entries = vec![
            (
                "MPM_GITHUB_TOKENS".to_string(),
                "poolA, poolB;poolC\npoolD".to_string(),
            ),
            ("MPM_GITHUB_TOKEN_2".to_string(), "two".to_string()),
            ("MPM_GITHUB_TOKEN_1".to_string(), "one".to_string()),
            ("MPM_GITHUB_TOKEN_10".to_string(), "ten".to_string()),
            ("MPM_GITHUB_TOKEN".to_string(), "single".to_string()),
        ];
        let tokens = github_api_tokens_from_env_entries(&entries);
        assert_eq!(
            tokens,
            vec!["poolA", "poolB", "poolC", "poolD", "one", "two", "ten", "single",]
        );
    }

    #[test]
    fn github_api_tokens_from_env_entries_supports_non_mpm_numbered_tokens() {
        let entries = vec![
            ("GITHUB_TOKEN_2".to_string(), "two".to_string()),
            ("GH_TOKEN_1".to_string(), "one".to_string()),
            ("GH_TOKEN_3".to_string(), "three".to_string()),
            ("GITHUB_TOKEN".to_string(), "fallback".to_string()),
        ];
        let tokens = github_api_tokens_from_env_entries(&entries);
        assert_eq!(tokens, vec!["one", "two", "three", "fallback"]);
    }

    #[test]
    fn github_api_tokens_from_env_entries_deduplicates_across_sources() {
        let entries = vec![
            (
                "MPM_GITHUB_TOKENS".to_string(),
                "same,other,same".to_string(),
            ),
            ("MPM_GITHUB_TOKEN_1".to_string(), "same".to_string()),
            ("GITHUB_TOKEN".to_string(), "other".to_string()),
            ("GH_TOKEN".to_string(), "third".to_string()),
        ];
        let tokens = github_api_tokens_from_env_entries(&entries);
        assert_eq!(tokens, vec!["same", "other", "third"]);
    }

    #[test]
    fn github_api_tokens_from_env_entries_caps_to_max_tokens() {
        let pool = (1..=(GITHUB_API_TOKENS_MAX + 20))
            .map(|idx| format!("token{idx}"))
            .collect::<Vec<_>>()
            .join(",");
        let entries = vec![("MPM_GITHUB_TOKENS".to_string(), pool)];
        let tokens = github_api_tokens_from_env_entries(&entries);
        let expected_last = format!("token{}", GITHUB_API_TOKENS_MAX);
        assert_eq!(tokens.len(), GITHUB_API_TOKENS_MAX);
        assert_eq!(tokens.first().map(String::as_str), Some("token1"));
        assert_eq!(
            tokens.last().map(String::as_str),
            Some(expected_last.as_str())
        );
    }

    #[test]
    fn github_unverified_manual_candidate_is_manual_and_activatable_only_for_transient_outages() {
        let candidate = github_unverified_manual_candidate(
            "example",
            "repo",
            "Example Repo",
            "sha256",
            "sha512",
            "GitHub local identify manual candidate: direct metadata repo hint found, but repository verification is unavailable (rate limited).".to_string(),
        );
        assert_eq!(candidate.source, "github");
        assert_eq!(candidate.project_id, "gh:example/repo");
        assert_eq!(candidate.version_id, "gh_repo_unverified");
        assert_eq!(candidate.confidence, "manual");
        assert_eq!(
            candidate.hashes.get("sha256").map(String::as_str),
            Some("sha256")
        );
        assert_eq!(
            candidate.hashes.get("sha512").map(String::as_str),
            Some("sha512")
        );
        assert!(provider_match_is_auto_activatable(&candidate));

        let non_transient = github_unverified_manual_candidate(
            "example",
            "repo",
            "Example Repo",
            "sha256",
            "sha512",
            "GitHub local identify manual candidate: direct metadata repo hint matched, but no verified release asset matched the local file.".to_string(),
        );
        assert!(!provider_match_is_auto_activatable(&non_transient));
    }

    #[test]
    fn github_provider_activation_rejects_invalid_project_ids() {
        let invalid_match = LocalImportedProviderMatch {
            source: "github".to_string(),
            project_id: "gh:https://meteorclient.com".to_string(),
            version_id: "gh_release:123".to_string(),
            name: "Invalid".to_string(),
            version_number: "1.0.0".to_string(),
            hashes: HashMap::new(),
            confidence: "deterministic".to_string(),
            reason: "invalid".to_string(),
        };
        assert!(!provider_match_is_auto_activatable(&invalid_match));

        let invalid_candidate = ProviderCandidate {
            source: "github".to_string(),
            project_id: "gh:https://meteorclient.com".to_string(),
            version_id: "gh_release:123".to_string(),
            name: "Invalid".to_string(),
            version_number: "1.0.0".to_string(),
            confidence: Some("deterministic".to_string()),
            reason: Some("invalid".to_string()),
        };
        assert!(!provider_candidate_is_auto_activatable(&invalid_candidate));
    }
}

#[cfg(test)]
mod update_check_resilience_tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn github_test_guard() -> MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("github test mutex lock")
    }

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
        let _guard = github_test_guard();
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
        let _guard = github_test_guard();
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
        let _guard = github_test_guard();
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
}

#[cfg(test)]
mod token_storage_tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn token_test_guard() -> MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("token test mutex lock")
    }

    fn make_temp_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("openjar-token-tests-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn persist_refresh_token_does_not_create_plaintext_fallback_file() {
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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

        let summary = migrate_legacy_refresh_tokens_from_path(&fallback_path)
            .expect("migrate fallback tokens");
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
        let _guard = token_test_guard();
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
}

#[cfg(test)]
mod runtime_and_playtime_tests {
    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("openjar-runtime-tests-{label}-{}", Uuid::new_v4()))
    }

    #[test]
    fn runtime_reconcile_copies_missing_entries_and_keeps_non_allowlisted_conflicts() {
        let instance_dir = temp_path("runtime-reconcile");
        fs::create_dir_all(instance_dir.join("runtime")).expect("create runtime");
        fs::create_dir_all(instance_dir.join("mods")).expect("create canonical mods");
        fs::create_dir_all(instance_dir.join("runtime").join(".meteor-client"))
            .expect("create runtime meteor");
        fs::write(
            instance_dir
                .join("runtime")
                .join(".meteor-client")
                .join("config.json"),
            br#"{"ok":true}"#,
        )
        .expect("write runtime meteor config");
        fs::write(instance_dir.join("mods").join("keep.jar"), b"canonical")
            .expect("write canonical mod");
        fs::write(instance_dir.join("runtime").join("mods"), b"bad")
            .expect("write conflicting runtime file");
        fs::write(
            instance_dir.join("runtime").join("options.txt"),
            b"runtime options",
        )
        .expect("write runtime options");
        fs::write(instance_dir.join("options.txt"), b"canonical options")
            .expect("write canonical options");

        reconcile_legacy_runtime_into_instance(&instance_dir).expect("reconcile runtime");

        assert!(instance_dir
            .join(".meteor-client")
            .join("config.json")
            .exists());
        assert_eq!(
            fs::read_to_string(instance_dir.join("options.txt")).expect("read options"),
            "canonical options"
        );
        assert!(instance_dir.join("mods").join("keep.jar").exists());
        assert!(runtime_reconcile_marker_path(&instance_dir).exists());

        let _ = fs::remove_dir_all(&instance_dir);
    }

    #[test]
    fn isolated_clone_excludes_transient_roots_and_keeps_game_content() {
        let instance_dir = temp_path("isolated-clone");
        let isolated_dir = instance_dir.join("runtime_sessions").join("launch");
        fs::create_dir_all(instance_dir.join("mods")).expect("create mods");
        fs::create_dir_all(instance_dir.join("config")).expect("create config");
        fs::create_dir_all(instance_dir.join("runtime_sessions").join("old"))
            .expect("create old session");
        fs::create_dir_all(instance_dir.join("snapshots").join("s1")).expect("create snapshot");
        fs::create_dir_all(instance_dir.join("logs").join("launches")).expect("create launch logs");
        fs::write(instance_dir.join("mods").join("a.jar"), b"jar").expect("write mod jar");
        fs::write(instance_dir.join("play_sessions.v1.json"), b"{}").expect("write play sessions");
        fs::write(
            instance_dir.join("logs").join("launches").join("x.log"),
            b"log",
        )
        .expect("write launch log");

        clone_instance_to_isolated_runtime(&instance_dir, &isolated_dir)
            .expect("clone isolated runtime");

        assert!(isolated_dir.join("mods").join("a.jar").exists());
        assert!(isolated_dir.join("config").exists());
        assert!(!isolated_dir.join("runtime_sessions").exists());
        assert!(!isolated_dir.join("snapshots").exists());
        assert!(!isolated_dir.join("play_sessions.v1.json").exists());
        assert!(!isolated_dir.join("logs").join("launches").exists());

        let _ = fs::remove_dir_all(&instance_dir);
    }

    #[test]
    fn playtime_store_tracks_native_session_duration_and_summary() {
        let instances_dir = temp_path("playtime");
        fs::create_dir_all(&instances_dir).expect("create instances root");
        let instance = Instance {
            id: "inst_playtime".to_string(),
            name: "Playtime".to_string(),
            folder_name: Some("Playtime".to_string()),
            mc_version: "1.20.1".to_string(),
            loader: "fabric".to_string(),
            created_at: now_iso(),
            icon_path: None,
            settings: InstanceSettings::default(),
        };
        let index = InstanceIndex {
            instances: vec![instance.clone()],
        };
        write_index(&instances_dir, &index).expect("write index");
        let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
        fs::create_dir_all(&instance_dir).expect("create instance dir");

        register_native_play_session_start(
            &instances_dir,
            &instance.id,
            "native_test",
            std::process::id(),
            false,
        )
        .expect("register active play session");
        let mut active = read_active_play_sessions_store(&instance_dir);
        assert_eq!(active.active.len(), 1);
        active.active[0].started_at = format!("unix:{}", Utc::now().timestamp().saturating_sub(5));
        write_active_play_sessions_store(&instance_dir, active).expect("write active store");

        let finalized = finalize_native_play_session(
            &instances_dir,
            &instance.id,
            "native_test",
            "success",
            false,
        )
        .expect("finalize play session");
        assert!(finalized.is_some());

        let summary =
            instance_playtime_summary(&instances_dir, &instance.id).expect("read playtime summary");
        assert!(summary.total_seconds >= 5);
        assert_eq!(summary.sessions_count, 1);
        assert_eq!(summary.tracking_scope, "native_only");

        let _ = fs::remove_dir_all(&instances_dir);
    }
}

#[cfg(test)]
mod instance_health_tests {
    use super::*;

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
}
