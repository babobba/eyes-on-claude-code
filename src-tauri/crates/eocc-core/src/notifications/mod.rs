#[cfg(feature = "ntfy")]
pub mod ntfy;
#[cfg(feature = "webhook")]
pub mod webhook;

use crate::state::{SessionInfo, SessionStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
}

impl fmt::Display for NotificationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationPriority::Low => write!(f, "low"),
            NotificationPriority::Normal => write!(f, "normal"),
            NotificationPriority::High => write!(f, "high"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionNotification {
    pub project_name: String,
    pub project_dir: String,
    pub session_id: String,
    pub old_status: Option<SessionStatus>,
    pub new_status: SessionStatus,
    pub message: String,
    pub priority: NotificationPriority,
}

impl SessionNotification {
    pub fn title(&self) -> String {
        let emoji = self.new_status.emoji();
        format!("{} {} - {}", emoji, self.project_name, self.status_label())
    }

    pub fn body(&self) -> String {
        if self.message.is_empty() {
            self.status_label()
        } else {
            self.message.clone()
        }
    }

    fn status_label(&self) -> String {
        match self.new_status {
            SessionStatus::Active => "Active".to_string(),
            SessionStatus::WaitingPermission => "Waiting for permission".to_string(),
            SessionStatus::WaitingInput => "Waiting for input".to_string(),
            SessionStatus::Completed => "Completed".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChannelConfig {
    Ntfy {
        server: String,
        topic: String,
        #[serde(default)]
        token: Option<String>,
    },
    Webhook {
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
    #[serde(default = "NotificationSettings::default_notify_on")]
    pub notify_on: Vec<SessionStatus>,
}

impl NotificationSettings {
    fn default_notify_on() -> Vec<SessionStatus> {
        vec![
            SessionStatus::WaitingPermission,
            SessionStatus::WaitingInput,
            SessionStatus::Completed,
        ]
    }
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            channels: Vec::new(),
            notify_on: Self::default_notify_on(),
        }
    }
}

pub trait NotificationSink: Send + Sync {
    fn name(&self) -> &str;
    fn send(&self, notification: &SessionNotification) -> Result<(), String>;
}

/// Load notification settings from a TOML file.
/// Returns default settings if the file doesn't exist or can't be parsed.
pub fn load_settings_from_file(path: &Path) -> NotificationSettings {
    if path.exists() {
        match fs::read_to_string(path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(settings) => return settings,
                Err(e) => {
                    log::error!(target: "eocc.settings", "Failed to parse notification settings TOML: {:?}", e)
                }
            },
            Err(e) => {
                log::error!(target: "eocc.settings", "Failed to read notification settings file: {:?}", e)
            }
        }
    }
    NotificationSettings::default()
}

/// Save notification settings to a TOML file.
/// Creates parent directories if needed.
pub fn save_settings_to_file(path: &Path, settings: &NotificationSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings directory: {:?}", e))?;
    }
    let content =
        toml::to_string_pretty(settings).map_err(|e| format!("Failed to serialize settings: {:?}", e))?;
    fs::write(path, content).map_err(|e| format!("Failed to write settings file: {:?}", e))?;
    Ok(())
}

/// Compare old session statuses to new state and produce notifications for transitions
/// into any of the `notify_on` statuses.
pub fn detect_status_transitions(
    old_statuses: &HashMap<String, SessionStatus>,
    new_sessions: &HashMap<String, SessionInfo>,
    notify_on: &[SessionStatus],
) -> Vec<SessionNotification> {
    let mut notifications = Vec::new();

    for (key, session) in new_sessions {
        if !notify_on.contains(&session.status) {
            continue;
        }

        let old_status = old_statuses.get(key);
        let changed = match old_status {
            None => true,
            Some(old) => *old != session.status,
        };

        if changed {
            let priority = match session.status {
                SessionStatus::WaitingPermission | SessionStatus::WaitingInput => {
                    NotificationPriority::High
                }
                SessionStatus::Completed => NotificationPriority::Normal,
                SessionStatus::Active => NotificationPriority::Low,
            };

            notifications.push(SessionNotification {
                project_name: session.project_name.clone(),
                project_dir: session.project_dir.clone(),
                session_id: key.clone(),
                old_status: old_status.cloned(),
                new_status: session.status.clone(),
                message: session.waiting_for.clone(),
                priority,
            });
        }
    }

    notifications
}

/// Build notification sinks from channel configs.
/// Returns sinks for enabled channels, skipping unsupported ones.
pub fn build_sinks(channels: &[ChannelConfig]) -> Vec<Box<dyn NotificationSink>> {
    let sinks: Vec<Box<dyn NotificationSink>> = channels
        .iter()
        .filter_map(|channel| -> Option<Box<dyn NotificationSink>> {
            match channel {
                #[cfg(feature = "ntfy")]
                ChannelConfig::Ntfy {
                    server,
                    topic,
                    token,
                } => Some(Box::new(ntfy::NtfySink::new(
                    server.clone(),
                    topic.clone(),
                    token.clone(),
                ))),
                #[cfg(feature = "webhook")]
                ChannelConfig::Webhook { url } => {
                    Some(Box::new(webhook::WebhookSink::new(url.clone())))
                }
                #[allow(unreachable_patterns)]
                _ => {
                    log::warn!(
                        "Notification channel configured but feature not enabled: {:?}",
                        channel
                    );
                    None
                }
            }
        })
        .collect();

    sinks
}

/// Send a notification to all provided sinks. Errors are logged but not propagated.
pub fn dispatch(sinks: &[Box<dyn NotificationSink>], notification: &SessionNotification) {
    for sink in sinks {
        if let Err(e) = sink.send(notification) {
            log::error!("Failed to send notification via {}: {}", sink.name(), e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(name: &str, status: SessionStatus) -> SessionInfo {
        SessionInfo {
            project_name: name.to_string(),
            project_dir: format!("/home/user/{}", name),
            status,
            last_event: "2025-01-01T00:00:00Z".to_string(),
            waiting_for: String::new(),
            tmux_pane: String::new(),
        }
    }

    #[test]
    fn detect_transitions_new_session_in_notify_list() {
        let old = HashMap::new();
        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/home/user/proj".to_string(),
            make_session("proj", SessionStatus::WaitingPermission),
        );

        let notifications =
            detect_status_transitions(&old, &new_sessions, &[SessionStatus::WaitingPermission]);
        assert_eq!(notifications.len(), 1);
        assert_eq!(
            notifications[0].new_status,
            SessionStatus::WaitingPermission
        );
        assert_eq!(notifications[0].project_name, "proj");
    }

    #[test]
    fn detect_transitions_status_change_triggers_notification() {
        let mut old = HashMap::new();
        old.insert("/home/user/proj".to_string(), SessionStatus::Active);

        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/home/user/proj".to_string(),
            make_session("proj", SessionStatus::WaitingPermission),
        );

        let notifications =
            detect_status_transitions(&old, &new_sessions, &[SessionStatus::WaitingPermission]);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].old_status, Some(SessionStatus::Active));
    }

    #[test]
    fn detect_transitions_no_change_no_notification() {
        let mut old = HashMap::new();
        old.insert(
            "/home/user/proj".to_string(),
            SessionStatus::WaitingPermission,
        );

        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/home/user/proj".to_string(),
            make_session("proj", SessionStatus::WaitingPermission),
        );

        let notifications =
            detect_status_transitions(&old, &new_sessions, &[SessionStatus::WaitingPermission]);
        assert_eq!(notifications.len(), 0);
    }

    #[test]
    fn detect_transitions_ignores_statuses_not_in_notify_on() {
        let old = HashMap::new();
        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/home/user/proj".to_string(),
            make_session("proj", SessionStatus::Active),
        );

        let notifications = detect_status_transitions(
            &old,
            &new_sessions,
            &[SessionStatus::WaitingPermission, SessionStatus::Completed],
        );
        assert_eq!(notifications.len(), 0);
    }

    #[test]
    fn detect_transitions_multiple_sessions() {
        let mut old = HashMap::new();
        old.insert("/home/user/a".to_string(), SessionStatus::Active);
        old.insert("/home/user/b".to_string(), SessionStatus::Active);

        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/home/user/a".to_string(),
            make_session("a", SessionStatus::WaitingPermission),
        );
        new_sessions.insert(
            "/home/user/b".to_string(),
            make_session("b", SessionStatus::Completed),
        );

        let notifications = detect_status_transitions(
            &old,
            &new_sessions,
            &[SessionStatus::WaitingPermission, SessionStatus::Completed],
        );
        assert_eq!(notifications.len(), 2);
    }

    #[test]
    fn notification_title_format() {
        let n = SessionNotification {
            project_name: "my-project".to_string(),
            project_dir: "/home/user/my-project".to_string(),
            session_id: "s1".to_string(),
            old_status: Some(SessionStatus::Active),
            new_status: SessionStatus::WaitingPermission,
            message: "Approve bash command".to_string(),
            priority: NotificationPriority::High,
        };
        assert_eq!(n.title(), "🔐 my-project - Waiting for permission");
        assert_eq!(n.body(), "Approve bash command");
    }

    #[test]
    fn notification_body_uses_status_when_message_empty() {
        let n = SessionNotification {
            project_name: "proj".to_string(),
            project_dir: "/proj".to_string(),
            session_id: "s1".to_string(),
            old_status: None,
            new_status: SessionStatus::Completed,
            message: String::new(),
            priority: NotificationPriority::Normal,
        };
        assert_eq!(n.body(), "Completed");
    }

    #[test]
    fn notification_settings_default() {
        let settings = NotificationSettings::default();
        assert!(!settings.enabled);
        assert!(settings.channels.is_empty());
        assert_eq!(settings.notify_on.len(), 3);
    }

    #[test]
    fn notification_settings_deserialize_with_defaults() {
        let json = r#"{"enabled": true, "channels": []}"#;
        let settings: NotificationSettings = serde_json::from_str(json).unwrap();
        assert!(settings.enabled);
        assert_eq!(settings.notify_on.len(), 3);
    }

    #[test]
    fn notification_settings_toml_roundtrip() {
        let settings = NotificationSettings {
            enabled: true,
            channels: vec![
                ChannelConfig::Ntfy {
                    server: "https://ntfy.sh".to_string(),
                    topic: "eocc-test".to_string(),
                    token: Some("secret".to_string()),
                },
                ChannelConfig::Webhook {
                    url: "https://hooks.slack.com/xxx".to_string(),
                },
            ],
            notify_on: vec![
                SessionStatus::WaitingPermission,
                SessionStatus::Completed,
            ],
        };
        let toml_str = toml::to_string_pretty(&settings).unwrap();
        let parsed: NotificationSettings = toml::from_str(&toml_str).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.channels.len(), 2);
        assert_eq!(parsed.notify_on.len(), 2);
        match &parsed.channels[0] {
            ChannelConfig::Ntfy { server, topic, token } => {
                assert_eq!(server, "https://ntfy.sh");
                assert_eq!(topic, "eocc-test");
                assert_eq!(token.as_deref(), Some("secret"));
            }
            _ => panic!("Expected Ntfy variant"),
        }
        match &parsed.channels[1] {
            ChannelConfig::Webhook { url } => {
                assert_eq!(url, "https://hooks.slack.com/xxx");
            }
            _ => panic!("Expected Webhook variant"),
        }
    }

    #[test]
    fn notification_settings_toml_deserialize_minimal() {
        let toml_str = r#"enabled = true
channels = []
"#;
        let settings: NotificationSettings = toml::from_str(toml_str).unwrap();
        assert!(settings.enabled);
        assert!(settings.channels.is_empty());
        assert_eq!(settings.notify_on.len(), 3);
    }

    #[test]
    fn load_settings_from_file_nonexistent() {
        let settings = load_settings_from_file(Path::new("/tmp/nonexistent_eocc_test.toml"));
        assert!(!settings.enabled);
        assert!(settings.channels.is_empty());
    }

    #[test]
    fn save_and_load_settings_file() {
        let dir = std::env::temp_dir().join("eocc_test_settings");
        let path = dir.join("notification_settings.toml");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);

        let settings = NotificationSettings {
            enabled: true,
            channels: vec![ChannelConfig::Webhook {
                url: "https://example.com/hook".to_string(),
            }],
            notify_on: vec![SessionStatus::Completed],
        };

        save_settings_to_file(&path, &settings).unwrap();
        let loaded = load_settings_from_file(&path);
        assert!(loaded.enabled);
        assert_eq!(loaded.channels.len(), 1);
        assert_eq!(loaded.notify_on.len(), 1);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn channel_config_ntfy_roundtrip() {
        let config = ChannelConfig::Ntfy {
            server: "https://ntfy.sh".to_string(),
            topic: "eocc-test".to_string(),
            token: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ChannelConfig = serde_json::from_str(&json).unwrap();
        match parsed {
            ChannelConfig::Ntfy {
                server,
                topic,
                token,
            } => {
                assert_eq!(server, "https://ntfy.sh");
                assert_eq!(topic, "eocc-test");
                assert!(token.is_none());
            }
            _ => panic!("Expected Ntfy variant"),
        }
    }

    #[test]
    fn channel_config_webhook_roundtrip() {
        let config = ChannelConfig::Webhook {
            url: "https://hooks.slack.com/xxx".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ChannelConfig = serde_json::from_str(&json).unwrap();
        match parsed {
            ChannelConfig::Webhook { url } => {
                assert_eq!(url, "https://hooks.slack.com/xxx");
            }
            _ => panic!("Expected Webhook variant"),
        }
    }

    #[test]
    fn priority_display() {
        assert_eq!(NotificationPriority::Low.to_string(), "low");
        assert_eq!(NotificationPriority::Normal.to_string(), "normal");
        assert_eq!(NotificationPriority::High.to_string(), "high");
    }

    #[test]
    fn build_sinks_empty_channels() {
        let sinks = build_sinks(&[]);
        assert!(sinks.is_empty());
    }

    #[test]
    fn detect_transitions_high_priority_for_waiting() {
        let old = HashMap::new();
        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/proj".to_string(),
            make_session("proj", SessionStatus::WaitingInput),
        );

        let notifications =
            detect_status_transitions(&old, &new_sessions, &[SessionStatus::WaitingInput]);
        assert_eq!(notifications[0].priority, NotificationPriority::High);
    }

    #[test]
    fn detect_transitions_normal_priority_for_completed() {
        let mut old = HashMap::new();
        old.insert("/proj".to_string(), SessionStatus::Active);

        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/proj".to_string(),
            make_session("proj", SessionStatus::Completed),
        );

        let notifications =
            detect_status_transitions(&old, &new_sessions, &[SessionStatus::Completed]);
        assert_eq!(notifications[0].priority, NotificationPriority::Normal);
    }
}
