# Security

This document describes the security model, hardening measures, and threat boundaries for EOCC.

## Network exposure

### API server

The embedded HTTP API server (`api_port` in notification settings) binds to `127.0.0.1` only. It is not reachable from other machines on the network.

To protect against local attacks from other processes, set `api_token` in your notification settings:

```toml
api_port = 8081
api_token = "a-strong-random-token"
```

All requests must then include the header `Authorization: Bearer <token>`. Unauthenticated requests receive a `401` response.

Port values below 1024 are rejected to avoid requiring elevated privileges.

### Request limits

- Request bodies are capped at 1 MB.
- URL-encoded components are capped at 1 KB.

## Input validation

### Hook script

The `eocc-hook` script uses `execFileSync` (array arguments) instead of shell interpolation to prevent command injection. Tmux pane IDs are validated against the pattern `%<digits>` before use.

### SSH transport

When connecting to remote sessions over SSH, hostnames and usernames are validated to reject empty values, leading hyphens, and characters outside the set `[a-zA-Z0-9._:\[\]-]`. This prevents SSH option injection via crafted hostnames.

### Git paths

Repository paths passed to git commands are validated to be absolute paths pointing to existing directories, preventing path traversal.

### Frontend URL parameters

The `tmux_pane` URL parameter is validated with the regex `^%\d+$`. The `project_dir` parameter must start with `/`. Invalid values are silently ignored.

### Tmux viewer input

The hidden text input used for keyboard capture in the tmux viewer enforces a `maxLength` of 4096 characters.

## XSS prevention

Terminal output displayed in the tmux viewer is processed through ANSI-to-HTML conversion and then sanitized with DOMPurify. Only `<span>` tags with `class` attributes are permitted; all other HTML is stripped.

## File permissions

The hook script installed to `~/.local/bin/eocc-hook` is set to mode `0700` (owner read/write/execute only), preventing other users on the system from reading or modifying it.

## Logging

Sensitive data such as tmux key input is not included in log messages. Send-key operations are logged at `debug` level with only the pane ID and transport type.
