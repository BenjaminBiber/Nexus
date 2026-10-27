import { useId } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { useAsync } from '../../hooks/useAsync.js';
import { DbModal } from './DbModal.js';

// Zeigt ein Protokoll des Dispatchers (Ende der Datei).
export function DispatchLogDialog({ name, onClose }: { name: string; onClose: () => void }) {
  const api = useApi();
  const log = useAsync(() => api.getDispatchLog(name), [api, name]);
  const titleId = useId();
  return (
    <DbModal labelledBy={titleId} onEscape={onClose} className="panel db-modal wide">
      <div className="db-section-head">
        <h2 id={titleId}>Protokoll {name}</h2>
        <button type="button" className="btn" onClick={onClose}>Schließen</button>
      </div>
      {log.state.status === 'loading' && <p className="muted">Wird geladen …</p>}
      {log.state.status === 'error' && <div className="banner" role="alert">{log.state.message}</div>}
      {log.state.status === 'ready' && <pre className="dispatch-log">{log.state.data}</pre>}
    </DbModal>
  );
}
