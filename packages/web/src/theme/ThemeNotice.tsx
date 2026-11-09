import { setThemeNotice, useThemeNotice } from './themeNoticeStore.js';

// Schließbarer Balken mit dem Hinweis zum Laden des Themes (z. B. Rückfall auf den Standard)
export function ThemeNotice() {
  const notice = useThemeNotice();
  if (!notice) return null;
  return (
    <div className="theme-notice" role="status">
      <span>{notice}</span>
      <button type="button" aria-label="Hinweis schließen" onClick={() => setThemeNotice(undefined)}>
        ×
      </button>
    </div>
  );
}
