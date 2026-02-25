use crate::events::apply_events_to_state;
use crate::notifications::{
    build_sinks, detect_status_transitions, dispatch, history::NotificationHistory,
    load_settings_from_file, NotificationSink,
};
use crate::state::{AppState, SessionStatus};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Configuration for the headless watcher.
pub struct WatcherConfig {
    pub eocc_dir: PathBuf,
    pub poll_interval: Duration,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Self {
            eocc_dir: PathBuf::from(home).join(".eocc"),
            poll_interval: Duration::from_secs(1),
        }
    }
}

/// Run the headless notification watcher. Blocks the current thread.
/// Watches `events.jsonl` for new events and dispatches notifications.
/// Call `stop_tx.send(())` to stop the watcher.
pub fn run(config: WatcherConfig, stop_rx: mpsc::Receiver<()>) {
    let log_dir = config.eocc_dir.join("logs");
    let events_path = log_dir.join("events.jsonl");
    let processing_path = log_dir.join("events.processing.jsonl");
    let settings_path = config.eocc_dir.join("notification_settings.toml");

    let mut state = AppState::default();
    let mut history = NotificationHistory::default();
    let mut last_notified: HashMap<String, Instant> = HashMap::new();

    // Load notification settings
    let mut notification_settings = load_settings_from_file(&settings_path);
    let mut sinks: Vec<Box<dyn NotificationSink>> = build_sinks(&notification_settings.channels);
    let mut settings_modified = file_modified_time(&settings_path);

    log::info!(
        "Headless watcher started: {} channel(s), enabled={}",
        sinks.len(),
        notification_settings.enabled
    );

    loop {
        // Check for stop signal
        if stop_rx.try_recv().is_ok() {
            log::info!("Headless watcher stopping");
            break;
        }

        // Hot-reload settings if changed
        let new_modified = file_modified_time(&settings_path);
        if new_modified != settings_modified {
            settings_modified = new_modified;
            notification_settings = load_settings_from_file(&settings_path);
            sinks = build_sinks(&notification_settings.channels);
            log::info!(
                "Settings reloaded: {} channel(s), enabled={}",
                sinks.len(),
                notification_settings.enabled
            );
        }

        // Read and process events
        let new_events = read_events_queue(&events_path, &processing_path);
        if !new_events.is_empty() {
            let old_statuses: HashMap<String, SessionStatus> = state
                .sessions
                .iter()
                .map(|(k, v)| (k.clone(), v.status.clone()))
                .collect();

            apply_events_to_state(&mut state, &new_events);

            if notification_settings.enabled && !sinks.is_empty() {
                let notifications = detect_status_transitions(
                    &old_statuses,
                    &state.sessions,
                    &notification_settings,
                );

                let now = Instant::now();
                for notification in &notifications {
                    if let Some(cd) = notification_settings.cooldown_seconds {
                        if cd > 0 {
                            if let Some(last) = last_notified.get(&notification.session_id) {
                                if now.duration_since(*last).as_secs() < cd {
                                    continue;
                                }
                            }
                        }
                    }
                    let record = dispatch(&sinks, notification);
                    last_notified.insert(notification.session_id.clone(), Instant::now());
                    history.push(record);
                }
            }
        }

        std::thread::sleep(config.poll_interval);
    }
}

fn file_modified_time(path: &Path) -> Option<std::time::SystemTime> {
    fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

fn read_events_queue(events_path: &Path, processing_path: &Path) -> Vec<crate::state::EventInfo> {
    if !events_path.exists() {
        return Vec::new();
    }

    // Atomically rename events.jsonl to events.processing.jsonl
    if fs::rename(events_path, processing_path).is_err() {
        return Vec::new();
    }

    let content = match fs::read_to_string(processing_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let _ = fs::remove_file(processing_path);

    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = WatcherConfig::default();
        assert!(config.eocc_dir.to_str().unwrap().ends_with(".eocc"));
        assert_eq!(config.poll_interval, Duration::from_secs(1));
    }

    #[test]
    fn read_events_queue_nonexistent() {
        let events = read_events_queue(
            Path::new("/tmp/nonexistent_eocc_events.jsonl"),
            Path::new("/tmp/nonexistent_eocc_processing.jsonl"),
        );
        assert!(events.is_empty());
    }

    #[test]
    fn read_events_queue_with_data() {
        let dir = std::env::temp_dir().join("eocc_watcher_test");
        let _ = fs::create_dir_all(&dir);
        let events_path = dir.join("events.jsonl");
        let processing_path = dir.join("events.processing.jsonl");

        let event_json = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"session_start","matcher":"hook","project_name":"test","project_dir":"/test","session_id":"s1","message":"","notification_type":"other","tool_name":"","tmux_pane":"","tmux_path":""}"#;
        fs::write(&events_path, format!("{}\n", event_json)).unwrap();

        let events = read_events_queue(&events_path, &processing_path);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].project_name, "test");
        assert!(!events_path.exists());
        assert!(!processing_path.exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
