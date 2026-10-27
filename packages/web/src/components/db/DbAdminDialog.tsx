import { useId, useRef, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { DbBoardDto, DbBoardPatch, DbProjectDto } from '../../api/dbTypes.js';
import { CommentIcon } from '../CommentIcon.js';
import { BoardEditDialog, ProjectEditDialog } from './DbAdminEditDialogs.js';
import { DbModal } from './DbModal.js';

// „2 Boards · 17 offen“ für die Kopfzeile eines Projekts
function projectSummary(projectBoards: DbBoardDto[]): string {
  const open = projectBoards.reduce((sum, b) => sum + b.openTickets, 0);
  return `${projectBoards.length} ${projectBoards.length === 1 ? 'Board' : 'Boards'} · ${open} offen`;
}

type Editing = { kind: 'project'; key: string } | { kind: 'board'; id: number };

// Verwaltung der Board-Datenbank: einklappbare Projekte mit ihren Boards. Neues Projekt, Projekt bearbeiten (Name) und
// Board bearbeiten (Name, eigener Repo-Pfad (lokal)) öffnen eigene Dialoge; ein Board löschen verweigert der Server bei
// offenen Tickets, ausdrücklich „auch offene Tickets“ löscht alles. Nie zwei Aufrufe zugleich; Esc bricht erst eine offene
// Löschabfrage oder den obersten Dialog ab, dann schließt es die Verwaltung.
export function DbAdminDialog({ projects, boards, onClose, onChanged }: {
  projects: DbProjectDto[];
  boards: DbBoardDto[];
  onClose: () => void;
  onChanged: () => void;
}) {
  const api = useApi();
  const [info, setInfo] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [adding, setAdding] = useState(false);
  const [newKey, setNewKey] = useState('');
  const [newName, setNewName] = useState('');
  const [editing, setEditing] = useState<Editing | null>(null);
  // Auf-/zugeklappt je Projektkey; ohne Eintrag ist nur das erste Projekt aufgeklappt
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  // Dialog „Board hinzufügen“: Key des Projekts (null = zu) und Name des neuen Boards
  const [addingBoard, setAddingBoard] = useState<string | null>(null);
  const [boardName, setBoardName] = useState('');
  const [deleting, setDeleting] = useState<{ id: number; all: boolean } | null>(null);
  const sending = useRef(false);
  const dialogRef = useRef<HTMLElement | null>(null);
  const titleId = useId();
  const addTitleId = useId();
  const addBoardTitleId = useId();

  const editedProject = editing?.kind === 'project' ? projects.find((p) => p.key === editing.key) : undefined;
  const editedBoard = editing?.kind === 'board' ? boards.find((b) => b.id === editing.id) : undefined;
  // Ein Dialog über der Verwaltung zeigt Fehler selbst
  const addedToProject = addingBoard === null ? undefined : projects.find((p) => p.key === addingBoard);
  const dialogOpen = adding || !!addedToProject || !!editedProject || !!editedBoard;

  const isExpanded = (p: DbProjectDto) => expanded[p.key] ?? p.key === projects[0]?.key;
  const toggle = (p: DbProjectDto) => setExpanded((e) => ({ ...e, [p.key]: !isExpanded(p) }));

  // Führt eine Aktion aus, meldet Erfolg oder Fehler und lädt die Seite nach; true bei Erfolg. Nie zwei zugleich.
  const run = async (work: () => Promise<unknown>, done: string): Promise<boolean> => {
    if (sending.current) return false;
    sending.current = true;
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      await work();
      setInfo(done);
      onChanged();
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    } finally {
      sending.current = false;
      setBusy(false);
    }
  };

  // Öffnet den Projekt-Dialog mit leeren Feldern
  const openAdd = () => {
    setError(null);
    setInfo(null);
    setNewKey('');
    setNewName('');
    setAdding(true);
  };

  // Schließt den Projekt-Dialog ohne Anlegen
  const closeAdd = () => {
    setAdding(false);
    setError(null);
  };

  // Öffnet den Bearbeiten-Dialog eines Projekts oder Boards
  const openEdit = (target: Editing) => {
    setError(null);
    setInfo(null);
    setEditing(target);
  };

  // Schließt den Bearbeiten-Dialog ohne Speichern
  const closeEdit = () => {
    setEditing(null);
    setError(null);
  };

  // Legt ein Projekt an, schließt den Dialog und klappt das neue Projekt auf
  const createProject = async () => {
    const key = newKey.trim();
    const name = newName.trim();
    if (!key || !name) return;
    if (await run(() => api.createDbProject(key, name), `Projekt ${key.toUpperCase()} angelegt.`)) {
      setExpanded((e) => ({ ...e, [key.toUpperCase()]: true }));
      setAdding(false);
    }
  };

  // Benennt ein Projekt um und schließt den Dialog
  const renameProject = async (p: DbProjectDto, name: string) => {
    if (await run(() => api.renameDbProject(p.key, name), `Projekt ${p.key} umbenannt.`)) setEditing(null);
  };

  // Öffnet den Board-Dialog eines Projekts mit leerem Namen
  const openAddBoard = (p: DbProjectDto) => {
    setError(null);
    setInfo(null);
    setBoardName('');
    setAddingBoard(p.key);
  };

  // Schließt den Board-Dialog ohne Anlegen
  const closeAddBoard = () => {
    setAddingBoard(null);
    setError(null);
  };

  // Legt ein Board im Projekt an, schließt den Dialog und klappt das Projekt auf
  const createBoard = async (p: DbProjectDto) => {
    const name = boardName.trim();
    if (!name) return;
    if (await run(() => api.createDbBoard(p.key, name), `Board „${name}“ angelegt.`)) {
      setExpanded((e) => ({ ...e, [p.key]: true }));
      setAddingBoard(null);
    }
  };

  // Speichert Name und/oder eigenen Repo-Pfad eines Boards (Pfad lokal auf diesem Rechner) und schließt den Dialog
  const saveBoard = async (b: DbBoardDto, patch: DbBoardPatch) => {
    const what = patch.name !== undefined && patch.repoPath !== undefined ? 'geändert' : patch.name !== undefined ? 'umbenannt' : 'Repo-Pfad gespeichert';
    if (await run(() => api.patchDbBoard(b.id, patch), `Board ${b.id} ${what}.`)) setEditing(null);
  };

  // Löscht das Board nach Bestätigung (der Server setzt ein gelöschtes aktives Board zurück)
  const removeBoard = async () => {
    if (!deleting) return;
    if (await run(() => api.deleteDbBoard(deleting.id, deleting.all), `Board ${deleting.id} gelöscht.`)) {
      dialogRef.current?.focus(); // die Abfrage verschwindet gleich; Esc soll weiter greifen
      setDeleting(null);
    }
  };

  // Bricht die Löschabfrage ab
  const cancelDelete = () => {
    setDeleting(null);
    setError(null);
    dialogRef.current?.focus();
  };

  const renderBoard = (b: DbBoardDto) => (
    <div key={b.id} className="db-admin-board">
      <div className="db-admin-line">
        <span className="db-admin-board-name">{b.name}</span>
        <button type="button" className="icon-btn" aria-label={`Board ${b.id} bearbeiten`} disabled={busy} onClick={() => openEdit({ kind: 'board', id: b.id })}>
          <CommentIcon name="edit" />
        </button>
        <span className="muted db-admin-board-meta">
          Board {b.id} · {b.openTickets} offen · {b.ownerName}
        </span>
        <button
          type="button"
          className="icon-btn danger-icon"
          aria-label={`Board ${b.id} löschen`}
          disabled={busy}
          onClick={() => {
            setError(null);
            setInfo(null);
            setDeleting({ id: b.id, all: false });
          }}
        >
          <CommentIcon name="trash" />
        </button>
      </div>
      {b.repoPath && (
        <div className="muted db-admin-path-line" title="Mein Repo-Pfad">
          <span className="mono">{b.repoPath}</span>
        </div>
      )}
      {deleting?.id === b.id && (
        <div role="alertdialog" aria-label={`Board ${b.id} löschen`} className="db-admin-confirm">
          <span>Board „{b.name}“ wirklich löschen?</span>
          <label className="option-row">
            <input type="checkbox" checked={deleting.all} disabled={busy} onChange={(e) => setDeleting({ id: b.id, all: e.target.checked })} />
            <span>Auch offene Tickets endgültig löschen</span>
          </label>
          {deleting.all && (
            <p className="muted">
              {b.openTickets === 1
                ? 'Dabei wird 1 offenes Ticket endgültig gelöscht.'
                : `Dabei werden ${b.openTickets} offene Tickets endgültig gelöscht.`}
            </p>
          )}
          <div className="db-admin-row">
            <button type="button" className="btn danger" disabled={busy} onClick={() => void removeBoard()}>
              Board endgültig löschen
            </button>
            <button type="button" className="btn" autoFocus disabled={busy} onClick={cancelDelete}>
              Abbrechen
            </button>
          </div>
        </div>
      )}
    </div>
  );

  const renderProject = (p: DbProjectDto) => {
    const projectBoards = boards.filter((b) => b.projectKey === p.key);
    const open = isExpanded(p);
    const bodyId = `db-admin-project-${p.key}`;
    return (
      <div key={p.key} className={`db-admin-project${open ? ' is-open' : ''}`}>
        <div className="db-admin-project-head">
          <button type="button" className="db-admin-toggle" aria-expanded={open} aria-controls={bodyId} onClick={() => toggle(p)}>
            <span className="db-admin-chevron" aria-hidden="true" />
            <span className="mono">{p.key}</span>
            <span className="db-admin-project-name">{p.name}</span>
          </button>
          <button type="button" className="icon-btn db-admin-edit" aria-label={`Projekt ${p.key} bearbeiten`} disabled={busy} onClick={() => openEdit({ kind: 'project', key: p.key })}>
            <CommentIcon name="edit" />
          </button>
          <span className="muted db-admin-summary">{projectSummary(projectBoards)}</span>
          <button type="button" className="icon-btn db-admin-edit" aria-label={`Board in ${p.key} hinzufügen`} disabled={busy} onClick={() => openAddBoard(p)}>
            <CommentIcon name="plus" />
          </button>
        </div>
        {open && (
          <div id={bodyId} className="db-admin-project-body">
            {projectBoards.length === 0 && <p className="muted">Noch keine Boards in diesem Projekt.</p>}
            {projectBoards.map(renderBoard)}
          </div>
        )}
      </div>
    );
  };

  return (
    <>
      <DbModal
        labelledBy={titleId}
        dialogRef={dialogRef}
        className="panel db-modal wide"
        onEscape={() => {
          if (deleting) cancelDelete();
          else onClose();
        }}
      >
        <div className="panel-head">
          <h2 id={titleId}>Verwaltung</h2>
          <button type="button" className="icon-btn" aria-label="Verwaltung schließen" autoFocus onClick={onClose}>
            ×
          </button>
        </div>
        {info && (
          <p role="status" className="notice-ok">
            {info}
          </p>
        )}
        {error && !dialogOpen && (
          <p role="alert" className="field-error">
            {error}
          </p>
        )}

        <section aria-label="Projekte" className="db-admin-section">
          <div className="db-admin-head">
            <h3>Projekte</h3>
            <span className="db-admin-rule" aria-hidden="true" />
            <button type="button" className="icon-btn db-admin-add" aria-label="Projekt hinzufügen" disabled={busy} onClick={openAdd}>
              +
            </button>
          </div>
          {projects.length === 0 && <p className="muted">Noch keine Projekte. Mit „+“ legst du das erste an.</p>}
          <div className="db-admin-projects">{projects.map(renderProject)}</div>
        </section>
      </DbModal>

      {adding && (
        <DbModal
          labelledBy={addTitleId}
          className="panel db-modal"
          onEscape={closeAdd}
          onSubmit={(event) => {
            event.preventDefault();
            void createProject();
          }}
        >
          <div className="panel-head">
            <h2 id={addTitleId}>Projekt hinzufügen</h2>
            <button type="button" className="icon-btn" aria-label="Projekt-Dialog schließen" onClick={closeAdd}>
              ×
            </button>
          </div>
          {error && (
            <p role="alert" className="field-error">
              {error}
            </p>
          )}
          <label className="field">
            <span>Projektkey</span>
            <input autoFocus placeholder="z. B. ARC" value={newKey} onChange={(e) => setNewKey(e.target.value)} />
          </label>
          <label className="field">
            <span>Projektname</span>
            <input placeholder="Name" value={newName} onChange={(e) => setNewName(e.target.value)} />
          </label>
          <div className="db-admin-row">
            <button type="submit" className="btn primary" disabled={busy || !newKey.trim() || !newName.trim()}>
              Projekt anlegen
            </button>
            <button type="button" className="btn" disabled={busy} onClick={closeAdd}>
              Abbrechen
            </button>
          </div>
        </DbModal>
      )}

      {addedToProject && (
        <DbModal
          labelledBy={addBoardTitleId}
          className="panel db-modal"
          onEscape={closeAddBoard}
          onSubmit={(event) => {
            event.preventDefault();
            void createBoard(addedToProject);
          }}
        >
          <div className="panel-head">
            <h2 id={addBoardTitleId}>Board hinzufügen</h2>
            <button type="button" className="icon-btn" aria-label="Board-Dialog schließen" onClick={closeAddBoard}>
              ×
            </button>
          </div>
          <p className="muted">
            Neues Board im Projekt <span className="mono">{addedToProject.key}</span> · {addedToProject.name}
          </p>
          {error && (
            <p role="alert" className="field-error">
              {error}
            </p>
          )}
          <label className="field">
            <span>Boardname</span>
            <input autoFocus placeholder="Name des neuen Boards" value={boardName} onChange={(e) => setBoardName(e.target.value)} />
          </label>
          <div className="db-admin-row">
            <button type="submit" className="btn primary" disabled={busy || !boardName.trim()}>
              Board anlegen
            </button>
            <button type="button" className="btn" disabled={busy} onClick={closeAddBoard}>
              Abbrechen
            </button>
          </div>
        </DbModal>
      )}

      {editedProject && (
        <ProjectEditDialog project={editedProject} busy={busy} error={error} onSave={(name) => void renameProject(editedProject, name)} onClose={closeEdit} />
      )}
      {editedBoard && (
        <BoardEditDialog board={editedBoard} busy={busy} error={error} onSave={(patch) => void saveBoard(editedBoard, patch)} onClose={closeEdit} />
      )}
    </>
  );
}
