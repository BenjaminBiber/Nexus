import { useApi } from '../../api/ApiContext.js';
import type { DiffHunk } from '../../api/sessionDetailTypes.js';
import { useAsync } from '../../hooks/useAsync.js';

// Startzeilen aus „@@ -12,7 +12,9 @@ …“; ohne passenden Kopf keine Zeilennummern
function hunkStart(header: string): { old: number; new: number } | null {
  const match = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(header);
  return match ? { old: Number(match[1]), new: Number(match[2]) } : null;
}

const SIGN = { add: '+', remove: '−', context: ' ' } as const;
const SPOKEN = { add: 'hinzugefügt', remove: 'entfernt', context: 'unverändert' } as const;

// Ein Hunk mit Kopfzeile und nummerierten Zeilen (alt | neu); Klassen wie LineDiffView (diff-line diff-add|remove|context)
function Hunk({ hunk }: { hunk: DiffHunk }) {
  const start = hunkStart(hunk.header);
  let oldLine = start?.old ?? 0;
  let newLine = start?.new ?? 0;
  return (
    <div className="fd-hunk">
      <div className="fd-hunk-head mono">{hunk.header}</div>
      {hunk.lines.map((line, index) => {
        const left = line.kind !== 'add' && start ? oldLine++ : null;
        const right = line.kind !== 'remove' && start ? newLine++ : null;
        return (
          <div key={index} className={`diff-line diff-${line.kind}`} aria-label={`${SPOKEN[line.kind]}: ${line.text}`}>
            <span className="fd-num" aria-hidden="true">{left ?? ''}</span>
            <span className="fd-num" aria-hidden="true">{right ?? ''}</span>
            <span className="diff-sign" aria-hidden="true">{SIGN[line.kind]}</span>
            <span className="diff-text">{line.text}</span>
          </div>
        );
      })}
    </div>
  );
}

// Diff einer geänderten Datei der Session; lädt beim Aufklappen und neu, wenn sich `version` ändert.
export function FileDiffView({ pid, sid, path, version }: { pid: string; sid: string; path: string; version: string }) {
  const api = useApi();
  const diff = useAsync(() => api.getSessionFileDiff(pid, sid, path), [api, pid, sid, path, version]);
  if (diff.state.status === 'loading') return <p className="muted fd-state" role="status">Diff wird geladen …</p>;
  if (diff.state.status === 'error') {
    return (
      <div className="fd-state" role="alert">
        <span>{diff.state.message}</span>{' '}
        <button type="button" className="btn small" onClick={diff.retry}>
          Erneut versuchen
        </button>
      </div>
    );
  }
  const { hunks, truncated } = diff.state.data;
  return (
    <div className="fd" aria-label={`Diff ${path}`}>
      {hunks.length === 0 ? (
        <p className="muted fd-state">Kein Textunterschied (z. B. Binärdatei oder nur Rechte geändert).</p>
      ) : (
        <div className="fd-scroll">
          {hunks.map((hunk, index) => (
            <Hunk key={index} hunk={hunk} />
          ))}
        </div>
      )}
      {truncated && <p className="muted fd-state">Diff gekürzt – die Datei hat mehr Änderungen, als hier angezeigt werden.</p>}
    </div>
  );
}
