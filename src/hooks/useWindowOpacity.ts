import { useEffect, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

export const useWindowOpacity = (activeOpacity: number, inactiveOpacity: number) => {
  // Use refs to always access the latest opacity values
  // This avoids race conditions when re-registering listeners
  const activeOpacityRef = useRef(activeOpacity);
  const inactiveOpacityRef = useRef(inactiveOpacity);

  // Keep refs in sync with props and apply immediately
  useEffect(() => {
    activeOpacityRef.current = activeOpacity;
    inactiveOpacityRef.current = inactiveOpacity;

    // Apply new opacity immediately based on current focus state
    getCurrentWindow()
      .isFocused()
      .then((focused) => {
        const opacity = focused ? activeOpacity : inactiveOpacity;
        document.body.style.setProperty('opacity', String(opacity), 'important');
      })
      .catch(console.error);
  }, [activeOpacity, inactiveOpacity]);

  // Register listener once, use refs for current values
  useEffect(() => {
    const applyOpacity = (active: boolean) => {
      const opacity = active ? activeOpacityRef.current : inactiveOpacityRef.current;
      document.body.style.setProperty('opacity', String(opacity), 'important');
    };

    let unlisten: UnlistenFn | undefined;

    // Listen for custom dashboard-active event from Rust backend
    listen<boolean>('dashboard-active', (event) => {
      applyOpacity(event.payload);
    }).then((u) => {
      unlisten = u;
    });

    // Set initial opacity based on current focus state
    getCurrentWindow()
      .isFocused()
      .then((focused) => applyOpacity(focused))
      .catch(console.error);

    return () => {
      unlisten?.();
    };
  }, []); // Empty dependency - listener registered once, refs provide current values
};
