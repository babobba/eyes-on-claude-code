use eocc_core::notifications::NotificationSettings;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

// Re-export all shared types from eocc-core so the rest of the crate
// can continue to `use crate::state::SessionInfo` etc. without changes.
pub use eocc_core::state::{
    CachedPaths, DashboardData, EventInfo, EventType, HookChannelResult, NotificationType,
    SessionInfo, SessionStatus, Settings, Transport,
};

#[derive(Default)]
pub struct AppState {
    pub sessions: HashMap<String, SessionInfo>,
    pub recent_events: VecDeque<EventInfo>,
    pub settings: Settings,
    pub cached_paths: CachedPaths,
    pub notification_settings: NotificationSettings,
    /// Tracks which notification channels the hook already dispatched successfully
    /// per session key. Cleared when the session transitions to a new status.
    pub hook_notified_channels: HashMap<String, Vec<HookChannelResult>>,
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

    /// Look up the transport for a session by project directory.
    /// Returns `Transport::Local` if the session is not found.
    pub fn get_transport(&self, project_dir: Option<&str>) -> Transport {
        if let Some(dir) = project_dir {
            if let Some(session) = self.sessions.get(dir) {
                return session.transport.clone();
            }
        }
        Transport::Local {}
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
        let old_status = self.sessions.get(&key).map(|s| s.status.clone());
        if old_status.as_ref() != Some(&status) {
            self.hook_notified_channels.remove(&key);
        }
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
