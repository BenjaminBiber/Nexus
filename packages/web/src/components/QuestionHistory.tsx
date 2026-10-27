import type { QuestionEntryDto } from '../api/questionTypes.js';
import { MarkdownView } from '../markdown/MarkdownView.js';

const KIND: Record<string, string> = { human: 'Mensch', session: 'Session', subagent: 'Subagent' };
const TYPE: Record<string, string> = {
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

// Datum und Uhrzeit untereinander für die schmale Spalte des Verlaufs
function DateCell({ iso }: { iso: string }) {
  const date = new Date(iso);
  return (
    <time className="entry-date" dateTime={iso}>
      {date.toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric' })}
      <br />
      {date.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' })}
    </time>
  );
}

// Verlauf einer Frage: Kommentare, Änderungen und Entscheidungen mit Datum, Autor und Art.
export function QuestionHistory({ entries }: { entries: QuestionEntryDto[] }) {
  return (
    <section className="panel roomy" aria-label="Verlauf">
      <h2>Verlauf</h2>
      {entries.length === 0 && <p className="muted">Noch keine Einträge.</p>}
      <div>
        {entries.map((entry, index) => {
          const unread = entry.type === 'comment' && entry.read === false;
          return (
            <article key={`${entry.at}-${index}`} className={`entry${unread ? ' unread' : ''}`}>
              <DateCell iso={entry.at} />
              <div>
                <div className="entry-meta">
                  <b>{entry.author}</b>
                  <span className="badge">{KIND[entry.kind] ?? entry.kind}</span>
                  <span className="muted">{TYPE[entry.type] ?? entry.type}</span>
                  {unread && <span className="badge unread">ungelesen</span>}
                </div>
                {entry.type === 'comment' ? <MarkdownView source={entry.text} /> : <p>{entry.text}</p>}
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
