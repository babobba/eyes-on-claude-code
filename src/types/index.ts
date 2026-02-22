// Session status matching Rust enum
export type SessionStatus = 'Active' | 'WaitingPermission' | 'WaitingInput' | 'Completed';

// Notification type matching Rust enum (snake_case from serde)
export type NotificationType = 'permission_prompt' | 'idle_prompt' | 'other';

// Event type matching Rust enum (snake_case from serde)
export type EventType =
  | 'session_start'
  | 'session_end'
  | 'notification'
  | 'stop'
  | 'post_tool_use'
  | 'user_prompt_submit'
  | 'unknown';

export interface SessionInfo {
  project_name: string;
  project_dir: string;
  status: SessionStatus;
  last_event: string;
  waiting_for: string;
  tmux_pane: string;
}

export interface EventInfo {
  timestamp: string;
  event: EventType;
  matcher: string;
  project_name: string;
  project_dir: string;
  session_id: string;
  message: string;
  notification_type: NotificationType;
  tool_name: string;
  tmux_pane: string;
}

export interface DashboardData {
  sessions: SessionInfo[];
  events: EventInfo[];
}

export interface Settings {
  always_on_top: boolean;
  minimum_mode_enabled: boolean;
  opacity_active: number;
  opacity_inactive: number;
  sound_enabled: boolean;
}

export interface GitInfo {
  branch: string;
  default_branch: string;
  latest_commit_hash: string;
  latest_commit_time: string;
  has_unstaged_changes: boolean;
  has_staged_changes: boolean;
  is_git_repo: boolean;
}

// Diff type for difit integration
export type DiffType = 'unstaged' | 'staged' | 'commit' | 'branch';

// Tmux pane information
export interface TmuxPane {
  session_name: string;
  window_index: number;
  window_name: string;
  pane_index: number;
  pane_id: string;
  is_active: boolean;
}

// Tmux pane size (columns x rows)
export interface TmuxPaneSize {
  width: number;
  height: number;
}

// Notification channel configuration (tagged union matching Rust ChannelConfig)
export interface NtfyChannel {
  type: 'ntfy';
  server: string;
  topic: string;
  token?: string | null;
}

export interface WebhookChannel {
  type: 'webhook';
  url: string;
}

export interface PushoverChannel {
  type: 'pushover';
  user_key: string;
  app_token: string;
  device?: string | null;
}

export interface DesktopChannel {
  type: 'desktop';
}

export type ChannelConfig = NtfyChannel | WebhookChannel | PushoverChannel | DesktopChannel;

export interface ProjectRule {
  pattern: string;
  enabled?: boolean | null;
  notify_on?: SessionStatus[] | null;
}

export interface NotificationSettings {
  enabled: boolean;
  channels: ChannelConfig[];
  notify_on: SessionStatus[];
  project_rules: ProjectRule[];
  cooldown_seconds?: number | null;
  title_template?: string | null;
  body_template?: string | null;
}

export interface ChannelResult {
  name: string;
  success: boolean;
  error?: string | null;
}

export interface NotificationRecord {
  timestamp: string;
  project_name: string;
  project_dir: string;
  status: string;
  channels: ChannelResult[];
}

// Status of each individual hook type
export interface HookStatus {
  session_start: boolean;
  session_end: boolean;
  stop: boolean;
  post_tool_use: boolean;
  user_prompt_submit: boolean;
  notification_permission: boolean;
  notification_idle: boolean;
}

// Setup status for installation flow
export interface SetupStatus {
  hook_installed: boolean;
  hook_path: string;
  hooks: HookStatus;
  merged_settings: string;
  init_error: string | null;
}
