import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  getDashboardData,
  removeSession,
  clearAllSessions,
  getSettings,
  getRepoGitInfo,
  getRepoBranches,
  getSetupStatus,
  checkClaudeSettings,
  openClaudeSettings,
  setWindowSizeForSetup,
  onStateUpdated,
  onSettingsUpdated,
  tmuxIsAvailable,
  tmuxListPanes,
  tmuxCapturePane,
  tmuxSendKeys,
  tmuxGetPaneSize,
  openTmuxViewer,
} from '../tauri';

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

beforeEach(() => {
  vi.clearAllMocks();
});

describe('Tauri command wrappers', () => {
  it('getDashboardData invokes correct command', async () => {
    mockInvoke.mockResolvedValue({ sessions: [], events: [] });
    await getDashboardData();
    expect(mockInvoke).toHaveBeenCalledWith('get_dashboard_data');
  });

  it('removeSession passes projectDir', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await removeSession('/path/to/project');
    expect(mockInvoke).toHaveBeenCalledWith('remove_session', { projectDir: '/path/to/project' });
  });

  it('clearAllSessions invokes correct command', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await clearAllSessions();
    expect(mockInvoke).toHaveBeenCalledWith('clear_all_sessions');
  });

  it('getSettings invokes correct command', async () => {
    mockInvoke.mockResolvedValue({});
    await getSettings();
    expect(mockInvoke).toHaveBeenCalledWith('get_settings');
  });

  it('getRepoGitInfo passes projectDir', async () => {
    mockInvoke.mockResolvedValue({});
    await getRepoGitInfo('/repo');
    expect(mockInvoke).toHaveBeenCalledWith('get_repo_git_info', { projectDir: '/repo' });
  });

  it('getRepoBranches passes projectDir', async () => {
    mockInvoke.mockResolvedValue([]);
    await getRepoBranches('/repo');
    expect(mockInvoke).toHaveBeenCalledWith('get_repo_branches', { projectDir: '/repo' });
  });

  it('getSetupStatus invokes correct command', async () => {
    mockInvoke.mockResolvedValue({});
    await getSetupStatus();
    expect(mockInvoke).toHaveBeenCalledWith('get_setup_status');
  });

  it('checkClaudeSettings invokes correct command', async () => {
    mockInvoke.mockResolvedValue({});
    await checkClaudeSettings();
    expect(mockInvoke).toHaveBeenCalledWith('check_claude_settings');
  });

  it('openClaudeSettings invokes correct command', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await openClaudeSettings();
    expect(mockInvoke).toHaveBeenCalledWith('open_claude_settings');
  });

  it('setWindowSizeForSetup passes enlarged flag', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await setWindowSizeForSetup(true);
    expect(mockInvoke).toHaveBeenCalledWith('set_window_size_for_setup', { enlarged: true });
  });

  it('tmuxIsAvailable invokes correct command', async () => {
    mockInvoke.mockResolvedValue(true);
    await tmuxIsAvailable();
    expect(mockInvoke).toHaveBeenCalledWith('tmux_is_available');
  });

  it('tmuxListPanes invokes correct command', async () => {
    mockInvoke.mockResolvedValue([]);
    await tmuxListPanes();
    expect(mockInvoke).toHaveBeenCalledWith('tmux_list_panes');
  });

  it('tmuxCapturePane passes paneId', async () => {
    mockInvoke.mockResolvedValue('');
    await tmuxCapturePane('%0');
    expect(mockInvoke).toHaveBeenCalledWith('tmux_capture_pane', { paneId: '%0' });
  });

  it('tmuxSendKeys passes paneId and keys', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await tmuxSendKeys('%0', 'ls');
    expect(mockInvoke).toHaveBeenCalledWith('tmux_send_keys', { paneId: '%0', keys: 'ls' });
  });

  it('tmuxGetPaneSize passes paneId', async () => {
    mockInvoke.mockResolvedValue({ width: 80, height: 24 });
    await tmuxGetPaneSize('%0');
    expect(mockInvoke).toHaveBeenCalledWith('tmux_get_pane_size', { paneId: '%0' });
  });

  it('openTmuxViewer passes paneId', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await openTmuxViewer('%0');
    expect(mockInvoke).toHaveBeenCalledWith('open_tmux_viewer', { paneId: '%0' });
  });
});

describe('Tauri event listeners', () => {
  it('onStateUpdated listens to state-updated event', async () => {
    const callback = vi.fn();
    const mockUnlisten = vi.fn();
    mockListen.mockResolvedValue(mockUnlisten);

    const unlisten = await onStateUpdated(callback);

    expect(mockListen).toHaveBeenCalledWith('state-updated', expect.any(Function));
    expect(unlisten).toBe(mockUnlisten);
  });

  it('onSettingsUpdated listens to settings-updated event', async () => {
    const callback = vi.fn();
    const mockUnlisten = vi.fn();
    mockListen.mockResolvedValue(mockUnlisten);

    const unlisten = await onSettingsUpdated(callback);

    expect(mockListen).toHaveBeenCalledWith('settings-updated', expect.any(Function));
    expect(unlisten).toBe(mockUnlisten);
  });
});
