import { createContext, useContext, type ReactNode } from 'react';
import type { ThemeHost } from './host.js';
import type { ThemeSettingValues, ThemeSettingsSchema, ThemeSettingsState } from './settings.js';

/** Context mit der Umsetzung des Hosts; null außerhalb des Providers */
const ThemeHostContext = createContext<ThemeHost | null>(null);

/** Stellt den Themes die Umsetzung des Hosts bereit (nur der Nexus-Host rendert ihn) */
export function ThemeHostProvider({ value, children }: { value: ThemeHost; children?: ReactNode }) {
  return <ThemeHostContext.Provider value={value}>{children}</ThemeHostContext.Provider>;
}

// Host aus dem Context; ohne Provider ein Fehler, der Hook und Provider nennt (intern, nicht im Index)
export function useHost(name: string): ThemeHost {
  const host = useContext(ThemeHostContext);
  if (!host) throw new Error(`${name} braucht den ThemeHostProvider des Nexus-Hosts – die Komponente läuft außerhalb der Nexus-Oberfläche.`);
  return host;
}

/** Einträge der Hauptnavigation */
export const useNav = () => useHost('useNav').nav;
/** Projekte, Boards und Board-Wahl */
export const useBoards = () => useHost('useBoards').boards;
/** Claude-Verbrauch */
export const useUsage = () => useHost('useUsage').usage;
/** Nutzereinstellungen lesen und ändern */
export const useSettings = () => useHost('useSettings').settings;
/** Eigene Einstellungen des Themes lesen und ändern; `S` ist das `settingsSchema` des Themes */
export function useThemeSettings<S extends ThemeSettingsSchema = ThemeSettingsSchema>(): ThemeSettingsState & { values: ThemeSettingValues<S> } {
  return useHost('useThemeSettings').themeSettings as ThemeSettingsState & { values: ThemeSettingValues<S> };
}
/** Angezeigtes Farbschema */
export const useColorScheme = () => useHost('useColorScheme').colorScheme;
/** Route wechseln */
export const useNavigate = () => useHost('useNavigate').navigate;

/** Globale Suche des Hosts öffnen */
export const useOpenSearch = () => useHost('useOpenSearch').openSearch;
