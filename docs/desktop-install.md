# Desktop App Installation

## Prerequisites

### All platforms

| Requirement | Version | Purpose |
|-------------|---------|---------|
| Node.js | 24+ | Frontend build tooling |
| pnpm | 10.28.1 | Package manager (pinned) |
| Rust | stable | Backend compilation |

### macOS

No additional system libraries required. Xcode command line tools must be installed:

```bash
xcode-select --install
```

### Linux (Ubuntu/Debian)

System libraries required for WebKit and GTK:

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  librsvg2-dev \
  libssl-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev
```

For building release bundles, also install:

```bash
sudo apt-get install -y patchelf
```

### Windows

No additional system libraries required. The MSVC build tools must be installed via Visual Studio.

## Installation from source

```bash
# Clone the repository
git clone https://github.com/joe-re/eyes-on-claude-code.git
cd eyes-on-claude-code

# Install Node.js dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build a release binary
pnpm tauri build
```

Release binaries are written to `src-tauri/target/release/bundle/`.

## Runtime dependencies (npm)

These packages are bundled into the frontend at build time:

| Package | Version | Purpose |
|---------|---------|---------|
| `@tauri-apps/api` | ^2.0.0 | Tauri IPC bridge (invoke commands, listen to events, manage windows) |
| `react` | ^19.0.0 | UI framework |
| `react-dom` | ^19.0.0 | React DOM renderer |
| `ansi_up` | ^6.0.6 | Convert ANSI escape sequences to HTML (tmux viewer) |

## Dev dependencies (npm)

These are only needed during development and CI:

| Package | Purpose |
|---------|---------|
| `@tauri-apps/cli` | Tauri CLI (`pnpm tauri dev/build`) |
| `vite` + `@vitejs/plugin-react` | Build tool and React plugin |
| `tailwindcss` + `@tailwindcss/vite` | Utility-first CSS |
| `typescript` | Type checking |
| `eslint` + plugins | Linting (react-hooks, react-refresh, prettier compat) |
| `prettier` | Code formatting |
| `vitest` + `jsdom` | Testing framework |
| `@testing-library/react` + `jest-dom` + `user-event` | React component testing |
| `@types/react` + `@types/react-dom` | TypeScript type definitions |

## Rust dependencies

These are compiled into the native binary:

| Crate | Purpose |
|-------|---------|
| `tauri` | App framework (window, tray, webview, menu, IPC) |
| `tauri-plugin-shell` | Shell command execution from frontend |
| `tauri-plugin-log` | Rotating file-based logging |
| `tauri-build` | Build-time code generation |
| `eocc-core` | Shared business logic (events, state, notifications) |
| `serde` + `serde_json` | JSON serialization/deserialization |
| `notify` | File system watcher (monitors `events.jsonl`) |
| `dirs` | Platform-specific config/home directories |
| `opener` | Open files/URLs in default application |
| `anyhow` | Error handling |
| `urlencoding` | URL-encode tmux pane IDs for webview URLs |
| `tiny_http` | Embedded HTTP API server |
| `log` | Logging facade |

### eocc-core sub-crate

| Crate | Feature | Purpose |
|-------|---------|---------|
| `serde` + `serde_json` | always | Type serialization |
| `toml` | always | Notification settings file parsing |
| `log` | always | Logging |
| `ureq` | `ntfy`, `webhook`, `pushover` | HTTP client for notification channels |
| `notify-rust` | `desktop_notifications` | Native OS notification popups |
| `env_logger` | `headless` | Console logging for headless watcher binary |

## Installed binary size

| Platform | Approximate size |
|----------|-----------------|
| macOS (aarch64) | ~15 MB |
| Linux (x64) | ~20 MB |
| Windows (x64) | ~20 MB |

The release profile uses `strip = true`, `lto = true`, and `codegen-units = 1` to minimize binary size.

## Pre-built releases

Download pre-built binaries from [GitHub Releases](https://github.com/joe-re/eyes-on-claude-code/releases). Available formats:

- **macOS**: `.dmg` (Apple Silicon)
- **Linux**: `.deb`, `.AppImage` (x64)
- **Windows**: `.msi`, `.exe` (x64)
