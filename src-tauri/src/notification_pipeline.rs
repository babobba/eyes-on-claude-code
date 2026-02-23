use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eocc_core::notifications::{self, NotificationSink};

use crate::state::{AppState, EventInfo, SessionStatus};

/// Result of processing events through the notification pipeline.
/// Contains data needed for I/O operations that should happen outside the state lock.
pub struct PipelineResult {
    pub pending_notifications: Vec<notifications::SessionNotification>,
    pub cooldown_seconds: Option<u64>,
}

/// Detect notification-worthy status transitions after events have been applied.
/// Must be called while holding the state lock, after `apply_events_to_state`.
pub fn detect_pending_notifications(
    old_statuses: &HashMap<String, SessionStatus>,
    state: &AppState,
    notification_sinks: &Arc<Mutex<Vec<Box<dyn NotificationSink>>>>,
) -> PipelineResult {
    let sinks_active = state.notification_settings.enabled
        && notification_sinks
            .lock()
            .map(|s| !s.is_empty())
            .unwrap_or(false);

    let pending = if sinks_active {
        notifications::detect_status_transitions(
            old_statuses,
            &state.sessions,
            &state.notification_settings,
        )
    } else {
        Vec::new()
    };

    PipelineResult {
        pending_notifications: pending,
        cooldown_seconds: state.notification_settings.cooldown_seconds,
    }
}

/// Capture old session statuses before applying events.
/// Returns a map of session key -> status that can be passed to `detect_pending_notifications`.
pub fn capture_old_statuses(state: &AppState) -> HashMap<String, SessionStatus> {
    state
        .sessions
        .iter()
        .map(|(k, v)| (k.clone(), v.status.clone()))
        .collect()
}

/// Dispatch pending notifications, respecting cooldown.
/// This performs blocking HTTP calls and should be called outside of any locks.
pub fn dispatch_notifications(
    pipeline: &PipelineResult,
    notification_sinks: &Arc<Mutex<Vec<Box<dyn NotificationSink>>>>,
    notification_history: &Arc<Mutex<notifications::history::NotificationHistory>>,
    last_notified: &mut HashMap<String, Instant>,
) {
    if pipeline.pending_notifications.is_empty() {
        return;
    }

    let Ok(sinks) = notification_sinks.lock() else {
        return;
    };

    let now = Instant::now();
    for notification in &pipeline.pending_notifications {
        if let Some(cd) = pipeline.cooldown_seconds {
            if cd > 0 {
                if let Some(last) = last_notified.get(&notification.session_id) {
                    if now.duration_since(*last).as_secs() < cd {
                        continue;
                    }
                }
            }
        }
        let record = notifications::dispatch(&sinks, notification);
        last_notified.insert(notification.session_id.clone(), Instant::now());
        if let Ok(mut history) = notification_history.lock() {
            history.push(record);
        }
    }
}

/// Convenience: capture old statuses, apply events, and detect pending notifications.
/// Performs the full in-lock pipeline. Returns the pipeline result for dispatching outside the lock.
pub fn apply_and_detect(
    state: &mut AppState,
    events: &[EventInfo],
    notification_sinks: &Arc<Mutex<Vec<Box<dyn NotificationSink>>>>,
) -> PipelineResult {
    let old_statuses = capture_old_statuses(state);
    crate::events::apply_events_to_state(state, events);
    detect_pending_notifications(&old_statuses, state, notification_sinks)
}
