import { useId, type ReactNode } from 'react';
import { Notice } from './Notice.js';

// Bausteine der Einstellungsseiten im Kartenstil (Telegram, Spracherkennung): Abschnitt, Stand-Etikett
// und die Leiste mit ungespeicherten Änderungen. Die Gestaltung steht unter „tg-*“ in settings.css.

export type Tone = 'ok' | 'info' | 'warn' | 'error';

// Abschnitt mit Überschrift und einem Satz darunter
export function CardSection({ title, description, children }: { title: string; description: string; children: ReactNode }) {
  const id = useId();
  return (
    <section className="tg-section" aria-labelledby={id}>
      <div className="tg-section-head">
        <h3 id={id}>{title}</h3>
        <p>{description}</p>
      </div>
      {children}
    </section>
  );
}

// Kurzes Etikett mit Punkt, z. B. „Läuft“ oder „Fehler“
export function StatusBadge({ tone, children }: { tone: Tone; children: ReactNode }) {
  return <span className={`tg-badge ${tone}`}><span aria-hidden="true" />{children}</span>;
}

// Leiste unten: Fehler, Erfolg oder die Zahl ungespeicherter Änderungen, dazu Verwerfen und Speichern
export function SaveBar({ changes, error, info, onDiscard, onSave }: {
  changes: number;
  error: string | null;
  info: string | null;
  onDiscard: () => void;
  onSave: () => void;
}) {
  return (
    <div className="tg-savebar">
      <span className="tg-savebar-state">
        {error ? (
          <Notice tone="error">{error}</Notice>
        ) : info ? (
          <Notice tone="ok">{info}</Notice>
        ) : changes > 0 ? (
          <><span className="tg-dot" aria-hidden="true" />{changes === 1 ? '1 ungespeicherte Änderung' : `${changes} ungespeicherte Änderungen`}</>
        ) : (
          'Alles gespeichert.'
        )}
      </span>
      <button type="button" className="btn" disabled={changes === 0} onClick={onDiscard}>Verwerfen</button>
      <button type="button" className="btn primary" disabled={changes === 0} onClick={onSave}>Speichern</button>
    </div>
  );
}

// Abstand zu jetzt in Worten: „gerade eben“, „vor 5 min“, „vor 2 h 10 min“, „vor 3 Tagen“
export function sinceText(iso: string, now: number): string {
  const minutes = Math.max(0, Math.floor((now - new Date(iso).getTime()) / 60_000));
  if (minutes < 1) return 'gerade eben';
  if (minutes < 60) return `vor ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return minutes % 60 === 0 ? `vor ${hours} h` : `vor ${hours} h ${minutes % 60} min`;
  const days = Math.floor(hours / 24);
  return days === 1 ? 'vor 1 Tag' : `vor ${days} Tagen`;
}

// Ein/Aus-Knopf im Kopf, wie bei den Accounts, nur mit Zeichen: an = hinterlegt, aus = Akzentknopf zum Einschalten
export function PowerToggle({ label, on, onChange }: { label: string; on: boolean; onChange: (next: boolean) => void }) {
  return (
    <button
      type="button"
      className={`icon-btn card-power${on ? ' on' : ''}`}
      aria-label={label}
      aria-pressed={on}
      title={on ? `${label}: an – zum Ausschalten klicken` : `${label}: aus – zum Einschalten klicken`}
      onClick={() => onChange(!on)}
    >
      <span className="material-icons" aria-hidden="true">power_settings_new</span>
    </button>
  );
}
