use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;

use crate::settings::get_config_dir;
use crate::state::{AppState, CachedPaths, EventInfo, SessionInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedRuntimeState {
    pub sessions: HashMap<String, SessionInfo>,
    pub recent_events: VecDeque<EventInfo>,
    #[serde(default)]
    pub cached_paths: CachedPaths,
}

fn get_runtime_state_file(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    get_config_dir(app).map(|dir| dir.join("runtime_state.json"))
}

pub fn load_runtime_state(app: &tauri::AppHandle) -> Option<PersistedRuntimeState> {
    let path = get_runtime_state_file(app).ok()?;
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Create a snapshot of the runtime state that can be saved outside of a lock.
pub fn create_runtime_snapshot(state: &AppState) -> PersistedRuntimeState {
    PersistedRuntimeState {
        sessions: state.sessions.clone(),
        recent_events: state.recent_events.clone(),
        cached_paths: state.cached_paths.clone(),
    }
}

/// Save a pre-created runtime state snapshot to disk.
/// This function performs file I/O and should be called outside of any locks.
pub fn save_runtime_snapshot(app: &tauri::AppHandle, persisted: &PersistedRuntimeState) {
    let config_dir = match get_config_dir(app) {
        Ok(path) => path,
        Err(e) => {
            log::error!(target: "eocc.persist", "Cannot determine app data dir: {}", e);
            return;
        }
    };

    if let Err(e) = fs::create_dir_all(&config_dir) {
        log::error!(target: "eocc.persist", "Failed to create app data dir: {:?}", e);
        return;
    }

    let path = config_dir.join("runtime_state.json");

    let content = match serde_json::to_string_pretty(persisted) {
        Ok(c) => c,
        Err(e) => {
            log::error!(target: "eocc.persist", "Failed to serialize runtime state: {:?}", e);
            return;
        }
    };

    if let Err(e) = fs::write(&path, content) {
        log::error!(target: "eocc.persist", "Failed to write runtime state: {:?}", e);
    }
}

pub fn save_runtime_state(app: &tauri::AppHandle, state: &AppState) {
    let persisted = create_runtime_snapshot(state);
    save_runtime_snapshot(app, &persisted);
}
