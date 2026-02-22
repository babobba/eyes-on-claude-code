#[cfg(feature = "desktop_notifications")]
pub mod desktop;
pub mod history;
#[cfg(feature = "ntfy")]
pub mod ntfy;
#[cfg(feature = "pushover")]
pub mod pushover;
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
    #[serde(skip)]
    pub title_template: Option<String>,
    #[serde(skip)]
    pub body_template: Option<String>,
}

impl SessionNotification {
    pub fn title(&self) -> String {
        if let Some(ref t) = self.title_template {
            return self.apply_template(t);
        }
        let emoji = self.new_status.emoji();
        format!("{} {} - {}", emoji, self.project_name, self.status_label())
    }

    pub fn body(&self) -> String {
        if let Some(ref t) = self.body_template {
            return self.apply_template(t);
        }
        if self.message.is_empty() {
            self.status_label()
        } else {
            self.message.clone()
        }
    }

    fn apply_template(&self, template: &str) -> String {
        template
            .replace("{project_name}", &self.project_name)
            .replace("{project_dir}", &self.project_dir)
            .replace("{status}", &self.status_label())
            .replace("{emoji}", self.new_status.emoji())
            .replace("{message}", &self.message)
            .replace("{priority}", &self.priority.to_string())
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
    Pushover {
        user_key: String,
        app_token: String,
        #[serde(default)]
        device: Option<String>,
    },
    Desktop {},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRule {
    pub pattern: String,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub notify_on: Option<Vec<SessionStatus>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
    #[serde(default = "NotificationSettings::default_notify_on")]
    pub notify_on: Vec<SessionStatus>,
    #[serde(default)]
    pub project_rules: Vec<ProjectRule>,
    #[serde(default)]
    pub cooldown_seconds: Option<u64>,
    #[serde(default)]
    pub title_template: Option<String>,
    #[serde(default)]
    pub body_template: Option<String>,
}

impl NotificationSettings {
    fn default_notify_on() -> Vec<SessionStatus> {
        vec![
            SessionStatus::WaitingPermission,
            SessionStatus::WaitingInput,
            SessionStatus::Completed,
        ]
    }

    /// Resolve the effective `enabled` and `notify_on` for a given project directory.
    /// Project rules are checked in order; the first matching rule wins.
    pub fn resolve_for_project(&self, project_dir: &str) -> (bool, &[SessionStatus]) {
        for rule in &self.project_rules {
            if pattern_matches(&rule.pattern, project_dir) {
                let enabled = rule.enabled.unwrap_or(self.enabled);
                let notify_on = rule.notify_on.as_deref().unwrap_or(&self.notify_on);
                return (enabled, notify_on);
            }
        }
        (self.enabled, &self.notify_on)
    }
}

/// Simple glob-like pattern matching for project rules.
/// Supports `*` (any chars within a segment) and `**` (any path segments).
fn pattern_matches(pattern: &str, path: &str) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    // Exact match
    if pattern == path {
        return true;
    }
    // Ends-with match: pattern starts with `**/`
    if let Some(suffix) = pattern.strip_prefix("**/") {
        return path.ends_with(suffix) || path.contains(&format!("/{}", suffix));
    }
    // Contains match: pattern starts with `*` and ends with `*`
    if pattern.starts_with('*') && pattern.ends_with('*') && pattern.len() > 2 {
        let inner = &pattern[1..pattern.len() - 1];
        return path.contains(inner);
    }
    // Prefix match
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix);
    }
    // Simple suffix match with single `*`
    if let Some(prefix) = pattern.strip_suffix('*') {
        return path.starts_with(prefix);
    }
    false
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            channels: Vec::new(),
            notify_on: Self::default_notify_on(),
            project_rules: Vec::new(),
            cooldown_seconds: None,
            title_template: None,
            body_template: None,
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
    let content = toml::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {:?}", e))?;
    fs::write(path, content).map_err(|e| format!("Failed to write settings file: {:?}", e))?;
    Ok(())
}

/// Compare old session statuses to new state and produce notifications for transitions
/// into any of the `notify_on` statuses. Uses per-project rules when available.
pub fn detect_status_transitions(
    old_statuses: &HashMap<String, SessionStatus>,
    new_sessions: &HashMap<String, SessionInfo>,
    settings: &NotificationSettings,
) -> Vec<SessionNotification> {
    let mut notifications = Vec::new();

    for (key, session) in new_sessions {
        let (project_enabled, notify_on) = settings.resolve_for_project(&session.project_dir);

        if !project_enabled || !notify_on.contains(&session.status) {
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
                title_template: settings.title_template.clone(),
                body_template: settings.body_template.clone(),
            });
        }
    }

    notifications
}

/// Build notification sinks from channel configs.
/// Returns sinks for enabled channels, skipping unsupported ones.
pub fn build_sinks(channels: &[ChannelConfig]) -> Vec<Box<dyn NotificationSink>> {
    channels
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
                #[cfg(feature = "pushover")]
                ChannelConfig::Pushover {
                    user_key,
                    app_token,
                    device,
                } => Some(Box::new(pushover::PushoverSink::new(
                    user_key.clone(),
                    app_token.clone(),
                    device.clone(),
                ))),
                #[cfg(feature = "desktop_notifications")]
                ChannelConfig::Desktop {} => Some(Box::new(desktop::DesktopSink::new())),
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
        .collect()
}

/// Send a notification to all provided sinks. Returns a history record.
pub fn dispatch(
    sinks: &[Box<dyn NotificationSink>],
    notification: &SessionNotification,
) -> history::NotificationRecord {
    let mut channel_results = Vec::new();

    for sink in sinks {
        match sink.send(notification) {
            Ok(()) => {
                channel_results.push(history::ChannelResult {
                    name: sink.name().to_string(),
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                log::error!("Failed to send notification via {}: {}", sink.name(), e);
                channel_results.push(history::ChannelResult {
                    name: sink.name().to_string(),
                    success: false,
                    error: Some(e),
                });
            }
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs();
            let nanos = d.subsec_nanos();
            format!(
                "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
                1970 + secs / 31_557_600,
                (secs % 31_557_600) / 2_629_800 + 1,
                (secs % 2_629_800) / 86400 + 1,
                (secs % 86400) / 3600,
                (secs % 3600) / 60,
                secs % 60,
                nanos / 1_000_000,
            )
        })
        .unwrap_or_default();

    history::NotificationRecord {
        timestamp: now,
        project_name: notification.project_name.clone(),
        project_dir: notification.project_dir.clone(),
        status: format!("{:?}", notification.new_status),
        channels: channel_results,
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

    fn make_settings(notify_on: Vec<SessionStatus>) -> NotificationSettings {
        NotificationSettings {
            enabled: true,
            channels: Vec::new(),
            notify_on,
            project_rules: Vec::new(),
            cooldown_seconds: None,
            title_template: None,
            body_template: None,
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

        let settings = make_settings(vec![SessionStatus::WaitingPermission]);
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
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

        let settings = make_settings(vec![SessionStatus::WaitingPermission]);
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
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

        let settings = make_settings(vec![SessionStatus::WaitingPermission]);
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
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

        let settings = make_settings(vec![
            SessionStatus::WaitingPermission,
            SessionStatus::Completed,
        ]);
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
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

        let settings = make_settings(vec![
            SessionStatus::WaitingPermission,
            SessionStatus::Completed,
        ]);
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
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
            title_template: None,
            body_template: None,
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
            title_template: None,
            body_template: None,
        };
        assert_eq!(n.body(), "Completed");
    }

    #[test]
    fn notification_settings_default() {
        let settings = NotificationSettings::default();
        assert!(!settings.enabled);
        assert!(settings.channels.is_empty());
        assert_eq!(settings.notify_on.len(), 3);
        assert!(settings.project_rules.is_empty());
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
            notify_on: vec![SessionStatus::WaitingPermission, SessionStatus::Completed],
            project_rules: Vec::new(),
            cooldown_seconds: None,
            title_template: None,
            body_template: None,
        };
        let toml_str = toml::to_string_pretty(&settings).unwrap();
        let parsed: NotificationSettings = toml::from_str(&toml_str).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.channels.len(), 2);
        assert_eq!(parsed.notify_on.len(), 2);
        match &parsed.channels[0] {
            ChannelConfig::Ntfy {
                server,
                topic,
                token,
            } => {
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
            project_rules: Vec::new(),
            cooldown_seconds: None,
            title_template: None,
            body_template: None,
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

        let settings = make_settings(vec![SessionStatus::WaitingInput]);
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
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

        let settings = make_settings(vec![SessionStatus::Completed]);
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
        assert_eq!(notifications[0].priority, NotificationPriority::Normal);
    }

    #[test]
    fn project_rule_disables_notifications() {
        let old = HashMap::new();
        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/home/user/noisy".to_string(),
            make_session("noisy", SessionStatus::WaitingPermission),
        );

        let settings = NotificationSettings {
            enabled: true,
            channels: Vec::new(),
            notify_on: vec![SessionStatus::WaitingPermission],
            project_rules: vec![ProjectRule {
                pattern: "**/noisy".to_string(),
                enabled: Some(false),
                notify_on: None,
            }],
            cooldown_seconds: None,
            title_template: None,
            body_template: None,
        };
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
        assert_eq!(notifications.len(), 0);
    }

    #[test]
    fn project_rule_overrides_notify_on() {
        let old = HashMap::new();
        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/home/user/important".to_string(),
            make_session("important", SessionStatus::Active),
        );

        let settings = NotificationSettings {
            enabled: true,
            channels: Vec::new(),
            notify_on: vec![SessionStatus::WaitingPermission],
            project_rules: vec![ProjectRule {
                pattern: "**/important".to_string(),
                enabled: None,
                notify_on: Some(vec![SessionStatus::Active]),
            }],
            cooldown_seconds: None,
            title_template: None,
            body_template: None,
        };
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
        assert_eq!(notifications.len(), 1);
    }

    #[test]
    fn pattern_matching() {
        assert!(pattern_matches("**/proj", "/home/user/proj"));
        assert!(pattern_matches("**/proj", "/other/path/proj"));
        assert!(!pattern_matches("**/proj", "/home/user/other"));
        assert!(pattern_matches("/home/user/**", "/home/user/anything/here"));
        assert!(pattern_matches("/home/user/proj*", "/home/user/proj-extra"));
        assert!(pattern_matches("/home/user/proj", "/home/user/proj"));
        assert!(!pattern_matches("/home/user/proj", "/home/user/other"));
    }

    #[test]
    fn channel_config_pushover_roundtrip() {
        let config = ChannelConfig::Pushover {
            user_key: "ukey123".to_string(),
            app_token: "atok456".to_string(),
            device: Some("phone".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ChannelConfig = serde_json::from_str(&json).unwrap();
        match parsed {
            ChannelConfig::Pushover {
                user_key,
                app_token,
                device,
            } => {
                assert_eq!(user_key, "ukey123");
                assert_eq!(app_token, "atok456");
                assert_eq!(device.as_deref(), Some("phone"));
            }
            _ => panic!("Expected Pushover variant"),
        }
    }

    #[test]
    fn channel_config_desktop_roundtrip() {
        let config = ChannelConfig::Desktop {};
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ChannelConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, ChannelConfig::Desktop {}));
    }

    #[test]
    fn toml_roundtrip_with_all_channel_types() {
        let settings = NotificationSettings {
            enabled: true,
            channels: vec![
                ChannelConfig::Ntfy {
                    server: "https://ntfy.sh".to_string(),
                    topic: "test".to_string(),
                    token: None,
                },
                ChannelConfig::Webhook {
                    url: "https://example.com".to_string(),
                },
                ChannelConfig::Pushover {
                    user_key: "u".to_string(),
                    app_token: "t".to_string(),
                    device: None,
                },
                ChannelConfig::Desktop {},
            ],
            notify_on: vec![SessionStatus::Completed],
            project_rules: vec![ProjectRule {
                pattern: "**/important".to_string(),
                enabled: Some(true),
                notify_on: Some(vec![SessionStatus::Active, SessionStatus::Completed]),
            }],
            cooldown_seconds: None,
            title_template: None,
            body_template: None,
        };
        let toml_str = toml::to_string_pretty(&settings).unwrap();
        let parsed: NotificationSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.channels.len(), 4);
        assert_eq!(parsed.project_rules.len(), 1);
        assert_eq!(parsed.project_rules[0].pattern, "**/important");
    }

    #[test]
    fn dispatch_returns_history_record() {
        let sinks: Vec<Box<dyn NotificationSink>> = vec![];
        let notification = SessionNotification {
            project_name: "proj".to_string(),
            project_dir: "/proj".to_string(),
            session_id: "s1".to_string(),
            old_status: None,
            new_status: SessionStatus::Completed,
            message: String::new(),
            priority: NotificationPriority::Normal,
            title_template: None,
            body_template: None,
        };
        let record = dispatch(&sinks, &notification);
        assert_eq!(record.project_name, "proj");
        assert!(record.channels.is_empty());
    }

    #[test]
    fn title_template_overrides_default() {
        let n = SessionNotification {
            project_name: "my-proj".to_string(),
            project_dir: "/home/user/my-proj".to_string(),
            session_id: "s1".to_string(),
            old_status: None,
            new_status: SessionStatus::Completed,
            message: "done".to_string(),
            priority: NotificationPriority::Normal,
            title_template: Some("{project_name} is now {status}".to_string()),
            body_template: None,
        };
        assert_eq!(n.title(), "my-proj is now Completed");
        assert_eq!(n.body(), "done");
    }

    #[test]
    fn body_template_overrides_default() {
        let n = SessionNotification {
            project_name: "proj".to_string(),
            project_dir: "/proj".to_string(),
            session_id: "s1".to_string(),
            old_status: None,
            new_status: SessionStatus::WaitingPermission,
            message: "approve bash".to_string(),
            priority: NotificationPriority::High,
            title_template: None,
            body_template: Some("{emoji} {message} ({priority})".to_string()),
        };
        assert_eq!(n.title(), "🔐 proj - Waiting for permission");
        assert_eq!(n.body(), "🔐 approve bash (high)");
    }

    #[test]
    fn templates_propagated_from_detect_transitions() {
        let old = HashMap::new();
        let mut new_sessions = HashMap::new();
        new_sessions.insert(
            "/proj".to_string(),
            make_session("proj", SessionStatus::Completed),
        );
        let settings = NotificationSettings {
            enabled: true,
            channels: Vec::new(),
            notify_on: vec![SessionStatus::Completed],
            project_rules: Vec::new(),
            cooldown_seconds: None,
            title_template: Some("{project_name}: {status}".to_string()),
            body_template: Some("{message}".to_string()),
        };
        let notifications = detect_status_transitions(&old, &new_sessions, &settings);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].title(), "proj: Completed");
    }

    #[test]
    fn cooldown_and_template_settings_roundtrip() {
        let settings = NotificationSettings {
            enabled: true,
            channels: Vec::new(),
            notify_on: vec![SessionStatus::Completed],
            project_rules: Vec::new(),
            cooldown_seconds: Some(30),
            title_template: Some("{emoji} {project_name}".to_string()),
            body_template: Some("{status}: {message}".to_string()),
        };
        let toml_str = toml::to_string_pretty(&settings).unwrap();
        let parsed: NotificationSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.cooldown_seconds, Some(30));
        assert_eq!(
            parsed.title_template.as_deref(),
            Some("{emoji} {project_name}")
        );
        assert_eq!(parsed.body_template.as_deref(), Some("{status}: {message}"));
    }
}
