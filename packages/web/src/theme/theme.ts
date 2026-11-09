import { BUILTIN_THEME_IDS, type ThemeMode } from '../api/types.js';

/** Ereignis nach jedem Wechsel des Farbschemas (Mermaid zeichnet daraufhin neu) */
export const THEME_EVENT = 'harness-theme';

/** Schlüssel im localStorage; index.html setzt damit das Schema schon vor dem ersten Zeichnen */
export const THEME_CACHE_KEY = 'harness-theme-mode';

/** Angezeigtes Schema als ID; „signal“ und „pastel“ sind helle Schemata mit eigener Palette, „win95“ zusätzlich mit eigenem Rahmen (Desktop, Fenster, Taskleiste), sonst ein installiertes Theme */
export type ResolvedTheme = string;

/** Erlaubte Form einer Theme-ID (wie im Manifest) */
const THEME_ID_PATTERN = /^[a-z0-9-]{2,40}$/;

// Theme-ID oder „system“, wenn sie weder eingebaut noch in `installed` ist
export function normalizeThemeId(mode: string, installed: readonly string[]): ThemeMode {
  return (BUILTIN_THEME_IDS as readonly string[]).includes(mode) || installed.includes(mode) ? mode : 'system';
}

// Angezeigtes Schema für eine Einstellung; „system“ folgt dem Betriebssystem
export function resolveTheme(mode: ThemeMode, prefersDark: boolean): ResolvedTheme {
  return mode === 'system' ? (prefersDark ? 'dark' : 'light') : mode;
}

// Bevorzugt das System ein dunkles Schema? (ohne matchMedia, z. B. in Tests: nein)
export function systemPrefersDark(): boolean {
  return typeof window.matchMedia === 'function' && window.matchMedia('(prefers-color-scheme: dark)').matches;
}

// Setzt das Schema am Wurzelelement (data-theme = angezeigtes Schema,data-theme-mode = Einstellung), merkt es sich und meldet den Wechsel
// Ohne Liste der installierten Themes (lädt noch) gilt jede syntaktisch gültige ID, sonst nur eingebaute und installierte
export function applyTheme(requested: ThemeMode, installed?: readonly string[]): void {
  const mode = installed ? normalizeThemeId(requested, installed) : THEME_ID_PATTERN.test(requested) ? requested : 'system';
  const resolved = resolveTheme(mode, systemPrefersDark());
  document.documentElement.dataset.theme = resolved;
  document.documentElement.dataset.themeMode = mode;
  try {
    window.localStorage.setItem(THEME_CACHE_KEY, mode);
  } catch {
    // localStorage gesperrt (z. B. privates Fenster): nur kein früher Start im richtigen Schema
  }
  window.dispatchEvent(new CustomEvent(THEME_EVENT, { detail: resolved }));
}

// Aktuell angezeigtes Schema (jede ID, auch von Themes außerhalb der eingebauten Liste); ohne Angabe hell
export function displayedTheme(): string {
  return document.documentElement.dataset.theme || 'light';
}

// Aktuell angezeigtes Schema, auf hell/dunkel abgebildet (Signal, Pastell und Windows 95 zählen als hell)
export function currentTheme(): 'light' | 'dark' {
  return document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light';
}
