import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type MutableRefObject } from 'react';
import {
  DB_COLUMN_LABELS, DB_PRIORITIES, DB_PRIORITY_LABELS, DB_STATUSES,
  type DbCardDto, type DbColumnId, type DbPriority, type DbStatus, type DbTicketDetailDto,
} from '../../api/dbTypes.js';
import { useApi } from '../../api/ApiContext.js';
import type { RawDispatchConfigDto } from '../../api/dispatchTypes.js';
import { isAutopilot } from '../../dbboard/autopilot.js';
import { canEditDependencies } from '../../dbboard/dependencyPicker.js';
import { useDbTicket, type DbChange } from '../../dbboard/useDbTicket.js';
import { useRestoreFocus } from '../../hooks/useRestoreFocus.js';
import { useLiveHandler } from '../../live/useLive.js';
import { ConflictDialog } from '../ConflictDialog.js';
import { DbAcceptanceDialog } from './DbAcceptanceDialog.js';
import { DbAskQuestionDialog } from './DbAskQuestionDialog.js';
import { BranchCombo } from './BranchCombo.js';
import { DbCreateBranchDialog } from './DbCreateBranchDialog.js';
import { DbDependencies } from './DbDependencies.js';
import { DbGraphDialog, type GraphScopeOption } from './DbGraphDialog.js';
import { DbTicketQuestions } from './DbTicketQuestions.js';
import { DbTicketSteps } from './DbTicketSteps.js';
import { DbTicketTabs } from './DbTicketTabs.js';
import { DbMergeRetry } from './DbMergeRetry.js';
import { DbPlanStart } from './DbPlanStart.js';
import { DbReview, showsReview } from './DbReview.js';
import { DbTicketWork } from './DbTicketWork.js';
import { ErrorState } from '../ErrorState.js';
import { LoadingState } from '../LoadingState.js';
import { ProgressRing } from '../ProgressRing.js';

export interface DbTicketPanelProps {
  ticketId: string;
  boardTickets: DbCardDto[];
  columns: Array<{ id: DbColumnId; title: string }>;
  onClose: () => void;
  onChanged: () => void;
  onOpenTicket: (id: string) => void;
  // Gehört das Board dem eigenen Benutzer? Nur dann gibt es „Mergen“/„Erneut mergen“ (ohne Angabe: nein)
  isOwnBoard?: (boardId: number) => boolean;
  // Hier legt das Panel seine Wachfunktion ab: Wechsel (anderes Ticket, anderes Board) laufen darüber und fragen bei ungespeicherten Eingaben nach
  guard?: MutableRefObject<LeaveGuard | null>;
  // Namensanfang der Agents des Dispatchers; mit ihm zeigt der Reiter „Agent“ einen Punkt, solange ein Agent das Ticket bearbeitet
  agentPrefix?: string;
  // Anzeigename des angemeldeten Benutzers (Avatar am Kommentarfeld)
  userName?: string;
}

// Führt einen Wechsel aus, sobald das Panel verlassen werden darf (ggf. nach Rückfrage)
export type LeaveGuard = (action: () => void) => void;

// Entwürfe der Textfelder; gespeichert wird beim Verlassen des Feldes
interface Drafts {
  title: string;
  assignee: string;
  labels: string;
  targetBranch: string;
  blockedReason: string;
}

const DRAFT_KEYS: Array<keyof Drafts> = ['title', 'assignee', 'labels', 'targetBranch', 'blockedReason'];

// Entwürfe aus dem geladenen Ticket
function draftsOf(ticket: DbTicketDetailDto): Drafts {
  return { title: ticket.title, assignee: ticket.assignee ?? '', labels: ticket.labels.join(', '), targetBranch: ticket.targetBranch ?? '', blockedReason: ticket.blockedReason ?? '' };
}

// Wert, wie ihn der Server speichert (getrimmt, Labels als „a, b“); Grundlage für Vergleiche
function normalise(key: keyof Drafts, value: string): string {
  if (key === 'labels') return labelsOf(value).join(', ');
  if (key === 'assignee' || key === 'blockedReason' || key === 'targetBranch') return value.trim();
  return value;
}

function labelsOf(value: string): string[] {
  return value.split(',').map((s) => s.trim()).filter(Boolean);
}

// Entwürfe samt dem Stand, aus dem sie hervorgingen; bearbeitet ist ein Feld, das von diesem Stand abweicht
interface Form {
  drafts: Drafts;
  base: Drafts;
}

// Neuer Ticketstand: nicht bearbeitete Felder übernehmen ihn, bearbeitete behalten die Eingabe
function rebase(form: Form | null, next: Drafts): Form {
  if (!form) return { drafts: next, base: next };
  const drafts = { ...next };
  for (const key of DRAFT_KEYS) if (form.drafts[key] !== form.base[key]) drafts[key] = form.drafts[key];
  return { drafts, base: next };
}

// Seitenpanel eines DB-Tickets über dem Board: Titel, Felder, Abhängigkeiten, Inhalte, Kommentare, Verlauf, Löschen.
export function DbTicketPanel(props: DbTicketPanelProps) {
  const t = useDbTicket(props.ticketId, props.onChanged);
  // Beim Öffnen holt das Panel den Fokus (Esc greift sofort), beim Schließen geht er an die öffnende Karte zurück
  const panelRef = useRef<HTMLElement | null>(null);
  useRestoreFocus(panelRef);
  useEffect(() => {
    const node = panelRef.current;
    if (node && !node.contains(document.activeElement)) node.focus();
  }, []);
  const ticket = t.ticket;
  // Branches des Board-Repos als Vorschläge für den Zielbranch; ohne Repo oder bei einem Fehler bleibt das Feld reine Texteingabe
  const api = useApi();
  const [branches, setBranches] = useState<string[]>([]);
  // Gibt es das Repo auf diesem Rechner? Nur dann lässt sich ein neuer Branch anlegen
  const [repoReady, setRepoReady] = useState(false);
  // Neuer Branch, der auf die Rückfrage „wirklich erstellen?“ wartet
  const [creatingBranch, setCreatingBranch] = useState<string | null>(null);
  const branchBoardId = ticket && !ticket.branch ? ticket.boardId : null;
  useEffect(() => {
    if (branchBoardId === null) return;
    let cancelled = false;
    void api.getDbBranches(branchBoardId).then(
      (list) => { if (!cancelled) { setBranches(list.branches); setRepoReady(list.reason === null); } },
      () => { if (!cancelled) { setBranches([]); setRepoReady(false); } },
    );
    return () => { cancelled = true; };
  }, [api, branchBoardId]);
  // Autopilot-Einstellung des Dispatchers für das Abzeichen; ohne Einstellung (Fehler) kein Abzeichen
  const [autopilot, setAutopilot] = useState<RawDispatchConfigDto['autopilot']>(null);
  useEffect(() => {
    let cancelled = false;
    void api.getDispatchConfig().then(
      (view) => { if (!cancelled) setAutopilot(view.raw.autopilot ?? null); },
      () => { if (!cancelled) setAutopilot(null); },
    );
    return () => { cancelled = true; };
  }, [api]);
  const [form, setForm] = useState<Form | null>(null);
  const drafts = form?.drafts ?? null;
  const [tabsDirty, setTabsDirty] = useState(false);
  // Offener Schritt-Entwurf mit Änderung oder offene Lösch-Rückfrage eines Schritts
  const [stepsDirty, setStepsDirty] = useState(false);
  // Wechsel, der auf die Rückfrage „Ungespeicherte Änderungen“ wartet
  const [leaving, setLeaving] = useState<{ action: () => void } | null>(null);
  // Wechsel, der auf das Speichern gerade verlassener Felder wartet
  const [leaveAfterSave, setLeaveAfterSave] = useState<{ action: () => void } | null>(null);
  const [fieldSaves, setFieldSaves] = useState(0);
  const fieldSaveFailed = useRef(false);
  const reloadPending = useRef(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [copied, setCopied] = useState(false);
  const [askAcceptance, setAskAcceptance] = useState(false);
  const [conflictHint, setConflictHint] = useState(false);
  // Dialog „Frage stellen“ zu diesem Ticket
  const [asking, setAsking] = useState(false);
  // Abhängigkeitsgraph des Tickets offen
  const [graphOpen, setGraphOpen] = useState(false);
  useEffect(() => {
    if (!t.conflict) setConflictHint(false);
  }, [t.conflict]);

  // Entwürfe beim Laden und bei jedem neuen Stand (Speichern, Kommentar, Live) nachziehen, ohne Eingaben zu verlieren
  useEffect(() => {
    if (ticket) setForm((f) => rebase(f, draftsOf(ticket)));
  }, [ticket]);

  // Ungespeichert ist ein Feld, das bearbeitet wurde und nicht dem Ticket entspricht
  const current = ticket ? draftsOf(ticket) : null;
  const fieldsDirty = !!form && !!current && DRAFT_KEYS.some((key) => form.drafts[key] !== form.base[key] && normalise(key, form.drafts[key]) !== normalise(key, current[key]));
  const dirty = fieldsDirty || tabsDirty || stepsDirty;

  // Änderungen anderer (Live) übernehmen; mit ungespeicherten Eingaben erst, wenn sie gespeichert oder verworfen sind
  const { reload } = t;
  useLiveHandler({
    onDbBoard: (boardId) => {
      if (!ticket || boardId !== ticket.boardId) return;
      if (dirty) reloadPending.current = true;
      else void reload();
    },
  });
  useEffect(() => {
    if (dirty || !reloadPending.current) return;
    reloadPending.current = false;
    void reload();
  }, [dirty, reload]);

  const edit = (key: keyof Drafts, value: string) => setForm((f) => (f ? { ...f, drafts: { ...f.drafts, [key]: value } } : f));

  // Schickt die Änderung eines Textfelds
  const saveField = (key: keyof Drafts, value: string): Promise<boolean> => {
    if (key === 'assignee') return t.save({ assignee: value.trim() || null });
    if (key === 'labels') return t.save({ labels: labelsOf(value) });
    if (key === 'targetBranch') return t.save({ targetBranch: value.trim() || null });
    if (key === 'blockedReason') return t.save({ blockedReason: value.trim() || null });
    return t.save({ [key]: value } as DbChange);
  };

  // Nach erfolgreichem Speichern gilt der vom Server normalisierte Wert als Stand des Feldes (sofern nicht inzwischen weitergetippt wurde)
  const { known } = t;
  const adoptSaved = (key: keyof Drafts, sent: string) => {
    const saved = known();
    if (!saved) return;
    const value = draftsOf(saved)[key];
    setForm((f) => (f && f.drafts[key] === sent ? { drafts: { ...f.drafts, [key]: value }, base: { ...f.base, [key]: value } } : f));
  };

  // Speichert ein Textfeld, wenn es sich geändert hat (nicht während der Rückfrage zum Verlassen);
  // `override` für Felder, die den neuen Wert selbst mitbringen (Vorschlagsliste des Zielbranches)
  const commit = (key: keyof Drafts, override?: string) => {
    if (!ticket || !drafts || leaving) return;
    const value = override ?? drafts[key];
    if (normalise(key, value) === normalise(key, draftsOf(ticket)[key])) return;
    setFieldSaves((n) => n + 1);
    void saveField(key, value).then((ok) => {
      if (!ok) fieldSaveFailed.current = true;
      else adoptSaved(key, value);
      setFieldSaves((n) => n - 1);
    });
  };

  // Verlassen (Schließen, anderes Ticket, anderes Board): erst das Speichern gerade verlassener Felder abwarten;
  // scheitert es, bleibt das Panel mit der Fehlermeldung offen; sonst mit ungespeicherten Eingaben nachfragen
  const leave = (action: () => void) => {
    if (leaving) return;
    if (fieldSaves > 0) {
      fieldSaveFailed.current = false;
      setLeaveAfterSave({ action });
    } else if (dirty) setLeaving({ action });
    else action();
  };
  useEffect(() => {
    if (!leaveAfterSave || fieldSaves > 0) return;
    setLeaveAfterSave(null);
    if (fieldSaveFailed.current) return;
    if (dirty) setLeaving(leaveAfterSave);
    else leaveAfterSave.action();
  }, [leaveAfterSave, fieldSaves, dirty]);

  // Wachfunktion für Wechsel von außen bereitstellen (immer mit dem neuesten Stand)
  const guard = props.guard;
  useLayoutEffect(() => {
    if (!guard) return;
    guard.current = leave;
    return () => {
      guard.current = null;
    };
  });

  // Enter verlässt das Feld; gespeichert wird dann über onBlur
  const blurOnEnter = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    event.currentTarget.blur();
  };

  // Schließen; mit ungespeicherten Eingaben erst nachfragen
  const requestClose = () => leave(props.onClose);

  // Ändert den Status; nach „Erledigt“ ohne Abnahme fragt erst der Abnahme-Dialog nach (wie beim Verschieben auf dem Board)
  const changeStatus = (status: DbStatus) => {
    if (!ticket || status === ticket.status) return;
    if (t.conflict) {
      setConflictHint(true);
      return;
    }
    if (status === 'done' && !ticket.acceptance.trim()) setAskAcceptance(true);
    else void t.save({ status });
  };

  // Kopiert die Ticket-ID in die Zwischenablage
  const copyId = async () => {
    try {
      await navigator.clipboard.writeText(props.ticketId);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  // Konflikt lösen; bei „Aktuellen Stand laden“ übernehmen die betroffenen Felder den aktuellen Stand, andere Eingaben bleiben
  const resolveConflict = (choice: 'reload' | 'overwrite') => {
    const conflict = t.conflict;
    if (choice === 'reload' && conflict) {
      const fresh = draftsOf(conflict.current);
      const keys = DRAFT_KEYS.filter((key) => key in conflict.change);
      setForm((f) => (f ? { ...f, drafts: { ...f.drafts, ...Object.fromEntries(keys.map((key) => [key, fresh[key]])) } } : f));
    }
    void t.resolveConflict(choice);
  };

  // Legt den bestätigten Branch im Repo an, nimmt ihn in die Vorschläge auf und speichert ihn als Zielbranch; liefert die Fehlermeldung
  const createBranch = async (name: string): Promise<string | null> => {
    if (branchBoardId === null) return null;
    try {
      await api.createDbBranch(branchBoardId, name);
    } catch (error) {
      return error instanceof Error ? error.message : 'Der Branch ließ sich nicht anlegen.';
    }
    setBranches((list) => (list.includes(name) ? list : [...list, name].sort((a, b) => a.localeCompare(b))));
    setCreatingBranch(null);
    edit('targetBranch', name);
    commit('targetBranch', name);
    return null;
  };

  // Löscht und schließt das Panel
  const remove = async () => {
    if (await t.remove()) props.onClose();
  };

  return (
    <>
      <aside
        ref={panelRef}
        tabIndex={-1}
        className="db-panel"
        role="dialog"
        aria-modal="false"
        aria-label={`Ticket ${props.ticketId}`}
        onKeyDown={(event) => {
          if (event.key === 'Escape') requestClose();
        }}
      >
        <div className="db-panel-head">
          <button type="button" className="icon-btn db-panel-close" aria-label="Schließen" title="Schließen" onClick={requestClose}>
            ×
          </button>
          <div className="db-panel-heading">
            <div className="db-panel-meta">
              <span className="card-id">{props.ticketId}</span>
              <button type="button" className="icon-btn" aria-label="Ticket-ID kopieren" title="Ticket-ID kopieren" onClick={() => void copyId()}>
                ⧉
              </button>
              {copied && (
                <span role="status" className="muted">
                  Kopiert
                </span>
              )}
              {ticket && (
                <span className="muted">
                  Version {ticket.rev} · angelegt von {ticket.createdByName}
                </span>
              )}
              {ticket && isAutopilot(autopilot, ticket.labels) && (
                <span className="badge" title="Läuft ohne Freigaben und Fragen durch den Dispatcher">
                  <span className="material-icons" aria-hidden="true">flight</span>
                  <span>Autopilot</span>
                </span>
              )}
            </div>
            {ticket && drafts && (
              <input className="db-title" aria-label="Titel" value={drafts.title} onChange={(e) => edit('title', e.target.value)} onBlur={() => commit('title')} onKeyDown={blurOnEnter} />
            )}
          </div>
        </div>
        {leaving && (
          <div role="alertdialog" aria-label="Ungespeicherte Änderungen" className="conflict">
            <strong>Ungespeicherte Änderungen</strong>
            <p>Beim Schließen oder Wechseln gehen die Eingaben verloren.</p>
            <div className="actions">
              <button type="button" className="btn" autoFocus onClick={() => setLeaving(null)}>
                Weiter bearbeiten
              </button>
              <button
                type="button"
                className="btn danger"
                onClick={() => {
                  setLeaving(null);
                  leaving.action();
                }}
              >
                Verwerfen
              </button>
            </div>
          </div>
        )}
        {t.state.status === 'loading' && <LoadingState text="Ticket wird geladen …" />}
        {t.state.status === 'error' && <ErrorState message={t.state.message} onRetry={t.retry} />}
        {ticket && drafts && (
          <div className="db-panel-body">
            <div className="db-panel-main">
              {t.conflict && <ConflictDialog conflict={t.conflict} onChoose={resolveConflict} />}
              {conflictHint && (
                <p role="status" className="field-error">
                  Erst den Konflikt auflösen.
                </p>
              )}
              {t.error && (
                <p role="alert" className="field-error">
                  {t.error}
                </p>
              )}
              <div className="db-fields">
                <label className="field">
                  <span>Status</span>
                  <select value={ticket.status} disabled={t.busy} onChange={(e) => changeStatus(e.target.value as DbStatus)}>
                    {DB_STATUSES.map((status) => (
                      <option key={status} value={status}>
                        {DB_COLUMN_LABELS[status]}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="field">
                  <span>Priorität</span>
                  <select value={ticket.priority} disabled={t.busy} onChange={(e) => void t.save({ priority: e.target.value as DbPriority })}>
                    {DB_PRIORITIES.map((priority) => (
                      <option key={priority} value={priority}>
                        {DB_PRIORITY_LABELS[priority]}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="option-row db-field-check">
                  <input type="checkbox" checked={ticket.needsPlanning} disabled={t.busy} onChange={(e) => void t.save({ needsPlanning: e.target.checked })} />
                  <span>Braucht Planung</span>
                </label>
                <label className="field">
                  <span>Zuständig</span>
                  <input value={drafts.assignee} placeholder="Niemand zugewiesen" onChange={(e) => edit('assignee', e.target.value)} onBlur={() => commit('assignee')} onKeyDown={blurOnEnter} />
                </label>
                <label className="field">
                  <span>Labels</span>
                  <input value={drafts.labels} placeholder="kommagetrennt" onChange={(e) => edit('labels', e.target.value)} onBlur={() => commit('labels')} onKeyDown={blurOnEnter} />
                </label>
                <label className="field">
                  <span>Zielbranch</span>
                  <BranchCombo
                    value={drafts.targetBranch}
                    options={branches}
                    placeholder="ausgecheckter Branch"
                    readOnly={!!ticket.branch}
                    title={ticket.branch ? 'Nach dem Start nicht mehr änderbar' : undefined}
                    onChange={(value) => edit('targetBranch', value)}
                    onCommit={(value) => commit('targetBranch', value)}
                    onCreate={repoReady ? setCreatingBranch : undefined}
                  />
                </label>
                <label className="field wide">
                  <span>Blockiert-Grund</span>
                  <input value={drafts.blockedReason} placeholder="Nicht blockiert" onChange={(e) => edit('blockedReason', e.target.value)} onBlur={() => commit('blockedReason')} onKeyDown={blurOnEnter} />
                </label>
              </div>
              <DbMergeRetry ticket={ticket} mine={props.isOwnBoard?.(ticket.boardId) ?? false} onDone={() => { void reload(); props.onChanged(); }} />
              {showsReview(ticket, props.isOwnBoard?.(ticket.boardId) ?? false) && (
                <DbReview ticket={ticket} onDone={() => { void reload(); props.onChanged(); }} />
              )}
              <DbTicketTabs
                ticket={ticket}
                busy={t.busy}
                conflictOpen={!!t.conflict}
                onSave={(field, value) => t.save({ [field]: value } as DbChange)}
                onComment={t.comment}
                agentPrefix={props.agentPrefix}
                userName={props.userName}
                onDirtyChange={setTabsDirty}
                questions={<DbTicketQuestions ticket={ticket} onAsk={() => setAsking(true)} />}
                steps={
                  <DbTicketSteps
                    ticket={ticket}
                    busy={t.busy || !!t.conflict}
                    onToggle={(number, done, expectText) => void t.step(number, done, expectText)}
                    onAdd={t.addStep}
                    onEdit={t.editStep}
                    onRemove={t.removeStep}
                    onMove={t.moveStep}
                    onDirtyChange={setStepsDirty}
                  />
                }
              />
            </div>
            <div className="db-panel-side">
              <section className="db-side-section" aria-label="Fortschritt">
                <h3 className="db-side-title">Fortschritt</h3>
                <ProgressRing done={ticket.stepsDone} total={ticket.stepsTotal} />
              </section>
              <DbPlanStart ticket={ticket} mine={props.isOwnBoard?.(ticket.boardId) ?? false} onDone={() => { void reload(); props.onChanged(); }} />
              <section className="db-side-section" aria-label="Abhängigkeiten">
                <h3 className="db-side-title">
                  Abhängigkeiten
                  {ticket.dependsOn.length > 0 && (
                    <span className="db-side-meta">
                      {ticket.dependsOn.length} Voraussetzung{ticket.dependsOn.length === 1 ? '' : 'en'}
                    </span>
                  )}
                </h3>
                {!canEditDependencies(ticket) && !ticket.dependsOn.length && !ticket.dependents.length && <p className="muted">Keine.</p>}
                <DbDependencies
                  key={ticket.id}
                  ticket={ticket}
                  boardTickets={props.boardTickets}
                  columns={props.columns}
                  busy={t.busy}
                  conflictOpen={!!t.conflict}
                  onChange={(update) => t.save((base) => ({ dependsOn: update(base.dependsOn) }))}
                  onOpen={props.onOpenTicket}
                  onShowGraph={() => setGraphOpen(true)}
                />
              </section>
              <DbTicketWork ticket={ticket} />
              <div className="db-panel-foot">
                {confirmDelete ? (
                  <div role="alertdialog" aria-label="Ticket löschen" className="actions">
                    <span>Ticket {ticket.id} endgültig löschen?</span>
                    <button type="button" className="btn danger" onClick={() => void remove()}>
                      Endgültig löschen
                    </button>
                    <button type="button" className="btn" onClick={() => setConfirmDelete(false)}>
                      Abbrechen
                    </button>
                  </div>
                ) : (
                  <button type="button" className="btn danger" onClick={() => setConfirmDelete(true)}>
                    Ticket löschen
                  </button>
                )}
              </div>
            </div>
          </div>
        )}
      </aside>
      {askAcceptance && ticket && (
        <DbAcceptanceDialog
          ticketId={ticket.id}
          onCancel={() => setAskAcceptance(false)}
          onConfirm={(acceptance) => {
            setAskAcceptance(false);
            void t.save({ status: 'done', acceptance });
          }}
        />
      )}
      {creatingBranch !== null && (
        <DbCreateBranchDialog name={creatingBranch} onCancel={() => setCreatingBranch(null)} onConfirm={() => createBranch(creatingBranch)} />
      )}
      {graphOpen && ticket && (
        <DbGraphDialog
          scopes={graphScopes(ticket)}
          onClose={() => setGraphOpen(false)}
          onOpenTicket={(id) => {
            setGraphOpen(false);
            props.onOpenTicket(id);
          }}
        />
      )}
      {asking && ticket && (
        <DbAskQuestionDialog
          projectKey={ticket.id.split('-')[0]}
          ticketId={ticket.id}
          onClose={() => setAsking(false)}
          onCreated={() => {
            setAsking(false);
            void reload();
            props.onChanged();
          }}
        />
      )}
    </>
  );
}

// Umfänge des Graphen aus Sicht eines Tickets: das Ticket selbst, seine Spec (Projekt = Präfix der ID) und sein Board
function graphScopes(ticket: { id: string; spec: string | null; boardId: number }): GraphScopeOption[] {
  const scopes: GraphScopeOption[] = [{ label: 'Ticket', scope: { kind: 'ticket', id: ticket.id } }];
  if (ticket.spec) scopes.push({ label: `Spec ${ticket.spec}`, scope: { kind: 'spec', project: ticket.id.split('-')[0], spec: ticket.spec } });
  scopes.push({ label: 'Board', scope: { kind: 'board', boardId: ticket.boardId } });
  return scopes;
}
