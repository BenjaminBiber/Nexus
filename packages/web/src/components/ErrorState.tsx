// Fehlerzustand mit Meldung und Schaltfläche zum erneuten Versuch.
export function ErrorState({ message, detail, onRetry }: { message: string; detail?: string; onRetry?: () => void }) {
  return (
    <div className="state" role="alert">
      <h2>{message}</h2>
      {detail && <p className="muted">{detail}</p>}
      {onRetry && (
        <button type="button" className="btn" onClick={onRetry}>
          Erneut versuchen
        </button>
      )}
    </div>
  );
}
