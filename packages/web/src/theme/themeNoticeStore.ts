import { useSyncExternalStore } from 'react';

/** Ereignis nach jeder Änderung des Theme-Hinweises */
export const THEME_NOTICE_EVENT = 'nexus-theme-notice';

let notice: string | undefined;

// Setzt den Hinweis zum Laden des Themes (undefined entfernt ihn)
export function setThemeNotice(message: string | undefined): void {
  notice = message;
  window.dispatchEvent(new Event(THEME_NOTICE_EVENT));
}

// Aktueller Hinweis oder undefined
export function getThemeNotice(): string | undefined {
  return notice;
}

// Meldet sich für Änderungen des Hinweises an
function subscribe(listener: () => void): () => void {
  window.addEventListener(THEME_NOTICE_EVENT, listener);
  return () => window.removeEventListener(THEME_NOTICE_EVENT, listener);
}

// Hinweis zum Laden des Themes; rendert bei jeder Änderung neu
export function useThemeNotice(): string | undefined {
  return useSyncExternalStore(subscribe, getThemeNotice);
}
