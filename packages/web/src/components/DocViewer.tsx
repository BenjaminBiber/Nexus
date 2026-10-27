import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import type { DocFileDto, DocTicketRefDto } from '../api/types.js';
import type { DocCommentDto } from '../api/types.js';
import { useDocComments, useDocContent, useDocRevision, useDocSection, useDocTicketRefs } from '../hooks/useDocs.js';
import { useDocQuestions } from '../hooks/docQuestionJobs.js';
import { useDocTickets } from '../hooks/useDocTickets.js';
import { HighlightedMarkdown } from '../markdown/HighlightedMarkdown.js';
import { anchorIn, clearHighlights, highlightQuote, lineRangeOf, readSelection, selectionRect, type Selected } from '../markdown/commentAnchors.js';
import { sliceLines } from '../markdown/docSections.js';
import { assignHeadingIds, type HeadingRef } from '../markdown/headingIds.js';
import { SectionEditor } from './SectionEditor.js';
import { DocHeadingTickets } from './DocHeadingTickets.js';
import { DocTicketPanel } from './DocTicketPanel.js';
import { CommentCard } from './CommentCard.js';
import { DocCommentPanel } from './DocCommentPanel.js';
import { DocOpenMenu } from './DocOpenMenu.js';
import { DocToolSection } from './DocToolSection.js';
import { DocsGutter, useDocsTools } from './DocsGutter.js';
import { EmptyState } from './EmptyState.js';
import { ErrorState } from './ErrorState.js';
import { LoadingState } from './LoadingState.js';
import { DraftDialog } from './DraftDialog.js';
import { QuestionDraftDialog } from './QuestionDraftDialog.js';
import { ReviseDialog } from './ReviseDialog.js';
import { SelectionPopover } from './SelectionPopover.js';

// Dekodiert einen Anker; bei ungültiger Kodierung bleibt der Rohwert
function safeDecode(raw: string): string {
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
}

// Ergänzung für den Dokumentkopf: Anzahl offener Kommentare, bei 0 nichts
function openLabel(count: number): string {
  if (count === 0) return '';
  return count === 1 ? ' · 1 offener Kommentar' : ` · ${count} offene Kommentare`;
}

// Zeigt ein Dokument gerendert mit Inhaltsverzeichnis; fehlende Spec-Dateien erklären, welcher Schritt sie erzeugt.
// `board` liest das Dokument aus dem Repo dieses Boards (Board-Umschalter der Docs-Seite), sonst aus dem Projektordner
export function DocViewer({ pid, path, doc, term, board }: { pid: string; path: string; doc: DocFileDto | undefined; term: string; board?: number }) {
  const missing = doc !== undefined && !doc.exists;
  const loaded = useDocContent(pid, missing ? null : path, board);
  const articleRef = useRef<HTMLElement>(null);
  const [headings, setHeadings] = useState<HeadingRef[]>([]);
  const { comments, add, update, remove, error } = useDocComments(pid, missing ? null : path, board);
  const [draft, setDraft] = useState<Selected | null>(null);
  const layoutRef = useRef<HTMLDivElement>(null);
  const [card, setCard] = useState<{ id: string; top: number; left: number; editing: boolean } | null>(null);
  const hideTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [composing, setComposing] = useState(false);
  const [anchor, setAnchor] = useState({ top: 0, left: 0 });
  const [lost, setLost] = useState<Set<string>>(new Set());
  const location = useLocation();
  const navigate = useNavigate();
  const content = loaded.state.status === 'ready' ? loaded.state.data.content : null;
  const shownComment = card ? comments.find((c) => c.id === card.id) : undefined;
  const openCount = comments.filter((c) => c.status === 'open').length;
  const hash = loaded.state.status === 'ready' ? loaded.state.data.hash : null;
  const sections = loaded.state.status === 'ready' ? (loaded.state.data.sections ?? []) : [];
  // Nur Superpowers-Dokumente (Specs und Pläne) lassen sich hier ändern; Glossar und ADRs bleiben lesbar
  const editable = doc?.group === 'spec' || doc?.group === 'plan';
  const [editor, setEditor] = useState<{ title: string; start: number; end: number; text: string } | null>(null);
  const [rangeError, setRangeError] = useState<string | null>(null);
  const [headingBtn, setHeadingBtn] = useState<{ index: number; top: number; left: number } | null>(null);
  const headingTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const section = useDocSection(pid, missing ? null : path, board);
  const { refs: ticketRefs, reload: reloadTicketRefs } = useDocTicketRefs(pid, missing ? null : path, board);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  // Chips der Tickets an den Stellen der getroffenen Überschriften (frei im Layout, siehe Effekt unten)
  const [ticketMarks, setTicketMarks] = useState<{ headingId: string; top: number; left: number; refs: DocTicketRefDto[] }[]>([]);
  const [revise, setRevise] = useState<{ quote: string; start: number; end: number } | null>(null);
  const revision = useDocRevision(pid, missing ? null : path, board);
  // Breite der Werkzeugspalte; außerhalb der Docs-Werkbank gibt es keinen Ziehgriff
  const tools = useDocsTools();
  const tickets = useDocTickets(pid, missing ? null : path, board);
  const questions = useDocQuestions(pid, missing ? null : path, board);

  // Vergibt nach dem Rendern die Überschrift-IDs und baut daraus das Verzeichnis
  useEffect(() => {
    setHeadings(articleRef.current ? assignHeadingIds(articleRef.current) : []);
  }, [content, term]);

  // Hebt offene Kommentare im Dokument hervor (nach dem Such-Effekt der Kindkomponente) und merkt verlorene Stellen
  useEffect(() => {
    const root = articleRef.current;
    if (!root) return;
    clearHighlights(root);
    const notFound = new Set<string>();
    comments.filter((c) => c.status === 'open').forEach((c) => {
      if (!highlightQuote(root, c.quote, c.id)) notFound.add(c.id);
    });
    // Gleicher Inhalt behält die alte Menge, damit kein Render-Kreislauf entsteht
    setLost((prev) => (prev.size === notFound.size && [...notFound].every((id) => prev.has(id)) ? prev : notFound));
  }, [comments, content, term]);

  // Setzt die Chips an die Überschriften, aus denen Tickets entstanden sind; der gerenderte Markdown bleibt unangetastet
  useEffect(() => {
    const layout = layoutRef.current;
    setTicketMarks(!layout ? [] : headings.flatMap((h) => {
      const refs = ticketRefs.filter((t) => t.heading === h.text);
      const el = document.getElementById(h.id);
      return refs.length === 0 || !el ? [] : [{ headingId: h.id, refs, ...anchorIn(layout, el.getBoundingClientRect()) }];
    }));
  }, [headings, ticketRefs]);

  // Springt beim Öffnen einer Adresse mit #abschnitt zur Stelle
  useEffect(() => {
    const id = safeDecode(location.hash.slice(1));
    if (id) document.getElementById(id)?.scrollIntoView?.({ behavior: 'smooth', block: 'start' });
  }, [headings, location.hash]);

  // Springt zur Überschrift und schreibt den Anker in die Adresse
  const jumpTo = (id: string) => {
    document.getElementById(id)?.scrollIntoView?.({ behavior: 'smooth', block: 'start' });
    navigate({ pathname: location.pathname, search: location.search, hash: `#${encodeURIComponent(id)}` }, { replace: true });
  };

  // Scrollt zur Hervorhebung des Kommentars, sonst zu seiner Überschrift
  const jumpToComment = (c: DocCommentDto) => {
    const marks = Array.from(articleRef.current?.querySelectorAll('mark[data-comment-id]') ?? []);
    const mark = marks.find((m) => m.getAttribute('data-comment-id') === c.id);
    const heading = headings.find((h) => h.text === c.heading);
    const target = mark ?? (heading ? document.getElementById(heading.id) : null);
    target?.scrollIntoView?.({ behavior: 'smooth', block: 'center' });
  };

  // Verwirft Entwurf und Popover
  const closeDraft = useCallback(() => { setDraft(null); setComposing(false); }, []);

  // Legt den Kommentar zur markierten Stelle an; der Entwurf bleibt bei einem Fehler erhalten
  const saveDraft = async (text: string) => {
    if (!draft || loaded.state.status !== 'ready') return;
    const lines = lineRangeOf(loaded.state.data.content, draft.quote);
    if (await add({ text, quote: draft.quote, heading: draft.heading, lines })) closeDraft();
  };

  // Blendet die Karte verzögert aus, damit die Maus von der Hervorhebung in die Karte wechseln kann
  const hideSoon = () => {
    clearTimeout(hideTimer.current);
    hideTimer.current = setTimeout(() => setCard((c) => (c && !c.editing ? null : c)), 200);
  };

  // Zeigt die Karte unter der Zeile der Hervorhebung, über der die Maus steht (mit editing: gleich zum Bearbeiten)
  const showCard = (e: React.MouseEvent, editing: boolean) => {
    const mark = (e.target as HTMLElement).closest?.('mark.doc-comment');
    const id = mark?.getAttribute('data-comment-id');
    if (!mark || !id || !layoutRef.current || comments.every((c) => c.id !== id)) return hideSoon();
    clearTimeout(hideTimer.current);
    if (card?.editing && card.id !== id) return;
    const line = Array.from(mark.getClientRects()).find((r) => e.clientY >= r.top && e.clientY <= r.bottom) ?? mark.getBoundingClientRect();
    const at = anchorIn(layoutRef.current, new DOMRect(e.clientX - 12, line.top, 0, line.height));
    setCard((c) => (c?.id === id && (c.editing || !editing) ? c : { id, ...at, editing }));
  };

  // Ein Klick auf eine Hervorhebung (ohne Textauswahl) öffnet den Kommentar zum Bearbeiten
  const clickCard = (e: React.MouseEvent) => {
    if (window.getSelection()?.isCollapsed === false) return;
    showCard(e, true);
  };

  // Speichert den neuen Text; die Karte bleibt offen und zeigt ihn danach wieder an
  const saveCard = async (text: string) => {
    if (card && (await update(card.id, { text }))) setCard({ ...card, editing: false });
  };

  // Merkt die Auswahl als Entwurf samt Position; das Textfeld öffnet erst der Knopf
  const captureSelection = () => {
    if (composing) return; // Beim Tippen bleibt der Entwurf unangetastet
    const selected = articleRef.current ? readSelection(articleRef.current) : null;
    if (!selected || !layoutRef.current) return;
    setDraft(selected);
    setComposing(false);
    setAnchor(anchorIn(layoutRef.current, selectionRect()));
  };

  // Abschnitte, die zu den Überschriften des Verzeichnisses passen – in Dokumentreihenfolge
  const editableSections = sections.filter((s) => s.level === 2 || s.level === 3);

  // Öffnet den Editor für den markierten Zeilenbereich; ohne Zuordnung bleibt ein Hinweis im Popover
  const openEditorForSelection = () => {
    if (!draft || content === null) return;
    const range = lineRangeOf(content, draft.quote);
    if (!range) return setRangeError('Die Markierung ließ sich keiner Zeile zuordnen');
    setRangeError(null);
    section.clearConflict();
    setEditor({ title: draft.heading ?? path, start: range[0], end: range[1], text: sliceLines(content, range[0], range[1]) });
    closeDraft();
  };

  // Öffnet den Editor für den ganzen Abschnitt hinter der Überschrift
  const openEditorForHeading = (index: number) => {
    const found = editableSections[index];
    if (!found || content === null) return;
    section.clearConflict();
    setEditor({ title: found.heading, start: found.startLine, end: found.endLine, text: sliceLines(content, found.startLine, found.endLine) });
    setHeadingBtn(null);
  };

  // Öffnet „Mit Claude ändern“ für den markierten Zeilenbereich; ohne Zuordnung bleibt ein Hinweis im Popover
  const openReviseForSelection = () => {
    if (!draft || content === null) return;
    const range = lineRangeOf(content, draft.quote);
    if (!range) return setRangeError('Stelle nicht gefunden, bitte Dokument neu laden');
    setRangeError(null);
    revision.reset();
    setRevise({ quote: draft.quote, start: range[0], end: range[1] });
    closeDraft();
  };

  // „Tickets erzeugen“: Claude zerlegt das ganze Dokument. Gibt es schon Tickets daraus, bleibt der Knopf gesperrt (sonst doppelte Tickets).
  const ticketsDone = ticketRefs.length > 0;
  const ticketsDoneHint = ticketsDone
    ? `Aus diesem Dokument ${ticketRefs.length === 1 ? 'ist schon 1 Ticket' : `sind schon ${ticketRefs.length} Tickets`} entstanden (${ticketRefs.map((t) => t.id).join(', ')}).`
    : undefined;
  const requestDocTickets = () => {
    if (!ticketsDone) void tickets.request();
  };

  // Legt die akzeptierten Entwürfe an; danach zeigt das Dokument die neuen Tickets (und sperrt „Tickets erzeugen“)
  const acceptDrafts = async (drafts: Parameters<typeof tickets.create>[0]) => {
    if (await tickets.create(drafts)) reloadTicketRefs();
  };

  // Schließt den Entwurfs-Dialog und speichert die bearbeiteten Entwürfe
  const closeDrafts = (drafts: Parameters<typeof tickets.close>[0]) => tickets.close(drafts);

  // Schließt den Dialog und verwirft Vorschlag und Fehler
  const closeRevise = () => {
    setRevise(null);
    revision.reset();
  };

  // Übernimmt den Vorschlag; nur ein Erfolg schließt den Dialog und lädt das Dokument neu
  const applyRevision = async (text: string) => {
    if (await revision.apply(text)) {
      closeRevise();
      loaded.reload();
    }
  };

  // Schreibt den Abschnitt; nur ein Erfolg schließt den Editor und lädt das Dokument neu
  const saveSection = async (text: string) => {
    if (!editor || !hash) return;
    if (await section.save({ startLine: editor.start, endLine: editor.end, content: text, expectedHash: hash })) {
      setEditor(null);
      loaded.reload();
    }
  };

  // Lädt nach einem Konflikt neu und verwirft den Editor
  const reloadAfterConflict = () => {
    section.clearConflict();
    setEditor(null);
    loaded.reload();
  };

  // Blendet den Knopf an der Überschrift verzögert aus, damit die Maus ihn erreicht
  const hideHeadingSoon = () => {
    clearTimeout(headingTimer.current);
    headingTimer.current = setTimeout(() => setHeadingBtn(null), 200);
  };

  // Zeigt den Knopf „Abschnitt bearbeiten“ an der Überschrift unter der Maus
  const showHeadingBtn = (e: React.MouseEvent) => {
    if (!editable || !layoutRef.current || !articleRef.current) return;
    const heading = (e.target as HTMLElement).closest?.('h2, h3');
    if (!heading) return hideHeadingSoon();
    const inDoc = Array.from(articleRef.current.querySelectorAll('h2, h3'));
    const index = inDoc.indexOf(heading);
    // Überschriften und Abschnitte stehen beide in Dokumentreihenfolge; passen die Längen nicht, ist die Zuordnung unsicher
    if (index < 0 || inDoc.length !== editableSections.length) return hideHeadingSoon();
    clearTimeout(headingTimer.current);
    setHeadingBtn({ index, ...anchorIn(layoutRef.current, heading.getBoundingClientRect()) });
  };

  if (missing) return <EmptyState title="Diese Datei existiert nicht mehr." />;
  if (loaded.state.status === 'loading') return <LoadingState variant="doc" text="Dokument wird geladen …" />;
  if (loaded.state.status === 'error') return <ErrorState message={loaded.state.message} onRetry={loaded.retry} />;
  return (
    <div className="doc-layout" ref={layoutRef}>
      <article
        className="doc"
        ref={articleRef}
        onMouseUp={captureSelection}
        onKeyUp={captureSelection}
        onMouseOver={(e) => { showCard(e, false); showHeadingBtn(e); }}
        onMouseLeave={() => { hideSoon(); hideHeadingSoon(); }}
        onClick={clickCard}
      >
        <div className="doc-head">
          <p className="muted mono doc-path">{path}{editable ? '' : ' · Dokument nur lesbar'}{openLabel(openCount)}</p>
          <DocOpenMenu pid={pid} path={path} board={board} />
        </div>
        <HighlightedMarkdown source={loaded.state.data.content} term={term} />
      </article>
      {tickets.result && (
        <DraftDialog
          pid={pid}
          path={path}
          result={tickets.result}
          created={tickets.created}
          creating={tickets.creating}
          error={tickets.error}
          onCreate={(drafts) => void acceptDrafts(drafts)}
          onClose={closeDrafts}
          onDiscard={tickets.discard}
        />
      )}
      {questions.result && (
        <QuestionDraftDialog
          pid={pid}
          path={path}
          result={questions.result}
          created={questions.created}
          creating={questions.creating}
          error={questions.error}
          onCreate={(drafts) => void questions.create(drafts)}
          onClose={questions.close}
        />
      )}
      {draft && (
        <SelectionPopover
          key={draft.quote}
          quote={draft.quote}
          top={anchor.top}
          left={anchor.left}
          composing={composing}
          error={rangeError ?? error}
          editable={editable}
          onEdit={openEditorForSelection}
          onRevise={openReviseForSelection}
          onCompose={() => setComposing(true)}
          onSave={saveDraft}
          onClose={closeDraft}
        />
      )}
      {headingBtn && editable && (
        <button
          type="button"
          className="heading-edit"
          aria-label="Abschnitt bearbeiten"
          style={{ top: headingBtn.top, left: headingBtn.left }}
          onMouseEnter={() => clearTimeout(headingTimer.current)}
          onMouseLeave={hideHeadingSoon}
          onClick={() => openEditorForHeading(headingBtn.index)}
        >
          Abschnitt bearbeiten
        </button>
      )}
      {ticketMarks.map((m) => (
        <DocHeadingTickets key={m.headingId} pid={pid} headingId={m.headingId} tickets={m.refs} top={m.top} left={m.left} />
      ))}
      {revise && hash && (
        <ReviseDialog
          key={`${revise.start}-${revise.end}`}
          quote={revise.quote}
          running={revision.running}
          result={revision.result}
          error={revision.error}
          saving={revision.saving}
          onRequest={(instruction) => void revision.request({ startLine: revise.start, endLine: revise.end, instruction, expectedHash: hash })}
          onApply={(text) => void applyRevision(text)}
          onClose={closeRevise}
        />
      )}
      {editor && (
        <SectionEditor
          key={`${editor.start}-${editor.end}`}
          title={editor.title}
          startLine={editor.start}
          endLine={editor.end}
          value={editor.text}
          saving={section.saving}
          conflict={section.conflict}
          error={section.error}
          onSave={(text) => void saveSection(text)}
          onReload={reloadAfterConflict}
          onClose={() => { setEditor(null); section.clearConflict(); }}
        />
      )}
      {shownComment && card && (
        <CommentCard
          key={card.id}
          comment={shownComment}
          top={card.top}
          left={card.left}
          editing={card.editing}
          onEdit={() => setCard({ ...card, editing: true })}
          onSave={saveCard}
          onClose={() => setCard(null)}
          onEnter={() => clearTimeout(hideTimer.current)}
          onLeave={hideSoon}
        />
      )}
      {tools && <DocsGutter label="Breite der Werkzeuge" width={tools.width} min={220} max={520} sign={-1} onChange={tools.setWidth} />}
      <div className="doc-side">
        {/* „Tickets erzeugen“ oben in der Werkzeugspalte, samt Warnung, Fortschritt und Fehler */}
        {editable && (
          <div className="doc-side-actions">
            {tickets.saved && !tickets.running ? (
              <>
                {/* Gespeicherte Entwürfe: öffnen, daneben neu erzeugen (ersetzt sie) und löschen */}
                <div className="doc-tickets-row">
                  <button type="button" className="btn primary doc-tickets-btn" onClick={tickets.open}>
                    <span className="material-icons" aria-hidden="true">edit_note</span>
                    Entwürfe öffnen
                  </button>
                  <button type="button" className="btn doc-tickets-icon" aria-label="Neu erzeugen" title={ticketsDoneHint ?? 'Neu erzeugen'} disabled={ticketsDone} onClick={requestDocTickets}>
                    <span className="material-icons" aria-hidden="true">refresh</span>
                  </button>
                  <button type="button" className="btn doc-tickets-icon danger" aria-label="Entwürfe löschen" title="Entwürfe löschen" onClick={() => setConfirmDiscard(true)}>
                    <span className="material-icons" aria-hidden="true">delete</span>
                  </button>
                </div>
                <p className="muted doc-tickets-status">
                  {tickets.saved.drafts.length === 1 ? '1 gespeicherter Entwurf' : `${tickets.saved.drafts.length} gespeicherte Entwürfe`}
                </p>
                {confirmDiscard && (
                  <div role="alertdialog" aria-labelledby="doc-drafts-discard" className="conflict doc-tickets-again">
                    <strong id="doc-drafts-discard">Gespeicherte Entwürfe löschen?</strong>
                    <p>{tickets.saved.drafts.length === 1 ? 'Der Entwurf geht' : `Die ${tickets.saved.drafts.length} Entwürfe gehen`} samt deinen Änderungen verloren.</p>
                    <div className="actions">
                      <button type="button" className="btn small" onClick={() => setConfirmDiscard(false)}>Abbrechen</button>
                      <button type="button" className="btn small danger" onClick={() => { setConfirmDiscard(false); tickets.discard(); }}>Löschen</button>
                    </div>
                  </div>
                )}
              </>
            ) : (
              <div className="doc-tickets-row">
                <button
                  type="button"
                  className={`btn primary doc-tickets-btn${tickets.running ? ' is-running' : ''}`}
                  disabled={tickets.running || ticketsDone}
                  title={ticketsDoneHint}
                  onClick={requestDocTickets}
                >
                  <span className="material-icons" aria-hidden="true">{tickets.running ? 'autorenew' : 'auto_awesome'}</span>
                  {tickets.running ? 'Tickets werden erzeugt …' : 'Tickets erzeugen'}
                </button>
                {tickets.running && (
                  <button type="button" className="btn doc-tickets-icon danger" aria-label="Erzeugung abbrechen" title="Abbrechen" onClick={tickets.cancel}>
                    <span className="material-icons" aria-hidden="true">stop_circle</span>
                  </button>
                )}
              </div>
            )}
            {ticketsDone && !tickets.running && <p className="muted doc-tickets-status">{ticketsDoneHint}</p>}
            {tickets.running && (
              <p role="status" className="muted doc-tickets-status">Claude zerlegt das Dokument in Tickets – das kann bis zu 5 Minuten dauern. Du kannst solange auf andere Seiten wechseln.</p>
            )}
            {tickets.error && !tickets.result && <p role="alert" className="error">{tickets.error}</p>}
            {/* „Fragen erzeugen“: Claude liest die offenen Fragen heraus, der Dialog legt die gewählten als Board-Fragen an */}
            <div className="doc-tickets-row">
              <button type="button" className={`btn doc-tickets-btn${questions.running ? ' is-running' : ''}`} disabled={questions.running} onClick={questions.request}>
                <span className="material-icons" aria-hidden="true">{questions.running ? 'autorenew' : 'help_outline'}</span>
                {questions.running ? 'Fragen werden gesucht …' : 'Fragen erzeugen'}
              </button>
              {questions.running && (
                <button type="button" className="btn doc-tickets-icon danger" aria-label="Fragensuche abbrechen" title="Abbrechen" onClick={questions.cancel}>
                  <span className="material-icons" aria-hidden="true">stop_circle</span>
                </button>
              )}
            </div>
            {questions.running && (
              <p role="status" className="muted doc-tickets-status">Claude sucht die offenen Fragen im Dokument – das kann einige Minuten dauern. Du kannst solange auf andere Seiten wechseln.</p>
            )}
            {questions.error && !questions.result && <p role="alert" className="error">{questions.error}</p>}
          </div>
        )}
        {headings.length > 0 && (
          <DocToolSection title="Inhalt">
            <nav className="toc" aria-label="Inhaltsverzeichnis">
              {headings.map((h) => (
                <button key={h.id} type="button" className={h.level === 3 ? 'toc-sub' : undefined} onClick={() => jumpTo(h.id)}>
                  {h.text}
                </button>
              ))}
            </nav>
          </DocToolSection>
        )}
        {ticketRefs.length > 0 && (
          <DocToolSection title="Tickets">
            <DocTicketPanel
              pid={pid}
              tickets={ticketRefs}
              canJump={(t) => headings.some((h) => h.text === t.heading)}
              onJump={(t) => {
                const heading = headings.find((h) => h.text === t.heading);
                if (heading) jumpTo(heading.id);
              }}
            />
          </DocToolSection>
        )}
        <DocToolSection title="Kommentare">
          <DocCommentPanel
            comments={comments}
            missing={lost}
            error={draft && composing ? null : error}
            onJump={jumpToComment}
            onUpdate={update}
            onRemove={remove}
          />
        </DocToolSection>
      </div>
    </div>
  );
}
