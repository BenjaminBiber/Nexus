import { useId, useRef, useState, type FormEvent } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { DB_COLUMN_LABELS, DB_PRIORITIES, DB_PRIORITY_LABELS, DB_STATUSES, type DbNewTicketInput, type DbPriority, type DbStatus, type DbTicketDto } from '../../api/dbTypes.js';
import { ACCEPTANCE_QUESTION } from './DbAcceptanceDialog.js';
import { DbModal } from './DbModal.js';

// Neues Ticket auf dem DB-Board: Titel, Beschreibung, Status, Priorität, Labels, Planungsbedarf.
// Bei „Erledigt“ ist eine Abnahme Pflicht; mit Eingaben fragen Abbrechen und Esc nach.
export function DbNewTicketDialog({ boardId, onClose, onCreated }: { boardId: number; onClose: () => void; onCreated: (ticket: DbTicketDto) => void }) {
  const api = useApi();
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [status, setStatus] = useState<DbStatus>('backlog');
  const [priority, setPriority] = useState<DbPriority>('normal');
  const [labels, setLabels] = useState('');
  const [needsPlanning, setNeedsPlanning] = useState(true);
  const [acceptance, setAcceptance] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [asking, setAsking] = useState(false);
  const sending = useRef(false);
  const titleId = useId();
  const done = status === 'done';
  const dirty = [title, description, labels, acceptance].some((text) => text.trim() !== '');

  // Legt das Ticket an; ohne Titel (bzw. ohne Abnahme bei „Erledigt“) nur ein Hinweis
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (sending.current) return;
    if (!title.trim()) return setError('Der Titel darf nicht leer sein.');
    if (done && !acceptance.trim()) return setError(`Für „Erledigt“ braucht es eine Abnahme: ${ACCEPTANCE_QUESTION}`);
    const input: DbNewTicketInput = {
      title: title.trim(), description, status, priority,
      labels: labels.split(',').map((l) => l.trim()).filter(Boolean), needsPlanning,
    };
    if (done) input.acceptance = acceptance.trim();
    sending.current = true;
    setBusy(true);
    setError(null);
    try {
      onCreated(await api.createDbTicket(boardId, input));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      sending.current = false;
      setBusy(false);
    }
  };

  // Abbrechen; mit Eingaben erst nachfragen
  const cancel = () => {
    if (busy) return;
    if (dirty) setAsking(true);
    else onClose();
  };

  return (
    <DbModal
      labelledBy={titleId}
      onSubmit={(e) => void submit(e)}
      onEscape={() => {
        if (asking) setAsking(false);
        else cancel();
      }}
    >
      <h2 id={titleId}>Neues Ticket</h2>
      <label className="field">
        <span>Titel</span>
        <input value={title} autoFocus onChange={(e) => setTitle(e.target.value)} />
      </label>
      <label className="field">
        <span>Beschreibung</span>
        <textarea rows={6} value={description} onChange={(e) => setDescription(e.target.value)} />
      </label>
      <div className="db-fields">
        <label className="field">
          <span>Status</span>
          <select value={status} onChange={(e) => setStatus(e.target.value as DbStatus)}>
            {DB_STATUSES.map((s) => (
              <option key={s} value={s}>
                {DB_COLUMN_LABELS[s]}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Priorität</span>
          <select value={priority} onChange={(e) => setPriority(e.target.value as DbPriority)}>
            {DB_PRIORITIES.map((p) => (
              <option key={p} value={p}>
                {DB_PRIORITY_LABELS[p]}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Labels</span>
          <input value={labels} placeholder="kommagetrennt" onChange={(e) => setLabels(e.target.value)} />
        </label>
        <label className="option-row">
          <input type="checkbox" checked={needsPlanning} onChange={(e) => setNeedsPlanning(e.target.checked)} />
          <span>Braucht Planung</span>
        </label>
      </div>
      {done && (
        <label className="field">
          <span>Abnahme</span>
          <textarea rows={3} placeholder={ACCEPTANCE_QUESTION} value={acceptance} onChange={(e) => setAcceptance(e.target.value)} />
        </label>
      )}
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
            Anlegen
          </button>
          <button type="button" className="btn" disabled={busy} onClick={cancel}>
            Abbrechen
          </button>
        </div>
      )}
    </DbModal>
  );
}
