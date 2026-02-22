import { useState, useEffect, useCallback } from 'react';
import type {
  NotificationSettings as NotificationSettingsType,
  ChannelConfig,
  ProjectRule,
  NotificationRecord,
} from '@/types';
import {
  getNotificationSettings,
  updateNotificationSettings,
  sendTestNotification,
  getNotificationHistory,
  clearNotificationHistory,
} from '@/lib/tauri';

const ALL_STATUSES = ['WaitingPermission', 'WaitingInput', 'Completed', 'Active'] as const;
const STATUS_LABELS: Record<string, string> = {
  WaitingPermission: 'Waiting for permission',
  WaitingInput: 'Waiting for input',
  Completed: 'Completed',
  Active: 'Active',
};

const inputClass =
  'bg-bg-secondary border border-bg-card rounded px-2 py-1 text-text-primary text-[0.625rem] outline-none focus:border-accent w-full';
const btnSmall =
  'bg-bg-card border-none text-text-primary rounded cursor-pointer hover:bg-accent py-0.5 px-1.5 text-[0.625rem]';

type Tab = 'settings' | 'history';

interface NotificationSettingsProps {
  onClose: () => void;
}

export const NotificationSettings = ({ onClose }: NotificationSettingsProps) => {
  const [settings, setSettings] = useState<NotificationSettingsType | null>(null);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [message, setMessage] = useState<{ text: string; type: 'success' | 'error' } | null>(null);
  const [tab, setTab] = useState<Tab>('settings');
  const [history, setHistory] = useState<NotificationRecord[]>([]);

  useEffect(() => {
    getNotificationSettings()
      .then(setSettings)
      .catch((err) => console.error('Failed to load notification settings:', err));
  }, []);

  useEffect(() => {
    if (tab === 'history') {
      getNotificationHistory()
        .then(setHistory)
        .catch((err) => console.error('Failed to load notification history:', err));
    }
  }, [tab]);

  const save = useCallback(async (updated: NotificationSettingsType) => {
    setSaving(true);
    setMessage(null);
    try {
      await updateNotificationSettings(updated);
      setSettings(updated);
      setMessage({ text: 'Settings saved', type: 'success' });
    } catch (err) {
      setMessage({ text: `Failed to save: ${err}`, type: 'error' });
    } finally {
      setSaving(false);
    }
  }, []);

  const handleTest = async () => {
    setTesting(true);
    setMessage(null);
    try {
      await sendTestNotification();
      setMessage({ text: 'Test notification sent', type: 'success' });
    } catch (err) {
      setMessage({ text: `${err}`, type: 'error' });
    } finally {
      setTesting(false);
    }
  };

  const addChannel = (type: ChannelConfig['type']) => {
    if (!settings) return;
    let newChannel: ChannelConfig;
    switch (type) {
      case 'ntfy':
        newChannel = { type: 'ntfy', server: 'https://ntfy.sh', topic: '', token: null };
        break;
      case 'webhook':
        newChannel = { type: 'webhook', url: '' };
        break;
      case 'pushover':
        newChannel = { type: 'pushover', user_key: '', app_token: '', device: null };
        break;
      case 'desktop':
        newChannel = { type: 'desktop' };
        break;
    }
    setSettings({ ...settings, channels: [...settings.channels, newChannel] });
  };

  const removeChannel = (index: number) => {
    if (!settings) return;
    setSettings({ ...settings, channels: settings.channels.filter((_, i) => i !== index) });
  };

  const updateChannel = (index: number, channel: ChannelConfig) => {
    if (!settings) return;
    const channels = [...settings.channels];
    channels[index] = channel;
    setSettings({ ...settings, channels });
  };

  const toggleStatus = (status: string) => {
    if (!settings) return;
    const notify_on = settings.notify_on.includes(status as (typeof ALL_STATUSES)[number])
      ? settings.notify_on.filter((s) => s !== status)
      : [...settings.notify_on, status as (typeof ALL_STATUSES)[number]];
    setSettings({ ...settings, notify_on });
  };

  const addProjectRule = () => {
    if (!settings) return;
    const rule: ProjectRule = { pattern: '', enabled: null, notify_on: null };
    setSettings({ ...settings, project_rules: [...settings.project_rules, rule] });
  };

  const removeProjectRule = (index: number) => {
    if (!settings) return;
    setSettings({
      ...settings,
      project_rules: settings.project_rules.filter((_, i) => i !== index),
    });
  };

  const updateProjectRule = (index: number, rule: ProjectRule) => {
    if (!settings) return;
    const rules = [...settings.project_rules];
    rules[index] = rule;
    setSettings({ ...settings, project_rules: rules });
  };

  if (!settings) {
    return <div className="p-3 text-text-secondary text-xs">Loading notification settings...</div>;
  }

  return (
    <div className="flex flex-col gap-2 p-3 text-xs overflow-y-auto">
      <div className="flex justify-between items-center">
        <h3 className="font-semibold text-sm">Notifications</h3>
        <div className="flex gap-2 items-center">
          <button
            onClick={() => setTab(tab === 'settings' ? 'history' : 'settings')}
            className="text-text-secondary hover:text-text-primary text-xs bg-transparent border-none cursor-pointer"
          >
            {tab === 'settings' ? 'History' : 'Settings'}
          </button>
          <button
            onClick={onClose}
            className="text-text-secondary hover:text-text-primary text-xs bg-transparent border-none cursor-pointer"
          >
            Close
          </button>
        </div>
      </div>

      {tab === 'history' ? (
        <HistoryTab
          history={history}
          onClear={async () => {
            await clearNotificationHistory();
            setHistory([]);
          }}
        />
      ) : (
        <>
          {/* Enable toggle */}
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={settings.enabled}
              onChange={(e) => setSettings({ ...settings, enabled: e.target.checked })}
              className="accent-accent"
            />
            <span>Enable notifications</span>
          </label>

          {/* Notify on statuses */}
          <div className="flex flex-col gap-1">
            <span className="text-text-secondary">Notify on:</span>
            <div className="flex flex-wrap gap-1.5">
              {ALL_STATUSES.map((status) => (
                <label key={status} className="flex items-center gap-1 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.notify_on.includes(status)}
                    onChange={() => toggleStatus(status)}
                    className="accent-accent"
                  />
                  <span className="text-[0.625rem]">{STATUS_LABELS[status]}</span>
                </label>
              ))}
            </div>
          </div>

          {/* Channels */}
          <div className="flex flex-col gap-1.5">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">Channels:</span>
              <div className="flex gap-1 flex-wrap">
                {(['ntfy', 'webhook', 'pushover', 'desktop'] as const).map((t) => (
                  <button key={t} onClick={() => addChannel(t)} className={btnSmall}>
                    + {t}
                  </button>
                ))}
              </div>
            </div>

            {settings.channels.length === 0 && (
              <div className="text-text-secondary text-[0.625rem] italic">
                No channels configured.
              </div>
            )}

            {settings.channels.map((channel, i) => (
              <ChannelEditor
                key={i}
                channel={channel}
                onUpdate={(ch) => updateChannel(i, ch)}
                onRemove={() => removeChannel(i)}
              />
            ))}
          </div>

          {/* Project rules */}
          <div className="flex flex-col gap-1.5">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">Project rules:</span>
              <button onClick={addProjectRule} className={btnSmall}>
                + rule
              </button>
            </div>

            {settings.project_rules.map((rule, i) => (
              <div key={i} className="bg-bg-card rounded-lg p-2 flex flex-col gap-1.5">
                <div className="flex justify-between items-center">
                  <span className="font-semibold text-[0.625rem] uppercase tracking-wider text-text-secondary">
                    Rule {i + 1}
                  </span>
                  <button
                    onClick={() => removeProjectRule(i)}
                    className="text-accent hover:text-text-primary bg-transparent border-none cursor-pointer text-[0.625rem]"
                  >
                    Remove
                  </button>
                </div>
                <input
                  type="text"
                  placeholder="Pattern (e.g. **/my-project)"
                  value={rule.pattern}
                  onChange={(e) => updateProjectRule(i, { ...rule, pattern: e.target.value })}
                  className={inputClass}
                />
                <label className="flex items-center gap-1 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={rule.enabled === false}
                    onChange={(e) =>
                      updateProjectRule(i, { ...rule, enabled: e.target.checked ? false : null })
                    }
                    className="accent-accent"
                  />
                  <span className="text-[0.625rem]">Disable notifications for this project</span>
                </label>
              </div>
            ))}
          </div>

          {/* Actions */}
          <div className="flex gap-2 pt-1">
            <button
              onClick={() => save(settings)}
              disabled={saving}
              className="bg-accent border-none text-text-primary rounded-lg cursor-pointer hover:opacity-80 disabled:opacity-50 py-1 px-3 text-[0.625rem] font-semibold"
            >
              {saving ? 'Saving...' : 'Save'}
            </button>
            <button
              onClick={handleTest}
              disabled={testing || !settings.enabled || settings.channels.length === 0}
              className="bg-bg-card border-none text-text-primary rounded-lg cursor-pointer hover:bg-accent disabled:opacity-50 py-1 px-3 text-[0.625rem]"
            >
              {testing ? 'Sending...' : 'Send Test'}
            </button>
          </div>

          {message && (
            <div
              className={`text-[0.625rem] ${message.type === 'success' ? 'text-success' : 'text-accent'}`}
            >
              {message.text}
            </div>
          )}
        </>
      )}
    </div>
  );
};

const ChannelEditor = ({
  channel,
  onUpdate,
  onRemove,
}: {
  channel: ChannelConfig;
  onUpdate: (ch: ChannelConfig) => void;
  onRemove: () => void;
}) => (
  <div className="bg-bg-card rounded-lg p-2 flex flex-col gap-1.5">
    <div className="flex justify-between items-center">
      <span className="font-semibold text-[0.625rem] uppercase tracking-wider text-text-secondary">
        {channel.type}
      </span>
      <button
        onClick={onRemove}
        className="text-accent hover:text-text-primary bg-transparent border-none cursor-pointer text-[0.625rem]"
      >
        Remove
      </button>
    </div>
    {channel.type === 'ntfy' && (
      <>
        <input
          type="text"
          placeholder="Server URL"
          value={channel.server}
          onChange={(e) => onUpdate({ ...channel, server: e.target.value })}
          className={inputClass}
        />
        <input
          type="text"
          placeholder="Topic"
          value={channel.topic}
          onChange={(e) => onUpdate({ ...channel, topic: e.target.value })}
          className={inputClass}
        />
        <input
          type="password"
          placeholder="Token (optional)"
          value={channel.token ?? ''}
          onChange={(e) => onUpdate({ ...channel, token: e.target.value || null })}
          className={inputClass}
        />
      </>
    )}
    {channel.type === 'webhook' && (
      <input
        type="text"
        placeholder="Webhook URL"
        value={channel.url}
        onChange={(e) => onUpdate({ ...channel, url: e.target.value })}
        className={inputClass}
      />
    )}
    {channel.type === 'pushover' && (
      <>
        <input
          type="text"
          placeholder="User Key"
          value={channel.user_key}
          onChange={(e) => onUpdate({ ...channel, user_key: e.target.value })}
          className={inputClass}
        />
        <input
          type="password"
          placeholder="App Token"
          value={channel.app_token}
          onChange={(e) => onUpdate({ ...channel, app_token: e.target.value })}
          className={inputClass}
        />
        <input
          type="text"
          placeholder="Device (optional)"
          value={channel.device ?? ''}
          onChange={(e) => onUpdate({ ...channel, device: e.target.value || null })}
          className={inputClass}
        />
      </>
    )}
    {channel.type === 'desktop' && (
      <span className="text-text-secondary text-[0.625rem]">
        Uses OS-native notifications. No additional configuration needed.
      </span>
    )}
  </div>
);

const HistoryTab = ({
  history,
  onClear,
}: {
  history: NotificationRecord[];
  onClear: () => void;
}) => (
  <div className="flex flex-col gap-1.5">
    <div className="flex justify-between items-center">
      <span className="text-text-secondary text-[0.625rem]">
        {history.length} notification{history.length !== 1 ? 's' : ''}
      </span>
      {history.length > 0 && (
        <button
          onClick={onClear}
          className="text-accent hover:text-text-primary bg-transparent border-none cursor-pointer text-[0.625rem]"
        >
          Clear
        </button>
      )}
    </div>
    {history.length === 0 && (
      <div className="text-text-secondary text-[0.625rem] italic">No notifications sent yet.</div>
    )}
    {[...history].reverse().map((record, i) => (
      <div key={i} className="bg-bg-card rounded-lg p-2 flex flex-col gap-0.5">
        <div className="flex justify-between items-center">
          <span className="font-semibold text-[0.625rem]">{record.project_name}</span>
          <span className="text-text-secondary text-[0.5rem]">
            {record.timestamp.replace('T', ' ').slice(0, 19)}
          </span>
        </div>
        <span className="text-text-secondary text-[0.625rem]">{record.status}</span>
        <div className="flex gap-1 flex-wrap">
          {record.channels.map((ch, j) => (
            <span
              key={j}
              className={`text-[0.5rem] px-1 rounded ${ch.success ? 'bg-success/20 text-success' : 'bg-accent/20 text-accent'}`}
            >
              {ch.name} {ch.success ? 'OK' : (ch.error ?? 'failed')}
            </span>
          ))}
        </div>
      </div>
    ))}
  </div>
);
