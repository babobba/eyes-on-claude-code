use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::constants::{MINI_VIEW_HEIGHT, MINI_VIEW_WIDTH, SETUP_MODAL_HEIGHT, SETUP_MODAL_WIDTH};
use crate::git::{get_branches, get_git_info, GitInfo};
use crate::persist::save_runtime_state;
use crate::settings::{save_notification_settings, save_settings};
use crate::setup::{self, SetupStatus};
use crate::state::{
    DashboardData, ManagedState, NotificationHistoryState, NotificationSinksState, Settings,
    Transport,
};
use crate::tmux::{self, TmuxPane, TmuxPaneSize};
use crate::tray::{emit_state_update, update_tray_and_badge};
use eocc_core::notifications::{self, NotificationSettings};

const LOCK_ERROR: &str = "Failed to acquire state lock";

#[tauri::command]
pub fn get_dashboard_data(state: tauri::State<'_, ManagedState>) -> Result<DashboardData, String> {
    let state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    Ok(state_guard.to_dashboard_data())
}

#[tauri::command]
pub fn remove_session(
    project_dir: String,
    state: tauri::State<'_, ManagedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    state_guard.sessions.remove(&project_dir);
    update_tray_and_badge(&app, &state_guard);
    emit_state_update(&app, &state_guard);
    save_runtime_state(&app, &state_guard);
    Ok(())
}

#[tauri::command]
pub fn clear_all_sessions(
    state: tauri::State<'_, ManagedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    state_guard.sessions.clear();
    update_tray_and_badge(&app, &state_guard);
    emit_state_update(&app, &state_guard);
    save_runtime_state(&app, &state_guard);
    Ok(())
}

#[tauri::command]
pub fn get_always_on_top(state: tauri::State<'_, ManagedState>) -> Result<bool, String> {
    let state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    Ok(state_guard.settings.always_on_top)
}

#[tauri::command]
pub fn set_always_on_top(
    enabled: bool,
    state: tauri::State<'_, ManagedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    state_guard.settings.always_on_top = enabled;
    save_settings(&app, &state_guard.settings);

    if let Some(window) = app.get_webview_window("dashboard") {
        let _ = window.set_always_on_top(enabled);
    }

    update_tray_and_badge(&app, &state_guard);
    Ok(())
}

/// Set window size for setup modal (enlarged) or normal miniview
#[tauri::command]
pub fn set_window_size_for_setup(enlarged: bool, app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("dashboard") {
        if enlarged {
            let _ = window.set_decorations(true);
            let _ = window.set_size(tauri::LogicalSize::new(
                SETUP_MODAL_WIDTH,
                SETUP_MODAL_HEIGHT,
            ));
            let _ = window.center();
        } else {
            let _ = window.set_decorations(false);
            let _ = window.set_size(tauri::LogicalSize::new(MINI_VIEW_WIDTH, MINI_VIEW_HEIGHT));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, ManagedState>) -> Result<Settings, String> {
    let state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    Ok(state_guard.settings.clone())
}

#[tauri::command]
pub fn set_opacity_active(
    opacity: f64,
    state: tauri::State<'_, ManagedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    state_guard.settings.opacity_active = opacity.clamp(0.1, 1.0);
    save_settings(&app, &state_guard.settings);
    Ok(())
}

#[tauri::command]
pub fn set_opacity_inactive(
    opacity: f64,
    state: tauri::State<'_, ManagedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    state_guard.settings.opacity_inactive = opacity.clamp(0.1, 1.0);
    save_settings(&app, &state_guard.settings);
    Ok(())
}

#[tauri::command]
pub fn get_repo_git_info(project_dir: String) -> GitInfo {
    get_git_info(&project_dir)
}

#[tauri::command]
pub fn get_repo_branches(project_dir: String) -> Vec<String> {
    get_branches(&project_dir)
}

// ============================================================================
// Notification commands
// ============================================================================

#[tauri::command]
pub fn get_notification_settings(
    state: tauri::State<'_, ManagedState>,
) -> Result<NotificationSettings, String> {
    let state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    Ok(state_guard.notification_settings.clone())
}

#[tauri::command]
pub fn update_notification_settings(
    settings: NotificationSettings,
    state: tauri::State<'_, ManagedState>,
    sinks_state: tauri::State<'_, NotificationSinksState>,
) -> Result<(), String> {
    // Save to TOML file
    save_notification_settings(&settings);

    // Rebuild sinks from new config
    let new_sinks = notifications::build_sinks(&settings.channels);
    log::info!(
        target: "eocc.notifications",
        "Rebuilt {} notification channel(s), enabled={}",
        new_sinks.len(),
        settings.enabled
    );

    // Update sinks
    if let Ok(mut sinks) = sinks_state.0.lock() {
        *sinks = new_sinks;
    }

    // Update in-memory state
    let mut state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    state_guard.notification_settings = settings;

    Ok(())
}

#[tauri::command]
pub fn send_test_notification(
    sinks_state: tauri::State<'_, NotificationSinksState>,
    history_state: tauri::State<'_, NotificationHistoryState>,
    state: tauri::State<'_, ManagedState>,
) -> Result<(), String> {
    let state_guard = state.0.lock().map_err(|_| LOCK_ERROR)?;
    if !state_guard.notification_settings.enabled {
        return Err("Notifications are disabled".to_string());
    }

    let notification = notifications::SessionNotification {
        project_name: "EOCC Test".to_string(),
        project_dir: "/test".to_string(),
        session_id: "test".to_string(),
        old_status: None,
        new_status: eocc_core::state::SessionStatus::WaitingInput,
        message: "This is a test notification from Eyes on Claude Code".to_string(),
        priority: notifications::NotificationPriority::Normal,
        title_template: state_guard.notification_settings.title_template.clone(),
        body_template: state_guard.notification_settings.body_template.clone(),
        click_url: None,
    };

    let sinks = sinks_state
        .0
        .lock()
        .map_err(|_| "Failed to acquire sinks lock")?;
    if sinks.is_empty() {
        return Err("No notification channels configured".to_string());
    }
    let record = notifications::dispatch(&sinks, &notification);
    if let Ok(mut history) = history_state.0.lock() {
        history.push(record);
    }
    Ok(())
}

#[tauri::command]
pub fn get_notification_history(
    history_state: tauri::State<'_, NotificationHistoryState>,
) -> Result<Vec<notifications::history::NotificationRecord>, String> {
    let history = history_state.0.lock().map_err(|_| LOCK_ERROR)?;
    Ok(history.records())
}

#[tauri::command]
pub fn clear_notification_history(
    history_state: tauri::State<'_, NotificationHistoryState>,
) -> Result<(), String> {
    let mut history = history_state.0.lock().map_err(|_| LOCK_ERROR)?;
    history.clear();
    Ok(())
}

// ============================================================================
// Setup commands
// ============================================================================

/// Get the current setup status
#[tauri::command]
pub fn get_setup_status(app: tauri::AppHandle) -> SetupStatus {
    setup::get_setup_status(&app)
}

/// Install the hook script to app data directory
#[tauri::command]
pub fn install_hook(app: tauri::AppHandle) -> Result<String, String> {
    let path = setup::install_hook_script(&app)?;
    Ok(path.to_string_lossy().to_string())
}

/// Check Claude settings and return merged settings if needed
#[tauri::command]
pub fn check_claude_settings(app: tauri::AppHandle) -> Result<SetupStatus, String> {
    // Ensure hook is installed first
    if !setup::is_hook_installed(&app) {
        setup::install_hook_script(&app)?;
    }
    Ok(setup::get_setup_status(&app))
}

/// Open the Claude settings.json file in the default editor
#[tauri::command]
pub fn open_claude_settings() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Failed to get home directory")?;
    let claude_dir = home.join(".claude");
    let settings_path = claude_dir.join("settings.json");

    // Create directory and file if they don't exist
    if !settings_path.exists() {
        std::fs::create_dir_all(&claude_dir)
            .map_err(|e| format!("Failed to create .claude directory: {:?}", e))?;
        std::fs::write(&settings_path, "{}\n")
            .map_err(|e| format!("Failed to create settings.json: {:?}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&settings_path)
            .spawn()
            .map_err(|e| format!("Failed to open settings: {:?}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&settings_path)
            .spawn()
            .map_err(|e| format!("Failed to open settings: {:?}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &settings_path.to_string_lossy()])
            .spawn()
            .map_err(|e| format!("Failed to open settings: {:?}", e))?;
    }

    Ok(())
}

// ============================================================================
// Tmux commands
// ============================================================================

/// Look up the transport for a session by project_dir.
fn get_transport_for_session(
    state: &tauri::State<'_, ManagedState>,
    project_dir: Option<&str>,
) -> Transport {
    if let Some(dir) = project_dir {
        if let Ok(state_guard) = state.0.lock() {
            if let Some(session) = state_guard.sessions.get(dir) {
                return session.transport.clone();
            }
        }
    }
    Transport::Local {}
}

#[tauri::command]
pub fn open_tmux_viewer(
    pane_id: String,
    project_dir: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    pane_id.hash(&mut hasher);
    let window_label = format!("tmux-viewer-{:x}", hasher.finish());

    // Check if window already exists - if so, focus it and return
    if let Some(existing_window) = app.get_webview_window(&window_label) {
        let _ = existing_window.show();
        let _ = existing_window.set_focus();
        return Ok(());
    }

    let mut url = format!("index.html?tmux_pane={}", urlencoding::encode(&pane_id));
    if let Some(ref dir) = project_dir {
        url.push_str(&format!("&project_dir={}", urlencoding::encode(dir)));
    }

    WebviewWindowBuilder::new(&app, &window_label, WebviewUrl::App(url.into()))
        .title(format!("tmux - {}", pane_id))
        .inner_size(800.0, 600.0)
        .center()
        .transparent(true)
        .decorations(true)
        .build()
        .map_err(|e| format!("Failed to create tmux viewer window: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn tmux_is_available() -> bool {
    tmux::is_tmux_available()
}

#[tauri::command]
pub fn tmux_list_panes() -> Result<Vec<TmuxPane>, String> {
    tmux::list_panes()
}

#[tauri::command]
pub fn tmux_capture_pane(
    pane_id: String,
    project_dir: Option<String>,
    state: tauri::State<'_, ManagedState>,
) -> Result<String, String> {
    let transport = get_transport_for_session(&state, project_dir.as_deref());
    tmux::capture_pane_with_transport(&pane_id, &transport)
}

#[tauri::command]
pub fn tmux_send_keys(
    pane_id: String,
    keys: String,
    project_dir: Option<String>,
    state: tauri::State<'_, ManagedState>,
) -> Result<(), String> {
    let transport = get_transport_for_session(&state, project_dir.as_deref());
    tmux::send_keys_with_transport(&pane_id, &keys, &transport)
}

#[tauri::command]
pub fn tmux_get_pane_size(
    pane_id: String,
    project_dir: Option<String>,
    state: tauri::State<'_, ManagedState>,
) -> Result<TmuxPaneSize, String> {
    let transport = get_transport_for_session(&state, project_dir.as_deref());
    tmux::get_pane_size_with_transport(&pane_id, &transport)
}
