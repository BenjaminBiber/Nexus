import type { ReactNode } from 'react';

type Tone = 'ok' | 'info' | 'warn' | 'error';

// Rollen je Ton: Erfolg wird angesagt, Fehler sofort, Hinweise stehen nur da
const ROLE: Record<Tone, 'status' | 'alert' | undefined> = { ok: 'status', info: undefined, warn: undefined, error: 'alert' };

// Einheitliche Meldung der Einstellungen (ok, Hinweis, Warnung, Fehler); role überschreibt die Rolle des Tons
export function Notice({ tone, role, label, children }: { tone: Tone; role?: 'note'; label?: string; children: ReactNode }) {
  return (
    <div className={`notice ${tone}`} role={role ?? ROLE[tone]} aria-label={label}>
      {children}
    </div>
  );
}
