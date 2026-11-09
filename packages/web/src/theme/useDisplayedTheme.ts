import { useEffect, useState } from 'react';
import { THEME_EVENT, displayedTheme } from './theme.js';

// Angezeigtes Schema; zieht bei jedem Wechsel (applyTheme) nach
export function useDisplayedTheme(): string {
  const [theme, setTheme] = useState(displayedTheme);
  useEffect(() => {
    const update = () => setTheme(displayedTheme());
    window.addEventListener(THEME_EVENT, update);
    return () => window.removeEventListener(THEME_EVENT, update);
  }, []);
  return theme;
}
