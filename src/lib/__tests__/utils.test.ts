import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { getStatusEmoji, getStatusClass, allHooksConfigured, formatRelativeTime } from '../utils';
import type { HookStatus } from '@/types';

describe('getStatusEmoji', () => {
  it('returns correct emoji for each status', () => {
    expect(getStatusEmoji('Active')).toBe('🟢');
    expect(getStatusEmoji('WaitingPermission')).toBe('🔐');
    expect(getStatusEmoji('WaitingInput')).toBe('⏳');
    expect(getStatusEmoji('Completed')).toBe('✅');
  });

  it('returns fallback emoji for unknown status', () => {
    expect(getStatusEmoji('Unknown' as never)).toBe('📌');
  });
});

describe('getStatusClass', () => {
  it('returns "waiting" for WaitingPermission', () => {
    expect(getStatusClass('WaitingPermission')).toBe('waiting');
  });

  it('returns "waiting" for WaitingInput', () => {
    expect(getStatusClass('WaitingInput')).toBe('waiting');
  });

  it('returns "completed" for Completed', () => {
    expect(getStatusClass('Completed')).toBe('completed');
  });

  it('returns "active" for Active', () => {
    expect(getStatusClass('Active')).toBe('active');
  });

  it('returns "active" as default for unknown status', () => {
    expect(getStatusClass('Unknown' as never)).toBe('active');
  });
});

describe('allHooksConfigured', () => {
  const allTrue: HookStatus = {
    session_start: true,
    session_end: true,
    stop: true,
    post_tool_use: true,
    user_prompt_submit: true,
    notification_permission: true,
    notification_idle: true,
  };

  it('returns true when all hooks are configured', () => {
    expect(allHooksConfigured(allTrue)).toBe(true);
  });

  it('returns false when session_start is missing', () => {
    expect(allHooksConfigured({ ...allTrue, session_start: false })).toBe(false);
  });

  it('returns false when session_end is missing', () => {
    expect(allHooksConfigured({ ...allTrue, session_end: false })).toBe(false);
  });

  it('returns false when stop is missing', () => {
    expect(allHooksConfigured({ ...allTrue, stop: false })).toBe(false);
  });

  it('returns false when post_tool_use is missing', () => {
    expect(allHooksConfigured({ ...allTrue, post_tool_use: false })).toBe(false);
  });

  it('returns false when user_prompt_submit is missing', () => {
    expect(allHooksConfigured({ ...allTrue, user_prompt_submit: false })).toBe(false);
  });

  it('returns false when notification_permission is missing', () => {
    expect(allHooksConfigured({ ...allTrue, notification_permission: false })).toBe(false);
  });

  it('returns false when notification_idle is missing', () => {
    expect(allHooksConfigured({ ...allTrue, notification_idle: false })).toBe(false);
  });

  it('returns false when all hooks are missing', () => {
    const allFalse: HookStatus = {
      session_start: false,
      session_end: false,
      stop: false,
      post_tool_use: false,
      user_prompt_submit: false,
      notification_permission: false,
      notification_idle: false,
    };
    expect(allHooksConfigured(allFalse)).toBe(false);
  });
});

describe('formatRelativeTime', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2025-06-15T12:00:00Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns empty string for empty input', () => {
    expect(formatRelativeTime('')).toBe('');
  });

  it('returns empty string for invalid date', () => {
    expect(formatRelativeTime('not-a-date')).toBe('');
  });

  it('returns "just now" for timestamps less than 60 seconds ago', () => {
    const thirtySecondsAgo = new Date(Date.now() - 30 * 1000).toISOString();
    expect(formatRelativeTime(thirtySecondsAgo)).toBe('just now');
  });

  it('returns "just now" for future dates', () => {
    const future = new Date(Date.now() + 60 * 1000).toISOString();
    expect(formatRelativeTime(future)).toBe('just now');
  });

  it('returns minutes for timestamps 1-59 minutes ago', () => {
    const fiveMinutesAgo = new Date(Date.now() - 5 * 60 * 1000).toISOString();
    expect(formatRelativeTime(fiveMinutesAgo)).toBe('5m ago');
  });

  it('returns hours for timestamps 1-23 hours ago', () => {
    const threeHoursAgo = new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString();
    expect(formatRelativeTime(threeHoursAgo)).toBe('3h ago');
  });

  it('returns days for timestamps 1-6 days ago', () => {
    const twoDaysAgo = new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString();
    expect(formatRelativeTime(twoDaysAgo)).toBe('2d ago');
  });

  it('returns formatted date for timestamps 7+ days ago', () => {
    const twoWeeksAgo = new Date(Date.now() - 14 * 24 * 60 * 60 * 1000).toISOString();
    const result = formatRelativeTime(twoWeeksAgo);
    expect(result).toMatch(/Jun \d+/);
  });
});
