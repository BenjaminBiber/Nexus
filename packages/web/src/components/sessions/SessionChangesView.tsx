import { useId, useState } from 'react';
import type { AsyncState } from '../../hooks/useAsync.js';
import type { FileChange, FileChangeStatus, SessionChanges } from '../../api/sessionDetailTypes.js';
import { formatNumber } from '../../sessions/format.js';
import { EmptyState } from '../EmptyState.js';
import { ErrorState } from '../ErrorState.js';
import { LoadingState } from '../LoadingState.js';
import { FileDiffView } from './FileDiffView.js';

const STATUS: Record<FileChangeStatus, { letter: string; label: string }> = {
  added: { letter: 'A', label: 'neu' },
  modified: { letter: 'M', label: 'geändert' },
  deleted: { letter: 'D', label: 'gelöscht' },
  renamed: { letter: 'R', label: 'umbenannt' },
};

// Verhältnis von + zu − als fünf Kästchen (wie man es von Code-Reviews kennt)
function Ratio({ added, removed }: { added: number; removed: number }) {
  const total = added + removed;
  const plus = total === 0 ? 0 : Math.round((added / total) * 5);
  const minus = total === 0 ? 0 : Math.min(5 - plus, Math.max(removed > 0 ? 1 : 0, Math.round((removed / total) * 5)));
  return (
    <span className="fc-ratio" aria-hidden="true">
      {Array.from({ length: 5 }, (_, i) => (
        <span key={i} className={i < plus ? 'add' : i < plus + minus ? 'remove' : ''} />
      ))}
    </span>
  );
}

// Eine Datei als aufklappbare Zeile: Status, Pfad (Ordner gedämpft), +/−; aufgeklappt der Diff
function FileRow({ pid, sid, file, version }: { pid: string; sid: string; file: FileChange; version: number }) {
  const [open, setOpen] = useState(false);
  const slash = file.path.lastIndexOf('/');
  const dir = slash >= 0 ? file.path.slice(0, slash + 1) : '';
  const base = file.path.slice(slash + 1);
  const status = STATUS[file.status];
  const bodyId = useId();
  return (
    <li className={`fc-file${open ? ' open' : ''}`}>
      <button type="button" className="fc-head" aria-expanded={open} aria-controls={bodyId} onClick={() => setOpen((o) => !o)}>
        <span className="material-icons tx-chevron" aria-hidden="true">expand_more</span>
        <span className={`fc-status ${file.status}`} title={status.label}>
          <span aria-hidden="true">{status.letter}</span>
          <span className="sr-only">{status.label}</span>
        </span>
        <span className="fc-path mono">
          {dir && <span className="fc-dir">{dir}</span>}
          <span className="fc-base">{base}</span>
        </span>
        <span className="fc-counts mono">
          <span className="fc-add">+{formatNumber(file.added)}</span> <span className="fc-remove">−{formatNumber(file.removed)}</span>
        </span>
        <Ratio added={file.added} removed={file.removed} />
      </button>
      {open && (
        <div id={bodyId} className="fc-body">
          {/* Neu laden, wenn sich die Datei laut Liste geändert hat */}
          <FileDiffView pid={pid} sid={sid} path={file.path} version={`${version}:${file.status}:${file.added}:${file.removed}`} />
        </div>
      )}
    </li>
  );
}

// Herkunft der Liste in einem Satz
function OriginNote({ changes }: { changes: SessionChanges }) {
  if (changes.origin === 'git') {
    return (
      <p className="fc-origin">
        <span className="material-icons" aria-hidden="true">account_tree</span>
        <span>
          Aus Git{changes.base ? <> gegen <span className="mono">{changes.base}</span></> : null} – committete und offene Änderungen im Arbeitsverzeichnis.
        </span>
      </p>
    );
  }
  return (
    <p className="fc-origin">
      <span className="material-icons" aria-hidden="true">history_edu</span>
      <span>Aus dem Transcript zusammengesetzt (Edit, MultiEdit, Write).</span>
    </p>
  );
}

// Tab Änderungen: Herkunft, Dateiliste mit Status und +/−, Diff je Datei beim Aufklappen.
export function SessionChangesView({ pid, sid, changes, version, onRetry }: { pid: string; sid: string; changes: AsyncState<SessionChanges>; version: number; onRetry: () => void }) {
  if (changes.status === 'loading') return <LoadingState text="Änderungen werden geladen …" />;
  if (changes.status === 'error') return <ErrorState message="Änderungen konnten nicht geladen werden" detail={changes.message} onRetry={onRetry} />;
  const data = changes.data;
  const added = data.files.reduce((sum, f) => sum + f.added, 0);
  const removed = data.files.reduce((sum, f) => sum + f.removed, 0);
  return (
    <div className="fc">
      {data.origin !== 'none' && <OriginNote changes={data} />}
      {data.note && (
        <p className="fc-note" role="note">
          <span className="material-icons" aria-hidden="true">info</span>
          <span>{data.note}</span>
        </p>
      )}
      {data.files.length === 0 ? (
        <EmptyState title="Keine Änderungen gefunden">
          {data.origin === 'none' ? 'Weder Git noch das Transcript zeigen geänderte Dateien.' : 'Diese Session hat noch keine Dateien geändert.'}
        </EmptyState>
      ) : (
        <>
          <p className="fc-summary">
            <strong>{data.files.length === 1 ? '1 Datei' : `${formatNumber(data.files.length)} Dateien`}</strong>
            <span className="fc-add mono">+{formatNumber(added)}</span>
            <span className="fc-remove mono">−{formatNumber(removed)}</span>
          </p>
          <ul className="fc-list" aria-label="Geänderte Dateien">
            {data.files.map((file) => (
              <FileRow key={file.path} pid={pid} sid={sid} file={file} version={version} />
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
