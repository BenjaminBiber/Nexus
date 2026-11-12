import { useSyncExternalStore } from 'react';

/** Ereignis nach jeder Änderung des Theme-Hinweises */
export const THEME_NOTICE_EVENT = 'nexus-theme-notice';

/** Hinweis zum Theme: Fehler eines kaputten Themes oder Info (z. B. abgesicherter Modus) */
export type ThemeNoticeEntry = { message: string; kind: 'error' | 'info' };

let notice: ThemeNoticeEntry | undefined;

// Setzt den Hinweis zum Laden des Themes (Text = Fehler, undefined entfernt ihn)
export function setThemeNotice(value: string | ThemeNoticeEntry | undefined): void {
  notice = typeof value === 'string' ? { message: value, kind: 'error' } : value;
  window.dispatchEvent(new Event(THEME_NOTICE_EVENT));
}

// Text des aktuellen Hinweises oder undefined
export function getThemeNotice(): string | undefined {
  return notice?.message;
}

// Aktueller Hinweis mit Art oder undefined
export function getThemeNoticeEntry(): ThemeNoticeEntry | undefined {
  return notice;
}

// Meldet sich für Änderungen des Hinweises an
function subscribe(listener: () => void): () => void {
  window.addEventListener(THEME_NOTICE_EVENT, listener);
  return () => window.removeEventListener(THEME_NOTICE_EVENT, listener);
}

// Hinweis zum Laden des Themes; rendert bei jeder Änderung neu
export function useThemeNotice(): ThemeNoticeEntry | undefined {
  return useSyncExternalStore(subscribe, getThemeNoticeEntry);
}
