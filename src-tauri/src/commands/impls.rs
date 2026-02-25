use crate::*;
use chrono::Local;
use reqwest::blocking::Client;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::Manager;
use uuid::Uuid;
use zip::write::FileOptions;

#[tauri::command]
pub(crate) fn get_launcher_settings(app: tauri::AppHandle) -> Result<LauncherSettings, String> {
    read_launcher_settings(&app)
}

#[tauri::command]
pub(crate) fn get_dev_mode_state() -> Result<bool, String> {
    Ok(is_dev_mode_enabled())
}

#[tauri::command]
pub(crate) fn set_dev_curseforge_api_key(
    app: tauri::AppHandle,
    args: SetDevCurseforgeApiKeyArgs,
) -> Result<String, String> {
    if !is_dev_mode_enabled() {
        return Err("Dev mode is disabled. Set MPM_DEV_MODE=1 and restart.".to_string());
    }
    let trimmed = args.key.trim().to_string();
    if trimmed.is_empty() {
        return Err("API key cannot be empty.".to_string());
    }
    std::env::set_var(DEV_RUNTIME_CURSEFORGE_API_KEY_ENV, &trimmed);
    let mut notes: Vec<String> = Vec::new();
    if let Err(e) = keyring_set_dev_curseforge_key(&trimmed) {
        notes.push(format!("Secure keychain save failed ({e})."));
    }
    write_dev_curseforge_key_file(&app, &trimmed)?;
    if notes.is_empty() {
        Ok(
            "Saved dev CurseForge API key. It is active immediately for this app session."
                .to_string(),
        )
    } else {
        Ok(format!(
            "Saved dev CurseForge API key to local fallback file and activated it. {}",
            notes.join(" ")
        ))
    }
}

#[tauri::command]
pub(crate) fn clear_dev_curseforge_api_key(app: tauri::AppHandle) -> Result<String, String> {
    if !is_dev_mode_enabled() {
        return Err("Dev mode is disabled. Set MPM_DEV_MODE=1 and restart.".to_string());
    }
    std::env::remove_var(DEV_RUNTIME_CURSEFORGE_API_KEY_ENV);
    let mut notes: Vec<String> = Vec::new();
    if let Err(e) = keyring_delete_dev_curseforge_key() {
        notes.push(format!("Keychain clear failed ({e})."));
    }
    if let Err(e) = clear_dev_curseforge_key_file(&app) {
        notes.push(format!("Local fallback clear failed ({e})."));
    }
    if notes.is_empty() {
        Ok("Cleared saved dev CurseForge API key.".to_string())
    } else {
        Ok(format!("Cleared runtime key. {}", notes.join(" ")))
    }
}

#[tauri::command]
pub(crate) fn get_curseforge_api_status() -> Result<CurseforgeApiStatus, String> {
    let Some((api_key, source)) = curseforge_api_key_with_source() else {
        return Ok(CurseforgeApiStatus {
            configured: false,
            env_var: None,
            key_hint: None,
            validated: false,
            message: "No CurseForge API key configured for this build. Maintainers can inject MPM_CURSEFORGE_API_KEY_BUILTIN at build time, use Dev mode secure key storage, or set MPM_CURSEFORGE_API_KEY for local development.".to_string(),
        });
    };

    let client = build_http_client()?;
    let url = format!(
        "{}/games/{}",
        CURSEFORGE_API_BASE, CURSEFORGE_GAME_ID_MINECRAFT
    );
    let resp = client.get(&url).header("x-api-key", api_key.clone()).send();

    match resp {
        Ok(response) => {
            if response.status().is_success() {
                Ok(CurseforgeApiStatus {
                    configured: true,
                    env_var: Some(source),
                    key_hint: Some(mask_secret(&api_key)),
                    validated: true,
                    message: "CurseForge API key is valid.".to_string(),
                })
            } else {
                let status = response.status().as_u16();
                let body = response.text().unwrap_or_default();
                let trimmed = body.chars().take(220).collect::<String>();
                Ok(CurseforgeApiStatus {
                    configured: true,
                    env_var: Some(source),
                    key_hint: Some(mask_secret(&api_key)),
                    validated: false,
                    message: if trimmed.is_empty() {
                        format!("CurseForge API key validation failed (HTTP {}).", status)
                    } else {
                        format!(
                            "CurseForge API key validation failed (HTTP {}): {}",
                            status, trimmed
                        )
                    },
                })
            }
        }
        Err(e) => Ok(CurseforgeApiStatus {
            configured: true,
            env_var: Some(source),
            key_hint: Some(mask_secret(&api_key)),
            validated: false,
            message: format!(
                "Could not validate CurseForge key right now (network/request error): {}",
                e
            ),
        }),
    }
}

#[tauri::command]
pub(crate) fn set_launcher_settings(
    app: tauri::AppHandle,
    args: SetLauncherSettingsArgs,
) -> Result<LauncherSettings, String> {
    let mut settings = read_launcher_settings(&app)?;
    if let Some(method) = args.default_launch_method {
        let parsed = LaunchMethod::parse(&method)
            .ok_or_else(|| "defaultLaunchMethod must be prism or native".to_string())?;
        settings.default_launch_method = parsed;
    }
    if let Some(java_path) = args.java_path {
        settings.java_path = java_path.trim().to_string();
    }
    if let Some(client_id) = args.oauth_client_id {
        settings.oauth_client_id = client_id.trim().to_string();
    }
    if let Some(cadence) = args.update_check_cadence {
        settings.update_check_cadence = normalize_update_check_cadence(&cadence);
    }
    if let Some(mode) = args.update_auto_apply_mode {
        settings.update_auto_apply_mode = normalize_update_auto_apply_mode(&mode);
    }
    if let Some(scope) = args.update_apply_scope {
        settings.update_apply_scope = normalize_update_apply_scope(&scope);
    }
    if let Some(auto_identify) = args.auto_identify_local_jars {
        settings.auto_identify_local_jars = auto_identify;
    }
    write_launcher_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub(crate) fn list_launcher_accounts(app: tauri::AppHandle) -> Result<Vec<LauncherAccount>, String> {
    read_launcher_accounts(&app)
}

#[tauri::command]
pub(crate) fn select_launcher_account(
    app: tauri::AppHandle,
    args: SelectLauncherAccountArgs,
) -> Result<LauncherSettings, String> {
    let accounts = read_launcher_accounts(&app)?;
    if !accounts.iter().any(|a| a.id == args.account_id) {
        return Err("Account not found".to_string());
    }
    let mut settings = read_launcher_settings(&app)?;
    settings.selected_account_id = Some(args.account_id);
    write_launcher_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub(crate) fn logout_microsoft_account(
    app: tauri::AppHandle,
    args: LogoutMicrosoftAccountArgs,
) -> Result<Vec<LauncherAccount>, String> {
    let mut accounts = read_launcher_accounts(&app)?;
    let removed_account = accounts
        .iter()
        .find(|account| account.id == args.account_id)
        .cloned();
    accounts.retain(|a| a.id != args.account_id);
    write_launcher_accounts(&app, &accounts)?;
    let mut settings = read_launcher_settings(&app)?;
    let removed_selected =
        settings.selected_account_id.as_deref() == Some(args.account_id.as_str());
    if removed_selected {
        settings.selected_account_id = None;
        write_launcher_settings(&app, &settings)?;
    }
    if let Some(account) = removed_account.as_ref() {
        if let Err(e) = keyring_delete_refresh_token_for_account(account) {
            eprintln!(
                "keyring delete failed for account {}: {}",
                args.account_id, e
            );
        }
        if let Err(e) = remove_refresh_token_recovery_fallback(&app, account) {
            eprintln!(
                "refresh-token recovery fallback cleanup failed for account {}: {}",
                args.account_id, e
            );
        }
        #[cfg(debug_assertions)]
        if let Err(e) = remove_refresh_token_debug_fallback(&app, account) {
            eprintln!(
                "debug refresh-token fallback cleanup failed for account {}: {}",
                args.account_id, e
            );
        }
    } else {
        delete_refresh_token_everywhere(&app, &args.account_id);
        if let Err(e) = remove_refresh_token_recovery_fallback_for_key(&app, &args.account_id) {
            eprintln!(
                "refresh-token recovery fallback cleanup failed for account key {}: {}",
                args.account_id, e
            );
        }
    }
    if removed_selected {
        if let Err(e) = keyring_delete_selected_refresh_token() {
            eprintln!("keyring delete failed for selected refresh alias: {}", e);
        }
    }
    Ok(accounts)
}

#[tauri::command]
pub(crate) async fn begin_microsoft_login(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BeginMicrosoftLoginResult, String> {
    let (client_id, client_id_source) = resolve_oauth_client_id_with_source(&app)?;
    let session_id = format!("ms_{}", Uuid::new_v4());
    let client_id_for_flow = client_id.clone();
    let flow = run_blocking_task("microsoft device code", move || {
        let client = build_http_client()?;
        microsoft_begin_device_code(&client, &client_id_for_flow)
    })
    .await?;
    let verification_uri = flow.verification_uri.clone();
    let user_code = flow.user_code.clone();
    let interval = if flow.interval == 0 { 5 } else { flow.interval };
    let expires_in = if flow.expires_in == 0 {
        900
    } else {
        flow.expires_in
    };
    let pending_message = flow
        .message
        .clone()
        .unwrap_or_else(|| format!("Open {} and enter code {}", verification_uri, user_code));

    set_login_session_state(
        &state.login_sessions,
        &session_id,
        "pending",
        Some(pending_message),
        None,
    );

    let sessions = state.login_sessions.clone();
    let app_for_thread = app.clone();
    let session_id_for_thread = session_id.clone();
    let client_id_for_thread = client_id.clone();
    let client_id_source_for_thread = client_id_source.clone();
    thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(expires_in + 20);
        let mut poll_interval_secs = interval.max(2);

        let client = match build_http_client() {
            Ok(c) => c,
            Err(e) => {
                set_login_session_state(
                    &sessions,
                    &session_id_for_thread,
                    "error",
                    Some(format!("build http client failed: {e}")),
                    None,
                );
                return;
            }
        };

        loop {
            if Instant::now() >= deadline {
                set_login_session_state(
                    &sessions,
                    &session_id_for_thread,
                    "error",
                    Some("Microsoft login timed out. Please try again.".to_string()),
                    None,
                );
                return;
            }

            let params = [
                ("client_id", client_id_for_thread.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", flow.device_code.as_str()),
            ];
            let response = match client
                .post(MS_TOKEN_URL)
                .header("Accept", "application/json")
                .form(&params)
                .send()
            {
                Ok(r) => r,
                Err(e) => {
                    set_login_session_state(
                        &sessions,
                        &session_id_for_thread,
                        "error",
                        Some(format!("Microsoft device token polling failed: {e}")),
                        None,
                    );
                    return;
                }
            };

            if response.status().is_success() {
                let token = match response.json::<MsoTokenResponse>() {
                    Ok(v) => v,
                    Err(e) => {
                        set_login_session_state(
                            &sessions,
                            &session_id_for_thread,
                            "error",
                            Some(format!("parse Microsoft device token response failed: {e}")),
                            None,
                        );
                        return;
                    }
                };

                let result = (|| -> Result<LauncherAccount, String> {
                    let refresh = token.refresh_token.ok_or_else(|| {
                        "Microsoft login did not return refresh token.".to_string()
                    })?;
                    let mc_access = microsoft_access_to_mc_token(&client, &token.access_token)?;
                    ensure_minecraft_entitlement(&client, &mc_access)?;
                    let profile = fetch_minecraft_profile(&client, &mc_access)?;
                    let account = LauncherAccount {
                        id: profile.id,
                        username: profile.name,
                        added_at: now_iso(),
                    };
                    persist_refresh_token_for_launcher_account_with_app(
                        &app_for_thread,
                        &account,
                        &refresh,
                    )?;
                    upsert_launcher_account(&app_for_thread, &account)?;

                    let mut settings = read_launcher_settings(&app_for_thread)?;
                    settings.selected_account_id = Some(account.id.clone());
                    write_launcher_settings(&app_for_thread, &settings)?;
                    Ok(account)
                })();

                match result {
                    Ok(account) => {
                        set_login_session_state(
                            &sessions,
                            &session_id_for_thread,
                            "success",
                            Some("Microsoft account connected.".to_string()),
                            Some(account),
                        );
                    }
                    Err(err) => {
                        set_login_session_state(
                            &sessions,
                            &session_id_for_thread,
                            "error",
                            Some(err),
                            None,
                        );
                    }
                }
                return;
            }

            let err_body = response
                .text()
                .unwrap_or_else(|_| "unknown token polling error".to_string());
            let parsed = serde_json::from_str::<serde_json::Value>(&err_body).ok();
            let err_code = parsed
                .as_ref()
                .and_then(|v| v.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let err_desc = parsed
                .as_ref()
                .and_then(|v| v.get("error_description"))
                .and_then(|v| v.as_str())
                .unwrap_or(err_body.as_str());

            if err_code.eq_ignore_ascii_case("authorization_pending") {
                thread::sleep(Duration::from_secs(poll_interval_secs));
                continue;
            }
            if err_code.eq_ignore_ascii_case("slow_down") {
                poll_interval_secs = (poll_interval_secs + 2).min(15);
                thread::sleep(Duration::from_secs(poll_interval_secs));
                continue;
            }
            if err_code.eq_ignore_ascii_case("authorization_declined")
                || err_code.eq_ignore_ascii_case("bad_verification_code")
                || err_code.eq_ignore_ascii_case("expired_token")
            {
                set_login_session_state(
                    &sessions,
                    &session_id_for_thread,
                    "error",
                    Some(format!("Microsoft login cancelled/expired: {err_desc}")),
                    None,
                );
                return;
            }

            set_login_session_state(
                &sessions,
                &session_id_for_thread,
                "error",
                Some(normalize_microsoft_login_error(
                    err_code,
                    err_desc,
                    &client_id_source_for_thread,
                )),
                None,
            );
            return;
        }
    });

    if let Err(e) = tauri::api::shell::open(&app.shell_scope(), verification_uri.clone(), None) {
        set_login_session_state(
            &state.login_sessions,
            &session_id,
            "pending",
            Some(format!(
                "Open {} and enter code {} (browser auto-open failed: {})",
                verification_uri, user_code, e
            )),
            None,
        );
    }

    Ok(BeginMicrosoftLoginResult {
        session_id,
        auth_url: verification_uri.clone(),
        user_code: Some(user_code),
        verification_uri: Some(verification_uri),
    })
}

#[tauri::command]
pub(crate) fn poll_microsoft_login(
    state: tauri::State<AppState>,
    args: PollMicrosoftLoginArgs,
) -> Result<MicrosoftLoginState, String> {
    let guard = state
        .login_sessions
        .lock()
        .map_err(|_| "lock login sessions failed".to_string())?;
    guard
        .get(&args.session_id)
        .cloned()
        .ok_or_else(|| "login session not found".to_string())
}

#[tauri::command]
pub(crate) fn list_running_instances(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<Vec<RunningInstance>, String> {
    let mut guard = state
        .running
        .lock()
        .map_err(|_| "lock running instances failed".to_string())?;
    let mut finished: Vec<String> = Vec::new();
    for (id, proc_entry) in guard.iter_mut() {
        if let Ok(mut child) = proc_entry.child.lock() {
            if let Ok(Some(status)) = child.try_wait() {
                finished.push(id.clone());
                emit_launch_state(
                    &app,
                    &proc_entry.meta.instance_id,
                    Some(&proc_entry.meta.launch_id),
                    &proc_entry.meta.method,
                    "exited",
                    &format!("Game exited with status {:?}", status.code()),
                );
            }
        }
    }
    for id in finished {
        guard.remove(&id);
    }
    let mut out: Vec<RunningInstance> = guard
        .values()
        .map(|v| {
            let mut meta = v.meta.clone();
            if meta.log_path.is_none() {
                meta.log_path = v.log_path.as_ref().map(|p| p.display().to_string());
            }
            meta
        })
        .collect();
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(out)
}

#[tauri::command]
pub(crate) fn stop_running_instance(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: StopRunningInstanceArgs,
) -> Result<(), String> {
    let removed = {
        let mut guard = state
            .running
            .lock()
            .map_err(|_| "lock running instances failed".to_string())?;
        guard.remove(&args.launch_id)
    };
    let Some(proc_entry) = removed else {
        return Err("Running instance not found".to_string());
    };
    {
        let mut stop_guard = state
            .stop_requested_launches
            .lock()
            .map_err(|_| "lock stop requested launches failed".to_string())?;
        stop_guard.insert(args.launch_id.clone());
    }
    if let Ok(mut child) = proc_entry.child.lock() {
        let _ = child.kill();
    }
    emit_launch_state(
        &app,
        &proc_entry.meta.instance_id,
        Some(&proc_entry.meta.launch_id),
        &proc_entry.meta.method,
        "stopped",
        "Instance stop requested.",
    );
    Ok(())
}

#[tauri::command]
pub(crate) fn cancel_instance_launch(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: CancelInstanceLaunchArgs,
) -> Result<String, String> {
    let instance_id = args.instance_id.trim();
    if instance_id.is_empty() {
        return Err("instanceId is required".to_string());
    }

    mark_launch_cancel_request(&state, instance_id)?;

    let mut stopped_any = false;
    let removed = {
        let mut guard = state
            .running
            .lock()
            .map_err(|_| "lock running instances failed".to_string())?;
        let keys = guard
            .iter()
            .filter_map(|(id, proc_entry)| {
                if proc_entry.meta.instance_id == instance_id {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut removed_entries = Vec::new();
        for key in keys {
            if let Some(entry) = guard.remove(&key) {
                removed_entries.push(entry);
            }
        }
        removed_entries
    };

    for proc_entry in removed {
        stopped_any = true;
        if let Ok(mut stop_guard) = state.stop_requested_launches.lock() {
            stop_guard.insert(proc_entry.meta.launch_id.clone());
        }
        if let Ok(mut child) = proc_entry.child.lock() {
            let _ = child.kill();
        }
        emit_launch_state(
            &app,
            &proc_entry.meta.instance_id,
            Some(&proc_entry.meta.launch_id),
            &proc_entry.meta.method,
            "stopped",
            "Launch cancelled by user.",
        );
    }

    if stopped_any {
        Ok("Launch cancellation requested. Stop signal sent.".to_string())
    } else {
        Ok("Launch cancellation requested.".to_string())
    }
}

#[tauri::command]
pub(crate) fn open_instance_path(
    app: tauri::AppHandle,
    args: OpenInstancePathArgs,
) -> Result<OpenInstancePathResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let (target, resolved_path, create_if_missing) =
        resolve_target_instance_path(&instance_dir, &args.target)?;
    open_path_in_shell(&resolved_path, create_if_missing)?;
    Ok(OpenInstancePathResult {
        target,
        path: resolved_path.display().to_string(),
    })
}

#[tauri::command]
pub(crate) fn reveal_config_editor_file(
    app: tauri::AppHandle,
    args: RevealConfigEditorFileArgs,
) -> Result<RevealConfigEditorFileResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let scope = args.scope.trim().to_lowercase();

    if scope == "instance" {
        let (opened, _) = reveal_path_in_shell(&instance_dir, false)?;
        return Ok(RevealConfigEditorFileResult {
            opened_path: opened.display().to_string(),
            revealed_file: false,
            virtual_file: true,
            message:
                "Instance config files are localStorage-backed. Opened the instance folder instead."
                    .to_string(),
        });
    }

    if scope == "world" {
        let world_id = args
            .world_id
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "worldId is required for world scope".to_string())?;
        let file_path = args
            .path
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "path is required for world scope".to_string())?;
        let world_root = world_root_dir(&instances_dir, &args.instance_id, &world_id)?;
        let (resolved, _) = resolve_world_file_path(&world_root, &file_path, true)?;
        let (opened, revealed_file) = reveal_path_in_shell(&resolved, true)?;
        return Ok(RevealConfigEditorFileResult {
            opened_path: opened.display().to_string(),
            revealed_file,
            virtual_file: false,
            message: if revealed_file {
                "Revealed file in your file manager.".to_string()
            } else {
                "Opened containing folder.".to_string()
            },
        });
    }

    Err("scope must be instance or world".to_string())
}

#[tauri::command]
pub(crate) fn read_instance_logs(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: ReadInstanceLogsArgs,
) -> Result<ReadInstanceLogsResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let source_raw = args.source.trim().to_lowercase();
    let max_lines = args.max_lines.unwrap_or(2500).clamp(200, 12000);
    let before_line = args.before_line;

    let (source, path) = match source_raw.as_str() {
        "latest_crash" | "latest-crash" | "crash" => (
            "latest_crash".to_string(),
            latest_crash_report_path(&instance_dir),
        ),
        "latest_launch" | "latest-launch" | "launch" => (
            "latest_launch".to_string(),
            latest_launch_log_path(&instance_dir),
        ),
        "live" => {
            let guard = state
                .running
                .lock()
                .map_err(|_| "lock running instances failed".to_string())?;
            let mut best: Option<(String, PathBuf)> = None;
            for proc_entry in guard.values() {
                if proc_entry.meta.instance_id != args.instance_id
                    || !proc_entry.meta.method.eq_ignore_ascii_case("native")
                {
                    continue;
                }
                let Some(path) = proc_entry.log_path.clone() else {
                    continue;
                };
                match &best {
                    Some((started_at, _))
                        if started_at.as_str() >= proc_entry.meta.started_at.as_str() => {}
                    _ => {
                        best = Some((proc_entry.meta.started_at.clone(), path));
                    }
                }
            }
            (
                "live".to_string(),
                best.map(|(_, path)| path)
                    .or_else(|| latest_launch_log_path(&instance_dir)),
            )
        }
        _ => return Err("source must be live, latest_launch, or latest_crash".to_string()),
    };

    let Some(path) = path else {
        return Ok(ReadInstanceLogsResult {
            source,
            path: String::new(),
            available: false,
            total_lines: 0,
            returned_lines: 0,
            truncated: false,
            start_line_no: None,
            end_line_no: None,
            next_before_line: None,
            lines: Vec::new(),
            updated_at: 0,
            message: Some("No log file found for this source yet.".to_string()),
        });
    };

    if !path.exists() || !path.is_file() {
        return Ok(ReadInstanceLogsResult {
            source,
            path: path.display().to_string(),
            available: false,
            total_lines: 0,
            returned_lines: 0,
            truncated: false,
            start_line_no: None,
            end_line_no: None,
            next_before_line: None,
            lines: Vec::new(),
            updated_at: 0,
            message: Some("Log file does not exist yet.".to_string()),
        });
    }

    let (lines, total_lines, truncated, start_line_no, end_line_no, next_before_line) =
        read_windowed_log_lines(&path, &source, max_lines, before_line)?;
    let updated_at = fs::metadata(&path)
        .map(|meta| modified_millis(&meta))
        .unwrap_or(0);
    Ok(ReadInstanceLogsResult {
        source,
        path: path.display().to_string(),
        available: true,
        total_lines,
        returned_lines: lines.len(),
        truncated,
        start_line_no,
        end_line_no,
        next_before_line,
        lines,
        updated_at,
        message: None,
    })
}

#[tauri::command]
pub(crate) async fn list_instance_snapshots(
    app: tauri::AppHandle,
    args: ListInstanceSnapshotsArgs,
) -> Result<Vec<SnapshotMeta>, String> {
    run_blocking_task("list instance snapshots", move || {
        list_instance_snapshots_inner(app, args)
    })
    .await
}

fn list_instance_snapshots_inner(
    app: tauri::AppHandle,
    args: ListInstanceSnapshotsArgs,
) -> Result<Vec<SnapshotMeta>, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    list_snapshots(&instance_dir)
}

#[tauri::command]
pub(crate) fn rollback_instance(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: RollbackInstanceArgs,
) -> Result<RollbackResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    {
        let guard = state
            .running
            .lock()
            .map_err(|_| "lock running instances failed".to_string())?;
        if guard
            .values()
            .any(|entry| entry.meta.instance_id == args.instance_id)
        {
            return Err(
                "Stop the running Minecraft session before rolling back this instance.".to_string(),
            );
        }
    }
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let snapshots = list_snapshots(&instance_dir)?;
    if snapshots.is_empty() {
        return Err("No snapshots found for this instance".to_string());
    }
    let selected = if let Some(snapshot_id) = args.snapshot_id.as_ref() {
        snapshots
            .into_iter()
            .find(|s| s.id == *snapshot_id)
            .ok_or_else(|| "Snapshot not found".to_string())?
    } else {
        snapshots
            .into_iter()
            .next()
            .ok_or_else(|| "No snapshots found for this instance".to_string())?
    };

    let snapshot_dir = snapshots_dir(&instance_dir).join(&selected.id);
    let lock_raw = fs::read_to_string(snapshot_lock_path(&snapshot_dir))
        .map_err(|e| format!("read snapshot lock failed: {e}"))?;
    let lock: Lockfile =
        serde_json::from_str(&lock_raw).map_err(|e| format!("parse snapshot lock failed: {e}"))?;

    let restored_files =
        restore_instance_content_zip(&snapshot_content_zip_path(&snapshot_dir), &instance_dir)?;
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;

    Ok(RollbackResult {
        snapshot_id: selected.id,
        created_at: selected.created_at,
        restored_files,
        message: "Rollback complete.".to_string(),
    })
}

#[tauri::command]
pub(crate) async fn list_instance_worlds(
    app: tauri::AppHandle,
    args: ListInstanceWorldsArgs,
) -> Result<Vec<InstanceWorld>, String> {
    run_blocking_task("list instance worlds", move || {
        list_instance_worlds_inner(app, args)
    })
    .await
}

fn list_instance_worlds_inner(
    app: tauri::AppHandle,
    args: ListInstanceWorldsArgs,
) -> Result<Vec<InstanceWorld>, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let saves_dir = instance_dir.join("saves");
    fs::create_dir_all(&saves_dir).map_err(|e| format!("mkdir saves failed: {e}"))?;

    let world_backups = list_world_backups(&instance_dir).unwrap_or_default();
    let mut backup_count_by_world: HashMap<String, usize> = HashMap::new();
    let mut latest_backup_by_world: HashMap<String, WorldBackupMeta> = HashMap::new();
    for meta in world_backups {
        *backup_count_by_world
            .entry(meta.world_id.clone())
            .or_insert(0) += 1;
        latest_backup_by_world
            .entry(meta.world_id.clone())
            .or_insert(meta);
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
        if name.trim().is_empty() {
            continue;
        }
        let latest = latest_backup_by_world.get(&name);
        out.push(InstanceWorld {
            id: name.clone(),
            name: name.clone(),
            path: path.display().to_string(),
            latest_backup_id: latest.map(|m| m.id.clone()),
            latest_backup_at: latest.map(|m| m.created_at.clone()),
            backup_count: backup_count_by_world.get(&name).copied().unwrap_or(0),
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub(crate) async fn get_instance_disk_usage(
    app: tauri::AppHandle,
    args: GetInstanceDiskUsageArgs,
) -> Result<u64, String> {
    run_blocking_task("get instance disk usage", move || {
        let instances_dir = app_instances_dir(&app)?;
        let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
        Ok(dir_total_size_bytes(&instance_dir))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_instance_last_run_metadata(
    app: tauri::AppHandle,
    args: GetInstanceLastRunMetadataArgs,
) -> Result<InstanceLastRunMetadata, String> {
    run_blocking_task("get instance last-run metadata", move || {
        let instances_dir = app_instances_dir(&app)?;
        read_instance_last_run_metadata(&instances_dir, &args.instance_id)
    })
    .await
}

fn running_instance_ids(state: &tauri::State<AppState>) -> Result<HashSet<String>, String> {
    let guard = state
        .running
        .lock()
        .map_err(|_| "lock running instances failed".to_string())?;
    Ok(guard
        .values()
        .map(|entry| entry.meta.instance_id.clone())
        .collect::<HashSet<_>>())
}

fn collect_world_config_files_recursive(
    world_root: &Path,
    current: &Path,
    out: &mut Vec<WorldConfigFileEntry>,
) -> Result<(), String> {
    let entries = fs::read_dir(current).map_err(|e| format!("read world directory failed: {e}"))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("read world entry failed: {e}"))?;
        let path = ent.path();
        let meta =
            fs::symlink_metadata(&path).map_err(|e| format!("read world metadata failed: {e}"))?;
        let file_type = meta.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_world_config_files_recursive(world_root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(world_root)
            .map_err(|_| "failed to compute relative world file path".to_string())?;
        let rel_text = rel
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        if rel_text.is_empty() {
            continue;
        }

        let mut sample = Vec::new();
        if let Ok(mut file) = File::open(&path) {
            let mut buf = [0u8; 1024];
            if let Ok(read_len) = file.read(&mut buf) {
                sample.extend_from_slice(&buf[..read_len]);
            }
        }
        let text_like = file_is_text_like(&path, &sample);
        let kind = infer_world_file_kind(&path, text_like);
        let readonly_reason = describe_non_editable_reason(&kind, text_like);
        out.push(WorldConfigFileEntry {
            path: rel_text,
            size_bytes: meta.len(),
            modified_at: modified_millis(&meta),
            editable: readonly_reason.is_none(),
            kind,
            readonly_reason,
        });
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn list_world_config_files(
    app: tauri::AppHandle,
    args: ListWorldConfigFilesArgs,
) -> Result<Vec<WorldConfigFileEntry>, String> {
    let instances_dir = app_instances_dir(&app)?;
    let world_root = world_root_dir(&instances_dir, &args.instance_id, &args.world_id)?;
    let mut out = Vec::new();
    collect_world_config_files_recursive(&world_root, &world_root, &mut out)?;
    out.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub(crate) fn read_world_config_file(
    app: tauri::AppHandle,
    args: ReadWorldConfigFileArgs,
) -> Result<ReadWorldConfigFileResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let world_root = world_root_dir(&instances_dir, &args.instance_id, &args.world_id)?;
    let (resolved_path, normalized_path) = resolve_world_file_path(&world_root, &args.path, true)?;
    let meta = fs::metadata(&resolved_path)
        .map_err(|e| format!("read world file metadata failed: {e}"))?;
    if !meta.is_file() {
        return Err("Requested world file is not a file".to_string());
    }

    let mut file =
        File::open(&resolved_path).map_err(|e| format!("open world file failed: {e}"))?;
    let mut sample_buf = vec![0u8; 4096];
    let read_len = file
        .read(&mut sample_buf)
        .map_err(|e| format!("read world file failed: {e}"))?;
    sample_buf.truncate(read_len);
    let text_like = file_is_text_like(&resolved_path, &sample_buf[..sample_buf.len().min(1024)]);
    let kind = infer_world_file_kind(&resolved_path, text_like);
    let readonly_reason = describe_non_editable_reason(&kind, text_like);
    if readonly_reason.is_some() {
        let preview =
            format_binary_preview(&sample_buf[..sample_buf.len().min(512)], meta.len(), &kind);
        return Ok(ReadWorldConfigFileResult {
            path: normalized_path,
            editable: false,
            kind,
            size_bytes: meta.len(),
            modified_at: modified_millis(&meta),
            readonly_reason,
            content: Some(preview),
            preview: Some("hex".to_string()),
        });
    }

    let mut bytes = sample_buf;
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("read world file failed: {e}"))?;
    let content =
        String::from_utf8(bytes).map_err(|_| "File is not valid UTF-8 text.".to_string())?;
    Ok(ReadWorldConfigFileResult {
        path: normalized_path,
        editable: true,
        kind,
        size_bytes: meta.len(),
        modified_at: modified_millis(&meta),
        readonly_reason: None,
        content: Some(content),
        preview: None,
    })
}

#[tauri::command]
pub(crate) fn write_world_config_file(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: WriteWorldConfigFileArgs,
) -> Result<WriteWorldConfigFileResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let running_ids = running_instance_ids(&state)?;
    if running_ids.contains(&args.instance_id) {
        return Err("Stop the running Minecraft session before saving world files.".to_string());
    }

    let world_root = world_root_dir(&instances_dir, &args.instance_id, &args.world_id)?;
    let (resolved_path, normalized_path) = resolve_world_file_path(&world_root, &args.path, true)?;
    let before_meta = fs::metadata(&resolved_path)
        .map_err(|e| format!("read world file metadata failed: {e}"))?;
    if !before_meta.is_file() {
        return Err("Requested world file is not a file".to_string());
    }
    if let Some(expected_modified_at) = args.expected_modified_at {
        let actual_modified_at = modified_millis(&before_meta);
        if expected_modified_at != actual_modified_at {
            return Err("File changed on disk. Reload and try saving again.".to_string());
        }
    }

    let mut sample = args.content.as_bytes().to_vec();
    if sample.len() > 1024 {
        sample.truncate(1024);
    }
    let text_like = file_is_text_like(&resolved_path, &sample);
    let kind = infer_world_file_kind(&resolved_path, text_like);
    if describe_non_editable_reason(&kind, text_like).is_some() {
        return Err("Binary or unsupported world file cannot be edited.".to_string());
    }

    let parent = resolved_path
        .parent()
        .ok_or_else(|| "Invalid world file path".to_string())?;
    let tmp_name = format!(".mpm-write-{}.tmp", Uuid::new_v4());
    let tmp_path = parent.join(tmp_name);
    fs::write(&tmp_path, args.content.as_bytes())
        .map_err(|e| format!("write temp world file failed: {e}"))?;
    if let Err(err) = fs::rename(&tmp_path, &resolved_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("replace world file failed: {err}"));
    }
    let after_meta = fs::metadata(&resolved_path)
        .map_err(|e| format!("read world file metadata failed: {e}"))?;
    Ok(WriteWorldConfigFileResult {
        path: normalized_path,
        size_bytes: after_meta.len(),
        modified_at: modified_millis(&after_meta),
        message: "World file saved.".to_string(),
    })
}

#[tauri::command]
pub(crate) fn rollback_instance_world_backup(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    args: RollbackInstanceWorldBackupArgs,
) -> Result<WorldRollbackResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    {
        let guard = state
            .running
            .lock()
            .map_err(|_| "lock running instances failed".to_string())?;
        if guard
            .values()
            .any(|entry| entry.meta.instance_id == args.instance_id)
        {
            return Err(
                "Stop the running Minecraft session before rolling back this world.".to_string(),
            );
        }
    }
    let world_id = args.world_id.trim();
    if world_id.is_empty() {
        return Err("World ID is required".to_string());
    }
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let backups = list_world_backups(&instance_dir)?;
    let selected = if let Some(backup_id) = args.backup_id.as_ref() {
        backups
            .into_iter()
            .find(|b| b.world_id == world_id && b.id == *backup_id)
            .ok_or_else(|| "World backup not found".to_string())?
    } else {
        backups
            .into_iter()
            .find(|b| b.world_id == world_id)
            .ok_or_else(|| "No world backup found for this world yet".to_string())?
    };

    let backup_dir = world_backups_dir(&instance_dir).join(&selected.id);
    let world_dir = instance_dir.join("saves").join(world_id);
    let restored_files = restore_world_backup_zip(&world_backup_zip_path(&backup_dir), &world_dir)?;
    Ok(WorldRollbackResult {
        world_id: world_id.to_string(),
        backup_id: selected.id.clone(),
        created_at: selected.created_at.clone(),
        restored_files,
        message: "World rollback complete.".to_string(),
    })
}

fn install_discover_content_inner(
    app: tauri::AppHandle,
    args: &InstallDiscoverContentArgs,
    snapshot_reason: Option<&str>,
) -> Result<InstalledMod, String> {
    let source = args.source.trim().to_lowercase();
    let content_type = normalize_lock_content_type(&args.content_type);
    if content_type == "modpacks" {
        return Err(
            "Modpacks are template-only here. Use Import as Template in Modpacks & Presets."
                .to_string(),
        );
    }

    if content_type == "mods" {
        if source == "curseforge" {
            return install_curseforge_mod_inner(
                app,
                InstallCurseforgeModArgs {
                    instance_id: args.instance_id.clone(),
                    project_id: args.project_id.clone(),
                    project_title: args.project_title.clone(),
                },
                snapshot_reason,
            );
        }
        let modrinth_reason = snapshot_reason;
        return install_modrinth_mod_inner(
            app,
            InstallModrinthModArgs {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                project_title: args.project_title.clone(),
            },
            modrinth_reason,
        );
    }

    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let content_label = content_type_display_name(&content_type);
    let source_label = if source == "curseforge" {
        "CurseForge"
    } else {
        "Modrinth"
    };

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "resolving".to_string(),
            downloaded: 0,
            total: Some(1),
            percent: None,
            message: Some(format!(
                "Resolving compatible {source_label} {content_label} file…"
            )),
        },
    );

    if let Some(reason) = snapshot_reason {
        let _ = create_instance_snapshot(&instances_dir, &args.instance_id, reason);
    }

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "downloading".to_string(),
            downloaded: 0,
            total: Some(1),
            percent: Some(0.8),
            message: Some(format!("Starting {source_label} {content_label} download…")),
        },
    );

    let new_entry = if source == "curseforge" {
        let api_key = curseforge_api_key().ok_or_else(missing_curseforge_key_message)?;
        install_curseforge_content_inner(
            &instance,
            &instance_dir,
            &mut lock,
            &client,
            &api_key,
            &args.project_id,
            args.project_title.as_deref(),
            &content_type,
            &args.target_worlds,
            None,
            |downloaded_bytes, total_bytes| {
                let ratio = match total_bytes {
                    Some(total) if total > 0 => downloaded_bytes as f64 / total as f64,
                    _ => unknown_progress_ratio(downloaded_bytes),
                };
                let visible_percent = if downloaded_bytes > 0 {
                    (ratio * 100.0).max(0.8)
                } else {
                    0.8
                };
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "downloading".to_string(),
                        downloaded: downloaded_bytes,
                        total: total_bytes,
                        percent: Some(visible_percent.clamp(0.0, 99.4)),
                        message: Some(format!(
                            "Downloading {source_label} {content_label}… · {}",
                            format_download_meter(downloaded_bytes, total_bytes)
                        )),
                    },
                );
            },
        )?
    } else {
        install_modrinth_content_inner(
            &instance,
            &instance_dir,
            &mut lock,
            &client,
            &args.project_id,
            args.project_title.as_deref(),
            &content_type,
            &args.target_worlds,
            |downloaded_bytes, total_bytes| {
                let ratio = match total_bytes {
                    Some(total) if total > 0 => downloaded_bytes as f64 / total as f64,
                    _ => unknown_progress_ratio(downloaded_bytes),
                };
                let visible_percent = if downloaded_bytes > 0 {
                    (ratio * 100.0).max(0.8)
                } else {
                    0.8
                };
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "downloading".to_string(),
                        downloaded: downloaded_bytes,
                        total: total_bytes,
                        percent: Some(visible_percent.clamp(0.0, 99.4)),
                        message: Some(format!(
                            "Downloading {source_label} {content_label}… · {}",
                            format_download_meter(downloaded_bytes, total_bytes)
                        )),
                    },
                );
            },
        )?
    };

    lock.entries
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;
    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "completed".to_string(),
            downloaded: 1,
            total: Some(1),
            percent: Some(100.0),
            message: Some(format!(
                "{} {} install complete",
                source_label, content_label
            )),
        },
    );
    Ok(lock_entry_to_installed(&instance_dir, &new_entry))
}

#[tauri::command]
pub(crate) async fn install_discover_content(
    app: tauri::AppHandle,
    args: InstallDiscoverContentArgs,
) -> Result<InstalledMod, String> {
    run_blocking_task("install discover content", move || {
        let subject = snapshot_install_subject(args.project_title.as_deref(), &args.project_id);
        let reason = format!("before-install-discover:{subject}");
        install_discover_content_inner(app, &args, Some(reason.as_str()))
    })
    .await
}

#[tauri::command]
pub(crate) fn preview_preset_apply(
    app: tauri::AppHandle,
    args: PreviewPresetApplyArgs,
) -> Result<PresetApplyPreview, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let spec = modpack::legacy_creator_preset_to_spec(&args.preset);
    let plan = modpack::resolver::resolve_modpack(
        &client,
        &instance,
        &spec,
        Some("recommended"),
        Some(spec.settings.clone()),
    )?;

    let skipped_disabled = args.preset.entries.iter().filter(|e| !e.enabled).count();
    let installable = plan.resolved_mods.len();
    let missing_world_targets = plan
        .failed_mods
        .iter()
        .filter(|f| {
            f.reason_code.eq_ignore_ascii_case("NoWorldTargets")
                || f.reason_text.to_lowercase().contains("world")
        })
        .map(|f| format!("{} ({})", f.name, f.project_id))
        .collect::<Vec<_>>();

    let mut provider_warnings = plan.warnings.clone();
    provider_warnings.extend(
        plan.failed_mods
            .iter()
            .filter(|f| f.required)
            .map(|f| format!("{}: {}", f.name, f.reason_text)),
    );
    provider_warnings.sort();
    provider_warnings.dedup();

    let duplicates = plan.conflicts.len();
    let valid = plan.failed_mods.iter().all(|f| !f.required);
    Ok(PresetApplyPreview {
        valid,
        installable_entries: installable,
        skipped_disabled_entries: skipped_disabled,
        missing_world_targets,
        provider_warnings,
        duplicate_entries: duplicates,
    })
}

#[tauri::command]
pub(crate) async fn apply_preset_to_instance(
    app: tauri::AppHandle,
    args: ApplyPresetToInstanceArgs,
) -> Result<PresetApplyResult, String> {
    run_blocking_task("apply preset to instance", move || {
        apply_preset_to_instance_inner(app, args)
    })
    .await
}

fn apply_preset_to_instance_inner(
    app: tauri::AppHandle,
    args: ApplyPresetToInstanceArgs,
) -> Result<PresetApplyResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let spec = modpack::legacy_creator_preset_to_spec(&args.preset);
    let plan = modpack::resolver::resolve_modpack(
        &client,
        &instance,
        &spec,
        Some("recommended"),
        Some(spec.settings.clone()),
    )?;

    let legacy_skipped = args.preset.entries.iter().filter(|e| !e.enabled).count();
    let (result, _lock_snapshot, _link) =
        modpack::apply::apply_plan_to_instance(&app, &plan, "unlinked", false)?;

    let mut by_content_type: HashMap<String, usize> = HashMap::new();
    for item in &plan.resolved_mods {
        *by_content_type
            .entry(normalize_lock_content_type(&item.content_type))
            .or_insert(0) += 1;
    }

    Ok(PresetApplyResult {
        message: result.message,
        installed_entries: result.applied_entries,
        skipped_entries: result.skipped_entries + legacy_skipped,
        failed_entries: result.failed_entries,
        snapshot_id: result.snapshot_id,
        by_content_type,
    })
}

#[tauri::command]
pub(crate) async fn search_discover_content(
    args: SearchDiscoverContentArgs,
) -> Result<DiscoverSearchResult, String> {
    run_blocking_task("search discover content", move || {
        std::panic::catch_unwind(|| search_discover_content_inner(args))
            .map_err(|_| "Discover search encountered an unexpected error".to_string())?
    })
    .await
}

fn search_discover_content_inner(
    args: SearchDiscoverContentArgs,
) -> Result<DiscoverSearchResult, String> {
    let source = args.source.trim().to_lowercase();
    let normalized_content_type = normalize_discover_content_type(&args.content_type);
    let client = build_http_client()?;
    if source == "modrinth" {
        return search_modrinth_discover(&client, &args);
    }
    if source == "curseforge" {
        return search_curseforge_discover(&client, &args);
    }

    let mut sub = args.clone();
    sub.offset = 0;
    sub.limit = (args.offset + args.limit).max(args.limit);

    let modrinth = search_modrinth_discover(&client, &sub).unwrap_or(DiscoverSearchResult {
        hits: vec![],
        offset: 0,
        limit: sub.limit,
        total_hits: 0,
    });

    let curseforge = if curseforge_api_key().is_some() {
        search_curseforge_discover(&client, &sub).unwrap_or(DiscoverSearchResult {
            hits: vec![],
            offset: 0,
            limit: sub.limit,
            total_hits: 0,
        })
    } else {
        DiscoverSearchResult {
            hits: vec![],
            offset: 0,
            limit: sub.limit,
            total_hits: 0,
        }
    };

    let mut merged = modrinth.hits;
    merged.extend(curseforge.hits);
    sort_discover_hits(&mut merged, &args.index);
    if source == "all" && normalized_content_type == "mods" {
        merged = blend_discover_hits_prefer_modrinth(merged);
    }
    let total_hits = modrinth.total_hits.saturating_add(curseforge.total_hits);
    let hits = merged
        .into_iter()
        .skip(args.offset)
        .take(args.limit)
        .collect::<Vec<_>>();

    Ok(DiscoverSearchResult {
        hits,
        offset: args.offset,
        limit: args.limit,
        total_hits,
    })
}

#[tauri::command]
pub(crate) async fn get_curseforge_project_detail(
    args: GetCurseforgeProjectArgs,
) -> Result<CurseforgeProjectDetail, String> {
    run_blocking_task("curseforge project detail", move || {
        get_curseforge_project_detail_inner(args)
    })
    .await
}

fn get_curseforge_project_detail_inner(
    args: GetCurseforgeProjectArgs,
) -> Result<CurseforgeProjectDetail, String> {
    let api_key = curseforge_api_key().ok_or_else(missing_curseforge_key_message)?;
    let project_id = parse_curseforge_project_id(&args.project_id)?;
    let client = build_http_client()?;

    let mod_resp = client
        .get(format!("{}/mods/{}", CURSEFORGE_API_BASE, project_id))
        .header("Accept", "application/json")
        .header("x-api-key", api_key.clone())
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
    let detail_content_type =
        infer_curseforge_project_content_type(&project, args.content_type.as_deref());

    let desc_url = format!("{}/mods/{}/description", CURSEFORGE_API_BASE, project_id);
    let description = match client
        .get(&desc_url)
        .header("Accept", "application/json")
        .header("x-api-key", api_key.clone())
        .send()
    {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>() {
            Ok(v) => v
                .get("data")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| project.summary.clone()),
            Err(_) => project.summary.clone(),
        },
        _ => project.summary.clone(),
    };

    let files_resp = client
        .get(format!(
            "{}/mods/{}/files?pageSize=60&index=0",
            CURSEFORGE_API_BASE, project_id
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
    let detail_files = files
        .into_iter()
        .take(40)
        .map(|f| CurseforgeProjectFileDetail {
            file_id: f.id.to_string(),
            display_name: f.display_name,
            file_name: f.file_name,
            file_date: f.file_date,
            game_versions: f.game_versions,
            download_url: f.download_url,
        })
        .collect::<Vec<_>>();

    let project_id_text = project.id.to_string();
    let external_url = Some(curseforge_external_project_url(
        &project_id_text,
        project.slug.as_deref(),
        &detail_content_type,
    ));
    let author_names = project
        .authors
        .into_iter()
        .map(|a| a.name)
        .collect::<Vec<_>>();
    let categories = project
        .categories
        .into_iter()
        .map(|c| c.name)
        .filter(|c| !c.trim().is_empty())
        .collect::<Vec<_>>();

    Ok(CurseforgeProjectDetail {
        source: "curseforge".to_string(),
        project_id: format!("cf:{}", project_id_text),
        title: project.name,
        slug: project.slug,
        summary: project.summary,
        description,
        author_names,
        downloads: project.download_count.max(0.0) as u64,
        categories,
        icon_url: project.logo.map(|l| l.url),
        date_modified: project.date_modified,
        external_url,
        files: detail_files,
    })
}

#[tauri::command]
pub(crate) fn import_provider_modpack_template(
    args: ImportProviderModpackArgs,
) -> Result<CreatorPreset, String> {
    let source = args.source.trim().to_lowercase();
    let client = build_http_client()?;
    if source == "curseforge" {
        let api_key = curseforge_api_key().ok_or_else(missing_curseforge_key_message)?;
        return import_curseforge_modpack_template_inner(
            &client,
            &api_key,
            &args.project_id,
            args.project_title.as_deref(),
        );
    }
    import_modrinth_modpack_template_inner(&client, &args.project_id, args.project_title.as_deref())
}

#[tauri::command]
pub(crate) fn export_presets_json(args: ExportPresetsJsonArgs) -> Result<PresetsJsonIoResult, String> {
    let path_text = args.output_path.trim();
    if path_text.is_empty() {
        return Err("outputPath is required".to_string());
    }

    let items = if let Some(arr) = args.payload.as_array() {
        arr.len()
    } else if let Some(arr) = args.payload.get("presets").and_then(|v| v.as_array()) {
        arr.len()
    } else {
        return Err("Preset payload must be an array or { presets: [] }".to_string());
    };

    let path = PathBuf::from(path_text);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir export directory failed: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(&args.payload)
        .map_err(|e| format!("serialize presets failed: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("write presets file failed: {e}"))?;

    Ok(PresetsJsonIoResult {
        path: path.display().to_string(),
        items,
    })
}

#[tauri::command]
pub(crate) fn import_presets_json(args: ImportPresetsJsonArgs) -> Result<serde_json::Value, String> {
    let path_text = args.input_path.trim();
    if path_text.is_empty() {
        return Err("inputPath is required".to_string());
    }
    let path = PathBuf::from(path_text);
    if !path.exists() || !path.is_file() {
        return Err("Preset file does not exist".to_string());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("read presets file failed: {e}"))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse presets file failed: {e}"))?;

    if parsed.is_array() || parsed.get("presets").and_then(|v| v.as_array()).is_some() {
        Ok(parsed)
    } else {
        Err("Preset file must contain an array or { presets: [] }".to_string())
    }
}

#[tauri::command]
pub(crate) async fn get_selected_account_diagnostics(
    app: tauri::AppHandle,
) -> Result<AccountDiagnostics, String> {
    run_blocking_task("account diagnostics", move || {
        get_selected_account_diagnostics_inner(app)
    })
    .await
}

fn get_selected_account_diagnostics_inner(
    app: tauri::AppHandle,
) -> Result<AccountDiagnostics, String> {
    let total_started = Instant::now();
    let settings = read_launcher_settings(&app)?;
    let mut diag = make_account_diagnostics_base(&settings);
    let Some(selected_id) = settings.selected_account_id.clone() else {
        return Ok(diag);
    };

    let mut accounts = read_launcher_accounts(&app)?;
    let Some(mut account) = accounts.iter().find(|a| a.id == selected_id).cloned() else {
        return Ok(fail_account_diag(
            diag,
            "account-not-found",
            "Selected account is missing. Reconnect Microsoft account.".to_string(),
        ));
    };
    diag.account = Some(account.clone());

    let (client_id, source) = match resolve_oauth_client_id_with_source(&app) {
        Ok(v) => v,
        Err(e) => return Ok(fail_account_diag(diag, "oauth-client-id-missing", e)),
    };
    diag.client_id_source = source;

    let refresh = match keyring_get_refresh_token_for_account(&app, &account, &accounts) {
        Ok(v) => v,
        Err(e)
            if e.starts_with("No refresh token found in secure storage")
                || e.starts_with("Multiple secure refresh tokens were found") =>
        {
            let Some(repaired) =
                maybe_repair_selected_account_with_available_token(&app, &account, &accounts)?
            else {
                return Ok(fail_account_diag(diag, "refresh-token-read-failed", e));
            };
            account = repaired;
            diag.account = Some(account.clone());
            match keyring_get_refresh_token_for_account(&app, &account, &accounts) {
                Ok(v) => v,
                Err(err) => return Ok(fail_account_diag(diag, "refresh-token-read-failed", err)),
            }
        }
        Err(e) => return Ok(fail_account_diag(diag, "refresh-token-read-failed", e)),
    };

    let client = match build_http_client() {
        Ok(c) => c,
        Err(e) => {
            return Ok(fail_account_diag(
                diag,
                "http-client-build-failed",
                format!("build http client failed: {e}"),
            ))
        }
    };

    let refresh_started = Instant::now();
    let refreshed = match microsoft_refresh_access_token(&client, &client_id, &refresh) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[account_diag] microsoft-refresh-failed after {}ms",
                refresh_started.elapsed().as_millis()
            );
            return Ok(fail_account_diag(diag, "microsoft-refresh-failed", e));
        }
    };
    let refresh_ms = refresh_started.elapsed().as_millis();
    if refresh_ms > 350 {
        eprintln!("[account_diag] microsoft_refresh_access_token: {refresh_ms}ms");
    }
    if let Some(new_refresh) = refreshed.refresh_token.as_ref() {
        if let Err(e) =
            persist_refresh_token_for_launcher_account_with_app(&app, &account, new_refresh)
        {
            return Ok(fail_account_diag(diag, "refresh-token-write-failed", e));
        }
    }

    let token_exchange_started = Instant::now();
    let mc_access = match microsoft_access_to_mc_token(&client, &refreshed.access_token) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[account_diag] token-exchange-failed after {}ms",
                token_exchange_started.elapsed().as_millis()
            );
            return Ok(fail_account_diag(diag, "token-exchange-failed", e));
        }
    };
    let token_exchange_ms = token_exchange_started.elapsed().as_millis();
    if token_exchange_ms > 350 {
        eprintln!("[account_diag] microsoft_access_to_mc_token: {token_exchange_ms}ms");
    }
    diag.token_exchange_status = "minecraft-token-ok".to_string();

    let entitlements_started = Instant::now();
    if let Err(e) = ensure_minecraft_entitlement(&client, &mc_access) {
        eprintln!(
            "[account_diag] entitlements-check-failed after {}ms",
            entitlements_started.elapsed().as_millis()
        );
        return Ok(fail_account_diag(diag, "entitlements-check-failed", e));
    }
    let entitlements_ms = entitlements_started.elapsed().as_millis();
    if entitlements_ms > 350 {
        eprintln!("[account_diag] ensure_minecraft_entitlement: {entitlements_ms}ms");
    }
    diag.entitlements_ok = true;

    let profile_started = Instant::now();
    let profile = match fetch_minecraft_profile(&client, &mc_access) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[account_diag] profile-fetch-failed after {}ms",
                profile_started.elapsed().as_millis()
            );
            return Ok(fail_account_diag(diag, "profile-fetch-failed", e));
        }
    };
    let profile_ms = profile_started.elapsed().as_millis();
    if profile_ms > 350 {
        eprintln!("[account_diag] fetch_minecraft_profile: {profile_ms}ms");
    }

    diag.minecraft_uuid = Some(profile.id.clone());
    diag.minecraft_username = Some(profile.name.clone());
    diag.skins = summarize_cosmetics(&profile.skins);
    diag.capes = summarize_cosmetics(&profile.capes);
    diag.cape_count = diag.capes.len();
    diag.skin_url = diag
        .skins
        .iter()
        .find(|s| s.state.eq_ignore_ascii_case("active"))
        .map(|s| s.url.clone())
        .or_else(|| diag.skins.first().map(|s| s.url.clone()));

    let mut synced_account = account.clone();
    let token_for_new_id = refreshed.refresh_token.as_ref().unwrap_or(&refresh);
    let mut account_changed = false;
    if synced_account.id != profile.id {
        let old_account_id = synced_account.id.clone();
        synced_account.id = profile.id.clone();
        if let Err(e) = persist_refresh_token_for_launcher_account_with_app(
            &app,
            &synced_account,
            token_for_new_id,
        ) {
            return Ok(fail_account_diag(diag, "refresh-token-write-failed", e));
        }
        accounts.retain(|a| a.id != old_account_id && a.id != synced_account.id);
        account_changed = true;
    }
    if synced_account.username != profile.name {
        synced_account.username = profile.name.clone();
        account_changed = true;
    }
    if account_changed {
        accounts.push(synced_account.clone());
        accounts.sort_by(|a, b| a.username.to_lowercase().cmp(&b.username.to_lowercase()));
        if let Err(e) = write_launcher_accounts(&app, &accounts) {
            return Ok(fail_account_diag(diag, "account-sync-failed", e));
        }
        let mut settings_to_write = settings.clone();
        settings_to_write.selected_account_id = Some(synced_account.id.clone());
        if let Err(e) = write_launcher_settings(&app, &settings_to_write) {
            return Ok(fail_account_diag(diag, "account-sync-failed", e));
        }
        diag.account = Some(synced_account);
    }

    diag.status = "connected".to_string();
    diag.token_exchange_status = "ok".to_string();
    diag.last_error = None;
    let total_ms = total_started.elapsed().as_millis();
    if total_ms > 600 {
        eprintln!("[account_diag] get_selected_account_diagnostics total: {total_ms}ms");
    }
    Ok(diag)
}

#[tauri::command]
pub(crate) async fn apply_selected_account_appearance(
    app: tauri::AppHandle,
    args: ApplySelectedAccountAppearanceArgs,
) -> Result<AccountDiagnostics, String> {
    run_blocking_task("apply selected account appearance", move || {
        apply_selected_account_appearance_inner(app, args)
    })
    .await
}

fn apply_selected_account_appearance_inner(
    app: tauri::AppHandle,
    args: ApplySelectedAccountAppearanceArgs,
) -> Result<AccountDiagnostics, String> {
    if !args.apply_skin && !args.apply_cape {
        return Err("Nothing to apply. Select skin and/or cape first.".to_string());
    }

    let settings = read_launcher_settings(&app)?;
    let client = build_http_client()?;
    let (account, mc_access_token) = build_selected_microsoft_auth(&app, &client, &settings)?;

    if args.apply_skin {
        let source = args
            .skin_source
            .as_deref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "No skin selected to apply.".to_string())?;
        apply_minecraft_skin(
            &client,
            &mc_access_token,
            source,
            args.skin_variant.as_deref(),
        )?;
    }

    if args.apply_cape {
        apply_minecraft_cape(&client, &mc_access_token, args.cape_id.as_deref())?;
    }

    let mut diag = make_account_diagnostics_base(&settings);
    diag.account = Some(account);
    if let Ok((_, source)) = resolve_oauth_client_id_with_source(&app) {
        diag.client_id_source = source;
    }
    diag.status = "connected".to_string();
    diag.token_exchange_status = "ok".to_string();
    diag.entitlements_ok = true;
    diag.last_error = None;

    let profile = fetch_minecraft_profile(&client, &mc_access_token)
        .map_err(|e| format!("post-apply profile refresh failed: {e}"))?;
    diag.minecraft_uuid = Some(profile.id.clone());
    diag.minecraft_username = Some(profile.name.clone());
    diag.skins = summarize_cosmetics(&profile.skins);
    diag.capes = summarize_cosmetics(&profile.capes);
    diag.cape_count = diag.capes.len();
    diag.skin_url = diag
        .skins
        .iter()
        .find(|s| s.state.eq_ignore_ascii_case("active"))
        .map(|s| s.url.clone())
        .or_else(|| diag.skins.first().map(|s| s.url.clone()));

    Ok(diag)
}

#[tauri::command]
pub(crate) async fn export_instance_mods_zip(
    app: tauri::AppHandle,
    args: ExportInstanceModsZipArgs,
) -> Result<ExportModsResult, String> {
    run_blocking_task("export instance mods zip", move || {
        export_instance_mods_zip_inner(app, args)
    })
    .await
}

fn export_instance_mods_zip_inner(
    app: tauri::AppHandle,
    args: ExportInstanceModsZipArgs,
) -> Result<ExportModsResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mods_dir = instance_dir.join("mods");
    if !mods_dir.exists() {
        return Err("Instance mods folder does not exist".to_string());
    }

    let output = if let Some(custom) = args.output_path.as_ref() {
        PathBuf::from(custom)
    } else {
        let base = home_dir()
            .map(|h| h.join("Downloads"))
            .filter(|p| p.exists())
            .unwrap_or_else(|| instance_dir.clone());
        base.join(default_export_filename(&instance.name))
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir export directory failed: {e}"))?;
    }

    let file = File::create(&output).map_err(|e| format!("create zip failed: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut files_count = 0usize;

    let read = fs::read_dir(&mods_dir).map_err(|e| format!("read mods directory failed: {e}"))?;
    for ent in read {
        let ent = ent.map_err(|e| format!("read mods entry failed: {e}"))?;
        let path = ent.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "invalid file name in mods directory".to_string())?;
        let lower = name.to_lowercase();
        if !(lower.ends_with(".jar") || lower.ends_with(".disabled")) {
            continue;
        }
        let mut src = File::open(&path).map_err(|e| format!("open '{}' failed: {e}", name))?;
        zip.start_file(name, options)
            .map_err(|e| format!("zip write header failed: {e}"))?;
        std::io::copy(&mut src, &mut zip)
            .map_err(|e| format!("zip write '{}' failed: {e}", name))?;
        files_count += 1;
    }

    zip.finish()
        .map_err(|e| format!("finalize zip failed: {e}"))?;

    Ok(ExportModsResult {
        output_path: output.display().to_string(),
        files_count,
    })
}

#[tauri::command]
pub(crate) fn list_instances(app: tauri::AppHandle) -> Result<Vec<Instance>, String> {
    let dir = app_instances_dir(&app)?;
    let mut idx = read_index(&dir)?;
    if migrate_instance_folder_names(&dir, &mut idx)? {
        write_index(&dir, &idx)?;
    }
    Ok(idx.instances)
}

fn create_instance_internal(
    app: &tauri::AppHandle,
    clean_name: String,
    clean_mc: String,
    loader_lc: String,
    icon_path: Option<String>,
) -> Result<Instance, String> {
    if clean_name.trim().is_empty() {
        return Err("name is required".to_string());
    }
    if clean_mc.trim().is_empty() {
        return Err("mc_version is required".to_string());
    }
    if parse_loader_for_instance(&loader_lc).is_none() {
        return Err("loader must be one of vanilla/fabric/forge/neoforge/quilt".to_string());
    }

    let dir = app_instances_dir(app)?;
    let mut idx = read_index(&dir)?;
    if migrate_instance_folder_names(&dir, &mut idx)? {
        write_index(&dir, &idx)?;
    }
    let folder_name = allocate_instance_folder_name(&dir, &idx, &clean_name, None, None);

    let mut inst = Instance {
        id: gen_id(),
        name: clean_name,
        folder_name: Some(folder_name.clone()),
        mc_version: clean_mc,
        loader: loader_lc,
        created_at: now_iso(),
        icon_path: None,
        settings: InstanceSettings::default(),
    };

    let inst_dir = dir.join(folder_name);
    fs::create_dir_all(inst_dir.join("mods")).map_err(|e| format!("mkdir mods failed: {e}"))?;
    fs::create_dir_all(inst_dir.join("config")).map_err(|e| format!("mkdir config failed: {e}"))?;
    fs::create_dir_all(inst_dir.join("resourcepacks"))
        .map_err(|e| format!("mkdir resourcepacks failed: {e}"))?;
    fs::create_dir_all(inst_dir.join("shaderpacks"))
        .map_err(|e| format!("mkdir shaderpacks failed: {e}"))?;
    fs::create_dir_all(inst_dir.join("saves")).map_err(|e| format!("mkdir saves failed: {e}"))?;

    let picked_icon_path = icon_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    if let Some(icon_source) = picked_icon_path {
        inst.icon_path = Some(copy_instance_icon_to_dir(&icon_source, &inst_dir)?);
    }

    write_instance_meta(&inst_dir, &inst)?;
    idx.instances.insert(0, inst.clone());
    write_index(&dir, &idx)?;
    write_lockfile(&dir, &inst.id, &Lockfile::default())?;

    Ok(inst)
}

#[tauri::command]
pub(crate) fn create_instance(app: tauri::AppHandle, args: CreateInstanceArgs) -> Result<Instance, String> {
    let loader_lc = parse_loader_for_instance(&args.loader)
        .ok_or_else(|| "loader must be one of vanilla/fabric/forge/neoforge/quilt".to_string())?;

    let clean_name = sanitize_name(&args.name);
    if clean_name.is_empty() {
        return Err("name is required".into());
    }
    let clean_mc = args.mc_version.trim().to_string();
    if clean_mc.is_empty() {
        return Err("mc_version is required".into());
    }
    create_instance_internal(&app, clean_name, clean_mc, loader_lc, args.icon_path)
}

#[tauri::command]
pub(crate) fn create_instance_from_modpack_file(
    app: tauri::AppHandle,
    args: CreateInstanceFromModpackFileArgs,
) -> Result<CreateInstanceFromModpackFileResult, String> {
    let file_path = PathBuf::from(args.file_path.trim());
    if !file_path.exists() || !file_path.is_file() {
        return Err("Selected modpack archive was not found.".to_string());
    }
    let (default_name, mc_version, loader, override_roots, mut warnings) =
        parse_modpack_file_info(&file_path)?;
    let final_name = sanitize_name(args.name.as_deref().unwrap_or(&default_name));
    if final_name.trim().is_empty() {
        return Err("Imported modpack name is empty.".to_string());
    }
    let instance =
        create_instance_internal(&app, final_name, mc_version, loader, args.icon_path.clone())?;
    let instances_dir = app_instances_dir(&app)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let imported_files =
        extract_overrides_from_modpack(&file_path, &instance_dir, &override_roots)?;
    if imported_files == 0 {
        warnings.push("No override files were found in the archive.".to_string());
    }
    Ok(CreateInstanceFromModpackFileResult {
        instance,
        imported_files,
        warnings,
    })
}

#[tauri::command]
pub(crate) fn list_launcher_import_sources() -> Result<Vec<LauncherImportSource>, String> {
    Ok(list_launcher_import_sources_inner())
}

#[tauri::command]
pub(crate) fn import_instance_from_launcher(
    app: tauri::AppHandle,
    args: ImportInstanceFromLauncherArgs,
) -> Result<ImportInstanceFromLauncherResult, String> {
    let source = list_launcher_import_sources_inner()
        .into_iter()
        .find(|s| s.id == args.source_id)
        .ok_or_else(|| "Selected launcher source was not found.".to_string())?;
    let source_path = PathBuf::from(source.source_path.trim());
    if !source_path.exists() || !source_path.is_dir() {
        return Err("Source launcher directory is unavailable.".to_string());
    }
    let fallback_name = format!("{} import", source.label);
    let final_name = sanitize_name(args.name.as_deref().unwrap_or(&fallback_name));
    if final_name.trim().is_empty() {
        return Err("Imported instance name is required.".to_string());
    }
    let loader = parse_loader_for_instance(&source.loader).unwrap_or_else(|| "vanilla".to_string());
    let instance = create_instance_internal(
        &app,
        final_name,
        source.mc_version.clone(),
        loader,
        args.icon_path.clone(),
    )?;
    let instances_dir = app_instances_dir(&app)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let imported_files = copy_launcher_source_into_instance(&source_path, &instance_dir)?;
    Ok(ImportInstanceFromLauncherResult {
        instance,
        imported_files,
    })
}

#[tauri::command]
pub(crate) fn update_instance(app: tauri::AppHandle, args: UpdateInstanceArgs) -> Result<Instance, String> {
    let dir = app_instances_dir(&app)?;
    let mut idx = read_index(&dir)?;
    if migrate_instance_folder_names(&dir, &mut idx)? {
        write_index(&dir, &idx)?;
    }
    let pos = idx
        .instances
        .iter()
        .position(|x| x.id == args.instance_id)
        .ok_or_else(|| "instance not found".to_string())?;
    let mut inst = idx.instances[pos].clone();
    let prev_dir = instance_dir_for_instance(&dir, &inst);
    let mut folder_name_override: Option<String> = None;

    if let Some(name) = args.name.as_ref() {
        let clean_name = sanitize_name(name);
        if clean_name.is_empty() {
            return Err("name is required".to_string());
        }
        inst.name = clean_name;
        let next_folder = allocate_instance_folder_name(
            &dir,
            &idx,
            &inst.name,
            Some(&inst.id),
            inst.folder_name.as_deref(),
        );
        folder_name_override = Some(next_folder);
    }
    if let Some(mc_version) = args.mc_version.as_ref() {
        let clean_mc = mc_version.trim().to_string();
        if clean_mc.is_empty() {
            return Err("mc_version is required".to_string());
        }
        inst.mc_version = clean_mc;
    }
    if let Some(loader) = args.loader.as_ref() {
        let parsed = parse_loader_for_instance(loader).ok_or_else(|| {
            "loader must be one of vanilla/fabric/forge/neoforge/quilt".to_string()
        })?;
        inst.loader = parsed;
    }
    if let Some(settings) = args.settings {
        inst.settings = normalize_instance_settings(settings);
    } else {
        inst.settings = normalize_instance_settings(inst.settings);
    }

    if let Some(next_folder) = folder_name_override {
        inst.folder_name = Some(next_folder);
    } else if inst
        .folder_name
        .as_ref()
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
    {
        inst.folder_name = Some(inst.id.clone());
    }

    let inst_dir = instance_dir_for_instance(&dir, &inst);
    if prev_dir != inst_dir && prev_dir.exists() && !inst_dir.exists() {
        fs::rename(&prev_dir, &inst_dir).map_err(|e| {
            format!(
                "rename instance folder failed ({} -> {}): {e}",
                prev_dir.display(),
                inst_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&inst_dir).map_err(|e| format!("mkdir instance dir failed: {e}"))?;
    write_instance_meta(&inst_dir, &inst)?;
    idx.instances[pos] = inst.clone();
    write_index(&dir, &idx)?;
    Ok(inst)
}

#[tauri::command]
pub(crate) fn detect_java_runtimes() -> Result<Vec<JavaRuntimeCandidate>, String> {
    Ok(detect_java_runtimes_inner())
}

#[tauri::command]
pub(crate) fn set_instance_icon(app: tauri::AppHandle, args: SetInstanceIconArgs) -> Result<Instance, String> {
    let dir = app_instances_dir(&app)?;
    let mut idx = read_index(&dir)?;
    let pos = idx
        .instances
        .iter()
        .position(|x| x.id == args.instance_id)
        .ok_or_else(|| "instance not found".to_string())?;

    let mut inst = idx.instances[pos].clone();
    let inst_dir = instance_dir_for_instance(&dir, &inst);
    fs::create_dir_all(&inst_dir).map_err(|e| format!("mkdir instance dir failed: {e}"))?;

    let next_icon_path = args
        .icon_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    inst.icon_path = if let Some(path) = next_icon_path {
        Some(copy_instance_icon_to_dir(&path, &inst_dir)?)
    } else {
        clear_instance_icon_files(&inst_dir)?;
        None
    };

    write_instance_meta(&inst_dir, &inst)?;
    idx.instances[pos] = inst.clone();
    write_index(&dir, &idx)?;
    Ok(inst)
}

#[tauri::command]
pub(crate) fn read_local_image_data_url(args: ReadLocalImageDataUrlArgs) -> Result<String, String> {
    let trimmed = args.path.trim();
    if trimmed.is_empty() {
        return Err("path is required".to_string());
    }
    let path = Path::new(trimmed);
    if !path.exists() || !path.is_file() {
        return Err("image file not found".to_string());
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.trim().to_ascii_lowercase())
        .ok_or_else(|| "image file must have an extension".to_string())?;
    if !allowed_icon_extension(&ext) {
        return Err("image must be png/jpg/jpeg/webp/bmp/gif".to_string());
    }

    let bytes = fs::read(path).map_err(|e| format!("read image failed: {e}"))?;
    if bytes.len() > MAX_LOCAL_IMAGE_BYTES {
        return Err("image file is too large (max 8MB)".to_string());
    }

    let mime =
        image_mime_for_extension(&ext).ok_or_else(|| "unsupported image type".to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

#[tauri::command]
pub(crate) fn delete_instance(app: tauri::AppHandle, args: DeleteInstanceArgs) -> Result<(), String> {
    let dir = app_instances_dir(&app)?;
    let mut idx = read_index(&dir)?;
    if migrate_instance_folder_names(&dir, &mut idx)? {
        write_index(&dir, &idx)?;
    }

    let target = idx
        .instances
        .iter()
        .find(|x| x.id == args.id)
        .cloned()
        .ok_or_else(|| "instance not found".to_string())?;

    let before = idx.instances.len();
    idx.instances.retain(|x| x.id != args.id);
    if idx.instances.len() == before {
        return Err("instance not found".into());
    }

    let inst_dir = instance_dir_for_instance(&dir, &target);
    if inst_dir.exists() {
        fs::remove_dir_all(inst_dir).map_err(|e| format!("remove dir failed: {e}"))?;
    }

    write_index(&dir, &idx)?;
    Ok(())
}

fn install_modrinth_mod_inner(
    app: tauri::AppHandle,
    args: InstallModrinthModArgs,
    snapshot_reason: Option<&str>,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let mods_dir = instance_dir.join("mods");
    fs::create_dir_all(&mods_dir).map_err(|e| format!("mkdir mods failed: {e}"))?;

    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "resolving".into(),
            downloaded: 0,
            total: None,
            percent: Some(1.0),
            message: Some("Resolving compatible versions and required dependencies…".into()),
        },
    );

    let client = build_http_client()?;

    let plan = resolve_modrinth_install_plan(&client, &instance, &args.project_id)?;
    let total_mods = plan.len();
    let dependency_mods = total_mods.saturating_sub(1);
    let total_actions = count_plan_install_actions(&instance_dir, &lock, &plan);

    if total_actions > 0 {
        if let Some(reason) = snapshot_reason {
            let _ = create_instance_snapshot(&instances_dir, &args.instance_id, reason);
        }
    }

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "resolving".into(),
            downloaded: 0,
            total: Some(total_actions as u64),
            percent: Some(if total_actions == 0 { 100.0 } else { 2.0 }),
            message: Some(format!(
                "Install plan ready: {} mod(s) total ({} required dependencies)",
                total_mods, dependency_mods
            )),
        },
    );

    let mut root_installed: Option<InstalledMod> = None;
    let mut completed_actions: usize = 0;

    for item in plan {
        let safe_filename =
            safe_mod_filename(&item.project_id, &item.version.id, &item.file.filename);

        if is_plan_entry_up_to_date(&instance_dir, &lock, &item) {
            if item.project_id == args.project_id {
                if let Some(existing) = lock
                    .entries
                    .iter()
                    .find(|e| e.project_id == args.project_id)
                {
                    root_installed = Some(lock_entry_to_installed(&instance_dir, existing));
                }
            }
            continue;
        }

        let final_path = mods_dir.join(&safe_filename);
        let tmp_path = mods_dir.join(format!("{safe_filename}.part"));
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "downloading".into(),
                downloaded: completed_actions as u64,
                total: Some(total_actions as u64),
                percent: Some(if total_actions == 0 {
                    100.0
                } else {
                    let base = (completed_actions as f64 / total_actions as f64) * 100.0;
                    if completed_actions == 0 {
                        base.max(0.2)
                    } else {
                        base
                    }
                }),
                message: Some(format!("Installing {} ({safe_filename})", item.project_id)),
            },
        );
        let mut stream_result = download_stream_to_temp_with_retry(
            &client,
            &item.file.url,
            &item.project_id,
            &tmp_path,
            |downloaded_bytes, total_bytes| {
                let ratio = match total_bytes {
                    Some(total) if total > 0 => downloaded_bytes as f64 / total as f64,
                    _ => unknown_progress_ratio(downloaded_bytes),
                };
                let overall = if total_actions == 0 {
                    100.0
                } else {
                    ((completed_actions as f64 + ratio) / total_actions as f64) * 100.0
                };
                let visible_overall = overall.max(0.2);
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "downloading".into(),
                        downloaded: downloaded_bytes,
                        total: total_bytes,
                        percent: Some(visible_overall.clamp(0.0, 99.4)),
                        message: Some(format!(
                            "Installing {} ({safe_filename}) · {}",
                            item.project_id,
                            format_download_meter(downloaded_bytes, total_bytes)
                        )),
                    },
                );
            },
        )?;

        if final_path.exists() {
            fs::remove_file(&final_path).map_err(|e| format!("remove old mod file failed: {e}"))?;
        }
        let post_process_started = Instant::now();
        fs::rename(&tmp_path, &final_path).map_err(|e| format!("move mod file failed: {e}"))?;
        stream_result.profile.post_process_ms = post_process_started.elapsed().as_millis();
        maybe_log_download_profile(&item.project_id, &stream_result.profile);

        remove_replaced_entries_for_project(
            &mut lock,
            &instance_dir,
            &item.project_id,
            Some(&safe_filename),
        )?;

        let fallback_name = item
            .version
            .name
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| item.project_id.clone());
        let resolved_name = if item.project_id == args.project_id {
            if let Some(title) = args.project_title.as_ref() {
                let clean = title.trim();
                if clean.is_empty() {
                    fetch_project_title(&client, &item.project_id).unwrap_or(fallback_name)
                } else {
                    clean.to_string()
                }
            } else {
                fetch_project_title(&client, &item.project_id).unwrap_or(fallback_name)
            }
        } else {
            fallback_name
        };

        let new_entry = LockEntry {
            source: "modrinth".into(),
            project_id: item.project_id.clone(),
            version_id: item.version.id.clone(),
            name: resolved_name.clone(),
            version_number: item.version.version_number.clone(),
            filename: safe_filename,
            content_type: "mods".to_string(),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            pinned_version: None,
            enabled: true,
            hashes: {
                let mut hashes = item.file.hashes.clone();
                if !stream_result.sha512.trim().is_empty() {
                    hashes
                        .entry("sha512".to_string())
                        .or_insert_with(|| stream_result.sha512.clone());
                }
                hashes
            },
            provider_candidates: vec![ProviderCandidate {
                source: "modrinth".to_string(),
                project_id: item.project_id.clone(),
                version_id: item.version.id.clone(),
                name: resolved_name.clone(),
                version_number: item.version.version_number.clone(),
                confidence: None,
                reason: None,
            }],
        };

        lock.entries.push(new_entry.clone());
        lock.entries
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        write_lockfile(&instances_dir, &args.instance_id, &lock)?;

        if item.project_id == args.project_id {
            root_installed = Some(lock_entry_to_installed(&instance_dir, &new_entry));
        }

        completed_actions += 1;
    }

    if root_installed.is_none() {
        if let Some(root_entry) = lock
            .entries
            .iter()
            .find(|e| e.project_id == args.project_id)
        {
            root_installed = Some(lock_entry_to_installed(&instance_dir, root_entry));
        }
    }

    let root_installed =
        root_installed.ok_or_else(|| "Root mod was not installed in lockfile".to_string())?;

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "completed".into(),
            downloaded: completed_actions as u64,
            total: Some(total_actions as u64),
            percent: Some(100.0),
            message: Some(format!(
                "Installed {} mod(s) ({} dependency mods)",
                total_mods, dependency_mods
            )),
        },
    );

    Ok(root_installed)
}

#[tauri::command]
pub(crate) async fn install_modrinth_mod(
    app: tauri::AppHandle,
    args: InstallModrinthModArgs,
) -> Result<InstalledMod, String> {
    run_blocking_task("install modrinth mod", move || {
        let subject = snapshot_install_subject(args.project_title.as_deref(), &args.project_id);
        let reason = format!("before-install-modrinth:{subject}");
        install_modrinth_mod_inner(app, args, Some(reason.as_str()))
    })
    .await
}

#[tauri::command]
pub(crate) async fn install_curseforge_mod(
    app: tauri::AppHandle,
    args: InstallCurseforgeModArgs,
) -> Result<InstalledMod, String> {
    run_blocking_task("install curseforge mod", move || {
        let subject = snapshot_install_subject(args.project_title.as_deref(), &args.project_id);
        let reason = format!("before-install-curseforge:{subject}");
        install_curseforge_mod_inner(app, args, Some(reason.as_str()))
    })
    .await
}

fn install_curseforge_mod_inner(
    app: tauri::AppHandle,
    args: InstallCurseforgeModArgs,
    snapshot_reason: Option<&str>,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let api_key = curseforge_api_key().ok_or_else(missing_curseforge_key_message)?;
    let client = build_http_client()?;
    let root_mod_id = parse_curseforge_project_id(&args.project_id)?;
    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id.clone(),
            stage: "resolving".to_string(),
            downloaded: 0,
            total: None,
            percent: Some(1.0),
            message: Some("Resolving CurseForge metadata and dependency chain…".to_string()),
        },
    );

    let install_plan = resolve_curseforge_dependency_chain(
        &client,
        &api_key,
        &instance,
        root_mod_id,
        |resolved_count, pending_count| {
            let denom = (resolved_count + pending_count).max(1) as f64;
            let ratio = resolved_count as f64 / denom;
            let percent = (1.0 + ratio * 28.0).clamp(1.0, 34.0);
            let detail = if pending_count > 0 {
                format!(
                    "Resolved {} project(s), {} pending…",
                    resolved_count, pending_count
                )
            } else {
                format!("Resolved {} project(s), preparing downloads…", resolved_count)
            };
            emit_install_progress(
                &app,
                InstallProgressEvent {
                    instance_id: args.instance_id.clone(),
                    project_id: args.project_id.clone(),
                    stage: "resolving".to_string(),
                    downloaded: resolved_count as u64,
                    total: Some((resolved_count + pending_count) as u64),
                    percent: Some(percent),
                    message: Some(format!("Resolving CurseForge metadata… {detail}")),
                },
            );
        },
    )?;
    let total_actions = install_plan.len().max(1);

    if let Some(reason) = snapshot_reason {
        let _ = create_instance_snapshot(&instances_dir, &args.instance_id, reason);
    }
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let mut root_entry: Option<LockEntry> = None;
    for (idx, plan_item) in install_plan.iter().enumerate() {
        let is_root = plan_item.mod_id == root_mod_id;
        emit_install_progress(
            &app,
            InstallProgressEvent {
                instance_id: args.instance_id.clone(),
                project_id: args.project_id.clone(),
                stage: "downloading".to_string(),
                downloaded: idx as u64,
                total: Some(total_actions as u64),
                percent: Some(
                    if idx == 0 {
                        0.2
                    } else {
                        (idx as f64) / (total_actions as f64) * 100.0
                    }
                    .clamp(0.0, 99.0),
                ),
                message: Some(if is_root {
                    "Downloading selected CurseForge mod…".to_string()
                } else {
                    format!(
                        "Downloading required dependency {}/{}…",
                        idx + 1,
                        total_actions
                    )
                }),
            },
        );

        let entry = install_curseforge_content_inner(
            &instance,
            &instance_dir,
            &mut lock,
            &client,
            &api_key,
            &plan_item.mod_id.to_string(),
            if is_root {
                args.project_title.as_deref()
            } else {
                None
            },
            "mods",
            &[],
            Some(&plan_item.file),
            |downloaded_bytes, total_bytes| {
                let ratio = match total_bytes {
                    Some(total) if total > 0 => downloaded_bytes as f64 / total as f64,
                    _ => unknown_progress_ratio(downloaded_bytes),
                };
                let overall = ((idx as f64 + ratio) / total_actions as f64) * 100.0;
                let visible_overall = overall.max(0.2);
                emit_install_progress(
                    &app,
                    InstallProgressEvent {
                        instance_id: args.instance_id.clone(),
                        project_id: args.project_id.clone(),
                        stage: "downloading".to_string(),
                        downloaded: downloaded_bytes,
                        total: total_bytes,
                        percent: Some(visible_overall.clamp(0.0, 99.4)),
                        message: Some(if is_root {
                            format!(
                                "Downloading selected CurseForge mod… · {}",
                                format_download_meter(downloaded_bytes, total_bytes)
                            )
                        } else {
                            format!(
                                "Downloading required dependency {}/{}… · {}",
                                idx + 1,
                                total_actions,
                                format_download_meter(downloaded_bytes, total_bytes)
                            )
                        }),
                    },
                );
            },
        )?;
        if is_root {
            root_entry = Some(entry);
        }
    }

    lock.entries
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;

    let entry =
        root_entry.ok_or_else(|| "Failed to resolve selected CurseForge project".to_string())?;

    emit_install_progress(
        &app,
        InstallProgressEvent {
            instance_id: args.instance_id.clone(),
            project_id: args.project_id,
            stage: "completed".to_string(),
            downloaded: total_actions as u64,
            total: Some(total_actions as u64),
            percent: Some(100.0),
            message: Some(if total_actions > 1 {
                format!(
                    "CurseForge install complete ({} required dependencies)",
                    total_actions.saturating_sub(1)
                )
            } else {
                "CurseForge install complete".to_string()
            }),
        },
    );

    Ok(lock_entry_to_installed(&instance_dir, &entry))
}

#[tauri::command]
pub(crate) async fn preview_modrinth_install(
    app: tauri::AppHandle,
    args: InstallModrinthModArgs,
) -> Result<InstallPlanPreview, String> {
    run_blocking_task("preview modrinth install", move || {
        preview_modrinth_install_inner(app, args)
    })
    .await
}

fn preview_modrinth_install_inner(
    app: tauri::AppHandle,
    args: InstallModrinthModArgs,
) -> Result<InstallPlanPreview, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;

    let client = build_http_client()?;

    let plan = resolve_modrinth_install_plan(&client, &instance, &args.project_id)?;
    let total_mods = plan.len();
    let dependency_mods = total_mods.saturating_sub(1);
    let will_install_mods = count_plan_install_actions(&instance_dir, &lock, &plan);

    Ok(InstallPlanPreview {
        total_mods,
        dependency_mods,
        will_install_mods,
    })
}

#[tauri::command]
pub(crate) async fn import_local_mod_file(
    app: tauri::AppHandle,
    args: ImportLocalModFileArgs,
) -> Result<InstalledMod, String> {
    run_blocking_task("import local mod file", move || {
        import_local_mod_file_inner(app, args)
    })
    .await
}

fn import_local_mod_file_inner(
    app: tauri::AppHandle,
    args: ImportLocalModFileArgs,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let normalized_content_type =
        normalize_lock_content_type(args.content_type.as_deref().unwrap_or("mods"));
    if !is_supported_local_content_type(&normalized_content_type) {
        return Err("Unsupported content type for local import".to_string());
    }

    let source_path = PathBuf::from(&args.file_path);
    if !source_path.exists() || !source_path.is_file() {
        return Err("Selected file does not exist".into());
    }
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !local_file_extension_allowed(&normalized_content_type, &ext) {
        return Err(format!(
            "Only {} files are supported for local {} import",
            local_file_extension_hint(&normalized_content_type),
            content_type_display_name(&normalized_content_type)
        ));
    }

    let source_name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid file name")?;
    let safe_filename = sanitize_filename(source_name);
    if safe_filename.is_empty() {
        return Err("Invalid file name".into());
    }

    let file_bytes = fs::read(&source_path).map_err(|e| format!("read file failed: {e}"))?;
    if normalized_content_type == "mods" {
        ensure_local_mod_loader_compatible(&instance, &safe_filename, &file_bytes)?;
        let mods_dir = instance_dir.join("mods");
        fs::create_dir_all(&mods_dir).map_err(|e| format!("mkdir mods failed: {e}"))?;
        let disabled_path = mods_dir.join(format!("{safe_filename}.disabled"));
        if disabled_path.exists() {
            fs::remove_file(&disabled_path)
                .map_err(|e| format!("cleanup disabled mod failed: {e}"))?;
        }
    }

    let worlds = if normalized_content_type == "datapacks" {
        let requested = if let Some(target_worlds) =
            args.target_worlds.clone().filter(|list| !list.is_empty())
        {
            target_worlds
        } else {
            list_instance_world_names(&instance_dir)?
        };
        normalize_target_worlds_for_datapack(&instance_dir, &requested)?
    } else {
        vec![]
    };
    write_download_to_content_targets(
        &instance_dir,
        &normalized_content_type,
        &safe_filename,
        &worlds,
        &file_bytes,
    )?;

    let detected_provider_matches = Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .build()
        .ok()
        .and_then(|client| {
            Some(detect_provider_matches_for_local_mod(
                &client,
                &file_bytes,
                &safe_filename,
                normalized_content_type == "mods",
            ))
        });
    let detected_provider_matches = detected_provider_matches.unwrap_or_default();
    let detected_provider =
        select_preferred_provider_match(&detected_provider_matches, None).cloned();
    let detected_provider_candidates = to_provider_candidates(&detected_provider_matches);

    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;
    lock.entries.retain(|e| {
        !(e.filename == safe_filename
            && normalize_lock_content_type(&e.content_type) == normalized_content_type)
    });

    if let Some(found) = detected_provider.as_ref() {
        remove_replaced_entries_for_content(
            &mut lock,
            &instance_dir,
            &found.project_id,
            &normalized_content_type,
        )?;
    }

    let new_entry = if let Some(found) = detected_provider {
        LockEntry {
            source: found.source,
            project_id: found.project_id,
            version_id: found.version_id,
            name: found.name,
            version_number: found.version_number,
            filename: safe_filename.clone(),
            content_type: normalized_content_type.clone(),
            target_scope: if normalized_content_type == "datapacks" {
                "world".to_string()
            } else {
                "instance".to_string()
            },
            target_worlds: worlds.clone(),
            pinned_version: None,
            enabled: true,
            hashes: found.hashes,
            provider_candidates: detected_provider_candidates,
        }
    } else {
        let project_id = format!(
            "local:{}:{}",
            normalized_content_type,
            safe_filename.to_lowercase()
        );
        LockEntry {
            source: "local".into(),
            project_id,
            version_id: format!("local_{}", now_millis()),
            name: infer_local_name(&safe_filename),
            version_number: "local-file".into(),
            filename: safe_filename.clone(),
            content_type: normalized_content_type.clone(),
            target_scope: if normalized_content_type == "datapacks" {
                "world".to_string()
            } else {
                "instance".to_string()
            },
            target_worlds: worlds,
            pinned_version: None,
            enabled: true,
            hashes: HashMap::new(),
            provider_candidates: vec![],
        }
    };

    lock.entries.push(new_entry.clone());
    lock.entries
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;

    Ok(lock_entry_to_installed(&instance_dir, &new_entry))
}

fn resolve_local_mod_sources_inner(
    app: &tauri::AppHandle,
    instance_id: &str,
    mode: &str,
    requested_content_types: Option<&[String]>,
) -> Result<LocalResolverResult, String> {
    let instances_dir = app_instances_dir(app)?;
    let _ = find_instance(&instances_dir, instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, instance_id)?;
    let mut lock = read_lockfile(&instances_dir, instance_id)?;
    let strict_local_only = mode.trim().to_ascii_lowercase() != "all";
    let content_types_filter: HashSet<String> = if let Some(requested) = requested_content_types {
        let mut allowed = requested
            .iter()
            .map(|value| normalize_lock_content_type(value))
            .filter(|value| is_supported_local_content_type(value))
            .collect::<HashSet<_>>();
        if allowed.is_empty() {
            supported_local_content_types()
                .iter()
                .map(|value| value.to_string())
                .collect()
        } else {
            allowed.drain().collect()
        }
    } else {
        supported_local_content_types()
            .iter()
            .map(|value| value.to_string())
            .collect()
    };

    let client = Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| format!("build http client failed: {e}"))?;

    let mut scanned_entries = 0usize;
    let mut resolved_entries = 0usize;
    let mut matches: Vec<LocalResolverMatch> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut changed = false;

    for idx in 0..lock.entries.len() {
        let source = lock.entries[idx].source.trim().to_ascii_lowercase();
        let is_local = source == "local";
        if strict_local_only && !is_local {
            continue;
        }
        if !is_local {
            continue;
        }
        let entry_content_type = normalize_lock_content_type(&lock.entries[idx].content_type);
        if !content_types_filter.contains(&entry_content_type) {
            continue;
        }
        scanned_entries += 1;
        let filename = lock.entries[idx].filename.clone();
        let existing = local_entry_file_read_path(&instance_dir, &lock.entries[idx])?;
        let Some(read_path) = existing else {
            warnings.push(format!("Skipped '{}': file missing on disk.", filename));
            continue;
        };
        let file_bytes = match fs::read(&read_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                warnings.push(format!("Skipped '{}': read failed ({err}).", filename));
                continue;
            }
        };
        let found_matches = detect_provider_matches_for_local_mod(
            &client,
            &file_bytes,
            &filename,
            entry_content_type == "mods",
        );
        let Some(found) = select_preferred_provider_match(&found_matches, None) else {
            continue;
        };
        let key_before = local_entry_key(&lock.entries[idx]);
        apply_provider_match_to_lock_entry(&mut lock.entries[idx], &found);
        lock.entries[idx].provider_candidates = to_provider_candidates(&found_matches);
        resolved_entries += 1;
        changed = true;
        matches.push(LocalResolverMatch {
            key: key_before,
            from_source: "local".to_string(),
            to_source: found.source.clone(),
            project_id: found.project_id.clone(),
            version_id: found.version_id.clone(),
            name: found.name.clone(),
            version_number: found.version_number.clone(),
            confidence: found.confidence.clone(),
            reason: found.reason.clone(),
        });
    }

    if changed {
        lock.entries
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        write_lockfile(&instances_dir, instance_id, &lock)?;
    }

    let remaining_local_entries = lock
        .entries
        .iter()
        .filter(|entry| entry.source.trim().eq_ignore_ascii_case("local"))
        .count();

    Ok(LocalResolverResult {
        instance_id: instance_id.to_string(),
        scanned_entries,
        resolved_entries,
        remaining_local_entries,
        matches,
        warnings,
    })
}

#[tauri::command]
pub(crate) async fn resolve_local_mod_sources(
    app: tauri::AppHandle,
    args: ResolveLocalModSourcesArgs,
) -> Result<LocalResolverResult, String> {
    run_blocking_task("resolve local mod sources", move || {
        let mode = args.mode.unwrap_or_else(|| "missing_only".to_string());
        resolve_local_mod_sources_inner(
            &app,
            &args.instance_id,
            &mode,
            args.content_types.as_deref(),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn check_instance_content_updates(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<ContentUpdateCheckResult, String> {
    run_blocking_task("check instance content updates", move || {
        check_instance_content_updates_command_inner(app, args)
    })
    .await
}

fn check_instance_content_updates_command_inner(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<ContentUpdateCheckResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let content_type_filter = normalize_update_content_type_filter(args.content_types.as_deref());
    check_instance_content_updates_inner(
        &client,
        &instance,
        &lock,
        UpdateScope::AllContent,
        content_type_filter.as_ref(),
    )
}

fn update_all_instance_content_inner(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<UpdateAllContentResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let content_type_filter = normalize_update_content_type_filter(args.content_types.as_deref());
    let check = check_instance_content_updates_inner(
        &client,
        &instance,
        &lock,
        UpdateScope::AllContent,
        content_type_filter.as_ref(),
    )?;

    if !check.updates.is_empty() {
        let _ = create_instance_snapshot(&instances_dir, &args.instance_id, "before-update-all");
    }
    let mut updated_entries = 0usize;
    let mut warnings = check.warnings.clone();
    let mut by_source: HashMap<String, usize> = HashMap::new();
    let mut by_content_type: HashMap<String, usize> = HashMap::new();
    let cf_key = curseforge_api_key();
    let prefetch_worker_cap = adaptive_update_prefetch_worker_cap(&check.updates);
    let prefetched_downloads =
        prefetch_update_downloads(&client, &check.updates, prefetch_worker_cap);

    for (idx, update) in check.updates.iter().enumerate() {
        let mut used_fast_path = false;
        let install_result = match try_fast_install_content_update(
            &instances_dir,
            &instance,
            &args,
            &client,
            cf_key.as_deref(),
            update,
            prefetched_downloads.get(&idx),
        ) {
            Ok(Some(installed)) => {
                used_fast_path = true;
                Ok(installed)
            }
            Ok(None) => install_discover_content_inner(
                app.clone(),
                &InstallDiscoverContentArgs {
                    instance_id: args.instance_id.clone(),
                    source: update.source.clone(),
                    project_id: update.project_id.clone(),
                    project_title: Some(update.name.clone()),
                    content_type: update.content_type.clone(),
                    target_worlds: update.target_worlds.clone(),
                },
                None,
            ),
            Err(fast_err) => {
                if update.source.trim().eq_ignore_ascii_case("curseforge")
                    && error_mentions_forbidden(&fast_err)
                {
                    warnings.push(format!(
                        "Skipped CurseForge update '{}' ({}): provider blocked automated download (403).",
                        update.name, update.project_id
                    ));
                    continue;
                }
                warnings.push(format!(
                    "Fast update fallback for '{}': {}",
                    update.name, fast_err
                ));
                install_discover_content_inner(
                    app.clone(),
                    &InstallDiscoverContentArgs {
                        instance_id: args.instance_id.clone(),
                        source: update.source.clone(),
                        project_id: update.project_id.clone(),
                        project_title: Some(update.name.clone()),
                        content_type: update.content_type.clone(),
                        target_worlds: update.target_worlds.clone(),
                    },
                    None,
                )
            }
        };

        match install_result {
            Ok(installed) => {
                if installed.version_id.trim() == update.current_version_id.trim() {
                    warnings.push(format!(
                        "No version change for '{}' (still {}).",
                        update.name,
                        if installed.version_number.trim().is_empty() {
                            installed.version_id.clone()
                        } else {
                            installed.version_number.clone()
                        }
                    ));
                    continue;
                }
                if !used_fast_path && update.content_type == "mods" && !update.enabled {
                    let disable_res = set_installed_mod_enabled(
                        app.clone(),
                        SetInstalledModEnabledArgs {
                            instance_id: args.instance_id.clone(),
                            version_id: installed.version_id,
                            enabled: false,
                        },
                    );
                    if let Err(err) = disable_res {
                        warnings.push(format!(
                            "Updated '{}' but failed to keep it disabled: {}",
                            update.name, err
                        ));
                    }
                }
                updated_entries += 1;
                *by_source.entry(update.source.clone()).or_insert(0) += 1;
                *by_content_type
                    .entry(update.content_type.clone())
                    .or_insert(0) += 1;
            }
            Err(err) => {
                warnings.push(format!("Failed to update '{}': {}", update.name, err));
            }
        }
    }

    Ok(UpdateAllContentResult {
        checked_entries: check.checked_entries,
        updated_entries,
        warnings,
        by_source,
        by_content_type,
    })
}

#[tauri::command]
pub(crate) async fn update_all_instance_content(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<UpdateAllContentResult, String> {
    run_blocking_task("update all instance content", move || {
        update_all_instance_content_inner(app, args)
    })
    .await
}

#[tauri::command]
pub(crate) async fn check_modrinth_updates(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<ModUpdateCheckResult, String> {
    run_blocking_task("check modrinth updates", move || {
        check_modrinth_updates_inner(app, args)
    })
    .await
}

fn check_modrinth_updates_inner(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<ModUpdateCheckResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let content = check_instance_content_updates_inner(
        &client,
        &instance,
        &lock,
        UpdateScope::ModrinthModsOnly,
        None,
    )?;
    Ok(content_updates_to_modrinth_result(content))
}

#[tauri::command]
pub(crate) async fn update_all_modrinth_mods(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<UpdateAllResult, String> {
    run_blocking_task("update all modrinth mods", move || {
        update_all_modrinth_mods_inner(app, args)
    })
    .await
}

fn update_all_modrinth_mods_inner(
    app: tauri::AppHandle,
    args: CheckUpdatesArgs,
) -> Result<UpdateAllResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let client = build_http_client()?;
    let check = check_instance_content_updates_inner(
        &client,
        &instance,
        &lock,
        UpdateScope::ModrinthModsOnly,
        None,
    )?;
    if !check.updates.is_empty() {
        let _ = create_instance_snapshot(&instances_dir, &args.instance_id, "before-update-all");
    }
    let mut updated_mods = 0usize;
    for update in &check.updates {
        match install_discover_content_inner(
            app.clone(),
            &InstallDiscoverContentArgs {
                instance_id: args.instance_id.clone(),
                source: "modrinth".to_string(),
                project_id: update.project_id.clone(),
                project_title: Some(update.name.clone()),
                content_type: "mods".to_string(),
                target_worlds: vec![],
            },
            None,
        ) {
            Ok(_) => updated_mods += 1,
            Err(_) => {}
        }
    }
    Ok(UpdateAllResult {
        checked_mods: check.checked_entries,
        updated_mods,
    })
}

#[tauri::command]
pub(crate) async fn launch_instance(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: LaunchInstanceArgs,
) -> Result<LaunchResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_settings = normalize_instance_settings(instance.settings.clone());
    let app_instance_dir = instance_dir_for_instance(&instances_dir, &instance);
    let settings = read_launcher_settings(&app)?;
    let method = if let Some(input) = args.method.as_ref() {
        LaunchMethod::parse(input).ok_or_else(|| "method must be prism or native".to_string())?
    } else {
        settings.default_launch_method.clone()
    };
    clear_launch_cancel_request(&state, &instance.id)?;
    if let Err(err) = mark_instance_launch_triggered(&instances_dir, &instance.id) {
        eprintln!(
            "instance last-run metadata launch marker write failed for '{}': {}",
            instance.id, err
        );
    }

    match method {
        LaunchMethod::Prism => {
            if is_launch_cancel_requested(&state, &instance.id)? {
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Prism.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }
            let prism_root = prism_root_dir()?;
            let prism_instance_id = find_prism_instance_id(&prism_root, &instance)?;
            let prism_mc_dir = prism_root
                .join("instances")
                .join(&prism_instance_id)
                .join("minecraft");

            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Prism.as_str(),
                "starting",
                "Preparing Prism sync…",
            );
            sync_prism_instance_content(&app_instance_dir, &prism_mc_dir)?;
            if is_launch_cancel_requested(&state, &instance.id)? {
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Prism.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }
            launch_prism_instance(&prism_root, &prism_instance_id)?;
            clear_launch_cancel_request(&state, &instance.id)?;

            Ok(LaunchResult {
                method: "prism".to_string(),
                launch_id: None,
                pid: None,
                prism_instance_id: Some(prism_instance_id),
                prism_root: Some(prism_root.display().to_string()),
                message: "Synced mods/config to Prism instance and launched it.".into(),
            })
        }
        LaunchMethod::Native => {
            let mut existing_native_runs_for_instance = 0usize;
            {
                let mut guard = state
                    .running
                    .lock()
                    .map_err(|_| "lock running instances failed".to_string())?;
                let mut finished: Vec<String> = Vec::new();
                for (id, proc_entry) in guard.iter_mut() {
                    if proc_entry.meta.instance_id != instance.id
                        || !proc_entry.meta.method.eq_ignore_ascii_case("native")
                    {
                        continue;
                    }
                    if let Ok(mut child) = proc_entry.child.lock() {
                        if let Ok(Some(_)) = child.try_wait() {
                            finished.push(id.clone());
                        } else {
                            existing_native_runs_for_instance += 1;
                        }
                    }
                }
                for id in finished {
                    guard.remove(&id);
                }
            }
            if is_launch_cancel_requested(&state, &instance.id)? {
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Native.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }

            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Preparing native launch…",
            );

            let java_executable = if !instance_settings.java_path.trim().is_empty() {
                let p = PathBuf::from(instance_settings.java_path.trim());
                if !p.exists() {
                    return Err(format!(
                        "Instance Java path does not exist: {}",
                        instance_settings.java_path
                    ));
                }
                p.display().to_string()
            } else {
                resolve_java_executable(&settings)?
            };
            let (java_major, java_version_line) = detect_java_major(&java_executable)?;
            let required_java = required_java_major_for_mc(&instance.mc_version);
            if java_major < required_java {
                return Err(format!(
                    "Java {} detected ({}), but Minecraft {} needs Java {}+. Update Java path in Instance Settings > Java & Memory or Settings > Launcher.",
                    java_major, java_version_line, instance.mc_version, required_java
                ));
            }
            if is_launch_cancel_requested(&state, &instance.id)? {
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Native.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }

            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Refreshing Microsoft session…",
            );
            let app_for_auth = app.clone();
            let settings_for_auth = settings.clone();
            let instance_for_auth = instance.clone();
            let (account, mc_access_token, loader, loader_version) =
                await_launch_stage_with_cancel(
                    &app,
                    &state,
                    &instance.id,
                    LaunchMethod::Native.as_str(),
                    "Authentication",
                    150,
                    async move {
                        tauri::async_runtime::spawn_blocking(move || {
                            resolve_native_auth_and_loader(
                                &app_for_auth,
                                &settings_for_auth,
                                &instance_for_auth,
                            )
                        })
                        .await
                        .map_err(|e| format!("native auth task join failed: {e}"))?
                    },
                )
                .await?;

            let launch_id = format!("native_{}", Uuid::new_v4());
            let use_isolated_runtime_session = existing_native_runs_for_instance > 0;
            let runtime_session_cleanup_dir = if use_isolated_runtime_session {
                Some(
                    app_instance_dir
                        .join("runtime_sessions")
                        .join(launch_id.replace(':', "_")),
                )
            } else {
                None
            };
            let runtime_dir = runtime_session_cleanup_dir
                .clone()
                .unwrap_or_else(|| app_instance_dir.join("runtime"));
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                if use_isolated_runtime_session {
                    "Preparing isolated runtime session…"
                } else {
                    "Preparing runtime files…"
                },
            );
            let app_instance_dir_for_sync = app_instance_dir.clone();
            let app_for_sync = app.clone();
            let runtime_dir_for_sync = runtime_dir.clone();
            let use_isolated_runtime_for_sync = use_isolated_runtime_session;
            await_launch_stage_with_cancel(
                &app,
                &state,
                &instance.id,
                LaunchMethod::Native.as_str(),
                if use_isolated_runtime_for_sync {
                    "Isolated runtime prep"
                } else {
                    "Runtime preparation"
                },
                150,
                async move {
                    tauri::async_runtime::spawn_blocking(move || {
                        fs::create_dir_all(&runtime_dir_for_sync)
                            .map_err(|e| format!("mkdir native runtime failed: {e}"))?;
                        if use_isolated_runtime_for_sync {
                            sync_instance_runtime_content_isolated(
                                &app_instance_dir_for_sync,
                                &runtime_dir_for_sync,
                            )?;
                        } else {
                            sync_instance_runtime_content(
                                &app_instance_dir_for_sync,
                                &runtime_dir_for_sync,
                            )?;
                        }
                        let cache_dir = launcher_cache_dir(&app_for_sync)?;
                        fs::create_dir_all(&cache_dir)
                            .map_err(|e| format!("mkdir launcher cache failed: {e}"))?;
                        wire_shared_cache(&cache_dir, &runtime_dir_for_sync)?;
                        Ok(())
                    })
                    .await
                    .map_err(|e| format!("runtime preparation task join failed: {e}"))?
                },
            )
            .await?;

            let runtime_dir_str = runtime_dir.display().to_string();
            let mc_version = instance.mc_version.clone();
            let username = account.username.clone();
            let profile_id = account.id.clone();

            let mut launcher = OpenLauncher::new(
                &runtime_dir_str,
                &java_executable,
                ol_version::Version {
                    minecraft_version: mc_version,
                    loader,
                    loader_version,
                },
            )
            .await;
            launcher.auth(ol_auth::Auth::new(
                "msa".to_string(),
                "{}".to_string(),
                username,
                profile_id,
                mc_access_token,
            ));
            launcher.jvm_arg(&format!("-Xmx{}M", instance_settings.memory_mb));
            for arg in effective_jvm_args(&instance_settings.jvm_args) {
                launcher.jvm_arg(&arg);
            }
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Installing game version files…",
            );
            await_launch_stage_with_cancel(
                &app,
                &state,
                &instance.id,
                LaunchMethod::Native.as_str(),
                "Version install",
                300,
                async {
                    launcher
                        .install_version()
                        .await
                        .map_err(|e| format!("native install version failed: {e}"))
                },
            )
            .await?;
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Installing assets…",
            );
            await_launch_stage_with_cancel(
                &app,
                &state,
                &instance.id,
                LaunchMethod::Native.as_str(),
                "Assets install",
                900,
                async {
                    launcher
                        .install_assets()
                        .await
                        .map_err(|e| format!("native install assets failed: {e}"))
                },
            )
            .await?;
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Installing libraries…",
            );
            await_launch_stage_with_cancel(
                &app,
                &state,
                &instance.id,
                LaunchMethod::Native.as_str(),
                "Libraries install",
                900,
                async {
                    launcher
                        .install_libraries()
                        .await
                        .map_err(|e| format!("native install libraries failed: {e}"))
                },
            )
            .await?;

            let persistent_logs_dir = launch_logs_dir(&app_instance_dir);
            fs::create_dir_all(&persistent_logs_dir)
                .map_err(|e| format!("create launch logs directory failed: {e}"))?;
            let launch_log_file_name = format!(
                "{}-{}.log",
                Local::now().format("%Y%m%d-%H%M%S"),
                launch_id.replace(':', "_")
            );
            let launch_log_path = persistent_logs_dir.join(launch_log_file_name);
            let launch_log_file = File::create(&launch_log_path)
                .map_err(|e| format!("create native launch log failed: {e}"))?;
            let launch_log_file_err = launch_log_file
                .try_clone()
                .map_err(|e| format!("clone native launch log handle failed: {e}"))?;
            emit_launch_state(
                &app,
                &instance.id,
                None,
                LaunchMethod::Native.as_str(),
                "starting",
                "Starting Java process…",
            );
            let mut command = launcher
                .command()
                .map_err(|e| format!("native launch command build failed: {e}"))?;
            command.stdout(Stdio::from(launch_log_file));
            command.stderr(Stdio::from(launch_log_file_err));
            let mut child = command
                .spawn()
                .map_err(|e| format!("native launch spawn failed: {e}"))?;
            if is_launch_cancel_requested(&state, &instance.id)? {
                let _ = child.kill();
                emit_launch_state(
                    &app,
                    &instance.id,
                    None,
                    LaunchMethod::Native.as_str(),
                    "stopped",
                    "Launch cancelled by user.",
                );
                clear_launch_cancel_request(&state, &instance.id)?;
                return Err("Launch cancelled by user.".to_string());
            }
            thread::sleep(Duration::from_millis(900));
            if let Ok(Some(status)) = child.try_wait() {
                if let Err(err) = mark_instance_launch_exit(&instances_dir, &instance.id, "crashed") {
                    eprintln!(
                        "instance last-run metadata crash marker write failed for '{}': {}",
                        instance.id, err
                    );
                }
                let tail = tail_lines_from_file(&launch_log_path, 24)
                    .map(|t| format!("\nRecent native-launch.log:\n{t}"))
                    .unwrap_or_default();
                return Err(format!(
                    "Native launch exited immediately with status {:?}. Check Java version/runtime mods. Log file: {}{}",
                    status.code(),
                    launch_log_path.display(),
                    tail
                ));
            }

            let pid = child.id();
            let child = Arc::new(Mutex::new(child));
            let keep_launcher_open = instance_settings.keep_launcher_open_while_playing;
            let close_launcher_on_exit = instance_settings.close_launcher_on_game_exit;
            let world_backup_interval_secs =
                u64::from(instance_settings.world_backup_interval_minutes.clamp(5, 15)) * 60;
            let world_backup_retention_count =
                usize::try_from(instance_settings.world_backup_retention_count.clamp(1, 2))
                    .unwrap_or(1);
            let log_path_text = launch_log_path.display().to_string();
            let running_meta = RunningInstance {
                launch_id: launch_id.clone(),
                instance_id: instance.id.clone(),
                instance_name: instance.name.clone(),
                method: "native".to_string(),
                pid,
                started_at: now_iso(),
                log_path: Some(log_path_text),
            };
            {
                let mut guard = state
                    .running
                    .lock()
                    .map_err(|_| "lock running instances failed".to_string())?;
                guard.insert(
                    launch_id.clone(),
                    RunningProcess {
                        meta: running_meta.clone(),
                        child: child.clone(),
                        log_path: Some(launch_log_path.clone()),
                    },
                );
            }
            clear_launch_cancel_request(&state, &instance.id)?;
            if !keep_launcher_open {
                if let Some(window) = app.get_window("main") {
                    let _ = window.minimize();
                }
            }
            emit_launch_state(
                &app,
                &instance.id,
                Some(&launch_id),
                LaunchMethod::Native.as_str(),
                "running",
                if use_isolated_runtime_session {
                    "Native launch started in isolated concurrent mode."
                } else {
                    "Native launch started."
                },
            );

            let running_state = state.running.clone();
            let stop_requested_state = state.stop_requested_launches.clone();
            let app_for_thread = app.clone();
            let launch_id_for_thread = launch_id.clone();
            let instance_id_for_thread = instance.id.clone();
            let instances_dir_for_thread = instances_dir.clone();
            let keep_launcher_open_for_thread = keep_launcher_open;
            let close_launcher_on_exit_for_thread = close_launcher_on_exit;
            let world_backup_interval_secs_for_thread = world_backup_interval_secs;
            let world_backup_retention_count_for_thread = world_backup_retention_count;
            let run_world_backups_for_thread = !use_isolated_runtime_session;
            let runtime_session_cleanup_for_thread = runtime_session_cleanup_dir.clone();
            thread::spawn(move || {
                let mut next_world_backup_at =
                    Instant::now() + Duration::from_secs(world_backup_interval_secs_for_thread);
                let (mut exit_kind, exit_message) = loop {
                    if run_world_backups_for_thread && Instant::now() >= next_world_backup_at {
                        let _ = create_world_backups_for_instance(
                            &instances_dir_for_thread,
                            &instance_id_for_thread,
                            "auto-world-backup",
                            world_backup_retention_count_for_thread,
                        );
                        next_world_backup_at = Instant::now()
                            + Duration::from_secs(world_backup_interval_secs_for_thread);
                    }
                    let waited = if let Ok(mut c) = child.lock() {
                        match c.try_wait() {
                            Ok(Some(status)) => {
                                Some((
                                    if status.success() {
                                        "success".to_string()
                                    } else {
                                        "crashed".to_string()
                                    },
                                    format!("Game exited with status {:?}", status.code()),
                                ))
                            }
                            Ok(None) => None,
                            Err(e) => Some((
                                "crashed".to_string(),
                                format!("Failed to wait for game process: {e}"),
                            )),
                        }
                    } else {
                        Some((
                            "crashed".to_string(),
                            "Failed to lock child process handle.".to_string(),
                        ))
                    };
                    if let Some(result) = waited {
                        break result;
                    }
                    thread::sleep(Duration::from_millis(450));
                };
                if let Ok(mut guard) = running_state.lock() {
                    guard.remove(&launch_id_for_thread);
                }
                let user_requested_stop = stop_requested_state
                    .lock()
                    .ok()
                    .map(|mut guard| guard.remove(&launch_id_for_thread))
                    .unwrap_or(false);
                let exit_message = if user_requested_stop {
                    exit_kind = "stopped".to_string();
                    "Instance stopped by user.".to_string()
                } else {
                    exit_message
                };
                if let Err(err) =
                    mark_instance_launch_exit(&instances_dir_for_thread, &instance_id_for_thread, &exit_kind)
                {
                    eprintln!(
                        "instance last-run metadata exit marker write failed for '{}': {}",
                        instance_id_for_thread, err
                    );
                }
                if let Some(path) = runtime_session_cleanup_for_thread {
                    let _ = remove_path_if_exists(&path);
                }
                emit_launch_state(
                    &app_for_thread,
                    &instance_id_for_thread,
                    Some(&launch_id_for_thread),
                    LaunchMethod::Native.as_str(),
                    "exited",
                    &exit_message,
                );
                if close_launcher_on_exit_for_thread {
                    app_for_thread.exit(0);
                    return;
                }
                if !keep_launcher_open_for_thread {
                    if let Some(window) = app_for_thread.get_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            });

            Ok(LaunchResult {
                method: "native".to_string(),
                launch_id: Some(launch_id),
                pid: Some(pid),
                prism_instance_id: None,
                prism_root: None,
                message: if use_isolated_runtime_session {
                    "Native launch started in isolated concurrent mode. This run uses temporary saves/config and will auto-clean on exit.".to_string()
                } else {
                    "Native launch started.".to_string()
                },
            })
        }
    }
}

fn count_occurrences(text: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    text.match_indices(needle).count()
}

fn replace_with_count(
    text: String,
    needle: &str,
    replacement: &str,
    applied: &mut usize,
) -> String {
    if needle.is_empty() || !text.contains(needle) {
        return text;
    }
    *applied += count_occurrences(&text, needle);
    text.replace(needle, replacement)
}

fn is_uuid_like(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '-'));
    if t.len() != 36 {
        return false;
    }
    let bytes = t.as_bytes();
    for (idx, ch) in bytes.iter().enumerate() {
        let ok = if [8, 13, 18, 23].contains(&idx) {
            *ch == b'-'
        } else {
            (*ch as char).is_ascii_hexdigit()
        };
        if !ok {
            return false;
        }
    }
    true
}

fn is_ipv4_like(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !(c.is_ascii_digit() || c == '.'));
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    for part in parts {
        if part.is_empty() || part.len() > 3 {
            return false;
        }
        if let Ok(value) = part.parse::<u16>() {
            if value > 255 {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

fn redact_sensitive_text(input: &str) -> (String, usize) {
    let mut out = input.to_string();
    let mut redactions = 0usize;
    if let Some(home) = home_dir() {
        let home_text = home.display().to_string();
        if !home_text.trim().is_empty() {
            out = replace_with_count(out, &home_text, "<HOME>", &mut redactions);
        }
    }

    let token_keys = [
        "access_token",
        "refresh_token",
        "authorization",
        "bearer",
        "xuid",
        "session",
    ];

    let mut lines_out: Vec<String> = Vec::new();
    for raw_line in out.lines() {
        let mut line = raw_line.to_string();
        let lower = line.to_ascii_lowercase();
        for key in token_keys {
            if !lower.contains(key) {
                continue;
            }
            if let Some(pos) = line.find('=') {
                line = format!("{}=<REDACTED>", line[..pos].trim_end());
                redactions += 1;
                break;
            }
            if let Some(pos) = line.find(':') {
                line = format!("{}: <REDACTED>", line[..pos].trim_end());
                redactions += 1;
                break;
            }
        }

        let words: Vec<String> = line
            .split_whitespace()
            .map(|token| {
                if is_uuid_like(token) || is_ipv4_like(token) {
                    redactions += 1;
                    "[REDACTED]".to_string()
                } else {
                    token.to_string()
                }
            })
            .collect();
        lines_out.push(words.join(" "));
    }
    (lines_out.join("\n"), redactions)
}

fn write_zip_text(
    zip: &mut zip::ZipWriter<File>,
    path: &str,
    text: &str,
    opts: FileOptions,
    files_count: &mut usize,
) -> Result<(), String> {
    zip.start_file(path, opts)
        .map_err(|e| format!("zip write header failed for '{path}': {e}"))?;
    zip.write_all(text.as_bytes())
        .map_err(|e| format!("zip write failed for '{path}': {e}"))?;
    *files_count += 1;
    Ok(())
}

fn detect_duplicate_enabled_mod_filenames(
    lock: &Lockfile,
    instance_dir: &Path,
) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for entry in &lock.entries {
        if !entry.enabled || normalize_lock_content_type(&entry.content_type) != "mods" {
            continue;
        }
        let (enabled_path, _) = mod_paths(instance_dir, &entry.filename);
        if !enabled_path.exists() {
            continue;
        }
        let key = entry.filename.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        *counts.entry(key).or_insert(0) += 1;
    }
    let mut out: Vec<(String, usize)> =
        counts.into_iter().filter(|(_, count)| *count > 1).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[tauri::command]
pub(crate) fn preflight_launch_compatibility(
    app: tauri::AppHandle,
    args: PreflightLaunchCompatibilityArgs,
) -> Result<LaunchCompatibilityReport, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_settings = normalize_instance_settings(instance.settings.clone());
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let settings = read_launcher_settings(&app)?;

    let mut items: Vec<LaunchCompatibilityItem> = Vec::new();
    let launch_method = args
        .method
        .as_deref()
        .and_then(LaunchMethod::parse)
        .unwrap_or(LaunchMethod::Native);
    if launch_method == LaunchMethod::Native {
        let required_java = required_java_major_for_mc(&instance.mc_version);
        let java_executable = if !instance_settings.java_path.trim().is_empty() {
            instance_settings.java_path.trim().to_string()
        } else {
            resolve_java_executable(&settings).unwrap_or_default()
        };
        if java_executable.trim().is_empty() {
            items.push(LaunchCompatibilityItem {
                code: "JAVA_PATH_UNRESOLVED".to_string(),
                title: "Java runtime path missing".to_string(),
                message: "Could not resolve a Java executable for this instance.".to_string(),
                severity: "blocker".to_string(),
                blocking: true,
            });
        } else if let Ok((java_major, version_line)) = detect_java_major(&java_executable) {
            if java_major < required_java {
                items.push(LaunchCompatibilityItem {
                    code: "JAVA_VERSION_INCOMPATIBLE".to_string(),
                    title: "Java version is too old".to_string(),
                    message: format!(
                        "Java {java_major} detected ({version_line}), but Minecraft {} needs Java {}+.",
                        instance.mc_version, required_java
                    ),
                    severity: "blocker".to_string(),
                    blocking: true,
                });
            }
        } else {
            items.push(LaunchCompatibilityItem {
                code: "JAVA_VERSION_CHECK_FAILED".to_string(),
                title: "Could not verify Java version".to_string(),
                message: "Launch may fail until Java runtime is verified.".to_string(),
                severity: "warning".to_string(),
                blocking: false,
            });
        }
    }

    let mut missing_enabled_mods = 0usize;
    let mut missing_enabled_non_mods = 0usize;
    for entry in &lock.entries {
        if !entry.enabled {
            continue;
        }
        if !entry_file_exists(&instance_dir, entry) {
            if normalize_lock_content_type(&entry.content_type) == "mods" {
                missing_enabled_mods += 1;
            } else {
                missing_enabled_non_mods += 1;
            }
        }
    }
    if missing_enabled_mods > 0 {
        items.push(LaunchCompatibilityItem {
            code: "MISSING_ENABLED_MOD_FILES".to_string(),
            title: "Enabled mod file missing".to_string(),
            message: format!("{missing_enabled_mods} enabled mod entries are missing on disk."),
            severity: "blocker".to_string(),
            blocking: true,
        });
    }
    if missing_enabled_non_mods > 0 {
        items.push(LaunchCompatibilityItem {
            code: "MISSING_ENABLED_NONMOD_FILES".to_string(),
            title: "Some enabled non-mod content is missing".to_string(),
            message: format!(
                "{missing_enabled_non_mods} enabled non-mod entries are missing on disk. This usually does not block launch."
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    let duplicates = detect_duplicate_enabled_mod_filenames(&lock, &instance_dir);
    if !duplicates.is_empty() {
        let preview = duplicates
            .iter()
            .take(3)
            .map(|(name, count)| format!("{name} ({count})"))
            .collect::<Vec<_>>()
            .join(", ");
        items.push(LaunchCompatibilityItem {
            code: "DUPLICATE_MOD_FILENAMES".to_string(),
            title: "Possible duplicate enabled mods".to_string(),
            message: format!(
                "Detected duplicate enabled mod filenames: {preview}. This can cause odd behavior, but may still launch."
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    let unresolved_local_entries = lock
        .entries
        .iter()
        .filter(|entry| entry.source.trim().eq_ignore_ascii_case("local"))
        .count();
    if unresolved_local_entries > 0 {
        items.push(LaunchCompatibilityItem {
            code: "LOCAL_ENTRIES_UNRESOLVED".to_string(),
            title: "Some local imports are unresolved".to_string(),
            message: format!(
                "{unresolved_local_entries} local entries are still source:\"local\" and may hide dependency/update metadata."
            ),
            severity: "warning".to_string(),
            blocking: false,
        });
    }

    if let Ok(store) = friend_link::store::read_store(&app) {
        if let Some(session) = friend_link::store::get_session(&store, &args.instance_id) {
            if !session.pending_conflicts.is_empty() {
                items.push(LaunchCompatibilityItem {
                    code: "FRIEND_LINK_PENDING_CONFLICTS".to_string(),
                    title: "Friend Link has pending conflicts".to_string(),
                    message: format!(
                        "{} conflicts pending; prelaunch reconcile may block launch.",
                        session.pending_conflicts.len()
                    ),
                    severity: "warning".to_string(),
                    blocking: false,
                });
            }
        }
    }

    let blocking_count = items.iter().filter(|item| item.blocking).count();
    let warning_count = items
        .iter()
        .filter(|item| !item.blocking && item.severity == "warning")
        .count();
    let status = if blocking_count > 0 {
        "blocked"
    } else if warning_count > 0 {
        "warning"
    } else {
        "ok"
    };

    Ok(LaunchCompatibilityReport {
        instance_id: args.instance_id,
        status: status.to_string(),
        checked_at: now_iso(),
        blocking_count,
        warning_count,
        unresolved_local_entries,
        items,
    })
}

#[tauri::command]
pub(crate) async fn export_instance_support_bundle(
    app: tauri::AppHandle,
    args: ExportInstanceSupportBundleArgs,
) -> Result<SupportBundleResult, String> {
    run_blocking_task("export instance support bundle", move || {
        export_instance_support_bundle_inner(app, args)
    })
    .await
}

fn export_instance_support_bundle_inner(
    app: tauri::AppHandle,
    args: ExportInstanceSupportBundleArgs,
) -> Result<SupportBundleResult, String> {
    let instances_dir = app_instances_dir(&app)?;
    let instance = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let include_raw_logs = args.include_raw_logs.unwrap_or(false);
    let output = if let Some(custom) = args.output_path.as_ref() {
        PathBuf::from(custom)
    } else {
        let base = home_dir()
            .map(|h| h.join("Downloads"))
            .filter(|p| p.exists())
            .unwrap_or_else(|| instance_dir.clone());
        let name = sanitize_filename(&instance.name.replace(' ', "-"));
        base.join(format!(
            "{}-support-bundle-{}.zip",
            if name.is_empty() {
                "instance"
            } else {
                name.as_str()
            },
            Local::now().format("%Y%m%d-%H%M%S")
        ))
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir export directory failed: {e}"))?;
    }

    let file = File::create(&output).map_err(|e| format!("create support bundle failed: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut files_count = 0usize;
    let mut redactions_applied = 0usize;

    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let installed = lock
        .entries
        .iter()
        .map(|entry| lock_entry_to_installed(&instance_dir, entry))
        .collect::<Vec<_>>();
    let installed_raw = serde_json::to_string_pretty(&installed)
        .map_err(|e| format!("serialize installed mods failed: {e}"))?;
    write_zip_text(
        &mut zip,
        "mods/installed_mods.json",
        &installed_raw,
        opts,
        &mut files_count,
    )?;

    let allowlist = friend_link::state::default_allowlist();
    let config_files = friend_link::state::collect_allowlisted_config_files(
        &instances_dir,
        &args.instance_id,
        &allowlist,
    )
    .unwrap_or_default();
    for file in &config_files {
        let (redacted, count) = redact_sensitive_text(&file.content);
        redactions_applied += count;
        write_zip_text(
            &mut zip,
            &format!("config/{}.redacted", file.path),
            &redacted,
            opts,
            &mut files_count,
        )?;
    }

    let log_targets = [
        ("logs/latest_launch", latest_launch_log_path(&instance_dir)),
        ("logs/latest_crash", latest_crash_report_path(&instance_dir)),
    ];
    for (base_name, maybe_path) in log_targets {
        let Some(path) = maybe_path else {
            continue;
        };
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let (redacted, count) = redact_sensitive_text(&raw);
        redactions_applied += count;
        write_zip_text(
            &mut zip,
            &format!("{base_name}.redacted.log"),
            &redacted,
            opts,
            &mut files_count,
        )?;
        if include_raw_logs {
            write_zip_text(
                &mut zip,
                &format!("{base_name}.raw.log"),
                &raw,
                opts,
                &mut files_count,
            )?;
        }
    }

    let perf_json = serde_json::to_string_pretty(&args.perf_actions)
        .map_err(|e| format!("serialize perf actions failed: {e}"))?;
    write_zip_text(
        &mut zip,
        "telemetry/perf_actions.json",
        &perf_json,
        opts,
        &mut files_count,
    )?;

    let manifest = serde_json::json!({
        "format": "openjar-support-bundle/v1",
        "generated_at": now_iso(),
        "instance": {
            "id": instance.id,
            "name": instance.name,
            "mc_version": instance.mc_version,
            "loader": instance.loader
        },
        "include_raw_logs": include_raw_logs,
        "files_count": files_count,
        "redactions_applied": redactions_applied,
        "config_files": config_files.len(),
        "mod_entries": installed.len(),
    });
    write_zip_text(
        &mut zip,
        "manifest.json",
        &serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("serialize manifest failed: {e}"))?,
        opts,
        &mut files_count,
    )?;

    zip.finish()
        .map_err(|e| format!("finalize support bundle failed: {e}"))?;

    Ok(SupportBundleResult {
        output_path: output.display().to_string(),
        files_count,
        redactions_applied,
        message: "Support bundle exported.".to_string(),
    })
}

#[tauri::command]
pub(crate) fn list_installed_mods(
    app: tauri::AppHandle,
    args: ListInstalledModsArgs,
) -> Result<Vec<InstalledMod>, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let lock = read_lockfile(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;

    let mut out: Vec<InstalledMod> = lock
        .entries
        .iter()
        .map(|e| lock_entry_to_installed(&instance_dir, e))
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub(crate) fn set_installed_mod_enabled(
    app: tauri::AppHandle,
    args: SetInstalledModEnabledArgs,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;

    let idx = lock
        .entries
        .iter()
        .position(|e| e.version_id == args.version_id)
        .ok_or_else(|| "installed mod entry not found".to_string())?;

    let mut changed = false;
    {
        let entry = &mut lock.entries[idx];
        let content_type = normalize_lock_content_type(&entry.content_type);
        let content_label = content_type_display_name(&content_type);

        if entry.enabled != args.enabled {
            match content_type.as_str() {
                "mods" | "resourcepacks" | "shaderpacks" => {
                    let (enabled_path, disabled_path) = if content_type == "mods" {
                        mod_paths(&instance_dir, &entry.filename)
                    } else {
                        content_paths_for_type(&instance_dir, &content_type, &entry.filename)
                    };
                    if args.enabled {
                        if enabled_path.exists() {
                            // already in place
                        } else if disabled_path.exists() {
                            if enabled_path.exists() {
                                fs::remove_file(&enabled_path).map_err(|e| {
                                    format!("remove existing enabled file failed: {e}")
                                })?;
                            }
                            fs::rename(&disabled_path, &enabled_path)
                                .map_err(|e| format!("enable {} failed: {e}", content_label))?;
                        } else {
                            return Err(format!("{} file not found on disk", content_label));
                        }
                    } else if disabled_path.exists() {
                        // already disabled path
                    } else if enabled_path.exists() {
                        if disabled_path.exists() {
                            fs::remove_file(&disabled_path).map_err(|e| {
                                format!("remove existing disabled file failed: {e}")
                            })?;
                        }
                        fs::rename(&enabled_path, &disabled_path)
                            .map_err(|e| format!("disable {} failed: {e}", content_label))?;
                    } else {
                        return Err(format!("{} file not found on disk", content_label));
                    }
                }
                "datapacks" => {
                    let target_worlds = if entry.target_worlds.is_empty() {
                        list_instance_world_names(&instance_dir)?
                    } else {
                        entry.target_worlds.clone()
                    };
                    let mut found_any = false;
                    for world in &target_worlds {
                        let (enabled_path, disabled_path) =
                            datapack_world_paths(&instance_dir, world, &entry.filename);
                        if args.enabled {
                            if enabled_path.exists() {
                                found_any = true;
                                continue;
                            }
                            if disabled_path.exists() {
                                found_any = true;
                                fs::rename(&disabled_path, &enabled_path).map_err(|e| {
                                    format!(
                                        "enable {} failed for world '{}': {e}",
                                        content_label, world
                                    )
                                })?;
                            }
                        } else {
                            if disabled_path.exists() {
                                found_any = true;
                                continue;
                            }
                            if enabled_path.exists() {
                                found_any = true;
                                fs::rename(&enabled_path, &disabled_path).map_err(|e| {
                                    format!(
                                        "disable {} failed for world '{}': {e}",
                                        content_label, world
                                    )
                                })?;
                            }
                        }
                    }
                    if !found_any {
                        return Err(format!("{} file not found on disk", content_label));
                    }
                }
                _ => {
                    return Err("Enable/disable is not supported for this content type".to_string())
                }
            }

            entry.enabled = args.enabled;
            changed = true;
        }
    }

    if changed {
        write_lockfile(&instances_dir, &args.instance_id, &lock)?;
    }

    let entry = lock.entries[idx].clone();
    Ok(lock_entry_to_installed(&instance_dir, &entry))
}

#[tauri::command]
pub(crate) fn set_installed_mod_provider(
    app: tauri::AppHandle,
    args: SetInstalledModProviderArgs,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;

    let idx = lock
        .entries
        .iter()
        .position(|e| e.version_id == args.version_id)
        .ok_or_else(|| "installed mod entry not found".to_string())?;

    let requested_source = args.source.trim().to_ascii_lowercase();
    if requested_source.is_empty() {
        return Err("source is required".to_string());
    }

    let entry = &mut lock.entries[idx];
    if entry.provider_candidates.is_empty() {
        entry.provider_candidates = lock_entry_provider_candidates(entry);
    }

    let candidate = entry
        .provider_candidates
        .iter()
        .find(|item| item.source.trim().eq_ignore_ascii_case(&requested_source))
        .cloned()
        .ok_or_else(|| "Requested provider is not available for this entry".to_string())?;

    apply_provider_candidate_to_lock_entry(entry, &candidate);
    write_lockfile(&instances_dir, &args.instance_id, &lock)?;

    let updated = lock.entries[idx].clone();
    Ok(lock_entry_to_installed(&instance_dir, &updated))
}

#[tauri::command]
pub(crate) fn remove_installed_mod(
    app: tauri::AppHandle,
    args: RemoveInstalledModArgs,
) -> Result<InstalledMod, String> {
    let instances_dir = app_instances_dir(&app)?;
    let _ = find_instance(&instances_dir, &args.instance_id)?;
    let instance_dir = instance_dir_for_id(&instances_dir, &args.instance_id)?;
    let mut lock = read_lockfile(&instances_dir, &args.instance_id)?;

    let idx = lock
        .entries
        .iter()
        .position(|e| e.version_id == args.version_id)
        .ok_or_else(|| "installed mod entry not found".to_string())?;
    let entry = lock.entries.remove(idx);
    let content_type = normalize_lock_content_type(&entry.content_type);

    match content_type.as_str() {
        "mods" | "resourcepacks" | "shaderpacks" => {
            let (enabled_path, disabled_path) = if content_type == "mods" {
                mod_paths(&instance_dir, &entry.filename)
            } else {
                content_paths_for_type(&instance_dir, &content_type, &entry.filename)
            };
            if enabled_path.exists() {
                fs::remove_file(&enabled_path).map_err(|e| {
                    format!(
                        "remove {} file '{}' failed: {e}",
                        content_type_display_name(&content_type),
                        enabled_path.display()
                    )
                })?;
            }
            if disabled_path.exists() {
                fs::remove_file(&disabled_path).map_err(|e| {
                    format!(
                        "remove disabled {} file '{}' failed: {e}",
                        content_type_display_name(&content_type),
                        disabled_path.display()
                    )
                })?;
            }
        }
        "datapacks" => {
            let target_worlds = if entry.target_worlds.is_empty() {
                list_instance_world_names(&instance_dir)?
            } else {
                entry.target_worlds.clone()
            };
            for world in &target_worlds {
                let (enabled_path, disabled_path) =
                    datapack_world_paths(&instance_dir, world, &entry.filename);
                if enabled_path.exists() {
                    fs::remove_file(&enabled_path).map_err(|e| {
                        format!(
                            "remove datapack file '{}' failed: {e}",
                            enabled_path.display()
                        )
                    })?;
                }
                if disabled_path.exists() {
                    fs::remove_file(&disabled_path).map_err(|e| {
                        format!(
                            "remove disabled datapack file '{}' failed: {e}",
                            disabled_path.display()
                        )
                    })?;
                }
            }
        }
        _ => {
            return Err("Delete is not supported for this content type".to_string());
        }
    }

    write_lockfile(&instances_dir, &args.instance_id, &lock)?;
    Ok(lock_entry_to_installed(&instance_dir, &entry))
}
