import { useEffect, useState, type FormEvent, type KeyboardEvent } from 'react';

export interface DbStepFormValue {
  text: string;
  detail: string;
}

// Eingabe für einen Schritt: Text (Pflicht) und optionale Anleitung. Enter im Textfeld speichert, Esc bricht ab
// (ohne das Panel zu schließen). onSave liefert true, wenn gespeichert wurde. onDirtyChange meldet, ob die Eingabe
// vom Ausgangswert abweicht (ungespeicherter Entwurf).
export function DbStepForm({ label, initial, busy, submitLabel, onSave, onCancel, onDirtyChange }: {
  label: string;
  initial?: DbStepFormValue;
  busy: boolean;
  submitLabel: string;
  onSave: (value: DbStepFormValue) => Promise<boolean>;
  onCancel: () => void;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const [text, setText] = useState(initial?.text ?? '');
  const [detail, setDetail] = useState(initial?.detail ?? '');
  const empty = !text.trim();
  const dirty = text !== (initial?.text ?? '') || detail !== (initial?.detail ?? '');
  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (empty || busy) return;
    await onSave({ text: text.trim(), detail });
  };

  const onKey = (event: KeyboardEvent) => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    onCancel();
  };

  return (
    <form className="db-step-form" aria-label={label} onSubmit={(e) => void submit(e)} onKeyDown={onKey}>
      <label className="field">
        <span>Text</span>
        <input value={text} autoFocus maxLength={1000} onChange={(e) => setText(e.target.value)} />
      </label>
      <label className="field">
        <span>Anleitung (optional, Markdown)</span>
        <textarea className="plain-text" rows={3} value={detail} onChange={(e) => setDetail(e.target.value)} />
      </label>
      <div className="actions">
        <button type="submit" className="btn primary small" disabled={empty || busy}>{submitLabel}</button>
        <button type="button" className="btn small" onClick={onCancel}>Abbrechen</button>
      </div>
    </form>
  );
}
