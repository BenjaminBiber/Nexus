import type { OriginKind } from '../api/questionTypes.js';
import { linksOf, tagsOf, type QuestionFacets, type QuestionFilterState } from '../question/filterQuestions.js';

const ORIGINS: Array<['' | OriginKind, string]> = [
  ['', 'alle'],
  ['human', 'Mensch'],
  ['session', 'Session'],
  ['subagent', 'Subagent'],
];

// Das gewählte Ziel bleibt auswählbar, auch wenn keine der angezeigten Fragen es nennt (z. B. aus einem Link)
function withCurrent(links: Array<{ target: string; type: string }>, current: string): Array<{ target: string; type: string }> {
  if (!current || links.some((l) => l.target === current)) return links;
  return [...links, { target: current, type: 'Bezug' }];
}

// Filterzeile der Fragenliste: Textsuche, Auswahlen für Tag, Bezug und Herkunft, Schalter „wartet auf“ und „recherchiert“.
export function QuestionFilters(props: {
  /** Fragen, aus denen Tags und Bezüge zur Auswahl stammen (Datei- oder DB-Fragen) */
  questions: Array<Pick<QuestionFacets, 'tags' | 'links'>>;
  /** Ohne eigenes Suchfeld, wenn die Seite schon eines hat */
  withSearch?: boolean;
  value: QuestionFilterState;
  onChange: (value: QuestionFilterState) => void;
}) {
  const { value, onChange, withSearch = true } = props;
  const set = (patch: Partial<QuestionFilterState>) => onChange({ ...value, ...patch });
  return (
    <div className="filters question-filters">
      {withSearch && (
        <label className="search">
          <span className="sr-only">Fragen durchsuchen</span>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" aria-hidden="true">
            <circle cx="7" cy="7" r="4.5" />
            <path d="M10.5 10.5L14 14" />
          </svg>
          <input type="search" value={value.text} onChange={(e) => set({ text: e.target.value })} placeholder="Fragen durchsuchen" />
        </label>
      )}
      <label>
        <span className="sr-only">Tag</span>
        <select value={value.tag} onChange={(e) => set({ tag: e.target.value })}>
          <option value="">Tag: alle</option>
          {tagsOf(props.questions).map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span className="sr-only">Bezug</span>
        <select value={value.link} onChange={(e) => set({ link: e.target.value })}>
          <option value="">Bezug: alle</option>
          {withCurrent(linksOf(props.questions), value.link).map((l) => (
            <option key={l.target} value={l.target}>
              {l.type} {l.target}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span className="sr-only">Herkunft</span>
        <select value={value.origin} onChange={(e) => set({ origin: e.target.value as '' | OriginKind })}>
          {ORIGINS.map(([v, label]) => (
            <option key={v} value={v}>
              {v === '' ? 'Herkunft: alle' : label}
            </option>
          ))}
        </select>
      </label>
      <span className="filter-divider" aria-hidden="true" />
      <button type="button" className={`fchip${value.waiting ? ' on' : ''}`} aria-pressed={value.waiting} onClick={() => set({ waiting: !value.waiting })}>
        wartet auf …
      </button>
      <button type="button" className={`fchip${value.researched ? ' on' : ''}`} aria-pressed={value.researched} onClick={() => set({ researched: !value.researched })}>
        recherchiert
      </button>
    </div>
  );
}
