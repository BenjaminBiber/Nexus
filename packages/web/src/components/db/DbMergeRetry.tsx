import type { DbTicketDetailDto } from '../../api/dbTypes.js';
import { useDispatchAction } from '../../dbboard/useDispatchAction.js';
import { showsReview } from './DbReview.js';

// Hinweis und Knopf zum Merge des Dispatchers: „Erneut mergen“ bei Blockiert mit Grund „Merge: …“ (Merge gescheitert),
// „Mergen“ bei Erledigt mit noch gesetztem Branch (z. B. von Hand abgeschlossen). Diese Knöpfe gibt es nur auf eigenen
// Boards (`mine`). „Abnehmen“ bei Blockiert mit Grund „Human Interaction: …“ gibt es immer: Erledigt, auf eigenem Board
// danach der Merge; auf eigenen Boards mit Branch übernimmt das der Review-Block (DbReview). Das Ergebnis bleibt auch nach dem Neuladen sichtbar, bis es ausgeblendet wird (Knopf oder nach 4 s).
export function DbMergeRetry({ ticket, mine, onDone }: { ticket: DbTicketDetailDto; mine: boolean; onDone: () => void }) {
  const { busy, error, outcome, hideOutcome, run } = useDispatchAction();
  const failed = ticket.status === 'blocked' && (ticket.blockedReason ?? '').startsWith('Merge:');
  const unmerged = ticket.status === 'done' && Boolean(ticket.branch);
  const reason = (ticket.blockedReason ?? '').trim();
  const human = ticket.status === 'blocked' && (reason.startsWith('Human Interaction:') || reason === 'Human Interaction') && !showsReview(ticket, mine);

  // Ergebniszeile mit Knopf zum Ausblenden
  const result = outcome && (
    <p role="status">
      {outcome}{' '}
      <button type="button" className="btn" onClick={hideOutcome}>Ausblenden</button>
    </p>
  );
  if (!failed && !unmerged && !human) return result ? <div className="db-merge-retry">{result}</div> : null;

  // Stößt den Merge (bei Human Interaction die Abnahme samt Merge) an; danach lädt das Panel neu
  const retry = () => run(human ? 'accept' : 'merge', ticket.id, ticket.boardId, onDone);

  // Grund des gescheiterten Merges ohne „Merge:“, immer mit Satzzeichen am Ende
  const cause = failed ? reason.slice('Merge:'.length).trim().replace(/[^.!?]$/, '$&.') : '';

  if (human) {
    const what = reason.slice('Human Interaction'.length).replace(/^:/, '').trim();
    return (
      <div className="db-merge-retry">
        <p>
          {`Wartet auf deine Abnahme${what ? `: ${what}` : ''}.`}
          {mine ? ' Abnehmen setzt Erledigt und mergt den Branch.' : ' Abnehmen setzt Erledigt; gemergt wird auf dem Board des Besitzers.'}
        </p>
        {error && <div className="banner" role="alert">{error}</div>}
        {result}
        <button type="button" className="btn primary" disabled={busy} onClick={() => void retry()}>Abnehmen</button>
      </div>
    );
  }

  return (
    <div className="db-merge-retry">
      <p>
        {failed
          ? `Der Dispatcher konnte nicht mergen${cause ? `: ${cause}` : '.'} Ursache beheben, dann erneut versuchen.`
          : `Der Branch ${ticket.branch} ist noch nicht gemergt.`}
        {!mine && ' Mergen kann nur der Besitzer des Boards.'}
      </p>
      {error && <div className="banner" role="alert">{error}</div>}
      {result}
      {mine && (
        <button type="button" className="btn primary" disabled={busy} onClick={() => void retry()}>{failed ? 'Erneut mergen' : 'Mergen'}</button>
      )}
    </div>
  );
}
