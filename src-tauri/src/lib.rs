use std::path::PathBuf;
use std::sync::Arc;

use agent_restore_core::{
    default_exclusion_summary, AgentRestoreEngine, Checkpoint, CheckpointDiffSummary,
    CheckpointFile, CoreError, CrashRecoveryReport, CreateCheckpointOptions,
    CreateCheckpointOutcome, CreateProjectRequest, ExecuteRestoreRequest, FileDiffResult, GcReport,
    GlobalSettings, IntegrityReport, MonitorService, Project, RestorePreview, RestoreResult,
    StorageStats, StructuredError, TriggerType, UpdateProjectRequest, WorkspaceStatus,
};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

struct AppState {
    engine: Arc<AgentRestoreEngine>,
    monitor: Arc<MonitorService>,
}

fn map_err(err: CoreError) -> StructuredError {
    err.to_structured()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExclusionDefaults {
    security: Vec<String>,
    caches: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppMeta {
    version: String,
    data_dir: String,
    monitoring_paused: bool,
    platform: String,
}

#[tauri::command]
fn get_app_meta(state: State<'_, AppState>) -> AppMeta {
    AppMeta {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: state.engine.data_dir().to_string_lossy().to_string(),
        monitoring_paused: state.monitor.is_paused(),
        platform: std::env::consts::OS.to_string(),
    }
}

#[tauri::command]
fn get_default_exclusions() -> ExclusionDefaults {
    let (security, caches) = default_exclusion_summary();
    ExclusionDefaults { security, caches }
}

#[tauri::command]
fn get_crash_recovery_report(
    state: State<'_, AppState>,
) -> Result<CrashRecoveryReport, StructuredError> {
    state.engine.get_crash_recovery_report().map_err(map_err)
}

#[tauri::command]
fn dismiss_crash_recovery_notices(state: State<'_, AppState>) -> Result<(), StructuredError> {
    state
        .engine
        .dismiss_crash_recovery_notices()
        .map_err(map_err)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<GlobalSettings, StructuredError> {
    state.engine.get_settings().map_err(map_err)
}

#[tauri::command]
fn save_settings(
    state: State<'_, AppState>,
    settings: GlobalSettings,
) -> Result<GlobalSettings, StructuredError> {
    let saved = state.engine.save_settings(settings).map_err(map_err)?;
    state.monitor.reload_projects();
    Ok(saved)
}

#[tauri::command]
fn set_monitoring_paused(state: State<'_, AppState>, paused: bool) -> bool {
    state.monitor.set_paused(paused);
    paused
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, StructuredError> {
    state.engine.list_projects().map_err(map_err)
}

#[tauri::command]
fn get_project(state: State<'_, AppState>, project_id: String) -> Result<Project, StructuredError> {
    state.engine.get_project(&project_id).map_err(map_err)
}

#[tauri::command]
fn add_project(
    state: State<'_, AppState>,
    request: CreateProjectRequest,
) -> Result<Project, StructuredError> {
    let project = state.engine.add_project(request).map_err(map_err)?;
    state.monitor.reload_projects();
    Ok(project)
}

#[tauri::command]
fn update_project(
    state: State<'_, AppState>,
    project_id: String,
    request: UpdateProjectRequest,
) -> Result<Project, StructuredError> {
    let project = state
        .engine
        .update_project(&project_id, request)
        .map_err(map_err)?;
    state.monitor.reload_projects();
    Ok(project)
}

#[tauri::command]
fn remove_project(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<GcReport, StructuredError> {
    let report = state.engine.remove_project(&project_id).map_err(map_err)?;
    state.monitor.reload_projects();
    Ok(report)
}

#[tauri::command]
fn create_checkpoint(
    state: State<'_, AppState>,
    project_id: String,
    label: Option<String>,
    note: Option<String>,
    force_if_unchanged: Option<bool>,
) -> Result<CreateCheckpointOutcome, StructuredError> {
    state
        .engine
        .create_checkpoint(CreateCheckpointOptions {
            project_id,
            label,
            note,
            trigger_type: TriggerType::Manual,
            force_if_unchanged: force_if_unchanged.unwrap_or(true),
        })
        .map_err(map_err)
}

#[tauri::command]
fn list_checkpoints(
    state: State<'_, AppState>,
    project_id: String,
    query: Option<String>,
    trigger_filter: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Checkpoint>, StructuredError> {
    state
        .engine
        .list_checkpoints(
            &project_id,
            query.as_deref(),
            trigger_filter.as_deref(),
            limit,
        )
        .map_err(map_err)
}

#[tauri::command]
fn rename_checkpoint(
    state: State<'_, AppState>,
    checkpoint_id: String,
    label: String,
    note: Option<String>,
) -> Result<Checkpoint, StructuredError> {
    state
        .engine
        .rename_checkpoint(&checkpoint_id, &label, note.as_deref())
        .map_err(map_err)
}

#[tauri::command]
fn delete_checkpoint(
    state: State<'_, AppState>,
    checkpoint_id: String,
) -> Result<GcReport, StructuredError> {
    state
        .engine
        .delete_checkpoint(&checkpoint_id)
        .map_err(map_err)
}

#[tauri::command]
fn get_checkpoint_files(
    state: State<'_, AppState>,
    checkpoint_id: String,
) -> Result<Vec<CheckpointFile>, StructuredError> {
    state
        .engine
        .get_checkpoint_files(&checkpoint_id)
        .map_err(map_err)
}

#[tauri::command]
fn diff_checkpoints(
    state: State<'_, AppState>,
    base_checkpoint_id: Option<String>,
    target_checkpoint_id: String,
) -> Result<CheckpointDiffSummary, StructuredError> {
    state
        .engine
        .diff_checkpoints(base_checkpoint_id.as_deref(), &target_checkpoint_id)
        .map_err(map_err)
}

#[tauri::command]
fn diff_checkpoint_with_workspace(
    state: State<'_, AppState>,
    project_id: String,
    checkpoint_id: String,
) -> Result<CheckpointDiffSummary, StructuredError> {
    state
        .engine
        .diff_checkpoint_with_workspace(&project_id, &checkpoint_id)
        .map_err(map_err)
}

#[tauri::command]
fn get_file_diff(
    state: State<'_, AppState>,
    project_id: String,
    checkpoint_id: String,
    rel_path: String,
    base_checkpoint_id: Option<String>,
    compare_with_workspace: bool,
) -> Result<FileDiffResult, StructuredError> {
    state
        .engine
        .get_file_diff(
            &project_id,
            &checkpoint_id,
            &rel_path,
            base_checkpoint_id.as_deref(),
            compare_with_workspace,
        )
        .map_err(map_err)
}

#[tauri::command]
fn get_workspace_status(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<WorkspaceStatus, StructuredError> {
    state
        .engine
        .get_workspace_status(&project_id)
        .map_err(map_err)
}

#[tauri::command]
fn preview_restore(
    state: State<'_, AppState>,
    project_id: String,
    checkpoint_id: String,
    selected_paths: Option<Vec<String>>,
    delete_extra_files: bool,
) -> Result<RestorePreview, StructuredError> {
    state
        .engine
        .preview_restore(
            &project_id,
            &checkpoint_id,
            selected_paths,
            delete_extra_files,
        )
        .map_err(map_err)
}

#[tauri::command]
fn execute_restore(
    state: State<'_, AppState>,
    request: ExecuteRestoreRequest,
) -> Result<RestoreResult, StructuredError> {
    let pid = request.project_id.clone();
    state.monitor.suspend_project(&pid);
    let res = state.engine.execute_restore(request).map_err(map_err);
    state.monitor.resume_project(&pid);
    res
}

#[tauri::command]
fn get_storage_stats(state: State<'_, AppState>) -> Result<StorageStats, StructuredError> {
    state.engine.get_storage_stats().map_err(map_err)
}

#[tauri::command]
fn run_gc(state: State<'_, AppState>) -> Result<GcReport, StructuredError> {
    state.engine.run_gc().map_err(map_err)
}

#[tauri::command]
fn apply_retention(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<GcReport, StructuredError> {
    state
        .engine
        .apply_retention_for_project(&project_id)
        .map_err(map_err)
}

#[tauri::command]
fn verify_integrity(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<IntegrityReport, StructuredError> {
    state
        .engine
        .verify_integrity(project_id.as_deref())
        .map_err(map_err)
}

fn resolve_data_dir(app: &AppHandle) -> PathBuf {
    if let Ok(custom) = std::env::var("AGENT_RESTORE_DATA_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    app.path()
        .app_local_data_dir()
        .unwrap_or_else(|_| PathBuf::from(".agent-restore-data"))
        .join("store")
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "tray_open", "Open Agent Restore", true, None::<&str>)?;
    let checkpoint_item = MenuItem::with_id(
        app,
        "tray_checkpoint_all",
        "Create Checkpoint (All Enabled)",
        true,
        None::<&str>,
    )?;
    let toggle_item = MenuItem::with_id(
        app,
        "tray_toggle_pause",
        "Pause / Resume Monitoring",
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "tray_quit", "Quit Agent Restore", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&open_item, &checkpoint_item, &toggle_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("Agent Restore — Local Checkpoint Protection")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray_open" => show_main_window(app),
            "tray_toggle_pause" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let next = !state.monitor.is_paused();
                    state.monitor.set_paused(next);
                }
            }
            "tray_checkpoint_all" => {
                let app_handle = app.clone();
                std::thread::spawn(move || {
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        if let Ok(projects) = state.engine.list_projects() {
                            for p in projects.into_iter().filter(|p| p.enabled) {
                                let _ = state.engine.create_checkpoint(CreateCheckpointOptions {
                                    project_id: p.id.clone(),
                                    label: Some("Tray Quick Checkpoint".to_string()),
                                    note: None,
                                    trigger_type: TriggerType::Manual,
                                    force_if_unchanged: false,
                                });
                            }
                            let _ = app_handle.emit("agent-restore://refresh", ());
                        }
                    }
                });
            }
            "tray_quit" => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.monitor.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _ = builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = resolve_data_dir(app.handle());
            let engine = Arc::new(
                AgentRestoreEngine::open(&data_dir)
                    .map_err(|e| format!("Failed to initialize Agent Restore store: {}", e))?,
            );

            let handle_for_events = app.handle().clone();
            let monitor = Arc::new(MonitorService::start(engine.clone(), move |ev| {
                let _ = handle_for_events.emit("agent-restore://monitor", &ev);
            }));

            app.manage(AppState { engine, monitor });
            let _ = setup_tray(app.handle());

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let minimize_to_tray = window
                    .app_handle()
                    .try_state::<AppState>()
                    .and_then(|st| st.engine.get_settings().ok())
                    .map(|s| s.minimize_to_tray)
                    .unwrap_or(false);

                if minimize_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_meta,
            get_default_exclusions,
            get_crash_recovery_report,
            dismiss_crash_recovery_notices,
            get_settings,
            save_settings,
            set_monitoring_paused,
            list_projects,
            get_project,
            add_project,
            update_project,
            remove_project,
            create_checkpoint,
            list_checkpoints,
            rename_checkpoint,
            delete_checkpoint,
            get_checkpoint_files,
            diff_checkpoints,
            diff_checkpoint_with_workspace,
            get_file_diff,
            get_workspace_status,
            preview_restore,
            execute_restore,
            get_storage_stats,
            run_gc,
            apply_retention,
            verify_integrity,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Restore");
}
