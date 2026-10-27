import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { DbTicketDetailDto } from '../../api/dbTypes.js';
import { formatDate } from '../../dbboard/formatDate.js';
import { MarkdownView } from '../../markdown/MarkdownView.js';
import { MermaidBlock } from '../../markdown/MermaidBlock.js';
import { AgentRuns } from './AgentRuns.js';
import { DbCommentButton } from './DbCommentButton.js';
import { DbCommentDialog } from './DbCommentDialog.js';
import { DbTicketTimeline } from './DbTicketTimeline.js';
import { Collapsible } from '../Collapsible.js';

export type DbTextField = 'description' | 'plan' | 'handbook' | 'diagram';
type TabKey = DbTextField | 'steps' | 'comments' | 'history' | 'agent';

const TEXT_TABS: Array<{ key: DbTextField; label: string; empty: string }> = [
  { key: 'description', label: 'Beschreibung', empty: 'Noch keine Beschreibung.' },
  { key: 'plan', label: 'Plan', empty: 'Noch kein Plan.' },
  { key: 'handbook', label: 'Handbuch', empty: 'Noch kein Handbuch.' },
  { key: 'diagram', label: 'Diagramm', empty: 'Kein Diagramm. Mermaid-Quelltext ohne Code-Zaun eintragen.' },
];

// Strg+Enter (oder Cmd+Enter) löst die Aktion aus
function ctrlEnter(action: () => void) {
  return (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      action();
    }
  };
}

// Reiter des Ticket-Panels: Texte mit Vorschau/Bearbeiten, Kommentare, Verlauf und Agent (Läufe des Dispatchers, live);
// meldet ungespeicherte Eingaben. Der Punkt am Reiter „Agent“: ein Lauf läuft oder ein Agent (agentPrefix) hat das Ticket in Arbeit.
export function DbTicketTabs({ ticket, busy, conflictOpen = false, agentPrefix, userName, steps, onSave, onComment, onDirtyChange }: {
  ticket: DbTicketDetailDto;
  busy: boolean;
  // Inhalt des Reiters „Schritte“; bleibt beim Reiterwechsel eingehängt (nur versteckt), damit offene Entwürfe erhalten bleiben
  steps?: ReactNode;
  conflictOpen?: boolean;
  // Namensanfang der Agents des Dispatchers (z. B. „junior-“)
  agentPrefix?: string;
  // Anzeigename des angemeldeten Benutzers (Avatar am Kommentarfeld)
  userName?: string;
  onSave: (field: DbTextField, value: string) => Promise<boolean>;
  onComment: (body: string) => Promise<boolean>;
  onDirtyChange: (dirty: boolean) => void;
}) {
  const [tab, setTab] = useState<TabKey>('description');
  const [editing, setEditing] = useState<DbTextField | null>(null);
  const [draft, setDraft] = useState('');
  const [comment, setComment] = useState('');
  // Dialog für einen neuen Kommentar; der Entwurf bleibt auch geschlossen erhalten
  const [commentOpen, setCommentOpen] = useState(false);
  const [askSwitch, setAskSwitch] = useState<DbTextField | null>(null);
  // Nicht gespeicherter Inhalt (z. B. Konflikt); erscheint er später im Ticket, schließt der Editor
  const [unsaved, setUnsaved] = useState<{ field: DbTextField; value: string } | null>(null);
  const sending = useRef(false);
  const [hint, setHint] = useState(false);
  useEffect(() => {
    if (!conflictOpen) setHint(false);
  }, [conflictOpen]);
  // Bei offenem Konflikt geht nichts hinaus: stattdessen kurz darauf hinweisen
  const guarded = (action: () => void) => () => {
    if (conflictOpen) setHint(true);
    else action();
  };
  const textDirty = editing !== null && draft !== ticket[editing];
  const dirty = textDirty || comment.trim() !== '';

  useEffect(() => onDirtyChange(dirty), [dirty, onDirtyChange]);

  // Läuft gerade ein Lauf des Dispatchers für das Ticket? Bei jedem neuen Ticketstand nachgefragt, bei offenem Reiter
  // meldet ihn die Lauf-Ansicht selbst; Fehler (z. B. älterer Server) lassen den Punkt einfach weg
  const api = useApi();
  const [agentRunning, setAgentRunning] = useState(false);
  useEffect(() => {
    let stale = false;
    api.getDispatchRuns(ticket.id).then(
      (runs) => { if (!stale) setAgentRunning(runs.some((r) => r.running)); },
      () => undefined,
    );
    return () => { stale = true; };
  }, [api, ticket.id, ticket.rev]);
  const agentActive = agentRunning || (ticket.status === 'in_progress' && !!agentPrefix && !!ticket.assignee?.startsWith(agentPrefix));

  // Nach „Meine Änderung trotzdem speichern“ steht der Inhalt im Ticket: Editor schließen
  useEffect(() => {
    if (!unsaved || ticket[unsaved.field] !== unsaved.value) return;
    setUnsaved(null);
    if (editing === unsaved.field && draft === unsaved.value) setEditing(null);
  }, [ticket, unsaved, editing, draft]);

  // Öffnet den Editor eines Textreiters mit dem gespeicherten Inhalt
  const openEditor = (field: DbTextField) => {
    setAskSwitch(null);
    setUnsaved(null);
    setDraft(ticket[field]);
    setEditing(field);
  };

  // Mit ungespeichertem Inhalt in einem anderen Reiter erst nachfragen
  const startEdit = (field: DbTextField) => {
    if (textDirty && editing !== field) setAskSwitch(field);
    else openEditor(field);
  };

  // Führt einen Schreibvorgang aus, solange kein anderer aus diesem Reiterbereich läuft (kein doppeltes Strg+Enter)
  const once = async (work: () => Promise<void>) => {
    if (sending.current || busy) return;
    sending.current = true;
    try {
      await work();
    } finally {
      sending.current = false;
    }
  };

  // Speichert den Editor und schließt ihn bei Erfolg
  const saveText = () => once(async () => {
    if (!editing) return;
    const field = editing;
    const value = draft;
    if (await onSave(field, value)) setEditing(null);
    else setUnsaved({ field, value });
  });

  // Schickt den Kommentar ab und leert das Feld bei Erfolg
  const sendComment = () => once(async () => {
    const body = comment.trim();
    if (body && (await onComment(body))) {
      setComment('');
      setCommentOpen(false);
    }
  });

  const textTab = (t: (typeof TEXT_TABS)[number]) => ({ key: t.key as TabKey, label: t.label, filled: ticket[t.key].trim() !== '' });
  const tabs: Array<{ key: TabKey; label: string; filled: boolean }> = [
    textTab(TEXT_TABS[0]),
    ...(steps ? [{ key: 'steps' as TabKey, label: `Schritte (${ticket.stepsDone}/${ticket.stepsTotal})`, filled: false }] : []),
    ...TEXT_TABS.slice(1).map(textTab),
    { key: 'comments', label: `Kommentare (${ticket.comments.length})`, filled: false },
    { key: 'history', label: 'Verlauf', filled: false },
    { key: 'agent', label: 'Agent', filled: agentActive },
  ];
  const text = TEXT_TABS.find((t) => t.key === tab);

  return (
    <div className="db-tabs">
      <div role="tablist" aria-label="Ticketinhalt" className="db-tablist">
        {tabs.map((t) => (
          <button
            key={t.key}
            type="button"
            role="tab"
            id={`db-tab-${t.key}`}
            aria-selected={tab === t.key}
            aria-controls="db-tabpanel"
            className={tab === t.key ? 'active' : ''}
            onClick={() => setTab(t.key)}
          >
            {t.label}
            {t.filled && (
              <span aria-hidden="true" className="db-tab-dot">
                {' '}•
              </span>
            )}
          </button>
        ))}
      </div>
      {/* eigener Name statt aria-labelledby, damit das Textfeld allein den Reiternamen als Beschriftung trägt */}
      {askSwitch && editing && (
        <div role="alertdialog" aria-label="Ungespeicherter Inhalt" className="conflict">
          <strong>Ungespeicherter Inhalt</strong>
          <p>Der Inhalt in „{TEXT_TABS.find((t) => t.key === editing)?.label}“ ist noch nicht gespeichert.</p>
          <div className="actions">
            <button
              type="button"
              className="btn"
              autoFocus
              onClick={() => {
                setAskSwitch(null);
                setTab(editing);
              }}
            >
              Weiter bearbeiten
            </button>
            <button type="button" className="btn danger" onClick={() => openEditor(askSwitch)}>
              Verwerfen
            </button>
          </div>
        </div>
      )}
      {hint && (
        <p role="status" className="field-error">
          Erst den Konflikt auflösen.
        </p>
      )}
      <div role="tabpanel" id="db-tabpanel" aria-label={`Reiter ${tabs.find((t) => t.key === tab)?.label ?? ''}`} className="db-tabpanel">
        {text && editing === text.key && (
          <>
            <textarea aria-label={text.label} rows={14} spellCheck={false} value={draft} onChange={(e) => setDraft(e.target.value)} onKeyDown={ctrlEnter(guarded(() => void saveText()))} autoFocus />
            <div className="actions">
              <button type="button" className="btn primary" disabled={busy} onClick={() => void saveText()}>
                Speichern (Strg+Enter)
              </button>
              <button type="button" className="btn" onClick={() => setEditing(null)}>
                Abbrechen
              </button>
            </div>
          </>
        )}
        {text && editing !== text.key && (
          <>
            <div className="actions">
              <button type="button" className="btn" onClick={() => startEdit(text.key)}>
                Bearbeiten
              </button>
            </div>
            {!ticket[text.key].trim() ? (
              <p className="muted">{text.empty}</p>
            ) : text.key === 'diagram' ? (
              <MermaidBlock source={ticket.diagram} title={`${ticket.id} – Diagramm`} />
            ) : (
              <MarkdownView source={ticket[text.key]} />
            )}
          </>
        )}
        {tab === 'comments' && (
          <>
            <div className="actions db-comment-add">
              <DbCommentButton onOpen={guarded(() => setCommentOpen(true))} busy={busy} />
            </div>
            {ticket.comments.length === 0 && <p className="muted">Noch keine Kommentare.</p>}
            {/* neueste oben */}
            <ol className="db-entries">
              {[...ticket.comments].reverse().map((c) => (
                <li key={c.id} className="entry">
                  <span className="entry-date">{formatDate(c.createdAt)}</span>
                  <div>
                    <div className="entry-meta">
                      <strong>{c.authorName}</strong>
                    </div>
                    <Collapsible text={c.body}>
                      <MarkdownView source={c.body} />
                    </Collapsible>
                  </div>
                </li>
              ))}
            </ol>
          </>
        )}
        {steps && <div className="db-steps-tab" hidden={tab !== 'steps'}>{steps}</div>}
        {tab === 'agent' && <AgentRuns id={ticket.id} refresh={`${ticket.status} ${ticket.assignee ?? ''}`} onRunning={setAgentRunning} />}
        {tab === 'history' && (
          <DbTicketTimeline
            ticketId={ticket.id}
            rev={ticket.rev}
            composer={{ onOpen: guarded(() => setCommentOpen(true)), busy }}
          />
        )}
      </div>
      {commentOpen && (
        <DbCommentDialog
          value={comment}
          onChange={setComment}
          onSend={() => void sendComment()}
          busy={busy}
          userName={userName}
          onClose={() => setCommentOpen(false)}
        />
      )}
    </div>
  );
}
