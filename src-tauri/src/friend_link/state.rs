use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalLockEntry {
    pub source: String,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub filename: String,
    pub content_type: String,
    pub target_scope: String,
    #[serde(default)]
    pub target_worlds: Vec<String>,
    pub enabled: bool,
    #[serde(default)]
    pub hashes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFileState {
    pub path: String,
    pub modified_at: i64,
    pub hash: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub state_hash: String,
    #[serde(default)]
    pub lock_entries: Vec<CanonicalLockEntry>,
    #[serde(default)]
    pub config_files: Vec<ConfigFileState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockEntryRaw {
    source: String,
    project_id: String,
    version_id: String,
    name: String,
    version_number: String,
    filename: String,
    #[serde(default = "default_content_type")]
    content_type: String,
    #[serde(default = "default_target_scope")]
    target_scope: String,
    #[serde(default)]
    target_worlds: Vec<String>,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    hashes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockFileRaw {
    #[serde(default = "default_lock_version")]
    version: u32,
    #[serde(default)]
    entries: Vec<LockEntryRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfigFileEntry {
    pub path: String,
    pub size_bytes: u64,
    pub modified_at: i64,
    pub editable: bool,
    pub kind: String,
    pub readonly_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadInstanceConfigFileResult {
    pub path: String,
    pub editable: bool,
    pub kind: String,
    pub size_bytes: u64,
    pub modified_at: i64,
    pub readonly_reason: Option<String>,
    pub content: Option<String>,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteInstanceConfigFileResult {
    pub path: String,
    pub size_bytes: u64,
    pub modified_at: i64,
    pub message: String,
}

fn default_content_type() -> String {
    "mods".to_string()
}

fn default_target_scope() -> String {
    "instance".to_string()
}

fn default_lock_version() -> u32 {
    2
}

fn normalized_content_type(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "mods" | "mod" => "mods".to_string(),
        "resourcepacks" | "resourcepack" | "texturepacks" | "texturepack" => "resourcepacks".to_string(),
        "shaderpacks" | "shaderpack" | "shader" | "shaders" => "shaderpacks".to_string(),
        "datapacks" | "datapack" => "datapacks".to_string(),
        _ => "mods".to_string(),
    }
}

fn normalized_target_scope(input: &str) -> String {
    match input.trim().to_lowercase().as_str() {
        "world" => "world".to_string(),
        _ => "instance".to_string(),
    }
}

pub fn default_allowlist() -> Vec<String> {
    vec![
        "options.txt".to_string(),
        "config/**/*.json".to_string(),
        "config/**/*.toml".to_string(),
        "config/**/*.properties".to_string(),
    ]
}

pub fn hard_excluded_prefixes() -> Vec<&'static str> {
    vec![
        "saves/",
        "logs/",
        "crash-reports/",
        "screenshots/",
        "resourcepacks/",
        "shaderpacks/",
        "mods/",
    ]
}

pub fn app_instances_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path_resolver()
        .app_data_dir()
        .ok_or_else(|| "Failed to resolve app data dir".to_string())?;
    Ok(base.join("instances"))
}

pub fn instance_dir(instances_dir: &Path, instance_id: &str) -> PathBuf {
    crate::instance_dir_for_id(instances_dir, instance_id)
        .unwrap_or_else(|_| instances_dir.join(instance_id))
}

pub fn lock_file_path(instances_dir: &Path, instance_id: &str) -> PathBuf {
    instance_dir(instances_dir, instance_id).join("lock.json")
}

pub fn safe_rel_path(raw: &str) -> Result<String, String> {
    let normalized = raw.replace('\\', "/").trim().trim_start_matches('/').to_string();
    if normalized.is_empty() {
        return Err("path is required".to_string());
    }
    if normalized.contains("..") {
        return Err("path traversal is not allowed".to_string());
    }
    Ok(normalized)
}

fn resolve_instance_file_path(instance_dir: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let rel = safe_rel_path(rel_path)?;
    Ok(instance_dir.join(rel))
}

fn modified_millis(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn normalize_hash_value(input: &str) -> String {
    input
        .trim()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn normalized_hashes_pairs(input: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut dedup = BTreeMap::<String, String>::new();
    for (raw_key, raw_value) in input {
        let key = raw_key.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        let value = normalize_hash_value(raw_value);
        if value.is_empty() {
            continue;
        }
        dedup
            .entry(key)
            .and_modify(|existing| {
                if value < *existing {
                    *existing = value.clone();
                }
            })
            .or_insert(value);
    }
    dedup.into_iter().collect()
}

fn normalized_hashes_map(input: &HashMap<String, String>) -> HashMap<String, String> {
    normalized_hashes_pairs(input).into_iter().collect()
}

fn lock_key_for(entry: &CanonicalLockEntry) -> String {
    format!(
        "lock::{}::{}::{}",
        entry.source.trim().to_lowercase(),
        entry.content_type.trim().to_lowercase(),
        entry.project_id.trim().to_lowercase()
    )
}

pub fn lock_entry_hash(entry: &CanonicalLockEntry) -> String {
    let mut normalized = entry.clone();
    normalized.target_worlds.sort();
    normalized.target_worlds.dedup();
    let normalized_hashes = normalized_hashes_pairs(&normalized.hashes);
    let canonical = (
        normalized.source.trim().to_ascii_lowercase(),
        normalized.project_id.trim().to_ascii_lowercase(),
        normalized.version_id.trim().to_string(),
        normalized.name.trim().to_string(),
        normalized.version_number.trim().to_string(),
        normalized.filename.trim().to_string(),
        normalized_content_type(&normalized.content_type),
        normalized_target_scope(&normalized.target_scope),
        normalized.target_worlds,
        normalized.enabled,
        normalized_hashes,
    );
    let raw = serde_json::to_vec(&canonical).unwrap_or_default();
    compute_sha256_hex(&raw)
}

pub fn config_file_hash(file: &ConfigFileState) -> String {
    file.hash.clone()
}

pub fn state_manifest(state: &SyncState) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for entry in &state.lock_entries {
        out.push((lock_key_for(entry), lock_entry_hash(entry), "lock_entry".to_string()));
    }
    for file in &state.config_files {
        out.push((
            format!("config::{}", file.path.to_lowercase()),
            config_file_hash(file),
            "config_file".to_string(),
        ));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn build_allowlist_globset(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            continue;
        }
        let glob = Glob::new(trimmed).map_err(|e| format!("invalid allowlist glob '{trimmed}': {e}"))?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|e| format!("build allowlist globset failed: {e}"))
}

fn path_is_excluded(rel_path: &str) -> bool {
    let lower = rel_path.to_lowercase();
    hard_excluded_prefixes()
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

fn path_matches_allowlist(rel_path: &str, allowlist: &[String], allowset: &GlobSet) -> bool {
    if path_is_excluded(rel_path) {
        return false;
    }
    if rel_path.eq_ignore_ascii_case("options.txt") {
        return true;
    }
    if allowlist.is_empty() {
        return false;
    }
    allowset.is_match(rel_path)
}

fn collect_files_recursive(root: &Path, current: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(current).map_err(|e| format!("read config directory failed: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read config directory entry failed: {e}"))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("read config metadata failed: {e}"))?;
        let ty = meta.file_type();
        if ty.is_symlink() {
            continue;
        }
        if ty.is_dir() {
            collect_files_recursive(root, &path, out)?;
            continue;
        }
        if ty.is_file() {
            out.push(path);
        }
    }
    let _ = root;
    Ok(())
}

fn normalize_rel_path(path: &Path, root: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let text = rel.to_string_lossy().replace('\\', "/");
    let clean = text.trim_start_matches('/').to_string();
    if clean.is_empty() {
        return None;
    }
    Some(clean)
}

pub fn collect_sync_state(
    instances_dir: &Path,
    instance_id: &str,
    allowlist: &[String],
) -> Result<SyncState, String> {
    let mut lock_entries = read_lock_entries(instances_dir, instance_id)?;
    lock_entries.sort_by(|a, b| lock_key_for(a).cmp(&lock_key_for(b)));

    let config_files = collect_allowlisted_config_files(instances_dir, instance_id, allowlist)?;

    let manifest_for_hash = state_manifest(&SyncState {
        state_hash: String::new(),
        lock_entries: lock_entries.clone(),
        config_files: config_files.clone(),
    });
    let state_hash = compute_sha256_hex(
        serde_json::to_vec(&manifest_for_hash)
            .map_err(|e| format!("serialize sync state for hashing failed: {e}"))?
            .as_slice(),
    );

    Ok(SyncState {
        state_hash,
        lock_entries,
        config_files,
    })
}

pub fn collect_allowlisted_config_files(
    instances_dir: &Path,
    instance_id: &str,
    allowlist: &[String],
) -> Result<Vec<ConfigFileState>, String> {
    let dir = instance_dir(instances_dir, instance_id);
    let allowset = build_allowlist_globset(allowlist)?;

    let mut candidate_paths: Vec<PathBuf> = Vec::new();
    let options_path = dir.join("options.txt");
    if options_path.exists() {
        candidate_paths.push(options_path);
    }

    let config_dir = dir.join("config");
    if config_dir.exists() && config_dir.is_dir() {
        collect_files_recursive(&dir, &config_dir, &mut candidate_paths)?;
    }

    let mut out = Vec::new();
    for path in candidate_paths {
        let Some(rel_path) = normalize_rel_path(&path, &dir) else {
            continue;
        };
        if !path_matches_allowlist(&rel_path, allowlist, &allowset) {
            continue;
        }

        let meta = fs::metadata(&path).map_err(|e| format!("read config metadata failed: {e}"))?;
        if !meta.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|e| format!("read config file failed: {e}"))?;
        let content = String::from_utf8(bytes.clone())
            .map_err(|_| format!("config file '{}' is not valid UTF-8", rel_path))?;

        out.push(ConfigFileState {
            path: rel_path,
            modified_at: modified_millis(&meta),
            hash: compute_sha256_hex(&bytes),
            content,
        });
    }

    out.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    Ok(out)
}

pub fn read_lock_entries(instances_dir: &Path, instance_id: &str) -> Result<Vec<CanonicalLockEntry>, String> {
    let path = lock_file_path(instances_dir, instance_id);
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("read lockfile failed: {e}"))?;
    let lock: LockFileRaw = serde_json::from_str(&raw).map_err(|e| format!("parse lockfile failed: {e}"))?;

    let mut out = Vec::new();
    for entry in lock.entries {
        let content_type = normalized_content_type(&entry.content_type);
        let mut target_worlds = entry.target_worlds;
        let target_scope = if content_type != "datapacks" {
            target_worlds.clear();
            "instance".to_string()
        } else {
            "world".to_string()
        };
        out.push(CanonicalLockEntry {
            source: entry.source,
            project_id: entry.project_id,
            version_id: entry.version_id,
            name: entry.name,
            version_number: entry.version_number,
            filename: entry.filename,
            content_type,
            target_scope,
            target_worlds: {
                target_worlds.sort();
                target_worlds.dedup();
                target_worlds
            },
            enabled: entry.enabled,
            hashes: normalized_hashes_map(&entry.hashes),
        });
    }
    Ok(out)
}

pub fn write_lock_entries(
    instances_dir: &Path,
    instance_id: &str,
    entries: &[CanonicalLockEntry],
) -> Result<(), String> {
    let path = lock_file_path(instances_dir, instance_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir instance dir failed: {e}"))?;
    }
    let mut normalized_entries = entries.to_vec();
    normalized_entries.sort_by(|a, b| lock_key_for(a).cmp(&lock_key_for(b)));

    let lock = LockFileRaw {
        version: 2,
        entries: normalized_entries
            .into_iter()
            .map(|entry| LockEntryRaw {
                source: entry.source,
                project_id: entry.project_id,
                version_id: entry.version_id,
                name: entry.name,
                version_number: entry.version_number,
                filename: entry.filename,
                content_type: normalized_content_type(&entry.content_type),
                target_scope: normalized_target_scope(&entry.target_scope),
                target_worlds: if normalized_content_type(&entry.content_type) == "datapacks" {
                    entry.target_worlds
                } else {
                    vec![]
                },
                enabled: entry.enabled,
                hashes: entry.hashes,
            })
            .collect(),
    };

    let raw = serde_json::to_string_pretty(&lock)
        .map_err(|e| format!("serialize lockfile failed: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("write lockfile failed: {e}"))
}

pub fn lock_entry_map(entries: &[CanonicalLockEntry]) -> HashMap<String, CanonicalLockEntry> {
    let mut map = HashMap::new();
    for entry in entries {
        map.insert(lock_key_for(entry), entry.clone());
    }
    map
}

pub fn config_file_map(files: &[ConfigFileState]) -> HashMap<String, ConfigFileState> {
    let mut map = HashMap::new();
    for file in files {
        map.insert(format!("config::{}", file.path.to_lowercase()), file.clone());
    }
    map
}

fn infer_file_kind(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.ends_with(".json") {
        "json".to_string()
    } else if lower.ends_with(".toml") {
        "toml".to_string()
    } else if lower.ends_with(".properties") {
        "properties".to_string()
    } else if lower.ends_with(".txt") {
        "text".to_string()
    } else {
        "file".to_string()
    }
}

fn describe_non_editable_reason(path: &Path, sample: &[u8]) -> Option<String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if matches!(ext.as_str(), "json" | "toml" | "properties" | "txt" | "cfg" | "conf" | "ini") {
        return None;
    }
    if sample.iter().any(|b| *b == 0u8) {
        return Some("Binary file cannot be edited here.".to_string());
    }
    None
}

pub fn list_instance_config_files(
    instances_dir: &Path,
    instance_id: &str,
) -> Result<Vec<InstanceConfigFileEntry>, String> {
    let dir = instance_dir(instances_dir, instance_id);
    let mut files = Vec::new();

    let options = dir.join("options.txt");
    if options.exists() && options.is_file() {
        let meta = fs::metadata(&options).map_err(|e| format!("read options metadata failed: {e}"))?;
        files.push(InstanceConfigFileEntry {
            path: "options.txt".to_string(),
            size_bytes: meta.len(),
            modified_at: modified_millis(&meta),
            editable: true,
            kind: "text".to_string(),
            readonly_reason: None,
        });
    }

    let config_dir = dir.join("config");
    if config_dir.exists() && config_dir.is_dir() {
        let mut raw = Vec::new();
        collect_files_recursive(&dir, &config_dir, &mut raw)?;
        for path in raw {
            let Some(rel_path) = normalize_rel_path(&path, &dir) else {
                continue;
            };
            let meta = fs::metadata(&path).map_err(|e| format!("read config metadata failed: {e}"))?;
            let mut sample = vec![0u8; 512];
            let mut file = fs::File::open(&path).map_err(|e| format!("open config file failed: {e}"))?;
            let read_len = file
                .read(&mut sample)
                .map_err(|e| format!("read config sample failed: {e}"))?;
            sample.truncate(read_len);
            let readonly_reason = describe_non_editable_reason(&path, &sample);
            files.push(InstanceConfigFileEntry {
                path: rel_path.clone(),
                size_bytes: meta.len(),
                modified_at: modified_millis(&meta),
                editable: readonly_reason.is_none(),
                kind: infer_file_kind(&rel_path),
                readonly_reason,
            });
        }
    }

    files.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    Ok(files)
}

pub fn read_instance_config_file(
    instances_dir: &Path,
    instance_id: &str,
    rel_path: &str,
) -> Result<ReadInstanceConfigFileResult, String> {
    let dir = instance_dir(instances_dir, instance_id);
    let path = resolve_instance_file_path(&dir, rel_path)?;
    let meta = fs::metadata(&path).map_err(|e| format!("read config metadata failed: {e}"))?;
    if !meta.is_file() {
        return Err("Requested config path is not a file".to_string());
    }
    let mut sample = vec![0u8; 512];
    let mut f = fs::File::open(&path).map_err(|e| format!("open config file failed: {e}"))?;
    let n = f
        .read(&mut sample)
        .map_err(|e| format!("read config sample failed: {e}"))?;
    sample.truncate(n);
    let readonly_reason = describe_non_editable_reason(&path, &sample);
    let normalized = safe_rel_path(rel_path)?;

    if readonly_reason.is_some() {
        return Ok(ReadInstanceConfigFileResult {
            path: normalized,
            editable: false,
            kind: infer_file_kind(rel_path),
            size_bytes: meta.len(),
            modified_at: modified_millis(&meta),
            readonly_reason,
            content: None,
            preview: Some("binary".to_string()),
        });
    }

    let bytes = fs::read(&path).map_err(|e| format!("read config file failed: {e}"))?;
    let content = String::from_utf8(bytes).map_err(|_| "File is not valid UTF-8 text.".to_string())?;

    Ok(ReadInstanceConfigFileResult {
        path: normalized,
        editable: true,
        kind: infer_file_kind(rel_path),
        size_bytes: meta.len(),
        modified_at: modified_millis(&meta),
        readonly_reason: None,
        content: Some(content),
        preview: None,
    })
}

pub fn write_instance_config_file(
    instances_dir: &Path,
    instance_id: &str,
    rel_path: &str,
    content: &str,
    expected_modified_at: Option<i64>,
) -> Result<WriteInstanceConfigFileResult, String> {
    let dir = instance_dir(instances_dir, instance_id);
    let path = resolve_instance_file_path(&dir, rel_path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir config dir failed: {e}"))?;
    }

    if path.exists() {
        let meta = fs::metadata(&path).map_err(|e| format!("read config metadata failed: {e}"))?;
        if let Some(expected) = expected_modified_at {
            let actual = modified_millis(&meta);
            if expected != actual {
                return Err("File changed on disk. Reload and try saving again.".to_string());
            }
        }
        if !meta.is_file() {
            return Err("Requested config path is not a file".to_string());
        }
    }

    let mut sample = content.as_bytes().to_vec();
    if sample.len() > 512 {
        sample.truncate(512);
    }
    if describe_non_editable_reason(&path, &sample).is_some() {
        return Err("Binary or unsupported config file cannot be edited.".to_string());
    }

    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "write".to_string())
    ));
    fs::write(&tmp, content.as_bytes()).map_err(|e| format!("write temp config file failed: {e}"))?;
    if let Err(err) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("replace config file failed: {err}"));
    }

    let meta = fs::metadata(&path).map_err(|e| format!("read config metadata failed: {e}"))?;
    Ok(WriteInstanceConfigFileResult {
        path: safe_rel_path(rel_path)?,
        size_bytes: meta.len(),
        modified_at: modified_millis(&meta),
        message: "Config file saved.".to_string(),
    })
}

pub fn preview_for_lock_entry(entry: &CanonicalLockEntry) -> String {
    format!(
        "{} {} {} {}",
        entry.name, entry.version_number, entry.source, entry.content_type
    )
}

pub fn preview_for_config_file(file: &ConfigFileState) -> String {
    format!("{} ({})", file.path, file.hash)
}

fn content_dir_for_type(instance_dir: &Path, content_type: &str) -> Option<PathBuf> {
    match normalized_content_type(content_type).as_str() {
        "mods" => Some(instance_dir.join("mods")),
        "resourcepacks" => Some(instance_dir.join("resourcepacks")),
        "shaderpacks" => Some(instance_dir.join("shaderpacks")),
        _ => None,
    }
}

pub fn lock_entry_paths(
    instances_dir: &Path,
    instance_id: &str,
    entry: &CanonicalLockEntry,
) -> Vec<PathBuf> {
    let instance_dir = instance_dir(instances_dir, instance_id);
    let content_type = normalized_content_type(&entry.content_type);
    if content_type == "datapacks" {
        let mut out = Vec::new();
        for world in &entry.target_worlds {
            let world_name = world.trim();
            if world_name.is_empty() {
                continue;
            }
            out.push(
                instance_dir
                    .join("saves")
                    .join(world_name)
                    .join("datapacks")
                    .join(&entry.filename),
            );
        }
        return out;
    }

    let Some(root) = content_dir_for_type(&instance_dir, &content_type) else {
        return vec![];
    };
    if content_type == "mods" {
        if entry.enabled {
            return vec![root.join(&entry.filename)];
        }
        return vec![root.join(format!("{}.disabled", entry.filename))];
    }
    vec![root.join(&entry.filename)]
}

pub fn read_lock_entry_bytes(
    instances_dir: &Path,
    instance_id: &str,
    entry: &CanonicalLockEntry,
) -> Result<Option<Vec<u8>>, String> {
    let mut paths = lock_entry_paths(instances_dir, instance_id, entry);
    if normalized_content_type(&entry.content_type) == "mods" {
        let instance_dir = instance_dir(instances_dir, instance_id);
        let mods_dir = instance_dir.join("mods");
        if entry.enabled {
            paths.push(mods_dir.join(format!("{}.disabled", entry.filename)));
        } else {
            paths.push(mods_dir.join(&entry.filename));
        }
    }
    for path in paths {
        if !path.exists() || !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|e| format!("read content file failed: {e}"))?;
        return Ok(Some(bytes));
    }
    Ok(None)
}

pub fn lock_entry_file_missing(
    instances_dir: &Path,
    instance_id: &str,
    entry: &CanonicalLockEntry,
) -> bool {
    let paths = lock_entry_paths(instances_dir, instance_id, entry);
    if paths.is_empty() {
        return true;
    }
    !paths.iter().all(|path| path.exists() && path.is_file())
}

pub fn write_lock_entry_bytes(
    instances_dir: &Path,
    instance_id: &str,
    entry: &CanonicalLockEntry,
    bytes: &[u8],
) -> Result<usize, String> {
    let paths = lock_entry_paths(instances_dir, instance_id, entry);
    if paths.is_empty() {
        return Err("no writable target paths for entry".to_string());
    }
    let mut wrote = 0usize;
    for path in paths {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir content dir failed: {e}"))?;
        }
        let tmp = path.with_extension(format!(
            "{}.sync.tmp",
            path.extension()
                .map(|v| v.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string())
        ));
        fs::write(&tmp, bytes).map_err(|e| format!("write temp content file failed: {e}"))?;
        if let Err(err) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(format!("replace content file failed: {err}"));
        }
        wrote += 1;
    }

    if normalized_content_type(&entry.content_type) == "mods" {
        let instance_dir = instance_dir(instances_dir, instance_id);
        let mods_dir = instance_dir.join("mods");
        let enabled_path = mods_dir.join(&entry.filename);
        let disabled_path = mods_dir.join(format!("{}.disabled", entry.filename));
        if entry.enabled {
            if disabled_path.exists() {
                let _ = fs::remove_file(&disabled_path);
            }
        } else if enabled_path.exists() {
            let _ = fs::remove_file(&enabled_path);
        }
    }

    Ok(wrote)
}
