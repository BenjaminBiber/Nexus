import { useId, useRef } from 'react';
import { DbCommentComposer } from './DbCommentComposer.js';
import { DbModal } from './DbModal.js';

// Dialog „Kommentar hinzufügen“: das bekannte Eingabefeld in einer Dialoghülle. Der Entwurf liegt bei den Reitern,
// deshalb geht beim Schließen (Esc, Abbrechen, Hintergrund) nichts verloren und es gibt keine Rückfrage.
export function DbCommentDialog({ value, onChange, onSend, busy, userName, onClose }: {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  busy: boolean;
  userName?: string;
  onClose: () => void;
}) {
  const titleId = useId();
  const textarea = useRef<HTMLTextAreaElement | null>(null);
  // Während des Sendens bleibt der Dialog stehen
  const close = () => {
    if (!busy) onClose();
  };
  return (
    <DbModal
      labelledBy={titleId}
      onEscape={close}
      onBackdrop={close}
      backdropClassName="db-dialog-backdrop"
      className="panel db-dialog db-comment-dialog"
      initialFocus={textarea}
    >
      <strong id={titleId}>Kommentar hinzufügen</strong>
      <DbCommentComposer
        value={value}
        onChange={onChange}
        onSend={onSend}
        busy={busy}
        userName={userName}
        label="Neuer Kommentar"
        placeholder="Kommentar schreiben …"
        button="Kommentieren"
        textareaRef={textarea}
      />
      <div className="actions">
        <button type="button" className="btn" disabled={busy} onClick={close}>
          Abbrechen
        </button>
      </div>
    </DbModal>
  );
}
