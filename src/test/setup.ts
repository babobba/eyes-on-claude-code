import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock @tauri-apps/api/event
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock @tauri-apps/api/window
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    setFocus: vi.fn(),
  })),
  getAllWindows: vi.fn(() => Promise.resolve([])),
}));

// Mock AudioContext for sound tests
class MockOscillator {
  type = 'sine';
  frequency = { value: 440 };
  connect = vi.fn();
  start = vi.fn();
  stop = vi.fn();
}

class MockGainNode {
  gain = { value: 1, exponentialRampToValueAtTime: vi.fn() };
  connect = vi.fn();
}

class MockAudioContext {
  currentTime = 0;
  destination = {};
  createOscillator = vi.fn(() => new MockOscillator());
  createGain = vi.fn(() => new MockGainNode());
}

Object.defineProperty(window, 'AudioContext', {
  value: MockAudioContext,
  writable: true,
});
