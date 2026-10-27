import { Link } from 'react-router-dom';

// Hinweis, wenn die Board-Datenbank fehlt, nicht erreichbar ist oder ein falsches Schema hat (Server 503).
export function DbUnavailable({ pid, message, onRetry }: { pid: string; message: string; onRetry: () => void }) {
  return (
    <div className="panel db-unavailable" role="alert">
      <h2>Board-Datenbank nicht verfügbar</h2>
      <p>{message}</p>
      <p>
        Verbindung prüfen unter <Link to={`/p/${pid}/settings#board-db`}>Einstellungen → Board-Datenbank</Link>.
      </p>
      <div className="actions">
        <button type="button" className="btn" onClick={onRetry}>
          Erneut versuchen
        </button>
      </div>
    </div>
  );
}
