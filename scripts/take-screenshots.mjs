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

const { chromium } = await import('/opt/node22/lib/node_modules/playwright/index.mjs');
import { spawn } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

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

const MOCK_DASHBOARD_DATA = {
  sessions: MOCK_SESSIONS,
  events: [],
};

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

        // App commands
        if (cmd === 'get_dashboard_data') return data.dashboardData;
        if (cmd === 'get_settings') return data.settings;
        if (cmd === 'get_setup_status') return data.setupStatus;
        if (cmd === 'get_repo_git_info') return data.gitInfo;
        if (cmd === 'get_repo_branches') return ['main', 'feature/new-dashboard', 'fix/auth-bug'];
        if (cmd === 'get_notification_settings') return {
          enabled: true, channels: [{ type: 'desktop' }],
          notify_on: ['WaitingPermission', 'WaitingInput', 'Completed'],
          project_rules: [], cooldown_seconds: null,
          title_template: null, body_template: null,
        };
        if (cmd === 'get_notification_history') return [];

        // Event listener registration — return an ID so listen() resolves
        if (cmd === 'plugin:event|listen') return data._nextEventId++;

        // Window plugin commands — return sensible defaults
        if (cmd === 'plugin:window|is_focused') return true;
        if (cmd === 'plugin:window|scale_factor') return 1;
        if (cmd === 'plugin:window|inner_size') return { width: 400, height: 600 };
        if (cmd === 'plugin:window|outer_position') return { x: 100, y: 100 };
        if (cmd === 'plugin:window|available_monitors') return [{
          name: 'default', position: { x: 0, y: 0 },
          size: { width: 1920, height: 1080 }, scaleFactor: 1,
        }];

        // No-op commands (set_size, set_focus, etc.)
        if (typeof cmd === 'string' && cmd.startsWith('plugin:')) return null;

        // Fallback
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

    // Timeout after 30s
    setTimeout(() => {
      if (!started) {
        proc.kill('SIGTERM');
        reject(new Error('Vite dev server did not start within 30s'));
      }
    }, 30_000);
  });
}

// ---------------------------------------------------------------------------
// Screenshot scenarios
// ---------------------------------------------------------------------------

async function takeScreenshots() {
  await mkdir(SCREENSHOTS_DIR, { recursive: true });

  console.log('Starting Vite dev server…');
  const vite = await startVite();
  console.log(`Vite ready at ${vite.url}`);

  const headed = process.argv.includes('--headed');
  const browser = await chromium.launch({ headless: !headed });

  try {
    // ---- 1. Dashboard with sessions ----
    {
      const mockData = {
        dashboardData: MOCK_DASHBOARD_DATA,
        settings: MOCK_SETTINGS,
        setupStatus: MOCK_SETUP_STATUS,
        gitInfo: MOCK_GIT_INFO,
        _nextEventId: 100,
      };

      const ctx = await browser.newContext({
        viewport: { width: 400, height: 520 },
        deviceScaleFactor: 2,
        colorScheme: 'dark',
      });
      const page = await ctx.newPage();
      await page.addInitScript({ content: buildMockScript(mockData) });
      await page.goto(vite.url, { waitUntil: 'networkidle' });
      await page.waitForTimeout(500);
      await page.screenshot({
        path: resolve(SCREENSHOTS_DIR, 'dashboard.png'),
        omitBackground: true,
      });
      console.log('  ✓ dashboard.png');
      await ctx.close();
    }

    // ---- 2. Empty state ----
    {
      const mockData = {
        dashboardData: { sessions: [], events: [] },
        settings: MOCK_SETTINGS,
        setupStatus: MOCK_SETUP_STATUS,
        gitInfo: MOCK_GIT_INFO,
        _nextEventId: 200,
      };

      const ctx = await browser.newContext({
        viewport: { width: 400, height: 300 },
        deviceScaleFactor: 2,
        colorScheme: 'dark',
      });
      const page = await ctx.newPage();
      await page.addInitScript({ content: buildMockScript(mockData) });
      await page.goto(vite.url, { waitUntil: 'networkidle' });
      await page.waitForTimeout(500);
      await page.screenshot({
        path: resolve(SCREENSHOTS_DIR, 'empty-state.png'),
        omitBackground: true,
      });
      console.log('  ✓ empty-state.png');
      await ctx.close();
    }

    // ---- 3. Session expanded (with git info) ----
    {
      const mockData = {
        dashboardData: MOCK_DASHBOARD_DATA,
        settings: MOCK_SETTINGS,
        setupStatus: MOCK_SETUP_STATUS,
        gitInfo: MOCK_GIT_INFO,
        _nextEventId: 300,
      };

      const ctx = await browser.newContext({
        viewport: { width: 400, height: 620 },
        deviceScaleFactor: 2,
        colorScheme: 'dark',
      });
      const page = await ctx.newPage();
      await page.addInitScript({ content: buildMockScript(mockData) });
      await page.goto(vite.url, { waitUntil: 'networkidle' });
      await page.waitForTimeout(500);

      // Click the first session card to expand it
      const firstCard = page.locator('.bg-bg-secondary').first();
      await firstCard.click();
      await page.waitForTimeout(600);

      await page.screenshot({
        path: resolve(SCREENSHOTS_DIR, 'session-expanded.png'),
        omitBackground: true,
      });
      console.log('  ✓ session-expanded.png');
      await ctx.close();
    }

    // ---- 4. Only waiting sessions (alert state) ----
    {
      const waitingSessions = MOCK_SESSIONS.filter(
        (s) => s.status === 'WaitingPermission' || s.status === 'WaitingInput'
      );
      const mockData = {
        dashboardData: { sessions: waitingSessions, events: [] },
        settings: MOCK_SETTINGS,
        setupStatus: MOCK_SETUP_STATUS,
        gitInfo: MOCK_GIT_INFO,
        _nextEventId: 400,
      };

      const ctx = await browser.newContext({
        viewport: { width: 400, height: 360 },
        deviceScaleFactor: 2,
        colorScheme: 'dark',
      });
      const page = await ctx.newPage();
      await page.addInitScript({ content: buildMockScript(mockData) });
      await page.goto(vite.url, { waitUntil: 'networkidle' });
      await page.waitForTimeout(500);
      await page.screenshot({
        path: resolve(SCREENSHOTS_DIR, 'waiting-sessions.png'),
        omitBackground: true,
      });
      console.log('  ✓ waiting-sessions.png');
      await ctx.close();
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
