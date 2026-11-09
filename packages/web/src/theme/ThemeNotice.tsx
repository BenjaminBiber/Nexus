import { isSafeTheme, normalThemeUrl, safeThemeUrl } from './safeTheme.js';
import { setThemeNotice, useThemeNotice } from './themeNoticeStore.js';

interface Props {
  /** Öffnet eine Adresse (Tests geben einen Spion mit) */
  assign?: (url: string) => void;
}

// Schließbarer Balken mit dem Hinweis zum Laden des Themes; bei Fehlern mit Weg in den abgesicherten Modus, darin mit Weg zurück
export function ThemeNotice({ assign = (url) => window.location.assign(url) }: Props) {
  const notice = useThemeNotice();
  if (!notice) return null;
  const safe = isSafeTheme();
  return (
    <div className="theme-notice" role="status">
      <span>{notice.message}</span>
      {notice.kind === 'error' && !safe && (
        <button type="button" className="theme-notice-action" onClick={() => assign(safeThemeUrl(window.location.href))}>
          Abgesichert neu laden
        </button>
      )}
      {safe && (
        <button type="button" className="theme-notice-action" onClick={() => assign(normalThemeUrl(window.location.href))}>
          Normal starten
        </button>
      )}
      <button type="button" aria-label="Hinweis schließen" onClick={() => setThemeNotice(undefined)}>
        ×
      </button>
    </div>
  );
}
