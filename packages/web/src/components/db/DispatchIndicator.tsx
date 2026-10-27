import type { DispatchStatusDto } from '../../api/dispatchTypes.js';

// Laufen nur Recherchen (kein Ticket-Lauf) und ist der Dispatcher weder pausiert noch fehlerhaft? Dann zeigt der Knopf
// „Recherche läuft“ statt „aus“, „bereit“ oder „außerhalb“
function researchOnly(status: DispatchStatusDto | null): number {
  if (!status || status.active > 0 || status.state === 'paused' || status.state === 'error') return 0;
  return status.research?.active ?? 0;
}

// Knopf in der Board-Kopfzeile: „Dispatcher: läuft · 1/1“ usw.; öffnet die Übersicht.
export function DispatchIndicator({ status, error, onOpen }: { status: DispatchStatusDto | null; error: string | null; onOpen: () => void }) {
  const researching = error ? 0 : researchOnly(status);
  const text = error ? 'Fehler' : researching ? `Recherche läuft${researching > 1 ? ` (${researching})` : ''}` : status?.message ?? '…';
  const state = error ? 'error' : researching ? 'running' : status?.state ?? 'off';
  return (
    <button type="button" className={`btn dispatch-indicator state-${state}`} onClick={onOpen} title={error ?? undefined} aria-label={error ? `Dispatcher: Fehler – ${error}` : undefined}>
      Dispatcher: {text}
    </button>
  );
}
