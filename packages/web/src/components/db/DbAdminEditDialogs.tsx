import { useId, useState } from 'react';
import type { DbBoardDto, DbBoardPatch, DbProjectDto } from '../../api/dbTypes.js';
import { CommentIcon } from '../CommentIcon.js';
import { DbModal } from './DbModal.js';
import { usePlatform } from '../../platform/PlatformContext.js';

// Dialog „Projekt bearbeiten“: Name ändern
export function ProjectEditDialog({ project, busy, error, onSave, onClose }: {
  project: DbProjectDto;
  busy: boolean;
  error: string | null;
  onSave: (name: string) => void;
  onClose: () => void;
}) {
  const titleId = useId();
  const [name, setName] = useState(project.name);
  const trimmed = name.trim();
  return (
    <DbModal
      labelledBy={titleId}
      className="panel db-modal"
      onEscape={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        if (trimmed && trimmed !== project.name) onSave(trimmed);
        else if (trimmed) onClose();
      }}
    >
      <div className="panel-head">
        <h2 id={titleId}>Projekt {project.key} bearbeiten</h2>
        <button type="button" className="icon-btn" aria-label="Bearbeiten schließen" onClick={onClose}>
          ×
        </button>
      </div>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
      <label className="field">
        <span>Projektname</span>
        <input autoFocus value={name} onChange={(e) => setName(e.target.value)} />
      </label>
      <div className="db-admin-row">
        <button type="submit" className="btn primary" disabled={busy || !trimmed}>
          Speichern
        </button>
        <button type="button" className="btn" disabled={busy} onClick={onClose}>
          Abbrechen
        </button>
      </div>
    </DbModal>
  );
}

// Dialog „Board bearbeiten“: Name und „Mein Repo-Pfad“ (lokal auf diesem Rechner). Der Ordner wird über den Ordnerdialog
// dieses Rechners gewählt (ein Browser-Dateifeld gäbe nur den Ordnernamen, nicht den Pfad heraus); der Pfad bleibt auch tippbar.
export function BoardEditDialog({ board, busy, error, onSave, onClose }: {
  board: DbBoardDto;
  busy: boolean;
  error: string | null;
  onSave: (patch: DbBoardPatch) => void;
  onClose: () => void;
}) {
  const platform = usePlatform();
  const titleId = useId();
  const [name, setName] = useState(board.name);
  const [repoPath, setRepoPath] = useState(board.repoPath ?? '');
  const [picking, setPicking] = useState(false);
  const [pickError, setPickError] = useState<string | null>(null);
  const trimmedName = name.trim();
  const trimmedPath = repoPath.trim();

  // Öffnet den Ordnerdialog, beginnend beim bisherigen Pfad; Abbruch ändert nichts
  const pick = async () => {
    setPicking(true);
    setPickError(null);
    try {
      const path = await platform.pickFolder(trimmedPath || undefined);
      if (path) setRepoPath(path);
    } catch (e) {
      setPickError((e as Error).message);
    } finally {
      setPicking(false);
    }
  };

  // Nur Geändertes senden; ein aus der gemeinsamen DB übernommener Pfad (Altbestand) wird beim Speichern lokal abgelegt
  const submit = () => {
    if (!trimmedName) return;
    const patch: DbBoardPatch = {};
    if (trimmedName !== board.name) patch.name = trimmedName;
    if (board.repoSource === 'legacy' || trimmedPath !== (board.repoPath ?? '')) patch.repoPath = trimmedPath || null;
    if (patch.name === undefined && patch.repoPath === undefined) onClose();
    else onSave(patch);
  };

  return (
    <DbModal
      labelledBy={titleId}
      className="panel db-modal"
      onEscape={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <div className="panel-head">
        <h2 id={titleId}>Board {board.id} bearbeiten</h2>
        <button type="button" className="icon-btn" aria-label="Bearbeiten schließen" onClick={onClose}>
          ×
        </button>
      </div>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
      <label className="field">
        <span>Boardname</span>
        <input autoFocus value={name} onChange={(e) => setName(e.target.value)} />
      </label>
      <div className="field">
        <label htmlFor={`${titleId}-repo`}>Mein Repo-Pfad</label>
        <div className="db-admin-path">
          <input
            id={`${titleId}-repo`}
            placeholder="Mein Ordner des Repos auf diesem Rechner"
            value={repoPath}
            onChange={(e) => setRepoPath(e.target.value)}
          />
          <button
            type="button"
            className="icon-btn"
            aria-label="Ordner wählen"
            title={picking ? 'Ordnerdialog ist offen' : 'Ordner wählen'}
            disabled={busy || picking}
            onClick={() => void pick()}
          >
            <CommentIcon name="folder" />
          </button>
        </div>
        <span className="muted">Gilt nur für dich auf diesem Rechner (~/.nexus/board-repos.yaml). Leer lassen heißt „kein Pfad“.</span>
        {pickError && (
          <span role="alert" className="field-error">
            {pickError}
          </span>
        )}
        {board.repoSource === 'legacy' && (
          <span className="muted">Aus der gemeinsamen DB übernommen (Altbestand) – Speichern legt ihn als deinen lokalen Pfad ab.</span>
        )}
      </div>
      <div className="db-admin-row">
        <button type="submit" className="btn primary" disabled={busy || picking || !trimmedName}>
          Speichern
        </button>
        <button type="button" className="btn" disabled={busy} onClick={onClose}>
          Abbrechen
        </button>
      </div>
    </DbModal>
  );
}
