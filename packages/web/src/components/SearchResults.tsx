import { Link } from 'react-router-dom';
import type { SearchHitDto } from '../api/types.js';
import { docHref, docKey } from '../board/docsTree.js';

// Trefferliste der Suche: Datei, Zeile und Auszug; ein Klick öffnet die Datei mit hervorgehobenem Begriff.
export function SearchResults({ hits, pid, term }: { hits: SearchHitDto[]; pid: string; term: string }) {
  if (hits.length === 0) return <p className="muted">Keine Treffer.</p>;
  return (
    <ul aria-label="Suchergebnisse" className="search-results">
      {hits.map((hit) => (
        <li key={`${docKey(hit)}:${hit.line}`}>
          <Link to={`${docHref(pid, hit)}${hit.board === undefined ? '?' : '&'}q=${encodeURIComponent(term)}`}>
            <span className="mono muted">
              {hit.path}:{hit.line}
            </span>
            <span>{hit.excerpt}</span>
          </Link>
        </li>
      ))}
    </ul>
  );
}
