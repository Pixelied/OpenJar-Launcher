use crate::{normalize_lock_content_type, LockEntry, Lockfile};
use serde::Serialize;
#[cfg(target_os = "macos")]
use std::collections::HashSet;
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LaunchMicRequirementSummary {
    pub required: bool,
    pub confidence: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detected_mods: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LaunchPermissionChecklistItem {
    pub key: String,
    pub label: String,
    pub status: String,
    pub required: bool,
    pub blocking: bool,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PermissionCompatibilitySignal {
    pub code: &'static str,
    pub title: String,
    pub message: String,
    pub severity: &'static str,
    pub blocking: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PermissionEvaluation {
    pub mic_requirement: Option<LaunchMicRequirementSummary>,
    pub checklist: Vec<LaunchPermissionChecklistItem>,
    pub signals: Vec<PermissionCompatibilitySignal>,
}

#[derive(Debug, Clone)]
struct MicDetectionRule {
    id: &'static str,
    confidence: &'static str,
    project_ids: &'static [&'static str],
    name_tokens: &'static [&'static str],
    filename_tokens: &'static [&'static str],
}

const MIC_DETECTION_RULES: &[MicDetectionRule] = &[
    MicDetectionRule {
        id: "simple_voice_chat",
        confidence: "high",
        project_ids: &[
            "mr:9egkb6k1",
            "mr:simple-voice-chat",
            "mr:simple_voice_chat",
            "cf:416089",
            "curseforge:416089",
        ],
        name_tokens: &["simple voice chat", "simplevoicechat"],
        filename_tokens: &["simple-voice-chat", "voicechat"],
    },
    MicDetectionRule {
        id: "plasmo_voice",
        confidence: "high",
        project_ids: &[
            "mr:1bzhdhsh",
            "mr:plasmo-voice",
            "mr:plasmovoice",
            "cf:317977",
            "curseforge:317977",
        ],
        name_tokens: &["plasmo voice", "plasmovoice"],
        filename_tokens: &["plasmo-voice", "plasmovoice"],
    },
    MicDetectionRule {
        id: "generic_voice_chat_pattern",
        confidence: "medium",
        project_ids: &[],
        name_tokens: &["voice chat", "voicechat"],
        filename_tokens: &["voicechat"],
    },
];

pub(crate) fn evaluate_launch_permissions(
    lock: &Lockfile,
    is_native_launch: bool,
    java_executable: Option<&str>,
) -> PermissionEvaluation {
    let mic_requirement = detect_microphone_requirement(lock);
    let mut checklist = vec![
        evaluate_microphone_permission(
            mic_requirement.required,
            mic_requirement.detected_mods.clone(),
            is_native_launch,
            java_executable,
        ),
        LaunchPermissionChecklistItem {
            key: "camera".to_string(),
            label: "Camera".to_string(),
            status: "not_required".to_string(),
            required: false,
            blocking: false,
            detail: "Not required for Minecraft launch right now.".to_string(),
            evidence: Vec::new(),
        },
        LaunchPermissionChecklistItem {
            key: "screen_recording".to_string(),
            label: "Screen Recording".to_string(),
            status: "not_required".to_string(),
            required: false,
            blocking: false,
            detail: "Not required for Minecraft launch right now.".to_string(),
            evidence: Vec::new(),
        },
        LaunchPermissionChecklistItem {
            key: "accessibility".to_string(),
            label: "Accessibility".to_string(),
            status: "not_required".to_string(),
            required: false,
            blocking: false,
            detail: "Not required for Minecraft launch right now.".to_string(),
            evidence: Vec::new(),
        },
    ];
    let mic_item = checklist
        .iter()
        .find(|item| item.key == "microphone")
        .cloned();
    let mut signals: Vec<PermissionCompatibilitySignal> = Vec::new();
    if let Some(mic) = mic_item {
        match mic.status.as_str() {
            "denied" => signals.push(PermissionCompatibilitySignal {
                code: "MIC_PERMISSION_DENIED",
                title: "Microphone permission denied for Java".to_string(),
                message: "Voice chat mod detected, but Java/Minecraft currently cannot use the microphone.".to_string(),
                severity: "blocker",
                blocking: true,
            }),
            "not_determined" => signals.push(PermissionCompatibilitySignal {
                code: "MIC_PERMISSION_REQUIRED",
                title: "Microphone permission needed for voice chat".to_string(),
                message: "Voice chat mod detected. Allow microphone access for Java/Minecraft before launch.".to_string(),
                severity: "blocker",
                blocking: true,
            }),
            "unavailable" if mic.required => signals.push(PermissionCompatibilitySignal {
                code: "MIC_PERMISSION_CHECK_UNAVAILABLE",
                title: "Microphone permission could not be auto-verified".to_string(),
                message: mic.detail.clone(),
                severity: "warning",
                blocking: false,
            }),
            "unknown" if mic.required => signals.push(PermissionCompatibilitySignal {
                code: "MIC_PERMISSION_CHECK_UNKNOWN",
                title: "Microphone permission status unknown".to_string(),
                message: mic.detail.clone(),
                severity: "warning",
                blocking: false,
            }),
            _ => {}
        }
    }
    PermissionEvaluation {
        mic_requirement: Some(mic_requirement),
        checklist: {
            for item in &mut checklist {
                if item.key == "microphone" && !item.required {
                    item.blocking = false;
                }
            }
            checklist
        },
        signals,
    }
}

fn detect_microphone_requirement(lock: &Lockfile) -> LaunchMicRequirementSummary {
    let mut matched_mods: Vec<String> = Vec::new();
    let mut matched_rules: Vec<&MicDetectionRule> = Vec::new();

    for entry in lock.entries.iter().filter(is_enabled_mod_entry) {
        if let Some(rule) = match_mic_rule(entry) {
            if !matched_rules.iter().any(|existing| existing.id == rule.id) {
                matched_rules.push(rule);
            }
            let label = entry_display_label(entry);
            if !label.is_empty() && !matched_mods.iter().any(|item| item.eq_ignore_ascii_case(&label)) {
                matched_mods.push(label);
            }
        }
    }

    let required = !matched_rules.is_empty();
    let confidence = if matched_rules.iter().any(|rule| rule.confidence == "high") {
        "high"
    } else if required {
        "medium"
    } else {
        "low"
    };
    let reasons = if required {
        matched_rules
            .iter()
            .map(|rule| {
                format!(
                    "Matched voice chat detection rule '{}' ({} confidence).",
                    rule.id, rule.confidence
                )
            })
            .collect()
    } else {
        vec!["No known voice chat mods detected in enabled mod entries.".to_string()]
    };

    LaunchMicRequirementSummary {
        required,
        confidence: confidence.to_string(),
        detected_mods: matched_mods,
        reasons,
    }
}

fn evaluate_microphone_permission(
    mic_required: bool,
    detected_mods: Vec<String>,
    is_native_launch: bool,
    java_executable: Option<&str>,
) -> LaunchPermissionChecklistItem {
    if !mic_required {
        return LaunchPermissionChecklistItem {
            key: "microphone".to_string(),
            label: "Microphone".to_string(),
            status: "not_required".to_string(),
            required: false,
            blocking: false,
            detail: "No known voice chat mod detected for this instance.".to_string(),
            evidence: Vec::new(),
        };
    }

    if !is_native_launch {
        return LaunchPermissionChecklistItem {
            key: "microphone".to_string(),
            label: "Microphone".to_string(),
            status: "unavailable".to_string(),
            required: true,
            blocking: false,
            detail: "Voice chat detected. Prism launch manages Java externally, so microphone permission cannot be auto-verified here.".to_string(),
            evidence: detected_mods,
        };
    }

    #[cfg(target_os = "macos")]
    {
        let java = java_executable.unwrap_or_default().trim();
        if java.is_empty() {
            return LaunchPermissionChecklistItem {
                key: "microphone".to_string(),
                label: "Microphone".to_string(),
                status: "unknown".to_string(),
                required: true,
                blocking: false,
                detail: "Voice chat detected, but Java path is unresolved so microphone permission could not be verified.".to_string(),
                evidence: detected_mods,
            };
        }
        match check_macos_microphone_permission_for_java(java) {
            MacMicrophonePermission::Granted => LaunchPermissionChecklistItem {
                key: "microphone".to_string(),
                label: "Microphone".to_string(),
                status: "granted".to_string(),
                required: true,
                blocking: false,
                detail: format!("Java has microphone permission ({java})."),
                evidence: detected_mods,
            },
            MacMicrophonePermission::Denied => LaunchPermissionChecklistItem {
                key: "microphone".to_string(),
                label: "Microphone".to_string(),
                status: "denied".to_string(),
                required: true,
                blocking: true,
                detail: "Voice chat detected and macOS currently denies microphone access for Java/Minecraft.".to_string(),
                evidence: detected_mods,
            },
            MacMicrophonePermission::NotDetermined => LaunchPermissionChecklistItem {
                key: "microphone".to_string(),
                label: "Microphone".to_string(),
                status: "not_determined".to_string(),
                required: true,
                blocking: true,
                detail: "Voice chat detected. Allow microphone access for Java/Minecraft in System Settings before launch.".to_string(),
                evidence: detected_mods,
            },
            MacMicrophonePermission::Unavailable(detail) => LaunchPermissionChecklistItem {
                key: "microphone".to_string(),
                label: "Microphone".to_string(),
                status: "unavailable".to_string(),
                required: true,
                blocking: false,
                detail,
                evidence: detected_mods,
            },
            MacMicrophonePermission::Unknown(detail) => LaunchPermissionChecklistItem {
                key: "microphone".to_string(),
                label: "Microphone".to_string(),
                status: "unknown".to_string(),
                required: true,
                blocking: false,
                detail,
                evidence: detected_mods,
            },
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = java_executable;
        LaunchPermissionChecklistItem {
            key: "microphone".to_string(),
            label: "Microphone".to_string(),
            status: "unavailable".to_string(),
            required: true,
            blocking: false,
            detail: "Voice chat detected. Automatic microphone permission checks are currently macOS-only; verify OS privacy settings if voice chat cannot hear input.".to_string(),
            evidence: detected_mods,
        }
    }
}

fn match_mic_rule(entry: &LockEntry) -> Option<&'static MicDetectionRule> {
    let project_id = normalize_token(&entry.project_id);
    let name = normalize_token(&entry.name);
    let filename = normalize_token(&entry.filename);

    MIC_DETECTION_RULES.iter().find(|rule| {
        rule.project_ids
            .iter()
            .map(|value| normalize_token(value))
            .any(|token| !token.is_empty() && token == project_id)
            || rule
                .name_tokens
                .iter()
                .map(|value| normalize_token(value))
                .any(|token| !token.is_empty() && name.contains(&token))
            || rule
                .filename_tokens
                .iter()
                .map(|value| normalize_token(value))
                .any(|token| !token.is_empty() && filename.contains(&token))
    })
}

fn is_enabled_mod_entry(entry: &&LockEntry) -> bool {
    entry.enabled && normalize_lock_content_type(&entry.content_type) == "mods"
}

fn entry_display_label(entry: &LockEntry) -> String {
    let name = entry.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    let filename = entry.filename.trim();
    if !filename.is_empty() {
        return filename.to_string();
    }
    entry.project_id.trim().to_string()
}

fn normalize_token(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
enum MacMicrophonePermission {
    Granted,
    Denied,
    NotDetermined,
    Unavailable(String),
    Unknown(String),
}

#[cfg(target_os = "macos")]
fn check_macos_microphone_permission_for_java(java_executable: &str) -> MacMicrophonePermission {
    let raw_path = PathBuf::from(java_executable);
    let canonical = raw_path
        .canonicalize()
        .unwrap_or_else(|_| raw_path.clone());
    let db_path = tcc_db_path();
    if !db_path.exists() {
        return MacMicrophonePermission::Unknown(
            "Could not find macOS TCC database for microphone permissions.".to_string(),
        );
    }
    let columns = match tcc_access_columns(&db_path) {
        Ok(cols) => cols,
        Err(err) => {
            if is_macos_tcc_access_restricted_error(&err) {
                return MacMicrophonePermission::Unavailable(
                    "Automatic microphone permission check is unavailable because macOS restricts access to privacy metadata for this app. You can still launch; verify Java/Minecraft under System Settings > Privacy & Security > Microphone.".to_string(),
                );
            }
            return MacMicrophonePermission::Unknown(format!(
                "Could not inspect macOS microphone permission database ({}).",
                sanitize_macos_tcc_error(&err)
            ))
        }
    };
    let mut candidates = vec![java_executable.to_string()];
    let canonical_str = canonical.to_string_lossy().to_string();
    if !canonical_str.is_empty()
        && !candidates
            .iter()
            .any(|value| value.eq_ignore_ascii_case(&canonical_str))
    {
        candidates.push(canonical_str);
    }
    let auth_value = if columns.contains("auth_value") {
        query_latest_tcc_int_value(&db_path, "auth_value", &candidates)
    } else if columns.contains("allowed") {
        query_latest_tcc_int_value(&db_path, "allowed", &candidates)
    } else {
        Err("No supported permission status column found.".to_string())
    };
    match auth_value {
        Ok(Some(value)) => {
            if columns.contains("auth_value") {
                if value >= 2 {
                    MacMicrophonePermission::Granted
                } else {
                    MacMicrophonePermission::Denied
                }
            } else if value >= 1 {
                MacMicrophonePermission::Granted
            } else {
                MacMicrophonePermission::Denied
            }
        }
        Ok(None) => MacMicrophonePermission::NotDetermined,
        Err(err) => {
            if is_macos_tcc_access_restricted_error(&err) {
                return MacMicrophonePermission::Unavailable(
                    "Automatic microphone permission check is unavailable because macOS restricts access to privacy metadata for this app. You can still launch; verify Java/Minecraft under System Settings > Privacy & Security > Microphone.".to_string(),
                );
            }
            MacMicrophonePermission::Unknown(format!(
                "Could not read macOS microphone permission status ({}).",
                sanitize_macos_tcc_error(&err)
            ))
        }
    }
}

#[cfg(target_os = "macos")]
fn tcc_db_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("com.apple.TCC")
        .join("TCC.db")
}

#[cfg(target_os = "macos")]
fn tcc_access_columns(path: &Path) -> Result<HashSet<String>, String> {
    let raw = run_sqlite(path, "PRAGMA table_info('access');")?;
    let mut out = HashSet::new();
    for line in raw.lines() {
        let mut parts = line.split('|');
        let _idx = parts.next();
        let Some(name) = parts.next() else {
            continue;
        };
        let token = name.trim().to_string();
        if !token.is_empty() {
            out.insert(token);
        }
    }
    if out.is_empty() {
        return Err("access table metadata was empty".to_string());
    }
    Ok(out)
}

#[cfg(target_os = "macos")]
fn query_latest_tcc_int_value(
    db_path: &Path,
    column_name: &str,
    clients: &[String],
) -> Result<Option<i64>, String> {
    if clients.is_empty() {
        return Ok(None);
    }
    let client_list = clients
        .iter()
        .map(|value| format!("'{}'", value.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT {column_name} FROM access WHERE service='kTCCServiceMicrophone' AND client IN ({client_list}) ORDER BY last_modified DESC LIMIT 1;"
    );
    let raw = run_sqlite(db_path, &sql)?;
    let line = raw.lines().next().unwrap_or("").trim().to_string();
    if line.is_empty() {
        return Ok(None);
    }
    line.parse::<i64>()
        .map(Some)
        .map_err(|e| format!("parse sqlite value failed: {e}"))
}

#[cfg(target_os = "macos")]
fn run_sqlite(path: &Path, sql: &str) -> Result<String, String> {
    let output = Command::new("sqlite3")
        .arg(path)
        .arg(sql)
        .output()
        .map_err(|e| format!("sqlite3 process failed: {e}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

fn is_macos_tcc_access_restricted_error(raw: &str) -> bool {
    let msg = raw.trim().to_ascii_lowercase();
    msg.contains("authorization denied")
        || msg.contains("not authorized")
        || msg.contains("permission denied")
        || msg.contains("operation not permitted")
}

fn sanitize_macos_tcc_error(raw: &str) -> String {
    if is_macos_tcc_access_restricted_error(raw) {
        "macOS denied access to privacy metadata".to_string()
    } else {
        raw.trim().to_string()
    }
}

pub(crate) fn trigger_java_microphone_permission_prompt(
    java_executable: &str,
) -> Result<String, String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = java_executable;
        Ok("Automatic Java microphone permission prompt is available on macOS only.".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        trigger_java_microphone_permission_prompt_macos(java_executable)
    }
}

pub(crate) fn open_microphone_system_settings() -> Result<String, String> {
    #[cfg(not(target_os = "macos"))]
    {
        Ok("Open your OS privacy settings and allow microphone access for Java/Minecraft, then click Re-check."
            .to_string())
    }

    #[cfg(target_os = "macos")]
    {
        open_microphone_system_settings_macos()
    }
}

fn microphone_settings_open_targets() -> &'static [&'static str] {
    &[
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        "x-apple.systempreferences:com.apple.preference.security?Privacy",
        "x-apple.systempreferences:",
        "/System/Applications/System Settings.app",
        "/System/Applications/System Preferences.app",
    ]
}

fn microphone_settings_open_success_message(target: &str) -> String {
    if target.contains("Privacy_Microphone") {
        "Opened System Settings > Privacy & Security > Microphone. Enable Java/Minecraft, then click Re-check."
            .to_string()
    } else {
        "Opened System Settings. Enable microphone access for Java/Minecraft, then click Re-check."
            .to_string()
    }
}

#[cfg(target_os = "macos")]
fn open_microphone_system_settings_macos() -> Result<String, String> {
    let mut failures: Vec<String> = Vec::new();
    for target in microphone_settings_open_targets() {
        match Command::new("/usr/bin/open").arg(target).output() {
            Ok(output) if output.status.success() => {
                return Ok(microphone_settings_open_success_message(target));
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let reason = if stderr.is_empty() {
                    format!("exit status {}", output.status)
                } else {
                    stderr
                };
                failures.push(format!("{} ({})", target, reason));
            }
            Err(err) => failures.push(format!("{} ({})", target, err)),
        }
    }
    let details = failures
        .into_iter()
        .take(2)
        .collect::<Vec<_>>()
        .join("; ");
    if details.is_empty() {
        Err("Could not open System Settings automatically.".to_string())
    } else {
        Err(format!(
            "Could not open System Settings automatically ({details})."
        ))
    }
}

#[cfg(target_os = "macos")]
fn trigger_java_microphone_permission_prompt_macos(java_executable: &str) -> Result<String, String> {
    let java = java_executable.trim();
    if java.is_empty() {
        return Err("Java executable is not configured for this instance.".to_string());
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let temp_root = std::env::temp_dir().join(format!("openjar-mic-probe-{nonce}"));
    fs::create_dir_all(&temp_root)
        .map_err(|e| format!("create microphone probe temp dir failed: {e}"))?;
    let source_path = temp_root.join("OpenJarMicProbe.java");
    fs::write(&source_path, java_mic_probe_source())
        .map_err(|e| format!("write microphone probe source failed: {e}"))?;

    let output = Command::new(java).arg(&source_path).output();
    let _ = fs::remove_file(&source_path);
    let _ = fs::remove_dir(&temp_root);

    let output = output.map_err(|e| format!("run Java microphone probe failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let merged = format!("{stdout}\n{stderr}");
    let merged_lower = merged.to_ascii_lowercase();

    if merged.contains("OPENJAR_MIC_GRANTED") {
        return Ok(
            "Java microphone probe succeeded and this Java runtime can access microphone input. If voice chat still cannot hear input, verify Java/Minecraft under System Settings > Privacy & Security > Microphone."
                .to_string(),
        );
    }
    if merged.contains("OPENJAR_MIC_DENIED") {
        return Ok(
            "Java microphone probe indicates access is denied. Open System Settings > Privacy & Security > Microphone and allow Java/Minecraft."
                .to_string(),
        );
    }
    if merged.contains("OPENJAR_MIC_NO_DEVICE") || merged.contains("OPENJAR_MIC_NO_LINE") {
        return Ok(
            "Java microphone probe ran but no input device was available. Connect a microphone, then try again."
                .to_string(),
        );
    }

    if merged_lower.contains("source-file mode") || merged_lower.contains("source file mode") {
        return Err(
            "This Java runtime does not support source-file execution for the microphone probe. Use Java 11+ for this helper."
                .to_string(),
        );
    }

    if !output.status.success() {
        return Err(format!(
            "Java microphone probe failed: {}",
            summarize_java_probe_error(&merged)
        ));
    }

    Ok("Java microphone probe ran. If no prompt appeared, launch Minecraft and then Re-check permissions."
        .to_string())
}

#[cfg(target_os = "macos")]
fn summarize_java_probe_error(raw: &str) -> String {
    let compact = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if compact.is_empty() {
        "unknown Java probe failure".to_string()
    } else {
        compact.chars().take(220).collect::<String>()
    }
}

#[cfg(target_os = "macos")]
fn java_mic_probe_source() -> &'static str {
    r#"import javax.sound.sampled.AudioFormat;
import javax.sound.sampled.AudioSystem;
import javax.sound.sampled.DataLine;
import javax.sound.sampled.LineUnavailableException;
import javax.sound.sampled.TargetDataLine;

public class OpenJarMicProbe {
    public static void main(String[] args) {
        AudioFormat format = new AudioFormat(16000.0f, 16, 1, true, false);
        DataLine.Info info = new DataLine.Info(TargetDataLine.class, format);
        if (!AudioSystem.isLineSupported(info)) {
            System.out.println("OPENJAR_MIC_NO_LINE");
            return;
        }
        try (TargetDataLine line = (TargetDataLine) AudioSystem.getLine(info)) {
            line.open(format);
            line.start();
            try {
                Thread.sleep(250L);
            } catch (InterruptedException ignored) {
            }
            line.stop();
            System.out.println("OPENJAR_MIC_GRANTED");
        } catch (SecurityException denied) {
            System.out.println("OPENJAR_MIC_DENIED");
        } catch (LineUnavailableException unavailable) {
            System.out.println("OPENJAR_MIC_NO_DEVICE");
        } catch (Throwable other) {
            System.out.println("OPENJAR_MIC_ERROR");
        }
    }
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LockEntry;
    use std::collections::HashMap;

    fn mk_entry(
        content_type: &str,
        enabled: bool,
        project_id: &str,
        name: &str,
        filename: &str,
    ) -> LockEntry {
        LockEntry {
            source: "modrinth".to_string(),
            project_id: project_id.to_string(),
            version_id: "v1".to_string(),
            name: name.to_string(),
            version_number: "1.0.0".to_string(),
            filename: filename.to_string(),
            content_type: content_type.to_string(),
            target_scope: "instance".to_string(),
            target_worlds: Vec::new(),
            pinned_version: None,
            enabled,
            hashes: HashMap::new(),
            provider_candidates: Vec::new(),
        }
    }

    #[test]
    fn mic_detection_matches_simple_voice_chat_filename() {
        let lock = Lockfile {
            version: 2,
            entries: vec![mk_entry(
                "mods",
                true,
                "mr:unknown",
                "My Voice Mod",
                "simple-voice-chat-fabric-2.5.0.jar",
            )],
        };
        let result = detect_microphone_requirement(&lock);
        assert!(result.required);
        assert_eq!(result.confidence, "high");
        assert!(!result.detected_mods.is_empty());
    }

    #[test]
    fn mic_detection_matches_known_project_id() {
        let lock = Lockfile {
            version: 2,
            entries: vec![mk_entry(
                "mods",
                true,
                "cf:416089",
                "Some Name",
                "voice-chat.jar",
            )],
        };
        let result = detect_microphone_requirement(&lock);
        assert!(result.required);
        assert_eq!(result.confidence, "high");
    }

    #[test]
    fn mic_detection_ignores_disabled_and_non_mod_entries() {
        let lock = Lockfile {
            version: 2,
            entries: vec![
                mk_entry(
                    "mods",
                    false,
                    "cf:416089",
                    "Simple Voice Chat",
                    "simple-voice-chat.jar",
                ),
                mk_entry(
                    "resourcepacks",
                    true,
                    "mr:9egkb6k1",
                    "Simple Voice Chat",
                    "simple-voice-chat.zip",
                ),
            ],
        };
        let result = detect_microphone_requirement(&lock);
        assert!(!result.required);
        assert_eq!(result.confidence, "low");
    }

    #[test]
    fn tcc_access_restricted_errors_are_detected() {
        assert!(is_macos_tcc_access_restricted_error(
            "Error: unable to open database \"/Users/test/Library/Application Support/com.apple.TCC/TCC.db\": authorization denied"
        ));
        assert!(is_macos_tcc_access_restricted_error("permission denied"));
        assert!(!is_macos_tcc_access_restricted_error("no such table: access"));
    }

    #[test]
    fn tcc_error_sanitizer_hides_raw_access_denied_details() {
        let sanitized = sanitize_macos_tcc_error(
            "Error: unable to open database \"/Users/test/Library/Application Support/com.apple.TCC/TCC.db\": authorization denied",
        );
        assert_eq!(sanitized, "macOS denied access to privacy metadata");

        let passthrough = sanitize_macos_tcc_error("no such table: access");
        assert_eq!(passthrough, "no such table: access");
    }

    #[test]
    fn microphone_settings_targets_prioritize_microphone_privacy_pane() {
        let targets = microphone_settings_open_targets();
        assert_eq!(
            targets.first().copied(),
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        );
        assert!(targets.iter().any(|value| value.contains("System Settings.app")));
    }

    #[test]
    fn microphone_settings_success_message_mentions_microphone_for_privacy_target() {
        let message = microphone_settings_open_success_message(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        );
        assert!(message.contains("Microphone"));
        assert!(message.contains("Re-check"));
    }
}
