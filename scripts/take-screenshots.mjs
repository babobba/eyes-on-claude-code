#!/usr/bin/env node
/**
 * Generate app screenshots for documentation/release using Playwright.
 *
 * Starts the Vite dev server and injects mock Tauri APIs so the React
 * frontend renders with realistic session data — no Rust backend needed.
 *
 * Usage:
 *   node scripts/take-screenshots.mjs          # headless (CI / xvfb)
 *   node scripts/take-screenshots.mjs --headed  # visible browser
 *
 * Output: screenshots/ directory in the repo root.
 */

import { spawn } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

async function loadPlaywright() {
  try {
    return await import('playwright');
  } catch {
    // Global install fallback (e.g. CI images with playwright in /opt)
    return await import('/opt/node22/lib/node_modules/playwright/index.mjs');
  }
}

const { chromium } = await loadPlaywright();

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const SCREENSHOTS_DIR = resolve(ROOT, 'screenshots');
const VITE_PORT = 14200; // avoid clashing with the default 1420

// ---------------------------------------------------------------------------
// Mock data
// ---------------------------------------------------------------------------

const now = new Date();
const minutesAgo = (m) => new Date(now.getTime() - m * 60_000).toISOString();

const MOCK_SESSIONS = [
  {
    project_name: 'eyes-on-claude-code',
    project_dir: '/home/user/eyes-on-claude-code',
    status: 'Active',
    last_event: minutesAgo(2),
    waiting_for: '',
    tmux_pane: '%0',
    transport: { type: 'local' },
  },
  {
    project_name: 'my-web-app',
    project_dir: '/home/user/projects/my-web-app',
    status: 'WaitingPermission',
    last_event: minutesAgo(1),
    waiting_for: 'Bash: npm test',
    tmux_pane: '%1',
    transport: { type: 'local' },
  },
  {
    project_name: 'api-server',
    project_dir: '/home/user/projects/api-server',
    status: 'WaitingInput',
    last_event: minutesAgo(5),
    waiting_for: 'Waiting for your response',
    tmux_pane: '%2',
    transport: { type: 'ssh', host: 'dev.example.com', port: 22 },
  },
  {
    project_name: 'data-pipeline',
    project_dir: '/home/user/projects/data-pipeline',
    status: 'Completed',
    last_event: minutesAgo(15),
    waiting_for: '',
    tmux_pane: '',
    transport: { type: 'local' },
  },
];

const TAILSCALE_SESSIONS = [
  {
    project_name: 'infra-config',
    project_dir: '/home/deploy/infra-config',
    status: 'Active',
    last_event: minutesAgo(1),
    waiting_for: '',
    tmux_pane: '%3',
    transport: { type: 'tailscale', host: 'devbox-01', user: 'deploy' },
  },
  {
    project_name: 'ml-training',
    project_dir: '/home/deploy/ml-training',
    status: 'WaitingPermission',
    last_event: minutesAgo(0),
    waiting_for: 'Bash: ./train.sh --gpu',
    tmux_pane: '%4',
    transport: { type: 'tailscale', host: 'gpu-server', user: 'deploy' },
  },
  MOCK_SESSIONS[0], // local session for contrast
];

// prettier-ignore
const MOCK_TERMINAL_OUTPUT = [
  '\x1b[32mdeploy@devbox-01\x1b[0m:\x1b[34m~/infra-config\x1b[0m$ claude',
  '',
  '\x1b[1m\u256d\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u256e\x1b[0m',
  '\x1b[1m\u2502\x1b[0m  \x1b[36mClaude Code\x1b[0m                                \x1b[1m\u2502\x1b[0m',
  '\x1b[1m\u2570\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u256f\x1b[0m',
  '',
  '\x1b[33m>\x1b[0m Refactoring the Terraform modules to use for_each',
  '  instead of count for the AWS security groups...',
  '',
  '\x1b[90m  modules/security-groups/main.tf\x1b[0m',
  '\x1b[32m  + resource "aws_security_group" "this" {\x1b[0m',
  '\x1b[32m  +   for_each = var.security_groups\x1b[0m',
  '\x1b[32m  +   name     = each.value.name\x1b[0m',
  '\x1b[32m  +   vpc_id   = var.vpc_id\x1b[0m',
  '\x1b[32m  + }\x1b[0m',
  '',
  '\x1b[31m  - resource "aws_security_group" "this" {\x1b[0m',
  '\x1b[31m  -   count  = length(var.security_groups)\x1b[0m',
  '\x1b[31m  -   name   = var.security_groups[count.index].name\x1b[0m',
  '\x1b[31m  -   vpc_id = var.vpc_id\x1b[0m',
  '\x1b[31m  - }\x1b[0m',
  '',
  '\x1b[36m\u23f3 Waiting for permission to run:\x1b[0m',
  '\x1b[1m   terraform plan -out=tfplan\x1b[0m',
  '',
].join('\n');

const MOCK_SETTINGS = {
  always_on_top: true,
  minimum_mode_enabled: true,
  opacity_active: 1.0,
  opacity_inactive: 0.6,
  sound_enabled: true,
};

const MOCK_SETUP_STATUS = {
  hook_installed: true,
  hook_path: '/home/user/.eocc/bin/eocc-hook',
  hooks: {
    session_start: true,
    session_end: true,
    stop: true,
    post_tool_use: true,
    user_prompt_submit: true,
    notification_permission: true,
    notification_idle: true,
  },
  merged_settings: '{}',
  init_error: null,
};

const MOCK_GIT_INFO = {
  branch: 'feature/new-dashboard',
  default_branch: 'main',
  latest_commit_hash: 'a3f7c2e',
  latest_commit_time: minutesAgo(10),
  has_unstaged_changes: true,
  has_staged_changes: false,
  is_git_repo: true,
};

// ---------------------------------------------------------------------------
// Tauri mock injection script (runs in the browser before page loads)
// ---------------------------------------------------------------------------

let _nextEventId = 1;

function buildMockData(overrides = {}) {
  return {
    dashboardData: { sessions: MOCK_SESSIONS, events: [] },
    settings: MOCK_SETTINGS,
    setupStatus: MOCK_SETUP_STATUS,
    gitInfo: MOCK_GIT_INFO,
    _nextEventId: _nextEventId++,
    ...overrides,
  };
}

function buildMockScript(data) {
  return `
    // Mock Tauri internals so @tauri-apps/api works without a Rust backend.
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      plugins: {},
      _listeners: {},
      _nextId: 1,

      transformCallback(cb, once) {
        const id = this._nextId++;
        const key = \`_\${id}\`;
        this._listeners[key] = cb;
        return id;
      },

      async invoke(cmd, args) {
        const data = ${JSON.stringify(data)};

        const commands = {
          // App commands
          get_dashboard_data: () => data.dashboardData,
          get_settings: () => data.settings,
          get_setup_status: () => data.setupStatus,
          get_repo_git_info: () => data.gitInfo,
          get_repo_branches: () => ['main', 'feature/new-dashboard', 'fix/auth-bug'],
          get_notification_settings: () => ({
            enabled: true, channels: [{ type: 'desktop' }],
            notify_on: ['WaitingPermission', 'WaitingInput', 'Completed'],
            project_rules: [], cooldown_seconds: null,
            title_template: null, body_template: null,
          }),
          get_notification_history: () => [],

          // Tmux commands
          tmux_capture_pane: () => data.tmuxContent || '(no content)',
          tmux_get_pane_size: () => data.tmuxPaneSize || { width: 80, height: 24 },
          tmux_is_available: () => true,
          tmux_list_panes: () => [],
          tmux_send_keys: () => null,
        };

        if (commands[cmd]) return commands[cmd]();

        // Event listener registration — return an ID so listen() resolves
        if (cmd === 'plugin:event|listen') return data._nextEventId++;

        // Window plugin commands — return sensible defaults
        const windowCommands = {
          'plugin:window|is_focused': () => true,
          'plugin:window|scale_factor': () => 1,
          'plugin:window|inner_size': () => ({ width: 400, height: 600 }),
          'plugin:window|outer_position': () => ({ x: 100, y: 100 }),
          'plugin:window|available_monitors': () => [{
            name: 'default', position: { x: 0, y: 0 },
            size: { width: 1920, height: 1080 }, scaleFactor: 1,
          }],
        };
        if (windowCommands[cmd]) return windowCommands[cmd]();

        // No-op commands (set_size, set_focus, etc.)
        if (typeof cmd === 'string' && cmd.startsWith('plugin:')) return null;

        console.warn('[mock] unhandled invoke:', cmd, args);
        return null;
      },
    };
    window.__TAURI__ = window.__TAURI_INTERNALS__;
  `;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Start the Vite dev server and return { url, kill() }. */
function startVite() {
  return new Promise((resolve, reject) => {
    const proc = spawn('npx', ['vite', '--port', String(VITE_PORT), '--strictPort'], {
      cwd: ROOT,
      stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, BROWSER: 'none' },
    });

    let started = false;

    const onData = (chunk) => {
      const text = chunk.toString();
      if (!started && text.includes(`localhost:${VITE_PORT}`)) {
        started = true;
        resolve({
          url: `http://localhost:${VITE_PORT}`,
          kill: () => proc.kill('SIGTERM'),
        });
      }
    };

    proc.stdout.on('data', onData);
    proc.stderr.on('data', onData);

    proc.on('error', reject);

    setTimeout(() => {
      if (!started) {
        proc.kill('SIGTERM');
        reject(new Error('Vite dev server did not start within 30s'));
      }
    }, 30_000);
  });
}

/**
 * Capture a single screenshot.
 *
 * @param {import('playwright').Browser} browser
 * @param {object} opts
 * @param {string}  opts.name       - Output filename (without directory)
 * @param {{width:number, height:number}} opts.viewport
 * @param {object}  opts.mockData   - Data passed to buildMockScript
 * @param {string}  opts.url        - Page URL to navigate to
 * @param {(page: import('playwright').Page) => Promise<void>} [opts.beforeCapture]
 *        - Optional async callback to interact with the page before the screenshot
 */
async function capture(browser, { name, viewport, mockData, url, beforeCapture }) {
  const ctx = await browser.newContext({
    viewport,
    deviceScaleFactor: 2,
    colorScheme: 'dark',
  });
  const page = await ctx.newPage();
  await page.addInitScript({ content: buildMockScript(mockData) });
  await page.goto(url, { waitUntil: 'networkidle' });
  await page.waitForTimeout(500);

  if (beforeCapture) {
    await beforeCapture(page);
  }

  await page.screenshot({
    path: resolve(SCREENSHOTS_DIR, name),
    omitBackground: true,
  });
  console.log(`  \u2713 ${name}`);
  await ctx.close();
}

/** Click the first session card to expand it. */
async function expandFirstSession(page) {
  await page.locator('.bg-bg-secondary').first().click();
  await page.waitForTimeout(600);
}

// ---------------------------------------------------------------------------
// Scenario definitions
// ---------------------------------------------------------------------------

function scenarios(baseUrl) {
  return [
    {
      name: 'dashboard.png',
      viewport: { width: 400, height: 520 },
      mockData: buildMockData(),
      url: baseUrl,
    },
    {
      name: 'empty-state.png',
      viewport: { width: 400, height: 300 },
      mockData: buildMockData({ dashboardData: { sessions: [], events: [] } }),
      url: baseUrl,
    },
    {
      name: 'session-expanded.png',
      viewport: { width: 400, height: 620 },
      mockData: buildMockData(),
      url: baseUrl,
      beforeCapture: expandFirstSession,
    },
    {
      name: 'waiting-sessions.png',
      viewport: { width: 400, height: 360 },
      mockData: buildMockData({
        dashboardData: {
          sessions: MOCK_SESSIONS.filter(
            (s) => s.status === 'WaitingPermission' || s.status === 'WaitingInput'
          ),
          events: [],
        },
      }),
      url: baseUrl,
    },
    {
      name: 'tailscale-session.png',
      viewport: { width: 400, height: 560 },
      mockData: buildMockData({
        dashboardData: { sessions: TAILSCALE_SESSIONS, events: [] },
        gitInfo: {
          ...MOCK_GIT_INFO,
          branch: 'main',
          has_unstaged_changes: false,
          latest_commit_hash: 'b8e1d4f',
        },
      }),
      url: baseUrl,
      beforeCapture: expandFirstSession,
    },
    {
      name: 'tmux-viewer.png',
      viewport: { width: 700, height: 500 },
      mockData: buildMockData({
        tmuxContent: MOCK_TERMINAL_OUTPUT,
        tmuxPaneSize: { width: 80, height: 40 },
      }),
      // %25 is URL-encoded '%', so %253 decodes to %3 (tmux pane ID)
      url: `${baseUrl}?tmux_pane=%253&project_dir=/home/deploy/infra-config`,
    },
  ];
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function takeScreenshots() {
  await mkdir(SCREENSHOTS_DIR, { recursive: true });

  console.log('Starting Vite dev server\u2026');
  const vite = await startVite();
  console.log(`Vite ready at ${vite.url}`);

  const headed = process.argv.includes('--headed');
  const browser = await chromium.launch({ headless: !headed });

  try {
    for (const scenario of scenarios(vite.url)) {
      await capture(browser, scenario);
    }
    console.log(`\nAll screenshots saved to ${SCREENSHOTS_DIR}/`);
  } finally {
    await browser.close();
    vite.kill();
  }
}

takeScreenshots().catch((err) => {
  console.error('Screenshot generation failed:', err);
  process.exit(1);
});
