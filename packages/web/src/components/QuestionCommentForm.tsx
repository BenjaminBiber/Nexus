import { useState, type FormEvent } from 'react';

// Kommentarfeld unter dem Verlauf einer Frage; Kommentare sind zunächst ungelesen, bis eine Session sie bekommt.
export function QuestionCommentForm({
  busy,
  error,
  onSend,
  hint = 'Der Kommentar gilt als ungelesen, bis eine Session ihn bekommen hat.',
}: {
  busy: boolean;
  error: string | null;
  onSend: (text: string) => Promise<boolean>;
  /** Hinweis neben dem Senden-Knopf */
  hint?: string;
}) {
  const [text, setText] = useState('');

  // Absenden; bei Erfolg wird das Feld geleert, leerer Text wird nicht gesendet
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!text.trim()) return;
    if (await onSend(text.trim())) setText('');
  };

  return (
    <form className="panel roomy" onSubmit={submit}>
      <label className="field section-label">
        <span>Neuer Kommentar</span>
        <textarea className="plain-text" value={text} onChange={(e) => setText(e.target.value)} rows={3} placeholder="Kommentar zur Frage schreiben" />
      </label>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
      <div className="comment-foot">
        <span className="muted">{hint}</span>
        <button type="submit" className="btn primary" disabled={busy || !text.trim()}>
          Kommentar senden
        </button>
      </div>
    </form>
  );
}
