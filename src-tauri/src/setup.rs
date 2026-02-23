use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

use crate::settings::get_log_dir;

/// Write content to a file atomically using temp file + rename pattern
fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    let temp_path = path.with_extension("tmp");

    // Write to temp file
    let mut file =
        fs::File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {:?}", e))?;
    file.write_all(content)
        .map_err(|e| format!("Failed to write to temp file: {:?}", e))?;
    file.sync_all()
        .map_err(|e| format!("Failed to sync temp file: {:?}", e))?;
    drop(file);

    // Rename temp file to target (atomic on most filesystems)
    fs::rename(&temp_path, path).map_err(|e| format!("Failed to rename temp file: {:?}", e))?;

    Ok(())
}

/// Hook type constants (matching Claude Code's hook event names)
mod hook_types {
    pub const SESSION_START: &str = "SessionStart";
    pub const SESSION_END: &str = "SessionEnd";
    pub const NOTIFICATION: &str = "Notification";
    pub const STOP: &str = "Stop";
    pub const POST_TOOL_USE: &str = "PostToolUse";
    pub const USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
}

/// Global storage for initialization error (set during app startup)
static INIT_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Set the initialization error (called from main.rs on setup failure)
pub fn set_init_error(error: String) {
    // Use unwrap_or_else to recover from poisoned mutex
    let mut guard = INIT_ERROR.lock().unwrap_or_else(|e| {
        eprintln!("[eocc] Mutex was poisoned, recovering: {:?}", e);
        e.into_inner()
    });
    *guard = Some(error);
}

/// Get the initialization error if any
pub fn get_init_error() -> Option<String> {
    // Use unwrap_or_else to recover from poisoned mutex
    let guard = INIT_ERROR.lock().unwrap_or_else(|e| {
        eprintln!("[eocc] Mutex was poisoned, recovering: {:?}", e);
        e.into_inner()
    });
    guard.clone()
}

/// Embedded script content (relative to src-tauri/src)
const HOOK_SCRIPT: &str = include_str!("../../eocc-hook");
const LIB_SCRIPT: &str = include_str!("../../eocc-lib.cjs");
const SERVER_SCRIPT: &str = include_str!("../../eocc-server");

/// Generate hooks config with the correct hook script path
fn generate_hooks_config(hook_script_path: &str) -> serde_json::Value {
    use hook_types::*;
    serde_json::json!({
        (NOTIFICATION): [
            {
                "matcher": "permission_prompt",
                "hooks": [{ "type": "command", "command": format!("{} notification permission_prompt", hook_script_path) }]
            },
            {
                "matcher": "idle_prompt",
                "hooks": [{ "type": "command", "command": format!("{} notification idle_prompt", hook_script_path) }]
            }
        ],
        (STOP): [
            { "hooks": [{ "type": "command", "command": format!("{} stop", hook_script_path) }] }
        ],
        (POST_TOOL_USE): [
            { "hooks": [{ "type": "command", "command": format!("{} post_tool_use", hook_script_path) }] }
        ],
        (USER_PROMPT_SUBMIT): [
            { "hooks": [{ "type": "command", "command": format!("{} user_prompt_submit", hook_script_path) }] }
        ],
        (SESSION_START): [
            {
                "matcher": "startup",
                "hooks": [{ "type": "command", "command": format!("{} session_start startup", hook_script_path) }]
            },
            {
                "matcher": "resume",
                "hooks": [{ "type": "command", "command": format!("{} session_start resume", hook_script_path) }]
            }
        ],
        (SESSION_END): [
            { "hooks": [{ "type": "command", "command": format!("{} session_end", hook_script_path) }] }
        ]
    })
}

/// Status of each individual hook type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookStatus {
    pub session_start: bool,
    pub session_end: bool,
    pub stop: bool,
    pub post_tool_use: bool,
    pub user_prompt_submit: bool,
    pub notification_permission: bool,
    pub notification_idle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStatus {
    pub hook_installed: bool,
    pub hook_path: String,
    pub hooks: HookStatus,
    pub merged_settings: String,
    pub init_error: Option<String>,
}

/// Get the symlink path for the hook script (avoids spaces in path)
pub fn get_hook_symlink_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Failed to get home directory")?;
    Ok(home.join(".local").join("bin").join("eocc-hook"))
}

/// Get the path to the hook script in the app data directory
pub fn get_hook_script_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {:?}", e))?;
    Ok(app_data_dir.join("eocc-hook"))
}

/// Get the Claude settings file path
pub fn get_claude_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join("settings.json"))
}

/// Install the hook script to the app data directory and create symlink
pub fn install_hook_script(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {:?}", e))?;

    // Create directory if it doesn't exist
    fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data directory: {:?}", e))?;

    let hook_path = app_data_dir.join("eocc-hook");
    let lib_path = app_data_dir.join("eocc-lib.cjs");
    let server_path = app_data_dir.join("eocc-server");

    // Write all scripts atomically
    atomic_write(&hook_path, HOOK_SCRIPT.as_bytes())?;
    atomic_write(&lib_path, LIB_SCRIPT.as_bytes())?;
    atomic_write(&server_path, SERVER_SCRIPT.as_bytes())?;

    // Make scripts executable (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for script_path in [&hook_path, &server_path] {
            let mut perms = fs::metadata(script_path)
                .map_err(|e| format!("Failed to get script permissions: {:?}", e))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(script_path, perms)
                .map_err(|e| format!("Failed to set script permissions: {:?}", e))?;
        }
    }

    // Create symlinks at ~/.local/bin/ (avoids spaces in path)
    #[cfg(unix)]
    {
        let symlink_dir = get_hook_symlink_path()?
            .parent()
            .ok_or("Failed to get symlink parent directory")?
            .to_path_buf();
        fs::create_dir_all(&symlink_dir)
            .map_err(|e| format!("Failed to create symlink directory: {:?}", e))?;

        let symlinks = [
            (&hook_path, symlink_dir.join("eocc-hook")),
            (&lib_path, symlink_dir.join("eocc-lib.cjs")),
            (&server_path, symlink_dir.join("eocc-server")),
        ];

        for (target, symlink_path) in &symlinks {
            if symlink_path.exists() {
                let metadata = fs::symlink_metadata(symlink_path)
                    .map_err(|e| format!("Failed to read symlink metadata: {:?}", e))?;
                if metadata.file_type().is_symlink() {
                    fs::remove_file(symlink_path)
                        .map_err(|e| format!("Failed to remove existing symlink: {:?}", e))?;
                } else {
                    return Err(format!(
                        "Path {} exists and is not a symlink. Please remove it manually.",
                        symlink_path.display()
                    ));
                }
            }
            std::os::unix::fs::symlink(target, symlink_path)
                .map_err(|e| format!("Failed to create symlink: {:?}", e))?;
        }
    }

    Ok(hook_path)
}

/// Check if the hook script is installed
pub fn is_hook_installed(app: &tauri::AppHandle) -> bool {
    get_hook_script_path(app)
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Check if a hook command contains our hook script
fn is_eocc_hook_command(command: &str) -> bool {
    // Check for eocc-hook as a standalone command or at end of a path
    // Valid patterns:
    //   "eocc-hook ..."           - command starts with eocc-hook
    //   "/path/to/eocc-hook ..."  - path ending with /eocc-hook
    //   "~/.local/bin/eocc-hook"  - tilde path
    //   ".../eocc-hook"           - ends with /eocc-hook
    command.starts_with("eocc-hook ")
        || command == "eocc-hook"
        || command.contains("/eocc-hook ")
        || command.ends_with("/eocc-hook")
}

/// Check if a hook array contains eocc-hook command, optionally with a specific matcher
fn has_eocc_hook_in_array(hooks_array: &serde_json::Value, required_matcher: Option<&str>) -> bool {
    let Some(arr) = hooks_array.as_array() else {
        return false;
    };

    for hook_entry in arr {
        // Check matcher if required
        if let Some(matcher) = required_matcher {
            let entry_matcher = hook_entry.get("matcher").and_then(|m| m.as_str());
            if entry_matcher != Some(matcher) {
                continue;
            }
        }

        // Each entry can have a "hooks" array with commands
        if let Some(hooks) = hook_entry.get("hooks") {
            if let Some(hooks_arr) = hooks.as_array() {
                for hook in hooks_arr {
                    if let Some(command) = hook.get("command").and_then(|c| c.as_str()) {
                        if is_eocc_hook_command(command) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check each hook type in Claude settings.json and return detailed status
pub fn check_claude_settings() -> HookStatus {
    let default_status = HookStatus {
        session_start: false,
        session_end: false,
        stop: false,
        post_tool_use: false,
        user_prompt_submit: false,
        notification_permission: false,
        notification_idle: false,
    };

    let Some(settings_path) = get_claude_settings_path() else {
        return default_status;
    };

    if !settings_path.exists() {
        return default_status;
    }

    let content = match fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(_) => return default_status,
    };

    // Strip JSONC comments before parsing
    let json_content = strip_jsonc_comments(&content);

    let settings: serde_json::Value = match serde_json::from_str(&json_content) {
        Ok(v) => v,
        Err(_) => return default_status,
    };

    // Check if hooks section exists
    let Some(hooks) = settings.get("hooks") else {
        return default_status;
    };

    use hook_types::*;

    // Check each hook type
    let session_start = hooks
        .get(SESSION_START)
        .map(|h| has_eocc_hook_in_array(h, None))
        .unwrap_or(false);

    let session_end = hooks
        .get(SESSION_END)
        .map(|h| has_eocc_hook_in_array(h, None))
        .unwrap_or(false);

    let stop = hooks
        .get(STOP)
        .map(|h| has_eocc_hook_in_array(h, None))
        .unwrap_or(false);

    let post_tool_use = hooks
        .get(POST_TOOL_USE)
        .map(|h| has_eocc_hook_in_array(h, None))
        .unwrap_or(false);

    let user_prompt_submit = hooks
        .get(USER_PROMPT_SUBMIT)
        .map(|h| has_eocc_hook_in_array(h, None))
        .unwrap_or(false);

    let notification_permission = hooks
        .get(NOTIFICATION)
        .map(|h| has_eocc_hook_in_array(h, Some("permission_prompt")))
        .unwrap_or(false);

    let notification_idle = hooks
        .get(NOTIFICATION)
        .map(|h| has_eocc_hook_in_array(h, Some("idle_prompt")))
        .unwrap_or(false);

    HookStatus {
        session_start,
        session_end,
        stop,
        post_tool_use,
        user_prompt_submit,
        notification_permission,
        notification_idle,
    }
}

/// Strip JSONC comments (// and /* */) from content
fn strip_jsonc_comments(content: &str) -> String {
    let mut result = String::new();
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if !in_string && c == '/' {
            if let Some(&next) = chars.peek() {
                if next == '/' {
                    // Line comment - skip until newline
                    chars.next();
                    while let Some(&ch) = chars.peek() {
                        if ch == '\n' {
                            break;
                        }
                        chars.next();
                    }
                    continue;
                } else if next == '*' {
                    // Block comment - skip until */
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '*' {
                            if let Some(&'/') = chars.peek() {
                                chars.next();
                                break;
                            }
                        }
                    }
                    continue;
                }
            }
        }

        result.push(c);
    }

    result
}

/// Merge hook arrays, replacing entries that match eocc-hook pattern
fn merge_hook_array(
    existing: Option<&serde_json::Value>,
    new_hooks: &serde_json::Value,
) -> serde_json::Value {
    let mut result: Vec<serde_json::Value> = Vec::new();

    // Keep existing hooks that don't contain eocc-hook
    if let Some(serde_json::Value::Array(existing_arr)) = existing {
        for hook in existing_arr {
            let hook_str = hook.to_string();
            // Skip old eocc hooks (will be replaced with new ones)
            if !hook_str.contains("eocc-hook") && !hook_str.contains("claude-monitor-hook") {
                result.push(hook.clone());
            }
        }
    }

    // Add new eocc hooks
    if let serde_json::Value::Array(new_arr) = new_hooks {
        for hook in new_arr {
            result.push(hook.clone());
        }
    }

    serde_json::Value::Array(result)
}

/// Generate merged settings JSON (existing settings + hooks)
pub fn generate_merged_settings(hook_script_path: &str) -> Result<String, String> {
    let new_hooks_config = generate_hooks_config(hook_script_path);

    let settings_path = get_claude_settings_path();
    let mut settings: serde_json::Value = if let Some(path) = &settings_path {
        if path.exists() {
            let content = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read settings: {:?}", e))?;
            // Strip JSONC comments before parsing
            let json_content = strip_jsonc_comments(&content);
            serde_json::from_str(&json_content)
                .map_err(|e| format!("Failed to parse settings: {:?}", e))?
        } else {
            serde_json::json!({})
        }
    } else {
        serde_json::json!({})
    };

    // Deep merge hooks - preserve existing hook types we don't configure
    let existing_hooks = settings.get("hooks").cloned();
    let mut merged_hooks = existing_hooks
        .as_ref()
        .and_then(|h| h.as_object().cloned())
        .unwrap_or_default();

    // Merge each hook type from our config
    if let Some(new_hooks_obj) = new_hooks_config.as_object() {
        for (hook_type, new_hook_array) in new_hooks_obj {
            let existing_array = existing_hooks.as_ref().and_then(|h| h.get(hook_type));
            merged_hooks.insert(
                hook_type.clone(),
                merge_hook_array(existing_array, new_hook_array),
            );
        }
    }

    // Update settings with merged hooks
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("hooks".to_string(), serde_json::Value::Object(merged_hooks));
    }

    serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {:?}", e))
}

/// Get the full setup status
pub fn get_setup_status(app: &tauri::AppHandle) -> SetupStatus {
    // Use tilde path for settings (portable and no spaces)
    let tilde_path = "~/.local/bin/eocc-hook".to_string();

    let hook_installed = is_hook_installed(app);
    let hooks = check_claude_settings();

    let merged_settings = generate_merged_settings(&tilde_path)
        .unwrap_or_else(|e| serde_json::json!({"error": e}).to_string());

    let init_error = get_init_error();

    SetupStatus {
        hook_installed,
        hook_path: tilde_path,
        hooks,
        merged_settings,
        init_error,
    }
}

/// Initialize setup: install hook script, create log directory
pub fn initialize_setup(app: &tauri::AppHandle) -> Result<(), String> {
    // Always install/update hook script to ensure latest version
    install_hook_script(app)?;

    // Create log directory (~/.eocc/logs)
    let log_dir = get_log_dir(app)?;
    fs::create_dir_all(&log_dir).map_err(|e| format!("Failed to create log directory: {:?}", e))?;

    // Create empty events file if it doesn't exist
    let events_file = log_dir.join("events.jsonl");
    if !events_file.exists() {
        fs::write(&events_file, "")
            .map_err(|e| format!("Failed to create events file: {:?}", e))?;
    }

    Ok(())
}
