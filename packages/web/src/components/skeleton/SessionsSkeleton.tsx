import { Sk } from './Sk.js';

// Platzhalter einer Session-Zeile; nutzt das Raster der echten Zeile (Punkt, Zustand, Zeit, Titel, Aktivität, Stern)
function RowSk({ i }: { i: number }) {
  return (
    <div className="session-row">
      <Sk w={8} h={8} round />
      <Sk w={90} h={20} round />
      <Sk w={60} h={12} />
      <Sk h={15} w={`${80 - (i % 3) * 15}%`} />
      <Sk h={12} w={`${60 - (i % 2) * 20}%`} />
      <Sk w={24} h={24} round />
    </div>
  );
}

// Platzhalter der Sessions-Seite: Seitenkopf, Filter und zwei Gruppen mit Kopfzeile und Zeilen
export function SessionsSkeleton() {
  return (
    <div className="page stack" role="status" aria-busy="true">
      <div className="page-head"><h1>Sessions</h1></div>
      <div aria-hidden="true" className="stack">
        <div className="chip-group"><Sk w={80} h={32} /><Sk w={96} h={32} /><Sk w={88} h={32} /></div>
        {[3, 2].map((rows, g) => (
          <div className="panel group" key={g}>
            <div className="group-head"><Sk w={64} h={14} /><Sk w={`${40 - g * 12}%`} h={16} /></div>
            {Array.from({ length: rows }, (_, i) => <RowSk key={i} i={i + g} />)}
          </div>
        ))}
      </div>
      <span className="sr-only">Sessions werden geladen …</span>
    </div>
  );
}
