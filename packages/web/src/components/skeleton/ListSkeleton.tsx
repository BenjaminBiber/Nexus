import { Sk } from './Sk.js';

// Platzhalter für Tabellen und Listen (Fragen, Sessions, Projekte): Zeilen mit Punkt, Text und Badge
export function ListSkeleton({ rows = 5 }: { rows?: number }) {
  return (
    <div className="panel sk-list" aria-hidden="true">
      {Array.from({ length: rows }, (_, i) => (
        <div className="sk-row" key={i}>
          <Sk w={28} h={28} round />
          <div className="sk-grow"><Sk h={15} w={`${70 - (i % 3) * 12}%`} /><Sk h={12} w={`${40 + (i % 2) * 15}%`} /></div>
          <Sk w={72} h={20} round />
        </div>
      ))}
    </div>
  );
}
