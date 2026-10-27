// Entscheiden oder Verwerfen: Option (links in der Recherche gewählt) oder Freitext, dazu wahlweise eine Begründung (Pflicht nur beim Verwerfen).
export function DecisionForm({
  picked,
  pickedTitle,
  text,
  reason,
  busy,
  error,
  onText,
  onReason,
  onDecide,
  onDiscard,
}: {
  picked: string | null;
  pickedTitle: string | null;
  text: string;
  reason: string;
  busy: boolean;
  error: string | null;
  onText: (value: string) => void;
  onReason: (value: string) => void;
  onDecide: () => void;
  onDiscard: () => void;
}) {
  const hasChoice = picked !== null || text.trim() !== '';
  // Entscheiden braucht nur die Wahl; die Begründung ist freiwillig (Pflicht nur beim Verwerfen)
  const ready = hasChoice;
  return (
    <section className="card decision-form" aria-label="Entscheiden">
      <h2 className="h3">Entscheiden</h2>
      <div className="decision-choice" role="status">
        {picked ? `Gewählt: Option ${picked}${pickedTitle ? ` · ${pickedTitle}` : ''}` : 'Noch keine Option gewählt.'}
      </div>
      <label className="field">
        <span className="muted">oder eigene Antwort (Freitext)</span>
        <input
          type="text"
          value={text}
          disabled={busy || picked !== null}
          placeholder="z. B. Erst bei CSV bleiben und später entscheiden"
          onChange={(e) => onText(e.target.value)}
        />
      </label>
      {picked !== null && <p className="muted hint">Mit gewählter Option ist der Freitext gesperrt. Wähle die Option ab, um eine eigene Antwort einzugeben.</p>}
      {picked === null && text.trim() !== '' && <p className="muted hint">Mit Freitext sind die Optionen gesperrt. Lösche den Text, um eine Option zu wählen.</p>}
      <label className="field">
        <span className="muted">Begründung</span>
        <textarea rows={3} value={reason} disabled={busy} placeholder="Warum fällt die Wahl so aus?" onChange={(e) => onReason(e.target.value)} />
      </label>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
      <div className="actions">
        <button type="button" className="btn primary" disabled={busy || !ready} onClick={onDecide}>
          Entscheiden
        </button>
        <button type="button" className="btn danger" disabled={busy} onClick={onDiscard}>
          Verwerfen
        </button>
        <span className="muted">Verwerfen verlangt eine Begründung, beim Entscheiden ist sie freiwillig.</span>
      </div>
    </section>
  );
}
