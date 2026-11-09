/** URL-Parameter des abgesicherten Modus: kein Theme-Code und keine Theme-CSS, nur Hell/Dunkel */
export const SAFE_THEME_PARAM = 'safe-theme';

// Ist der abgesicherte Modus aktiv? (?safe-theme, auch mit Wert oder neben anderen Parametern)
export function isSafeTheme(search: string = window.location.search): boolean {
  return new URLSearchParams(search).has(SAFE_THEME_PARAM);
}

// Dieselbe Adresse mit ?safe-theme (Pfad, Hash und andere Parameter bleiben)
export function safeThemeUrl(href: string): string {
  const url = new URL(href);
  if (!url.searchParams.has(SAFE_THEME_PARAM)) url.search = url.search ? `${url.search}&${SAFE_THEME_PARAM}` : `?${SAFE_THEME_PARAM}`;
  return url.toString();
}

// Dieselbe Adresse ohne safe-theme
export function normalThemeUrl(href: string): string {
  const url = new URL(href);
  url.searchParams.delete(SAFE_THEME_PARAM);
  return url.toString();
}
