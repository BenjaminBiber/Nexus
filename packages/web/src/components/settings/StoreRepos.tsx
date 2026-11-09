import { useState, type FormEvent } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { StoreRepoDto } from '../../api/types.js';
import { useAsync } from '../../hooks/useAsync.js';

// Store-Repos des Nutzers: Liste, Hinzufügen per URL (Prüfung durch den Server) und Entfernen nach Rückfrage.
// Ohne eigene Hülle: steht im Dialog „Theme-Stores“ der Themes-Seite.
export function StoreRepos({ onChanged }: { onChanged?: () => void } = {}) {
  const api = useApi();
  const list = useAsync(() => api.listStoreRepos(), [api]);
  const [url, setUrl] = useState('');
  const [error, setError] = useState<string | null>(null);
  const repos = list.state.status === 'ready' ? list.state.data : [];

  // Trägt das Repo ein; bei Fehler bleibt die Eingabe stehen und die Meldung des Servers erscheint
  const add = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const added = await api.addStoreRepo(url.trim());
      list.set([...repos, added]);
      setUrl('');
      onChanged?.();
    } catch (err) {
      setError((err as Error).message);
    }
  };

  // Entfernt nach Bestätigung
  const remove = async (repo: StoreRepoDto) => {
    if (!window.confirm(`Repo „${repo.url}“ entfernen?`)) return;
    setError(null);
    try {
      await api.removeStoreRepo(repo.id);
      list.set(repos.filter((x) => x.id !== repo.id));
      onChanged?.();
    } catch (err) {
      setError((err as Error).message);
    }
  };

  return (
    <div className="store-repos">
      {list.state.status === 'error' && <p className="muted">Repos konnten nicht geladen werden.</p>}
      {list.state.status === 'ready' && repos.length === 0 && <p className="muted">Noch keine Repos eingetragen.</p>}
      <ul className="store-repo-list">
        {repos.map((repo) => (
          <li key={repo.id} className="store-repo-row">
            <span className="material-icons" aria-hidden="true">storefront</span>
            <span className="store-repo-url">{repo.url}</span>
            <button type="button" className="icon-btn" aria-label={`Repo entfernen: ${repo.url}`} onClick={() => void remove(repo)}>
              <span className="material-icons" aria-hidden="true">delete</span>
            </button>
          </li>
        ))}
      </ul>
      <form className="store-repo-add" onSubmit={(e) => void add(e)}>
        <input type="url" aria-label="Repo-URL" placeholder="https://…/nexus-themes.json" value={url} onChange={(e) => setUrl(e.target.value)} />
        <button type="submit" className="btn primary" disabled={!url.trim()}>Hinzufügen</button>
      </form>
      {error && <p className="save-status error" role="alert">{error}</p>}
    </div>
  );
}
