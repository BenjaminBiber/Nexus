import { useEffect, useRef, useState } from 'react';
import { ClaudeIcon } from './NavIcons.js';

interface Props {
  quote: string;
  top: number;
  left: number;
  composing: boolean;
  error: string | null;
  /** nur Superpowers-Dokumente lassen sich bearbeiten; sonst fehlt der Knopf */
  editable: boolean;
  onEdit: () => void;
  /** öffnet den Dialog „Mit Claude ändern“ für die Markierung */
  onRevise: () => void;
  onCompose: () => void;
  onSave: (text: string) => Promise<unknown>;
  onClose: () => void;
}

// Knopf an der Auswahl; nach Klick das Formular mit Zitat und Textfeld
export function SelectionPopover({ quote, top, left, composing, error, editable, onEdit, onRevise, onCompose, onSave, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [text, setText] = useState('');
  const [saving, setSaving] = useState(false);

  // Schließt bei Klick daneben; getippter Text geht dabei nie verloren
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current?.contains(e.target as Node)) return;
      if (!composing || text.trim() === '') onClose();
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [composing, text, onClose]);

  // Speichert den Entwurf und sperrt den Knopf, bis die Antwort da ist
  const save = async () => {
    if (saving) return;
    setSaving(true);
    try {
      await onSave(text.trim());
    } finally {
      setSaving(false);
    }
  };

  // Escape verwirft, Strg+Enter speichert (nur mit Text)
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && text.trim() !== '') void save();
  };

  if (!composing) {
    return (
      <div ref={ref} className="selection-popover" style={{ top, left }}>
        <div className="selection-popover-actions" role="group" aria-label="Aktionen zur Auswahl">
          {/* Symbolknöpfe; der Name steht als Tooltip und für Screenreader am Knopf */}
          {editable && (
            <button type="button" className="selection-popover-icon" aria-label="Bearbeiten" title="Bearbeiten" onClick={onEdit}>
              <span className="material-icons" aria-hidden="true">edit</span>
            </button>
          )}
          {editable && (
            <button type="button" className="selection-popover-icon" aria-label="Mit Claude ändern" title="Mit Claude ändern" onClick={onRevise}>
              <ClaudeIcon />
            </button>
          )}
          <button type="button" className="selection-popover-trigger" aria-label="Kommentieren" title="Kommentieren" onClick={onCompose}>
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M21 15a2 2 0 0 1-2 2H8l-5 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
          </button>
        </div>
        {/* Lässt sich die Markierung keiner Zeile zuordnen, sagt das der Hinweis statt eines stummen Knopfs */}
        {error && <p role="alert" className="error selection-popover-error">{error}</p>}
      </div>
    );
  }
  return (
    <div ref={ref} className="selection-popover selection-popover-form" role="dialog" aria-label="Kommentar zur Stelle" style={{ top, left }} onKeyDown={onKeyDown}>
      <blockquote className="doc-comment-draft-quote">{quote}</blockquote>
      <textarea aria-label="Kommentar" autoFocus value={text} onChange={(e) => setText(e.target.value)} />
      {error && <p role="alert" className="error">{error}</p>}
      <div className="doc-comment-actions">
        <span className="selection-popover-hint">Strg+Enter speichert</span>
        <button type="button" className="btn small" onClick={onClose}>Abbrechen</button>
        <button type="button" className="btn small primary" disabled={saving || text.trim() === ''} onClick={save}>Speichern</button>
      </div>
    </div>
  );
}
