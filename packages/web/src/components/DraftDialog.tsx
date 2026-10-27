import { useEffect, useId, useState } from 'react';
import { Link } from 'react-router-dom';
import type { DocTicketResultDto, TicketDraftDto, TicketDraftResultDto } from '../api/types.js';
import { CYCLE_REASON } from '../dbboard/dependencyPicker.js';
import { DbModal } from './db/DbModal.js';

interface Props {
  pid: string;
  /** Dokument, aus dem die Entwürfe stammen (Kopfzeile) */
  path: string;
  result: TicketDraftResultDto;
  created: DocTicketResultDto | null;
  creating: boolean;
  error: string | null;
  onCreate: (drafts: TicketDraftDto[]) => void;
  /** Schließt den Dialog; die bearbeiteten Entwürfe werden gespeichert */
  onClose: (drafts: TicketDraftDto[]) => void;
  /** Verwirft die gespeicherten Entwürfe der Datei */
  onDiscard: () => void;
}

/** Entwurf im Dialog; die Labels bleiben während des Bearbeitens ein Komma-Text */
type Card = TicketDraftDto & { labelsText: string };

// Macht aus einem Entwurf eine bearbeitbare Karte
function toCard(draft: TicketDraftDto): Card {
  return { ...draft, labelsText: draft.labels.join(', ') };
}

// Macht aus der Karte wieder einen Entwurf: leere Schritte und Labels fallen weg
function toDraft(card: Card): TicketDraftDto {
  const { labelsText, ...draft } = card;
  return {
    ...draft,
    steps: draft.steps.map((s) => s.trim()).filter((s) => s !== ''),
    labels: labelsText.split(',').map((l) => l.trim()).filter((l) => l !== ''),
  };
}

// Entfernt eine Karte und rechnet die Abhängigkeiten auf die neue Reihenfolge um; Bezüge auf die verworfene Karte fallen weg
function dropCard(cards: Card[], index: number): Card[] {
  return cards
    .filter((_, i) => i !== index)
    .map((c) => ({ ...c, dependsOn: c.dependsOn.filter((d) => d !== index).map((d) => (d > index ? d - 1 : d)) }));
}

// Alle Karten, die direkt oder über Umwege von der Karte abhängen; sie als Voraussetzung zu wählen ergäbe einen Kreis
export function draftDependents(cards: Pick<Card, 'dependsOn'>[], index: number): Set<number> {
  const found = new Set<number>();
  const queue = [index];
  while (queue.length) {
    const current = queue.shift()!;
    cards.forEach((c, i) => {
      if (c.dependsOn.includes(current) && !found.has(i) && i !== index) {
        found.add(i);
        queue.push(i);
      }
    });
  }
  return found;
}

function Icon({ name }: { name: string }) {
  return <span className="material-icons" aria-hidden="true">{name}</span>;
}

// Dialog „Ticket-Entwürfe“ im Aufbau des Dispatchers: links die Entwürfe als Kacheln, rechts die Details der gewählten Kachel
// zum Bearbeiten. Verworfene Kacheln fallen samt Bezügen weg, „Akzeptieren“ legt den Rest als Tickets an.
export function DraftDialog({ pid, path, result, created, creating, error, onCreate, onClose, onDiscard }: Props) {
  const titleId = useId();
  const [cards, setCards] = useState<Card[]>(() => result.drafts.map(toCard));
  const [selected, setSelected] = useState(0);

  // Ein neuer Lauf füllt die Karten; danach bleiben die eigenen Änderungen erhalten
  useEffect(() => {
    setCards(result.drafts.map(toCard));
    setSelected(0);
  }, [result]);

  const patch = (index: number, change: Partial<Card>) =>
    setCards((prev) => prev.map((c, i) => (i === index ? { ...c, ...change } : c)));

  // Verwirft eine Kachel; die Auswahl bleibt auf derselben Karte bzw. rückt auf die nächste
  const drop = (index: number) => {
    setCards((prev) => dropCard(prev, index));
    setSelected((s) => (s > index ? s - 1 : Math.max(0, Math.min(s, cards.length - 2))));
  };

  const card = cards[selected];
  const close = () => onClose(cards.map(toDraft));
  const steps = cards.reduce((n, c) => n + c.steps.length, 0);

  return (
    <DbModal labelledBy={titleId} onEscape={close} className="dd-modal draft-modal">
      <header className="dd-head">
        <h2 id={titleId}>Ticket-Entwürfe</h2>
        {!created && <span className="dd-state state-idle">{cards.length} {cards.length === 1 ? 'Entwurf' : 'Entwürfe'} · {steps} Schritte</span>}
        <span className="dd-checked mono" title={path}>{path.slice(path.lastIndexOf('/') + 1)}</span>
        {result.costUsd !== null && <span className="dd-checked">Kosten: {result.costUsd.toFixed(2)} USD</span>}
        <div className="dd-tools">
          <button type="button" className="dd-tool plain" aria-label="Schließen" title="Schließen" onClick={close}><Icon name="close" /></button>
        </div>
      </header>
      {created ? (
        <div className="dd-pad draft-result">
          {created.created.map((c) => (
            <p key={c.id}>
              <Link className="mono" to={`/p/${pid}/board/t/${c.id}`}>{c.id}</Link>{' '}
              {cards[c.index]?.title ?? `Entwurf ${c.index + 1}`}
            </p>
          ))}
          {created.skipped.map((s) => (
            <p key={s.index} className="muted">
              {cards[s.index]?.title ?? `Entwurf ${s.index + 1}`}: übersprungen: {s.reason}{' '}
              {s.existingId && <Link className="mono" to={`/p/${pid}/board/t/${s.existingId}`}>bereits vorhanden</Link>}
            </p>
          ))}
          {created.created.length === 0 && created.skipped.length === 0 && <p className="muted">Es wurde nichts angelegt.</p>}
        </div>
      ) : (
        // Während des Anlegens bleibt der Inhalt sichtbar, ist aber gesperrt
        <div className={`draft-body${creating ? ' is-busy' : ''}`} inert={creating || undefined}>
          <div className="draft-tiles-pane">
            {result.dropped.length > 0 && (
              <ul className="draft-dropped muted">
                {result.dropped.map((d) => <li key={d}>{d}</li>)}
              </ul>
            )}
            {cards.length === 0 && <p className="dd-empty">Keine Entwürfe übrig.</p>}
            <ul className="draft-tiles" aria-label="Entwürfe">
              {cards.map((c, index) => (
                <li key={index} className={`dd-card draft-tile${index === selected ? ' is-active' : ''}`}>
                  <button type="button" className="draft-tile-pick" aria-pressed={index === selected} onClick={() => setSelected(index)}>
                    <span className="dd-card-top">
                      <span className="dd-id">#{index + 1}</span>
                      {c.dependsOn.length > 0 && <span className="dd-kind kind-plan">nach {c.dependsOn.map((d) => `#${d + 1}`).join(', ')}</span>}
                    </span>
                    <span className="dd-card-title draft-tile-title">{c.title.trim() || 'Ohne Titel'}</span>
                    <span className="dd-card-foot">
                      <span>{c.steps.length} {c.steps.length === 1 ? 'Schritt' : 'Schritte'}</span>
                      {c.source.heading && <span className="draft-tile-source" title={c.source.heading}>{c.source.heading}</span>}
                    </span>
                  </button>
                  <button type="button" className="dd-tool plain draft-tile-drop" aria-label={`Entwurf ${index + 1} verwerfen`} title="Verwerfen" onClick={() => drop(index)}>
                    <Icon name="delete_outline" />
                  </button>
                </li>
              ))}
            </ul>
          </div>
          {card && <DraftDetail key={selected} index={selected} card={card} cards={cards} onPatch={(change) => patch(selected, change)} />}
        </div>
      )}
      <footer className="draft-foot">
        {error && <p role="alert" className="error">{error}</p>}
        {!created && (
          <>
            <button type="button" className="btn small draft-discard" disabled={creating} onClick={onDiscard}>Entwürfe verwerfen</button>
            {creating ? (
              <span role="status" className="muted">Lege {cards.length === 1 ? 'ein Ticket' : `${cards.length} Tickets`} im Backlog an …</span>
            ) : (
              <span className="muted">Legt {cards.length === 1 ? 'ein Ticket' : `${cards.length} Tickets`} im Backlog mit Planungsbedarf an.</span>
            )}
            <button
              type="button"
              className={`btn primary draft-accept${creating ? ' is-running' : ''}`}
              disabled={creating || cards.length === 0}
              aria-busy={creating || undefined}
              onClick={() => onCreate(cards.map(toDraft))}
            >
              {creating && <Icon name="autorenew" />}
              {creating ? 'Wird angelegt …' : 'Akzeptieren'}
            </button>
          </>
        )}
      </footer>
    </DbModal>
  );
}

// Details der gewählten Kachel: Titel, Beschreibung, Abnahme, Schritte, Labels und Abhängigkeiten
function DraftDetail({ index, card, cards, onPatch }: { index: number; card: Card; cards: Card[]; onPatch: (change: Partial<Card>) => void }) {
  const n = index + 1;
  const setStep = (step: number, text: string) => onPatch({ steps: card.steps.map((s, i) => (i === step ? text : s)) });
  const toggleDep = (dep: number) =>
    onPatch({ dependsOn: card.dependsOn.includes(dep) ? card.dependsOn.filter((d) => d !== dep) : [...card.dependsOn, dep].sort((a, b) => a - b) });
  const dependents = draftDependents(cards, index);

  return (
    <section className="draft-detail" aria-label={`Entwurf ${n}`}>
      <p className="draft-source muted">
        Entwurf #{n}{card.source.heading ? ` · aus „${card.source.heading}“` : ''} · Zeilen {card.source.startLine}–{card.source.endLine}
      </p>
      <label className="draft-field">
        <span>Titel</span>
        <input aria-label={`Titel (Entwurf ${n})`} value={card.title} onChange={(e) => onPatch({ title: e.target.value })} />
      </label>
      <label className="draft-field">
        <span>Beschreibung</span>
        <textarea aria-label={`Beschreibung (Entwurf ${n})`} rows={5} value={card.description} onChange={(e) => onPatch({ description: e.target.value })} />
      </label>
      <label className="draft-field">
        <span>Abnahme</span>
        <textarea aria-label={`Abnahme (Entwurf ${n})`} rows={3} value={card.acceptance} onChange={(e) => onPatch({ acceptance: e.target.value })} />
      </label>
      <div className="draft-field">
        <span>Schritte</span>
        <ol className="draft-steps">
          {card.steps.map((step, s) => (
            <li key={s} className="draft-step">
              <input aria-label={`Schritt ${s + 1} (Entwurf ${n})`} value={step} onChange={(e) => setStep(s, e.target.value)} />
              <button
                type="button"
                className="dd-tool plain"
                aria-label={`Schritt ${s + 1} entfernen (Entwurf ${n})`}
                title="Schritt entfernen"
                onClick={() => onPatch({ steps: card.steps.filter((_, i) => i !== s) })}
              >
                <Icon name="remove_circle_outline" />
              </button>
            </li>
          ))}
        </ol>
        <button type="button" className="btn small draft-add-step" aria-label={`Schritt hinzufügen (Entwurf ${n})`} onClick={() => onPatch({ steps: [...card.steps, ''] })}>
          + Schritt
        </button>
      </div>
      <label className="draft-field">
        <span>Labels (mit Komma getrennt)</span>
        <input aria-label={`Labels (Entwurf ${n})`} value={card.labelsText} onChange={(e) => onPatch({ labelsText: e.target.value })} />
      </label>
      {cards.length > 1 && (
        <fieldset className="draft-field draft-deps" aria-label={`Abhängigkeiten (Entwurf ${n})`}>
          <span>Hängt ab von</span>
          {cards.map((other, i) => {
            if (i === index) return null;
            const checked = card.dependsOn.includes(i);
            // Ein schon gesetzter Kreis bleibt abwählbar, ein neuer lässt sich nicht anlegen
            const cycle = dependents.has(i);
            return (
              <label key={i} className={`draft-dep${cycle ? ' blocked' : ''}`} title={cycle ? `#${i + 1} läuft erst nach #${n} – das wäre ein Kreis` : undefined}>
                <input type="checkbox" checked={checked} disabled={cycle && !checked} onChange={() => toggleDep(i)} />
                <span className="dd-id">#{i + 1}</span> {other.title.trim() || 'Ohne Titel'}
                {cycle && <span className="draft-dep-reason">{CYCLE_REASON}</span>}
              </label>
            );
          })}
        </fieldset>
      )}
    </section>
  );
}
