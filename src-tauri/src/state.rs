use eocc_core::notifications::NotificationSettings;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

fn default_ssh_port() -> u16 {
    22
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Transport {
    Local {},
    Ssh {
        host: String,
        #[serde(default = "default_ssh_port")]
        port: u16,
        user: Option<String>,
        identity_file: Option<String>,
    },
    Mosh {
        host: String,
        #[serde(default = "default_ssh_port")]
        port: u16,
        user: Option<String>,
        mosh_port: Option<u16>,
    },
    Tailscale {
        host: String,
        user: Option<String>,
        identity_file: Option<String>,
    },
}

impl Default for Transport {
    fn default() -> Self {
        Transport::Local {}
    }
}

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
    #[serde(default)]
    pub transport_type: String,
    #[serde(default)]
    pub transport_host: String,
    #[serde(default)]
    pub transport_port: String,
    #[serde(default)]
    pub transport_user: String,
}

impl EventInfo {
    pub fn to_transport(&self) -> Transport {
        match self.transport_type.as_str() {
            "ssh" => Transport::Ssh {
                host: self.transport_host.clone(),
                port: self.transport_port.parse().unwrap_or(22),
                user: non_empty(&self.transport_user),
                identity_file: None,
            },
            "mosh" => Transport::Mosh {
                host: self.transport_host.clone(),
                port: self.transport_port.parse().unwrap_or(22),
                user: non_empty(&self.transport_user),
                mosh_port: None,
            },
            "tailscale" => Transport::Tailscale {
                host: self.transport_host.clone(),
                user: non_empty(&self.transport_user),
                identity_file: None,
            },
            _ => Transport::Local {},
        }
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
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
    #[serde(default)]
    pub transport: Transport,
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
    pub notification_settings: NotificationSettings,
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
        // Sort sessions by last_event timestamp in descending order (newest first)
        // Empty timestamps are sorted to the end
        let mut sessions: Vec<SessionInfo> = self.sessions.values().cloned().collect();
        sessions.sort_by(|a, b| {
            match (a.last_event.is_empty(), b.last_event.is_empty()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater, // Empty goes to end
                (false, true) => std::cmp::Ordering::Less,    // Non-empty comes first
                (false, false) => b.last_event.cmp(&a.last_event), // Descending order
            }
        });

        DashboardData {
            sessions,
            events: self.recent_events.iter().cloned().collect(),
        }
    }

    /// Insert or update a session with the given status and waiting_for info
    pub fn upsert_session(
        &mut self,
        key: String,
        event: &EventInfo,
        status: SessionStatus,
        waiting_for: String,
    ) {
        let transport = event.to_transport();
        self.sessions
            .entry(key)
            .and_modify(|s| {
                s.status = status.clone();
                s.last_event = event.timestamp.clone();
                s.waiting_for = waiting_for.clone();
                if !event.tmux_pane.is_empty() {
                    s.tmux_pane = event.tmux_pane.clone();
                }
                if !matches!(transport, Transport::Local {}) {
                    s.transport = transport.clone();
                }
            })
            .or_insert_with(|| SessionInfo {
                project_name: event.project_name.clone(),
                project_dir: event.project_dir.clone(),
                status,
                last_event: event.timestamp.clone(),
                waiting_for,
                tmux_pane: event.tmux_pane.clone(),
                transport,
            });
    }
}

pub struct ManagedState(pub Arc<Mutex<AppState>>);

pub struct NotificationSinksState(
    pub Arc<Mutex<Vec<Box<dyn eocc_core::notifications::NotificationSink>>>>,
);

pub struct NotificationHistoryState(
    pub Arc<Mutex<eocc_core::notifications::history::NotificationHistory>>,
);
