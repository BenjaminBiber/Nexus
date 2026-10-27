import { useEffect, useId, useState } from 'react';
import { Link } from 'react-router-dom';
import type { DocQuestionResultDto, QuestionDraftDto, QuestionDraftInputDto, QuestionDraftResultDto } from '../api/types.js';
import { DbModal } from './db/DbModal.js';
import { LoadingState } from './LoadingState.js';

interface Props {
  pid: string;
  /** Dokument, aus dem die Fragen stammen (Kopfzeile) */
  path: string;
  result: QuestionDraftResultDto;
  created: DocQuestionResultDto | null;
  creating: boolean;
  error: string | null;
  onCreate: (drafts: QuestionDraftInputDto[]) => void;
  /** Schließt den Dialog; die Entwürfe sind danach weg */
  onClose: () => void;
}

/** Entwurf im Dialog samt Häkchen „anlegen“ und eigener Antwort (leer = Frage bleibt offen) */
type Card = QuestionDraftDto & { chosen: boolean; answer: string };

function Icon({ name }: { name: string }) {
  return <span className="material-icons" aria-hidden="true">{name}</span>;
}

// Dialog „Offene Fragen“ im Aufbau der Ticket-Entwürfe: links die gefundenen Fragen mit Häkchen, rechts Titel und
// Beschreibung der gewählten zum Bearbeiten, dazu wahlweise die Antwort. „Anlegen“ legt die angehakten als Board-Fragen an –
// beantwortete gleich als entschieden. Die Entwürfe werden nicht gespeichert; wer ungenutzte Entwürfe schließt, wird gefragt.
export function QuestionDraftDialog({ pid, path, result, created, creating, error, onCreate, onClose }: Props) {
  const titleId = useId();
  const [cards, setCards] = useState<Card[]>(() => result.drafts.map((d) => ({ ...d, chosen: true, answer: '' })));
  const [selected, setSelected] = useState(0);
  const [confirmClose, setConfirmClose] = useState(false);

  // Ein neuer Lauf füllt die Karten; danach bleiben die eigenen Änderungen erhalten
  useEffect(() => {
    setCards(result.drafts.map((d) => ({ ...d, chosen: true, answer: '' })));
    setSelected(0);
  }, [result]);

  const patch = (index: number, change: Partial<Card>) =>
    setCards((prev) => prev.map((c, i) => (i === index ? { ...c, ...change } : c)));

  const chosen = cards.filter((c) => c.chosen);
  const ready = chosen.length > 0 && chosen.every((c) => c.title.trim() !== '');
  const answered = chosen.filter((c) => c.answer.trim() !== '').length;
  const card = cards[selected];
  // Ohne angelegte Fragen gehen die Entwürfe beim Schließen verloren – dann erst nachfragen
  const close = () => (created || cards.length === 0 ? onClose() : setConfirmClose(true));
  // Eine Antwort geht nur mit, wenn sie nicht leer ist; dann entscheidet der Server die Frage gleich
  const create = () => onCreate(chosen.map(({ chosen: _, answer, ...d }) => ({
    ...d, title: d.title.trim(), description: d.description.trim(), ...(answer.trim() ? { answer: answer.trim() } : {}),
  })));
  const createLabel = `${chosen.length === 1 ? '1 Frage' : `${chosen.length} Fragen`} anlegen${answered > 0 ? `, ${answered} davon entschieden` : ''}`;
  const stillOpen = created ? created.created.filter((c) => !c.answered).length : 0;
  // Titel der angelegten oder übersprungenen Frage über den Platz in der gesendeten Auswahl
  const sentTitle = (index: number) => chosen[index]?.title ?? `Frage ${index + 1}`;

  return (
    <DbModal labelledBy={titleId} onEscape={close} className="dd-modal draft-modal">
      <header className="dd-head">
        <h2 id={titleId}>Offene Fragen</h2>
        {!created && cards.length > 0 && <span className="dd-state state-idle">{chosen.length} von {cards.length} gewählt</span>}
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
              <Link className="mono" to={`/p/${pid}/questions/${c.id}`}>{c.id}</Link> {sentTitle(c.index)}{' '}
              <span className={`badge ${c.answered ? 'q-answered' : 'q-new'}`}>{c.answered ? 'entschieden' : 'offen'}</span>
              {c.answerError && <span className="muted"> · Antwort nicht gespeichert: {c.answerError}</span>}
            </p>
          ))}
          {created.skipped.map((s) => (
            <p key={s.index} className="muted">{sentTitle(s.index)}: übersprungen: {s.reason}</p>
          ))}
          {created.created.length === 0 && created.skipped.length === 0 && <p className="muted">Es wurde nichts angelegt.</p>}
          {stillOpen > 0 && (
            <p className="muted">
              Ist die automatische Recherche eingeschaltet (Einstellungen → Dispatcher → Recherchen), greift der Dispatcher die offenen
              Fragen auf; sonst startest du sie auf der Seite der Frage mit „Recherche starten“.
            </p>
          )}
        </div>
      ) : (
        <div className="draft-body">
          <div className="draft-tiles-pane">
            {result.dropped.length > 0 && (
              <ul className="draft-dropped muted">
                {result.dropped.map((d) => <li key={d}>{d}</li>)}
              </ul>
            )}
            {cards.length === 0 && <p className="dd-empty">Claude hat in diesem Dokument keine offenen Fragen gefunden.</p>}
            <ul className="draft-tiles" aria-label="Fragen">
              {cards.map((c, index) => (
                <li key={index} className={`dd-card draft-tile${index === selected ? ' is-active' : ''}${c.chosen ? '' : ' is-off'}`}>
                  <button type="button" className="draft-tile-pick" aria-pressed={index === selected} onClick={() => setSelected(index)}>
                    <span className="dd-card-top">
                      <span className="dd-id">#{index + 1}</span>
                      {c.answer.trim() && <span className="badge q-answered">beantwortet</span>}
                    </span>
                    <span className="dd-card-title draft-tile-title">{c.title.trim() || 'Ohne Titel'}</span>
                    <span className="dd-card-foot">
                      {c.source.heading && <span className="draft-tile-source" title={c.source.heading}>{c.source.heading}</span>}
                    </span>
                  </button>
                  <input
                    type="checkbox"
                    className="draft-tile-choose"
                    aria-label={`Frage ${index + 1} anlegen`}
                    checked={c.chosen}
                    onChange={() => patch(index, { chosen: !c.chosen })}
                  />
                </li>
              ))}
            </ul>
          </div>
          {card && (
            <section className="draft-detail" aria-label={`Frage ${selected + 1}`}>
              <p className="draft-source muted">
                Frage #{selected + 1}{card.source.heading ? ` · aus „${card.source.heading}“` : ''} · Zeilen {card.source.startLine}–{card.source.endLine}
              </p>
              <label className="draft-field">
                <span>Frage</span>
                <input aria-label={`Titel (Frage ${selected + 1})`} value={card.title} onChange={(e) => patch(selected, { title: e.target.value })} />
              </label>
              <label className="draft-field">
                <span>Kontext</span>
                <textarea aria-label={`Beschreibung (Frage ${selected + 1})`} rows={6} value={card.description} onChange={(e) => patch(selected, { description: e.target.value })} />
              </label>
              <label className="draft-field">
                <span>Antwort (optional)</span>
                <textarea
                  aria-label={`Antwort (Frage ${selected + 1})`}
                  rows={4}
                  placeholder="Weißt du die Antwort schon? Dann wird die Frage gleich als entschieden angelegt."
                  value={card.answer}
                  onChange={(e) => patch(selected, { answer: e.target.value })}
                />
              </label>
            </section>
          )}
        </div>
      )}
      <footer className="draft-foot">
        {creating && <LoadingState variant="detail" text="Fragen werden angelegt …" />}
        {error && <p role="alert" className="error">{error}</p>}
        {confirmClose && !created ? (
          <div role="alertdialog" aria-labelledby="question-drafts-close" className="conflict doc-tickets-again">
            <strong id="question-drafts-close">Entwürfe verwerfen?</strong>
            <p>Die gefundenen Fragen werden nicht gespeichert und gehen samt deinen Änderungen verloren.</p>
            <div className="actions">
              <button type="button" className="btn small" onClick={() => setConfirmClose(false)}>Weiter bearbeiten</button>
              <button type="button" className="btn small danger" onClick={onClose}>Verwerfen</button>
            </div>
          </div>
        ) : !created && cards.length === 0 ? (
          <button type="button" className="btn" onClick={onClose}>Schließen</button>
        ) : !created && (
          <>
            <span className="muted">
              Legt die gewählten im Projekt des Boards an, mit Quelle und den Tickets desselben Abschnitts. Mit Antwort gleich entschieden, sonst offen für die Recherche.
            </span>
            <button type="button" className="btn primary" disabled={creating || !ready} onClick={create}>{createLabel}</button>
          </>
        )}
      </footer>
    </DbModal>
  );
}
