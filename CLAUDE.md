# CLAUDE.md

## Project overview

Eyes on Claude Code (EOCC) is a Tauri v2 desktop application that monitors Claude Code sessions via global hooks. It provides a menubar/tray icon and a dashboard UI for tracking multiple Claude Code sessions across projects. Key features include session state monitoring, git diff viewing (via difit), tmux pane integration, and notification sounds.

**Repository**: `joe-re/eyes-on-claude-code`
**License**: MIT

## Architecture

This is a **Tauri v2** app with a React + TypeScript frontend and a Rust backend.

### Directory structure

```
├── src/                    # Frontend (React + TypeScript)
│   ├── App.tsx             # Root component: dashboard layout, session list, setup modal
│   ├── main.tsx            # React entry point
│   ├── index.css           # Global styles (Tailwind CSS v4)
│   ├── components/         # React components
│   │   ├── SessionCard.tsx # Individual session display with git info and actions
│   │   ├── SessionList.tsx # Session list container
│   │   ├── SetupModal.tsx  # First-launch hook installation UI
│   │   ├── Header.tsx      # Dashboard header with status counts
│   │   ├── MinimumView.tsx # Compact view when window is unfocused
│   │   ├── TmuxViewer.tsx  # Tmux pane viewer with keyboard input
│   │   ├── DiffButton.tsx  # Diff launcher button
│   │   ├── BranchCombobox.tsx # Branch selector for diff comparison
│   │   ├── StatCard.tsx    # Status count display
│   │   ├── EmptyState.tsx  # Empty state placeholder
│   │   └── icons.tsx       # SVG icon components
│   ├── context/            # React context for app state
│   │   ├── AppContext.tsx   # Provider: fetches data, listens to Tauri events, plays sounds
│   │   ├── appContextStore.ts # Context definition and defaults
│   │   └── useAppContext.ts   # Context hook
│   ├── hooks/              # Custom React hooks
│   │   ├── useWindowDrag.ts   # Window drag behavior
│   │   └── useWindowOpacity.ts # Opacity management on focus/blur
│   ├── lib/                # Utility modules
│   │   ├── tauri.ts        # Tauri command invocations and event listeners
│   │   ├── audio.ts        # Sound effect playback
│   │   └── utils.ts        # General helpers
│   └── types/
│       └── index.ts        # TypeScript type definitions (mirrors Rust types)
├── src-tauri/              # Backend (Rust + Tauri)
│   ├── src/
│   │   ├── main.rs         # App entry point: window creation, tray, file watcher setup
│   │   ├── state.rs        # Core data types: AppState, SessionInfo, Settings, enums
│   │   ├── events.rs       # Event queue reading and processing (events.jsonl)
│   │   ├── commands.rs     # Tauri command handlers (invoked from frontend)
│   │   ├── setup.rs        # Hook installation, Claude settings merging
│   │   ├── menu.rs         # App menu and tray menu construction
│   │   ├── tray.rs         # Tray icon updates and badge management
│   │   ├── git.rs          # Git operations (branch, status, commits)
│   │   ├── difit.rs        # Difit process management for diff viewing
│   │   ├── tmux.rs         # Tmux pane interaction commands
│   │   ├── persist.rs      # Runtime state persistence (save/load sessions)
│   │   ├── settings.rs     # Settings file I/O, directory resolution
│   │   └── constants.rs    # Shared constants (icon data, window sizes)
│   ├── Cargo.toml          # Rust dependencies
│   ├── tauri.conf.json     # Tauri configuration
│   └── icons/              # App icons for all platforms
├── app/src-tauri/src/
│   └── persist.rs          # Standalone persist module (used by app build)
├── eocc-hook               # Node.js hook script (installed to ~/.local/bin/eocc-hook)
├── scripts/
│   ├── audit-licenses.mjs  # License compatibility audit script
│   ├── generate-icons.sh   # Icon generation helper
│   └── setup-macos-codesign.sh # macOS code signing setup
├── .github/workflows/
│   ├── lint.yml            # CI: ESLint, Prettier, TypeScript, cargo fmt, clippy
│   └── release.yml         # CI: Multi-platform build and GitHub release
└── docs/
    └── macos-notarization-setup.md
```

### Data flow

1. Claude Code triggers hooks defined in `~/.claude/settings.json`
2. The `eocc-hook` Node.js script appends events as JSON lines to `~/.eocc/logs/events.jsonl`
3. The Rust backend watches `events.jsonl` via the `notify` crate file watcher
4. Events are atomically renamed to a processing file, parsed, and applied to `AppState`
5. State changes are emitted to the frontend via Tauri events (`state-updated`)
6. The React frontend updates the dashboard in real time

### Key types

Frontend types in `src/types/index.ts` mirror Rust types in `src-tauri/src/state.rs`. Session statuses: `Active`, `WaitingPermission`, `WaitingInput`, `Completed`. When modifying these types, update both sides.

## Important rules

- Write Git commit messages in English.
- Write comments in English.
- Do not add comments when the code is concise and its meaning is clearly understandable.

## Tech stack

| Layer | Technology |
|-------|-----------|
| Framework | Tauri v2 |
| Frontend | React 19, TypeScript, Vite 7 |
| Styling | Tailwind CSS v4 (via `@tailwindcss/vite` plugin) |
| Backend | Rust (2021 edition) |
| Package manager | pnpm 10.28.1 (pinned in `.tool-versions` and `packageManager` field) |
| Linting (TS) | ESLint 9 (flat config), Prettier |
| Linting (Rust) | `cargo fmt`, `cargo clippy` |

## Development commands

```bash
# Install dependencies
pnpm install

# Run in development mode (starts both Vite dev server and Tauri)
pnpm tauri dev

# Build for production
pnpm tauri build

# Frontend only (Vite dev server on port 1420)
pnpm dev

# TypeScript + Vite build (frontend only)
pnpm build
```

### Linting and formatting

```bash
# Run all linters (TS + Rust)
pnpm lint

# TypeScript linting
pnpm lint:ts
pnpm lint:ts:fix

# TypeScript type checking
pnpm typecheck

# Rust linting
pnpm lint:rust
pnpm lint:rust:fix

# Prettier formatting
pnpm format          # write
pnpm format:check    # check only
```

### Other scripts

```bash
# License audit
pnpm licenses:audit

# Icon generation
./scripts/generate-icons.sh
```

## Code style and conventions

### TypeScript / React

- **Path aliases**: Use `@/` to reference `src/` (configured in `tsconfig.json` and `vite.config.ts`)
- **Strict mode**: TypeScript strict mode is enabled with `noUnusedLocals` and `noUnusedParameters`
- **Formatting**: Prettier with single quotes, semicolons, trailing commas (`es5`), 100 char line width
- **Components**: Functional components with hooks. Keep components small and readable.
- **Tauri bridge**: All Tauri `invoke` calls and event listeners are centralized in `src/lib/tauri.ts`
- **State management**: React Context (`AppProvider`) with Tauri event listeners, no external state library

### Rust

- **Error handling**: Use `anyhow::Result` for error propagation. Avoid panics in runtime paths.
- **Serialization**: `serde` with `rename_all = "snake_case"` for enum variants to match the JSON event format
- **Tauri commands**: Defined in `src-tauri/src/commands.rs`, registered in `main.rs`
- **State**: Shared via `Arc<Mutex<AppState>>` managed by Tauri
- **Formatting**: Standard `cargo fmt` style

### General

- ESLint ignores `dist` and `src-tauri` directories
- Prettier ignores `dist`, `src-tauri`, `node_modules`, `pnpm-lock.yaml`, `eocc-hook`, `*.md`, and `.claude`
- The `eocc-hook` script uses CommonJS (`require`) since it runs standalone via Node.js

## CI/CD

### Pull request checks (`.github/workflows/lint.yml`)

Triggered on PRs that change source files. Three jobs:
1. **Frontend Lint**: ESLint, Prettier check, TypeScript type check
2. **Rust Lint**: `cargo fmt --check`, `cargo clippy -- -D warnings`
3. **Build**: Full `pnpm tauri build` (depends on both lint jobs passing)

### Release (`.github/workflows/release.yml`)

Triggered by pushing a `v*` tag or manual dispatch. Builds for macOS (aarch64), Linux (x64), and Windows (x64). Creates a draft GitHub release with all platform artifacts.

## Release process

1. Update version in both `src-tauri/tauri.conf.json` and `package.json`
2. Commit the version bump
3. Create and push a git tag: `git tag v<version> && git push origin v<version>`
4. GitHub Actions builds all platforms and creates a draft release
5. Review and publish the release on GitHub

## Testing

There are no automated tests in this project. Verify changes by:
- Running `pnpm typecheck` for TypeScript type safety
- Running `pnpm lint` for linting
- Running `pnpm tauri dev` for manual testing
