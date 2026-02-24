# Configuration Reference

EOCC uses two configuration files:

| File | Format | Purpose |
|------|--------|---------|
| `~/.eocc/notification_settings.toml` | TOML | Notification channels, rules, templates |
| `<app-data>/settings.json` | JSON | Desktop app preferences (managed by the app UI) |

Claude Code's own settings are in `~/.claude/settings.json` (hook registration).

## Notification settings

File: `~/.eocc/notification_settings.toml`

This file is used by both the desktop app and the headless hook script. Changes are hot-reloaded by the desktop app.

### Minimal example

```toml
enabled = true

[[channels]]
type = "ntfy"
server = "https://ntfy.sh"
topic = "my-claude-alerts"
```

### Full reference

```toml
# Master enable/disable switch
enabled = true

# Which status transitions trigger notifications
# Options: "waiting_permission", "waiting_input", "completed", "active"
# Default: ["waiting_permission", "waiting_input", "completed"]
notify_on = ["waiting_permission", "waiting_input", "completed"]

# Minimum seconds between notifications for the same session
# Prevents notification spam during rapid status changes
cooldown_seconds = 30

# Custom title template (optional)
# Available variables: {project_name}, {project_dir}, {status}, {emoji}, {message}, {priority}
title_template = "{emoji} {project_name} - {status}"

# Custom body template (optional)
# Additional variables: {connect_url}, {transport_type}, {tmux_session}, {tmux_pane}
body_template = "{message}"

# Desktop app API server port (optional)
# Enables the embedded HTTP API for external integrations
# Must be >= 1024; ports below 1024 are rejected
api_port = 8081

# Bearer token for API server authentication (optional)
# When set, all API requests must include an Authorization header:
#   Authorization: Bearer <token>
api_token = "my-secret-token"

# External URL for click-through links in notifications (optional)
# Points to your eocc-server instance
external_url = "https://eocc.example.com"

# --- Notification channels ---

# ntfy (https://ntfy.sh)
[[channels]]
type = "ntfy"
server = "https://ntfy.sh"     # or your self-hosted instance
topic = "my-claude-alerts"
token = "tk_abc123"            # optional: access token for private topics

# Webhook (Slack, Discord, custom endpoints)
[[channels]]
type = "webhook"
url = "https://hooks.slack.com/services/T00/B00/xxx"

# Pushover (https://pushover.net)
[[channels]]
type = "pushover"
user_key = "u1234567890"
app_token = "a1234567890"
device = "my-phone"            # optional: target specific device

# Desktop notifications (desktop app only, ignored by hook script)
[[channels]]
type = "desktop"

# --- Per-project rules ---
# Rules are checked in order; the first matching rule wins.

# Disable notifications for noisy projects
[[project_rules]]
pattern = "**/scratch-project"
enabled = false

# Override notify_on for important projects
[[project_rules]]
pattern = "**/production-api"
notify_on = ["waiting_permission", "waiting_input", "completed", "active"]

# Match by directory prefix
[[project_rules]]
pattern = "/home/user/work/**"
notify_on = ["waiting_permission", "completed"]
```

### Channel types

#### ntfy

Push notifications via [ntfy.sh](https://ntfy.sh) or a self-hosted ntfy server.

```toml
[[channels]]
type = "ntfy"
server = "https://ntfy.sh"     # required
topic = "my-topic"             # required
token = "tk_secret"            # optional
```

- Supports priority mapping (waiting = high, completed = normal, active = low)
- Supports click-through URLs when `external_url` is configured
- Tags are set automatically based on status (lock, hourglass, check mark)

#### webhook

Posts a JSON payload to any HTTP endpoint. Works with Slack, Discord, or custom services.

```toml
[[channels]]
type = "webhook"
url = "https://hooks.slack.com/services/T00/B00/xxx"  # required
```

Payload format:

```json
{
  "text": "Title\nBody",
  "project_name": "my-project",
  "project_dir": "/home/user/my-project",
  "status": "waiting_permission",
  "priority": "high",
  "connect_url": "ssh://user@host",
  "tmux_session": "main",
  "tmux_pane": "%3"
}
```

#### pushover

Push notifications via [Pushover](https://pushover.net).

```toml
[[channels]]
type = "pushover"
user_key = "u1234567890"       # required
app_token = "a1234567890"      # required
device = "my-phone"            # optional
```

#### desktop

Native OS notification popups. Only works in the desktop app (ignored by the hook script).

```toml
[[channels]]
type = "desktop"
```

### Pattern matching

Project rules use glob-like patterns:

| Pattern | Matches |
|---------|---------|
| `**/name` | Any path ending with `/name` |
| `/prefix/**` | Any path starting with `/prefix/` |
| `/path/prefix*` | Any path starting with `/path/prefix` |
| `*substring*` | Any path containing `substring` |
| `/exact/path` | Exact match only |

### Template variables

Available in `title_template` and `body_template`:

| Variable | Description | Example |
|----------|-------------|---------|
| `{project_name}` | Project directory basename | `my-project` |
| `{project_dir}` | Full project path | `/home/user/my-project` |
| `{status}` | Human-readable status | `Waiting for permission` |
| `{emoji}` | Status emoji | `🔐` |
| `{message}` | Event message (tool name, etc.) | `Approve bash command` |
| `{priority}` | Notification priority | `high` |
| `{connect_url}` | SSH/mosh connect URL | `ssh://user@host` |
| `{transport_type}` | Transport type | `ssh` |
| `{tmux_session}` | tmux session name | `main` |
| `{tmux_pane}` | tmux pane ID | `%3` |

## Claude Code hook settings

File: `~/.claude/settings.json`

Register the EOCC hook to receive events from Claude Code:

```json
{
  "hooks": {
    "session_start": [
      { "command": "eocc-hook session_start" }
    ],
    "stop": [
      { "command": "eocc-hook stop" }
    ],
    "notification": [
      { "command": "eocc-hook notification $CLAUDE_NOTIFICATION_TYPE" }
    ],
    "post_tool_use": [
      { "command": "eocc-hook post_tool_use $CLAUDE_TOOL_NAME" }
    ],
    "user_prompt_submit": [
      { "command": "eocc-hook user_prompt_submit" }
    ]
  }
}
```

The desktop app can install these hooks automatically via the setup modal on first launch.

## Desktop app settings

Managed by the app UI and stored in the platform-specific app data directory:

| Setting | Default | Description |
|---------|---------|-------------|
| `always_on_top` | `false` | Keep dashboard window above other windows |
| `minimum_mode_enabled` | `true` | Show compact view when window is unfocused |
| `sound_enabled` | `true` | Play sounds on status changes |
| `opacity_active` | `1.0` | Window opacity when focused (0.1 - 1.0) |
| `opacity_inactive` | `0.3` | Window opacity when unfocused (0.1 - 1.0) |
