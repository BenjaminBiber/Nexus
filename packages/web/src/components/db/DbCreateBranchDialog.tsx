import { useId, useState } from 'react';
import { DbModal } from './DbModal.js';

// Rückfrage vor dem Anlegen eines neuen Zielbranches im Board-Repo; bei einem Fehler bleibt der Dialog mit der Meldung offen.
export function DbCreateBranchDialog({ name, onConfirm, onCancel }: { name: string; onConfirm: () => Promise<string | null>; onCancel: () => void }) {
  const titleId = useId();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <DbModal
      role="alertdialog"
      labelledBy={titleId}
      onEscape={onCancel}
      backdropClassName="db-dialog-backdrop"
      className="panel db-dialog"
      onSubmit={(event) => {
        event.preventDefault();
        if (busy) return;
        setBusy(true);
        setError(null);
        void onConfirm().then((message) => {
          setBusy(false);
          setError(message);
        });
      }}
    >
      <strong id={titleId}>Wollen Sie Branch {name} wirklich erstellen?</strong>
      <p className="db-dialog-hint">Der Branch wird lokal im Repo des Boards angelegt (von origin/{name}, sonst vom ausgecheckten Stand) und als Zielbranch gespeichert.</p>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
      <div className="actions">
        <button type="submit" className="btn primary" disabled={busy} autoFocus>
          Erstellen
        </button>
        <button type="button" className="btn" onClick={onCancel}>
          Abbrechen
        </button>
      </div>
    </DbModal>
  );
}
