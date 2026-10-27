import { Sk } from './Sk.js';

// Platzhalter eines Karten (Ticket) im Board: Nummer + Badge, Titel, Schritte
function CardSk({ lines }: { lines: number }) {
  return (
    <div className="card">
      <div className="card-top"><Sk w={52} h={12} /><Sk w={64} h={20} round /></div>
      <Sk h={16} w="85%" />
      {lines > 1 && <Sk h={16} w="55%" />}
      <Sk h={12} w="35%" />
    </div>
  );
}

// Platzhalter für das Board: drei Spalten mit Kopf und unterschiedlich vielen Karten
export function BoardSkeleton() {
  return (
    <div className="columns" aria-hidden="true">
      {[2, 3, 1].map((cards, i) => (
        <div className="column" key={i}>
          <div className="column-head"><Sk w={90} h={16} /><Sk w={20} h={14} /></div>
          {Array.from({ length: cards }, (_, k) => <CardSk key={k} lines={(i + k) % 2 + 1} />)}
        </div>
      ))}
    </div>
  );
}
