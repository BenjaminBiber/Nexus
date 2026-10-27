import { Link, useLocation } from 'react-router-dom';
import { docHref } from '../board/docsTree.js';
import { useDocTicketJobs, type DocTicketJob } from '../hooks/docTicketJobs.js';

// Dateiname eines Dokuments ohne Ordner
const fileName = (path: string) => path.slice(path.lastIndexOf('/') + 1);

// Was der Lauf gerade macht; null, wenn es nichts zu melden gibt (z. B. Tickets schon angelegt)
function describe(job: DocTicketJob): string | null {
  if (job.running) return 'Claude erzeugt Ticket-Entwürfe …';
  if (job.creating) return 'Tickets werden angelegt …';
  if (job.created) return null;
  if (job.result) return `${job.result.drafts.length} ${job.result.drafts.length === 1 ? 'Entwurf' : 'Entwürfe'} bereit`;
  if (job.error) return `Fehlgeschlagen: ${job.error}`;
  return null;
}

// Hinweis unten rechts auf Ticket-Erzeugungen zu Dokumenten, die gerade nicht offen sind: läuft noch, Entwürfe bereit oder Fehler.
// „Öffnen“ führt zum Dokument, dort stehen Ladeanzeige, Entwurfs-Dialog oder Fehler.
export function DocTicketJobsIndicator() {
  const { jobs, list } = useDocTicketJobs();
  const location = useLocation();
  const board = new URLSearchParams(location.search).get('board');
  const here = `${location.pathname}${board ? `?board=${board}` : ''}`;
  const shown = list.flatMap((job) => {
    const text = describe(job);
    const href = docHref(job.pid, job);
    return text && href !== here ? [{ job, text, href }] : [];
  });
  if (shown.length === 0) return null;
  return (
    <ul className="doc-jobs" aria-label="Ticket-Erzeugung">
      {shown.map(({ job, text, href }) => (
        <li key={job.key} className={`doc-job${job.error && !job.result ? ' is-error' : ''}${job.result ? ' is-ready' : ''}`}>
          <span className="material-icons doc-job-icon" aria-hidden="true">
            {job.running || job.creating ? 'autorenew' : job.result ? 'task_alt' : 'error_outline'}
          </span>
          <div className="doc-job-text">
            <strong title={job.path}>{fileName(job.path)}</strong>
            <span>{text}</span>
          </div>
          <Link className="btn small primary" to={href}>{job.result ? 'Ansehen' : 'Öffnen'}</Link>
          {job.running && (
            <button type="button" className="doc-job-close" aria-label={`Erzeugung zu ${fileName(job.path)} abbrechen`} title="Abbrechen" onClick={() => void jobs.cancel(job)}>
              <span className="material-icons" aria-hidden="true">stop_circle</span>
            </button>
          )}
          {!job.running && !job.creating && (
            <button type="button" className="doc-job-close" aria-label={`Hinweis zu ${fileName(job.path)} schließen`} title="Verwerfen" onClick={() => jobs.reset(job.key)}>
              <span className="material-icons" aria-hidden="true">close</span>
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}
