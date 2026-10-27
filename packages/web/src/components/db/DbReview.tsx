import { useEffect, useState, type KeyboardEvent } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { DbTicketDetailDto } from '../../api/dbTypes.js';
import type { DispatchReviewDto, DispatchReviewFileDto, DispatchReviewPatchDto, DispatchTestRunDto } from '../../api/dispatchTypes.js';
import { DbDrawer } from './DbDrawer.js';
import { reviewTree, type ReviewNode } from './reviewTree.js';

// Wie lange das Ergebnis stehen bleibt (wie in DbMergeRetry)
const OUTCOME_MS = 4000;

// Bis zu so vielen Dateien sind die Ordner des Baums anfangs offen
const OPEN_DIRS_UP_TO = 5;

// Bezeichnung des Git-Status einer Datei
const STATUS_LABELS: Record<string, string> = { A: 'neu', M: 'geändert', D: 'gelöscht', R: 'umbenannt', C: 'kopiert', T: 'Typ geändert' };

// Erscheint der Review-Block? Blockiert mit Grund „Human Interaction“, Branch gesetzt, eigenes Board
export function showsReview(ticket: DbTicketDetailDto, mine: boolean): boolean {
  const reason = (ticket.blockedReason ?? '').trim();
  const human = reason.startsWith('Human Interaction:') || reason === 'Human Interaction';
  return mine && ticket.status === 'blocked' && human && Boolean(ticket.branch);
}

// Review vor dem Merge im Ticket-Panel: Diff des Ticket-Branches gegen das Ziel (Dateien aufklappbar), Testläufe des Agenten
// und die Aktionen „Übernehmen“ (Abnehmen samt Merge) und „Zurück an den Agenten“ (Rückmeldung Pflicht, Strg+Enter sendet).
export function DbReview({ ticket, onDone }: { ticket: DbTicketDetailDto; onDone: () => void }) {
  const api = useApi();
  const [review, setReview] = useState<DispatchReviewDto | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<string | null>(null);
  const [returning, setReturning] = useState(false);
  const [comment, setComment] = useState('');

  // Review laden, sobald sich Ticket oder Branch ändern
  useEffect(() => {
    let alive = true;
    setReview(null);
    setLoadError(null);
    api.getDispatchReview(ticket.id)
      .then((r) => { if (alive) setReview(r); })
      .catch((e: Error) => { if (alive) setLoadError(e.message); });
    return () => { alive = false; };
  }, [api, ticket.id, ticket.branch, ticket.rev]);
  // Ergebnis nach Ablauf der Zeit ausblenden
  useEffect(() => {
    if (outcome === null) return undefined;
    const timer = setTimeout(() => setOutcome(null), OUTCOME_MS);
    return () => clearTimeout(timer);
  }, [outcome]);

  // Führt eine Aktion aus und zeigt ihr Ergebnis; danach lädt das Panel neu
  const act = async (work: () => Promise<{ result?: { text: string } }>) => {
    setBusy(true);
    setError(null);
    setOutcome(null);
    try {
      const answer = await work();
      setOutcome(answer.result?.text ?? null);
      setReturning(false);
      setComment('');
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const accept = () => act(() => api.acceptDispatchTicket(ticket.id, ticket.boardId));
  const sendBack = () => (comment.trim() ? act(() => api.returnDispatchTicket(ticket.id, comment.trim(), ticket.boardId)) : Promise.resolve());
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && e.ctrlKey) {
      e.preventDefault();
      void sendBack();
    }
  };

  return (
    <DbDrawer id="review" title="Freigabe" className="db-review" meta={reviewMeta(review)}>
      <p className="db-review-head">{`Wartet auf Freigabe: ${ticket.branch}${review?.target ? ` → ${review.target}` : ''}`}</p>
      {loadError && <div className="banner" role="alert">{loadError}</div>}
      {review && !review.available && <p className="db-review-note">{review.reason}</p>}
      {review?.available && <ReviewFiles ticketId={ticket.id} files={review.files} />}
      {review && <ReviewTests tests={review.tests} runLog={review.runLog} />}
      {error && <div className="banner" role="alert">{error}</div>}
      {outcome && (
        <p role="status">
          {outcome}{' '}
          <button type="button" className="btn" onClick={() => setOutcome(null)}>Ausblenden</button>
        </p>
      )}
      {returning && (
        <label className="field wide">
          <span>Rückmeldung an den Agenten</span>
          <textarea
            className="plain-text" rows={4} value={comment} autoFocus disabled={busy}
            placeholder="Was soll der Agent ändern? (Strg+Enter sendet)"
            onChange={(e) => setComment(e.target.value)} onKeyDown={onKey}
          />
        </label>
      )}
      <div className="actions">
        <button type="button" className="btn primary" disabled={busy} onClick={() => void accept()}>Übernehmen</button>
        {returning ? (
          <>
            <button type="button" className="btn" disabled={busy || !comment.trim()} onClick={() => void sendBack()}>Zurück an den Agenten</button>
            <button type="button" className="btn" disabled={busy} onClick={() => { setReturning(false); setComment(''); }}>Abbrechen</button>
          </>
        ) : (
          <button type="button" className="btn" disabled={busy} onClick={() => setReturning(true)}>Zurück an den Agenten …</button>
        )}
      </div>
    </DbDrawer>
  );
}

// Kurzinfo im Kopf: Dateien und Testergebnis
function reviewMeta(review: DispatchReviewDto | null): string | undefined {
  if (!review) return undefined;
  const parts = review.available ? [`${review.files.length} ${review.files.length === 1 ? 'Datei' : 'Dateien'}`] : [];
  const red = review.tests.filter((t) => t.ok === false).length;
  if (review.tests.length) parts.push(red ? `${red} Test${red === 1 ? '' : 's'} rot` : 'Tests grün');
  return parts.join(' · ') || undefined;
}

// Dateibaum; der Patch einer Datei wird beim ersten Aufklappen geladen
function ReviewFiles({ ticketId, files }: { ticketId: string; files: DispatchReviewFileDto[] }) {
  if (!files.length) return <p className="db-review-note">Keine Änderungen gegenüber dem Ziel.</p>;
  const add = files.reduce((n, f) => n + f.additions, 0);
  const del = files.reduce((n, f) => n + f.deletions, 0);
  return (
    <div className="db-review-files">
      <p className="db-review-note">{`${files.length} ${files.length === 1 ? 'Datei' : 'Dateien'} geändert, +${add} −${del}`}</p>
      <ReviewNodes ticketId={ticketId} nodes={reviewTree(files)} open={files.length <= OPEN_DIRS_UP_TO} />
    </div>
  );
}

// Ordner und Dateien einer Ebene; Ordner zeigen Anzahl und Zeilen ihrer Dateien
function ReviewNodes({ ticketId, nodes, open }: { ticketId: string; nodes: ReviewNode[]; open: boolean }) {
  return (
    <div className="db-review-level">
      {nodes.map((node) => node.kind === 'file'
        ? <ReviewFile key={node.file.path} ticketId={ticketId} file={node.file} name={node.name} />
        : (
          <details key={node.path} className="db-review-dir" open={open}>
            <summary title={node.path}>
              <span className="db-review-folder" aria-hidden="true" />
              <span className="db-review-path">{node.name}</span>
              <span className="db-review-count">{`${node.files} ${node.files === 1 ? 'Datei' : 'Dateien'} · +${node.additions} −${node.deletions}`}</span>
            </summary>
            <ReviewNodes ticketId={ticketId} nodes={node.children} open={open} />
          </details>
        ))}
    </div>
  );
}

// Eine Datei: Kopf mit Status und Zeilenzahlen, aufgeklappt der Patch
function ReviewFile({ ticketId, file, name }: { ticketId: string; file: DispatchReviewFileDto; name: string }) {
  const api = useApi();
  const [patch, setPatch] = useState<DispatchReviewPatchDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [requested, setRequested] = useState(false);
  // Lädt den Patch genau einmal
  const onToggle = (open: boolean) => {
    if (!open || requested) return;
    setRequested(true);
    api.getDispatchReviewFile(ticketId, file.path).then(setPatch).catch((e: Error) => setError(e.message));
  };
  return (
    <details className="db-review-file" onToggle={(e) => onToggle((e.currentTarget as HTMLDetailsElement).open)}>
      <summary>
        <span className={`db-review-status s-${file.status}`}>{STATUS_LABELS[file.status] ?? file.status}</span>
        <span className="db-review-path" title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}>{file.oldPath ? `${file.oldPath.split('/').pop()} → ${name}` : name}</span>
        <span className="db-review-count">{file.binary ? 'binär' : `+${file.additions} −${file.deletions}`}</span>
      </summary>
      {error && <div className="banner" role="alert">{error}</div>}
      {!patch && !error && <p className="db-review-note">Lädt …</p>}
      {patch && <PatchView patch={patch} />}
    </details>
  );
}

// Patch mit Zeilen-Klassen add, del und hunk
function PatchView({ patch }: { patch: DispatchReviewPatchDto }) {
  const lines = patch.patch.replace(/\n$/, '').split('\n');
  return (
    <>
      <pre className="db-review-patch">
        {lines.map((line, i) => (
          <span key={i} className={lineClass(line)}>{`${line}\n`}</span>
        ))}
      </pre>
      {patch.truncated && <p className="db-review-note">Der Patch ist sehr groß und wurde gekürzt.</p>}
    </>
  );
}

// Klasse einer Patch-Zeile
function lineClass(line: string): string {
  if (line.startsWith('@@')) return 'hunk';
  if (line.startsWith('+++') || line.startsWith('---')) return 'meta';
  if (line.startsWith('+')) return 'add';
  if (line.startsWith('-')) return 'del';
  return '';
}

// Testläufe des Agenten mit Ergebnis und aufklappbarer Ausgabe
function ReviewTests({ tests, runLog }: { tests: DispatchTestRunDto[]; runLog: string | null }) {
  // Wechselt in den Reiter „Agent“ des Panels (dort stehen die Lauf-Protokolle)
  const openLog = () => document.getElementById('db-tab-agent')?.click();
  return (
    <div className="db-review-tests">
      <h4>Testläufe des Agenten</h4>
      {!tests.length && <p className="db-review-note">Keine Testläufe erkannt (Tests, Build oder Lint über Bash oder PowerShell).</p>}
      {tests.map((t, i) => (
        <details key={i} className="db-review-test">
          <summary>
            <span className={`db-review-badge ${t.ok === true ? 'ok' : t.ok === false ? 'fail' : 'none'}`}>
              {t.ok === true ? 'grün' : t.ok === false ? 'rot' : 'ohne Ergebnis'}
            </span>
            <code>{t.command}</code>
          </summary>
          <pre className="db-review-output">{t.output || '(keine Ausgabe)'}</pre>
        </details>
      ))}
      {runLog && <button type="button" className="btn db-review-log" onClick={openLog}>{`Protokoll (${runLog})`}</button>}
    </div>
  );
}
