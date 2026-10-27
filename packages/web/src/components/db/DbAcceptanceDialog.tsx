import { useId, useState } from 'react';
import { DbModal } from './DbModal.js';

export const ACCEPTANCE_QUESTION = 'Woran sieht man, dass es fertig ist?';

// Fragt beim Verschieben nach „Erledigt“ die fehlende Abnahme ab; „Abbrechen“ lässt das Ticket unverändert.
export function DbAcceptanceDialog({ ticketId, onConfirm, onCancel }: { ticketId: string; onConfirm: (acceptance: string) => void; onCancel: () => void }) {
  const [text, setText] = useState('');
  const titleId = useId();
  const ready = text.trim().length > 0;
  return (
    <DbModal
      labelledBy={titleId}
      onEscape={onCancel}
      backdropClassName="db-dialog-backdrop"
      className="panel db-dialog"
      onSubmit={(event) => {
        event.preventDefault();
        if (ready) onConfirm(text.trim());
      }}
    >
      <strong id={titleId}>{ACCEPTANCE_QUESTION}</strong>
      <p className="db-dialog-hint">{ticketId} hat noch keine Abnahme. Ohne sie lässt es sich nicht nach „Erledigt“ verschieben.</p>
      <textarea aria-label={ACCEPTANCE_QUESTION} rows={4} autoFocus value={text} onChange={(e) => setText(e.target.value)} />
      <div className="actions">
        <button type="submit" className="btn primary" disabled={!ready}>
          Verschieben
        </button>
        <button type="button" className="btn" onClick={onCancel}>
          Abbrechen
        </button>
      </div>
    </DbModal>
  );
}
