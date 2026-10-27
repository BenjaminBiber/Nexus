import { useId, type KeyboardEvent, type RefObject } from 'react';

// Kürzel für den Avatar: Anfangsbuchstaben zweier Wörter, sonst die ersten zwei Buchstaben („Florian“ → „FL“)
export function initials(name: string | undefined): string {
  const words = (name ?? '').trim().split(/[\s._-]+/).filter(Boolean);
  if (!words.length) return '?';
  const letters = words.length > 1 ? words[0][0] + words[1][0] : words[0].slice(0, 2);
  return letters.toUpperCase();
}

// Eingabe für einen neuen Kommentar: Avatar, Textfeld, Senden-Knopf darunter rechts; Strg+Enter (Cmd+Enter) sendet.
export function DbCommentComposer({ value, onChange, onSend, busy, userName, label, placeholder, button, textareaRef }: {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  busy: boolean;
  userName?: string;
  // Beschriftung des Textfelds (für Screenreader und Tests)
  label: string;
  placeholder?: string;
  button: string;
  // Zugriff auf das Textfeld, z. B. für den ersten Fokus im Dialog
  textareaRef?: RefObject<HTMLTextAreaElement | null>;
}) {
  const id = useId();
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      onSend();
    }
  };
  return (
    <div className="db-composer">
      <span className="db-avatar" aria-hidden="true" title={userName}>{initials(userName)}</span>
      <div className="db-composer-box">
        <label className="sr-only" htmlFor={id}>{label}</label>
        <textarea
          id={id}
          ref={textareaRef}
          className="plain-text"
          rows={3}
          placeholder={placeholder}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
        />
        <div className="db-composer-actions">
          <span className="muted">Strg+Enter</span>
          <button type="button" className="btn primary" disabled={busy || !value.trim()} onClick={onSend}>
            {button}
          </button>
        </div>
      </div>
    </div>
  );
}
