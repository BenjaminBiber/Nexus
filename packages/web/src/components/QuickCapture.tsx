import { useRef, useState, type FormEvent } from 'react';

// Schnellerfassung: ein Satz als Titel, Enter legt die Frage an (Herkunft Mensch, Anlass „spontan“).
// Optional mit Knopf zum Anlegen (`submitLabel`) und Knopf zum ausführlichen Formular (`onDetailed`); sendet nie doppelt.
export function QuickCapture({ onCreate, onDetailed, submitLabel }: {
  onCreate: (title: string) => Promise<void>;
  onDetailed?: () => void;
  submitLabel?: string;
}) {
  const [title, setTitle] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const sending = useRef(false);

  // Prüft den Titel und legt die Frage an; bei Fehlern bleibt die Eingabe stehen
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (sending.current) return;
    if (!title.trim()) {
      setError('Bitte einen Titel eingeben.');
      return;
    }
    sending.current = true;
    setBusy(true);
    setError(null);
    try {
      await onCreate(title.trim());
      setTitle('');
    } catch (failure) {
      setError((failure as Error).message);
    } finally {
      sending.current = false;
      setBusy(false);
    }
  };

  return (
    <form className="quick-capture" onSubmit={(e) => void submit(e)}>
      <label className="quick-field">
        <span className="sr-only">Neue Frage</span>
        <input
          type="text"
          value={title}
          disabled={busy}
          placeholder="Frage in einem Satz tippen, Enter legt sie an"
          aria-label="Neue Frage"
          onChange={(e) => setTitle(e.target.value)}
        />
      </label>
      {submitLabel && (
        <button type="submit" className="btn" disabled={busy}>
          {submitLabel}
        </button>
      )}
      {onDetailed && (
        <button type="button" className="btn" onClick={onDetailed}>
          Ausführlich anlegen
        </button>
      )}
      {error && (
        <p role="alert" className="field-error quick-error">
          {error}
        </p>
      )}
    </form>
  );
}
