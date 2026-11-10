import type { ThemeSettingsState } from './settings.js';
import type { NavIconName, ShellParts } from './slots.js';

/** Eintrag der Hauptnavigation */
export interface NavItem {
  id: NavIconName;
  label: string;
  /** Ziel der Route, z. B. `/p/<pid>/board` */
  href: string;
  /** Zeigt die offene Seite auf dieses Ziel? */
  active: boolean;
  /** Zähler am Eintrag (z. B. offene Aufgaben) */
  badge?: number;
}

/** Ein Board samt Projekt oder ein Projekt ohne Board */
export interface BoardRef {
  project: string;
  board: number | null;
}

/** Projekte und Boards der Board-Datenbank samt aktueller Wahl */
export interface BoardsState {
  current: BoardRef | null;
  projects: { id: string; name: string }[];
  boards: { id: number; project: string; name: string }[];
  loading: boolean;
  /** Wählt ein Board bzw. Projekt (wie die Board-Auswahl der Seitenleiste) */
  choose(ref: BoardRef): void;
}

/** Auslastung eines Limit-Fensters; `resetsAt` ist ein ISO-Zeitpunkt */
export interface UsageWindow {
  percent: number;
  resetsAt: string | null;
}

/** Claude-Verbrauch des ersten aktiven Accounts; `reason` erklärt fehlende Werte */
export interface UsageState {
  enabled: boolean;
  available: boolean;
  reason: string | null;
  fiveHour: UsageWindow | null;
  sevenDay: UsageWindow | null;
}

/** Nutzereinstellungen, die ein Theme lesen und ändern darf */
export interface ThemeSettings {
  theme: string;
  claudeUsage: boolean;
  compactCards: boolean;
  heroImages: boolean;
}

/** Einstellungen des Nutzers; `values` ist null, solange sie laden */
export interface SettingsState {
  values: ThemeSettings | null;
  loading: boolean;
  update(patch: Partial<ThemeSettings>): Promise<void>;
}

/** Angezeigtes Farbschema: hell/dunkel, ID des Themes (data-theme) und Einstellung (data-theme-mode) */
export interface ColorSchemeState {
  scheme: 'light' | 'dark';
  themeId: string;
  mode: string;
}

/** Vertrag zwischen Nexus-Host und Theme: alles, was die Hooks des SDK liefern */
export interface ThemeHost {
  nav: NavItem[];
  boards: BoardsState;
  usage: UsageState;
  settings: SettingsState;
  /** Eigene Einstellungen des angezeigten Themes (`settingsSchema`) */
  themeSettings: ThemeSettingsState;
  colorScheme: ColorSchemeState;
  navigate(to: string): void;
  /** Öffnet die globale Suche des Hosts */
  openSearch(): void;
  /** Fertige Bausteine des Rahmens (wie `ShellProps.parts`) */
  parts: ShellParts;
}
