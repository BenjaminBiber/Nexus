import { useRef, useState } from 'react';

interface Props {
  title: string;
  startLine: number;
  endLine: number;
  value: string;
  saving: boolean;
  conflict: boolean;
  error: string | null;
  onSave: (text: string) => void;
  onReload: () => void;
  onClose: () => void;
}

// Editor für das rohe Markdown eines Zeilenbereichs; bei einem Konflikt bleibt der getippte Text stehen
export function SectionEditor({ title, startLine, endLine, value, saving, conflict, error, onSave, onReload, onClose }: Props) {
  const [text, setText] = useState(value);
  const boxRef = useRef<HTMLTextAreaElement>(null);

  // Escape schließt, Strg+Enter speichert
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !saving && !conflict) onSave(text);
  };

  // Legt den eigenen Text in die Zwischenablage; ohne Clipboard-API bleibt er wenigstens markiert
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      boxRef.current?.select();
    }
  };

  return (
    <div className="section-editor" role="dialog" aria-label="Abschnitt bearbeiten" onKeyDown={onKeyDown}>
      <div className="section-editor-head">
        <strong>{title}</strong>
        <span className="muted">Zeilen {startLine}–{endLine}</span>
      </div>
      <textarea ref={boxRef} aria-label="Markdown" autoFocus value={text} onChange={(e) => setText(e.target.value)} />
      {conflict && (
        <p role="alert" className="error">Das Dokument wurde zwischenzeitlich geändert. Es wurde nichts überschrieben.</p>
      )}
      {!conflict && error && <p role="alert" className="error">{error}</p>}
      <div className="doc-comment-actions">
        <span className="selection-popover-hint">Strg+Enter speichert</span>
        {conflict && <button type="button" className="btn small" onClick={() => void copy()}>Text kopieren</button>}
        {conflict && <button type="button" className="btn small" onClick={onReload}>Dokument neu laden</button>}
        <button type="button" className="btn small" onClick={onClose}>Abbrechen</button>
        <button type="button" className="btn small primary" disabled={saving || conflict} onClick={() => onSave(text)}>Speichern</button>
      </div>
    </div>
  );
}
