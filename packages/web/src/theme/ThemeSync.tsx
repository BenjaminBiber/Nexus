import { useEffect } from 'react';
import { useApi } from '../api/ApiContext.js';
import { applyTheme } from './theme.js';

// Übernimmt beim Start das gespeicherte Farbschema und folgt bei „system“ dem Wechsel im Betriebssystem.
export function ThemeSync() {
  const api = useApi();
  useEffect(() => {
    let cancelled = false;
    // Installierte Themes parallel laden; schlägt das fehl, bleibt der Link aus dem Cache (kein Flackern)
    const manifests = Promise.resolve()
      .then(() => api.listThemes())
      .catch(() => undefined);
    Promise.resolve()
      .then(() => api.getUserSettings())
      .then(
        async (settings) => {
          const installed = await manifests;
          if (!cancelled) applyTheme(settings.theme ?? 'system', installed);
        },
        () => undefined,
      );
    const media = typeof window.matchMedia === 'function' ? window.matchMedia('(prefers-color-scheme: dark)') : null;
    const onSystemChange = () => {
      if ((document.documentElement.dataset.themeMode ?? 'system') === 'system') applyTheme('system');
    };
    media?.addEventListener('change', onSystemChange);
    return () => {
      cancelled = true;
      media?.removeEventListener('change', onSystemChange);
    };
  }, [api]);
  return null;
}
