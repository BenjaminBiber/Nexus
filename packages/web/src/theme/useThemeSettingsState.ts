import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ThemeSettingsState } from '@nexus/theme-sdk';
import { useApi } from '../api/ApiContext.js';
import { getTheme } from './registry.js';
import { THEME_SETTINGS_EVENT, resolveThemeSettings } from './themeSettings.js';

// Eigene Einstellungen eines Themes: lädt bei Theme-Wechsel und nach jedem Speichern neu; ohne Schema leer und ohne Server-Aufruf
export function useThemeSettingsState(themeId: string): ThemeSettingsState {
  const api = useApi();
  const schema = getTheme(themeId)?.settingsSchema;
  const [stored, setStored] = useState<{ themeId: string; values: Record<string, unknown> } | null>(null);
  useEffect(() => {
    if (!schema) return;
    let alive = true;
    const load = () => {
      Promise.resolve(api.getThemeSettings(themeId)).then(
        (values) => alive && setStored({ themeId, values }),
        () => alive && setStored({ themeId, values: {} }),
      );
    };
    load();
    const onSaved = (event: Event) => {
      if ((event as CustomEvent<{ themeId: string }>).detail?.themeId === themeId) load();
    };
    window.addEventListener(THEME_SETTINGS_EVENT, onSaved);
    return () => {
      alive = false;
      window.removeEventListener(THEME_SETTINGS_EVENT, onSaved);
    };
  }, [api, themeId, schema]);
  const current = stored?.themeId === themeId ? stored.values : null;
  const values = useMemo(() => resolveThemeSettings(schema, current ?? {}), [schema, current]);
  const set = useCallback(
    async (key: string, value: boolean | string) => {
      const saved = await api.saveThemeSettings(themeId, { ...values, [key]: value });
      setStored({ themeId, values: saved });
      window.dispatchEvent(new CustomEvent(THEME_SETTINGS_EVENT, { detail: { themeId } }));
    },
    [api, themeId, values],
  );
  return useMemo(() => ({ values, loading: Boolean(schema) && current === null, set }), [values, schema, current, set]);
}
