import { Sk } from './Sk.js';

// Platzhalter für ein Dokument: Inhaltsverzeichnis links, Überschrift und Textabsätze rechts
export function DocSkeleton() {
  return (
    <div className="doc-layout" aria-hidden="true">
      <div className="toc">
        <Sk w={90} h={12} />
        {[80, 65, 72, 55].map((w, i) => <Sk key={i} h={14} w={`${w}%`} />)}
      </div>
      <div className="doc sk-doc">
        <Sk h={28} w="45%" />
        <Sk h={14} /><Sk h={14} w="95%" /><Sk h={14} w="80%" />
        <Sk h={20} w="30%" />
        <Sk h={14} /><Sk h={14} w="88%" />
      </div>
    </div>
  );
}
