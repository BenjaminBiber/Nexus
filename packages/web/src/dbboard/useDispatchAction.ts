import { useCallback, useEffect, useState } from 'react';
import { useApi } from '../api/ApiContext.js';

// Wie lange das Ergebnis stehen bleibt (wie die Info-Hinweise am Board)
const OUTCOME_MS = 4000;

export type DispatchActionKind = 'accept' | 'merge';

// Abnehmen (Human Interaction, danach der Merge) oder erneut Mergen über die Endpunkte des Dispatchers, mit Laufzustand,
// Fehler und Ergebnis; das Ergebnis blendet sich nach 4 s aus. Genutzt vom Ticket-Panel und von „Meine Aufgaben“.
export function useDispatchAction() {
  const api = useApi();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<string | null>(null);
  // Ergebnis nach Ablauf der Zeit ausblenden; ein neues Ergebnis startet die Zeit neu
  useEffect(() => {
    if (outcome === null) return undefined;
    const timer = setTimeout(() => setOutcome(null), OUTCOME_MS);
    return () => clearTimeout(timer);
  }, [outcome]);

  // Stößt die Aktion an; onDone läuft nach Erfolg (z. B. neu laden)
  const run = useCallback(async (kind: DispatchActionKind, ticketId: string, boardId: number | undefined, onDone: () => void) => {
    setBusy(true);
    setError(null);
    setOutcome(null);
    try {
      const answer = kind === 'accept' ? await api.acceptDispatchTicket(ticketId, boardId) : await api.retryDispatchMerge(ticketId, boardId);
      setOutcome(answer.result?.text ?? null);
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }, [api]);

  return { busy, error, outcome, hideOutcome: () => setOutcome(null), run };
}
