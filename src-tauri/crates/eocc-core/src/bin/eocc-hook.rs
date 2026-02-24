//! eocc-hook — Claude Code event hook handler (Rust)
//!
//! Usage: eocc-hook <event_type> [matcher]
//!
//! Receives events from Claude Code hooks, logs them to ~/.eocc/logs/events.jsonl,
//! optionally forwards to a webhook URL, and dispatches notifications.

use eocc_core::hook_state;
use eocc_core::notifications::{
    self, build_sinks, load_settings_from_file, NotificationPriority, SessionNotification,
};
use eocc_core::state::{EventInfo, EventType, HookChannelResult, NotificationType, SessionStatus};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    if let Err(e) = run() {
        log::error!("Hook error: {}", e);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let event_type_str = args.get(1).map(|s| s.as_str()).unwrap_or("unknown");
    let matcher = args.get(2).map(|s| s.as_str()).unwrap_or("");

    let raw_input = read_stdin();

    let project_dir = env_or("CLAUDE_PROJECT_DIR", "unknown");
    let project_name = basename(&project_dir);
    let tmux_pane = env_or("TMUX_PANE", "");

    let ssh_connection = env_or("SSH_CONNECTION", "");
    let ssh_parts: Vec<&str> = if ssh_connection.is_empty() {
        Vec::new()
    } else {
        ssh_connection.split_whitespace().collect()
    };

    let transport_type = env_or(
        "EOCC_TRANSPORT",
        if !ssh_connection.is_empty() {
            "ssh"
        } else {
            "local"
        },
    );
    let transport_host = env_or(
        "EOCC_TRANSPORT_HOST",
        ssh_parts.get(2).copied().unwrap_or(""),
    );
    let transport_port = env_or(
        "EOCC_TRANSPORT_PORT",
        ssh_parts.get(3).copied().unwrap_or("22"),
    );
    let transport_user = env_or("EOCC_TRANSPORT_USER", &env_or("USER", ""));

    let tmux_session = if !tmux_pane.is_empty() {
        resolve_tmux_session(&tmux_pane)
    } else {
        String::new()
    };

    let session_id = get_string(&raw_input, "session_id", "unknown");
    let message = get_string(&raw_input, "message", "");
    let mut notification_type_str = get_string(&raw_input, "notification_type", "");
    let tool_name = get_string(&raw_input, "tool_name", "");

    if event_type_str == "notification" && notification_type_str.is_empty() {
        notification_type_str = matcher.to_string();
    }

    let (npx_path, tmux_path_bin) = if event_type_str == "session_start" || event_type_str == "stop"
    {
        (which_command("npx"), which_command("tmux"))
    } else {
        (String::new(), String::new())
    };

    let timestamp = iso_timestamp_utc();

    let notification_type = match notification_type_str.as_str() {
        "permission_prompt" => NotificationType::PermissionPrompt,
        "idle_prompt" => NotificationType::IdlePrompt,
        _ => NotificationType::Other,
    };

    let event = EventInfo {
        timestamp: timestamp.clone(),
        event_type: parse_event_type(event_type_str),
        matcher: matcher.to_string(),
        project_name: project_name.clone(),
        project_dir: project_dir.clone(),
        session_id,
        message: message.clone(),
        notification_type,
        tool_name: tool_name.clone(),
        tmux_pane: tmux_pane.clone(),
        npx_path,
        tmux_path: tmux_path_bin,
        transport_type: String::new(),
        transport_host: String::new(),
        transport_port: String::new(),
        transport_user: String::new(),
        notification_results: Vec::new(),
    };

    let home = home_dir();
    let log_dir = home.join(".eocc").join("logs");
    fs::create_dir_all(&log_dir)?;
    let log_file = log_dir.join("events.jsonl");

    // Post to webhook if configured
    if let Ok(url) = std::env::var("EOCC_WEBHOOK_URL") {
        if !url.is_empty() {
            let mut webhook_payload = serde_json::to_value(&event).unwrap_or(serde_json::json!({}));
            if let Some(obj) = webhook_payload.as_object_mut() {
                obj.insert(
                    "transport_type".to_string(),
                    Value::String(transport_type.clone()),
                );
                obj.insert(
                    "transport_host".to_string(),
                    Value::String(transport_host.clone()),
                );
                obj.insert(
                    "transport_port".to_string(),
                    Value::String(transport_port.clone()),
                );
                obj.insert(
                    "transport_user".to_string(),
                    Value::String(transport_user.clone()),
                );
                obj.insert(
                    "tmux_session".to_string(),
                    Value::String(tmux_session.clone()),
                );
            }
            post_webhook(
                &url,
                &serde_json::to_string(&webhook_payload).unwrap_or_default(),
            );
        }
    }

    append_line(&log_file, &serde_json::to_string(&event)?);

    // Dispatch notifications for notification-worthy events
    const NOTIFY_EVENTS: &[&str] = &["notification", "stop", "session_start", "session_end"];
    if NOTIFY_EVENTS.contains(&event_type_str) {
        let waiting_info = if !message.is_empty() {
            &message
        } else {
            &tool_name
        };

        let viewer_url = env_or("EOCC_VIEWER_URL", "");
        let transport = TransportInfo {
            type_: transport_type,
            host: transport_host,
            port: transport_port,
            user: transport_user,
            tmux_session,
            tmux_pane,
            viewer_url,
        };

        let ctx = NotifyContext {
            home: &home,
            event_type: event_type_str,
            notification_type: &notification_type_str,
            project_dir: &project_dir,
            project_name: &project_name,
            waiting_info,
            transport: &transport,
            log_file: &log_file,
        };
        try_dispatch_notification(&ctx);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Notification dispatch
// ---------------------------------------------------------------------------

struct TransportInfo {
    type_: String,
    host: String,
    port: String,
    user: String,
    tmux_session: String,
    tmux_pane: String,
    viewer_url: String,
}

struct NotifyContext<'a> {
    home: &'a Path,
    event_type: &'a str,
    notification_type: &'a str,
    project_dir: &'a str,
    project_name: &'a str,
    waiting_info: &'a str,
    transport: &'a TransportInfo,
    log_file: &'a Path,
}

fn try_dispatch_notification(ctx: &NotifyContext<'_>) {
    if let Err(e) = dispatch_notification_inner(ctx) {
        log::error!("Notification dispatch error: {}", e);
    }
}

fn dispatch_notification_inner(ctx: &NotifyContext<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let settings_file = ctx.home.join(".eocc").join("notification_settings.toml");
    let state_file = ctx.home.join(".eocc").join("hook_state.json");

    let settings = load_settings_from_file(&settings_file);
    if !settings.enabled || settings.channels.is_empty() {
        return Ok(());
    }

    let new_status = match map_event_to_status(ctx.event_type, ctx.notification_type) {
        Some(status) => status,
        None => return Ok(()),
    };

    let session_key = if ctx.project_dir.is_empty() || ctx.project_dir == "unknown" {
        ctx.project_name
    } else {
        ctx.project_dir
    };

    let mut hook_state = hook_state::load(&state_file);

    // session_end: remove session
    let Some(new_status) = new_status else {
        hook_state.sessions.remove(session_key);
        hook_state::save(&state_file, &hook_state);
        return Ok(());
    };

    let new_status_str = status_to_string(&new_status);
    let prev = hook_state.sessions.get(session_key);
    let old_status_str = prev.map(|p| p.status.clone());
    let prev_last_notified = prev.and_then(|p| p.last_notified);

    // Update stored status
    hook_state.sessions.insert(
        session_key.to_string(),
        hook_state::HookSessionState {
            status: new_status_str.to_string(),
            last_notified: prev_last_notified,
        },
    );

    // No change
    if old_status_str.as_deref() == Some(new_status_str) {
        hook_state::save(&state_file, &hook_state);
        return Ok(());
    }

    // Check project rules
    let (enabled, notify_on) = settings.resolve_for_project(ctx.project_dir);
    if !enabled || !notify_on.contains(&new_status) {
        hook_state::save(&state_file, &hook_state);
        return Ok(());
    }

    // Check cooldown
    if let Some(cd) = settings.cooldown_seconds {
        if let Some(last) = prev_last_notified {
            let now_ms = millis_since_epoch();
            let elapsed_secs = now_ms.saturating_sub(last) / 1000;
            if elapsed_secs < cd {
                hook_state::save(&state_file, &hook_state);
                return Ok(());
            }
        }
    }

    // Build notification
    let priority = match new_status {
        SessionStatus::WaitingPermission | SessionStatus::WaitingInput => {
            NotificationPriority::High
        }
        SessionStatus::Completed => NotificationPriority::Normal,
        SessionStatus::Active => NotificationPriority::Low,
    };

    let click_url = build_connect_url(ctx.transport);

    let notification = SessionNotification {
        project_name: ctx.project_name.to_string(),
        project_dir: ctx.project_dir.to_string(),
        session_id: String::new(),
        old_status: old_status_str.as_deref().and_then(parse_session_status),
        new_status: new_status.clone(),
        message: ctx.waiting_info.to_string(),
        priority,
        title_template: settings.title_template.clone(),
        body_template: settings.body_template.clone(),
        click_url: if click_url.is_empty() {
            None
        } else {
            Some(click_url)
        },
    };

    let sinks = build_sinks(&settings.channels);
    let record = notifications::dispatch(&sinks, &notification);

    // Write notification_result event
    let channel_results: Vec<HookChannelResult> = record
        .channels
        .iter()
        .map(|c| HookChannelResult {
            channel: c.name.clone(),
            ok: c.success,
            error: c.error.clone(),
        })
        .collect();

    if !channel_results.is_empty() {
        let result_event = EventInfo {
            timestamp: iso_timestamp_utc(),
            event_type: EventType::NotificationResult,
            matcher: String::new(),
            project_name: ctx.project_name.to_string(),
            project_dir: ctx.project_dir.to_string(),
            session_id: String::new(),
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
            notification_results: channel_results,
        };
        if let Ok(json) = serde_json::to_string(&result_event) {
            append_line(ctx.log_file, &json);
        }
    }

    // Update last_notified
    if let Some(session) = hook_state.sessions.get_mut(session_key) {
        session.last_notified = Some(millis_since_epoch());
    }
    hook_state::save(&state_file, &hook_state);

    Ok(())
}

// ---------------------------------------------------------------------------
// Event / status mapping
// ---------------------------------------------------------------------------

/// Maps an event type string to a session status.
/// Returns `Some(Some(status))` for known events, `Some(None)` for session_end
/// (remove session), and `None` for unknown events (skip).
fn map_event_to_status(event_type: &str, notification_type: &str) -> Option<Option<SessionStatus>> {
    match event_type {
        "session_start" | "post_tool_use" | "user_prompt_submit" => {
            Some(Some(SessionStatus::Active))
        }
        "session_end" => Some(None),
        "stop" => Some(Some(SessionStatus::Completed)),
        "notification" => match notification_type {
            "permission_prompt" => Some(Some(SessionStatus::WaitingPermission)),
            "idle_prompt" => Some(Some(SessionStatus::WaitingInput)),
            _ => Some(Some(SessionStatus::Active)),
        },
        _ => None,
    }
}

fn status_to_string(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Active => "active",
        SessionStatus::WaitingPermission => "waiting_permission",
        SessionStatus::WaitingInput => "waiting_input",
        SessionStatus::Completed => "completed",
    }
}

fn parse_session_status(s: &str) -> Option<SessionStatus> {
    match s {
        "active" => Some(SessionStatus::Active),
        "waiting_permission" => Some(SessionStatus::WaitingPermission),
        "waiting_input" => Some(SessionStatus::WaitingInput),
        "completed" => Some(SessionStatus::Completed),
        _ => None,
    }
}

fn parse_event_type(s: &str) -> EventType {
    match s {
        "session_start" => EventType::SessionStart,
        "session_end" => EventType::SessionEnd,
        "notification" => EventType::Notification,
        "notification_result" => EventType::NotificationResult,
        "stop" => EventType::Stop,
        "post_tool_use" => EventType::PostToolUse,
        "user_prompt_submit" => EventType::UserPromptSubmit,
        _ => EventType::Unknown,
    }
}

// ---------------------------------------------------------------------------
// I/O helpers
// ---------------------------------------------------------------------------

fn read_stdin() -> Value {
    if std::io::stdin().is_terminal() {
        return serde_json::json!({});
    }
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() {
        return serde_json::json!({});
    }
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        return serde_json::json!({});
    }
    serde_json::from_str(trimmed).unwrap_or(serde_json::json!({}))
}

fn append_line(file_path: &Path, line: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(file_path) {
        let _ = writeln!(f, "{}", line);
    }
}

fn post_webhook(url: &str, body: &str) {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();
    match agent
        .post(url)
        .set("Content-Type", "application/json")
        .send_string(body)
    {
        Ok(_) => {}
        Err(e) => log::debug!("Webhook post failed: {}", e),
    }
}

// ---------------------------------------------------------------------------
// System helpers
// ---------------------------------------------------------------------------

fn which_command(cmd: &str) -> String {
    Command::new("which")
        .arg(cmd)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn resolve_tmux_session(pane_id: &str) -> String {
    if !pane_id.starts_with('%') {
        return String::new();
    }
    Command::new("tmux")
        .args(["display-message", "-p", "-t", pane_id, "#{session_name}"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()))
}

/// Returns env var value, falling back to `default` if unset or empty.
fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn basename(p: &str) -> String {
    let clean = p.trim_end_matches('/');
    if clean.is_empty() {
        return "unknown".to_string();
    }
    Path::new(clean)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn get_string(v: &Value, key: &str, fallback: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => fallback.to_string(),
    }
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

fn build_connect_url(transport: &TransportInfo) -> String {
    if !transport.viewer_url.is_empty() {
        let base = transport.viewer_url.trim_end_matches('/');
        return if !transport.tmux_pane.is_empty() {
            format!("{}/tmux/{}", base, urlencoded(&transport.tmux_pane))
        } else {
            base.to_string()
        };
    }

    if transport.host.is_empty() || transport.type_ == "local" {
        return String::new();
    }

    let user = if !transport.user.is_empty() {
        format!("{}@", transport.user)
    } else {
        String::new()
    };
    let tmux_cmd = if !transport.tmux_session.is_empty() {
        format!(";tmux attach -t {}", transport.tmux_session)
    } else {
        String::new()
    };

    match transport.type_.as_str() {
        "ssh" | "tailscale" => {
            let port = if !transport.port.is_empty() && transport.port != "22" {
                format!(":{}", transport.port)
            } else {
                String::new()
            };
            format!("ssh://{}{}{}{}", user, transport.host, port, tmux_cmd)
        }
        "mosh" => format!("mosh://{}{}{}", user, transport.host, tmux_cmd),
        _ => String::new(),
    }
}

fn urlencoded(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Time helpers
// ---------------------------------------------------------------------------

fn iso_timestamp_utc() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = dur.as_secs();
    let millis = dur.subsec_millis();

    let days = total_secs / 86400;
    let day_secs = (total_secs % 86400) as u32;
    let h = day_secs / 3600;
    let min = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    let (y, m, d) = civil_from_days(days as i64);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y, m, d, h, min, s, millis
    )
}

/// Howard Hinnant's civil_from_days algorithm.
/// Converts days since 1970-01-01 to (year, month, day).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn millis_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
