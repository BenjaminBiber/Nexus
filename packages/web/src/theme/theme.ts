import { BUILTIN_THEME_IDS, type ThemeMode } from '../api/types.js';
import { getTheme } from './registry.js';
import { isSafeTheme } from './safeTheme.js';
import { SNIPPETS_LINK_ID } from './snippets.js';

/** Ereignis nach jedem Wechsel des Farbschemas (Mermaid zeichnet daraufhin neu) */
export const THEME_EVENT = 'harness-theme';

/** Schlüssel im localStorage; index.html setzt damit das Schema schon vor dem ersten Zeichnen */
export const THEME_CACHE_KEY = 'harness-theme-mode';

/** Schlüssel im localStorage für die URL der theme.css eines installierten Themes (index.html bindet sie früh ein) */
export const THEME_CSS_CACHE_KEY = 'harness-theme-css';

/** Schlüssel im localStorage für die Startfarbe eines installierten Themes (Hintergrund der Startanimation) */
export const THEME_START_COLOR_KEY = 'harness-theme-start-color';

/** ID des Links auf die theme.css des aktiven installierten Themes */
export const THEME_LINK_ID = 'nexus-theme-css';


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

/** Installiertes Theme, wie es applyTheme braucht (Auszug aus dem Manifest) */
export type InstalledTheme = string | { id: string; startColor?: string | null };

// URL der theme.css eines installierten Themes (vom Server unter /themes/<id>/ ausgeliefert)
export function themeCssUrl(id: string): string {
  return `/themes/${id}/theme.css`;
}

// Bindet die theme.css eines installierten Themes ein (ersetzt einen vorhandenen Link) oder entfernt sie (id = null)
function setThemeLink(id: string | null): void {
  const existing = document.getElementById(THEME_LINK_ID);
  if (id === null) {
    existing?.remove();
    return;
  }
  const href = themeCssUrl(id);
  if (existing instanceof HTMLLinkElement && existing.getAttribute('href') === href) return;
  const link = document.createElement('link');
  link.id = THEME_LINK_ID;
  link.rel = 'stylesheet';
  link.href = href;
  if (existing) existing.replaceWith(link);
  else {
    // CSS-Schnipsel des Nutzers liegen über dem aktiven Theme (D8)
    const snippets = document.getElementById(SNIPPETS_LINK_ID);
    if (snippets?.parentNode === document.head) document.head.insertBefore(link, snippets);
    else document.head.appendChild(link);
  }
}

// Setzt das Schema am Wurzelelement (data-theme = angezeigtes Schema,data-theme-mode = Einstellung), merkt es sich und meldet den Wechsel
// Ohne Liste der installierten Themes (lädt noch) gilt jede syntaktisch gültige ID, sonst nur eingebaute und installierte.
// Installierte Themes bringen ihre theme.css als Link mit; URL und Startfarbe landen für den frühen Start im localStorage.
export function applyTheme(requested: ThemeMode, installed?: readonly InstalledTheme[]): void {
  const ids = installed?.map((theme) => (typeof theme === 'string' ? theme : theme.id));
  // Im Code angemeldete Themes gelten auch ohne Paket als bekannt, bringen aber keine theme.css mit
  const mode = ids ? normalizeThemeId(requested, getTheme(requested) ? [...ids, requested] : ids) : THEME_ID_PATTERN.test(requested) ? requested : 'system';
  // Abgesicherter Modus: installierte Themes zeigen nur Hell/Dunkel nach System, ohne theme.css; die Einstellung bleibt gespeichert
  const safe = !(BUILTIN_THEME_IDS as readonly string[]).includes(mode) && isSafeTheme();
  const resolved = resolveTheme(safe ? 'system' : mode, systemPrefersDark());
  const external = !(BUILTIN_THEME_IDS as readonly string[]).includes(mode) && (ids ? ids.includes(mode) : !getTheme(mode));
  document.documentElement.dataset.theme = resolved;
  document.documentElement.dataset.themeMode = mode;
  setThemeLink(external && !safe ? mode : null);
  if (safe) {
    window.dispatchEvent(new CustomEvent(THEME_EVENT, { detail: resolved }));
    return;
  }
  const manifest = installed?.find((theme): theme is Exclude<InstalledTheme, string> => typeof theme !== 'string' && theme.id === mode);
  try {
    window.localStorage.setItem(THEME_CACHE_KEY, mode);
    if (external) {
      window.localStorage.setItem(THEME_CSS_CACHE_KEY, themeCssUrl(mode));
      // Ohne Liste (lädt noch) bleibt die gemerkte Startfarbe stehen
      if (manifest?.startColor) window.localStorage.setItem(THEME_START_COLOR_KEY, manifest.startColor);
      else if (installed) window.localStorage.removeItem(THEME_START_COLOR_KEY);
    } else {
      window.localStorage.removeItem(THEME_CSS_CACHE_KEY);
      window.localStorage.removeItem(THEME_START_COLOR_KEY);
    }
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
