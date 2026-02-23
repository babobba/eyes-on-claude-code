# Headless Server Installation

The headless server components run without a GUI and are designed for remote machines, containers, and SSH sessions where Claude Code runs unattended.

## Components

| File | Size | Purpose |
|------|------|---------|
| `eocc-hook` | ~9 KB | Claude Code hook script. Logs events and dispatches notifications. |
| `eocc-server` | ~20 KB | HTTP dashboard and tmux web viewer. |
| `eocc-lib.cjs` | ~14 KB | Shared library (TOML parsing, notification dispatch, HTTP helpers). |

**Total: ~42 KB**. No `npm install` required.

## Prerequisites

| Requirement | Purpose |
|-------------|---------|
| Node.js 18+ | Runtime for hook and server scripts |
| tmux (optional) | Required for tmux viewer and pane interaction |
| ttyd (optional) | Full terminal experience in the web viewer |

All three scripts use only Node.js built-in modules (`fs`, `path`, `os`, `http`, `https`, `child_process`). There are no npm dependencies.

## Installation

### Option 1: Copy scripts manually

```bash
# Copy the three files to your server
scp eocc-hook eocc-server eocc-lib.cjs user@server:~/.local/bin/

# Make the scripts executable
ssh user@server 'chmod +x ~/.local/bin/eocc-hook ~/.local/bin/eocc-server'
```

### Option 2: Clone and symlink

```bash
git clone https://github.com/joe-re/eyes-on-claude-code.git ~/eocc
ln -s ~/eocc/eocc-hook ~/.local/bin/eocc-hook
ln -s ~/eocc/eocc-server ~/.local/bin/eocc-server
ln -s ~/eocc/eocc-lib.cjs ~/.local/bin/eocc-lib.cjs
```

**Important**: `eocc-hook` requires `eocc-lib.cjs` to be in the same directory (it uses `require("./eocc-lib.cjs")`).

## Hook setup

Register the hook in Claude Code's settings file (`~/.claude/settings.json`):

```json
{
  "hooks": {
    "session_start": [{ "command": "eocc-hook session_start" }],
    "stop": [{ "command": "eocc-hook stop" }],
    "notification": [{ "command": "eocc-hook notification $CLAUDE_NOTIFICATION_TYPE" }],
    "post_tool_use": [{ "command": "eocc-hook post_tool_use $CLAUDE_TOOL_NAME" }],
    "user_prompt_submit": [{ "command": "eocc-hook user_prompt_submit" }]
  }
}
```

The hook reads from stdin (Claude passes JSON context) and appends events to `~/.eocc/logs/events.jsonl`.

## Running the server

```bash
# Start with defaults (port 8080, bind 0.0.0.0)
eocc-server

# Custom port and host
eocc-server --port 3000 --host 127.0.0.1

# Using environment variables
EOCC_PORT=3000 EOCC_HOST=127.0.0.1 eocc-server
```

### Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `EOCC_PORT` | `8080` | HTTP listen port |
| `EOCC_HOST` | `0.0.0.0` | HTTP bind address |
| `EOCC_DIR` | `~/.eocc` | Override EOCC data directory |

### Server endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Session dashboard (HTML, auto-refreshes every 3s) |
| GET | `/tmux/:paneId` | tmux web viewer (ttyd redirect or fallback HTML) |
| GET | `/api/sessions` | Current session state (JSON) |
| GET | `/api/tmux/:paneId/capture` | Capture tmux pane content (plain text) |
| POST | `/api/tmux/:paneId/send` | Send keys to tmux pane (JSON body: `{"keys": "..."}`) |

### Running as a systemd service

```ini
# ~/.config/systemd/user/eocc-server.service
[Unit]
Description=Eyes on Claude Code Server
After=network.target

[Service]
ExecStart=/home/user/.local/bin/eocc-server --port 8080
Restart=on-failure
RestartSec=5
Environment=PATH=/usr/local/bin:/usr/bin:/bin

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now eocc-server
```

## Hook environment variables

The hook script reads these environment variables set by Claude Code or the user:

| Variable | Set by | Purpose |
|----------|--------|---------|
| `CLAUDE_PROJECT_DIR` | Claude Code | Current project directory |
| `TMUX_PANE` | tmux | Current tmux pane ID (e.g., `%3`) |
| `SSH_CONNECTION` | sshd | Auto-detects SSH transport |
| `EOCC_TRANSPORT` | user | Override transport type (`ssh`, `mosh`, `tailscale`, `local`) |
| `EOCC_TRANSPORT_HOST` | user | Override remote host |
| `EOCC_TRANSPORT_PORT` | user | Override SSH port |
| `EOCC_TRANSPORT_USER` | user | Override remote user |
| `EOCC_WEBHOOK_URL` | user | Send events to a webhook endpoint |
| `EOCC_VIEWER_URL` | user | Base URL for the eocc-server web viewer |

## Notifications without the desktop app

The hook script dispatches notifications directly to configured channels (ntfy, webhook, Pushover) without needing the desktop app. See [configuration.md](configuration.md) for notification channel setup.
