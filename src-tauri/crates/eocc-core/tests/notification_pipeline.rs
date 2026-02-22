use eocc_core::events::apply_events_to_state;
use eocc_core::notifications::{
    build_sinks, detect_status_transitions, dispatch, history::NotificationHistory,
    load_settings_from_file, save_settings_to_file, ChannelConfig, NotificationSettings,
    ProjectRule,
};
use eocc_core::state::{AppState, EventInfo, EventType, NotificationType, SessionStatus};
use std::collections::HashMap;

fn make_event(
    event_type: EventType,
    project_name: &str,
    project_dir: &str,
    session_id: &str,
) -> EventInfo {
    EventInfo {
        timestamp: "2025-01-01T00:00:00Z".to_string(),
        event_type,
        matcher: "hook".to_string(),
        project_name: project_name.to_string(),
        project_dir: project_dir.to_string(),
        session_id: session_id.to_string(),
        message: String::new(),
        notification_type: NotificationType::Other,
        tool_name: String::new(),
        tmux_pane: String::new(),
        npx_path: String::new(),
        tmux_path: String::new(),
        transport_type: String::new(),
        transport_host: String::new(),
        transport_port: String::new(),
        transport_user: String::new(),
    }
}

fn make_notification_event(
    project_name: &str,
    project_dir: &str,
    session_id: &str,
    notification_type: NotificationType,
    message: &str,
) -> EventInfo {
    EventInfo {
        timestamp: "2025-01-01T00:01:00Z".to_string(),
        event_type: EventType::Notification,
        matcher: "hook".to_string(),
        project_name: project_name.to_string(),
        project_dir: project_dir.to_string(),
        session_id: session_id.to_string(),
        message: message.to_string(),
        notification_type,
        tool_name: String::new(),
        tmux_pane: String::new(),
        npx_path: String::new(),
        tmux_path: String::new(),
        transport_type: String::new(),
        transport_host: String::new(),
        transport_port: String::new(),
        transport_user: String::new(),
    }
}

/// Full pipeline: session_start -> notification(permission) -> detect transitions -> dispatch
#[test]
fn full_pipeline_session_start_to_notification() {
    let mut state = AppState::default();
    let settings = NotificationSettings {
        enabled: true,
        channels: Vec::new(),
        notify_on: vec![SessionStatus::WaitingPermission, SessionStatus::Completed],
        project_rules: Vec::new(),
        cooldown_seconds: None,
        title_template: None,
        body_template: None,
        api_port: None,
        external_url: None,
    };
    let mut history = NotificationHistory::default();

    // Step 1: session_start event
    let events = vec![make_event(
        EventType::SessionStart,
        "my-project",
        "/home/user/my-project",
        "session-1",
    )];
    let old_statuses: HashMap<String, SessionStatus> = state
        .sessions
        .iter()
        .map(|(k, v)| (k.clone(), v.status.clone()))
        .collect();
    apply_events_to_state(&mut state, &events);

    // After session_start, session is Active
    assert_eq!(state.sessions.len(), 1);
    let session = state.sessions.values().next().unwrap();
    assert_eq!(session.status, SessionStatus::Active);

    // No notifications for Active (not in notify_on)
    let notifications = detect_status_transitions(&old_statuses, &state.sessions, &settings);
    assert!(notifications.is_empty());

    // Step 2: notification(permission) event transitions to WaitingPermission
    let old_statuses: HashMap<String, SessionStatus> = state
        .sessions
        .iter()
        .map(|(k, v)| (k.clone(), v.status.clone()))
        .collect();

    let events2 = vec![make_notification_event(
        "my-project",
        "/home/user/my-project",
        "session-1",
        NotificationType::PermissionPrompt,
        "Approve bash command",
    )];
    apply_events_to_state(&mut state, &events2);

    assert_eq!(
        state.sessions.values().next().unwrap().status,
        SessionStatus::WaitingPermission
    );

    // Should produce a notification
    let notifications = detect_status_transitions(&old_statuses, &state.sessions, &settings);
    assert_eq!(notifications.len(), 1);
    assert_eq!(
        notifications[0].new_status,
        SessionStatus::WaitingPermission
    );
    assert_eq!(notifications[0].project_name, "my-project");

    // Dispatch and record in history
    let sinks = build_sinks(&[]);
    let record = dispatch(&sinks, &notifications[0]);
    history.push(record);

    assert_eq!(history.records().len(), 1);
    assert_eq!(history.records()[0].project_name, "my-project");
}

/// Pipeline with per-project rules: one project notifies, another is disabled
#[test]
fn pipeline_with_project_rules() {
    let mut state = AppState::default();
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
        api_port: None,
        external_url: None,
    };

    // Start two sessions
    let events = vec![
        make_event(
            EventType::SessionStart,
            "important",
            "/home/user/important",
            "s1",
        ),
        make_event(EventType::SessionStart, "noisy", "/home/user/noisy", "s2"),
    ];
    apply_events_to_state(&mut state, &events);

    // Transition both to WaitingPermission
    let old_statuses: HashMap<String, SessionStatus> = state
        .sessions
        .iter()
        .map(|(k, v)| (k.clone(), v.status.clone()))
        .collect();

    let events2 = vec![
        make_notification_event(
            "important",
            "/home/user/important",
            "s1",
            NotificationType::PermissionPrompt,
            "approve",
        ),
        make_notification_event(
            "noisy",
            "/home/user/noisy",
            "s2",
            NotificationType::PermissionPrompt,
            "approve",
        ),
    ];
    apply_events_to_state(&mut state, &events2);

    let notifications = detect_status_transitions(&old_statuses, &state.sessions, &settings);
    // Only "important" should generate a notification (noisy is disabled)
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].project_name, "important");
}

/// Pipeline with templates: verify custom title/body are used
#[test]
fn pipeline_with_templates() {
    let mut state = AppState::default();
    let settings = NotificationSettings {
        enabled: true,
        channels: Vec::new(),
        notify_on: vec![SessionStatus::WaitingPermission],
        project_rules: Vec::new(),
        cooldown_seconds: None,
        title_template: Some("{project_name} needs attention".to_string()),
        body_template: Some("{status}: {message}".to_string()),
        api_port: None,
        external_url: None,
    };

    let events = vec![make_event(EventType::SessionStart, "proj", "/proj", "s1")];
    apply_events_to_state(&mut state, &events);

    let old_statuses: HashMap<String, SessionStatus> = state
        .sessions
        .iter()
        .map(|(k, v)| (k.clone(), v.status.clone()))
        .collect();

    let events2 = vec![make_notification_event(
        "proj",
        "/proj",
        "s1",
        NotificationType::PermissionPrompt,
        "approve bash",
    )];
    apply_events_to_state(&mut state, &events2);

    let notifications = detect_status_transitions(&old_statuses, &state.sessions, &settings);
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].title(), "proj needs attention");
    assert_eq!(
        notifications[0].body(),
        "Waiting for permission: approve bash"
    );
}

/// Config roundtrip: save to TOML file, load it back, use for notifications
#[test]
fn config_load_and_use() {
    let dir = std::env::temp_dir().join("eocc_integration_test");
    let path = dir.join("notification_settings.toml");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);

    let settings = NotificationSettings {
        enabled: true,
        channels: vec![
            ChannelConfig::Webhook {
                url: "https://example.com/hook".to_string(),
            },
            ChannelConfig::Desktop {},
        ],
        notify_on: vec![SessionStatus::Completed, SessionStatus::WaitingPermission],
        project_rules: vec![ProjectRule {
            pattern: "**/test-proj".to_string(),
            enabled: Some(true),
            notify_on: Some(vec![SessionStatus::Active]),
        }],
        cooldown_seconds: Some(60),
        title_template: Some("{emoji} {project_name}".to_string()),
        body_template: None,
        api_port: None,
        external_url: None,
    };

    save_settings_to_file(&path, &settings).unwrap();
    let loaded = load_settings_from_file(&path);

    assert!(loaded.enabled);
    assert_eq!(loaded.channels.len(), 2);
    assert_eq!(loaded.notify_on.len(), 2);
    assert_eq!(loaded.project_rules.len(), 1);
    assert_eq!(loaded.cooldown_seconds, Some(60));
    assert_eq!(
        loaded.title_template.as_deref(),
        Some("{emoji} {project_name}")
    );
    assert!(loaded.body_template.is_none());

    // Verify the loaded settings work for transition detection
    let mut state = AppState::default();
    let events = vec![make_event(
        EventType::SessionStart,
        "test-proj",
        "/home/user/test-proj",
        "s1",
    )];
    apply_events_to_state(&mut state, &events);

    let old_statuses: HashMap<String, SessionStatus> = HashMap::new();
    let notifications = detect_status_transitions(&old_statuses, &state.sessions, &loaded);
    // test-proj rule overrides notify_on to [Active], and session starts Active
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].new_status, SessionStatus::Active);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

/// Dispatch with zero sinks produces an empty-channel history record
#[test]
fn dispatch_no_sinks_records_empty() {
    let sinks = build_sinks(&[]);
    let notification = eocc_core::notifications::SessionNotification {
        project_name: "proj".to_string(),
        project_dir: "/proj".to_string(),
        session_id: "s1".to_string(),
        old_status: None,
        new_status: SessionStatus::Completed,
        message: "done".to_string(),
        priority: eocc_core::notifications::NotificationPriority::Normal,
        title_template: None,
        body_template: None,
        click_url: None,
    };

    let record = dispatch(&sinks, &notification);
    assert_eq!(record.project_name, "proj");
    assert!(record.channels.is_empty());
    assert!(!record.timestamp.is_empty());
}

/// Multiple session lifecycle: start -> wait -> complete with history tracking
#[test]
fn multi_session_lifecycle_with_history() {
    let mut state = AppState::default();
    let settings = NotificationSettings {
        enabled: true,
        channels: Vec::new(),
        notify_on: vec![SessionStatus::WaitingPermission, SessionStatus::Completed],
        project_rules: Vec::new(),
        cooldown_seconds: None,
        title_template: None,
        body_template: None,
        api_port: None,
        external_url: None,
    };
    let mut history = NotificationHistory::default();
    let sinks = build_sinks(&[]);

    // Start sessions
    let events = vec![
        make_event(EventType::SessionStart, "a", "/a", "s1"),
        make_event(EventType::SessionStart, "b", "/b", "s2"),
    ];
    apply_events_to_state(&mut state, &events);

    // Transition to waiting
    let old = state
        .sessions
        .iter()
        .map(|(k, v)| (k.clone(), v.status.clone()))
        .collect();

    let events2 = vec![
        make_notification_event(
            "a",
            "/a",
            "s1",
            NotificationType::PermissionPrompt,
            "approve",
        ),
        make_notification_event(
            "b",
            "/b",
            "s2",
            NotificationType::PermissionPrompt,
            "approve",
        ),
    ];
    apply_events_to_state(&mut state, &events2);

    let notifications = detect_status_transitions(&old, &state.sessions, &settings);
    assert_eq!(notifications.len(), 2);
    for n in &notifications {
        history.push(dispatch(&sinks, n));
    }

    // Now complete sessions
    let old = state
        .sessions
        .iter()
        .map(|(k, v)| (k.clone(), v.status.clone()))
        .collect();

    let events3 = vec![
        {
            let mut e = make_event(EventType::Stop, "a", "/a", "s1");
            e.timestamp = "2025-01-01T00:02:00Z".to_string();
            e
        },
        {
            let mut e = make_event(EventType::Stop, "b", "/b", "s2");
            e.timestamp = "2025-01-01T00:02:00Z".to_string();
            e
        },
    ];
    apply_events_to_state(&mut state, &events3);

    let notifications = detect_status_transitions(&old, &state.sessions, &settings);
    assert_eq!(notifications.len(), 2);
    for n in &notifications {
        history.push(dispatch(&sinks, n));
    }

    // Total: 4 notification records (2 waiting + 2 completed)
    assert_eq!(history.records().len(), 4);
}
