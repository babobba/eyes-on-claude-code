import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  DashboardData,
  GitInfo,
  NotificationRecord,
  NotificationSettings,
  Settings,
  SetupStatus,
  TmuxPane,
  TmuxPaneSize,
} from '@/types';

// Commands
export const getDashboardData = () => invoke<DashboardData>('get_dashboard_data');
export const removeSession = (projectDir: string) => invoke('remove_session', { projectDir });
export const clearAllSessions = () => invoke('clear_all_sessions');
export const getSettings = () => invoke<Settings>('get_settings');
export const getRepoGitInfo = (projectDir: string) =>
  invoke<GitInfo>('get_repo_git_info', { projectDir });

export const getRepoBranches = (projectDir: string) =>
  invoke<string[]>('get_repo_branches', { projectDir });

// Notification commands
export const getNotificationSettings = () =>
  invoke<NotificationSettings>('get_notification_settings');
export const updateNotificationSettings = (settings: NotificationSettings) =>
  invoke('update_notification_settings', { settings });
export const sendTestNotification = () => invoke('send_test_notification');
export const getNotificationHistory = () =>
  invoke<NotificationRecord[]>('get_notification_history');
export const clearNotificationHistory = () => invoke('clear_notification_history');

// Setup commands
export const getSetupStatus = () => invoke<SetupStatus>('get_setup_status');
export const checkClaudeSettings = () => invoke<SetupStatus>('check_claude_settings');
export const openClaudeSettings = () => invoke('open_claude_settings');
export const setWindowSizeForSetup = (enlarged: boolean) =>
  invoke('set_window_size_for_setup', { enlarged });

// Event listeners
export const onStateUpdated = (callback: (data: DashboardData) => void): Promise<UnlistenFn> => {
  return listen<DashboardData>('state-updated', (event) => callback(event.payload));
};

export const onSettingsUpdated = (callback: (settings: Settings) => void): Promise<UnlistenFn> => {
  return listen<Settings>('settings-updated', (event) => callback(event.payload));
};

export const onWindowFocus = (callback: () => void): Promise<UnlistenFn> => {
  return listen('tauri://focus', callback);
};

// Tmux commands
export const tmuxIsAvailable = () => invoke<boolean>('tmux_is_available');
export const tmuxListPanes = () => invoke<TmuxPane[]>('tmux_list_panes');
export const tmuxCapturePane = (paneId: string, projectDir?: string) =>
  invoke<string>('tmux_capture_pane', { paneId, projectDir });
export const tmuxSendKeys = (paneId: string, keys: string, projectDir?: string) =>
  invoke('tmux_send_keys', { paneId, keys, projectDir });
export const tmuxGetPaneSize = (paneId: string, projectDir?: string) =>
  invoke<TmuxPaneSize>('tmux_get_pane_size', { paneId, projectDir });
export const openTmuxViewer = (paneId: string, projectDir?: string) =>
  invoke('open_tmux_viewer', { paneId, projectDir });
