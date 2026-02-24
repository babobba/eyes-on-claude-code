use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookSessionState {
    pub status: String,
    #[serde(default)]
    pub last_notified: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookState {
    pub sessions: HashMap<String, HookSessionState>,
}

pub fn load(path: &Path) -> HookState {
    fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, state: &HookState) {
    let Ok(content) = serde_json::to_string(state) else {
        return;
    };
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    if fs::write(&tmp, &content).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_hook_state() {
        let state = HookState::default();
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn load_nonexistent_returns_default() {
        let state = load(Path::new("/tmp/nonexistent_eocc_hook_state.json"));
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("eocc_hook_state_test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("hook_state.json");
        let _ = fs::remove_file(&path);

        let mut state = HookState::default();
        state.sessions.insert(
            "/home/user/proj".to_string(),
            HookSessionState {
                status: "active".to_string(),
                last_notified: Some(1700000000000),
            },
        );

        save(&path, &state);
        let loaded = load(&path);
        assert_eq!(loaded.sessions.len(), 1);
        let session = loaded.sessions.get("/home/user/proj").unwrap();
        assert_eq!(session.status, "active");
        assert_eq!(session.last_notified, Some(1700000000000));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn load_invalid_json_returns_default() {
        let dir = std::env::temp_dir().join("eocc_hook_state_test_invalid");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("hook_state.json");
        fs::write(&path, "not valid json").unwrap();

        let state = load(&path);
        assert!(state.sessions.is_empty());

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn session_state_without_last_notified() {
        let json = r#"{"sessions":{"/proj":{"status":"completed"}}}"#;
        let state: HookState = serde_json::from_str(json).unwrap();
        let session = state.sessions.get("/proj").unwrap();
        assert_eq!(session.status, "completed");
        assert!(session.last_notified.is_none());
    }
}
