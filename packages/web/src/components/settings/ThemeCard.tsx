import type { CSSProperties } from 'react';

/** Farben der Mini-Vorschau einer Theme-Kachel */
export type ThemePalette = { bg: string; side: string; card: string; line: string; accent: string; text: string };

// Werte aus variables.css (hell bzw. dunkel); installierte Themes ohne Vorschaubild bauen darauf auf
export const LIGHT_PALETTE: ThemePalette = { bg: '#f7f7f8', side: '#fbfbfc', card: '#ffffff', line: '#e8e8ec', accent: '#4f46e5', text: '#111114' };
export const DARK_PALETTE: ThemePalette = { bg: '#0c0c0e', side: '#111114', card: '#161619', line: '#26262b', accent: '#8b8cf8', text: '#ededf0' };

/** Ein wählbares Theme: eingebauter Modus (System/Hell/Dunkel) oder installiertes Paket */
export type ThemeEntry = {
  id: string;
  name: string;
  /** Zeile unter dem Namen, z. B. „Ada · v1.2.0“ */
  meta: string;
  builtin: boolean;
  scheme: 'light' | 'dark' | 'system';
  /** Ersetzt Teile der Oberfläche mit eigenem Code (Slots) */
  hasCode: boolean;
  /** URL eines Vorschaubilds aus dem Paket */
  image?: string;
  palette: ThemePalette;
};

const SCHEME_LABEL: Record<ThemeEntry['scheme'], string> = { light: 'Hell', dark: 'Dunkel', system: 'Folgt System' };

type Props = {
  theme: ThemeEntry;
  active: boolean;
  disabled: boolean;
  onApply: () => void;
  onRemove?: () => void;
};

// Kachel eines Themes: Vorschau, Name mit „aktuell“, Herkunft und Merkmale; ein Klick auf die Kachel wendet das Theme an,
// installierte Themes lassen sich über den Papierkorb neben dem Namen entfernen
export function ThemeCard({ theme, active, disabled, onApply, onRemove }: Props) {
  return (
    <li className={`theme-card${active ? ' active' : ''}`} aria-label={theme.name} aria-current={active ? 'true' : undefined}>
      {/* Liegt über der ganzen Kachel, damit sie überall klickbar ist; der Entfernen-Knopf liegt darüber */}
      <button
        type="button"
        className="theme-select"
        aria-label={active ? `${theme.name} in Verwendung` : `${theme.name} anwenden`}
        title={active ? undefined : 'Anwenden'}
        disabled={disabled || active}
        onClick={onApply}
      />
      <ThemePreview theme={theme} />
      <div className="theme-card-title">
        <div className="theme-card-name">
          <strong>{theme.name}</strong>
          {active && <span className="theme-badge current">aktuell</span>}
          {onRemove && (
            <button type="button" className="icon-btn theme-remove" aria-label={`${theme.name} entfernen`} title="Entfernen" disabled={disabled} onClick={onRemove}>
              <span className="material-icons" aria-hidden="true">delete</span>
            </button>
          )}
        </div>
        <span className="theme-card-meta">{theme.meta}</span>
      </div>
      <div className="theme-card-chips">
        {theme.builtin ? (
          <span className="theme-chip official">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3l8 3v6c0 4.5-3.4 8.3-8 9-4.6-.7-8-4.5-8-9V6l8-3z" /><path d="M9 12l2 2 4-4" /></svg>
            Eingebaut
          </span>
        ) : (
          <span className="theme-chip">Installiert</span>
        )}
        <span className="theme-chip">{SCHEME_LABEL[theme.scheme]}</span>
        {theme.hasCode && (
          <span className="theme-chip with-code" title="Ersetzt Teile der Oberfläche mit eigenem Code">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 7l-5 5 5 5" /><path d="M16 7l5 5-5 5" /></svg>
            Mit Code
          </span>
        )}
      </div>
    </li>
  );
}

// Vorschau: Bild aus dem Paket oder eine kleine Oberfläche in den Farben des Themes; „System“ zeigt hell und dunkel geteilt
function ThemePreview({ theme }: { theme: ThemeEntry }) {
  if (theme.image) return <img className="theme-preview" src={theme.image} alt={`Vorschau ${theme.name}`} />;
  return (
    <div className="theme-preview" aria-hidden="true">
      <MiniUi palette={theme.scheme === 'system' ? LIGHT_PALETTE : theme.palette} />
      {theme.scheme === 'system' && <MiniUi palette={DARK_PALETTE} className="split" />}
    </div>
  );
}

function MiniUi({ palette, className }: { palette: ThemePalette; className?: string }) {
  const vars = {
    '--p-bg': palette.bg,
    '--p-side': palette.side,
    '--p-card': palette.card,
    '--p-line': palette.line,
    '--p-accent': palette.accent,
    '--p-text': palette.text,
  } as CSSProperties;
  return (
    <div className={`mini-ui${className ? ` ${className}` : ''}`} style={vars}>
      <div className="mini-side">
        <i style={{ width: '70%', opacity: 0.55 }} />
        <i className="accent" style={{ width: '88%' }} />
        <i style={{ width: '60%', opacity: 0.3 }} />
        <i style={{ width: '76%', opacity: 0.3 }} />
      </div>
      <div className="mini-main">
        <i className="title" />
        <div className="mini-card">
          <i style={{ width: '80%' }} />
          <i style={{ width: '60%' }} />
          <b />
        </div>
      </div>
    </div>
  );
}
