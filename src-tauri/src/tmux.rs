use crate::state::Transport;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

static CACHED_TMUX_PATH: Mutex<Option<String>> = Mutex::new(None);

/// Set the cached tmux path from hook events
pub fn set_cached_tmux_path(path: &str) {
    if !path.is_empty() {
        if let Ok(mut cached) = CACHED_TMUX_PATH.lock() {
            *cached = Some(path.to_string());
            log::info!(target: "eocc.tmux", "Cached tmux path set to: {}", path);
        }
    }
}

fn get_tmux_path() -> Option<PathBuf> {
    if let Ok(cached) = CACHED_TMUX_PATH.lock() {
        if let Some(ref path_str) = *cached {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmuxPane {
    pub session_name: String,
    pub window_index: u32,
    pub window_name: String,
    pub pane_index: u32,
    pub pane_id: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmuxPaneSize {
    pub width: u32,
    pub height: u32,
}

fn validate_pane_id(pane_id: &str) -> Result<(), String> {
    // tmux pane ID format: %[0-9]+
    if pane_id.starts_with('%')
        && !pane_id[1..].is_empty()
        && pane_id[1..].chars().all(|c| c.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(format!("Invalid pane ID format: {}", pane_id))
    }
}

/// Validate SSH hostname: alphanumeric, dots, hyphens, colons (IPv6), and brackets.
fn validate_ssh_host(host: &str) -> Result<(), String> {
    if host.is_empty() {
        return Err("SSH host cannot be empty".to_string());
    }
    if host.starts_with('-') {
        return Err(format!("SSH host must not start with '-': {}", host));
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || ".-:[]_".contains(c))
    {
        return Err(format!("SSH host contains invalid characters: {}", host));
    }
    Ok(())
}

/// Validate SSH username: alphanumeric, underscore, hyphen, dot.
fn validate_ssh_user(user: &str) -> Result<(), String> {
    if user.is_empty() {
        return Err("SSH user cannot be empty".to_string());
    }
    if user.starts_with('-') {
        return Err(format!("SSH user must not start with '-': {}", user));
    }
    if !user
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        return Err(format!("SSH user contains invalid characters: {}", user));
    }
    Ok(())
}

/// Build SSH command arguments for a remote transport.
fn build_ssh_args(
    host: &str,
    port: u16,
    user: &Option<String>,
    identity_file: &Option<String>,
) -> Result<Vec<String>, String> {
    validate_ssh_host(host)?;
    if let Some(u) = user {
        validate_ssh_user(u)?;
    }

    let mut args = vec![
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "ConnectTimeout=5".to_string(),
    ];
    if let Some(key) = identity_file {
        args.push("-i".to_string());
        args.push(key.clone());
    }
    args.push("-p".to_string());
    args.push(port.to_string());
    let target = match user {
        Some(u) => format!("{}@{}", u, host),
        None => host.to_string(),
    };
    args.push(target);
    Ok(args)
}

/// Execute a command, routing through the session's transport if remote.
fn run_via_transport(
    transport: &Transport,
    program: &str,
    args: &[&str],
) -> Result<String, String> {
    match transport {
        Transport::Local {} => {
            // Local: run command directly (existing behavior)
            let output = Command::new(program)
                .args(args)
                .output()
                .map_err(|e| format!("Failed to execute {}: {}", program, e))?;
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("{} command failed: {}", program, stderr.trim()))
            }
        }
        Transport::Ssh {
            host,
            port,
            user,
            identity_file,
        } => {
            let mut ssh_args = build_ssh_args(host, *port, user, identity_file)?;
            ssh_args.push(program.to_string());
            ssh_args.extend(args.iter().map(|a| a.to_string()));
            let str_args: Vec<&str> = ssh_args.iter().map(|s| s.as_str()).collect();
            let output = Command::new("ssh")
                .args(&str_args)
                .output()
                .map_err(|e| format!("Failed to execute ssh: {}", e))?;
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("ssh command failed: {}", stderr.trim()))
            }
        }
        Transport::Mosh {
            host, port, user, ..
        } => {
            // Mosh doesn't support non-interactive command execution.
            // Fall back to SSH for tmux commands.
            let ssh_transport = Transport::Ssh {
                host: host.clone(),
                port: *port,
                user: user.clone(),
                identity_file: None,
            };
            run_via_transport(&ssh_transport, program, args)
        }
        Transport::Tailscale {
            host,
            user,
            identity_file,
        } => {
            // Try `tailscale ssh` first, fall back to plain SSH with Tailscale IP
            let tailscale_available = Command::new("tailscale")
                .arg("version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if tailscale_available {
                validate_ssh_host(host)?;
                if let Some(u) = user {
                    validate_ssh_user(u)?;
                }
                let target = match user {
                    Some(u) => format!("{}@{}", u, host),
                    None => host.clone(),
                };
                let mut ts_args = vec!["ssh".to_string(), target, program.to_string()];
                ts_args.extend(args.iter().map(|a| a.to_string()));
                let str_args: Vec<&str> = ts_args.iter().map(|s| s.as_str()).collect();
                let output = Command::new("tailscale")
                    .args(&str_args)
                    .output()
                    .map_err(|e| format!("Failed to execute tailscale ssh: {}", e))?;
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("tailscale ssh command failed: {}", stderr.trim()))
                }
            } else {
                // Fallback to standard SSH
                let ssh_transport = Transport::Ssh {
                    host: host.clone(),
                    port: 22,
                    user: user.clone(),
                    identity_file: identity_file.clone(),
                };
                run_via_transport(&ssh_transport, program, args)
            }
        }
    }
}

fn run_tmux_command(args: &[&str]) -> Result<String, String> {
    let tmux_path = get_tmux_path().ok_or_else(|| {
        "tmux path not available. Please start a Claude Code session first.".to_string()
    })?;

    let output = Command::new(&tmux_path)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute tmux: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("tmux command failed: {}", stderr.trim()))
    }
}

fn run_tmux_with_transport(transport: &Transport, args: &[&str]) -> Result<String, String> {
    match transport {
        Transport::Local {} => run_tmux_command(args),
        _ => run_via_transport(transport, "tmux", args),
    }
}

pub fn is_tmux_available() -> bool {
    get_tmux_path().is_some()
}

pub fn list_panes() -> Result<Vec<TmuxPane>, String> {
    list_panes_with_transport(&Transport::Local {})
}

pub fn list_panes_with_transport(transport: &Transport) -> Result<Vec<TmuxPane>, String> {
    let format =
        "#{session_name}|#{window_index}|#{window_name}|#{pane_index}|#{pane_id}|#{pane_active}";
    let output = run_tmux_with_transport(transport, &["list-panes", "-a", "-F", format])?;

    let panes: Vec<TmuxPane> = output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 6 {
                Some(TmuxPane {
                    session_name: parts[0].to_string(),
                    window_index: parts[1].parse().unwrap_or(0),
                    window_name: parts[2].to_string(),
                    pane_index: parts[3].parse().unwrap_or(0),
                    pane_id: parts[4].to_string(),
                    is_active: parts[5] == "1",
                })
            } else {
                None
            }
        })
        .collect();

    Ok(panes)
}

pub fn capture_pane_with_transport(pane_id: &str, transport: &Transport) -> Result<String, String> {
    validate_pane_id(pane_id)?;
    run_tmux_with_transport(
        transport,
        &[
            "capture-pane",
            "-p",
            "-e",
            "-S",
            "-",
            "-E",
            "-",
            "-t",
            pane_id,
        ],
    )
}

pub fn send_keys_with_transport(
    pane_id: &str,
    keys: &str,
    transport: &Transport,
) -> Result<(), String> {
    validate_pane_id(pane_id)?;
    log::debug!(target: "eocc.tmux", "send_keys: pane_id={}, transport={:?}", pane_id, transport);
    let result = run_tmux_with_transport(transport, &["send-keys", "-t", pane_id, keys]);
    log::debug!(target: "eocc.tmux", "send_keys result: {:?}", result);
    result?;
    Ok(())
}

pub fn get_pane_size_with_transport(
    pane_id: &str,
    transport: &Transport,
) -> Result<TmuxPaneSize, String> {
    validate_pane_id(pane_id)?;
    let output = run_tmux_with_transport(
        transport,
        &[
            "display-message",
            "-p",
            "-t",
            pane_id,
            "#{pane_width}x#{pane_height}",
        ],
    )?;
    let trimmed = output.trim();
    let parts: Vec<&str> = trimmed.split('x').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid pane size format: {}", trimmed));
    }
    let width = parts[0]
        .parse()
        .map_err(|_| format!("Invalid width: {}", parts[0]))?;
    let height = parts[1]
        .parse()
        .map_err(|_| format!("Invalid height: {}", parts[1]))?;
    Ok(TmuxPaneSize { width, height })
}
