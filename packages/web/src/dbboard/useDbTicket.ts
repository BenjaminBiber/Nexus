import { useCallback, useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { DbStepChange, DbStepInput, DbTicketDetailDto, DbTicketPatch } from '../api/dbTypes.js';
import { useAsync } from '../hooks/useAsync.js';

export type DbChange = Omit<DbTicketPatch, 'rev'>;

// Änderung oder Vorschrift, die sie erst beim Senden aus dem dann zuletzt bekannten Stand berechnet
export type DbChangeInput = DbChange | ((base: DbTicketDetailDto) => DbChange);

// Meldung, wenn sich die Schritte zwischen Anzeige und Aktion geändert haben (409 bei einer Schritt-Aktion)
export const STEPS_CHANGED_MESSAGE = 'Die Schritte wurden inzwischen geändert – bitte erneut versuchen.';

export interface DbConflict {
  current: DbTicketDetailDto;
  change: DbChange;
}

// Ein DB-Ticket laden, ändern, kommentieren und löschen; eine veraltete Version führt zum Konflikt mit dem aktuellen Stand.
// Alle Schreibvorgänge laufen nacheinander über eine Warteschlange und nutzen jeweils die zuletzt bekannte Version,
// damit kurz nacheinander ausgelöste eigene Änderungen nicht miteinander in Konflikt geraten.
// Solange ein Konflikt offen ist, gehen keine weiteren Änderungen oder Kommentare hinaus (sie liefern false).
export function useDbTicket(id: string, onChanged: () => void) {
  const api = useApi();
  const loaded = useAsync(() => api.getDbTicket(id), [api, id]);
  const [conflict, setConflict] = useState<DbConflict | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(0);
  const ticket = loaded.state.status === 'ready' ? loaded.state.data : null;
  const { set, refresh } = loaded;
  const latest = useRef<DbTicketDetailDto | null>(ticket);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const conflicted = useRef(false);
  const loadError = useRef<string | null>(null);

  // Geladene oder übernommene Stände merken (Laden, Live-Nachladen, Konflikt übernehmen)
  useEffect(() => {
    latest.current = ticket;
  }, [ticket]);

  // Übernimmt einen Stand vom Server sofort als zuletzt bekannte Version
  const apply = useCallback((data: DbTicketDetailDto) => {
    latest.current = data;
    set(data);
  }, [set]);

  // Hängt einen Schreibvorgang an die Warteschlange; er startet erst, wenn der vorige abgeschlossen ist
  const enqueue = useCallback(<T,>(work: () => Promise<T>): Promise<T> => {
    setPending((n) => n + 1);
    const run = queue.current.then(work).finally(() => setPending((n) => n - 1));
    queue.current = run.catch(() => undefined);
    return run;
  }, []);

  // Schickt eine Änderung mit der zuletzt bekannten (oder angegebenen) Version; true bei Erfolg
  const send = useCallback(async (input: DbChangeInput, rev?: number): Promise<boolean> => {
    const base = latest.current;
    if (!base || conflicted.current) return false;
    const change = typeof input === 'function' ? input(base) : input;
    setError(null);
    try {
      apply(await api.patchDbTicket(id, { ...change, rev: rev ?? base.rev }));
      setConflict(null);
      onChanged();
      return true;
    } catch (e) {
      if (e instanceof ApiError && e.code === 'conflict' && e.current) {
        conflicted.current = true;
        setConflict({ current: e.current as DbTicketDetailDto, change });
      } else setError((e as Error).message);
      return false;
    }
  }, [api, id, apply, onChanged]);

  // Speichert eine Änderung über die Warteschlange; true bei Erfolg
  const save = useCallback((change: DbChangeInput, rev?: number): Promise<boolean> => enqueue(() => send(change, rev)), [enqueue, send]);

  // Konflikt lösen: aktuellen Stand übernehmen oder die eigene Änderung auf den aktuellen Stand schreiben
  const resolveConflict = useCallback(async (choice: 'reload' | 'overwrite'): Promise<boolean> => {
    if (!conflict) return false;
    apply(conflict.current);
    conflicted.current = false;
    if (choice === 'reload') {
      setConflict(null);
      return false;
    }
    return save(conflict.change, conflict.current.rev);
  }, [conflict, apply, save]);

  // Hängt einen Kommentar an (ebenfalls über die Warteschlange, da er die Version erhöht); true bei Erfolg
  const comment = useCallback((body: string): Promise<boolean> => enqueue(async () => {
    if (conflicted.current) return false;
    setError(null);
    try {
      apply(await api.addDbComment(id, body));
      onChanged();
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    }
  }), [api, id, apply, enqueue, onChanged]);

  // Schritt-Änderung über die Warteschlange (sie erhöht die Version) und Übernahme des gelieferten Stands; true bei Erfolg.
  // Ein 409 (Schritte inzwischen geändert) öffnet keinen Konflikt-Dialog: der aktuelle Stand wird übernommen und gemeldet.
  const stepWork = useCallback((work: () => Promise<DbTicketDetailDto>): Promise<boolean> => enqueue(async () => {
    if (conflicted.current) return false;
    setError(null);
    try {
      apply(await work());
      onChanged();
      return true;
    } catch (e) {
      if (e instanceof ApiError && e.code === 'conflict' && e.current) {
        apply(e.current as DbTicketDetailDto);
        setError(STEPS_CHANGED_MESSAGE);
      } else setError((e as Error).message);
      return false;
    }
  }), [apply, enqueue, onChanged]);

  // Die Schritt-Aktionen schicken den Text mit, den der Schritt an dieser Nummer zuletzt hatte (expectText)
  // Hakt einen Schritt ab oder nimmt den Haken zurück; true bei Erfolg
  const step = useCallback((number: number, done: boolean, expectText?: string) =>
    stepWork(() => api.setDbStep(id, number, done, expectText)), [api, id, stepWork]);
  // Legt einen Schritt an; true bei Erfolg
  const addStep = useCallback((input: DbStepInput) => stepWork(() => api.addDbStep(id, input)), [api, id, stepWork]);
  // Ändert Text und/oder Anleitung eines Schritts; true bei Erfolg
  const editStep = useCallback((number: number, change: DbStepChange, expectText?: string) =>
    stepWork(() => api.editDbStep(id, number, change, expectText)), [api, id, stepWork]);
  // Löscht einen Schritt; true bei Erfolg
  const removeStep = useCallback((number: number, expectText?: string) =>
    stepWork(() => api.removeDbStep(id, number, expectText)), [api, id, stepWork]);
  // Verschiebt einen Schritt; true bei Erfolg
  const moveStep = useCallback((number: number, to: number, expectText?: string) =>
    stepWork(() => api.moveDbStep(id, number, to, expectText)), [api, id, stepWork]);

  // Löscht das Ticket endgültig, erst nach den noch laufenden Schreibvorgängen; true bei Erfolg
  const remove = useCallback((): Promise<boolean> => enqueue(async () => {
    setError(null);
    try {
      await api.deleteDbTicket(id);
      onChanged();
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    }
  }), [api, id, enqueue, onChanged]);

  // Lädt den aktuellen Stand nach (Live-Meldung); bei einem Fehler bleibt die Anzeige und die Meldung erscheint
  const reload = useCallback(async () => {
    const message = await refresh();
    if (message) {
      loadError.current = `Aktueller Stand konnte nicht geladen werden: ${message}`;
      setError(loadError.current);
    } else if (loadError.current) {
      // Nur die eigene Meldung zurücknehmen, nicht den Fehler eines Schreibvorgangs
      const shown = loadError.current;
      loadError.current = null;
      setError((e) => (e === shown ? null : e));
    }
  }, [refresh]);

  // Zuletzt bekannter Stand (nach einem Schreibvorgang sofort der vom Server gelieferte)
  const known = useCallback(() => latest.current, []);

  return { state: loaded.state, known, reload, retry: loaded.retry, ticket, save, comment, step, addStep, editStep, removeStep, moveStep, remove, conflict, resolveConflict, error, busy: pending > 0 };
}
