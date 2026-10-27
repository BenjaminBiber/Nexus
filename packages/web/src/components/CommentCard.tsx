import { useEffect, useRef, useState } from 'react';
import type { DocCommentDto } from '../api/types.js';
import { CommentIcon } from './CommentIcon.js';

interface Props {
  comment: DocCommentDto;
  top: number;
  left: number;
  editing: boolean;
  onEdit: () => void;
  onSave: (text: string) => Promise<unknown>;
  onClose: () => void;
  onEnter: () => void;
  onLeave: () => void;
}

// Karte an der Hervorhebung: zeigt den Kommentar und lässt ihn direkt bearbeiten
export function CommentCard({ comment, top, left, editing, onEdit, onSave, onClose, onEnter, onLeave }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [text, setText] = useState(comment.text);
  const [saving, setSaving] = useState(false);

  // Beginnt beim Bearbeiten mit dem aktuellen Text des Kommentars
  useEffect(() => {
    if (editing) setText(comment.text);
  }, [editing, comment.text]);

  // Schließt eine festgehaltene Karte bei Klick außerhalb von Karte und Hervorhebung
  useEffect(() => {
    if (!editing) return;
    const onDown = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!ref.current?.contains(target) && !target.closest?.('mark.doc-comment')) onClose();
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [editing, onClose]);

  // Speichert den getrimmten Text und sperrt den Knopf, bis die Antwort da ist
  const save = async () => {
    if (saving) return;
    setSaving(true);
    try {
      await onSave(text.trim());
    } finally {
      setSaving(false);
    }
  };

  // Escape schließt, Strg+Enter speichert (nur mit Text)
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && text.trim() !== '') void save();
  };

  return (
    <div
      ref={ref}
      className={`comment-hover${editing ? ' editing' : ''}`}
      role="dialog"
      aria-label="Kommentar"
      style={{ top, left }}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      onKeyDown={onKeyDown}
    >
      {editing ? (
        <>
          <textarea aria-label="Kommentar bearbeiten" autoFocus value={text} onChange={(e) => setText(e.target.value)} />
          <div className="doc-comment-actions">
            <span className="selection-popover-hint">Strg+Enter speichert</span>
            <button type="button" className="btn small" onClick={onClose}>Abbrechen</button>
            <button type="button" className="btn small primary" disabled={saving || text.trim() === ''} onClick={save}>Speichern</button>
          </div>
        </>
      ) : (
        <div className="comment-hover-row">
          <div>
            <p className="comment-hover-text">{comment.text}</p>
            {comment.resolution && <p className="comment-hover-meta">Antwort: {comment.resolution}</p>}
            {comment.heading && <p className="comment-hover-meta">Abschnitt: {comment.heading}</p>}
          </div>
          <button type="button" className="doc-comment-action icon" aria-label="Kommentar bearbeiten" title="Bearbeiten" onClick={onEdit}>
            <CommentIcon name="edit" />
          </button>
        </div>
      )}
    </div>
  );
}
