use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::persist::{create_runtime_snapshot, save_runtime_snapshot};
use crate::state::{AppState, EventInfo, Transport};
use crate::tmux;
use crate::tray::{emit_state_update, update_tray_and_badge};
use eocc_core::notifications::{self, NotificationSink};

const WEB_TMUX_VIEWER_HTML: &str = include_str!("web_tmux_viewer.html");

pub fn start_api_server(
    port: u16,
    api_token: Option<String>,
    app_handle: tauri::AppHandle,
    state: Arc<Mutex<AppState>>,
    notification_sinks: Arc<Mutex<Vec<Box<dyn NotificationSink>>>>,
    notification_history: Arc<Mutex<notifications::history::NotificationHistory>>,
) {
    std::thread::spawn(move || {
        let addr = format!("127.0.0.1:{}", port);
        let server = match tiny_http::Server::http(&addr) {
            Ok(s) => s,
            Err(e) => {
                log::error!(
                    target: "eocc.api",
                    "Failed to start API server on {}: {}",
                    addr,
                    e
                );
                return;
            }
        };

        if api_token.is_some() {
            log::info!(target: "eocc.api", "API server listening on {} (token auth enabled)", addr);
        } else {
            log::info!(target: "eocc.api", "API server listening on {} (no auth)", addr);
        }

        let mut last_notified: HashMap<String, Instant> = HashMap::new();

        for request in server.incoming_requests() {
            // Verify bearer token if configured
            if let Some(ref token) = api_token {
                let authorized = request
                    .headers()
                    .iter()
                    .any(|h| {
                        h.field.equiv("Authorization")
                            && h.value.as_str() == format!("Bearer {}", token)
                    });
                if !authorized {
                    json_response(request, 401, r#"{"error":"Unauthorized"}"#);
                    continue;
                }
            }

            let url = request.url().to_string();
            let method = request.method().to_string();

            // Parse path and query string
            let (path, query) = match url.split_once('?') {
                Some((p, q)) => (p.to_string(), q.to_string()),
                None => (url.clone(), String::new()),
            };

            let path_str: &str = &path;
            match (method.as_str(), path_str) {
                ("POST", "/api/events") => {
                    handle_post_events(
                        request,
                        &app_handle,
                        &state,
                        &notification_sinks,
                        &notification_history,
                        &mut last_notified,
                    );
                }
                ("GET", p) if p.starts_with("/api/tmux/") && p.ends_with("/capture") => {
                    handle_tmux_capture(request, p, &query, &state);
                }
                ("POST", p) if p.starts_with("/api/tmux/") && p.ends_with("/send") => {
                    handle_tmux_send(request, p, &state);
                }
                ("GET", p) if p.starts_with("/tmux/") => {
                    handle_web_tmux_viewer(request, p, &query);
                }
                ("GET", "/") => {
                    let response =
                        tiny_http::Response::from_string("EOCC API server").with_status_code(200);
                    let _ = request.respond(response);
                }
                _ => {
                    let response =
                        tiny_http::Response::from_string("Not Found").with_status_code(404);
                    let _ = request.respond(response);
                }
            }
        }
    });
}

fn read_body(request: &mut tiny_http::Request) -> Result<String, String> {
    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .map_err(|e| format!("Failed to read request body: {}", e))?;
    Ok(body)
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?.to_string();
            let value = parts.next().unwrap_or("").to_string();
            Some((key, urldecode(&value)))
        })
        .collect()
}

fn urldecode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                result.push(byte);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            result.push(b' ');
            i += 1;
            continue;
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

fn extract_pane_id(path: &str, prefix: &str, suffix: &str) -> Option<String> {
    let stripped = path.strip_prefix(prefix)?;
    let pane_id = stripped.strip_suffix(suffix)?;
    Some(urldecode(pane_id))
}

fn get_transport_for_session(state: &Arc<Mutex<AppState>>, project_dir: Option<&str>) -> Transport {
    state
        .lock()
        .map(|s| s.get_transport(project_dir))
        .unwrap_or(Transport::Local {})
}

fn json_response(request: tiny_http::Request, status: u16, body: &str) {
    let header = tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap();
    let response = tiny_http::Response::from_string(body)
        .with_status_code(status)
        .with_header(header);
    let _ = request.respond(response);
}

fn handle_post_events(
    mut request: tiny_http::Request,
    app_handle: &tauri::AppHandle,
    state: &Arc<Mutex<AppState>>,
    notification_sinks: &Arc<Mutex<Vec<Box<dyn NotificationSink>>>>,
    notification_history: &Arc<Mutex<notifications::history::NotificationHistory>>,
    last_notified: &mut HashMap<String, Instant>,
) {
    let body = match read_body(&mut request) {
        Ok(b) => b,
        Err(e) => {
            json_response(request, 400, &format!(r#"{{"error":"{}"}}"#, e));
            return;
        }
    };

    // Support both single event and array of events
    let events: Vec<EventInfo> = if body.trim_start().starts_with('[') {
        match serde_json::from_str(&body) {
            Ok(events) => events,
            Err(e) => {
                json_response(
                    request,
                    400,
                    &format!(r#"{{"error":"Invalid JSON array: {}"}}"#, e),
                );
                return;
            }
        }
    } else {
        match serde_json::from_str::<EventInfo>(&body) {
            Ok(event) => vec![event],
            Err(e) => {
                json_response(
                    request,
                    400,
                    &format!(r#"{{"error":"Invalid JSON: {}"}}"#, e),
                );
                return;
            }
        }
    };

    if events.is_empty() {
        json_response(request, 200, r#"{"processed":0}"#);
        return;
    }

    log::info!(
        target: "eocc.api",
        "Received {} event(s) via webhook",
        events.len()
    );

    // Process events through the shared notification pipeline
    let (snapshot, pipeline) = {
        let Ok(mut state_guard) = state.lock() else {
            json_response(request, 500, r#"{"error":"State lock failed"}"#);
            return;
        };

        let pipeline = crate::notification_pipeline::apply_and_detect(
            &mut state_guard,
            &events,
            notification_sinks,
        );
        update_tray_and_badge(app_handle, &state_guard);
        emit_state_update(app_handle, &state_guard);

        (create_runtime_snapshot(&state_guard), pipeline)
    };

    save_runtime_snapshot(app_handle, &snapshot);
    crate::notification_pipeline::dispatch_notifications(
        &pipeline,
        notification_sinks,
        notification_history,
        last_notified,
    );

    let count = events.len();
    json_response(request, 200, &format!(r#"{{"processed":{}}}"#, count));
}

fn handle_tmux_capture(
    request: tiny_http::Request,
    path: &str,
    query: &str,
    state: &Arc<Mutex<AppState>>,
) {
    let pane_id = match extract_pane_id(path, "/api/tmux/", "/capture") {
        Some(id) => id,
        None => {
            json_response(request, 400, r#"{"error":"Invalid pane ID"}"#);
            return;
        }
    };

    let params = parse_query(query);
    let project_dir = params.get("project_dir").map(|s| s.as_str());
    let transport = get_transport_for_session(state, project_dir);

    match tmux::capture_pane_with_transport(&pane_id, &transport) {
        Ok(content) => {
            let header =
                tiny_http::Header::from_bytes("Content-Type", "text/plain; charset=utf-8").unwrap();
            let cors = tiny_http::Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap();
            let response = tiny_http::Response::from_string(content)
                .with_status_code(200)
                .with_header(header)
                .with_header(cors);
            let _ = request.respond(response);
        }
        Err(e) => {
            json_response(request, 500, &format!(r#"{{"error":"{}"}}"#, e));
        }
    }
}

fn handle_tmux_send(mut request: tiny_http::Request, path: &str, state: &Arc<Mutex<AppState>>) {
    let pane_id = match extract_pane_id(path, "/api/tmux/", "/send") {
        Some(id) => id,
        None => {
            json_response(request, 400, r#"{"error":"Invalid pane ID"}"#);
            return;
        }
    };

    let body = match read_body(&mut request) {
        Ok(b) => b,
        Err(e) => {
            json_response(request, 400, &format!(r#"{{"error":"{}"}}"#, e));
            return;
        }
    };

    #[derive(serde::Deserialize)]
    struct SendKeysRequest {
        keys: String,
        #[serde(default)]
        project_dir: Option<String>,
    }

    let req: SendKeysRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            json_response(
                request,
                400,
                &format!(r#"{{"error":"Invalid JSON: {}"}}"#, e),
            );
            return;
        }
    };

    let transport = get_transport_for_session(state, req.project_dir.as_deref());

    match tmux::send_keys_with_transport(&pane_id, &req.keys, &transport) {
        Ok(()) => {
            json_response(request, 200, r#"{"ok":true}"#);
        }
        Err(e) => {
            json_response(request, 500, &format!(r#"{{"error":"{}"}}"#, e));
        }
    }
}

fn handle_web_tmux_viewer(request: tiny_http::Request, path: &str, query: &str) {
    let pane_id = match path.strip_prefix("/tmux/") {
        Some(id) if !id.is_empty() => urldecode(id),
        _ => {
            let response =
                tiny_http::Response::from_string("Missing pane ID").with_status_code(400);
            let _ = request.respond(response);
            return;
        }
    };

    let params = parse_query(query);
    let project_dir = params.get("project_dir").cloned().unwrap_or_default();

    let html = WEB_TMUX_VIEWER_HTML
        .replace("{{PANE_ID}}", &html_escape(&pane_id))
        .replace("{{PROJECT_DIR}}", &html_escape(&project_dir));

    let header = tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
    let response = tiny_http::Response::from_string(html)
        .with_status_code(200)
        .with_header(header);
    let _ = request.respond(response);
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
