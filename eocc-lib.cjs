/**
 * eocc-lib.js - Shared library for eocc-hook and eocc-server
 *
 * Contains notification dispatch, TOML parsing, connect URL building,
 * and HTTP helpers used by both the hook (short-lived) and the server
 * (long-running event poller + HTTP viewer).
 */

const fs = require("node:fs");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STATUS_EMOJI = {
  active: "\u{1F7E2}",
  waiting_permission: "\u{1F510}",
  waiting_input: "\u{23F3}",
  completed: "\u{2705}",
};

const STATUS_LABELS = {
  active: "Active",
  waiting_permission: "Waiting for permission",
  waiting_input: "Waiting for input",
  completed: "Completed",
};

const NTFY_TAGS = {
  active: "green_circle",
  waiting_permission: "lock",
  waiting_input: "hourglass",
  completed: "white_check_mark",
};

// ---------------------------------------------------------------------------
// Status mapping
// ---------------------------------------------------------------------------

function mapEventToStatus(eventType, notificationType) {
  switch (eventType) {
    case "session_start":
    case "post_tool_use":
    case "user_prompt_submit":
      return "active";
    case "session_end":
      return null;
    case "stop":
      return "completed";
    case "notification":
      if (notificationType === "permission_prompt") return "waiting_permission";
      if (notificationType === "idle_prompt") return "waiting_input";
      return "active";
    default:
      return undefined;
  }
}

function mapPriority(status) {
  if (status === "waiting_permission" || status === "waiting_input") return "high";
  if (status === "completed") return "normal";
  return "low";
}

// ---------------------------------------------------------------------------
// TOML parser (handles the subset used by notification_settings.toml)
// ---------------------------------------------------------------------------

function parseTomlValue(raw) {
  const v = raw.trim();
  if (v === "true") return true;
  if (v === "false") return false;
  if (/^-?\d+$/.test(v)) return parseInt(v, 10);
  const dq = v.match(/^"((?:[^"\\]|\\.)*)"$/);
  if (dq) return dq[1].replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  const sq = v.match(/^'([^']*)'$/);
  if (sq) return sq[1];
  if (v.startsWith("[") && v.endsWith("]")) {
    const inner = v.slice(1, -1).trim();
    if (!inner) return [];
    return inner.split(",").map((s) => parseTomlValue(s));
  }
  return v;
}

function parseNotificationSettings(filePath) {
  try {
    const text = fs.readFileSync(filePath, "utf8");
    const settings = {
      enabled: false,
      channels: [],
      notify_on: ["waiting_permission", "waiting_input", "completed"],
      project_rules: [],
      cooldown_seconds: null,
      title_template: null,
      body_template: null,
    };
    let currentSection = null;
    let currentItem = null;

    function flushItem() {
      if (!currentItem) return;
      if (currentSection === "channels") settings.channels.push(currentItem);
      else if (currentSection === "project_rules") settings.project_rules.push(currentItem);
    }

    for (const rawLine of text.split("\n")) {
      let line = "";
      let inStr = false;
      let q = null;
      for (let i = 0; i < rawLine.length; i++) {
        const c = rawLine[i];
        if (!inStr && (c === '"' || c === "'")) {
          inStr = true;
          q = c;
        } else if (inStr && c === q) {
          inStr = false;
        } else if (!inStr && c === "#") {
          break;
        }
        line += c;
      }
      line = line.trim();
      if (!line) continue;

      if (line === "[[channels]]") {
        flushItem();
        currentSection = "channels";
        currentItem = {};
        continue;
      }
      if (line === "[[project_rules]]") {
        flushItem();
        currentSection = "project_rules";
        currentItem = {};
        continue;
      }

      const eqIdx = line.indexOf("=");
      if (eqIdx === -1) continue;
      const key = line.slice(0, eqIdx).trim();
      const value = parseTomlValue(line.slice(eqIdx + 1));

      if (currentSection && currentItem) {
        currentItem[key] = value;
      } else {
        settings[key] = value;
      }
    }
    flushItem();
    return settings;
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Pattern matching & project rules
// ---------------------------------------------------------------------------

function patternMatches(pattern, filePath) {
  if (!pattern) return false;
  if (pattern === filePath) return true;
  if (pattern.startsWith("**/")) {
    const suffix = pattern.slice(3);
    return filePath.endsWith(suffix) || filePath.includes("/" + suffix);
  }
  if (pattern.startsWith("*") && pattern.endsWith("*") && pattern.length > 2) {
    return filePath.includes(pattern.slice(1, -1));
  }
  if (pattern.endsWith("/**")) {
    return filePath.startsWith(pattern.slice(0, -3));
  }
  if (pattern.endsWith("*")) {
    return filePath.startsWith(pattern.slice(0, -1));
  }
  return false;
}

function resolveForProject(settings, projectDir) {
  for (const rule of settings.project_rules || []) {
    if (patternMatches(rule.pattern, projectDir)) {
      const enabled =
        rule.enabled !== undefined && rule.enabled !== null ? rule.enabled : settings.enabled;
      const notifyOn = rule.notify_on || settings.notify_on;
      return { enabled, notifyOn };
    }
  }
  return { enabled: settings.enabled, notifyOn: settings.notify_on };
}

// ---------------------------------------------------------------------------
// Connect URL building
// ---------------------------------------------------------------------------

function buildConnectUrl(transport) {
  if (!transport) return "";

  if (transport.viewer_url) {
    let url = transport.viewer_url.replace(/\/+$/, "");
    if (transport.tmux_pane) {
      url += `/tmux/${encodeURIComponent(transport.tmux_pane)}`;
    }
    return url;
  }

  if (!transport.host || transport.type === "local") return "";

  const user = transport.user ? `${transport.user}@` : "";
  const tmuxCmd = transport.tmux_session ? `;tmux attach -t ${transport.tmux_session}` : "";

  switch (transport.type) {
    case "ssh":
    case "tailscale": {
      const port = transport.port && transport.port !== "22" ? `:${transport.port}` : "";
      return `ssh://${user}${transport.host}${port}${tmuxCmd}`;
    }
    case "mosh":
      return `mosh://${user}${transport.host}${tmuxCmd}`;
    default:
      return "";
  }
}

// ---------------------------------------------------------------------------
// Template engine
// ---------------------------------------------------------------------------

function applyTemplate(template, vars) {
  return template
    .replace(/\{project_name\}/g, vars.projectName)
    .replace(/\{project_dir\}/g, vars.projectDir)
    .replace(/\{status\}/g, vars.statusLabel)
    .replace(/\{emoji\}/g, vars.emoji)
    .replace(/\{message\}/g, vars.message)
    .replace(/\{priority\}/g, vars.priority)
    .replace(/\{connect_url\}/g, vars.connectUrl)
    .replace(/\{transport_type\}/g, vars.transportType)
    .replace(/\{tmux_session\}/g, vars.tmuxSession)
    .replace(/\{tmux_pane\}/g, vars.tmuxPane);
}

function buildNotification(settings, projectName, projectDir, status, message, transport) {
  const emoji = STATUS_EMOJI[status] || "";
  const statusLabel = STATUS_LABELS[status] || status;
  const priority = mapPriority(status);
  const connectUrl = buildConnectUrl(transport);
  const transportType = (transport && transport.type) || "local";
  const tmuxSession = (transport && transport.tmux_session) || "";
  const tmuxPane = (transport && transport.tmux_pane) || "";

  const vars = {
    projectName,
    projectDir,
    statusLabel,
    emoji,
    message: message || "",
    priority,
    connectUrl,
    transportType,
    tmuxSession,
    tmuxPane,
  };

  const title = settings.title_template
    ? applyTemplate(settings.title_template, vars)
    : `${emoji} ${projectName} - ${statusLabel}`;

  const defaultBody = connectUrl
    ? `${message || statusLabel}\n${connectUrl}`
    : message || statusLabel;

  const body = settings.body_template ? applyTemplate(settings.body_template, vars) : defaultBody;

  return { title, body, priority, status, projectName, projectDir, connectUrl, tmuxSession, tmuxPane };
}

// ---------------------------------------------------------------------------
// Hook state persistence
// ---------------------------------------------------------------------------

function loadHookState(stateFile) {
  try {
    return JSON.parse(fs.readFileSync(stateFile, "utf8"));
  } catch {
    return { sessions: {} };
  }
}

function saveHookState(stateFile, state) {
  try {
    const tmpFile = stateFile + ".tmp." + process.pid;
    fs.writeFileSync(tmpFile, JSON.stringify(state), "utf8");
    fs.renameSync(tmpFile, stateFile);
  } catch {
    // Best effort
  }
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function httpPost(url, headers, body) {
  return new Promise((resolve) => {
    try {
      const parsedUrl = new URL(url);
      const mod = parsedUrl.protocol === "https:" ? require("node:https") : require("node:http");
      const options = {
        hostname: parsedUrl.hostname,
        port: parsedUrl.port || (parsedUrl.protocol === "https:" ? 443 : 80),
        path: parsedUrl.pathname + parsedUrl.search,
        method: "POST",
        headers: { ...headers, "Content-Length": Buffer.byteLength(body) },
        timeout: 5000,
      };
      const req = mod.request(options, (res) => {
        res.resume();
        resolve({ ok: res.statusCode >= 200 && res.statusCode < 300, status: res.statusCode });
      });
      req.on("error", (err) => resolve({ ok: false, error: err.message }));
      req.on("timeout", () => {
        req.destroy();
        resolve({ ok: false, error: "timeout" });
      });
      req.write(body);
      req.end();
    } catch (err) {
      resolve({ ok: false, error: err.message || "request_failed" });
    }
  });
}

const MAX_RETRIES = 3;
const RETRY_BASE_MS = 1000;

async function httpPostWithRetry(url, headers, body) {
  let result = await httpPost(url, headers, body);
  for (let attempt = 1; attempt <= MAX_RETRIES && !result.ok; attempt++) {
    await sleep(Math.log(attempt + 1) * RETRY_BASE_MS);
    result = await httpPost(url, headers, body);
  }
  return result;
}

// ---------------------------------------------------------------------------
// Channel dispatchers
// ---------------------------------------------------------------------------

function sendNtfy(channel, notification) {
  const server = (channel.server || "").replace(/\/+$/, "");
  const url = `${server}/${channel.topic}`;
  const priorityMap = { high: "high", normal: "default", low: "low" };
  const headers = {
    Title: notification.title,
    Priority: priorityMap[notification.priority] || "default",
    Tags: NTFY_TAGS[notification.status] || "",
  };
  if (notification.connectUrl) headers["Click"] = notification.connectUrl;
  if (channel.token) headers["Authorization"] = `Bearer ${channel.token}`;
  return httpPostWithRetry(url, headers, notification.body);
}

function sendWebhookChannel(channel, notification) {
  const payload = JSON.stringify({
    text: `${notification.title}\n${notification.body}`,
    project_name: notification.projectName,
    project_dir: notification.projectDir,
    status: notification.status,
    priority: notification.priority,
    connect_url: notification.connectUrl || null,
    tmux_session: notification.tmuxSession || null,
    tmux_pane: notification.tmuxPane || null,
  });
  return httpPostWithRetry(channel.url, { "Content-Type": "application/json" }, payload);
}

function sendPushover(channel, notification) {
  const priorityMap = { high: "1", normal: "0", low: "-1" };
  const enc = (s) => encodeURIComponent(s);
  let form =
    `token=${enc(channel.app_token)}&user=${enc(channel.user_key)}` +
    `&title=${enc(notification.title)}&message=${enc(notification.body)}` +
    `&priority=${priorityMap[notification.priority] || "0"}`;
  if (channel.device) form += `&device=${enc(channel.device)}`;
  if (notification.connectUrl) form += `&url=${enc(notification.connectUrl)}&url_title=Connect`;
  return httpPostWithRetry("https://api.pushover.net/1/messages.json", {
    "Content-Type": "application/x-www-form-urlencoded",
  }, form);
}

async function dispatchToChannels(settings, notification) {
  const promises = [];
  for (const channel of settings.channels || []) {
    try {
      switch (channel.type) {
        case "ntfy":
          promises.push(sendNtfy(channel, notification));
          break;
        case "webhook":
          promises.push(sendWebhookChannel(channel, notification));
          break;
        case "pushover":
          promises.push(sendPushover(channel, notification));
          break;
      }
    } catch {
      // Best effort, continue to next channel
    }
  }
  return Promise.all(promises);
}

// ---------------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------------

module.exports = {
  STATUS_EMOJI,
  STATUS_LABELS,
  NTFY_TAGS,
  mapEventToStatus,
  mapPriority,
  parseNotificationSettings,
  patternMatches,
  resolveForProject,
  buildConnectUrl,
  applyTemplate,
  buildNotification,
  loadHookState,
  saveHookState,
  sleep,
  httpPost,
  httpPostWithRetry,
  sendNtfy,
  sendWebhookChannel,
  sendPushover,
  dispatchToChannels,
};
