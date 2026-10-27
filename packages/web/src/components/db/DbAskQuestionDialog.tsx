import { useId, useRef, useState, type FormEvent } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { DbQuestionDetailDto } from '../../api/dbTypes.js';
import { DbModal } from './DbModal.js';

// Kommagetrennte Eingabe als Liste ohne Leereinträge
function listOf(value: string): string[] {
  return value.split(',').map((s) => s.trim()).filter(Boolean);
}

// Dialog „Frage stellen“: im Board-Projekt, auf Wunsch vorbelegt mit einem Ticket, das dann auf die Frage wartet.
// Mit eigenen Eingaben fragen Abbrechen und Esc nach; während des Sendens sind die Knöpfe gesperrt.
export function DbAskQuestionDialog({ projectKey, ticketId, onClose, onCreated }: {
  projectKey: string;
  ticketId?: string;
  onClose: () => void;
  onCreated: (question: DbQuestionDetailDto) => void;
}) {
  const api = useApi();
  const titleId = useId();
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const initialTickets = ticketId ?? '';
  const initialOccasion = ticketId ? `Ticket ${ticketId}` : '';
  const [tickets, setTickets] = useState(initialTickets);
  const [occasion, setOccasion] = useState(initialOccasion);
  const [tags, setTags] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [asking, setAsking] = useState(false);
  const sending = useRef(false);
  const dirty = [title, description, tags].some((text) => text.trim() !== '') || tickets !== initialTickets || occasion !== initialOccasion;

  // Prüft den Titel und stellt die Frage; Fehler des Servers bleiben im Dialog sichtbar
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (sending.current) return;
    if (!title.trim()) {
      setError('Bitte einen Titel angeben.');
      return;
    }
    sending.current = true;
    setBusy(true);
    setError(null);
    try {
      const created = await api.createDbQuestion(projectKey, {
        title: title.trim(),
        ...(description.trim() ? { description } : {}),
        occasion: occasion.trim() || null,
        tags: listOf(tags),
        ticketIds: listOf(tickets),
      });
      onCreated(created);
    } catch (failure) {
      setError((failure as Error).message);
    } finally {
      sending.current = false;
      setBusy(false);
    }
  };

  // Abbrechen; mit eigenen Eingaben erst nachfragen
  const cancel = () => {
    if (busy) return;
    if (dirty) setAsking(true);
    else onClose();
  };

  return (
    <DbModal
      labelledBy={titleId}
      onEscape={() => {
        if (asking) setAsking(false);
        else cancel();
      }}
      backdropClassName="db-dialog-backdrop"
      className="panel db-dialog db-ask-question"
      onSubmit={(event) => void submit(event)}
    >
      <strong id={titleId}>{ticketId ? `Frage zu ${ticketId} stellen` : `Neue Frage in ${projectKey}`}</strong>
      <p className="db-dialog-hint">Betroffene Tickets warten, bis die Frage entschieden ist. Entscheiden kannst nur du.</p>
      <label className="field">
        <span>Titel</span>
        <input autoFocus value={title} onChange={(e) => setTitle(e.target.value)} />
      </label>
      <label className="field">
        <span>Beschreibung</span>
        <textarea className="plain-text" rows={4} value={description} onChange={(e) => setDescription(e.target.value)} />
      </label>
      <label className="field">
        <span>Betroffene Tickets</span>
        <input value={tickets} placeholder="kommagetrennt, z. B. ARC-0007" onChange={(e) => setTickets(e.target.value)} />
      </label>
      <label className="field">
        <span>Anlass</span>
        <input value={occasion} onChange={(e) => setOccasion(e.target.value)} />
      </label>
      <label className="field">
        <span>Tags</span>
        <input value={tags} placeholder="kommagetrennt" onChange={(e) => setTags(e.target.value)} />
      </label>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
      {asking ? (
        <div role="alertdialog" aria-label="Eingaben verwerfen" className="actions">
          <span>Eingaben verwerfen?</span>
          <button type="button" className="btn danger" onClick={onClose}>
            Verwerfen
          </button>
          <button type="button" className="btn" autoFocus onClick={() => setAsking(false)}>
            Weiter bearbeiten
          </button>
        </div>
      ) : (
        <div className="actions">
          <button type="submit" className="btn primary" disabled={busy}>
            Frage stellen
          </button>
          <button type="button" className="btn" disabled={busy} onClick={cancel}>
            Abbrechen
          </button>
        </div>
      )}
    </DbModal>
  );
}
