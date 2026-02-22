use crate::state::{AppState, EventInfo, EventType, NotificationType, SessionInfo, SessionStatus};

/// Process a single event and update the app state.
/// This is the pure logic version without side effects (no tmux path caching).
/// The main app wraps this with additional side effects.
pub fn process_event(state: &mut AppState, event: EventInfo) {
    state.recent_events.push_back(event.clone());
    if state.recent_events.len() > 50 {
        state.recent_events.pop_front();
    }

    let key = if event.project_dir.is_empty() {
        event.project_name.clone()
    } else {
        event.project_dir.clone()
    };

    match event.event_type {
        EventType::SessionStart => {
            state.cached_paths.update_from_event(&event);
            let transport = event.to_transport();
            state.sessions.insert(
                key,
                SessionInfo {
                    project_name: event.project_name.clone(),
                    project_dir: event.project_dir.clone(),
                    status: SessionStatus::Active,
                    last_event: event.timestamp.clone(),
                    waiting_for: String::new(),
                    tmux_pane: event.tmux_pane,
                    transport,
                },
            );
        }
        EventType::SessionEnd => {
            state.sessions.remove(&key);
        }
        EventType::Notification => {
            let new_status = match event.notification_type {
                NotificationType::PermissionPrompt => SessionStatus::WaitingPermission,
                NotificationType::IdlePrompt => SessionStatus::WaitingInput,
                NotificationType::Other => SessionStatus::Active,
            };
            let waiting_info = if !event.message.is_empty() {
                event.message.clone()
            } else if !event.tool_name.is_empty() {
                event.tool_name.clone()
            } else {
                String::new()
            };
            state.upsert_session(key, &event, new_status, waiting_info);
        }
        EventType::Stop => {
            state.cached_paths.update_from_event(&event);
            state.upsert_session(key, &event, SessionStatus::Completed, String::new());
        }
        EventType::PostToolUse => {
            state.upsert_session(key, &event, SessionStatus::Active, String::new());
        }
        EventType::UserPromptSubmit => {
            state.upsert_session(key, &event, SessionStatus::Active, String::new());
        }
        EventType::Unknown => {
            if let Some(session) = state.sessions.get_mut(&key) {
                session.last_event = event.timestamp;
            }
        }
    }
}

/// Apply multiple parsed events to the AppState.
pub fn apply_events_to_state(state: &mut AppState, events: &[EventInfo]) {
    for event in events {
        process_event(state, event.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(event_type: EventType, notification_type: NotificationType) -> EventInfo {
        EventInfo {
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            event_type,
            matcher: String::new(),
            project_name: "test-project".to_string(),
            project_dir: "/home/user/test-project".to_string(),
            session_id: "session-1".to_string(),
            message: String::new(),
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

    fn make_simple_event(event_type: EventType) -> EventInfo {
        make_event(event_type, NotificationType::Other)
    }

    // -- SessionStart --

    #[test]
    fn session_start_creates_active_session() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));

        assert_eq!(state.sessions.len(), 1);
        let session = state.sessions.get("/home/user/test-project").unwrap();
        assert_eq!(session.status, SessionStatus::Active);
        assert_eq!(session.project_name, "test-project");
    }

    #[test]
    fn session_start_uses_project_name_when_dir_empty() {
        let mut state = AppState::default();
        let mut event = make_simple_event(EventType::SessionStart);
        event.project_dir = String::new();
        event.project_name = "fallback-name".into();
        process_event(&mut state, event);

        assert!(state.sessions.contains_key("fallback-name"));
    }

    #[test]
    fn session_start_captures_tmux_pane() {
        let mut state = AppState::default();
        let mut event = make_simple_event(EventType::SessionStart);
        event.tmux_pane = "%3".into();
        process_event(&mut state, event);

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .tmux_pane,
            "%3"
        );
    }

    #[test]
    fn session_start_updates_cached_paths() {
        let mut state = AppState::default();
        let mut event = make_simple_event(EventType::SessionStart);
        event.npx_path = "/usr/local/bin/npx".into();
        process_event(&mut state, event);

        assert_eq!(state.cached_paths.npx_path, "/usr/local/bin/npx");
    }

    // -- SessionEnd --

    #[test]
    fn session_end_removes_session() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));
        assert_eq!(state.sessions.len(), 1);

        process_event(&mut state, make_simple_event(EventType::SessionEnd));
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn session_end_on_nonexistent_is_noop() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionEnd));
        assert!(state.sessions.is_empty());
    }

    // -- Notification --

    #[test]
    fn notification_permission_sets_waiting_permission() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));
        process_event(
            &mut state,
            make_event(EventType::Notification, NotificationType::PermissionPrompt),
        );

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::WaitingPermission
        );
    }

    #[test]
    fn notification_idle_sets_waiting_input() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));
        process_event(
            &mut state,
            make_event(EventType::Notification, NotificationType::IdlePrompt),
        );

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::WaitingInput
        );
    }

    #[test]
    fn notification_other_keeps_active() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));
        process_event(
            &mut state,
            make_event(EventType::Notification, NotificationType::Other),
        );

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Active
        );
    }

    #[test]
    fn notification_uses_message_as_waiting_info() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));

        let mut notification =
            make_event(EventType::Notification, NotificationType::PermissionPrompt);
        notification.message = "Approve bash command".into();
        process_event(&mut state, notification);

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .waiting_for,
            "Approve bash command"
        );
    }

    #[test]
    fn notification_falls_back_to_tool_name() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));

        let mut notification =
            make_event(EventType::Notification, NotificationType::PermissionPrompt);
        notification.tool_name = "Bash".into();
        process_event(&mut state, notification);

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .waiting_for,
            "Bash"
        );
    }

    #[test]
    fn notification_creates_session_if_not_exists() {
        let mut state = AppState::default();
        process_event(
            &mut state,
            make_event(EventType::Notification, NotificationType::PermissionPrompt),
        );

        assert_eq!(state.sessions.len(), 1);
        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::WaitingPermission
        );
    }

    // -- Stop --

    #[test]
    fn stop_sets_completed() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));
        process_event(&mut state, make_simple_event(EventType::Stop));

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Completed
        );
    }

    #[test]
    fn stop_updates_cached_paths() {
        let mut state = AppState::default();
        let mut stop = make_simple_event(EventType::Stop);
        stop.npx_path = "/usr/bin/npx".into();
        stop.tmux_path = "/usr/bin/tmux".into();
        process_event(&mut state, stop);

        assert_eq!(state.cached_paths.npx_path, "/usr/bin/npx");
        assert_eq!(state.cached_paths.tmux_path, "/usr/bin/tmux");
    }

    // -- PostToolUse --

    #[test]
    fn post_tool_use_sets_active() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));
        process_event(
            &mut state,
            make_event(EventType::Notification, NotificationType::PermissionPrompt),
        );
        process_event(&mut state, make_simple_event(EventType::PostToolUse));

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Active
        );
    }

    // -- UserPromptSubmit --

    #[test]
    fn user_prompt_submit_sets_active() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));
        process_event(&mut state, make_simple_event(EventType::Stop));
        process_event(&mut state, make_simple_event(EventType::UserPromptSubmit));

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Active
        );
    }

    // -- Unknown --

    #[test]
    fn unknown_updates_timestamp_on_existing() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::SessionStart));

        let mut unknown = make_simple_event(EventType::Unknown);
        unknown.timestamp = "2025-01-01T01:00:00Z".into();
        process_event(&mut state, unknown);

        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .last_event,
            "2025-01-01T01:00:00Z"
        );
    }

    #[test]
    fn unknown_is_noop_when_no_session() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::Unknown));
        assert!(state.sessions.is_empty());
    }

    // -- Recent events buffer --

    #[test]
    fn recent_events_are_appended() {
        let mut state = AppState::default();
        process_event(&mut state, make_simple_event(EventType::PostToolUse));
        assert_eq!(state.recent_events.len(), 1);
    }

    #[test]
    fn recent_events_capped_at_50() {
        let mut state = AppState::default();
        for i in 0..60 {
            let mut event = make_simple_event(EventType::PostToolUse);
            event.timestamp = format!("2025-01-01T{:02}:00:00Z", i % 24);
            process_event(&mut state, event);
        }
        assert_eq!(state.recent_events.len(), 50);
    }

    // -- apply_events_to_state --

    #[test]
    fn apply_events_processes_multiple() {
        let mut state = AppState::default();
        let events = vec![
            make_simple_event(EventType::SessionStart),
            make_event(EventType::Notification, NotificationType::PermissionPrompt),
            make_simple_event(EventType::PostToolUse),
            make_simple_event(EventType::Stop),
        ];

        apply_events_to_state(&mut state, &events);

        assert_eq!(state.sessions.len(), 1);
        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Completed
        );
        assert_eq!(state.recent_events.len(), 4);
    }

    #[test]
    fn apply_events_empty_is_noop() {
        let mut state = AppState::default();
        apply_events_to_state(&mut state, &[]);
        assert!(state.sessions.is_empty());
        assert!(state.recent_events.is_empty());
    }

    // -- Full lifecycle functional test --

    #[test]
    fn full_session_lifecycle() {
        let mut state = AppState::default();

        // 1. Session starts
        process_event(&mut state, make_simple_event(EventType::SessionStart));
        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Active
        );

        // 2. Tool use
        process_event(&mut state, make_simple_event(EventType::PostToolUse));
        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Active
        );

        // 3. Permission prompt
        let mut perm = make_event(EventType::Notification, NotificationType::PermissionPrompt);
        perm.message = "Run bash command".into();
        process_event(&mut state, perm);
        let session = state.sessions.get("/home/user/test-project").unwrap();
        assert_eq!(session.status, SessionStatus::WaitingPermission);
        assert_eq!(session.waiting_for, "Run bash command");
        assert_eq!(state.waiting_session_count(), 1);

        // 4. User approves
        process_event(&mut state, make_simple_event(EventType::PostToolUse));
        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Active
        );
        assert_eq!(state.waiting_session_count(), 0);

        // 5. Response completes
        process_event(&mut state, make_simple_event(EventType::Stop));
        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Completed
        );

        // 6. New prompt
        process_event(&mut state, make_simple_event(EventType::UserPromptSubmit));
        assert_eq!(
            state
                .sessions
                .get("/home/user/test-project")
                .unwrap()
                .status,
            SessionStatus::Active
        );

        // 7. Session ends
        process_event(&mut state, make_simple_event(EventType::SessionEnd));
        assert!(state.sessions.is_empty());
        assert_eq!(state.recent_events.len(), 7);
    }

    // -- Multi-session functional test --

    #[test]
    fn multiple_sessions_independent() {
        let mut state = AppState::default();

        let mut start1 = make_simple_event(EventType::SessionStart);
        start1.project_name = "project-a".into();
        start1.project_dir = "/home/user/project-a".into();
        process_event(&mut state, start1);

        let mut start2 = make_simple_event(EventType::SessionStart);
        start2.project_name = "project-b".into();
        start2.project_dir = "/home/user/project-b".into();
        process_event(&mut state, start2);
        assert_eq!(state.sessions.len(), 2);

        // First session waiting
        let mut perm = make_event(EventType::Notification, NotificationType::PermissionPrompt);
        perm.project_dir = "/home/user/project-a".into();
        perm.project_name = "project-a".into();
        process_event(&mut state, perm);

        assert_eq!(
            state.sessions.get("/home/user/project-a").unwrap().status,
            SessionStatus::WaitingPermission
        );
        assert_eq!(
            state.sessions.get("/home/user/project-b").unwrap().status,
            SessionStatus::Active
        );
        assert_eq!(state.waiting_session_count(), 1);

        // Second session completes
        let mut stop = make_simple_event(EventType::Stop);
        stop.project_dir = "/home/user/project-b".into();
        stop.project_name = "project-b".into();
        process_event(&mut state, stop);
        assert_eq!(
            state.sessions.get("/home/user/project-b").unwrap().status,
            SessionStatus::Completed
        );
        assert_eq!(state.waiting_session_count(), 1);

        // End first session
        let mut end = make_simple_event(EventType::SessionEnd);
        end.project_dir = "/home/user/project-a".into();
        end.project_name = "project-a".into();
        process_event(&mut state, end);
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(state.waiting_session_count(), 0);
    }
}
