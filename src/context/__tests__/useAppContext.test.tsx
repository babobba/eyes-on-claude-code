import { describe, it, expect } from 'vitest';
import { renderHook } from '@testing-library/react';
import { type ReactNode } from 'react';
import { useAppContext } from '../useAppContext';
import { AppContext, type AppContextValue } from '../appContextStore';

describe('useAppContext', () => {
  it('throws when used outside AppProvider', () => {
    expect(() => {
      renderHook(() => useAppContext());
    }).toThrow('useAppContext must be used within AppProvider');
  });

  it('returns context value when used within AppProvider', () => {
    const mockValue: AppContextValue = {
      dashboardData: { sessions: [], events: [] },
      settings: {
        always_on_top: true,
        minimum_mode_enabled: true,
        opacity_active: 1.0,
        opacity_inactive: 1.0,
        sound_enabled: true,
      },
      isLoading: false,
      refreshData: async () => {},
    };

    const wrapper = ({ children }: { children: ReactNode }) => (
      <AppContext.Provider value={mockValue}>{children}</AppContext.Provider>
    );

    const { result } = renderHook(() => useAppContext(), { wrapper });
    expect(result.current).toBe(mockValue);
    expect(result.current.dashboardData.sessions).toEqual([]);
    expect(result.current.isLoading).toBe(false);
  });
});
