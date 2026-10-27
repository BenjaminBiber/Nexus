import { useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { ApiError } from '../../api/ApiError.js';
import type { DispatchPromptKindDto } from '../../api/dispatchTypes.js';
import { LineDiffView } from '../LineDiffView.js';

export interface PromptPreviewProps {
  kind: DispatchPromptKindDto;
  /** Der aktuelle, noch ungespeicherte Inhalt des Editors; leer = Standardvorlage */
  template: string;
  /** Standardvorlage des Servers; fehlt sie (älterer Server), gilt jede eigene Vorlage als abweichend */
  defaultTemplate?: string;
}

interface Result {
  prompt: string;
  /** Derselbe Lauf mit der Standardvorlage – nur beim Vergleich gefüllt */
  standard: string | null;
  /** Mit Beispielwerten gerendert, nicht mit denen eines Tickets */
  sample: boolean;
}

// Meldung des Servers; eine abgelehnte Vorlage nennt zusätzlich die fehlenden Pflichtteile und unbekannten Platzhalter
function message(error: unknown): string {
  if (!(error instanceof ApiError)) return 'Die Vorschau ist fehlgeschlagen.';
  const details = error.details ?? {};
  const list = (value: unknown) => (Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : []);
  const missing = list(details.missing);
  const unknown = list(details.unknown);
  const extra = [
    missing.length ? `Fehlende Pflichtteile: ${missing.join('; ')}` : '',
    unknown.length ? `Unbekannte Platzhalter: ${unknown.map((n) => `{{${n}}}`).join(', ')}` : '',
  ].filter(Boolean);
  return [error.message, ...extra].join(' ');
}

// Vorschau einer Vorlage: rendert den fertigen Prompt über den Server – mit den Werten eines Tickets bzw. einer Frage
// oder mit Beispielwerten. Rein lesend: die Vorschau fasst das Formular nicht an und löst kein Auto-Speichern aus.
export function PromptPreview({ kind, template, defaultTemplate }: PromptPreviewProps) {
  const api = useApi();
  const [id, setId] = useState('');
  const [compare, setCompare] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<Result | null>(null);

  // Eine Vorlage, die leer ist oder wörtlich dem Standard entspricht, rendert denselben Prompt – ein Vergleich lohnt nicht
  const own = template.trim() === '' || template.trim() === (defaultTemplate ?? '').trim() ? null : template;

  const run = async (withCompare: boolean) => {
    setBusy(true);
    setError(null);
    try {
      const ticket = id.trim() || undefined;
      // Der Standard läuft mit derselben ID, damit nur die Vorlage und nicht die Werte den Unterschied erzeugen
      const [mine, standard] = await Promise.all([
        api.dispatchPromptPreview(kind, own, ticket),
        withCompare && own !== null ? api.dispatchPromptPreview(kind, null, ticket) : Promise.resolve(null),
      ]);
      setResult({ prompt: mine.prompt, standard: standard ? standard.prompt : null, sample: mine.sample });
    } catch (e) {
      setResult(null);
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };

  // Der Umschalter wirkt sofort: liegt schon eine Vorschau vor, wird sie mit der neuen Einstellung neu geholt
  const toggleCompare = (on: boolean) => {
    setCompare(on);
    if (result) void run(on);
  };

  const lines = result ? result.prompt.split('\n').length : 0;

  return (
    <div className="prompt-preview-box">
      <div className="prompt-preview-head">
        <label className="field">
          <span>Ticket-/Frage-ID (leer = Beispielwerte)</span>
          <input className="mono" value={id} placeholder={kind === 'research' ? 'ARC-F0007' : 'ARC-0042'} onChange={(e) => setId(e.target.value)} />
        </label>
        <button type="button" className="btn" disabled={busy} onClick={() => void run(compare)}>Vorschau</button>
        <label className="option-row">
          <input type="checkbox" checked={compare} onChange={(e) => toggleCompare(e.target.checked)} />
          <span>Unterschiede zum Standard</span>
        </label>
      </div>
      {busy && <p className="muted">Wird gerendert …</p>}
      {error && <div className="banner" role="alert">{error}</div>}
      {result && !busy && (
        <>
          <p className="muted">
            {result.sample ? 'Beispielwerte' : `Werte von ${id.trim()}`} · {lines} {lines === 1 ? 'Zeile' : 'Zeilen'} · {result.prompt.length} Zeichen
          </p>
          {compare && own === null && <p className="muted">Entspricht dem Standard</p>}
          {compare && result.standard !== null
            ? <LineDiffView original={result.standard} proposal={result.prompt} label="Unterschiede zum Standard-Prompt" className="prompt-diff" />
            : <pre className="plain-text prompt-preview" aria-label="Gerenderter Prompt">{result.prompt}</pre>}
        </>
      )}
    </div>
  );
}
