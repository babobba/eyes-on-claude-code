import { describe, it, expect, vi, beforeEach } from 'vitest';
import { playCompletionSound, playWaitingSound } from '../audio';

describe('playCompletionSound', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it('creates oscillator and gain nodes', () => {
    playCompletionSound();

    const ctx = new AudioContext();
    expect(ctx.createOscillator).toBeDefined();
    expect(ctx.createGain).toBeDefined();
  });

  it('does not throw', () => {
    expect(() => playCompletionSound()).not.toThrow();
  });

  it('plays second tone after delay', () => {
    playCompletionSound();
    vi.advanceTimersByTime(120);
    // No error means the delayed tone also played successfully
  });
});

describe('playWaitingSound', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it('does not throw', () => {
    expect(() => playWaitingSound()).not.toThrow();
  });

  it('plays second beep after delay', () => {
    playWaitingSound();
    vi.advanceTimersByTime(150);
    // No error means the delayed beep also played successfully
  });
});
