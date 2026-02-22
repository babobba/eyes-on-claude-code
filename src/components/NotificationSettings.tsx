import { useState, useEffect, useCallback } from 'react';
import type { NotificationSettings as NotificationSettingsType, ChannelConfig } from '@/types';
import {
  getNotificationSettings,
  updateNotificationSettings,
  sendTestNotification,
} from '@/lib/tauri';

const ALL_STATUSES = ['WaitingPermission', 'WaitingInput', 'Completed', 'Active'] as const;
const STATUS_LABELS: Record<string, string> = {
  WaitingPermission: 'Waiting for permission',
  WaitingInput: 'Waiting for input',
  Completed: 'Completed',
  Active: 'Active',
};

interface NotificationSettingsProps {
  onClose: () => void;
}

export const NotificationSettings = ({ onClose }: NotificationSettingsProps) => {
  const [settings, setSettings] = useState<NotificationSettingsType | null>(null);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [message, setMessage] = useState<{ text: string; type: 'success' | 'error' } | null>(null);

  useEffect(() => {
    getNotificationSettings()
      .then(setSettings)
      .catch((err) => console.error('Failed to load notification settings:', err));
  }, []);

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

  const addChannel = (type: 'ntfy' | 'webhook') => {
    if (!settings) return;
    const newChannel: ChannelConfig =
      type === 'ntfy'
        ? { type: 'ntfy', server: 'https://ntfy.sh', topic: '', token: null }
        : { type: 'webhook', url: '' };
    const updated = { ...settings, channels: [...settings.channels, newChannel] };
    setSettings(updated);
  };

  const removeChannel = (index: number) => {
    if (!settings) return;
    const updated = { ...settings, channels: settings.channels.filter((_, i) => i !== index) };
    setSettings(updated);
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

  if (!settings) {
    return <div className="p-3 text-text-secondary text-xs">Loading notification settings...</div>;
  }

  return (
    <div className="flex flex-col gap-2 p-3 text-xs">
      <div className="flex justify-between items-center">
        <h3 className="font-semibold text-sm">Notifications</h3>
        <button
          onClick={onClose}
          className="text-text-secondary hover:text-text-primary text-xs bg-transparent border-none cursor-pointer"
        >
          Close
        </button>
      </div>

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
          <div className="flex gap-1">
            <button
              onClick={() => addChannel('ntfy')}
              className="bg-bg-card border-none text-text-primary rounded cursor-pointer hover:bg-accent py-0.5 px-1.5 text-[0.625rem]"
            >
              + ntfy
            </button>
            <button
              onClick={() => addChannel('webhook')}
              className="bg-bg-card border-none text-text-primary rounded cursor-pointer hover:bg-accent py-0.5 px-1.5 text-[0.625rem]"
            >
              + webhook
            </button>
          </div>
        </div>

        {settings.channels.length === 0 && (
          <div className="text-text-secondary text-[0.625rem] italic">
            No channels configured. Add an ntfy or webhook channel above.
          </div>
        )}

        {settings.channels.map((channel, i) => (
          <div key={i} className="bg-bg-card rounded-lg p-2 flex flex-col gap-1.5">
            <div className="flex justify-between items-center">
              <span className="font-semibold text-[0.625rem] uppercase tracking-wider text-text-secondary">
                {channel.type}
              </span>
              <button
                onClick={() => removeChannel(i)}
                className="text-accent hover:text-text-primary bg-transparent border-none cursor-pointer text-[0.625rem]"
              >
                Remove
              </button>
            </div>
            {channel.type === 'ntfy' ? (
              <>
                <input
                  type="text"
                  placeholder="Server URL"
                  value={channel.server}
                  onChange={(e) => updateChannel(i, { ...channel, server: e.target.value })}
                  className="bg-bg-secondary border border-bg-card rounded px-2 py-1 text-text-primary text-[0.625rem] outline-none focus:border-accent"
                />
                <input
                  type="text"
                  placeholder="Topic"
                  value={channel.topic}
                  onChange={(e) => updateChannel(i, { ...channel, topic: e.target.value })}
                  className="bg-bg-secondary border border-bg-card rounded px-2 py-1 text-text-primary text-[0.625rem] outline-none focus:border-accent"
                />
                <input
                  type="password"
                  placeholder="Token (optional)"
                  value={channel.token ?? ''}
                  onChange={(e) =>
                    updateChannel(i, {
                      ...channel,
                      token: e.target.value || null,
                    })
                  }
                  className="bg-bg-secondary border border-bg-card rounded px-2 py-1 text-text-primary text-[0.625rem] outline-none focus:border-accent"
                />
              </>
            ) : (
              <input
                type="text"
                placeholder="Webhook URL"
                value={channel.url}
                onChange={(e) => updateChannel(i, { ...channel, url: e.target.value })}
                className="bg-bg-secondary border border-bg-card rounded px-2 py-1 text-text-primary text-[0.625rem] outline-none focus:border-accent"
              />
            )}
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

      {/* Status message */}
      {message && (
        <div
          className={`text-[0.625rem] ${message.type === 'success' ? 'text-success' : 'text-accent'}`}
        >
          {message.text}
        </div>
      )}
    </div>
  );
};
