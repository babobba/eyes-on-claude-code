# Lessons Learned

Architectural decisions and insights captured during development.

## Hook-level notification dispatch

**Problem:** The original architecture required the EOCC desktop app (Tauri) to be running locally to dispatch notifications. The hook wrote events to `~/.eocc/logs/events.jsonl`, and the Tauri app's file watcher read that file, detected status transitions, and dispatched to notification channels (ntfy, webhook, pushover). This meant that when Claude Code runs on a remote server or inside a container, notifications could not fire — the events file lives on the remote machine, but the desktop app (and its notification pipeline) runs on the local machine with a display.

**Insight:** The hook itself runs wherever Claude Code runs. It already has all the event information at the moment it fires. There is no reason it cannot read `~/.eocc/notification_settings.toml` and dispatch notifications directly to HTTP-based channels (ntfy, webhook, pushover). The hook does not need the Tauri runtime, Rust notification sinks, or a display server — it just needs Node.js built-in `http`/`https` modules to make fire-and-forget POST requests.

**Decision:** Add a direct notification dispatch path to `eocc-hook` that:
1. Parses `~/.eocc/notification_settings.toml` using a minimal inline TOML parser (no npm dependencies)
2. Tracks session state transitions in `~/.eocc/hook_state.json` (since the hook is stateless — it runs once per event and exits)
3. Mirrors the Rust transition detection logic: maps event types to session statuses, checks `notify_on` filters, applies per-project rules, and enforces cooldown
4. Fires HTTP requests to ntfy, webhook, and pushover channels (skips desktop — no display server on headless hosts)
5. All operations are best-effort and wrapped in try/catch to never block or fail the hook

**Tradeoffs:**
- The hook now has duplicated logic (TOML parsing, pattern matching, status mapping) that also exists in the Rust core. This is intentional — the hook must be self-contained with zero external dependencies.
- The TOML parser is minimal and handles only the subset used by `notification_settings.toml`. It is not a general-purpose TOML parser.
- Desktop notifications (`notify-rust`) cannot be dispatched from the hook since headless machines have no display server. The desktop channel only works via the Tauri app.
- The file-based event pipeline (`events.jsonl` → file watcher → Tauri app) is still needed for the dashboard UI, tray icon, tmux viewer, and other desktop-app features. The hook dispatch is an additional, independent notification path.

**Result:** Notifications work everywhere Claude Code runs — local machines, SSH sessions, containers, CI environments — as long as `notification_settings.toml` is configured with HTTP-based channels.

## Stateless hook with file-backed state

**Problem:** The hook runs once per event invocation and exits. Notification dispatch requires knowing the *previous* session status to detect transitions (e.g., Active → WaitingPermission). Without state, the hook would either fire on every event or not be able to detect meaningful transitions.

**Decision:** Use a lightweight JSON file (`~/.eocc/hook_state.json`) to persist per-session status and cooldown timestamps between hook invocations. The file is small, reads/writes are synchronous, and atomic writes (write to temp file, then rename) prevent corruption from concurrent hook invocations across multiple Claude sessions.

**Tradeoff:** Multiple concurrent Claude sessions could race on this file. The worst case is a duplicate notification or a missed one — both acceptable for a best-effort system.

## TOML parsing without dependencies

**Problem:** The hook uses CommonJS with only Node.js built-in modules (`node:fs`, `node:path`, `node:os`, `node:http`, `node:https`). Adding npm dependencies (like a TOML parser) would require a build step and complicate installation.

**Decision:** Write a minimal inline TOML parser that handles the specific format produced by the app's `toml::to_string_pretty()`:
- Simple key-value pairs (booleans, integers, quoted strings)
- Inline arrays (`["a", "b"]`)
- Array-of-tables sections (`[[channels]]`, `[[project_rules]]`)
- Comment stripping (respecting quoted strings)

This is sufficient for `notification_settings.toml` and avoids any external dependency.

## Dual notification paths

The notification system now has two independent dispatch paths:

1. **Desktop path** (Tauri app): File watcher → event processing → transition detection → all sinks including desktop notifications. Requires the EOCC app running locally.
2. **Hook path** (eocc-hook): Direct dispatch to HTTP channels (ntfy, webhook, pushover). Works anywhere Claude Code runs.

Both paths read the same `notification_settings.toml` and apply the same rules. They can run simultaneously without conflict — the desktop app handles the dashboard/UI while the hook ensures notifications reach external services regardless of the EOCC app's availability.
