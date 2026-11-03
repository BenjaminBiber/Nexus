// Ladezustand: das animierte Nexus-„N“ zentriert in Dauerschleife plus Text für Screenreader und Beschriftung (z. B. „Tickets werden geladen …“).
export function LoadingState({ text }: { text: string }) {
  return (
    <div className="state loading" role="status" aria-busy="true">
      <img className="loader-n" src="/loader-n.webp" alt="" width={96} height={96} />
      <span className="muted loader-caption">{text}</span>
    </div>
  );
}
