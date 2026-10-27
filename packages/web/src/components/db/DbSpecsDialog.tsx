import { useId } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { specTopic } from '../../board/specTopic.js';
import { useAsync } from '../../hooks/useAsync.js';
import { ErrorState } from '../ErrorState.js';
import { LoadingState } from '../LoadingState.js';
import { SpecProgressBar } from '../SpecProgressBar.js';
import { DbModal } from './DbModal.js';

// Fortschritt der Specs des Boards als Dialog (Knopf „Specs“ im Board-Kopf); ein Klick auf eine Spec öffnet ihren Abhängigkeitsgraphen.
export function DbSpecsDialog({ boardId, onClose, onOpenSpec }: { boardId: number; onClose: () => void; onOpenSpec: (spec: string) => void }) {
  const api = useApi();
  const titleId = useId();
  const progress = useAsync(() => api.getDbSpecProgress(boardId), [api, boardId]);
  return (
    <DbModal labelledBy={titleId} onEscape={onClose} onBackdrop={onClose} className="panel db-modal db-specs-dialog">
      <div className="panel-head">
        <h2 id={titleId}>Specs</h2>
        <button type="button" className="icon-btn" aria-label="Specs schließen" autoFocus onClick={onClose}>
          ×
        </button>
      </div>
      {progress.state.status === 'loading' && <LoadingState text="Specs werden geladen …" />}
      {progress.state.status === 'error' && <ErrorState message={progress.state.message} onRetry={progress.retry} />}
      {progress.state.status === 'ready' && (progress.state.data.length === 0 ? (
        <p className="muted">Auf diesem Board gehört noch kein Ticket zu einer Spec.</p>
      ) : (
        <ul className="db-spec-list">
          {progress.state.data.map((p) => (
            <li key={p.spec}>
              {/* Beschriftet wird mit dem Thema; den vollen Pfad nennt der Hinweis, gefiltert wird weiter mit dem rohen Wert */}
              <button type="button" className="db-spec-name" title={`${p.spec} – Abhängigkeiten zeigen`} onClick={() => onOpenSpec(p.spec)}>
                {specTopic(p.spec)}
              </button>
              <SpecProgressBar progress={p} />
            </li>
          ))}
        </ul>
      ))}
    </DbModal>
  );
}
