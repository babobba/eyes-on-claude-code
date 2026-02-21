use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    SessionStart,
    SessionEnd,
    Notification,
    Stop,
    PostToolUse,
    UserPromptSubmit,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    PermissionPrompt,
    IdlePrompt,
    #[serde(other)]
    #[default]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInfo {
    pub timestamp: String,
    #[serde(rename = "event")]
    pub event_type: EventType,
    pub matcher: String,
    pub project_name: String,
    pub project_dir: String,
    pub session_id: String,
    pub message: String,
    #[serde(default)]
    pub notification_type: NotificationType,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub tmux_pane: String,
    #[serde(default)]
    pub npx_path: String,
    #[serde(default)]
    pub tmux_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub project_name: String,
    pub project_dir: String,
    pub status: SessionStatus,
    pub last_event: String,
    #[serde(default)]
    pub waiting_for: String,
    #[serde(default)]
    pub tmux_pane: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    WaitingPermission,
    WaitingInput,
    Completed,
}

impl SessionStatus {
    pub fn emoji(&self) -> &str {
        match self {
            SessionStatus::Active => "🟢",
            SessionStatus::WaitingPermission => "🔐",
            SessionStatus::WaitingInput => "⏳",
            SessionStatus::Completed => "✅",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub sessions: Vec<SessionInfo>,
    pub events: Vec<EventInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "Settings::default_always_on_top")]
    pub always_on_top: bool,
    #[serde(default = "Settings::default_minimum_mode_enabled")]
    pub minimum_mode_enabled: bool,
    #[serde(default = "Settings::default_opacity_active")]
    pub opacity_active: f64,
    #[serde(default = "Settings::default_opacity_inactive")]
    pub opacity_inactive: f64,
    #[serde(default = "Settings::default_sound_enabled")]
    pub sound_enabled: bool,
}

impl Settings {
    pub const DEFAULT_ALWAYS_ON_TOP: bool = true;
    pub const DEFAULT_MINIMUM_MODE_ENABLED: bool = true;
    pub const DEFAULT_OPACITY_ACTIVE: f64 = 1.0;
    pub const DEFAULT_OPACITY_INACTIVE: f64 = 1.0;
    pub const DEFAULT_SOUND_ENABLED: bool = true;

    fn default_always_on_top() -> bool {
        Self::DEFAULT_ALWAYS_ON_TOP
    }

    fn default_minimum_mode_enabled() -> bool {
        Self::DEFAULT_MINIMUM_MODE_ENABLED
    }

    fn default_opacity_active() -> f64 {
        Self::DEFAULT_OPACITY_ACTIVE
    }

    fn default_opacity_inactive() -> f64 {
        Self::DEFAULT_OPACITY_INACTIVE
    }

    fn default_sound_enabled() -> bool {
        Self::DEFAULT_SOUND_ENABLED
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            always_on_top: Self::DEFAULT_ALWAYS_ON_TOP,
            minimum_mode_enabled: Self::DEFAULT_MINIMUM_MODE_ENABLED,
            opacity_active: Self::DEFAULT_OPACITY_ACTIVE,
            opacity_inactive: Self::DEFAULT_OPACITY_INACTIVE,
            sound_enabled: Self::DEFAULT_SOUND_ENABLED,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachedPaths {
    #[serde(default)]
    pub npx_path: String,
    #[serde(default)]
    pub tmux_path: String,
}

impl CachedPaths {
    pub fn update_from_event(&mut self, event: &EventInfo) {
        if !event.npx_path.is_empty() {
            self.npx_path = event.npx_path.clone();
        }
        if !event.tmux_path.is_empty() {
            self.tmux_path = event.tmux_path.clone();
        }
    }
}

#[derive(Default)]
pub struct AppState {
    pub sessions: HashMap<String, SessionInfo>,
    pub recent_events: VecDeque<EventInfo>,
    pub settings: Settings,
    pub cached_paths: CachedPaths,
}

impl AppState {
    pub fn waiting_session_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| {
                s.status == SessionStatus::WaitingPermission
                    || s.status == SessionStatus::WaitingInput
            })
            .count()
    }

    pub fn to_dashboard_data(&self) -> DashboardData {
        let mut sessions: Vec<SessionInfo> = self.sessions.values().cloned().collect();
        sessions.sort_by(|a, b| match (a.last_event.is_empty(), b.last_event.is_empty()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            (false, false) => b.last_event.cmp(&a.last_event),
        });

        DashboardData {
            sessions,
            events: self.recent_events.iter().cloned().collect(),
        }
    }

    pub fn upsert_session(
        &mut self,
        key: String,
        event: &EventInfo,
        status: SessionStatus,
        waiting_for: String,
    ) {
        self.sessions
            .entry(key)
            .and_modify(|s| {
                s.status = status.clone();
                s.last_event = event.timestamp.clone();
                s.waiting_for = waiting_for.clone();
                if !event.tmux_pane.is_empty() {
                    s.tmux_pane = event.tmux_pane.clone();
                }
            })
            .or_insert_with(|| SessionInfo {
                project_name: event.project_name.clone(),
                project_dir: event.project_dir.clone(),
                status,
                last_event: event.timestamp.clone(),
                waiting_for,
                tmux_pane: event.tmux_pane.clone(),
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(event_type: EventType) -> EventInfo {
        EventInfo {
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            event_type,
            matcher: String::new(),
            project_name: "test-project".to_string(),
            project_dir: "/home/user/test-project".to_string(),
            session_id: "session-1".to_string(),
            message: String::new(),
            notification_type: NotificationType::Other,
            tool_name: String::new(),
            tmux_pane: String::new(),
            npx_path: String::new(),
            tmux_path: String::new(),
        }
    }

    // -- EventType serde --

    #[test]
    fn event_type_deserializes_all_variants() {
        assert_eq!(
            serde_json::from_str::<EventType>(r#""session_start""#).unwrap(),
            EventType::SessionStart
        );
        assert_eq!(
            serde_json::from_str::<EventType>(r#""session_end""#).unwrap(),
            EventType::SessionEnd
        );
        assert_eq!(
            serde_json::from_str::<EventType>(r#""notification""#).unwrap(),
            EventType::Notification
        );
        assert_eq!(
            serde_json::from_str::<EventType>(r#""stop""#).unwrap(),
            EventType::Stop
        );
        assert_eq!(
            serde_json::from_str::<EventType>(r#""post_tool_use""#).unwrap(),
            EventType::PostToolUse
        );
        assert_eq!(
            serde_json::from_str::<EventType>(r#""user_prompt_submit""#).unwrap(),
            EventType::UserPromptSubmit
        );
    }

    #[test]
    fn event_type_unknown_variant_fallback() {
        assert_eq!(
            serde_json::from_str::<EventType>(r#""something_new""#).unwrap(),
            EventType::Unknown
        );
    }

    #[test]
    fn event_type_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&EventType::SessionStart).unwrap(),
            r#""session_start""#
        );
        assert_eq!(
            serde_json::to_string(&EventType::PostToolUse).unwrap(),
            r#""post_tool_use""#
        );
    }

    // -- NotificationType serde --

    #[test]
    fn notification_type_deserializes_variants() {
        assert_eq!(
            serde_json::from_str::<NotificationType>(r#""permission_prompt""#).unwrap(),
            NotificationType::PermissionPrompt
        );
        assert_eq!(
            serde_json::from_str::<NotificationType>(r#""idle_prompt""#).unwrap(),
            NotificationType::IdlePrompt
        );
        assert_eq!(
            serde_json::from_str::<NotificationType>(r#""something_else""#).unwrap(),
            NotificationType::Other
        );
    }

    #[test]
    fn notification_type_default_is_other() {
        assert_eq!(NotificationType::default(), NotificationType::Other);
    }

    // -- SessionStatus --

    #[test]
    fn session_status_emojis() {
        assert_eq!(SessionStatus::Active.emoji(), "🟢");
        assert_eq!(SessionStatus::WaitingPermission.emoji(), "🔐");
        assert_eq!(SessionStatus::WaitingInput.emoji(), "⏳");
        assert_eq!(SessionStatus::Completed.emoji(), "✅");
    }

    #[test]
    fn session_status_serializes_pascal_case() {
        assert_eq!(
            serde_json::to_string(&SessionStatus::WaitingPermission).unwrap(),
            r#""WaitingPermission""#
        );
    }

    // -- Settings --

    #[test]
    fn settings_default_values() {
        let settings = Settings::default();
        assert!(settings.always_on_top);
        assert!(settings.minimum_mode_enabled);
        assert_eq!(settings.opacity_active, 1.0);
        assert_eq!(settings.opacity_inactive, 1.0);
        assert!(settings.sound_enabled);
    }

    #[test]
    fn settings_deserializes_with_defaults_for_missing_fields() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(settings.always_on_top);
        assert!(settings.minimum_mode_enabled);
        assert_eq!(settings.opacity_active, 1.0);
        assert!(settings.sound_enabled);
    }

    #[test]
    fn settings_deserializes_overrides() {
        let json = r#"{"always_on_top": false, "opacity_active": 0.8, "sound_enabled": false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(!settings.always_on_top);
        assert_eq!(settings.opacity_active, 0.8);
        assert!(!settings.sound_enabled);
        assert!(settings.minimum_mode_enabled);
        assert_eq!(settings.opacity_inactive, 1.0);
    }

    #[test]
    fn settings_roundtrip() {
        let settings = Settings {
            always_on_top: false,
            minimum_mode_enabled: false,
            opacity_active: 0.5,
            opacity_inactive: 0.3,
            sound_enabled: false,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.always_on_top);
        assert!(!deserialized.minimum_mode_enabled);
        assert_eq!(deserialized.opacity_active, 0.5);
        assert_eq!(deserialized.opacity_inactive, 0.3);
        assert!(!deserialized.sound_enabled);
    }

    // -- CachedPaths --

    #[test]
    fn cached_paths_default_is_empty() {
        let paths = CachedPaths::default();
        assert!(paths.npx_path.is_empty());
        assert!(paths.tmux_path.is_empty());
    }

    #[test]
    fn cached_paths_update_sets_nonempty_paths() {
        let mut paths = CachedPaths::default();
        let mut event = make_event(EventType::SessionStart);
        event.npx_path = "/usr/local/bin/npx".to_string();
        event.tmux_path = "/usr/local/bin/tmux".to_string();

        paths.update_from_event(&event);
        assert_eq!(paths.npx_path, "/usr/local/bin/npx");
        assert_eq!(paths.tmux_path, "/usr/local/bin/tmux");
    }

    #[test]
    fn cached_paths_ignores_empty_values() {
        let mut paths = CachedPaths {
            npx_path: "/existing/npx".to_string(),
            tmux_path: "/existing/tmux".to_string(),
        };
        let event = make_event(EventType::SessionStart);

        paths.update_from_event(&event);
        assert_eq!(paths.npx_path, "/existing/npx");
        assert_eq!(paths.tmux_path, "/existing/tmux");
    }

    // -- AppState --

    #[test]
    fn app_state_default_is_empty() {
        let state = AppState::default();
        assert!(state.sessions.is_empty());
        assert!(state.recent_events.is_empty());
        assert_eq!(state.waiting_session_count(), 0);
    }

    #[test]
    fn waiting_count_counts_permission_and_input() {
        let mut state = AppState::default();
        state.sessions.insert(
            "p1".into(),
            SessionInfo {
                project_name: "p1".into(),
                project_dir: "/p1".into(),
                status: SessionStatus::WaitingPermission,
                last_event: String::new(),
                waiting_for: String::new(),
                tmux_pane: String::new(),
            },
        );
        state.sessions.insert(
            "p2".into(),
            SessionInfo {
                project_name: "p2".into(),
                project_dir: "/p2".into(),
                status: SessionStatus::WaitingInput,
                last_event: String::new(),
                waiting_for: String::new(),
                tmux_pane: String::new(),
            },
        );
        assert_eq!(state.waiting_session_count(), 2);
    }

    #[test]
    fn waiting_count_ignores_active_and_completed() {
        let mut state = AppState::default();
        state.sessions.insert(
            "active".into(),
            SessionInfo {
                project_name: "active".into(),
                project_dir: "/active".into(),
                status: SessionStatus::Active,
                last_event: String::new(),
                waiting_for: String::new(),
                tmux_pane: String::new(),
            },
        );
        state.sessions.insert(
            "completed".into(),
            SessionInfo {
                project_name: "completed".into(),
                project_dir: "/completed".into(),
                status: SessionStatus::Completed,
                last_event: String::new(),
                waiting_for: String::new(),
                tmux_pane: String::new(),
            },
        );
        assert_eq!(state.waiting_session_count(), 0);
    }

    #[test]
    fn dashboard_data_sorts_by_timestamp_desc() {
        let mut state = AppState::default();
        state.sessions.insert(
            "old".into(),
            SessionInfo {
                project_name: "old".into(),
                project_dir: "/old".into(),
                status: SessionStatus::Active,
                last_event: "2025-01-01T00:00:00Z".into(),
                waiting_for: String::new(),
                tmux_pane: String::new(),
            },
        );
        state.sessions.insert(
            "new".into(),
            SessionInfo {
                project_name: "new".into(),
                project_dir: "/new".into(),
                status: SessionStatus::Active,
                last_event: "2025-01-02T00:00:00Z".into(),
                waiting_for: String::new(),
                tmux_pane: String::new(),
            },
        );

        let data = state.to_dashboard_data();
        assert_eq!(data.sessions[0].project_name, "new");
        assert_eq!(data.sessions[1].project_name, "old");
    }

    #[test]
    fn dashboard_data_empty_timestamps_sort_last() {
        let mut state = AppState::default();
        state.sessions.insert(
            "no-ts".into(),
            SessionInfo {
                project_name: "no-ts".into(),
                project_dir: "/no-ts".into(),
                status: SessionStatus::Active,
                last_event: String::new(),
                waiting_for: String::new(),
                tmux_pane: String::new(),
            },
        );
        state.sessions.insert(
            "has-ts".into(),
            SessionInfo {
                project_name: "has-ts".into(),
                project_dir: "/has-ts".into(),
                status: SessionStatus::Active,
                last_event: "2025-01-01T00:00:00Z".into(),
                waiting_for: String::new(),
                tmux_pane: String::new(),
            },
        );

        let data = state.to_dashboard_data();
        assert_eq!(data.sessions[0].project_name, "has-ts");
        assert_eq!(data.sessions[1].project_name, "no-ts");
    }

    #[test]
    fn upsert_inserts_new_session() {
        let mut state = AppState::default();
        let event = make_event(EventType::SessionStart);
        state.upsert_session(
            "/home/user/test-project".into(),
            &event,
            SessionStatus::Active,
            String::new(),
        );

        assert_eq!(state.sessions.len(), 1);
        let session = state.sessions.get("/home/user/test-project").unwrap();
        assert_eq!(session.status, SessionStatus::Active);
        assert_eq!(session.project_name, "test-project");
    }

    #[test]
    fn upsert_updates_existing_session() {
        let mut state = AppState::default();
        let event = make_event(EventType::SessionStart);
        state.upsert_session(
            "/proj".into(),
            &event,
            SessionStatus::Active,
            String::new(),
        );

        let mut event2 = make_event(EventType::Notification);
        event2.timestamp = "2025-01-01T01:00:00Z".into();
        state.upsert_session(
            "/proj".into(),
            &event2,
            SessionStatus::WaitingPermission,
            "Approve tool".into(),
        );

        assert_eq!(state.sessions.len(), 1);
        let session = state.sessions.get("/proj").unwrap();
        assert_eq!(session.status, SessionStatus::WaitingPermission);
        assert_eq!(session.waiting_for, "Approve tool");
        assert_eq!(session.last_event, "2025-01-01T01:00:00Z");
    }

    #[test]
    fn upsert_preserves_tmux_pane_when_event_pane_empty() {
        let mut state = AppState::default();
        let mut event = make_event(EventType::SessionStart);
        event.tmux_pane = "%0".into();
        state.upsert_session(
            "/proj".into(),
            &event,
            SessionStatus::Active,
            String::new(),
        );

        let event2 = make_event(EventType::PostToolUse);
        state.upsert_session(
            "/proj".into(),
            &event2,
            SessionStatus::Active,
            String::new(),
        );
        assert_eq!(state.sessions.get("/proj").unwrap().tmux_pane, "%0");
    }

    // -- EventInfo deserialization --

    #[test]
    fn event_info_full_json_roundtrip() {
        let json = r#"{
            "timestamp": "2025-01-01T00:00:00Z",
            "event": "session_start",
            "matcher": "",
            "project_name": "myproject",
            "project_dir": "/home/user/myproject",
            "session_id": "abc123",
            "message": "",
            "notification_type": "other",
            "tool_name": "",
            "tmux_pane": "%0",
            "npx_path": "/usr/bin/npx",
            "tmux_path": "/usr/bin/tmux"
        }"#;
        let event: EventInfo = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, EventType::SessionStart);
        assert_eq!(event.project_name, "myproject");
        assert_eq!(event.tmux_pane, "%0");
        assert_eq!(event.npx_path, "/usr/bin/npx");
    }

    #[test]
    fn event_info_optional_fields_default() {
        let json = r#"{
            "timestamp": "2025-01-01T00:00:00Z",
            "event": "post_tool_use",
            "matcher": "",
            "project_name": "proj",
            "project_dir": "/proj",
            "session_id": "s1",
            "message": ""
        }"#;
        let event: EventInfo = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, EventType::PostToolUse);
        assert_eq!(event.notification_type, NotificationType::Other);
        assert!(event.tool_name.is_empty());
        assert!(event.tmux_pane.is_empty());
        assert!(event.npx_path.is_empty());
        assert!(event.tmux_path.is_empty());
    }
}
