import { useId, useRef, useState, type FormEvent, type KeyboardEvent, type ReactNode } from 'react';
import type { DbQuestionDetailDto, DbQuestionOptionDto, DbQuestionResearchDto } from '../../api/dbTypes.js';
import { historyEntries } from '../../dbquestion/questionDtos.js';
import { ORIGIN_KIND, dateDe, dateTimeDe, timeDe } from '../../dbquestion/questionFormat.js';
import { SECTION, isSection, parseFindings, splitEffort, type MdSection, type QuestionParts } from '../../dbquestion/questionSections.js';
import { MarkdownView } from '../../markdown/MarkdownView.js';
import { AgentRuns } from './AgentRuns.js';
import { OptionProsCons } from './DbQuestionDecide.js';

type Tab = 'description' | 'research' | 'history' | 'runs';

const TABS: Tab[] = ['description', 'research', 'history', 'runs'];

const TAB_LABELS: Record<Tab, string> = { description: 'Beschreibung', research: 'Recherche', history: 'Verlauf', runs: 'Läufe' };

const ENTRY_TYPE: Record<string, string> = {
  created: 'angelegt',
  comment: 'Kommentar',
  title: 'Titel geändert',
  description: 'Beschreibung geändert',
  tags: 'Tags geändert',
  occasion: 'Anlass geändert',
  links: 'Bezüge geändert',
  depends: 'Abhängigkeiten geändert',
  research: 'Recherche ergänzt',
  decided: 'Entschieden',
  discarded: 'Verworfen',
  reopened: 'Wieder geöffnet',
  tickets: 'Tickets geändert',
  'ticket-deleted': 'Ticket gelöscht',
};

// Abschnitt mit Überschrift und Markdown
function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="qx-sec">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

// Abschnitte der Beschreibung; bei offenen Fragen ohne „Worum es geht“ und „Zu entscheiden“ (stehen in der Entscheidungskarte)
function DescriptionPanel({ parts, open, researched }: { parts: QuestionParts; open: boolean; researched: boolean }) {
  const { intro, sections } = parts.description;
  const shown = sections.filter((s) => s.body !== '' && !(open && isSection(s.title, ...SECTION.about, ...SECTION.decide)));
  // Bei offenen Fragen mit Recherche stammen Varianten und Empfehlung hier aus der Planung (die der Recherche stehen oben zur Wahl)
  const titleOf = (s: MdSection) => {
    if (!(open && researched)) return s.title;
    if (isSection(s.title, ...SECTION.variants)) return 'Varianten aus der Planung';
    if (isSection(s.title, ...SECTION.recommendation)) return 'Empfehlung der Planung';
    return s.title;
  };
  if (!intro && shown.length === 0) {
    return <p className="qx-muted">{sections.length > 0 ? 'Alles Weitere steht oben in der Entscheidung.' : '(keine Beschreibung)'}</p>;
  }
  return (
    <>
      {intro && <MarkdownView source={intro} />}
      {shown.map((s, index) => (
        <Section key={`${s.title}-${index}`} title={titleOf(s)}>
          <MarkdownView source={s.body} />
        </Section>
      ))}
    </>
  );
}

// Fundstellen als aufklappbare Liste „Pfad – was dort passiert“; Text in anderer Form bleibt Markdown
function Findings({ body, open }: { body: string; open: boolean }) {
  const list = parseFindings(body);
  return (
    <details className="qx-findings" open={open}>
      <summary>Fundstellen im Code{list ? ` · ${list.length}` : ''}</summary>
      {list ? (
        <ul>
          {list.map((f, index) => (
            <li key={`${f.path}-${index}`}>
              <code title={f.path}>{f.path}</code>
              {f.note && <span>{f.note}</span>}
            </li>
          ))}
        </ul>
      ) : (
        <MarkdownView source={body} />
      )}
    </details>
  );
}

// Optionen einer Recherche, nur lesbar und je Option aufklappbar (entschiedene Fragen, ältere Recherchen)
function ReadOnlyOptions({ options, chosen, note }: { options: DbQuestionOptionDto[]; chosen: string | null; note: string | null }) {
  if (options.length === 0) return null;
  return (
    <Section title="Optionen">
      {note && <p className="qx-muted">{note}</p>}
      <div className="qx-ro-options">
        {options.map((o) => {
          const { text, effort } = splitEffort(o.text);
          return (
            <details key={o.id} className="qx-ro-option">
              <summary>
                <span className="mono qx-ro-key">{o.id}</span>
                <span className="qx-ro-title">{o.title}</span>
                {o.id === chosen && <span className="qx-chip accent">gewählt</span>}
                {o.recommended && <span className="qx-chip ok">empfohlen</span>}
                {effort && <span className="qx-chip">Aufwand: {effort}</span>}
                <span className="material-icons qx-ro-chevron" aria-hidden="true">
                  expand_more
                </span>
              </summary>
              <div className="qx-ro-body">
                {text && <p>{text}</p>}
                <OptionProsCons option={o} />
              </div>
            </details>
          );
        })}
      </div>
    </Section>
  );
}

// Neueste Recherche: Kopf, Abschnitte des Kontexts (ohne die, die schon in der Entscheidungskarte oder als Folgefrage stehen),
// Fundstellen; bei entschiedenen Fragen die Optionen nur lesbar. Ältere Recherchen eingeklappt darunter.
function ResearchPanel({ question, parts }: { question: DbQuestionDetailDto; parts: QuestionParts }) {
  const latest = question.research.at(-1);
  if (!latest || !parts.context) {
    return <p className="qx-muted">Noch keine Recherche. Ein Agent kann im Code nachsehen und Optionen vorschlagen – rechts unter „Recherche“.</p>;
  }
  const open = question.state === 'open';
  const decision = question.decision;
  const late = decision !== null && latest.at > decision.at;
  const skip = (s: MdSection) =>
    s.body === '' ||
    isSection(s.title, ...SECTION.followUp) ||
    (open && isSection(s.title, ...SECTION.today)) ||
    (open && parts.decide?.from === 'research' && isSection(s.title, ...SECTION.decide)) ||
    (open && parts.about?.from === 'research' && isSection(s.title, ...SECTION.about));
  const sections = parts.context.sections.filter((s) => !skip(s));
  const options = open ? null : (
    <ReadOnlyOptions
      key="options"
      options={latest.options}
      chosen={decision?.option ?? null}
      note={late ? 'nachträglich ergänzt, nicht mehr wählbar' : null}
    />
  );
  const todayIndex = sections.findIndex((s) => isSection(s.title, ...SECTION.today));
  const body: ReactNode[] = [];
  if (todayIndex < 0 && options) body.push(options);
  sections.forEach((s, index) => {
    const key = `${s.title}-${index}`;
    if (isSection(s.title, ...SECTION.findings)) body.push(<Findings key={key} body={s.body} open={open} />);
    else
      body.push(
        <Section key={key} title={isSection(s.title, ...SECTION.today) ? 'Kontext · Heute' : s.title}>
          <MarkdownView source={s.body} />
        </Section>,
      );
    if (index === todayIndex && options) body.push(options);
  });
  const older = question.research.slice(0, -1).reverse();
  return (
    <>
      <div className="qx-research-meta">
        <span className="qx-badge">neueste</span>
        <strong>{dateTimeDe(latest.at)}</strong>
        <span className="qx-muted">
          {latest.byName} ({ORIGIN_KIND[latest.kind] ?? latest.kind})
          {open && latest.options.length > 0 ? ` · ihre ${latest.options.length} Optionen stehen oben zur Wahl` : ''}
        </span>
      </div>
      {parts.context.intro && <MarkdownView source={parts.context.intro} />}
      {body}
      {older.length > 0 && <OlderResearch research={older} />}
    </>
  );
}

// Frühere Recherchen, eingeklappt und nur lesbar
function OlderResearch({ research }: { research: DbQuestionResearchDto[] }) {
  return (
    <Section title={research.length === 1 ? 'Frühere Recherche' : 'Frühere Recherchen'}>
      {research.map((r) => (
        <details key={r.id} className="qx-older">
          <summary>
            {dateTimeDe(r.at)} · {r.byName} ({ORIGIN_KIND[r.kind] ?? r.kind})
          </summary>
          <div className="qx-older-body">
            {r.context.trim() && <MarkdownView source={r.context} />}
            <ReadOnlyOptions options={r.options} chosen={null} note={null} />
          </div>
        </details>
      ))}
    </Section>
  );
}

// Verlauf (Änderungen und Kommentare) mit Kommentarfeld
function HistoryPanel({ question, busy, error, onComment }: {
  question: DbQuestionDetailDto;
  busy: boolean;
  error: string | null;
  onComment: (body: string) => Promise<boolean>;
}) {
  const fieldId = useId();
  const [text, setText] = useState('');
  const entries = historyEntries(question);

  // Sendet den Kommentar; bei Erfolg wird das Feld geleert
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!text.trim()) return;
    if (await onComment(text.trim())) setText('');
  };

  return (
    <>
      {entries.length === 0 && <p className="qx-muted">Noch keine Einträge.</p>}
      {entries.length > 0 && (
        <ol className="qx-history">
          {entries.map((entry, index) => (
            <li key={`${entry.at}-${index}`} className="qx-entry">
              <time dateTime={entry.at}>
                {dateDe(entry.at)}
                <br />
                {timeDe(entry.at)}
              </time>
              <div className="qx-entry-body">
                <div className="qx-entry-meta">
                  <strong>{entry.author}</strong>
                  <span className="qx-chip">{ORIGIN_KIND[entry.kind] ?? entry.kind}</span>
                  <span className={`qx-entry-type ${entry.type}`}>{ENTRY_TYPE[entry.type] ?? entry.type}</span>
                </div>
                {entry.type === 'comment' ? <MarkdownView source={entry.text} /> : entry.text && <p>{entry.text}</p>}
              </div>
            </li>
          ))}
        </ol>
      )}
      <form className="qx-comment" onSubmit={(event) => void submit(event)}>
        <label htmlFor={fieldId} className="sr-only">
          Neuer Kommentar
        </label>
        <textarea id={fieldId} rows={2} placeholder="Kommentar zur Frage schreiben" value={text} onChange={(event) => setText(event.target.value)} />
        {error && (
          <p role="alert" className="qx-error">
            {error}
          </p>
        )}
        <div className="qx-comment-foot">
          <span className="qx-muted">
            Sessions sehen Kommentare mit <code>nexus board question show</code>
          </span>
          <button type="submit" className="qx-btn" disabled={busy || !text.trim()}>
            Kommentar senden
          </button>
        </div>
      </form>
    </>
  );
}

// Inhalt einer Frage in Reitern: Beschreibung, Recherche, Verlauf, Läufe des Dispatchers
export function DbQuestionTabs({ question, parts, runRefresh, busy, commentError, onComment }: {
  question: DbQuestionDetailDto;
  parts: QuestionParts;
  /** Neuer Wert lädt die Läufe sofort nach (laufende Recherche) */
  runRefresh: string;
  busy: boolean;
  commentError: string | null;
  onComment: (body: string) => Promise<boolean>;
}) {
  const baseId = useId();
  const [tab, setTab] = useState<Tab>('description');
  const refs = useRef(new Map<Tab, HTMLButtonElement>());
  const open = question.state === 'open';
  const counts: Partial<Record<Tab, number>> = {
    research: question.research.length,
    history: question.history.length + question.comments.length,
  };

  // Pfeiltasten wechseln den Reiter (ringsum), Pos1/Ende springen an den Rand
  const onKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    const index = TABS.indexOf(tab);
    const next =
      event.key === 'ArrowRight' ? (index + 1) % TABS.length
        : event.key === 'ArrowLeft' ? (index - 1 + TABS.length) % TABS.length
          : event.key === 'Home' ? 0
            : event.key === 'End' ? TABS.length - 1
              : null;
    if (next === null) return;
    event.preventDefault();
    setTab(TABS[next]);
    refs.current.get(TABS[next])?.focus();
  };

  return (
    <section className="qx-content" aria-label="Inhalt">
      <div role="tablist" aria-label="Inhalt der Frage" className="qx-tabs">
        {TABS.map((value) => {
          const count = counts[value] ?? 0;
          return (
            <button
              key={value}
              ref={(el) => {
                if (el) refs.current.set(value, el);
                else refs.current.delete(value);
              }}
              id={`${baseId}-tab-${value}`}
              type="button"
              role="tab"
              className="qx-tab"
              aria-selected={tab === value}
              aria-controls={`${baseId}-panel`}
              tabIndex={tab === value ? 0 : -1}
              onClick={() => setTab(value)}
              onKeyDown={onKey}
            >
              {TAB_LABELS[value]}
              {count > 0 && (
                <>
                  {' '}
                  <span className="qx-tab-count">{count}</span>
                </>
              )}
            </button>
          );
        })}
      </div>
      <div role="tabpanel" id={`${baseId}-panel`} aria-labelledby={`${baseId}-tab-${tab}`} className={`qx-panel ${tab}`}>
        {tab === 'description' && <DescriptionPanel parts={parts} open={open} researched={question.research.length > 0} />}
        {tab === 'research' && <ResearchPanel question={question} parts={parts} />}
        {tab === 'history' && <HistoryPanel question={question} busy={busy} error={commentError} onComment={onComment} />}
        {tab === 'runs' && <AgentRuns id={question.id} refresh={runRefresh} empty="Zu dieser Frage gab es noch keinen Recherche-Lauf." />}
      </div>
    </section>
  );
}
