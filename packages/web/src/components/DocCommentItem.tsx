import { useState } from 'react';
import type { DocCommentDto } from '../api/types.js';
import { CommentIcon } from './CommentIcon.js';

interface Props {
  comment: DocCommentDto;
  notFound: boolean;
  onJump: () => void;
  onUpdate: (patch: { text?: string; status?: 'open' | 'resolved' }) => Promise<boolean>;
  onRemove: () => void;
}

// Ein Kommentar in der Seitenleiste: Zitat, Text, Antwort und Aktionen
export function DocCommentItem({ comment, notFound, onJump, onUpdate, onRemove }: Props) {
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(comment.text);
  const [saving, setSaving] = useState(false);
  const resolved = comment.status === 'resolved';

  // Öffnet das Bearbeiten mit dem aktuellen Text des Kommentars
  const startEdit = () => {
    setText(comment.text);
    setEditing(true);
  };

  // Speichert den getrimmten Text und schließt das Bearbeiten nur bei Erfolg
  const save = async () => {
    if (saving) return;
    setSaving(true);
    try {
      if (await onUpdate({ text: text.trim() })) setEditing(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <li className={`doc-comment-item${resolved ? ' resolved' : ''}`}>
      <button type="button" className="doc-comment-quote" onClick={onJump}>
        „{comment.quote}“
      </button>
      {notFound && <p className="muted">Stelle nicht mehr gefunden</p>}
      {editing ? (
        <>
          <textarea aria-label="Kommentar bearbeiten" value={text} onChange={(e) => setText(e.target.value)} />
          <div className="doc-comment-actions">
            <button type="button" className="btn small" onClick={() => setEditing(false)}>Abbrechen</button>
            <button type="button" className="btn small primary" disabled={saving || text.trim() === ''} onClick={save}>Speichern</button>
          </div>
        </>
      ) : (
        <p className="doc-comment-text">{comment.text}</p>
      )}
      {comment.resolution && <p className="doc-comment-resolution">Antwort: {comment.resolution}</p>}
      {comment.heading && <p className="muted">Abschnitt: {comment.heading}</p>}
      {!editing && (
        <div className="doc-comment-actions">
          <button type="button" className="doc-comment-action" onClick={() => onUpdate({ status: resolved ? 'open' : 'resolved' })}>
            <CommentIcon name={resolved ? 'undo' : 'check'} />
            {resolved ? 'Wieder öffnen' : 'Erledigt'}
          </button>
          <button type="button" className="doc-comment-action icon push" aria-label="Bearbeiten" title="Bearbeiten" onClick={startEdit}>
            <CommentIcon name="edit" />
          </button>
          <button type="button" className="doc-comment-action icon danger" aria-label="Löschen" title="Löschen" onClick={onRemove}>
            <CommentIcon name="trash" />
          </button>
        </div>
      )}
    </li>
  );
}
