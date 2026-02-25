# Headless Server Installation

The headless server components run without a GUI and are designed for remote machines, containers, and SSH sessions where Claude Code runs unattended.

## Components

| File | Purpose |
|------|---------|
| `eocc-hook` | Compiled Rust binary. Logs events and dispatches notifications. |
| `eocc-server` | Node.js HTTP dashboard and tmux web viewer. |

No npm dependencies required for `eocc-hook` (standalone binary). `eocc-server` uses only Node.js built-in modules.

## Prerequisites

| Requirement | Purpose |
|-------------|---------|
| Node.js 18+ | Runtime for `eocc-server` only |
| tmux (optional) | Required for tmux viewer and pane interaction |
| ttyd (optional) | Full terminal experience in the web viewer |

The `eocc-hook` binary has **no runtime dependencies** — it is a statically-linked Rust binary.

## Installation

### Option 1: Download pre-built binary

Download `eocc-hook` for your platform from the [GitHub releases](https://github.com/joe-re/eyes-on-claude-code/releases) page.

```bash
# Copy the binary and server script to your server
scp eocc-hook eocc-server user@server:~/.local/bin/

# Make them executable
ssh user@server 'chmod +x ~/.local/bin/eocc-hook ~/.local/bin/eocc-server'
```

### Option 2: Build from source

```bash
git clone https://github.com/joe-re/eyes-on-claude-code.git ~/eocc
cd ~/eocc

# Build the hook binary
cargo build --release -p eocc-core --bin eocc-hook --features headless

# Install
cp target/release/eocc-hook ~/.local/bin/eocc-hook
ln -s ~/eocc/eocc-server ~/.local/bin/eocc-server
chmod +x ~/.local/bin/eocc-hook ~/.local/bin/eocc-server
```

### Option 3: cargo install

```bash
cargo install --git https://github.com/joe-re/eyes-on-claude-code \
  --features headless eocc-hook
```

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

The hook binary reads these environment variables set by Claude Code or the user:

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

The hook binary dispatches notifications directly to configured channels (ntfy, webhook, Pushover) without needing the desktop app. See [configuration.md](configuration.md) for notification channel setup.
