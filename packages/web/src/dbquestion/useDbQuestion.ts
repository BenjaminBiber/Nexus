import { useCallback, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { DbQuestionDetailDto } from '../api/dbTypes.js';
import { useAsync } from '../hooks/useAsync.js';
import { useDbProjectWatch, useLiveHandler } from '../live/useLive.js';

// Eine Änderung mit Version und dem Stand, auf den sie angewendet wird (beim erneuten Ausführen nach einem Konflikt der aktuelle)
type Action = (rev: number, base: DbQuestionDetailDto) => Promise<DbQuestionDetailDto>;

// Änderung einer Liste (Tickets oder Abhängigkeiten), unabhängig vom Stand, auf den sie angewendet wird
export type DbListChange = (current: string[]) => string[];

export interface DbQuestionConflict {
  current: DbQuestionDetailDto;
  retry: Action;
  scope: 'decision' | 'relations';
}

// Lädt eine Frage der Board-Datenbank und führt Änderungen mit Versionsprüfung aus; Konflikte werden zur Entscheidung angeboten.
// Live: beobachtet das Board-Projekt der Frage und lädt bei „dbquestions“ ohne Ladeanzeige nach.
export function useDbQuestion(id: string | undefined) {
  const api = useApi();
  const [unavailable, setUnavailable] = useState<string | null>(null);
  const loaded = useAsync(async () => {
    if (!id) return new Promise<never>(() => undefined);
    try {
      const data = await api.getDbQuestion(id);
      setUnavailable(null);
      return data;
    } catch (error) {
      if (error instanceof ApiError && error.status === 503) setUnavailable(error.message);
      throw error;
    }
  }, [api, id]);
  const question = loaded.state.status === 'ready' ? loaded.state.data : null;
  useDbProjectWatch(question?.projectId ?? null);
  useLiveHandler({
    onDbQuestions: (projectId) => {
      if (question && projectId === question.projectId) void loaded.refresh();
    },
    onReset: () => void loaded.refresh(),
  });
  const [conflict, setConflict] = useState<DbQuestionConflict | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [commentError, setCommentError] = useState<string | null>(null);
  const [relationError, setRelationError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Führt eine Änderung aus; liefert, ob sie geklappt hat. Ein Konflikt merkt sich die Änderung für „Meine Fassung behalten“
  const run = useCallback(
    async (action: Action, rev?: number, scope: DbQuestionConflict['scope'] = 'decision', base?: DbQuestionDetailDto): Promise<boolean> => {
      if (!question) return false;
      setBusy(true);
      setError(null);
      setRelationError(null);
      try {
        loaded.set(await action(rev ?? question.rev, base ?? question));
        setConflict(null);
        return true;
      } catch (failure) {
        if (failure instanceof ApiError && failure.code === 'conflict' && failure.current) {
          setConflict({ current: failure.current as DbQuestionDetailDto, retry: action, scope });
        } else if (scope === 'relations') {
          setRelationError((failure as Error).message);
        } else {
          setError((failure as Error).message);
        }
        return false;
      } finally {
        setBusy(false);
      }
    },
    [question, loaded],
  );

  // Entscheidet mit Option oder Freitext und Begründung
  const decide = useCallback(
    (input: { option?: string; text?: string; reason: string }) => run((rev) => api.decideDbQuestion(id!, { rev, ...input })),
    [api, id, run],
  );

  // Verwirft mit Begründung
  const discard = useCallback((reason: string) => run((rev) => api.discardDbQuestion(id!, rev, reason)), [api, id, run]);

  // Öffnet wieder, auf Wunsch mit Grund
  const reopen = useCallback((reason?: string) => run((rev) => api.reopenDbQuestion(id!, rev, reason)), [api, id, run]);

  // Ändert die betroffenen Tickets (auf den Stand angewendet, mit dem gespeichert wird)
  const setTickets = useCallback(
    (change: DbListChange) => run((rev, base) => api.putDbQuestionTickets(id!, rev, change(base.ticketIds)), undefined, 'relations'),
    [api, id, run],
  );

  // Ändert die Abhängigkeiten (wie setTickets)
  const setDepends = useCallback(
    (change: DbListChange) => run((rev, base) => api.putDbQuestionDepends(id!, rev, change(base.dependsOn)), undefined, 'relations'),
    [api, id, run],
  );

  // Hängt einen Kommentar an (ohne Versionsprüfung); liefert, ob es geklappt hat
  const comment = useCallback(
    async (body: string): Promise<boolean> => {
      if (!id) return false;
      setBusy(true);
      setCommentError(null);
      try {
        loaded.set(await api.addDbQuestionComment(id, body));
        return true;
      } catch (failure) {
        setCommentError((failure as Error).message);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [api, id, loaded],
  );

  // Im Konflikt: aktuellen Stand übernehmen oder die eigene Änderung auf den aktuellen Stand anwenden
  const resolveConflict = useCallback(
    async (choice: 'reload' | 'keep') => {
      if (!conflict) return;
      if (choice === 'reload') {
        loaded.set(conflict.current);
        setConflict(null);
      } else {
        await run(conflict.retry, conflict.current.rev, conflict.scope, conflict.current);
      }
    },
    [conflict, loaded, run],
  );

  return {
    state: loaded.state, retry: loaded.retry, refresh: loaded.refresh, unavailable, decide, discard, reopen, comment, setTickets, setDepends,
    conflict, resolveConflict, error, commentError, relationError, busy,
  };
}
