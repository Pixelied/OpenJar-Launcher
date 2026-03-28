use super::RunFinding;
use crate::{entry_file_exists, normalize_lock_content_type, Instance, Lockfile};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(crate) struct ClassifierInput<'a> {
    pub instance: &'a Instance,
    pub lock: &'a Lockfile,
    pub instance_dir: &'a Path,
    pub launch_log_text: &'a str,
    pub crash_log_text: &'a str,
    pub java_major: Option<u32>,
    pub required_java_major: u32,
    pub exit_code: Option<i32>,
    pub exit_message: Option<&'a str>,
}

pub(crate) struct ClassifierOutput {
    pub findings: Vec<RunFinding>,
    pub phase: Option<String>,
    pub suspect_mod_tokens: Vec<String>,
    pub config_paths: Vec<String>,
}

fn clamp_confidence(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    let bounded = value.clamp(0.0, 1.0);
    (bounded * 100.0).round() / 100.0
}

fn clean_snippet(raw: &str) -> String {
    let collapsed = raw
        .replace('\t', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.len() <= 180 {
        return collapsed;
    }
    format!("{}...", &collapsed[..177])
}

fn normalize_mod_token(raw: &str) -> String {
    let token = raw
        .trim()
        .to_lowercase()
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '.')
        .trim_end_matches(".jar")
        .to_string();
    if token.len() < 3 {
        return String::new();
    }
    match token.as_str() {
        "minecraft" | "java" | "client" | "server" | "forge" | "fabric" | "quilt" | "neoforge"
        | "mod" | "mods" | "loader" | "mixin" | "core" => String::new(),
        _ => token,
    }
}

fn loader_label(loader: &str) -> &'static str {
    match loader.trim().to_lowercase().as_str() {
        "fabric" => "Fabric",
        "forge" => "Forge",
        "quilt" => "Quilt",
        "neoforge" => "NeoForge",
        _ => "Vanilla",
    }
}

fn text_contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn collect_evidence(lines: &[String], needles: &[&str], limit: usize) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for line in lines {
        let lower = line.to_lowercase();
        if !text_contains_any(&lower, needles) {
            continue;
        }
        let cleaned = clean_snippet(line);
        if cleaned.is_empty() || out.contains(&cleaned) {
            continue;
        }
        out.push(cleaned);
        if out.len() >= limit {
            break;
        }
    }
    out
}

fn detect_phase(lines: &[String]) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for (idx, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        let phase = if text_contains_any(
            &lower,
            &[
                "mixin",
                "mod loading has failed",
                "constructing mods",
                "loading mods",
                "bootstrap",
            ],
        ) {
            Some("early_init")
        } else if text_contains_any(
            &lower,
            &[
                "loading world",
                "joining world",
                "integrated server",
                "preparing spawn",
                "saving chunks",
            ],
        ) {
            Some("world_load")
        } else if text_contains_any(
            &lower,
            &["shader", "opengl", "glfw", "render", "framebuffer", "gpu"],
        ) {
            Some("render")
        } else if text_contains_any(
            &lower,
            &[
                "disconnect",
                "handshake",
                "login packet",
                "timed out",
                "connection reset",
                "join server",
            ],
        ) {
            Some("network_join")
        } else {
            None
        };
        if let Some(found) = phase {
            match best {
                Some((best_idx, _)) if idx >= best_idx => {}
                _ => best = Some((idx, found)),
            }
        }
    }
    best.map(|(_, phase)| phase.to_string())
}

fn detect_loader_mismatch_from_filename(loader: &str, filename: &str) -> bool {
    let normalized_loader = loader.trim().to_lowercase();
    let lower_file = filename.to_lowercase();
    if normalized_loader == "fabric" || normalized_loader == "quilt" {
        return lower_file.contains("forge") || lower_file.contains("neoforge");
    }
    if normalized_loader == "forge" || normalized_loader == "neoforge" {
        return lower_file.contains("fabric") || lower_file.contains("quilt");
    }
    false
}

fn collect_mod_tokens_from_line(line: &str) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let lower = line.to_lowercase();
    let parts = lower
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '.')
        .collect::<Vec<_>>();
    for token in parts {
        if token.len() < 3 {
            continue;
        }
        if token.ends_with(".json") || token.ends_with(".toml") {
            continue;
        }
        if token.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let normalized = normalize_mod_token(token);
        if normalized.is_empty() {
            continue;
        }
        if out.contains(&normalized) {
            continue;
        }
        out.push(normalized);
    }
    out
}

fn extract_config_path(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    let marker = lower.find("config/")?;
    let tail = &line[marker..];
    let mut path = String::new();
    for ch in tail.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '/' | '\\' | '-' | '_' | '.') {
            path.push(ch);
            continue;
        }
        break;
    }
    if path.is_empty() {
        return None;
    }
    let normalized = path.replace('\\', "/");
    if normalized.ends_with(".json")
        || normalized.ends_with(".toml")
        || normalized.ends_with(".yaml")
        || normalized.ends_with(".yml")
        || normalized.ends_with(".properties")
    {
        return Some(normalized);
    }
    None
}

fn push_or_update_finding(findings: &mut Vec<RunFinding>, mut next: RunFinding) {
    next.confidence = clamp_confidence(next.confidence);
    if let Some(existing) = findings.iter_mut().find(|item| item.id == next.id) {
        if next.confidence > existing.confidence {
            existing.confidence = next.confidence;
        }
        for snippet in next.evidence {
            if existing.evidence.len() >= 3 {
                break;
            }
            if !existing.evidence.contains(&snippet) {
                existing.evidence.push(snippet);
            }
        }
        if existing.mod_id.is_none() {
            existing.mod_id = next.mod_id;
        }
        if existing.file_path.is_none() {
            existing.file_path = next.file_path;
        }
        return;
    }
    findings.push(next);
}

pub(crate) fn classify(input: &ClassifierInput<'_>) -> ClassifierOutput {
    let mut lines: Vec<String> = input
        .launch_log_text
        .lines()
        .chain(input.crash_log_text.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(14_000)
        .map(|line| line.to_string())
        .collect();
    if let Some(msg) = input.exit_message {
        if !msg.trim().is_empty() {
            lines.push(msg.trim().to_string());
        }
    }

    let phase = detect_phase(&lines);
    let mut findings = Vec::<RunFinding>::new();
    let mut suspect_mod_tokens = HashSet::<String>::new();
    let mut config_paths = HashSet::<String>::new();

    let mut missing_enabled_mods = Vec::<String>::new();
    let mut duplicate_mods = HashMap::<String, usize>::new();
    for entry in &input.lock.entries {
        if !entry.enabled || normalize_lock_content_type(&entry.content_type) != "mods" {
            continue;
        }
        if !entry_file_exists(input.instance_dir, entry) {
            missing_enabled_mods.push(entry.filename.clone());
        }
        *duplicate_mods
            .entry(entry.filename.to_lowercase())
            .or_insert(0) += 1;
        if detect_loader_mismatch_from_filename(&input.instance.loader, &entry.filename) {
            suspect_mod_tokens.insert(normalize_mod_token(&entry.project_id));
        }
    }

    if !missing_enabled_mods.is_empty() {
        let preview = missing_enabled_mods
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "missing_enabled_mod_jars".to_string(),
                category: "mod_state".to_string(),
                title: "Missing enabled mod jar(s)".to_string(),
                explanation: format!(
                    "{} enabled mod file(s) are missing from disk, so those mods cannot load.",
                    missing_enabled_mods.len()
                ),
                confidence: 0.95,
                evidence: preview,
                likely_fix: Some(
                    "Disable or reinstall the missing mod jars, then relaunch.".to_string(),
                ),
                mod_id: None,
                file_path: None,
            },
        );
    }

    let duplicate_list = duplicate_mods
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(name, count)| format!("{} ({} copies)", name, count))
        .collect::<Vec<_>>();
    if !duplicate_list.is_empty() {
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "duplicate_mod_jars".to_string(),
                category: "mod_state".to_string(),
                title: "Duplicate mod jar(s) detected".to_string(),
                explanation:
                    "Duplicate jars can cause classpath conflicts and random startup failures."
                        .to_string(),
                confidence: 0.9,
                evidence: duplicate_list.into_iter().take(3).collect(),
                likely_fix: Some("Keep one jar per mod version and remove duplicates.".to_string()),
                mod_id: None,
                file_path: None,
            },
        );
    }

    let loader_mismatch_lines = collect_evidence(
        &lines,
        &[
            "requires fabric",
            "requires forge",
            "requires neoforge",
            "requires quilt",
            "not a valid mod file",
            "wrong side",
            "incompatible mod set",
        ],
        3,
    );
    if !loader_mismatch_lines.is_empty() {
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "mixed_loader_or_wrong_mod_type".to_string(),
                category: "loader".to_string(),
                title: "Mixed loader or wrong mod type".to_string(),
                explanation: format!(
                    "At least one jar appears incompatible with the {} loader stack.",
                    loader_label(&input.instance.loader)
                ),
                confidence: 0.91,
                evidence: loader_mismatch_lines,
                likely_fix: Some(
                    "Use only mods built for this loader and Minecraft version.".to_string(),
                ),
                mod_id: None,
                file_path: None,
            },
        );
    }

    let dependency_lines = collect_evidence(
        &lines,
        &[
            "missing mandatory dependency",
            "depends on",
            "requires",
            "mod loading has failed",
            "could not find required",
            "is missing",
        ],
        4,
    );
    if !dependency_lines.is_empty() {
        let mut mod_hint: Option<String> = None;
        for line in &dependency_lines {
            for token in collect_mod_tokens_from_line(line) {
                if token.contains("fabric") || token.contains("forge") || token.contains("quilt") {
                    continue;
                }
                mod_hint = Some(token.clone());
                suspect_mod_tokens.insert(token);
                break;
            }
            if mod_hint.is_some() {
                break;
            }
        }
        let title = if let Some(token) = mod_hint.as_ref() {
            format!("Mod did not load: {}", token)
        } else {
            "Mod did not load".to_string()
        };
        let reason = if dependency_lines
            .iter()
            .any(|line| line.to_lowercase().contains("missing mandatory dependency"))
        {
            "A required dependency is missing."
        } else if dependency_lines.iter().any(|line| {
            line.to_lowercase().contains("requires") && line.to_lowercase().contains("present")
        }) {
            "Detected incompatible dependency versions."
        } else {
            "A loader/dependency validation error prevented mod initialization."
        };
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "mod_did_not_load".to_string(),
                category: "mod_loading".to_string(),
                title,
                explanation: reason.to_string(),
                confidence: 0.94,
                evidence: dependency_lines,
                likely_fix: Some(
                    "Install required dependencies or align versions to one compatible set."
                        .to_string(),
                ),
                mod_id: mod_hint,
                file_path: None,
            },
        );
    }

    let broken_jar_lines = collect_evidence(
        &lines,
        &[
            "zip end header",
            "invalid or corrupt jar",
            "failed to read mod file",
            "jar signature",
            "bad zip",
        ],
        3,
    );
    if !broken_jar_lines.is_empty() {
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "broken_mod_jar".to_string(),
                category: "mod_loading".to_string(),
                title: "Mod jar appears broken".to_string(),
                explanation: "A jar could not be parsed or read, so loading stopped early."
                    .to_string(),
                confidence: 0.9,
                evidence: broken_jar_lines,
                likely_fix: Some("Replace the affected jar with a clean copy.".to_string()),
                mod_id: None,
                file_path: None,
            },
        );
    }

    let core_conflict_lines = collect_evidence(
        &lines,
        &[
            "architectury",
            "cloth-config",
            "forge config api",
            "fabric-api",
            "duplicate",
            "already present",
            "incompatible",
        ],
        4,
    );
    if core_conflict_lines.len() >= 2 {
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "core_library_conflict".to_string(),
                category: "mod_loading".to_string(),
                title: "Core library conflict pattern".to_string(),
                explanation: "Shared core libraries appear duplicated or incompatible across mods."
                    .to_string(),
                confidence: 0.82,
                evidence: core_conflict_lines,
                likely_fix: Some(
                    "Update/remove overlapping core library mods as a group.".to_string(),
                ),
                mod_id: None,
                file_path: None,
            },
        );
    }

    let oom_lines = collect_evidence(
        &lines,
        &[
            "outofmemoryerror",
            "java heap space",
            "gc overhead limit exceeded",
            "unable to allocate",
            "allocation failure",
        ],
        3,
    );
    if !oom_lines.is_empty() {
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "out_of_memory".to_string(),
                category: "runtime".to_string(),
                title: "Out of memory or GC thrash".to_string(),
                explanation: "The JVM ran out of memory or spent too long in garbage collection."
                    .to_string(),
                confidence: 0.97,
                evidence: oom_lines,
                likely_fix: Some(
                    "Increase memory for this instance and reduce heavy render/shader load."
                        .to_string(),
                ),
                mod_id: None,
                file_path: None,
            },
        );
    }

    let java_mismatch_lines = collect_evidence(
        &lines,
        &[
            "unsupportedclassversionerror",
            "class file version",
            "requires java",
            "java runtime",
        ],
        3,
    );
    let java_too_old = input
        .java_major
        .map(|major| major < input.required_java_major)
        .unwrap_or(false);
    if java_too_old || !java_mismatch_lines.is_empty() {
        let mut evidence = java_mismatch_lines;
        if java_too_old {
            evidence.insert(
                0,
                format!(
                    "Java {} detected, but Minecraft {} requires Java {}+",
                    input.java_major.unwrap_or(0),
                    input.instance.mc_version,
                    input.required_java_major
                ),
            );
        }
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "java_version_mismatch".to_string(),
                category: "runtime".to_string(),
                title: "Java version mismatch".to_string(),
                explanation:
                    "Minecraft and mods were launched with an incompatible Java major version."
                        .to_string(),
                confidence: if java_too_old { 0.99 } else { 0.85 },
                evidence,
                likely_fix: Some(format!(
                    "Switch this instance to Java {} or newer.",
                    input.required_java_major
                )),
                mod_id: None,
                file_path: None,
            },
        );
    }

    let config_lines = collect_evidence(
        &lines,
        &[
            "config",
            "toml",
            "json",
            "yaml",
            "properties",
            "failed to parse",
            "parse error",
            "invalid config",
        ],
        5,
    );
    if !config_lines.is_empty() {
        for line in &config_lines {
            if let Some(path) = extract_config_path(line) {
                config_paths.insert(path);
            }
        }
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "config_parse_error".to_string(),
                category: "config".to_string(),
                title: "Config parse error".to_string(),
                explanation: "A config file could not be parsed during startup.".to_string(),
                confidence: 0.9,
                evidence: config_lines,
                likely_fix: Some("Reset or fix the broken config file(s) and retry.".to_string()),
                mod_id: None,
                file_path: config_paths.iter().next().cloned(),
            },
        );
    }

    let permission_lines = collect_evidence(
        &lines,
        &[
            "permission denied",
            "access denied",
            "operation not permitted",
            "is being used by another process",
            "file is locked",
            "read-only file system",
        ],
        4,
    );
    if !permission_lines.is_empty() {
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "file_permission_or_lock".to_string(),
                category: "filesystem".to_string(),
                title: "File permission or lock issue".to_string(),
                explanation: "A required file could not be read/written due to permissions or file locks.".to_string(),
                confidence: 0.87,
                evidence: permission_lines,
                likely_fix: Some("Close tools that lock files (AV/editors), then retry with writable instance paths.".to_string()),
                mod_id: None,
                file_path: None,
            },
        );
    }

    let gpu_lines = collect_evidence(
        &lines,
        &[
            "opengl",
            "glfw error",
            "failed to create context",
            "shader",
            "driver",
            "renderer",
        ],
        4,
    );
    if !gpu_lines.is_empty() {
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "gpu_or_render_path".to_string(),
                category: "render".to_string(),
                title: "GPU/render pipeline issue".to_string(),
                explanation: "Rendering initialization failed (OpenGL/shader/driver path)."
                    .to_string(),
                confidence: 0.81,
                evidence: gpu_lines,
                likely_fix: Some(
                    "Disable shaderpacks, test without render mods, and verify GPU driver/runtime."
                        .to_string(),
                ),
                mod_id: None,
                file_path: None,
            },
        );
    }

    if let Some(phase_value) = phase.as_ref() {
        let phase_label = match phase_value.as_str() {
            "early_init" => "during early initialization",
            "world_load" => "during world load",
            "render" => "in the render/shader stage",
            "network_join" => "while joining a server",
            _ => "during launch",
        };
        let evidence = input
            .exit_message
            .map(|m| vec![clean_snippet(m)])
            .unwrap_or_default();
        push_or_update_finding(
            &mut findings,
            RunFinding {
                id: "failure_phase".to_string(),
                category: "timeline".to_string(),
                title: "Failure stage identified".to_string(),
                explanation: format!("The failure most likely happened {phase_label}."),
                confidence: 0.72,
                evidence,
                likely_fix: Some("Prioritize fixes related to this stage first.".to_string()),
                mod_id: None,
                file_path: None,
            },
        );
    }

    if let Some(code) = input.exit_code {
        if code != 0 && findings.is_empty() {
            push_or_update_finding(
                &mut findings,
                RunFinding {
                    id: "nonzero_exit_code".to_string(),
                    category: "runtime".to_string(),
                    title: "Game exited with a non-zero status".to_string(),
                    explanation: format!("Minecraft exited with code {code}."),
                    confidence: 0.65,
                    evidence: vec![format!("Exit code: {code}")],
                    likely_fix: Some(
                        "Open latest logs to inspect the first root-cause error.".to_string(),
                    ),
                    mod_id: None,
                    file_path: None,
                },
            );
        }
    }

    findings.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
    });

    let suspect_mod_tokens = suspect_mod_tokens
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let config_paths = config_paths.into_iter().collect::<Vec<_>>();

    ClassifierOutput {
        findings,
        phase,
        suspect_mod_tokens,
        config_paths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Instance, InstanceSettings, LockEntry, Lockfile};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn make_instance(loader: &str, mc: &str) -> Instance {
        Instance {
            id: "inst_test".to_string(),
            name: "Test Instance".to_string(),
            origin: "custom".to_string(),
            folder_name: None,
            mc_version: mc.to_string(),
            loader: loader.to_string(),
            created_at: "now".to_string(),
            icon_path: None,
            settings: InstanceSettings::default(),
        }
    }

    fn make_lock_entry(filename: &str, enabled: bool) -> LockEntry {
        LockEntry {
            source: "modrinth".to_string(),
            project_id: filename.replace(".jar", ""),
            version_id: "v1".to_string(),
            name: filename.to_string(),
            version_number: "1.0.0".to_string(),
            filename: filename.to_string(),
            content_type: "mods".to_string(),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            pinned_version: None,
            enabled,
            hashes: HashMap::new(),
            provider_candidates: vec![],
            local_analysis: None,
        }
    }

    fn make_temp_instance_dir() -> PathBuf {
        let root = std::env::temp_dir().join(format!("openjar-run-report-test-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("mods")).expect("create temp mods dir");
        root
    }

    #[test]
    fn detects_out_of_memory() {
        let instance = make_instance("fabric", "1.20.1");
        let lock = Lockfile::default();
        let dir = make_temp_instance_dir();
        let output = classify(&ClassifierInput {
            instance: &instance,
            lock: &lock,
            instance_dir: &dir,
            launch_log_text: "java.lang.OutOfMemoryError: Java heap space",
            crash_log_text: "",
            java_major: Some(17),
            required_java_major: 17,
            exit_code: Some(1),
            exit_message: Some("Game exited"),
        });
        assert!(output
            .findings
            .iter()
            .any(|item| item.id == "out_of_memory"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_java_version_mismatch() {
        let instance = make_instance("fabric", "1.20.6");
        let lock = Lockfile::default();
        let dir = make_temp_instance_dir();
        let output = classify(&ClassifierInput {
            instance: &instance,
            lock: &lock,
            instance_dir: &dir,
            launch_log_text: "UnsupportedClassVersionError",
            crash_log_text: "",
            java_major: Some(8),
            required_java_major: 17,
            exit_code: Some(1),
            exit_message: None,
        });
        assert!(output
            .findings
            .iter()
            .any(|item| item.id == "java_version_mismatch"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_config_parse_path() {
        let instance = make_instance("forge", "1.20.1");
        let lock = Lockfile::default();
        let dir = make_temp_instance_dir();
        let output = classify(&ClassifierInput {
            instance: &instance,
            lock: &lock,
            instance_dir: &dir,
            launch_log_text: "Failed to parse config config/sodium-options.json near line 3",
            crash_log_text: "",
            java_major: Some(17),
            required_java_major: 17,
            exit_code: Some(1),
            exit_message: None,
        });
        assert!(output
            .findings
            .iter()
            .any(|item| item.id == "config_parse_error"));
        assert!(output
            .config_paths
            .iter()
            .any(|path| path.contains("config/sodium-options.json")));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_missing_enabled_mod_file() {
        let instance = make_instance("fabric", "1.20.1");
        let lock = Lockfile {
            version: 2,
            entries: vec![make_lock_entry("missing-mod.jar", true)],
        };
        let dir = make_temp_instance_dir();
        let output = classify(&ClassifierInput {
            instance: &instance,
            lock: &lock,
            instance_dir: &dir,
            launch_log_text: "",
            crash_log_text: "",
            java_major: Some(17),
            required_java_major: 17,
            exit_code: Some(1),
            exit_message: None,
        });
        assert!(output
            .findings
            .iter()
            .any(|item| item.id == "missing_enabled_mod_jars"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_loader_mismatch_from_log() {
        let instance = make_instance("fabric", "1.20.1");
        let lock = Lockfile {
            version: 2,
            entries: vec![make_lock_entry("fancy-forge-addon.jar", true)],
        };
        let dir = make_temp_instance_dir();
        fs::write(dir.join("mods").join("fancy-forge-addon.jar"), b"jar").expect("write jar");
        let output = classify(&ClassifierInput {
            instance: &instance,
            lock: &lock,
            instance_dir: &dir,
            launch_log_text: "Mod fancy-addon requires Forge but Fabric loader is active",
            crash_log_text: "",
            java_major: Some(17),
            required_java_major: 17,
            exit_code: Some(1),
            exit_message: None,
        });
        assert!(output
            .findings
            .iter()
            .any(|item| item.id == "mixed_loader_or_wrong_mod_type"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_permission_issue() {
        let instance = make_instance("fabric", "1.20.1");
        let lock = Lockfile::default();
        let dir = make_temp_instance_dir();
        let output = classify(&ClassifierInput {
            instance: &instance,
            lock: &lock,
            instance_dir: &dir,
            launch_log_text: "Access denied: file is being used by another process",
            crash_log_text: "",
            java_major: Some(17),
            required_java_major: 17,
            exit_code: Some(1),
            exit_message: None,
        });
        assert!(output
            .findings
            .iter()
            .any(|item| item.id == "file_permission_or_lock"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_render_phase() {
        let instance = make_instance("fabric", "1.20.1");
        let lock = Lockfile::default();
        let dir = make_temp_instance_dir();
        let output = classify(&ClassifierInput {
            instance: &instance,
            lock: &lock,
            instance_dir: &dir,
            launch_log_text: "GLFW error: failed to create OpenGL context",
            crash_log_text: "shader compilation failed",
            java_major: Some(17),
            required_java_major: 17,
            exit_code: Some(1),
            exit_message: None,
        });
        assert_eq!(output.phase.as_deref(), Some("render"));
        assert!(output
            .findings
            .iter()
            .any(|item| item.id == "gpu_or_render_path"));
        let _ = fs::remove_dir_all(dir);
    }
}
