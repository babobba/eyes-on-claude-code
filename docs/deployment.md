# Deployment Guide

## Deployment modes

EOCC supports three deployment modes:

| Mode | Components | Use case |
|------|-----------|----------|
| **Desktop only** | Desktop app + hook | Local development on macOS/Linux/Windows |
| **Headless server** | Hook + server scripts | Remote machines, containers, SSH sessions |
| **Hybrid** | Desktop app + remote hook via webhook | Local dashboard monitoring remote sessions |

## Desktop only

Install the desktop app and let it configure hooks automatically.

```bash
# Install from pre-built release (macOS example)
# Download .dmg from https://github.com/joe-re/eyes-on-claude-code/releases

# Or build from source
git clone https://github.com/joe-re/eyes-on-claude-code.git
cd eyes-on-claude-code
pnpm install
pnpm tauri build
```

On first launch, the app presents a setup modal that:
1. Installs `eocc-hook` to the app's data directory
2. Offers to merge hook config into `~/.claude/settings.json`

## Headless server

For remote machines where Claude Code runs in tmux.

### Quick start

```bash
# 1. Copy scripts to the remote machine
scp eocc-hook eocc-server eocc-lib.cjs user@remote:~/.local/bin/

# 2. SSH into the machine and make executable
ssh user@remote
chmod +x ~/.local/bin/eocc-hook ~/.local/bin/eocc-server

# 3. Register hooks with Claude Code
cat > ~/.claude/settings.json << 'EOF'
{
  "hooks": {
    "session_start": [{ "command": "eocc-hook session_start" }],
    "stop": [{ "command": "eocc-hook stop" }],
    "notification": [{ "command": "eocc-hook notification $CLAUDE_NOTIFICATION_TYPE" }],
    "post_tool_use": [{ "command": "eocc-hook post_tool_use $CLAUDE_TOOL_NAME" }],
    "user_prompt_submit": [{ "command": "eocc-hook user_prompt_submit" }]
  }
}
EOF

# 4. Configure notifications
cat > ~/.eocc/notification_settings.toml << 'EOF'
enabled = true

[[channels]]
type = "ntfy"
server = "https://ntfy.sh"
topic = "my-claude-alerts"
EOF

# 5. Start the web dashboard
eocc-server --port 8080
```

### Container deployment

The hook and server scripts have zero npm dependencies. Copy just three files:

```dockerfile
FROM node:24-slim

# Copy EOCC scripts (42 KB total)
COPY eocc-hook eocc-server eocc-lib.cjs /usr/local/bin/
RUN chmod +x /usr/local/bin/eocc-hook /usr/local/bin/eocc-server

# Install tmux for pane viewer (optional)
RUN apt-get update && apt-get install -y tmux && rm -rf /var/lib/apt/lists/*

# Register hooks
RUN mkdir -p /root/.claude && echo '{ \
  "hooks": { \
    "session_start": [{ "command": "eocc-hook session_start" }], \
    "stop": [{ "command": "eocc-hook stop" }], \
    "notification": [{ "command": "eocc-hook notification $CLAUDE_NOTIFICATION_TYPE" }], \
    "post_tool_use": [{ "command": "eocc-hook post_tool_use $CLAUDE_TOOL_NAME" }], \
    "user_prompt_submit": [{ "command": "eocc-hook user_prompt_submit" }] \
  } \
}' > /root/.claude/settings.json

EXPOSE 8080
CMD ["eocc-server", "--port", "8080"]
```

### Docker Compose example

```yaml
services:
  claude-worker:
    image: my-claude-worker
    volumes:
      - eocc-data:/root/.eocc
    environment:
      - EOCC_VIEWER_URL=http://eocc-server:8080

  eocc-server:
    image: node:24-slim
    volumes:
      - eocc-data:/root/.eocc
      - ./eocc-hook:/usr/local/bin/eocc-hook
      - ./eocc-server:/usr/local/bin/eocc-server
      - ./eocc-lib.cjs:/usr/local/bin/eocc-lib.cjs
    ports:
      - "8080:8080"
    command: ["eocc-server", "--port", "8080"]

volumes:
  eocc-data:
```

## Hybrid: Desktop + remote webhook

Monitor remote Claude Code sessions from your local desktop app.

### On the remote machine

Set the `EOCC_WEBHOOK_URL` environment variable so the hook forwards events to your desktop app's API server:

```bash
# In your shell profile (~/.bashrc, ~/.zshrc)
export EOCC_WEBHOOK_URL="http://your-desktop:8081/api/webhook"
```

### On your desktop

Enable the API server in `~/.eocc/notification_settings.toml`:

```toml
enabled = true
api_port = 8081

[[channels]]
type = "desktop"
```

The desktop app's embedded API server receives webhook events and updates the dashboard in real time, just as if the sessions were local.

## Running eocc-server behind a reverse proxy

### Nginx

```nginx
server {
    listen 443 ssl;
    server_name eocc.example.com;

    ssl_certificate     /etc/ssl/certs/eocc.pem;
    ssl_certificate_key /etc/ssl/private/eocc.key;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Caddy

```
eocc.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

When using a reverse proxy, set `external_url` in your notification settings so click-through links point to the public URL:

```toml
external_url = "https://eocc.example.com"
```

## tmux viewer modes

The web viewer at `/tmux/:paneId` supports two modes:

| Mode | Requirement | Experience |
|------|-------------|------------|
| **ttyd** | `ttyd` installed | Full interactive terminal in the browser |
| **Fallback** | tmux only | Polling-based capture with keyboard buttons (Approve, Deny, Ctrl+C) |

To install ttyd:

```bash
# macOS
brew install ttyd

# Ubuntu/Debian
sudo apt-get install ttyd

# From source
git clone https://github.com/tsl0922/ttyd.git
cd ttyd && mkdir build && cd build
cmake .. && make && sudo make install
```

## Monitoring multiple machines

For teams or multi-machine setups, each remote machine runs `eocc-hook` independently. Options for centralized monitoring:

1. **Webhook forwarding**: Each machine sets `EOCC_WEBHOOK_URL` to a central server
2. **ntfy topic**: All machines push to the same ntfy topic
3. **Shared eocc-server**: Mount a shared `~/.eocc` directory (NFS, etc.)

## Security considerations

- `eocc-server` has no authentication. Use a reverse proxy with auth or bind to `127.0.0.1` if exposed.
- The tmux send-keys endpoint (`POST /api/tmux/:paneId/send`) can execute arbitrary keystrokes in tmux panes. Restrict access accordingly.
- Notification settings may contain API tokens. Protect `~/.eocc/notification_settings.toml` with `chmod 600`.
- The webhook payload contains project paths and session IDs. Use HTTPS for webhook URLs.
