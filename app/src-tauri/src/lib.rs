use chrono::{
    DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, SecondsFormat, TimeZone,
    Timelike, Utc,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WindowEvent,
};
use walkdir::WalkDir;

const LOOKBACK_MINUTES: i64 = 5;
const HIGH_TOKEN_THRESHOLD: i64 = 200_000;
const ABNORMAL_TOKEN_THRESHOLD: i64 = 1_000_000;
const PRECEDING_ACTION_WINDOW_SECONDS: i64 = 60;
const PRODUCT_ASSET_PREFIX: &str = "CodexTokenUsage";
const DEFAULT_UPDATE_SOURCE: &str = "DonaldL81/codex-token-usage";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageFilters {
    date_from: Option<String>,
    date_to: Option<String>,
    project: Option<String>,
    session: Option<String>,
    search: Option<String>,
    only_anomalies: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardData {
    generated_at: String,
    data_dir: String,
    database_path: String,
    sessions_root: String,
    config: AppConfigDto,
    scan_state: ScanStateDto,
    metrics: MetricsDto,
    detail_rows: Vec<DetailRowDto>,
    summary_rows: Vec<SummaryRowDto>,
    daily_buckets: Vec<TrendBucketDto>,
    monthly_buckets: Vec<TrendBucketDto>,
    hourly_buckets: Vec<HourlyBucketDto>,
    composition: Vec<CompositionDto>,
    top_sessions: Vec<TopSessionDto>,
    top_projects: Vec<TopProjectDto>,
    project_options: Vec<FilterOptionDto>,
    session_options: Vec<FilterOptionDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigDto {
    sessions_root: String,
    include_archived: bool,
    refresh_interval_seconds: i64,
    include_messages_in_export: bool,
    retention_days: i64,
    update_source: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigInput {
    sessions_root: Option<String>,
    include_archived: Option<bool>,
    refresh_interval_seconds: Option<i64>,
    include_messages_in_export: Option<bool>,
    retention_days: Option<i64>,
    update_source: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResultDto {
    path: String,
    row_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildResultDto {
    backup_path: Option<String>,
    scan_state: ScanStateDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileDto {
    path: String,
    file_name: String,
    modified_local: String,
    size_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupActionDto {
    backup_path: Option<String>,
    restored_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionResultDto {
    backup_path: Option<String>,
    deleted_count: usize,
    cutoff_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUpdateInput {
    download_url: String,
    version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfoDto {
    current_version: String,
    source: String,
    latest_version: Option<String>,
    release_name: Option<String>,
    published_at: Option<String>,
    release_notes: Option<String>,
    download_url: Option<String>,
    release_page_url: Option<String>,
    has_update: bool,
    message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDownloadProgressDto {
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    percent: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUpdateResultDto {
    path: String,
    file_name: String,
    size_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallDownloadedUpdateInput {
    package_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRuntimeInfoDto {
    current_exe_path: String,
    stable_entry_path: String,
    stable_entry_exists: bool,
    is_stable_entry: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStableEntryResultDto {
    stable_entry_path: String,
    installed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallDownloadedUpdateResultDto {
    updater_script_path: String,
    stable_entry_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStateDto {
    last_cutoff_utc: Option<String>,
    last_run_utc: Option<String>,
    sessions_root: String,
    ledger_token_events: i64,
    last_run_new_token_events: i64,
    last_run_files_scanned: i64,
    last_run_parse_errors: i64,
    include_archived: bool,
    error: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsDto {
    total_tokens: i64,
    input_tokens: i64,
    cached_input_tokens: i64,
    non_cached_input_tokens: i64,
    output_tokens: i64,
    reasoning_output_tokens: i64,
    token_event_count: usize,
    project_count: usize,
    session_count: usize,
    turn_count: usize,
    user_message_count: usize,
    abnormal_count: usize,
    cache_rate: f64,
    daily_average_tokens: i64,
    hourly_peak_tokens: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailRowDto {
    row_key: String,
    parent_key: Option<String>,
    has_children: bool,
    level: u8,
    kind: String,
    node: String,
    node_tooltip: String,
    start_time: String,
    last_time: String,
    time: String,
    project: String,
    session_id: String,
    turn_id: String,
    event: String,
    input_tokens: i64,
    cached_input_tokens: i64,
    non_cached_input_tokens: i64,
    output_tokens: i64,
    reasoning_output_tokens: i64,
    total_tokens: i64,
    status: String,
    status_reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRowDto {
    row_key: String,
    parent_key: Option<String>,
    has_children: bool,
    level: u8,
    name: String,
    project: String,
    session_id: String,
    session_count: usize,
    message_count: usize,
    input_tokens: i64,
    cached_input_tokens: i64,
    output_tokens: i64,
    reasoning_output_tokens: i64,
    total_tokens: i64,
    status: String,
    status_reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlyBucketDto {
    date: String,
    hour: u32,
    total_tokens: i64,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendBucketDto {
    label: String,
    total_tokens: i64,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionDto {
    name: String,
    total_tokens: i64,
    ratio: f64,
    tone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopSessionDto {
    session_id: String,
    session_name: String,
    project: String,
    project_name: String,
    total_tokens: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopProjectDto {
    project: String,
    project_name: String,
    total_tokens: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterOptionDto {
    value: String,
    label: String,
    title: String,
    project: Option<String>,
}

#[derive(Debug, Clone)]
struct TokenEvent {
    _event_key: String,
    local_time: String,
    utc_time: String,
    date: String,
    hour: u32,
    session_id: String,
    turn_id: String,
    token_event_index: i64,
    project: String,
    cwd: String,
    user_message: String,
    user_message_preview: String,
    last_input_tokens: i64,
    last_cached_input_tokens: i64,
    last_output_tokens: i64,
    last_reasoning_output_tokens: i64,
    last_total_tokens: i64,
    primary_used_percent: Option<f64>,
    status: String,
    preceding_action: Option<String>,
}

#[derive(Debug, Default)]
struct DisplayNameMaps {
    session_titles: HashMap<String, String>,
    workspace_labels: HashMap<String, String>,
}

#[derive(Debug, Default, Clone)]
struct TokenSums {
    input_tokens: i64,
    cached_input_tokens: i64,
    output_tokens: i64,
    reasoning_output_tokens: i64,
    total_tokens: i64,
}

#[derive(Debug)]
struct ParsedTokenEvent {
    event_key: String,
    local_time: String,
    utc_time: String,
    date: String,
    hour: u32,
    session_id: String,
    turn_id: String,
    turn_key: String,
    token_event_index: i64,
    file_path: String,
    project: String,
    cwd: Option<String>,
    originator: Option<String>,
    source: Option<String>,
    cli_version: Option<String>,
    model_provider: Option<String>,
    session_started_local: Option<String>,
    turn_started_local: Option<String>,
    user_message: Option<String>,
    user_message_preview: Option<String>,
    model_context_window: Option<i64>,
    last_input_tokens: i64,
    last_cached_input_tokens: i64,
    last_output_tokens: i64,
    last_reasoning_output_tokens: i64,
    last_total_tokens: i64,
    session_input_tokens: i64,
    session_cached_input_tokens: i64,
    session_output_tokens: i64,
    session_reasoning_output_tokens: i64,
    session_total_tokens: i64,
    rate_limit_plan_type: Option<String>,
    primary_used_percent: Option<f64>,
    primary_window_minutes: Option<i64>,
    primary_resets_at_local: Option<String>,
    secondary_used_percent: Option<f64>,
    secondary_window_minutes: Option<i64>,
    secondary_resets_at_local: Option<String>,
    status: String,
    event_time_utc: DateTime<Utc>,
}

#[derive(Debug, Default)]
struct ParsedFile {
    events: Vec<ParsedTokenEvent>,
    context_actions: Vec<ParsedContextAction>,
    parse_errors: i64,
}

#[derive(Debug, Clone)]
struct ContextActionCandidate {
    event_time_utc: DateTime<Utc>,
    session_id: String,
    turn_id: String,
    action_type: String,
    tool_name: String,
    summary: String,
}

#[derive(Debug)]
struct ParsedContextAction {
    token_event_key: String,
    local_time: String,
    utc_time: String,
    session_id: String,
    turn_id: String,
    action_type: String,
    tool_name: String,
    summary: String,
}

#[derive(Debug, Clone)]
struct ToolCallContext {
    tool_name: String,
    summary: String,
}

#[derive(Default)]
struct SessionContext {
    session_id: Option<String>,
    session_started_local: Option<String>,
    cwd: Option<String>,
    originator: Option<String>,
    source: Option<String>,
    cli_version: Option<String>,
    model_provider: Option<String>,
    current_turn_id: Option<String>,
    current_turn_started_local: Option<String>,
    current_user_message: Option<String>,
    current_model_context_window: Option<i64>,
}

#[tauri::command]
async fn refresh_usage(filters: Option<UsageFilters>) -> Result<DashboardData, String> {
    let filters = filters.unwrap_or_default();

    tauri::async_runtime::spawn_blocking(move || {
        let paths = AppPaths::new().map_err(|error| error.to_string())?;
        let mut conn = open_database(&paths).map_err(|error| error.to_string())?;
        init_database(&conn).map_err(|error| error.to_string())?;

        let config = read_app_config(&conn).map_err(|error| error.to_string())?;
        let sessions_root = PathBuf::from(&config.sessions_root);
        let scan_state = match scan_sessions(&mut conn, &sessions_root, config.include_archived) {
            Ok(state) => state,
            Err(error) => {
                let mut state = read_scan_state(&conn, &sessions_root, config.include_archived)
                    .map_err(|e| e.to_string())?;
                state.error = Some(error.to_string());
                state
            }
        };

        query_dashboard(&conn, &paths, filters, config, scan_state)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("刷新任务执行失败：{error}"))?
}

#[tauri::command]
fn query_usage(filters: Option<UsageFilters>) -> Result<DashboardData, String> {
    let paths = AppPaths::new().map_err(|error| error.to_string())?;
    let conn = open_database(&paths).map_err(|error| error.to_string())?;
    init_database(&conn).map_err(|error| error.to_string())?;
    let config = read_app_config(&conn).map_err(|error| error.to_string())?;
    let sessions_root = PathBuf::from(&config.sessions_root);
    let scan_state = read_scan_state(&conn, &sessions_root, config.include_archived)
        .map_err(|error| error.to_string())?;

    query_dashboard(
        &conn,
        &paths,
        filters.unwrap_or_default(),
        config,
        scan_state,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_settings(config: AppConfigInput) -> Result<AppConfigDto, String> {
    let paths = AppPaths::new().map_err(|error| error.to_string())?;
    let conn = open_database(&paths).map_err(|error| error.to_string())?;
    init_database(&conn).map_err(|error| error.to_string())?;
    write_app_config(&conn, config).map_err(|error| error.to_string())
}

#[tauri::command]
fn export_detail_csv(filters: Option<UsageFilters>) -> Result<ExportResultDto, String> {
    export_usage_csv(ExportKind::Detail, filters.unwrap_or_default(), None)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn export_summary_csv(filters: Option<UsageFilters>) -> Result<ExportResultDto, String> {
    export_usage_csv(ExportKind::Summary, filters.unwrap_or_default(), None)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rebuild_ledger() -> Result<RebuildResultDto, String> {
    let paths = AppPaths::new().map_err(|error| error.to_string())?;
    let backup_path = backup_database_file(&paths).map_err(|error| error.to_string())?;
    let conn = open_database(&paths).map_err(|error| error.to_string())?;
    init_database(&conn).map_err(|error| error.to_string())?;
    let config = read_app_config(&conn).map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM token_events", [])
        .map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM token_context_actions", [])
        .map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM context_action_backfill_state", [])
        .map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM scan_state", [])
        .map_err(|error| error.to_string())?;
    let sessions_root = PathBuf::from(&config.sessions_root);
    let scan_state = read_scan_state(&conn, &sessions_root, config.include_archived)
        .map_err(|error| error.to_string())?;

    Ok(RebuildResultDto {
        backup_path: backup_path.map(|path| path.display().to_string()),
        scan_state,
    })
}

#[tauri::command]
fn backup_ledger() -> Result<BackupActionDto, String> {
    let paths = AppPaths::new().map_err(|error| error.to_string())?;
    let backup_path = backup_database_file(&paths).map_err(|error| error.to_string())?;
    Ok(BackupActionDto {
        backup_path: backup_path.map(|path| path.display().to_string()),
        restored_path: None,
    })
}

#[tauri::command]
fn list_backups() -> Result<Vec<BackupFileDto>, String> {
    let paths = AppPaths::new().map_err(|error| error.to_string())?;
    list_backup_files(&paths).map_err(|error| error.to_string())
}

#[tauri::command]
fn restore_ledger_backup(path: String) -> Result<BackupActionDto, String> {
    let paths = AppPaths::new().map_err(|error| error.to_string())?;
    let backup_dir = paths.data_dir.join("backups");
    let requested = PathBuf::from(path);
    if !path_is_inside(&backup_dir, &requested) {
        return Err("Backup path is outside the managed backups directory.".to_string());
    }
    if !requested.exists() {
        return Err(format!("Backup file not found: {}", requested.display()));
    }
    let pre_restore_backup = backup_database_file(&paths).map_err(|error| error.to_string())?;
    fs::copy(&requested, &paths.database_path).map_err(|error| error.to_string())?;
    Ok(BackupActionDto {
        backup_path: pre_restore_backup.map(|path| path.display().to_string()),
        restored_path: Some(requested.display().to_string()),
    })
}

#[tauri::command]
fn apply_retention_policy() -> Result<RetentionResultDto, String> {
    apply_retention().map_err(|error| error.to_string())
}

#[tauri::command]
fn check_update(source: Option<String>) -> Result<UpdateInfoDto, String> {
    let update_source = match source.map(|value| value.trim().to_string()) {
        Some(value) if !value.is_empty() => value,
        _ => {
            let paths = AppPaths::new().map_err(|error| error.to_string())?;
            let conn = open_database(&paths).map_err(|error| error.to_string())?;
            init_database(&conn).map_err(|error| error.to_string())?;
            read_app_config(&conn)
                .map_err(|error| error.to_string())?
                .update_source
        }
    };
    check_update_from_source(&update_source).map_err(|error| error.to_string())
}

#[tauri::command]
fn download_update_package(
    app: tauri::AppHandle,
    input: DownloadUpdateInput,
) -> Result<DownloadUpdateResultDto, String> {
    download_update_package_to_cache(&app, input).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_update_runtime_info() -> Result<UpdateRuntimeInfoDto, String> {
    update_runtime_info().map_err(|error| error.to_string())
}

#[tauri::command]
fn install_stable_entry() -> Result<InstallStableEntryResultDto, String> {
    install_current_exe_to_stable_entry().map_err(|error| error.to_string())
}

#[tauri::command]
fn install_downloaded_update(
    app: tauri::AppHandle,
    input: InstallDownloadedUpdateInput,
) -> Result<InstallDownloadedUpdateResultDto, String> {
    let result = start_external_updater(input).map_err(|error| error.to_string())?;
    let app_for_exit = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        app_for_exit.exit(0);
    });
    Ok(result)
}

#[tauri::command]
fn open_release_page(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !is_http_url(trimmed) {
        return Err("只能打开 http 或 https 发布页。".to_string());
    }
    Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", trimmed])
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(windows)]
            if let Ok(current_exe) = env::current_exe() {
                if let Err(error) = ensure_desktop_shortcut(&current_exe) {
                    eprintln!("failed to create desktop shortcut: {error}");
                }
            }

            let open = MenuItem::with_id(app, "open", "打开窗口", true, None::<&str>)?;
            let refresh = MenuItem::with_id(app, "refresh", "刷新统计", true, None::<&str>)?;
            let check_update =
                MenuItem::with_id(app, "check_update", "检查更新", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &refresh, &check_update, &quit])?;
            let tray_icon = Image::from_bytes(include_bytes!("../icons/tray-icon-24.png"))?;
            TrayIconBuilder::with_id("main")
                .icon(tray_icon)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .tooltip("Codex Token Usage")
                .build(app)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "refresh" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit("tray-refresh", ());
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "check_update" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit("tray-check-update", ());
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.minimize();
            }
        })
        .invoke_handler(tauri::generate_handler![
            refresh_usage,
            query_usage,
            save_settings,
            export_detail_csv,
            export_summary_csv,
            rebuild_ledger,
            backup_ledger,
            list_backups,
            restore_ledger_backup,
            apply_retention_policy,
            check_update,
            download_update_package,
            get_update_runtime_info,
            install_stable_entry,
            install_downloaded_update,
            open_release_page
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Codex Token Usage");
}

struct AppPaths {
    data_dir: PathBuf,
    database_path: PathBuf,
}

impl AppPaths {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir = default_data_dir();
        std::fs::create_dir_all(&data_dir)?;
        let database_path = data_dir.join("codex-token-usage.sqlite");
        Ok(Self {
            data_dir,
            database_path,
        })
    }
}

fn default_data_dir() -> PathBuf {
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        return PathBuf::from(local_app_data).join("CodexTokenUsage");
    }
    if let Ok(home) = env::var("HOME").or_else(|_| env::var("USERPROFILE")) {
        return PathBuf::from(home).join(".codex-token-usage");
    }
    PathBuf::from(".codex-token-usage")
}

fn update_runtime_info() -> Result<UpdateRuntimeInfoDto, Box<dyn std::error::Error>> {
    let current_exe = env::current_exe()?;
    let stable_entry = stable_entry_path()?;
    Ok(UpdateRuntimeInfoDto {
        current_exe_path: current_exe.display().to_string(),
        stable_entry_path: stable_entry.display().to_string(),
        stable_entry_exists: stable_entry.exists(),
        is_stable_entry: paths_refer_to_same_file(&current_exe, &stable_entry),
    })
}

fn install_current_exe_to_stable_entry(
) -> Result<InstallStableEntryResultDto, Box<dyn std::error::Error>> {
    let current_exe = env::current_exe()?;
    let stable_entry = stable_entry_path()?;
    if paths_refer_to_same_file(&current_exe, &stable_entry) {
        #[cfg(windows)]
        if let Err(error) = ensure_desktop_shortcut(&stable_entry) {
            eprintln!("failed to update desktop shortcut: {error}");
        }
        return Ok(InstallStableEntryResultDto {
            stable_entry_path: stable_entry.display().to_string(),
            installed: false,
        });
    }

    copy_exe_with_recovery(&current_exe, &stable_entry)?;
    write_stable_marker(&stable_entry)?;
    #[cfg(windows)]
    if let Err(error) = ensure_desktop_shortcut(&stable_entry) {
        eprintln!("failed to update desktop shortcut: {error}");
    }
    Ok(InstallStableEntryResultDto {
        stable_entry_path: stable_entry.display().to_string(),
        installed: true,
    })
}

fn start_external_updater(
    input: InstallDownloadedUpdateInput,
) -> Result<InstallDownloadedUpdateResultDto, Box<dyn std::error::Error>> {
    let paths = AppPaths::new()?;
    let update_dir = paths.data_dir.join("updates");
    fs::create_dir_all(&update_dir)?;

    let package_path = PathBuf::from(input.package_path);
    if !path_is_inside(&update_dir, &package_path) {
        return Err("更新包必须位于本工具管理的更新缓存目录。".into());
    }
    if !package_path.exists() {
        return Err(format!("更新包不存在：{}", package_path.display()).into());
    }
    if package_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| !value.eq_ignore_ascii_case("exe"))
        .unwrap_or(true)
    {
        return Err("更新包必须是 exe 文件。".into());
    }
    if package_path.metadata()?.len() < 128 * 1024 {
        return Err("更新包过小，已取消安装。".into());
    }

    let stable_entry = stable_entry_path()?;
    let marker_path = stable_marker_path()?;
    let updater_dir = paths.data_dir.join("updater");
    fs::create_dir_all(&updater_dir)?;
    let updater_script_path = updater_dir.join("Install-CodexTokenUsageUpdate.ps1");
    fs::write(&updater_script_path, updater_script())?;

    let mut command = Command::new("powershell");
    command
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&updater_script_path)
        .arg("-Source")
        .arg(&package_path)
        .arg("-Target")
        .arg(&stable_entry)
        .arg("-CurrentPid")
        .arg(std::process::id().to_string())
        .arg("-Marker")
        .arg(&marker_path);
    spawn_hidden(&mut command)?;

    Ok(InstallDownloadedUpdateResultDto {
        updater_script_path: updater_script_path.display().to_string(),
        stable_entry_path: stable_entry.display().to_string(),
    })
}

fn stable_program_dir() -> PathBuf {
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("Programs")
            .join("CodexTokenUsage");
    }
    default_data_dir().join("Programs").join("CodexTokenUsage")
}

fn stable_entry_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(stable_program_dir().join("CodexTokenUsage.exe"))
}

fn stable_marker_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(stable_program_dir().join(".stable-entry"))
}

#[cfg(windows)]
fn ensure_desktop_shortcut(target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let working_directory = target
        .parent()
        .ok_or_else(|| format!("快捷方式目标路径无父目录：{}", target.display()))?;
    let target_value = target.to_string_lossy().replace('\'', "''");
    let working_directory_value = working_directory.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$desktop = [Environment]::GetFolderPath([Environment+SpecialFolder]::DesktopDirectory); \
         New-Item -ItemType Directory -Force -Path $desktop | Out-Null; \
         $shortcutPath = Join-Path $desktop 'Codex Token Usage.lnk'; \
         $shell = New-Object -ComObject WScript.Shell; \
         $shortcut = $shell.CreateShortcut($shortcutPath); \
         $shortcut.TargetPath = '{target_value}'; \
         $shortcut.WorkingDirectory = '{working_directory_value}'; \
         $shortcut.IconLocation = '{target_value},0'; \
         $shortcut.Description = 'Codex Token Usage'; \
         $shortcut.Save()"
    );
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-WindowStyle",
        "Hidden",
        "-Command",
        script.as_str(),
    ]);
    spawn_hidden(&mut command)?;
    Ok(())
}

fn write_stable_marker(stable_entry: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let marker_path = stable_marker_path()?;
    if let Some(parent) = marker_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        marker_path,
        format!(
            "version={}\npath={}\nupdated_at={}\n",
            env!("CARGO_PKG_VERSION"),
            stable_entry.display(),
            Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
        ),
    )?;
    Ok(())
}

fn copy_exe_with_recovery(source: &Path, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !source.exists() {
        return Err(format!("源程序不存在：{}", source.display()).into());
    }
    if source.metadata()?.len() < 128 * 1024 {
        return Err("源程序文件过小，已取消复制。".into());
    }

    let target_dir = target
        .parent()
        .ok_or_else(|| format!("目标路径无父目录：{}", target.display()))?;
    fs::create_dir_all(target_dir)?;
    let temp_path = target.with_extension("exe.copying");
    let backup_path = target.with_extension("exe.previous");
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }
    fs::copy(source, &temp_path)?;
    if temp_path.metadata()?.len() < 128 * 1024 {
        let _ = fs::remove_file(&temp_path);
        return Err("复制后的程序文件过小，已取消替换。".into());
    }

    if backup_path.exists() {
        fs::remove_file(&backup_path)?;
    }
    if target.exists() {
        fs::rename(target, &backup_path)?;
    }

    match fs::rename(&temp_path, target) {
        Ok(_) => {
            let _ = fs::remove_file(&backup_path);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            if backup_path.exists() && !target.exists() {
                let _ = fs::rename(&backup_path, target);
            }
            Err(error.into())
        }
    }
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    let Ok(left) = left.canonicalize() else {
        return false;
    };
    let Ok(right) = right.canonicalize() else {
        return false;
    };
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn spawn_hidden(command: &mut Command) -> std::io::Result<std::process::Child> {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.spawn()
}

fn updater_script() -> &'static str {
    r#"
param(
    [Parameter(Mandatory = $true)][string]$Source,
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][int]$CurrentPid,
    [Parameter(Mandatory = $true)][string]$Marker
)

$ErrorActionPreference = "Stop"
$TargetDir = Split-Path -Parent $Target
$LogPath = Join-Path $TargetDir "update-last.log"

function Write-UpdateLog {
    param([string]$Message)
    New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
    Add-Content -LiteralPath $LogPath -Value ("{0} {1}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss"), $Message) -Encoding UTF8
}

try {
    Write-UpdateLog "start source=$Source target=$Target pid=$CurrentPid"
    if (-not (Test-Path -LiteralPath $Source)) {
        throw "Source package not found: $Source"
    }
    New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null

    for ($i = 0; $i -lt 60; $i++) {
        $process = Get-Process -Id $CurrentPid -ErrorAction SilentlyContinue
        if (-not $process) {
            break
        }
        Start-Sleep -Milliseconds 500
    }

    $process = Get-Process -Id $CurrentPid -ErrorAction SilentlyContinue
    if ($process) {
        Stop-Process -Id $CurrentPid -Force
        Start-Sleep -Milliseconds 500
    }

    $TempPath = "$Target.copying"
    $BackupPath = "$Target.previous"
    if (Test-Path -LiteralPath $TempPath) {
        Remove-Item -LiteralPath $TempPath -Force
    }

    $copied = $false
    for ($i = 0; $i -lt 10; $i++) {
        try {
            Copy-Item -LiteralPath $Source -Destination $TempPath -Force
            $copied = $true
            break
        } catch {
            Start-Sleep -Milliseconds 500
        }
    }
    if (-not $copied) {
        throw "Failed to copy update package to temp path."
    }
    if ((Get-Item -LiteralPath $TempPath).Length -lt 131072) {
        throw "Temp update file is too small."
    }

    if (Test-Path -LiteralPath $BackupPath) {
        Remove-Item -LiteralPath $BackupPath -Force
    }
    if (Test-Path -LiteralPath $Target) {
        Move-Item -LiteralPath $Target -Destination $BackupPath -Force
    }

    try {
        Move-Item -LiteralPath $TempPath -Destination $Target -Force
    } catch {
        if ((Test-Path -LiteralPath $BackupPath) -and -not (Test-Path -LiteralPath $Target)) {
            Move-Item -LiteralPath $BackupPath -Destination $Target -Force
        }
        throw
    }

    Set-Content -LiteralPath $Marker -Value ("updated_at={0}`npath={1}" -f (Get-Date -Format "o"), $Target) -Encoding UTF8
    Write-UpdateLog "success"
    Start-Process -FilePath $Target -WorkingDirectory $TargetDir
    exit 0
} catch {
    Write-UpdateLog ("failed " + $_.Exception.Message)
    if (Test-Path -LiteralPath $Target) {
        Start-Process -FilePath $Target -WorkingDirectory $TargetDir
    }
    exit 1
}
"#
}

fn check_update_from_source(source: &str) -> Result<UpdateInfoDto, Box<dyn std::error::Error>> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let source = source.trim();
    if source.is_empty() {
        return Ok(UpdateInfoDto {
            current_version,
            source: String::new(),
            latest_version: None,
            release_name: None,
            published_at: None,
            release_notes: None,
            download_url: None,
            release_page_url: None,
            has_update: false,
            message: "默认更新源为空，暂时无法检查更新。".to_string(),
        });
    }

    let api_url = normalize_update_source(source)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent(format!("{PRODUCT_ASSET_PREFIX}/{}", current_version))
        .build()?;
    let release: Value = client.get(&api_url).send()?.error_for_status()?.json()?;
    let tag = get_str(&release, "tag_name").unwrap_or_default();
    let latest_version = normalize_release_version(&tag);
    if latest_version.is_empty() {
        return Err("更新源响应中没有 tag_name，当前仅支持 GitHub Latest Release 格式。".into());
    }

    let selected_asset = select_release_asset(&release);
    let download_url = selected_asset.as_ref().map(|(_, url, _)| url.clone());
    let has_update = is_newer_version(&latest_version, &current_version);
    let message = if has_update {
        format!("发现新版本 {latest_version}")
    } else {
        format!("当前已是最新版本 {current_version}")
    };

    Ok(UpdateInfoDto {
        current_version,
        source: source.to_string(),
        latest_version: Some(latest_version),
        release_name: get_str(&release, "name"),
        published_at: get_str(&release, "published_at"),
        release_notes: get_str(&release, "body"),
        download_url,
        release_page_url: get_str(&release, "html_url"),
        has_update,
        message,
    })
}

fn download_update_package_to_cache(
    app: &tauri::AppHandle,
    input: DownloadUpdateInput,
) -> Result<DownloadUpdateResultDto, Box<dyn std::error::Error>> {
    let url = input.download_url.trim();
    if !is_http_url(url) {
        return Err("下载地址必须是 http 或 https。".into());
    }
    let paths = AppPaths::new()?;
    let update_dir = paths.data_dir.join("updates");
    fs::create_dir_all(&update_dir)?;

    let file_name = update_file_name(url, input.version.as_deref());
    let target_path = update_dir.join(&file_name);
    let temp_path = update_dir.join(format!("{file_name}.download"));
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }

    emit_download_progress(app, 0, None);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent(format!(
            "{PRODUCT_ASSET_PREFIX}/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?;
    let mut response = client.get(url).send()?.error_for_status()?;
    let total = response.content_length();
    let mut file = File::create(&temp_path)?;
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let size = response.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        file.write_all(&buffer[..size])?;
        downloaded += size as u64;
        emit_download_progress(app, downloaded, total);
    }
    file.flush()?;

    if let Some(total) = total {
        if downloaded != total {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("下载大小不完整：{downloaded}/{total} bytes").into());
        }
    }
    if downloaded < 128 * 1024 {
        let _ = fs::remove_file(&temp_path);
        return Err("下载文件过小，已取消保存。".into());
    }
    if target_path.exists() {
        fs::remove_file(&target_path)?;
    }
    fs::rename(&temp_path, &target_path)?;

    Ok(DownloadUpdateResultDto {
        path: target_path.display().to_string(),
        file_name,
        size_bytes: downloaded,
    })
}

fn normalize_update_source(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    let trimmed = source.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("更新源为空。".into());
    }
    if trimmed.starts_with("https://api.github.com/repos/") && trimmed.ends_with("/releases/latest")
    {
        return Ok(trimmed.to_string());
    }
    if let Some(repo) = trimmed.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = repo.split('/').filter(|part| !part.is_empty()).collect();
        if parts.len() >= 2 {
            return Ok(format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                parts[0], parts[1]
            ));
        }
    }
    if !trimmed.contains("://") && trimmed.matches('/').count() == 1 {
        let parts: Vec<&str> = trimmed.split('/').collect();
        return Ok(format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            parts[0], parts[1]
        ));
    }
    if is_http_url(trimmed) {
        return Ok(trimmed.to_string());
    }
    Err("更新源格式不支持，请填写 owner/repo、GitHub 仓库地址或 Latest Release API。".into())
}

fn select_release_asset(release: &Value) -> Option<(String, String, u64)> {
    let assets = release.get("assets")?.as_array()?;
    let mut candidates = assets.iter().filter_map(|asset| {
        let name = get_str(asset, "name")?;
        let url = get_str(asset, "browser_download_url")?;
        let size = asset.get("size").and_then(Value::as_u64).unwrap_or(0);
        Some((name, url, size))
    });
    let all = candidates.by_ref().collect::<Vec<_>>();
    all.iter()
        .find(|(name, _, _)| {
            let lower = name.to_ascii_lowercase();
            lower.contains(&PRODUCT_ASSET_PREFIX.to_ascii_lowercase())
                && lower.contains("portable")
                && lower.ends_with(".exe")
        })
        .or_else(|| {
            all.iter().find(|(name, _, _)| {
                let lower = name.to_ascii_lowercase();
                lower.contains(&PRODUCT_ASSET_PREFIX.to_ascii_lowercase())
                    && lower.ends_with(".exe")
            })
        })
        .or_else(|| {
            all.iter()
                .find(|(name, _, _)| name.to_ascii_lowercase().ends_with(".exe"))
        })
        .cloned()
}

fn update_file_name(download_url: &str, version: Option<&str>) -> String {
    let from_url = download_url
        .rsplit('/')
        .next()
        .and_then(|value| value.split('?').next())
        .map(sanitize_file_name)
        .filter(|value| value.to_ascii_lowercase().ends_with(".exe"));
    from_url.unwrap_or_else(|| {
        let version = version
            .map(normalize_release_version)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
        format!("{PRODUCT_ASSET_PREFIX}-{version}-windows-x64-portable.exe")
    })
}

fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('.')
        .trim()
        .to_string()
}

fn emit_download_progress(app: &tauri::AppHandle, downloaded_bytes: u64, total_bytes: Option<u64>) {
    let percent = total_bytes
        .filter(|total| *total > 0)
        .map(|total| (downloaded_bytes as f64 / total as f64) * 100.0);
    let _ = app.emit(
        "update-download-progress",
        UpdateDownloadProgressDto {
            downloaded_bytes,
            total_bytes,
            percent,
        },
    );
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

fn normalize_release_version(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    match (
        parse_version_triplet(latest),
        parse_version_triplet(current),
    ) {
        (Some(latest), Some(current)) => latest > current,
        _ => latest != current,
    }
}

fn parse_version_triplet(value: &str) -> Option<(u64, u64, u64)> {
    let version = normalize_release_version(value);
    let stable = version.split('-').next()?;
    let parts = stable
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.len() != 3 {
        return None;
    }
    Some((parts[0], parts[1], parts[2]))
}

fn default_sessions_root() -> PathBuf {
    if let Ok(codex_home) = env::var("CODEX_HOME") {
        return PathBuf::from(codex_home).join("sessions");
    }
    if let Ok(home) = env::var("HOME").or_else(|_| env::var("USERPROFILE")) {
        return PathBuf::from(home).join(".codex").join("sessions");
    }
    PathBuf::from(".codex").join("sessions")
}

fn default_codex_home() -> Option<PathBuf> {
    if let Ok(codex_home) = env::var("CODEX_HOME") {
        return Some(PathBuf::from(codex_home));
    }
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .ok()
        .map(|home| PathBuf::from(home).join(".codex"))
}

fn codex_home_from_sessions_root(sessions_root: &str) -> Option<PathBuf> {
    let root = PathBuf::from(sessions_root);
    let leaf = root.file_name()?.to_string_lossy().to_ascii_lowercase();
    if leaf == "sessions" || leaf == "archived_sessions" {
        root.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

fn load_display_name_maps(sessions_root: &str) -> DisplayNameMaps {
    let codex_home = codex_home_from_sessions_root(sessions_root).or_else(default_codex_home);
    let Some(codex_home) = codex_home else {
        return DisplayNameMaps::default();
    };

    DisplayNameMaps {
        session_titles: read_session_titles(&codex_home),
        workspace_labels: read_workspace_labels(&codex_home),
    }
}

fn read_session_titles(codex_home: &Path) -> HashMap<String, String> {
    let path = codex_home.join("session_index.jsonl");
    let Ok(file) = File::open(path) else {
        return HashMap::new();
    };
    let reader = BufReader::new(file);
    let mut titles = HashMap::new();

    for line in reader.lines().map_while(Result::ok) {
        let Ok(record) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = get_str(&record, "id").filter(|value| !value.is_empty()) else {
            continue;
        };
        let Some(title) = get_str(&record, "thread_name").filter(|value| !value.is_empty()) else {
            continue;
        };
        titles.insert(id, title);
    }

    titles
}

fn read_workspace_labels(codex_home: &Path) -> HashMap<String, String> {
    let path = codex_home.join(".codex-global-state.json");
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let Ok(record) = serde_json::from_str::<Value>(&text) else {
        return HashMap::new();
    };
    let Some(labels) = record
        .get("electron-workspace-root-labels")
        .and_then(Value::as_object)
    else {
        return HashMap::new();
    };

    labels
        .iter()
        .filter_map(|(path, label)| {
            let label = label.as_str()?.trim();
            if label.is_empty() {
                None
            } else {
                Some((normalize_codex_path(path), label.to_string()))
            }
        })
        .collect()
}

fn normalize_codex_path(path: &str) -> String {
    let trimmed = path.trim().trim_start_matches("\\\\?\\");
    trimmed
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn default_app_config() -> AppConfigDto {
    AppConfigDto {
        sessions_root: default_sessions_root().display().to_string(),
        include_archived: true,
        refresh_interval_seconds: 0,
        include_messages_in_export: false,
        retention_days: 0,
        update_source: DEFAULT_UPDATE_SOURCE.to_string(),
    }
}

fn read_app_config(conn: &Connection) -> rusqlite::Result<AppConfigDto> {
    let default_config = default_app_config();
    let config = conn
        .query_row(
            r#"
            SELECT sessions_root, include_archived, refresh_interval_seconds,
                include_messages_in_export, retention_days, update_source
            FROM app_config WHERE id = 1
            "#,
            [],
            |row| {
                let sessions_root: Option<String> = row.get(0)?;
                Ok(AppConfigDto {
                    sessions_root: sessions_root
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| default_config.sessions_root.clone()),
                    include_archived: true,
                    refresh_interval_seconds: normalize_refresh_interval(row.get(2)?),
                    include_messages_in_export: row.get::<_, i64>(3)? != 0,
                    retention_days: normalize_retention_days(row.get(4)?),
                    update_source: row
                        .get::<_, Option<String>>(5)?
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| default_config.update_source.clone()),
                })
            },
        )
        .optional()?;

    Ok(config.unwrap_or(default_config))
}

fn write_app_config(conn: &Connection, input: AppConfigInput) -> rusqlite::Result<AppConfigDto> {
    let current = read_app_config(conn)?;
    let sessions_root = input
        .sessions_root
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(current.sessions_root);
    let _ignored_include_archived = input.include_archived;
    let include_archived = true;
    let refresh_interval_seconds = input
        .refresh_interval_seconds
        .map(normalize_refresh_interval)
        .unwrap_or(current.refresh_interval_seconds);
    let include_messages_in_export = input
        .include_messages_in_export
        .unwrap_or(current.include_messages_in_export);
    let retention_days = input
        .retention_days
        .map(normalize_retention_days)
        .unwrap_or(current.retention_days);
    let _ignored_update_source = input.update_source;
    let update_source = DEFAULT_UPDATE_SOURCE.to_string();

    conn.execute(
        r#"
        INSERT INTO app_config (
            id, sessions_root, include_archived, refresh_interval_seconds,
            include_messages_in_export, retention_days, update_source
        )
        VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(id) DO UPDATE SET
            sessions_root = excluded.sessions_root,
            include_archived = excluded.include_archived,
            refresh_interval_seconds = excluded.refresh_interval_seconds,
            include_messages_in_export = excluded.include_messages_in_export,
            retention_days = excluded.retention_days,
            update_source = excluded.update_source
        "#,
        params![
            sessions_root,
            include_archived as i64,
            refresh_interval_seconds,
            include_messages_in_export as i64,
            retention_days,
            update_source
        ],
    )?;

    read_app_config(conn)
}

fn normalize_refresh_interval(seconds: i64) -> i64 {
    if seconds <= 0 {
        0
    } else {
        seconds.clamp(30, 86_400)
    }
}

fn normalize_retention_days(days: i64) -> i64 {
    days.clamp(0, 3650)
}

fn open_database(paths: &AppPaths) -> rusqlite::Result<Connection> {
    Connection::open(&paths.database_path)
}

fn init_database(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS token_events (
            event_key TEXT PRIMARY KEY,
            local_time TEXT NOT NULL,
            utc_time TEXT NOT NULL,
            date TEXT NOT NULL,
            hour INTEGER NOT NULL,
            session_id TEXT NOT NULL,
            turn_id TEXT NOT NULL,
            turn_key TEXT NOT NULL,
            token_event_index INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            project TEXT NOT NULL,
            cwd TEXT,
            originator TEXT,
            source TEXT,
            cli_version TEXT,
            model_provider TEXT,
            session_started_local TEXT,
            turn_started_local TEXT,
            user_message TEXT,
            user_message_preview TEXT,
            model_context_window INTEGER,
            last_input_tokens INTEGER NOT NULL,
            last_cached_input_tokens INTEGER NOT NULL,
            last_output_tokens INTEGER NOT NULL,
            last_reasoning_output_tokens INTEGER NOT NULL,
            last_total_tokens INTEGER NOT NULL,
            session_input_tokens INTEGER NOT NULL,
            session_cached_input_tokens INTEGER NOT NULL,
            session_output_tokens INTEGER NOT NULL,
            session_reasoning_output_tokens INTEGER NOT NULL,
            session_total_tokens INTEGER NOT NULL,
            rate_limit_plan_type TEXT,
            primary_used_percent REAL,
            primary_window_minutes INTEGER,
            primary_resets_at_local TEXT,
            secondary_used_percent REAL,
            secondary_window_minutes INTEGER,
            secondary_resets_at_local TEXT,
            status TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS scan_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            last_cutoff_utc TEXT,
            last_run_utc TEXT,
            sessions_root TEXT NOT NULL,
            ledger_token_events INTEGER NOT NULL DEFAULT 0,
            last_run_new_token_events INTEGER NOT NULL DEFAULT 0,
            last_run_files_scanned INTEGER NOT NULL DEFAULT 0,
            last_run_parse_errors INTEGER NOT NULL DEFAULT 0,
            include_archived INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS app_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            sessions_root TEXT,
            include_archived INTEGER NOT NULL DEFAULT 1,
            refresh_interval_seconds INTEGER NOT NULL DEFAULT 0,
            include_messages_in_export INTEGER NOT NULL DEFAULT 0,
            retention_days INTEGER NOT NULL DEFAULT 0,
            update_source TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS token_context_actions (
            token_event_key TEXT PRIMARY KEY,
            local_time TEXT NOT NULL,
            utc_time TEXT NOT NULL,
            session_id TEXT NOT NULL,
            turn_id TEXT NOT NULL,
            action_type TEXT NOT NULL,
            tool_name TEXT NOT NULL,
            summary TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS context_action_backfill_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            sessions_root TEXT NOT NULL,
            include_archived INTEGER NOT NULL DEFAULT 0,
            completed_utc TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_token_events_date ON token_events(date);
        CREATE INDEX IF NOT EXISTS idx_token_events_project ON token_events(project);
        CREATE INDEX IF NOT EXISTS idx_token_events_session ON token_events(session_id);
        CREATE INDEX IF NOT EXISTS idx_token_events_turn ON token_events(turn_key);
        CREATE INDEX IF NOT EXISTS idx_token_context_actions_turn ON token_context_actions(session_id, turn_id);
        "#,
    )?;
    ensure_column(
        conn,
        "scan_state",
        "last_run_parse_errors",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "app_config",
        "retention_days",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "app_config",
        "update_source",
        "TEXT NOT NULL DEFAULT ''",
    )
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|name| name == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

fn scan_sessions(
    conn: &mut Connection,
    sessions_root: &Path,
    include_archived: bool,
) -> Result<ScanStateDto, Box<dyn std::error::Error>> {
    if !sessions_root.exists() {
        return Err(format!(
            "Codex sessions root does not exist: {}",
            sessions_root.display()
        )
        .into());
    }

    let previous_cutoff = read_last_cutoff(conn)?;
    let scan_cutoff = previous_cutoff.map(|cutoff| cutoff - Duration::minutes(LOOKBACK_MINUTES));
    let mut files_scanned = 0_i64;
    let mut new_events = 0_i64;
    let mut parse_errors = 0_i64;
    let mut latest_event_utc = previous_cutoff;

    let tx = conn.transaction()?;
    for root in scan_roots(sessions_root, include_archived) {
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            if let Some(cutoff) = scan_cutoff {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let modified_utc: DateTime<Utc> = modified.into();
                        if modified_utc < cutoff {
                            continue;
                        }
                    }
                }
            }

            files_scanned += 1;
            let parsed_file = parse_codex_jsonl(entry.path())?;
            parse_errors += parsed_file.parse_errors;
            for parsed in &parsed_file.events {
                latest_event_utc = Some(match latest_event_utc {
                    Some(current) => current.max(parsed.event_time_utc),
                    None => parsed.event_time_utc,
                });
                new_events += insert_token_event(&tx, parsed)? as i64;
            }
            for action in &parsed_file.context_actions {
                insert_context_action(&tx, action)?;
            }
        }
    }

    tx.commit()?;
    if previous_cutoff.is_none() {
        write_context_action_backfill_state(conn, sessions_root, include_archived)?;
    } else {
        ensure_context_actions_backfilled(conn, sessions_root, include_archived)?;
    }

    let ledger_events: i64 =
        conn.query_row("SELECT COUNT(*) FROM token_events", [], |row| row.get(0))?;
    let cutoff_text = latest_event_utc.map(format_utc_iso);
    let last_run_utc = format_utc_iso(Utc::now());

    conn.execute(
        r#"
        INSERT INTO scan_state (
            id, last_cutoff_utc, last_run_utc, sessions_root, ledger_token_events,
            last_run_new_token_events, last_run_files_scanned, last_run_parse_errors,
            include_archived
        )
        VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(id) DO UPDATE SET
            last_cutoff_utc = excluded.last_cutoff_utc,
            last_run_utc = excluded.last_run_utc,
            sessions_root = excluded.sessions_root,
            ledger_token_events = excluded.ledger_token_events,
            last_run_new_token_events = excluded.last_run_new_token_events,
            last_run_files_scanned = excluded.last_run_files_scanned,
            last_run_parse_errors = excluded.last_run_parse_errors,
            include_archived = excluded.include_archived
        "#,
        params![
            cutoff_text,
            last_run_utc,
            sessions_root.display().to_string(),
            ledger_events,
            new_events,
            files_scanned,
            parse_errors,
            include_archived as i64
        ],
    )?;

    read_scan_state(conn, sessions_root, include_archived).map_err(Into::into)
}

fn scan_roots(sessions_root: &Path, include_archived: bool) -> Vec<PathBuf> {
    let mut roots = vec![sessions_root.to_path_buf()];
    if include_archived {
        if let Some(codex_home) = sessions_root.parent() {
            let archived = codex_home.join("archived_sessions");
            if archived.exists() {
                roots.push(archived);
            }
        }
    }
    roots
}

fn insert_token_event(conn: &Connection, event: &ParsedTokenEvent) -> rusqlite::Result<usize> {
    conn.execute(
        r#"
        INSERT OR IGNORE INTO token_events (
            event_key, local_time, utc_time, date, hour, session_id, turn_id, turn_key,
            token_event_index, file_path, project, cwd, originator, source, cli_version,
            model_provider, session_started_local, turn_started_local, user_message,
            user_message_preview, model_context_window, last_input_tokens,
            last_cached_input_tokens, last_output_tokens, last_reasoning_output_tokens,
            last_total_tokens, session_input_tokens, session_cached_input_tokens,
            session_output_tokens, session_reasoning_output_tokens, session_total_tokens,
            rate_limit_plan_type, primary_used_percent, primary_window_minutes,
            primary_resets_at_local, secondary_used_percent, secondary_window_minutes,
            secondary_resets_at_local, status
        )
        VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
            ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39
        )
        "#,
        params![
            event.event_key,
            event.local_time,
            event.utc_time,
            event.date,
            event.hour,
            event.session_id,
            event.turn_id,
            event.turn_key,
            event.token_event_index,
            event.file_path,
            event.project,
            event.cwd,
            event.originator,
            event.source,
            event.cli_version,
            event.model_provider,
            event.session_started_local,
            event.turn_started_local,
            event.user_message,
            event.user_message_preview,
            event.model_context_window,
            event.last_input_tokens,
            event.last_cached_input_tokens,
            event.last_output_tokens,
            event.last_reasoning_output_tokens,
            event.last_total_tokens,
            event.session_input_tokens,
            event.session_cached_input_tokens,
            event.session_output_tokens,
            event.session_reasoning_output_tokens,
            event.session_total_tokens,
            event.rate_limit_plan_type,
            event.primary_used_percent,
            event.primary_window_minutes,
            event.primary_resets_at_local,
            event.secondary_used_percent,
            event.secondary_window_minutes,
            event.secondary_resets_at_local,
            event.status
        ],
    )
}

fn insert_context_action(
    conn: &Connection,
    action: &ParsedContextAction,
) -> rusqlite::Result<usize> {
    conn.execute(
        r#"
        INSERT OR REPLACE INTO token_context_actions (
            token_event_key, local_time, utc_time, session_id, turn_id,
            action_type, tool_name, summary
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        params![
            action.token_event_key,
            action.local_time,
            action.utc_time,
            action.session_id,
            action.turn_id,
            action.action_type,
            action.tool_name,
            action.summary
        ],
    )
}

fn ensure_context_actions_backfilled(
    conn: &mut Connection,
    sessions_root: &Path,
    include_archived: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if is_context_action_backfill_current(conn, sessions_root, include_archived)? {
        return Ok(());
    }

    let token_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM token_events", [], |row| row.get(0))?;
    if token_count > 0 {
        let tx = conn.transaction()?;
        for root in scan_roots(sessions_root, include_archived) {
            for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
                if !entry.file_type().is_file() {
                    continue;
                }
                if entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl") {
                    continue;
                }
                let parsed_file = parse_codex_jsonl(entry.path())?;
                for action in &parsed_file.context_actions {
                    insert_context_action(&tx, action)?;
                }
            }
        }
        tx.commit()?;
    }

    write_context_action_backfill_state(conn, sessions_root, include_archived)?;
    Ok(())
}

fn is_context_action_backfill_current(
    conn: &Connection,
    sessions_root: &Path,
    include_archived: bool,
) -> rusqlite::Result<bool> {
    let expected_root = sessions_root.display().to_string();
    let expected_archived = include_archived as i64;
    let current = conn
        .query_row(
            r#"
            SELECT sessions_root, include_archived
            FROM context_action_backfill_state
            WHERE id = 1
            "#,
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    Ok(matches!(
        current,
        Some((root, archived)) if root == expected_root && archived == expected_archived
    ))
}

fn write_context_action_backfill_state(
    conn: &Connection,
    sessions_root: &Path,
    include_archived: bool,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"
        INSERT INTO context_action_backfill_state (
            id, sessions_root, include_archived, completed_utc
        )
        VALUES (1, ?1, ?2, ?3)
        ON CONFLICT(id) DO UPDATE SET
            sessions_root = excluded.sessions_root,
            include_archived = excluded.include_archived,
            completed_utc = excluded.completed_utc
        "#,
        params![
            sessions_root.display().to_string(),
            include_archived as i64,
            format_utc_iso(Utc::now())
        ],
    )?;
    Ok(())
}

fn parse_codex_jsonl(path: &Path) -> Result<ParsedFile, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut context = SessionContext::default();
    let mut token_event_index = 0_i64;
    let mut events = Vec::new();
    let mut context_actions = Vec::new();
    let mut recent_actions: Vec<ContextActionCandidate> = Vec::new();
    let mut tool_call_contexts: HashMap<String, ToolCallContext> = HashMap::new();
    let mut parse_errors = 0_i64;

    for line in reader.lines() {
        let line = line?;
        if !(line.contains("\"session_meta\"")
            || line.contains("\"turn_context\"")
            || line.contains("\"event_msg\"")
            || line.contains("\"response_item\""))
        {
            continue;
        }
        let record: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                parse_errors += 1;
                continue;
            }
        };
        let record_type = get_str(&record, "type");
        match record_type.as_deref() {
            Some("session_meta") => {
                let payload = &record["payload"];
                context.session_id =
                    get_str(payload, "session_id").or_else(|| get_str(payload, "id"));
                context.session_started_local = get_str(payload, "timestamp")
                    .and_then(|timestamp| parse_codex_time(&timestamp))
                    .map(format_local_time);
                context.cwd = get_str(payload, "cwd");
                context.originator = get_str(payload, "originator");
                context.source = get_str(payload, "source");
                context.cli_version = get_str(payload, "cli_version");
                context.model_provider = get_str(payload, "model_provider");
            }
            Some("turn_context") => {
                let payload = &record["payload"];
                if let Some(turn_id) = get_str(payload, "turn_id") {
                    context.current_turn_id = Some(turn_id);
                }
                if let Some(cwd) = get_str(payload, "cwd") {
                    context.cwd = Some(cwd);
                }
            }
            Some("event_msg") => {
                let payload = &record["payload"];
                match get_str(payload, "type").as_deref() {
                    Some("task_started") => {
                        context.current_turn_id = get_str(payload, "turn_id");
                        context.current_turn_started_local = payload
                            .get("started_at")
                            .and_then(parse_unix_seconds)
                            .map(format_local_time);
                        context.current_model_context_window =
                            payload.get("model_context_window").and_then(Value::as_i64);
                    }
                    Some("user_message") => {
                        context.current_user_message = get_str(payload, "message");
                    }
                    Some("token_count") => {
                        token_event_index += 1;
                        let Some(timestamp) = get_str(&record, "timestamp") else {
                            continue;
                        };
                        let Some(event_time_utc) = parse_codex_time(&timestamp) else {
                            continue;
                        };
                        let info = &payload["info"];
                        let last_usage = &info["last_token_usage"];
                        let total_usage = &info["total_token_usage"];
                        let rate_limits = &payload["rate_limits"];
                        let primary_limit = &rate_limits["primary"];
                        let secondary_limit = &rate_limits["secondary"];

                        let session_id = context
                            .session_id
                            .clone()
                            .unwrap_or_else(|| session_id_from_path(path));
                        let turn_id = get_str(payload, "turn_id")
                            .or_else(|| context.current_turn_id.clone())
                            .unwrap_or_else(|| "unknown-turn".to_string());
                        let turn_key = format!("{session_id}|{turn_id}");
                        let cwd = context.cwd.clone();
                        let project = project_from_cwd(cwd.as_deref());
                        let local_time = event_time_utc.with_timezone(&Local);
                        let user_message = context.current_user_message.clone();
                        let user_message_preview = user_message.as_deref().map(preview_text);
                        let event_key = format!(
                            "{}|{}|{}|{}|{}",
                            path.display(),
                            format_utc_compact(event_time_utc),
                            session_id,
                            turn_id,
                            token_event_index
                        );

                        let last_input_tokens = token_value(last_usage, "input_tokens");
                        let last_cached_input_tokens =
                            token_value(last_usage, "cached_input_tokens");
                        let last_output_tokens = token_value(last_usage, "output_tokens");
                        let last_reasoning_output_tokens =
                            token_value(last_usage, "reasoning_output_tokens");
                        let last_total_tokens = token_value(last_usage, "total_tokens");
                        let primary_used_percent =
                            primary_limit.get("used_percent").and_then(Value::as_f64);
                        let status = classify_status(
                            last_total_tokens,
                            last_output_tokens,
                            last_reasoning_output_tokens,
                            primary_used_percent,
                        );

                        events.push(ParsedTokenEvent {
                            event_key: event_key.clone(),
                            local_time: format_local_time(event_time_utc),
                            utc_time: format_utc_compact(event_time_utc),
                            date: format!(
                                "{:04}-{:02}-{:02}",
                                local_time.year(),
                                local_time.month(),
                                local_time.day()
                            ),
                            hour: local_time.hour(),
                            session_id: session_id.clone(),
                            turn_id: turn_id.clone(),
                            turn_key,
                            token_event_index,
                            file_path: path.display().to_string(),
                            project,
                            cwd,
                            originator: context.originator.clone(),
                            source: context.source.clone(),
                            cli_version: context.cli_version.clone(),
                            model_provider: context.model_provider.clone(),
                            session_started_local: context.session_started_local.clone(),
                            turn_started_local: context.current_turn_started_local.clone(),
                            user_message,
                            user_message_preview,
                            model_context_window: info
                                .get("model_context_window")
                                .and_then(Value::as_i64)
                                .or(context.current_model_context_window),
                            last_input_tokens,
                            last_cached_input_tokens,
                            last_output_tokens,
                            last_reasoning_output_tokens,
                            last_total_tokens,
                            session_input_tokens: token_value(total_usage, "input_tokens"),
                            session_cached_input_tokens: token_value(
                                total_usage,
                                "cached_input_tokens",
                            ),
                            session_output_tokens: token_value(total_usage, "output_tokens"),
                            session_reasoning_output_tokens: token_value(
                                total_usage,
                                "reasoning_output_tokens",
                            ),
                            session_total_tokens: token_value(total_usage, "total_tokens"),
                            rate_limit_plan_type: get_str(rate_limits, "plan_type"),
                            primary_used_percent,
                            primary_window_minutes: primary_limit
                                .get("window_minutes")
                                .and_then(Value::as_i64),
                            primary_resets_at_local: primary_limit
                                .get("resets_at")
                                .and_then(parse_unix_seconds)
                                .map(format_local_time),
                            secondary_used_percent: secondary_limit
                                .get("used_percent")
                                .and_then(Value::as_f64),
                            secondary_window_minutes: secondary_limit
                                .get("window_minutes")
                                .and_then(Value::as_i64),
                            secondary_resets_at_local: secondary_limit
                                .get("resets_at")
                                .and_then(parse_unix_seconds)
                                .map(format_local_time),
                            status,
                            event_time_utc,
                        });

                        if let Some(action) = select_preceding_action(
                            &recent_actions,
                            &session_id,
                            &turn_id,
                            event_time_utc,
                        ) {
                            context_actions.push(ParsedContextAction {
                                token_event_key: event_key,
                                local_time: format_local_time(action.event_time_utc),
                                utc_time: format_utc_compact(action.event_time_utc),
                                session_id: action.session_id.clone(),
                                turn_id: action.turn_id.clone(),
                                action_type: action.action_type.clone(),
                                tool_name: action.tool_name.clone(),
                                summary: action.summary.clone(),
                            });
                        }
                    }
                    Some("patch_apply_end")
                    | Some("mcp_tool_call_end")
                    | Some("web_search_end") => {
                        if let Some(action) = context_action_from_event_msg(&record, &context, path)
                        {
                            recent_actions.push(action);
                            prune_recent_actions(&mut recent_actions);
                        }
                    }
                    _ => {}
                }
            }
            Some("response_item") => {
                if let Some(action) = context_action_from_response_item(
                    &record,
                    &context,
                    path,
                    &mut tool_call_contexts,
                ) {
                    recent_actions.push(action);
                    prune_recent_actions(&mut recent_actions);
                }
            }
            _ => {}
        }
    }

    Ok(ParsedFile {
        events,
        context_actions,
        parse_errors,
    })
}

fn context_action_from_response_item(
    record: &Value,
    context: &SessionContext,
    path: &Path,
    tool_call_contexts: &mut HashMap<String, ToolCallContext>,
) -> Option<ContextActionCandidate> {
    let payload = &record["payload"];
    let item_type = get_str(payload, "type")?;
    let timestamp = get_str(record, "timestamp")?;
    let event_time_utc = parse_codex_time(&timestamp)?;
    let session_id = current_session_id(context, path);
    let turn_id = current_turn_id(context);

    match item_type.as_str() {
        "function_call" | "custom_tool_call" => {
            let tool_name = get_str(payload, "name").unwrap_or_else(|| "tool".to_string());
            let input = payload
                .get("arguments")
                .or_else(|| payload.get("input"))
                .map(value_to_summary_text)
                .unwrap_or_default();
            let summary = summarize_tool_call(&tool_name, &input);
            if let Some(call_id) = get_str(payload, "call_id") {
                tool_call_contexts.insert(
                    call_id,
                    ToolCallContext {
                        tool_name: tool_name.clone(),
                        summary: summary.clone(),
                    },
                );
            }
            Some(ContextActionCandidate {
                event_time_utc,
                session_id,
                turn_id,
                action_type: "工具调用".to_string(),
                tool_name,
                summary,
            })
        }
        "function_call_output" | "custom_tool_call_output" => {
            let call_context = get_str(payload, "call_id")
                .and_then(|call_id| tool_call_contexts.get(&call_id).cloned());
            let output_len = payload
                .get("output")
                .map(value_to_summary_text)
                .map(|output| output.chars().count())
                .unwrap_or(0);
            let tool_name = call_context
                .as_ref()
                .map(|context| context.tool_name.clone())
                .unwrap_or_else(|| "tool".to_string());
            let summary = match call_context {
                Some(context) if output_len > 0 => {
                    format!("{}，输出约 {} 字", context.summary, output_len)
                }
                Some(context) => context.summary,
                None if output_len > 0 => format!("工具输出，输出约 {output_len} 字"),
                None => "工具输出".to_string(),
            };
            Some(ContextActionCandidate {
                event_time_utc,
                session_id,
                turn_id,
                action_type: "工具输出".to_string(),
                tool_name,
                summary,
            })
        }
        "web_search_call" => {
            let query = payload
                .get("action")
                .and_then(|action| get_str(action, "query"))
                .or_else(|| get_str(payload, "query"))
                .unwrap_or_else(|| "网页搜索".to_string());
            Some(ContextActionCandidate {
                event_time_utc,
                session_id,
                turn_id,
                action_type: "网页搜索".to_string(),
                tool_name: "web_search".to_string(),
                summary: format!("网页搜索 {}", preview_inline(&query, 120)),
            })
        }
        _ => None,
    }
}

fn context_action_from_event_msg(
    record: &Value,
    context: &SessionContext,
    path: &Path,
) -> Option<ContextActionCandidate> {
    let payload = &record["payload"];
    let payload_type = get_str(payload, "type")?;
    let timestamp = get_str(record, "timestamp")?;
    let event_time_utc = parse_codex_time(&timestamp)?;
    let session_id = current_session_id(context, path);
    let turn_id = get_str(payload, "turn_id").unwrap_or_else(|| current_turn_id(context));

    match payload_type.as_str() {
        "patch_apply_end" => Some(ContextActionCandidate {
            event_time_utc,
            session_id,
            turn_id,
            action_type: "应用补丁".to_string(),
            tool_name: "apply_patch".to_string(),
            summary: "应用补丁完成".to_string(),
        }),
        "mcp_tool_call_end" => {
            let app_name = get_str(payload, "app_name").unwrap_or_else(|| "MCP".to_string());
            let action_name =
                get_str(payload, "action_name").unwrap_or_else(|| "工具调用".to_string());
            Some(ContextActionCandidate {
                event_time_utc,
                session_id,
                turn_id,
                action_type: "MCP工具".to_string(),
                tool_name: action_name.clone(),
                summary: format!("{app_name} {action_name}"),
            })
        }
        "web_search_end" => {
            let query = get_str(payload, "query").unwrap_or_else(|| "网页搜索".to_string());
            Some(ContextActionCandidate {
                event_time_utc,
                session_id,
                turn_id,
                action_type: "网页搜索".to_string(),
                tool_name: "web_search".to_string(),
                summary: format!("网页搜索 {}", preview_inline(&query, 120)),
            })
        }
        _ => None,
    }
}

fn select_preceding_action(
    actions: &[ContextActionCandidate],
    session_id: &str,
    turn_id: &str,
    token_time_utc: DateTime<Utc>,
) -> Option<ContextActionCandidate> {
    actions
        .iter()
        .rev()
        .find(|action| {
            action.session_id == session_id
                && action.turn_id == turn_id
                && action.event_time_utc <= token_time_utc
                && token_time_utc - action.event_time_utc
                    <= Duration::seconds(PRECEDING_ACTION_WINDOW_SECONDS)
        })
        .cloned()
}

fn prune_recent_actions(actions: &mut Vec<ContextActionCandidate>) {
    const MAX_RECENT_ACTIONS: usize = 80;
    if actions.len() > MAX_RECENT_ACTIONS {
        let drain_count = actions.len() - MAX_RECENT_ACTIONS;
        actions.drain(0..drain_count);
    }
}

fn current_session_id(context: &SessionContext, path: &Path) -> String {
    context
        .session_id
        .clone()
        .unwrap_or_else(|| session_id_from_path(path))
}

fn current_turn_id(context: &SessionContext) -> String {
    context
        .current_turn_id
        .clone()
        .unwrap_or_else(|| "unknown-turn".to_string())
}

fn value_to_summary_text(value: &Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn summarize_tool_call(tool_name: &str, input: &str) -> String {
    let normalized = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if tool_name == "shell_command" {
        let command = extract_json_string(&normalized, "command").unwrap_or(normalized);
        return summarize_shell_command(&command);
    }
    if tool_name == "apply_patch" {
        return "应用补丁".to_string();
    }
    if normalized.is_empty() {
        format!("调用 {tool_name}")
    } else {
        format!("调用 {tool_name} {}", preview_inline(&normalized, 120))
    }
}

fn summarize_shell_command(command: &str) -> String {
    let preview = preview_inline(command, 120);
    let lower = command.to_lowercase();
    if lower.contains("get-content") || lower.starts_with("type ") {
        format!("读取文件 {preview}")
    } else if lower.starts_with("rg ") || lower.contains(" rg ") {
        format!("搜索代码 {preview}")
    } else {
        format!("运行命令 {preview}")
    }
}

fn extract_json_string(value: &str, key: &str) -> Option<String> {
    serde_json::from_str::<Value>(value)
        .ok()
        .and_then(|json| get_str(&json, key))
}

fn preview_inline(value: &str, limit: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= limit {
        normalized
    } else {
        let mut preview = normalized.chars().take(limit).collect::<String>();
        preview.push('…');
        preview
    }
}

fn query_dashboard(
    conn: &Connection,
    paths: &AppPaths,
    filters: UsageFilters,
    config: AppConfigDto,
    scan_state: ScanStateDto,
) -> rusqlite::Result<DashboardData> {
    let all_events = load_events(conn)?;
    let display_names = load_display_name_maps(&scan_state.sessions_root);
    let filtered_events = apply_filters(&all_events, &filters, &display_names);
    let monthly_events = apply_filters(
        &all_events,
        &recent_month_filters(&filters, 6),
        &display_names,
    );
    let metrics = build_metrics(&filtered_events);

    Ok(DashboardData {
        generated_at: format_utc_iso(Utc::now()),
        data_dir: paths.data_dir.display().to_string(),
        database_path: paths.database_path.display().to_string(),
        sessions_root: scan_state.sessions_root.clone(),
        config,
        scan_state,
        metrics,
        detail_rows: build_detail_rows(&filtered_events, usize::MAX, &display_names),
        summary_rows: build_summary_rows(&filtered_events, usize::MAX),
        daily_buckets: build_daily_buckets(&filtered_events),
        monthly_buckets: build_recent_monthly_buckets(&monthly_events, &filters, 6),
        hourly_buckets: build_hourly_buckets(&filtered_events),
        composition: build_composition(&filtered_events),
        top_sessions: build_top_sessions(&filtered_events, 6, &display_names),
        top_projects: build_top_projects(&filtered_events, 6, &display_names),
        project_options: build_project_options(&all_events, &display_names),
        session_options: build_session_options(&all_events, &display_names),
    })
}

enum ExportKind {
    Detail,
    Summary,
}

fn export_usage_csv(
    kind: ExportKind,
    filters: UsageFilters,
    _include_messages: Option<bool>,
) -> Result<ExportResultDto, Box<dyn std::error::Error>> {
    let paths = AppPaths::new()?;
    let conn = open_database(&paths)?;
    init_database(&conn)?;
    let config = read_app_config(&conn)?;
    let all_events = load_events(&conn)?;
    let display_names = load_display_name_maps(&config.sessions_root);
    let filtered_events = apply_filters(&all_events, &filters, &display_names);
    let export_dir = paths.data_dir.join("exports");
    fs::create_dir_all(&export_dir)?;

    let file_prefix = match kind {
        ExportKind::Detail => "detail",
        ExportKind::Summary => "project-session-summary",
    };
    let output_path = export_dir.join(format!(
        "codex-token-usage-{file_prefix}-{}.csv",
        format_file_timestamp(Utc::now())
    ));
    let mut file = File::create(&output_path)?;
    file.write_all("\u{feff}".as_bytes())?;

    let row_count = match kind {
        ExportKind::Detail => write_detail_export(&mut file, &filtered_events, false)?,
        ExportKind::Summary => {
            let rows = build_summary_rows(&filtered_events, usize::MAX);
            write_summary_export(&mut file, &rows)?
        }
    };

    Ok(ExportResultDto {
        path: output_path.display().to_string(),
        row_count,
    })
}

fn write_detail_export(
    file: &mut File,
    events: &[TokenEvent],
    include_messages: bool,
) -> std::io::Result<usize> {
    let message_header = if include_messages {
        "用户输入"
    } else {
        "用户输入预览"
    };
    write_csv_row(
        file,
        &[
            "本地时间",
            "UTC时间",
            "日期",
            "项目",
            "会话ID",
            "Turn ID",
            "Token明细序号",
            "工作目录",
            message_header,
            "本次输入Token",
            "本次缓存输入Token",
            "本次非缓存输入Token",
            "本次输出Token",
            "本次推理输出Token",
            "本次总Token",
            "状态",
            "状态原因",
        ],
    )?;

    for event in events {
        let message = if include_messages {
            event.user_message.as_str()
        } else {
            event.user_message_preview.as_str()
        };
        write_csv_row(
            file,
            &[
                event.local_time.as_str(),
                event.utc_time.as_str(),
                event.date.as_str(),
                event.project.as_str(),
                event.session_id.as_str(),
                event.turn_id.as_str(),
                &event.token_event_index.to_string(),
                event.cwd.as_str(),
                message,
                &event.last_input_tokens.to_string(),
                &event.last_cached_input_tokens.to_string(),
                &non_cached_tokens(event.last_input_tokens, event.last_cached_input_tokens)
                    .to_string(),
                &event.last_output_tokens.to_string(),
                &event.last_reasoning_output_tokens.to_string(),
                &event.last_total_tokens.to_string(),
                event.status.as_str(),
                &classify_status_reason(
                    event.last_total_tokens,
                    event.last_output_tokens,
                    event.last_reasoning_output_tokens,
                    event.primary_used_percent,
                ),
            ],
        )?;
    }

    Ok(events.len())
}

fn write_summary_export(file: &mut File, rows: &[SummaryRowDto]) -> std::io::Result<usize> {
    write_csv_row(
        file,
        &[
            "层级",
            "名称",
            "项目",
            "会话ID",
            "会话数",
            "消息数",
            "输入Token",
            "缓存输入Token",
            "输出Token",
            "推理输出Token",
            "总Token",
            "状态",
            "状态原因",
        ],
    )?;

    for row in rows {
        write_csv_row(
            file,
            &[
                &row.level.to_string(),
                row.name.as_str(),
                row.project.as_str(),
                row.session_id.as_str(),
                &row.session_count.to_string(),
                &row.message_count.to_string(),
                &row.input_tokens.to_string(),
                &row.cached_input_tokens.to_string(),
                &row.output_tokens.to_string(),
                &row.reasoning_output_tokens.to_string(),
                &row.total_tokens.to_string(),
                row.status.as_str(),
                row.status_reason.as_str(),
            ],
        )?;
    }

    Ok(rows.len())
}

fn write_csv_row(file: &mut File, fields: &[&str]) -> std::io::Result<()> {
    let line = fields
        .iter()
        .map(|field| escape_csv_cell(field))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(file, "{line}")
}

fn escape_csv_cell(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn backup_database_file(paths: &AppPaths) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    if !paths.database_path.exists() {
        return Ok(None);
    }
    let backup_dir = paths.data_dir.join("backups");
    fs::create_dir_all(&backup_dir)?;
    let backup_path = backup_dir.join(format!(
        "codex-token-usage-before-rebuild-{}.sqlite",
        format_file_timestamp(Utc::now())
    ));
    fs::copy(&paths.database_path, &backup_path)?;
    Ok(Some(backup_path))
}

fn list_backup_files(paths: &AppPaths) -> Result<Vec<BackupFileDto>, Box<dyn std::error::Error>> {
    let backup_dir = paths.data_dir.join("backups");
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut rows = Vec::new();
    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let modified_utc: DateTime<Utc> = metadata.modified()?.into();
        let file_name = entry.file_name().to_string_lossy().to_string();
        rows.push(BackupFileDto {
            path: entry.path().display().to_string(),
            file_name,
            modified_local: format_local_time(modified_utc),
            size_bytes: metadata.len(),
        });
    }
    rows.sort_by(|a, b| b.modified_local.cmp(&a.modified_local));
    Ok(rows)
}

fn path_is_inside(base: &Path, candidate: &Path) -> bool {
    let Ok(base) = base.canonicalize() else {
        return false;
    };
    let Ok(candidate) = candidate.canonicalize() else {
        return false;
    };
    candidate.starts_with(base)
}

fn apply_retention() -> Result<RetentionResultDto, Box<dyn std::error::Error>> {
    let paths = AppPaths::new()?;
    let backup_path = backup_database_file(&paths)?;
    let conn = open_database(&paths)?;
    init_database(&conn)?;
    let config = read_app_config(&conn)?;
    if config.retention_days <= 0 {
        return Ok(RetentionResultDto {
            backup_path: backup_path.map(|path| path.display().to_string()),
            deleted_count: 0,
            cutoff_date: None,
        });
    }

    let cutoff = Local::now().date_naive() - Duration::days(config.retention_days);
    let cutoff_date = cutoff.format("%Y-%m-%d").to_string();
    let deleted_count = conn.execute(
        "DELETE FROM token_events WHERE date < ?1",
        params![cutoff_date],
    )?;
    conn.execute(
        r#"
        DELETE FROM token_context_actions
        WHERE token_event_key NOT IN (SELECT event_key FROM token_events)
        "#,
        [],
    )?;
    update_scan_ledger_count(&conn)?;

    Ok(RetentionResultDto {
        backup_path: backup_path.map(|path| path.display().to_string()),
        deleted_count,
        cutoff_date: Some(cutoff.format("%Y-%m-%d").to_string()),
    })
}

fn update_scan_ledger_count(conn: &Connection) -> rusqlite::Result<()> {
    let ledger_events: i64 =
        conn.query_row("SELECT COUNT(*) FROM token_events", [], |row| row.get(0))?;
    conn.execute(
        "UPDATE scan_state SET ledger_token_events = ?1 WHERE id = 1",
        params![ledger_events],
    )?;
    Ok(())
}

fn load_events(conn: &Connection) -> rusqlite::Result<Vec<TokenEvent>> {
    let mut statement = conn.prepare(
        r#"
        SELECT
            token_events.event_key, token_events.local_time, token_events.utc_time,
            token_events.date, token_events.hour, token_events.session_id,
            token_events.turn_id, token_events.token_event_index,
            token_events.project, COALESCE(token_events.cwd, ''),
            COALESCE(token_events.user_message, ''),
            COALESCE(token_events.user_message_preview, ''),
            token_events.last_input_tokens, token_events.last_cached_input_tokens,
            token_events.last_output_tokens, token_events.last_reasoning_output_tokens,
            token_events.last_total_tokens, token_events.primary_used_percent, token_events.status,
            token_context_actions.summary
        FROM token_events
        LEFT JOIN token_context_actions
            ON token_context_actions.token_event_key = token_events.event_key
        ORDER BY token_events.utc_time, token_events.session_id, token_events.token_event_index
        "#,
    )?;

    let rows = statement.query_map([], |row| {
        Ok(TokenEvent {
            _event_key: row.get(0)?,
            local_time: row.get(1)?,
            utc_time: row.get(2)?,
            date: row.get(3)?,
            hour: row.get::<_, i64>(4)? as u32,
            session_id: row.get(5)?,
            turn_id: row.get(6)?,
            token_event_index: row.get(7)?,
            project: row.get(8)?,
            cwd: row.get(9)?,
            user_message: row.get(10)?,
            user_message_preview: row.get(11)?,
            last_input_tokens: row.get(12)?,
            last_cached_input_tokens: row.get(13)?,
            last_output_tokens: row.get(14)?,
            last_reasoning_output_tokens: row.get(15)?,
            last_total_tokens: row.get(16)?,
            primary_used_percent: row.get(17)?,
            status: normalize_status_label(row.get(18)?),
            preceding_action: row.get(19)?,
        })
    })?;

    rows.collect()
}

fn apply_filters(
    events: &[TokenEvent],
    filters: &UsageFilters,
    display_names: &DisplayNameMaps,
) -> Vec<TokenEvent> {
    let search = filters
        .search
        .as_deref()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());

    events
        .iter()
        .filter(|event| {
            if let Some(date_from) = filters
                .date_from
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                if event.date.as_str() < date_from {
                    return false;
                }
            }
            if let Some(date_to) = filters.date_to.as_deref().filter(|value| !value.is_empty()) {
                if event.date.as_str() > date_to {
                    return false;
                }
            }
            if let Some(project) = filters.project.as_deref().filter(|value| !value.is_empty()) {
                if event.project != project {
                    return false;
                }
            }
            if let Some(session) = filters.session.as_deref().filter(|value| !value.is_empty()) {
                if event.session_id != session {
                    return false;
                }
            }
            if filters.only_anomalies.unwrap_or(false) && !is_super_high_status(&event.status) {
                return false;
            }
            if let Some(search) = search.as_deref() {
                let project_name = project_display_name(
                    &event.project,
                    std::slice::from_ref(event),
                    display_names,
                );
                let session_name = display_names
                    .session_titles
                    .get(&event.session_id)
                    .map(String::as_str)
                    .unwrap_or("");
                let haystack = format!(
                    "{} {} {} {} {} {} {} {} {} {}",
                    event.project,
                    project_name,
                    event.session_id,
                    session_name,
                    event.turn_id,
                    event.user_message,
                    event.user_message_preview,
                    event.cwd,
                    event.utc_time,
                    event.preceding_action.as_deref().unwrap_or("")
                )
                .to_lowercase();
                if !haystack.contains(search) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

fn build_metrics(events: &[TokenEvent]) -> MetricsDto {
    let sums = sum_events(events);
    let project_count = events
        .iter()
        .map(|event| event.project.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let session_count = events
        .iter()
        .map(|event| event.session_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let turn_count = events
        .iter()
        .map(|event| (event.session_id.as_str(), event.turn_id.as_str()))
        .collect::<BTreeSet<_>>()
        .len();
    let user_message_count = events
        .iter()
        .filter(|event| !event.user_message_preview.is_empty())
        .map(|event| (event.session_id.as_str(), event.turn_id.as_str()))
        .collect::<BTreeSet<_>>()
        .len();
    let abnormal_count = events
        .iter()
        .filter(|event| is_super_high_status(&event.status))
        .count();
    let dates = events
        .iter()
        .map(|event| event.date.as_str())
        .collect::<BTreeSet<_>>();
    let daily_average_tokens = if dates.is_empty() {
        0
    } else {
        sums.total_tokens / dates.len() as i64
    };
    let hourly_peak_tokens = build_hourly_buckets(events)
        .into_iter()
        .map(|bucket| bucket.total_tokens)
        .max()
        .unwrap_or(0);

    MetricsDto {
        total_tokens: sums.total_tokens,
        input_tokens: sums.input_tokens,
        cached_input_tokens: sums.cached_input_tokens,
        non_cached_input_tokens: non_cached_tokens(sums.input_tokens, sums.cached_input_tokens),
        output_tokens: sums.output_tokens,
        reasoning_output_tokens: sums.reasoning_output_tokens,
        token_event_count: events.len(),
        project_count,
        session_count,
        turn_count,
        user_message_count,
        abnormal_count,
        cache_rate: percent(sums.cached_input_tokens, sums.input_tokens),
        daily_average_tokens,
        hourly_peak_tokens,
    }
}

fn build_detail_rows(
    events: &[TokenEvent],
    limit: usize,
    display_names: &DisplayNameMaps,
) -> Vec<DetailRowDto> {
    let mut rows = Vec::new();
    let mut by_project: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
    for event in events {
        by_project
            .entry(event.project.clone())
            .or_default()
            .push(event.clone());
    }

    let mut project_groups: Vec<_> = by_project.into_iter().collect();
    project_groups.sort_by(
        |(left_project, left_events), (right_project, right_events)| {
            latest_utc(right_events)
                .cmp(latest_utc(left_events))
                .then_with(|| left_project.cmp(right_project))
        },
    );

    for (project, project_events) in project_groups {
        let project_key = detail_key("project", &[&project]);
        let project_node = project_display_name(&project, &project_events, display_names);
        let project_tooltip = project_tooltip(&project_node, &project, &project_events);
        push_detail_row(
            &mut rows,
            project_key.clone(),
            None,
            true,
            0,
            "Project",
            project_node,
            project_tooltip,
            &project_events,
            None,
        );
        if rows.len() >= limit {
            break;
        }

        let mut by_session: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
        for event in project_events.iter().cloned() {
            by_session
                .entry(event.session_id.clone())
                .or_default()
                .push(event);
        }

        let mut session_groups: Vec<_> = by_session.into_iter().collect();
        session_groups.sort_by(
            |(left_session, left_events), (right_session, right_events)| {
                latest_utc(right_events)
                    .cmp(latest_utc(left_events))
                    .then_with(|| left_session.cmp(right_session))
            },
        );

        for (session_id, session_events) in session_groups {
            let session_key = detail_key("session", &[&project, &session_id]);
            let session_node = display_names
                .session_titles
                .get(&session_id)
                .cloned()
                .unwrap_or_else(|| format!("Session {}", short_id(&session_id)));
            let session_tooltip = format!("{session_node}\n会话ID：{session_id}");
            push_detail_row(
                &mut rows,
                session_key.clone(),
                Some(project_key.clone()),
                true,
                1,
                "Session",
                session_node,
                session_tooltip,
                &session_events,
                None,
            );
            if rows.len() >= limit {
                break;
            }

            let mut by_turn: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
            for event in session_events.iter().cloned() {
                by_turn
                    .entry(event.turn_id.clone())
                    .or_default()
                    .push(event);
            }

            let mut turn_groups: Vec<_> = by_turn.into_iter().collect();
            turn_groups.sort_by(|(left_turn, left_events), (right_turn, right_events)| {
                latest_utc(right_events)
                    .cmp(latest_utc(left_events))
                    .then_with(|| left_turn.cmp(right_turn))
            });

            for (turn_id, turn_events) in turn_groups {
                let input_key = detail_key("input", &[&session_id, &turn_id]);
                let turn_key = detail_key("turn", &[&session_id, &turn_id]);
                let user_message = detail_user_message(&turn_events);
                let user_preview = preview_text(&user_message);
                push_detail_row(
                    &mut rows,
                    input_key.clone(),
                    Some(session_key.clone()),
                    true,
                    2,
                    "UserInput",
                    format!("用户输入  {user_preview}"),
                    format!("用户输入\n{user_message}"),
                    &turn_events,
                    None,
                );
                push_detail_row(
                    &mut rows,
                    turn_key.clone(),
                    Some(input_key),
                    true,
                    3,
                    "Turn",
                    format!("Turn {turn_id}"),
                    format!("Turn ID：{turn_id}"),
                    &turn_events,
                    None,
                );
                if rows.len() >= limit {
                    break;
                }

                let mut token_events = turn_events.clone();
                token_events.sort_by(|left, right| {
                    right
                        .utc_time
                        .cmp(&left.utc_time)
                        .then_with(|| right.token_event_index.cmp(&left.token_event_index))
                });
                for event in &token_events {
                    let token_index = event.token_event_index.to_string();
                    push_detail_row(
                        &mut rows,
                        detail_key("token", &[&session_id, &turn_id, &token_index]),
                        Some(turn_key.clone()),
                        false,
                        4,
                        "TokenCount",
                        format!("TokenCount #{}", event.token_event_index),
                        token_count_tooltip(event, &user_message),
                        std::slice::from_ref(event),
                        Some(event.token_event_index),
                    );
                    if rows.len() >= limit {
                        break;
                    }
                }
                if rows.len() >= limit {
                    break;
                }
            }
            if rows.len() >= limit {
                break;
            }
        }
    }

    rows
}

fn latest_utc(events: &[TokenEvent]) -> &str {
    events
        .iter()
        .map(|event| event.utc_time.as_str())
        .max()
        .unwrap_or("")
}

fn detail_user_message(events: &[TokenEvent]) -> String {
    events
        .iter()
        .find_map(|event| {
            let message = tooltip_text(&event.user_message);
            if !message.is_empty() {
                Some(message)
            } else if !event.user_message_preview.trim().is_empty() {
                Some(tooltip_text(&event.user_message_preview))
            } else {
                None
            }
        })
        .unwrap_or_else(|| "未记录用户输入".to_string())
}

fn token_count_tooltip(event: &TokenEvent, user_message: &str) -> String {
    let preceding_action = event
        .preceding_action
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("未识别到明确前置动作，可能来自用户输入或历史上下文");
    format!(
        "Token记录 #{}\n前置动作：{}\n完整用户输入：\n{}",
        event.token_event_index, preceding_action, user_message
    )
}

fn tooltip_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

fn push_detail_row(
    rows: &mut Vec<DetailRowDto>,
    row_key: String,
    parent_key: Option<String>,
    has_children: bool,
    level: u8,
    kind: &str,
    node: String,
    node_tooltip: String,
    events: &[TokenEvent],
    event_index: Option<i64>,
) {
    if events.is_empty() {
        return;
    }
    let sums = sum_events(events);
    let identity = &events[0];
    let first = events
        .iter()
        .min_by(|left, right| left.utc_time.cmp(&right.utc_time))
        .unwrap_or(identity);
    let last = events
        .iter()
        .max_by(|left, right| left.utc_time.cmp(&right.utc_time))
        .unwrap_or(identity);
    rows.push(DetailRowDto {
        row_key,
        parent_key,
        has_children,
        level,
        kind: kind.to_string(),
        node,
        node_tooltip,
        start_time: first.local_time.clone(),
        last_time: last.local_time.clone(),
        time: if first.utc_time == last.utc_time {
            first.local_time.clone()
        } else {
            format!("{} ~ {}", first.local_time, last.local_time)
        },
        project: identity.project.clone(),
        session_id: if level == 0 {
            "-".to_string()
        } else {
            identity.session_id.clone()
        },
        turn_id: if level <= 1 {
            "-".to_string()
        } else {
            identity.turn_id.clone()
        },
        event: event_index
            .map(|value| format!("#{value}"))
            .unwrap_or_else(|| "-".to_string()),
        input_tokens: sums.input_tokens,
        cached_input_tokens: sums.cached_input_tokens,
        non_cached_input_tokens: non_cached_tokens(sums.input_tokens, sums.cached_input_tokens),
        output_tokens: sums.output_tokens,
        reasoning_output_tokens: sums.reasoning_output_tokens,
        total_tokens: sums.total_tokens,
        status: aggregate_status(events),
        status_reason: aggregate_status_reason(events),
    });
}

fn build_summary_rows(events: &[TokenEvent], limit: usize) -> Vec<SummaryRowDto> {
    let mut rows = Vec::new();
    let mut by_project: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
    for event in events {
        by_project
            .entry(event.project.clone())
            .or_default()
            .push(event.clone());
    }

    for (project, project_events) in by_project {
        let project_key = summary_key("project", &[&project]);
        rows.push(summary_row(
            project_key.clone(),
            None,
            true,
            0,
            &project,
            &project,
            "",
            &project_events,
        ));
        if rows.len() >= limit {
            break;
        }

        let mut by_session: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
        for event in project_events {
            by_session
                .entry(event.session_id.clone())
                .or_default()
                .push(event);
        }
        let mut session_rows: Vec<_> = by_session
            .into_iter()
            .map(|(session, rows)| {
                summary_row(
                    summary_key("session", &[&project, &session]),
                    Some(project_key.clone()),
                    false,
                    1,
                    &format!("session {}", short_id(&session)),
                    &project,
                    &session,
                    &rows,
                )
            })
            .collect();
        session_rows.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
        for row in session_rows.into_iter().take(8) {
            rows.push(row);
            if rows.len() >= limit {
                break;
            }
        }
    }

    rows
}

fn summary_row(
    row_key: String,
    parent_key: Option<String>,
    has_children: bool,
    level: u8,
    name: &str,
    project: &str,
    session_id: &str,
    events: &[TokenEvent],
) -> SummaryRowDto {
    let sums = sum_events(events);
    let session_count = events
        .iter()
        .map(|event| event.session_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let message_count = events
        .iter()
        .map(|event| (event.session_id.as_str(), event.turn_id.as_str()))
        .collect::<BTreeSet<_>>()
        .len();

    SummaryRowDto {
        row_key,
        parent_key,
        has_children,
        level,
        name: name.to_string(),
        project: project.to_string(),
        session_id: session_id.to_string(),
        session_count,
        message_count,
        input_tokens: sums.input_tokens,
        cached_input_tokens: sums.cached_input_tokens,
        output_tokens: sums.output_tokens,
        reasoning_output_tokens: sums.reasoning_output_tokens,
        total_tokens: sums.total_tokens,
        status: aggregate_status(events),
        status_reason: aggregate_status_reason(events),
    }
}

fn build_hourly_buckets(events: &[TokenEvent]) -> Vec<HourlyBucketDto> {
    let mut buckets: BTreeMap<(String, u32), Vec<TokenEvent>> = BTreeMap::new();
    for event in events {
        buckets
            .entry((event.date.clone(), event.hour))
            .or_default()
            .push(event.clone());
    }

    buckets
        .into_iter()
        .map(|((date, hour), events)| {
            let total_tokens = events.iter().map(|event| event.last_total_tokens).sum();
            HourlyBucketDto {
                date,
                hour,
                total_tokens,
                status: aggregate_status(&events),
            }
        })
        .collect()
}

fn build_daily_buckets(events: &[TokenEvent]) -> Vec<TrendBucketDto> {
    build_trend_buckets(events, |event| event.date.clone())
}

fn build_recent_monthly_buckets(
    events: &[TokenEvent],
    filters: &UsageFilters,
    months: usize,
) -> Vec<TrendBucketDto> {
    let labels = recent_month_labels(filters, months);
    let mut buckets: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
    for event in events {
        buckets
            .entry(event.date.chars().take(7).collect())
            .or_default()
            .push(event.clone());
    }

    labels
        .into_iter()
        .map(|label| {
            let events = buckets.remove(&label).unwrap_or_default();
            TrendBucketDto {
                label,
                total_tokens: events.iter().map(|event| event.last_total_tokens).sum(),
                status: if events.is_empty() {
                    "正常".to_string()
                } else {
                    aggregate_status(&events)
                },
            }
        })
        .collect()
}

fn recent_month_filters(filters: &UsageFilters, months: usize) -> UsageFilters {
    let labels = recent_month_labels(filters, months);
    let mut next = filters.clone();
    if let Some(first_label) = labels.first() {
        next.date_from = Some(format!("{first_label}-01"));
    }
    next.date_to = Some(month_window_end(filters));
    next
}

fn recent_month_labels(filters: &UsageFilters, months: usize) -> Vec<String> {
    let end_date = filters
        .date_to
        .as_deref()
        .and_then(parse_filter_date)
        .unwrap_or_else(|| Local::now().date_naive());
    let end_index = end_date.year() * 12 + end_date.month() as i32 - 1;
    let start_index = end_index - months.saturating_sub(1) as i32;
    (0..months)
        .map(|offset| month_label_from_index(start_index + offset as i32))
        .collect()
}

fn month_window_end(filters: &UsageFilters) -> String {
    filters
        .date_to
        .as_deref()
        .and_then(parse_filter_date)
        .map(|date| date.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| Local::now().date_naive().format("%Y-%m-%d").to_string())
}

fn parse_filter_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn month_label_from_index(index: i32) -> String {
    let year = index.div_euclid(12);
    let month = index.rem_euclid(12) + 1;
    format!("{year}-{month:02}")
}

fn build_trend_buckets(
    events: &[TokenEvent],
    label_for: impl Fn(&TokenEvent) -> String,
) -> Vec<TrendBucketDto> {
    let mut buckets: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
    for event in events {
        buckets
            .entry(label_for(event))
            .or_default()
            .push(event.clone());
    }

    buckets
        .into_iter()
        .map(|(label, events)| TrendBucketDto {
            label,
            total_tokens: events.iter().map(|event| event.last_total_tokens).sum(),
            status: aggregate_status(&events),
        })
        .collect()
}

fn build_composition(events: &[TokenEvent]) -> Vec<CompositionDto> {
    let sums = sum_events(events);
    let total = sums.total_tokens.max(1);
    vec![
        composition_item("输入", sums.input_tokens, total, "blue"),
        composition_item("缓存输入", sums.cached_input_tokens, total, "green"),
        composition_item(
            "非缓存输入",
            non_cached_tokens(sums.input_tokens, sums.cached_input_tokens),
            total,
            "orange",
        ),
        composition_item("输出", sums.output_tokens, total, "purple"),
        composition_item("推理输出", sums.reasoning_output_tokens, total, "red"),
    ]
}

fn composition_item(name: &str, value: i64, total: i64, tone: &str) -> CompositionDto {
    CompositionDto {
        name: name.to_string(),
        total_tokens: value,
        ratio: percent(value, total),
        tone: tone.to_string(),
    }
}

fn build_top_sessions(
    events: &[TokenEvent],
    limit: usize,
    display_names: &DisplayNameMaps,
) -> Vec<TopSessionDto> {
    let mut by_session: HashMap<String, Vec<&TokenEvent>> = HashMap::new();
    for event in events {
        by_session
            .entry(event.session_id.clone())
            .or_default()
            .push(event);
    }

    let mut rows: Vec<_> = by_session
        .into_iter()
        .map(|(session_id, events)| {
            let total_tokens = events.iter().map(|event| event.last_total_tokens).sum();
            let project = events
                .first()
                .map(|event| event.project.clone())
                .unwrap_or_default();
            let owned_events = events
                .iter()
                .map(|event| (*event).to_owned())
                .collect::<Vec<_>>();
            let project_name = project_display_name(&project, &owned_events, display_names);
            let session_name = display_names
                .session_titles
                .get(&session_id)
                .cloned()
                .unwrap_or_else(|| format!("Session {}", short_id(&session_id)));
            TopSessionDto {
                session_id,
                session_name,
                project,
                project_name,
                total_tokens,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    rows.truncate(limit);
    rows
}

fn build_top_projects(
    events: &[TokenEvent],
    limit: usize,
    display_names: &DisplayNameMaps,
) -> Vec<TopProjectDto> {
    let mut by_project: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
    for event in events {
        by_project
            .entry(event.project.clone())
            .or_default()
            .push(event.clone());
    }

    let mut rows: Vec<_> = by_project
        .into_iter()
        .map(|(project, events)| TopProjectDto {
            project_name: project_display_name(&project, &events, display_names),
            project,
            total_tokens: events.iter().map(|event| event.last_total_tokens).sum(),
        })
        .collect();
    rows.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    rows.truncate(limit);
    rows
}

fn build_project_options(
    events: &[TokenEvent],
    display_names: &DisplayNameMaps,
) -> Vec<FilterOptionDto> {
    let mut by_project: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
    for event in events {
        by_project
            .entry(event.project.clone())
            .or_default()
            .push(event.clone());
    }

    let mut options: Vec<_> = by_project
        .into_iter()
        .map(|(project, events)| {
            let label = project_display_name(&project, &events, display_names);
            let title = if label == project {
                project.clone()
            } else {
                format!("{label}\n项目：{project}")
            };
            FilterOptionDto {
                value: project,
                label,
                title,
                project: None,
            }
        })
        .collect();
    options.sort_by(|left, right| {
        left.label
            .cmp(&right.label)
            .then(left.value.cmp(&right.value))
    });
    options
}

fn build_session_options(
    events: &[TokenEvent],
    display_names: &DisplayNameMaps,
) -> Vec<FilterOptionDto> {
    let mut by_session: BTreeMap<String, Vec<TokenEvent>> = BTreeMap::new();
    for event in events {
        by_session
            .entry(event.session_id.clone())
            .or_default()
            .push(event.clone());
    }
    let mut options: Vec<_> = by_session
        .into_iter()
        .map(|(session_id, events)| {
            let project = events
                .first()
                .map(|event| event.project.clone())
                .unwrap_or_default();
            let project_name = project_display_name(&project, &events, display_names);
            let label = display_names
                .session_titles
                .get(&session_id)
                .cloned()
                .unwrap_or_else(|| format!("Session {}", short_id(&session_id)));
            let title = format!("{label}\n项目：{project_name}\nSession ID：{session_id}");
            FilterOptionDto {
                value: session_id,
                label,
                title,
                project: Some(project),
            }
        })
        .collect();
    options.sort_by(|left, right| {
        left.label
            .cmp(&right.label)
            .then(left.value.cmp(&right.value))
    });
    options
}

fn sum_events(events: &[TokenEvent]) -> TokenSums {
    events.iter().fold(TokenSums::default(), |mut sums, event| {
        sums.input_tokens += event.last_input_tokens;
        sums.cached_input_tokens += event.last_cached_input_tokens;
        sums.output_tokens += event.last_output_tokens;
        sums.reasoning_output_tokens += event.last_reasoning_output_tokens;
        sums.total_tokens += event.last_total_tokens;
        sums
    })
}

fn aggregate_status(events: &[TokenEvent]) -> String {
    if events
        .iter()
        .any(|event| is_super_high_status(&event.status))
    {
        "超高".to_string()
    } else if events.iter().any(|event| event.status == "偏高") {
        "偏高".to_string()
    } else {
        "正常".to_string()
    }
}

fn aggregate_status_reason(events: &[TokenEvent]) -> String {
    let mut reasons = BTreeSet::new();
    for event in events {
        let reason = classify_status_reason(
            event.last_total_tokens,
            event.last_output_tokens,
            event.last_reasoning_output_tokens,
            event.primary_used_percent,
        );
        if reason != "正常范围内" {
            reasons.insert(reason);
        }
    }
    if reasons.is_empty() {
        "正常范围内".to_string()
    } else {
        reasons.into_iter().collect::<Vec<_>>().join("；")
    }
}

fn read_last_cutoff(conn: &Connection) -> rusqlite::Result<Option<DateTime<Utc>>> {
    let value: Option<String> = conn
        .query_row(
            "SELECT last_cutoff_utc FROM scan_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()?
        .flatten();

    Ok(value.and_then(|text| parse_codex_time(&text)))
}

fn read_scan_state(
    conn: &Connection,
    sessions_root: &Path,
    include_archived: bool,
) -> rusqlite::Result<ScanStateDto> {
    let state = conn
        .query_row(
            r#"
            SELECT last_cutoff_utc, last_run_utc, sessions_root, ledger_token_events,
                last_run_new_token_events, last_run_files_scanned, last_run_parse_errors,
                include_archived
            FROM scan_state WHERE id = 1
            "#,
            [],
            |row| {
                Ok(ScanStateDto {
                    last_cutoff_utc: row.get(0)?,
                    last_run_utc: row.get(1)?,
                    sessions_root: row.get(2)?,
                    ledger_token_events: row.get(3)?,
                    last_run_new_token_events: row.get(4)?,
                    last_run_files_scanned: row.get(5)?,
                    last_run_parse_errors: row.get(6)?,
                    include_archived: row.get::<_, i64>(7)? != 0,
                    error: None,
                })
            },
        )
        .optional()?;

    Ok(state.unwrap_or_else(|| ScanStateDto {
        last_cutoff_utc: None,
        last_run_utc: None,
        sessions_root: sessions_root.display().to_string(),
        ledger_token_events: 0,
        last_run_new_token_events: 0,
        last_run_files_scanned: 0,
        last_run_parse_errors: 0,
        include_archived,
        error: None,
    }))
}

fn get_str(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn token_value(usage: &Value, key: &str) -> i64 {
    usage.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn parse_unix_seconds(value: &Value) -> Option<DateTime<Utc>> {
    let seconds = value.as_i64()?;
    Utc.timestamp_opt(seconds, 0).single()
}

fn parse_codex_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.with_timezone(&Utc))
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|datetime| Utc.from_utc_datetime(&datetime))
        })
}

fn format_local_time(value: DateTime<Utc>) -> String {
    value
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S %:z")
        .to_string()
}

fn format_utc_compact(value: DateTime<Utc>) -> String {
    value.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn format_utc_iso(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn format_file_timestamp(value: DateTime<Utc>) -> String {
    value.format("%Y%m%d-%H%M%S").to_string()
}

fn project_from_cwd(cwd: Option<&str>) -> String {
    cwd.and_then(|value| Path::new(value).file_name())
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn project_display_name(
    project: &str,
    events: &[TokenEvent],
    display_names: &DisplayNameMaps,
) -> String {
    events
        .iter()
        .find_map(|event| {
            let normalized = normalize_codex_path(&event.cwd);
            display_names.workspace_labels.get(&normalized).cloned()
        })
        .unwrap_or_else(|| project.to_string())
}

fn project_tooltip(display_name: &str, project: &str, events: &[TokenEvent]) -> String {
    let cwd = events
        .iter()
        .map(|event| event.cwd.as_str())
        .find(|cwd| !cwd.is_empty());
    match cwd {
        Some(cwd) if display_name != project => {
            format!("{display_name}\n项目：{project}\n工作目录：{cwd}")
        }
        Some(cwd) => format!("{display_name}\n工作目录：{cwd}"),
        None => display_name.to_string(),
    }
}

fn session_id_from_path(path: &Path) -> String {
    path.file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown-session".to_string())
}

fn preview_text(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    const LIMIT: usize = 120;
    if normalized.chars().count() <= LIMIT {
        normalized
    } else {
        let mut preview = normalized.chars().take(LIMIT).collect::<String>();
        preview.push('…');
        preview
    }
}

fn classify_status(
    total: i64,
    output: i64,
    reasoning: i64,
    primary_used_percent: Option<f64>,
) -> String {
    let reason = classify_status_reason(total, output, reasoning, primary_used_percent);
    if reason.starts_with("超高") {
        "超高".to_string()
    } else if reason.starts_with("偏高") {
        "偏高".to_string()
    } else {
        "正常".to_string()
    }
}

fn classify_status_reason(
    total: i64,
    output: i64,
    reasoning: i64,
    primary_used_percent: Option<f64>,
) -> String {
    if total >= ABNORMAL_TOKEN_THRESHOLD {
        return format!("超高：单条 token_count 达到 {ABNORMAL_TOKEN_THRESHOLD}");
    }
    if primary_used_percent.unwrap_or(0.0) >= 95.0 {
        return "超高：主额度窗口使用率达到 95%".to_string();
    }
    if total >= HIGH_TOKEN_THRESHOLD {
        return format!("偏高：单条 token_count 达到 {HIGH_TOKEN_THRESHOLD}");
    }
    if output > 0 && reasoning > output * 3 {
        return "偏高：推理输出超过普通输出 3 倍".to_string();
    }
    "正常范围内".to_string()
}

fn is_super_high_status(status: &str) -> bool {
    matches!(status, "超高" | "异常")
}

fn normalize_status_label(status: String) -> String {
    if status == "异常" {
        "超高".to_string()
    } else {
        status
    }
}

fn non_cached_tokens(input: i64, cached: i64) -> i64 {
    input.saturating_sub(cached)
}

fn percent(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        ((value as f64 / total as f64) * 1000.0).round() / 10.0
    }
}

fn short_id(value: &str) -> String {
    value.chars().take(8).collect()
}

fn detail_key(kind: &str, parts: &[&str]) -> String {
    tree_key("detail", kind, parts)
}

fn summary_key(kind: &str, parts: &[&str]) -> String {
    tree_key("summary", kind, parts)
}

fn tree_key(scope: &str, kind: &str, parts: &[&str]) -> String {
    let mut key = format!("{scope}:{kind}");
    for part in parts {
        key.push(':');
        key.push_str(&part.replace(':', "_"));
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scan_is_incremental_and_deduped() {
        let temp = tempdir().unwrap();
        let sessions_root = temp.path().join("sessions");
        fs::create_dir_all(&sessions_root).unwrap();
        let file_path = sessions_root.join("sample-session.jsonl");
        fs::write(&file_path, sample_jsonl()).unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();

        let first = scan_sessions(&mut conn, &sessions_root, false).unwrap();
        assert_eq!(first.last_run_new_token_events, 2);
        assert_eq!(first.ledger_token_events, 2);

        let second = scan_sessions(&mut conn, &sessions_root, false).unwrap();
        assert_eq!(second.last_run_new_token_events, 0);
        assert_eq!(second.ledger_token_events, 2);
    }

    #[test]
    fn dashboard_keeps_project_session_turn_and_token_sums() {
        let temp = tempdir().unwrap();
        let sessions_root = temp.path().join("sessions");
        fs::create_dir_all(&sessions_root).unwrap();
        fs::write(sessions_root.join("sample-session.jsonl"), sample_jsonl()).unwrap();

        let paths = AppPaths {
            data_dir: temp.path().join("data"),
            database_path: temp.path().join("usage.sqlite"),
        };
        fs::create_dir_all(&paths.data_dir).unwrap();
        let mut conn = Connection::open(&paths.database_path).unwrap();
        init_database(&conn).unwrap();
        let state = scan_sessions(&mut conn, &sessions_root, false).unwrap();
        let config = default_app_config();
        let dashboard =
            query_dashboard(&conn, &paths, UsageFilters::default(), config, state).unwrap();

        assert_eq!(dashboard.metrics.total_tokens, 600);
        assert_eq!(dashboard.metrics.cached_input_tokens, 300);
        assert_eq!(dashboard.metrics.non_cached_input_tokens, 180);
        assert_eq!(dashboard.metrics.project_count, 1);
        assert_eq!(dashboard.metrics.session_count, 1);
        assert_eq!(dashboard.metrics.turn_count, 1);
        assert!(dashboard
            .detail_rows
            .iter()
            .any(|row| row.kind == "UserInput" && row.node.contains("统计 token")));
        assert!(dashboard.detail_rows.iter().any(|row| {
            row.kind == "UserInput" && row.node_tooltip.contains("用户输入\n帮我统计 token 消耗")
        }));
        assert!(dashboard.detail_rows.iter().any(|row| {
            row.kind == "TokenCount"
                && row.node_tooltip.contains("前置动作：读取文件")
                && row.node_tooltip.contains("完整用户输入：")
                && row.node_tooltip.contains("帮我统计 token 消耗")
                && !row.node_tooltip.contains("归属用户输入")
                && !row.node_tooltip.contains("Turn ID")
                && !row.node_tooltip.contains("Token事件时间")
                && !row.node_tooltip.contains("Token构成")
                && !row.node_tooltip.contains("距离")
                && !row.node_tooltip.contains("置信度")
        }));
        assert!(dashboard
            .detail_rows
            .iter()
            .any(|row| row.level == 0 && row.parent_key.is_none() && row.has_children));
        assert!(dashboard
            .detail_rows
            .iter()
            .any(|row| row.level == 4 && row.parent_key.is_some() && !row.has_children));
        assert!(dashboard
            .summary_rows
            .iter()
            .any(|row| row.level == 0 && row.parent_key.is_none() && row.has_children));
        assert!(dashboard
            .session_options
            .iter()
            .any(|option| option.value == "sample-session"
                && option.project.as_deref() == Some("codex-token-usage")));
    }

    #[test]
    fn parser_falls_back_when_session_meta_and_turn_id_are_missing() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("fallback-session.jsonl");
        fs::write(
            &file_path,
            r#"{"type":"event_msg","timestamp":"2026-07-10T10:00:03Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":1,"total_tokens":14},"total_token_usage":{"input_tokens":10,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":1,"total_tokens":14}},"rate_limits":{"primary":{"used_percent":1},"secondary":{}}}}"#,
        )
        .unwrap();

        let parsed = parse_codex_jsonl(&file_path).unwrap();

        assert_eq!(parsed.parse_errors, 0);
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].session_id, "fallback-session");
        assert_eq!(parsed.events[0].turn_id, "unknown-turn");
        assert_eq!(parsed.events[0].project, "unknown");
    }

    #[test]
    fn parser_keeps_multiple_token_counts_in_one_turn() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("sample-session.jsonl");
        fs::write(&file_path, sample_jsonl()).unwrap();

        let parsed = parse_codex_jsonl(&file_path).unwrap();

        assert_eq!(parsed.events.len(), 2);
        assert_eq!(parsed.context_actions.len(), 2);
        assert!(parsed.context_actions[0].summary.contains("读取文件"));
        assert_eq!(parsed.events[0].turn_id, "turn-1");
        assert_eq!(parsed.events[1].turn_id, "turn-1");
        assert_eq!(parsed.events[0].token_event_index, 1);
        assert_eq!(parsed.events[1].token_event_index, 2);
    }

    #[test]
    fn scanner_counts_parse_errors_without_dropping_valid_events() {
        let temp = tempdir().unwrap();
        let sessions_root = temp.path().join("sessions");
        fs::create_dir_all(&sessions_root).unwrap();
        let file_path = sessions_root.join("sample-session.jsonl");
        fs::write(
            &file_path,
            format!("{{\"type\":\"event_msg\",\n{}\n", sample_jsonl()),
        )
        .unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();

        let state = scan_sessions(&mut conn, &sessions_root, false).unwrap();

        assert_eq!(state.last_run_parse_errors, 1);
        assert_eq!(state.ledger_token_events, 2);
    }

    #[test]
    fn scanner_can_include_archived_sessions() {
        let temp = tempdir().unwrap();
        let codex_home = temp.path().join(".codex");
        let sessions_root = codex_home.join("sessions");
        let archived_root = codex_home.join("archived_sessions");
        fs::create_dir_all(&sessions_root).unwrap();
        fs::create_dir_all(&archived_root).unwrap();
        fs::write(sessions_root.join("active.jsonl"), sample_jsonl()).unwrap();
        fs::write(archived_root.join("archived.jsonl"), sample_jsonl()).unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();

        let state = scan_sessions(&mut conn, &sessions_root, true).unwrap();

        assert!(state.include_archived);
        assert_eq!(state.last_run_files_scanned, 2);
        assert_eq!(state.ledger_token_events, 4);
    }

    #[test]
    fn settings_are_saved_and_loaded() {
        let conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();
        assert!(read_app_config(&conn).unwrap().include_archived);

        let saved = write_app_config(
            &conn,
            AppConfigInput {
                sessions_root: Some("D:\\Codex\\sessions".to_string()),
                include_archived: Some(false),
                refresh_interval_seconds: Some(5),
                include_messages_in_export: Some(true),
                retention_days: Some(90),
                update_source: Some("owner/repo".to_string()),
            },
        )
        .unwrap();
        let loaded = read_app_config(&conn).unwrap();

        assert_eq!(saved.sessions_root, "D:\\Codex\\sessions");
        assert!(saved.include_archived);
        assert!(loaded.include_archived);
        assert_eq!(loaded.refresh_interval_seconds, 30);
        assert!(loaded.include_messages_in_export);
        assert_eq!(loaded.retention_days, 90);
        assert_eq!(loaded.update_source, DEFAULT_UPDATE_SOURCE);
    }

    #[test]
    fn update_source_accepts_github_repo_forms() {
        assert_eq!(
            normalize_update_source("owner/repo").unwrap(),
            "https://api.github.com/repos/owner/repo/releases/latest"
        );
        assert_eq!(
            normalize_update_source("https://github.com/owner/repo").unwrap(),
            "https://api.github.com/repos/owner/repo/releases/latest"
        );
    }

    #[test]
    fn update_version_compare_handles_semver_tags() {
        assert!(is_newer_version("v0.3.2", "0.3.1"));
        assert!(!is_newer_version("0.3.1", "0.3.1"));
        assert!(!is_newer_version("0.3.0", "0.3.1"));
    }

    #[test]
    fn update_asset_selection_prefers_portable_exe() {
        let release = serde_json::json!({
            "assets": [
                {
                    "name": "CodexTokenUsage-0.3.1-windows-x64-installer.exe",
                    "browser_download_url": "https://example.test/installer.exe",
                    "size": 20
                },
                {
                    "name": "CodexTokenUsage-0.3.1-windows-x64-portable.exe",
                    "browser_download_url": "https://example.test/portable.exe",
                    "size": 10
                }
            ]
        });

        let selected = select_release_asset(&release).unwrap();
        assert_eq!(selected.1, "https://example.test/portable.exe");
    }

    #[test]
    fn display_name_maps_read_codex_sidebar_titles() {
        let temp = tempdir().unwrap();
        let codex_home = temp.path().join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("session_index.jsonl"),
            r#"{"id":"session-a","thread_name":"旧标题","updated_at":"2026-07-10T10:00:00Z"}
{"id":"session-a","thread_name":"[开发]token统计0.0.1","updated_at":"2026-07-10T10:01:00Z"}"#,
        )
        .unwrap();
        fs::write(
            codex_home.join(".codex-global-state.json"),
            r#"{"electron-workspace-root-labels":{"\\\\?\\D:\\Desktop\\AI\\codex_about":"0-codex_about"}}"#,
        )
        .unwrap();

        let titles = read_session_titles(&codex_home);
        let labels = read_workspace_labels(&codex_home);

        assert_eq!(titles.get("session-a").unwrap(), "[开发]token统计0.0.1");
        assert_eq!(
            labels
                .get(&normalize_codex_path(r"\\?\D:\Desktop\AI\codex_about"))
                .unwrap(),
            "0-codex_about"
        );

        let display_names = DisplayNameMaps {
            session_titles: titles,
            workspace_labels: labels,
        };
        let events = vec![TokenEvent {
            _event_key: "event-1".to_string(),
            local_time: "2026-07-10 18:00:00 +08:00".to_string(),
            utc_time: "2026-07-10 10:00:00".to_string(),
            date: "2026-07-10".to_string(),
            hour: 18,
            session_id: "session-a".to_string(),
            turn_id: "turn-a".to_string(),
            token_event_index: 1,
            project: "codex_about".to_string(),
            cwd: r"\\?\D:\Desktop\AI\codex_about".to_string(),
            user_message: String::new(),
            user_message_preview: String::new(),
            last_input_tokens: 1,
            last_cached_input_tokens: 0,
            last_output_tokens: 0,
            last_reasoning_output_tokens: 0,
            last_total_tokens: 1,
            primary_used_percent: None,
            status: "正常".to_string(),
            preceding_action: None,
        }];
        let project_options = build_project_options(&events, &display_names);
        let session_options = build_session_options(&events, &display_names);

        assert_eq!(project_options[0].value, "codex_about");
        assert_eq!(project_options[0].label, "0-codex_about");
        assert_eq!(session_options[0].value, "session-a");
        assert_eq!(session_options[0].label, "[开发]token统计0.0.1");
    }

    #[test]
    fn context_actions_backfill_existing_token_events() {
        let temp = tempdir().unwrap();
        let sessions_root = temp.path().join("sessions");
        fs::create_dir_all(&sessions_root).unwrap();
        let file_path = sessions_root.join("sample-session.jsonl");
        fs::write(&file_path, sample_jsonl()).unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();
        let parsed = parse_codex_jsonl(&file_path).unwrap();
        for event in &parsed.events {
            insert_token_event(&conn, event).unwrap();
        }

        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM token_context_actions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(before, 0);

        ensure_context_actions_backfilled(&mut conn, &sessions_root, false).unwrap();
        let after: i64 = conn
            .query_row("SELECT COUNT(*) FROM token_context_actions", [], |row| {
                row.get(0)
            })
            .unwrap();
        let events = load_events(&conn).unwrap();

        assert_eq!(after, 2);
        assert!(events.iter().any(|event| event
            .preceding_action
            .as_deref()
            .unwrap_or("")
            .contains("读取文件")));
    }

    #[test]
    fn detail_export_can_include_message_text() {
        let temp = tempdir().unwrap();
        let sessions_root = temp.path().join("sessions");
        fs::create_dir_all(&sessions_root).unwrap();
        fs::write(sessions_root.join("sample-session.jsonl"), sample_jsonl()).unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        init_database(&conn).unwrap();
        scan_sessions(&mut conn, &sessions_root, false).unwrap();
        let events = load_events(&conn).unwrap();
        let output_path = temp.path().join("detail.csv");
        let mut file = File::create(&output_path).unwrap();

        let rows = write_detail_export(&mut file, &events, true).unwrap();
        drop(file);
        let csv_text = fs::read_to_string(output_path).unwrap();

        assert_eq!(rows, 2);
        assert!(csv_text.contains("帮我统计 token 消耗"));
    }

    #[test]
    fn backup_database_file_copies_existing_ledger() {
        let temp = tempdir().unwrap();
        let paths = AppPaths {
            data_dir: temp.path().join("data"),
            database_path: temp.path().join("data").join("usage.sqlite"),
        };
        fs::create_dir_all(&paths.data_dir).unwrap();
        fs::write(&paths.database_path, "ledger").unwrap();

        let backup = backup_database_file(&paths).unwrap().unwrap();

        assert!(backup.exists());
        assert_eq!(fs::read_to_string(backup).unwrap(), "ledger");
    }

    #[test]
    fn dashboard_keeps_cross_date_hour_buckets() {
        let temp = tempdir().unwrap();
        let sessions_root = temp.path().join("sessions");
        fs::create_dir_all(&sessions_root).unwrap();
        fs::write(sessions_root.join("cross-date.jsonl"), cross_date_jsonl()).unwrap();

        let paths = AppPaths {
            data_dir: temp.path().join("data"),
            database_path: temp.path().join("usage.sqlite"),
        };
        fs::create_dir_all(&paths.data_dir).unwrap();
        let mut conn = Connection::open(&paths.database_path).unwrap();
        init_database(&conn).unwrap();
        let state = scan_sessions(&mut conn, &sessions_root, false).unwrap();
        let config = default_app_config();
        let dashboard =
            query_dashboard(&conn, &paths, UsageFilters::default(), config, state).unwrap();
        let dates = dashboard
            .hourly_buckets
            .iter()
            .map(|bucket| bucket.date.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(dashboard.metrics.token_event_count, 2);
        assert_eq!(dates.len(), 2);
    }

    fn sample_jsonl() -> String {
        r#"{"type":"session_meta","payload":{"session_id":"sample-session","timestamp":"2026-07-10T10:00:00Z","cwd":"D:\\Desktop\\AI\\codex_about\\codex-token-usage","originator":"codex","cli_version":"1.0.0","source":"test","model_provider":"openai"}}
{"type":"event_msg","timestamp":"2026-07-10T10:00:01Z","payload":{"type":"task_started","turn_id":"turn-1","started_at":1783677601,"model_context_window":200000}}
{"type":"event_msg","timestamp":"2026-07-10T10:00:02Z","payload":{"type":"user_message","message":"帮我统计 token 消耗"}}
{"type":"response_item","timestamp":"2026-07-10T10:00:02.500Z","payload":{"type":"function_call","name":"shell_command","call_id":"call-read","arguments":"{\"command\":\"Get-Content app/src/App.svelte\"}"}}
{"type":"response_item","timestamp":"2026-07-10T10:00:02.800Z","payload":{"type":"function_call_output","call_id":"call-read","output":"line one\nline two"}}
{"type":"event_msg","timestamp":"2026-07-10T10:00:03Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":120,"cached_input_tokens":80,"output_tokens":30,"reasoning_output_tokens":10,"total_tokens":160},"total_token_usage":{"input_tokens":120,"cached_input_tokens":80,"output_tokens":30,"reasoning_output_tokens":10,"total_tokens":160},"model_context_window":200000},"rate_limits":{"plan_type":"pro","primary":{"used_percent":12.5,"window_minutes":300,"resets_at":1783679600},"secondary":{"used_percent":1.5,"window_minutes":10080,"resets_at":1783689600}}}}
{"type":"event_msg","timestamp":"2026-07-10T10:00:04Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":360,"cached_input_tokens":220,"output_tokens":70,"reasoning_output_tokens":10,"total_tokens":440},"total_token_usage":{"input_tokens":480,"cached_input_tokens":300,"output_tokens":100,"reasoning_output_tokens":20,"total_tokens":600},"model_context_window":200000},"rate_limits":{"plan_type":"pro","primary":{"used_percent":12.8,"window_minutes":300,"resets_at":1783679600},"secondary":{"used_percent":1.8,"window_minutes":10080,"resets_at":1783689600}}}}"#
            .to_string()
    }

    fn cross_date_jsonl() -> String {
        r#"{"type":"session_meta","payload":{"session_id":"cross-date","timestamp":"2026-07-10T01:00:00Z","cwd":"D:\\Desktop\\AI\\cross-date"}}
{"type":"event_msg","timestamp":"2026-07-10T01:00:01Z","payload":{"type":"task_started","turn_id":"turn-a","started_at":1783645201}}
{"type":"event_msg","timestamp":"2026-07-10T01:00:02Z","payload":{"type":"user_message","message":"第一天"}}
{"type":"event_msg","timestamp":"2026-07-10T01:00:03Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10,"cached_input_tokens":0,"output_tokens":5,"reasoning_output_tokens":1,"total_tokens":16},"total_token_usage":{"input_tokens":10,"cached_input_tokens":0,"output_tokens":5,"reasoning_output_tokens":1,"total_tokens":16}},"rate_limits":{"primary":{"used_percent":1},"secondary":{}}}}
{"type":"event_msg","timestamp":"2026-07-11T01:00:01Z","payload":{"type":"task_started","turn_id":"turn-b","started_at":1783731601}}
{"type":"event_msg","timestamp":"2026-07-11T01:00:02Z","payload":{"type":"user_message","message":"第二天"}}
{"type":"event_msg","timestamp":"2026-07-11T01:00:03Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":20,"cached_input_tokens":5,"output_tokens":7,"reasoning_output_tokens":2,"total_tokens":29},"total_token_usage":{"input_tokens":30,"cached_input_tokens":5,"output_tokens":12,"reasoning_output_tokens":3,"total_tokens":45}},"rate_limits":{"primary":{"used_percent":2},"secondary":{}}}}"#
            .to_string()
    }
}
