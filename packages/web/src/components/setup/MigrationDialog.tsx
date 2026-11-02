import { useId, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { BoardMigrationDto, BoardMigrationResultDto, MigrationReason } from '../../api/types.js';
import { DbModal } from '../db/DbModal.js';

const REASONS: Record<MigrationReason, string> = {
  'legacy-plugin': 'altes Plugin eingeschaltet',
  'legacy-rules': 'alte Regeln vorhanden',
};

type Phase = { kind: 'idle' } | { kind: 'busy' } | { kind: 'done'; result: BoardMigrationResultDto } | { kind: 'failed'; message: string };

// Bietet beim Wechsel auf ein Board an, dessen Repo vom alten AI Harness auf Nexus umzustellen (wie `nexus init`).
export function MigrationDialog({ boardId, check, pluginInstalled, onLater, onNever, onClose }: {
  boardId: number; check: BoardMigrationDto; pluginInstalled: boolean; onLater: () => void; onNever: () => void; onClose: () => void;
}) {
  const api = useApi();
  const titleId = useId();
  const [phase, setPhase] = useState<Phase>({ kind: 'idle' });
  const busy = phase.kind === 'busy';
  const migrate = () => {
    setPhase({ kind: 'busy' });
    api.migrateBoard(boardId).then(
      (result) => setPhase({ kind: 'done', result }),
      (error: unknown) => setPhase({ kind: 'failed', message: error instanceof Error ? error.message : String(error) }),
    );
  };
  return (
    <DbModal role="alertdialog" labelledBy={titleId} onEscape={() => { if (!busy) (phase.kind === 'done' ? onClose : onLater)(); }} backdropClassName="db-dialog-backdrop" className="panel db-dialog">
      <strong id={titleId}>Projekt auf Nexus umstellen</strong>
      <p className="db-dialog-hint">
        Das Projekt <code>{check.repoPath}</code> ist noch für den alten AI Harness eingerichtet.
      </p>
      <ul>
        {check.reasons.map((reason) => (
          <li key={reason}>{REASONS[reason]}</li>
        ))}
      </ul>
      {busy && <p role="status">Projekt wird umgestellt …</p>}
      {phase.kind === 'done' && (
        <div role="status">
          <p>Umgestellt.</p>
          {phase.result.written.length + phase.result.removed.length > 0 && (
            <ul>
              {phase.result.written.map((path) => <li key={`w-${path}`}>{path}</li>)}
              {phase.result.removed.map((path) => <li key={`r-${path}`}><s>{path}</s></li>)}
            </ul>
          )}
          {phase.result.kept.length > 0 && (
            <p className="db-dialog-hint">
              Von Hand geändert, bitte selbst prüfen und löschen: {phase.result.kept.join(', ')}
            </p>
          )}
          {!pluginInstalled && <p className="db-dialog-hint">Claude nutzt die neuen Regeln erst mit installiertem Plugin.</p>}
        </div>
      )}
      {phase.kind === 'failed' && (
        <p role="alert" className="field-error">
          {phase.message}
        </p>
      )}
      <div className="actions">
        {phase.kind === 'done' ? (
          <button type="button" className="btn primary" onClick={onClose} autoFocus>
            Schließen
          </button>
        ) : (
          <>
            <button type="button" className="btn primary" disabled={busy} onClick={migrate} autoFocus>
              {phase.kind === 'failed' ? 'Erneut versuchen' : 'Migrieren'}
            </button>
            <button type="button" className="btn" disabled={busy} onClick={onLater}>
              Nicht jetzt
            </button>
            <button type="button" className="btn" disabled={busy} onClick={onNever}>
              Für dieses Board nicht mehr fragen
            </button>
          </>
        )}
      </div>
    </DbModal>
  );
}
