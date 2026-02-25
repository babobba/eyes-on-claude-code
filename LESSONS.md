# Lessons Learned

Architectural decisions and insights captured during development.

## Hook-level notification dispatch

**Problem:** The original architecture required the EOCC desktop app (Tauri) to be running locally to dispatch notifications. The hook wrote events to `~/.eocc/logs/events.jsonl`, and the Tauri app's file watcher read that file, detected status transitions, and dispatched to notification channels (ntfy, webhook, pushover). This meant that when Claude Code runs on a remote server or inside a container, notifications could not fire — the events file lives on the remote machine, but the desktop app (and its notification pipeline) runs on the local machine with a display.

**Insight:** The hook itself runs wherever Claude Code runs. It already has all the event information at the moment it fires. There is no reason it cannot read `~/.eocc/notification_settings.toml` and dispatch notifications directly to HTTP-based channels (ntfy, webhook, pushover). The hook does not need the Tauri runtime, Rust notification sinks, or a display server — it just needs HTTP requests to make fire-and-forget POST calls.

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

## TOML parsing without dependencies (historical)

**Problem:** When the hook was a Node.js script using only built-in modules, adding npm dependencies (like a TOML parser) would have required a build step and complicated installation.

**Original decision:** A minimal inline TOML parser was written that handled the specific format produced by the app's `toml::to_string_pretty()`. This is no longer needed — the hook is now a Rust binary that uses the `toml` crate directly.

## Dual notification paths

The notification system now has two independent dispatch paths:

1. **Desktop path** (Tauri app): File watcher → event processing → transition detection → all sinks including desktop notifications. Requires the EOCC app running locally.
2. **Hook path** (eocc-hook): Direct dispatch to HTTP channels (ntfy, webhook, pushover). Works anywhere Claude Code runs.

Both paths read the same `notification_settings.toml` and apply the same rules. They can run simultaneously without conflict — the desktop app handles the dashboard/UI while the hook ensures notifications reach external services regardless of the EOCC app's availability.

## Notification deduplication: runtime coordination vs. static ownership

**Problem:** With dual notification paths (hook + desktop app), both dispatch to the same HTTP channels, causing duplicate notifications when the desktop app is running.

**Implemented solution (runtime coordination):** The hook writes a `notification_result` event to `events.jsonl` after dispatch, containing per-channel `{channel, ok, error}` results. The desktop app reads these results, stores them in `hook_notified_channels` on `AppState`, and filters sinks at dispatch time — skipping channels where the hook already succeeded.

This required:
- A new `NotificationResult` event type and `HookChannelResult` struct across both JS and Rust
- A `notification_results` field on every `EventInfo` (defaulting to empty)
- A `hook_notified_channels` HashMap on both `AppState` structs (eocc-core and main app)
- Clearing logic in 4 places: `SessionStart`, `SessionEnd`, `upsert_session` (on status change), and the result event itself
- A `dispatch_to_sinks` function that accepts a filtered subset of sinks
- Pipeline changes to capture hook results while holding the state lock, then pass them to dispatch

**Race condition:** There is a window between when the desktop reads the status-change event and when the `notification_result` event arrives. During this window the desktop may dispatch a duplicate. This is acceptable (at most one extra notification per transition) but not eliminable with the two-event approach.

**Better approach — static channel ownership:** Instead of runtime coordination, partition channels at configuration time:

```toml
[[channels]]
type = "ntfy"
owner = "hook"    # hook dispatches, desktop skips

[[channels]]
type = "desktop"
owner = "app"     # desktop dispatches, hook skips (can't anyway — no display server)
```

The hook ignores `owner = "app"` channels; the desktop ignores `owner = "hook"` channels. This eliminates:
- The `notification_result` event type entirely
- The `notification_results` field on `EventInfo`
- The `hook_notified_channels` map and all clearing logic
- The sink filtering in the pipeline
- The race condition (no coordination needed)

The ownership model works because the two paths have a natural partition: the hook handles HTTP-based channels (ntfy, webhook, pushover) that work on headless machines, and the desktop handles native OS notifications (sound, badge, desktop notification) that require a display server. There is almost no legitimate case where both should dispatch to the same HTTP endpoint.

**Even better — atomic single-event write:** If runtime coordination is needed, embed the results in the original event instead of writing a second line. The hook would buffer the event, dispatch notifications, then write one line with everything:

```js
const results = await lib.dispatchToChannels(settings, notification);
appendLine(logFile, JSON.stringify({ ...eventPayload, notification_results: results }));
```

This eliminates the race condition entirely — the desktop sees the status change and dispatch results atomically. The tradeoff is the desktop sees events ~1-5s later (HTTP round-trip time), but the hook already handled the urgent channels.

**Separate coordination file:** An alternative to multiplexing into `events.jsonl` is a dedicated `~/.eocc/notification_state.json` (similar to `hook_state.json`) that the desktop reads before dispatching. This keeps `events.jsonl` as a pure session-state stream without notification metadata polluting every `EventInfo`. The desktop checks the file synchronously — no new event types, no extra struct fields.

**Takeaway:** When two processes need to avoid duplicating work, prefer static partitioning (configuration-time decision) over runtime coordination (protocol between processes). Runtime coordination adds types, state, clearing logic, and race windows. Static partitioning adds one config field and a filter in each process's startup path.

## Hook language choice: Node.js vs. Rust (resolved)

**Problem:** The `eocc-hook` was originally a Node.js CommonJS script that duplicated logic already present in the Rust `eocc-core` crate: TOML parsing, session status mapping, transition detection, notification dispatch, pattern matching, and cooldown enforcement. Every new feature (notification channels, filtering rules, project rules) had to be implemented twice and kept in sync across languages.

**Why Node.js was originally chosen:** Claude Code guarantees `node` is available wherever it runs, so the hook had zero installation dependencies beyond copying a single script file. No compilation step, no platform-specific binaries, no runtime to install.

**Resolution:** The hook was rewritten as a Rust binary (`src-tauri/crates/eocc-core/src/bin/eocc-hook.rs`) that imports `eocc-core` directly — sharing types, logic, and the real `toml` crate. The Node.js `eocc-hook` and `eocc-lib.cjs` were deleted. The desktop app's `build.rs` compiles the hook binary and embeds it via `include_bytes!` for auto-installation.

**Result:**
- Single source of truth for all hook logic (types, transition detection, notification dispatch)
- No hand-rolled TOML parser — uses the same `toml` crate as the desktop app
- No Node.js runtime dependency on headless servers
- The `eocc-server` (web dashboard) remains Node.js since it has no shared logic with the core

**Takeaway:** When you have a workspace with a shared logic crate, write companion tools in the same language. A second implementation in a different language creates a maintenance boundary that grows with every feature. The installation convenience of a scripting language is real but finite; the maintenance cost of duplicated logic is ongoing.
