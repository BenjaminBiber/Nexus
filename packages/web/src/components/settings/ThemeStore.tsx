import { useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { ApiError } from '../../api/ApiError.js';
import type { ThemeManifest } from '../../api/generated/themes.js';
import { serverUrl } from '../../api/nexusBridge.js';
import type { StoreIndexEntryDto, StoreIndexRepoDto, StoreUpdateDto } from '../../api/types.js';
import { useAsync } from '../../hooks/useAsync.js';
import { Notice } from './Notice.js';
import { ThemeInstallDialog } from './ThemeInstallDialog.js';

type Props = {
  installed: ThemeManifest[];
  onInstalled: () => void;
  reloadKey?: number;
  /** Öffnet die Verwaltung der Theme-Stores (Knopf, solange keiner eingetragen ist) */
  onManageRepos?: () => void;
};

// Offene Rückfrage: Installation eines Store-Eintrags oder Update mit Code/neuen Domains
type Pending =
  | { kind: 'install'; entry: StoreIndexEntryDto }
  | { kind: 'update'; update: StoreUpdateDto; hasCode: boolean; domains: string[] };

// Store-Liste aller eingetragenen Repos: Zustand der Stores (aufklappbar, mit erneuter Prüfung), Suche, Kacheln mit Vorschau
// und Installieren (Rückfrage als eigener Dialog) sowie Updates (Bestätigung bei Code oder neuen Domains, D2)
export function ThemeStore({ installed, onInstalled, reloadKey = 0, onManageRepos }: Props) {
  const api = useApi();
  const index = useAsync(() => api.getStoreIndex(), [api, reloadKey]);
  const updates = useAsync(() => api.getStoreUpdates(), [api, reloadKey]);
  const [query, setQuery] = useState('');
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [rechecked, setRechecked] = useState(false);

  const data = index.state.status === 'ready' ? index.state.data : null;
  const q = query.trim().toLowerCase();
  const entries = (data?.entries ?? []).filter((e) => !q || [e.name, e.id, e.description ?? ''].some((s) => s.toLowerCase().includes(q)));
  const updateList = updates.state.status === 'ready' ? updates.state.data.updates : [];
  const isInstalled = (e: StoreIndexEntryDto) => installed.some((m) => m.id === e.id && m.version === e.version);

  // Führt eine Server-Aktion aus; Fehler zeigen die Meldung des Servers
  const act = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await fn();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const install = (entry: StoreIndexEntryDto) => act(async () => {
    try {
      const result = await api.installFromStore(entry.repoId, entry.id);
      setNotice(`„${result.manifest.name}“ installiert${result.verified ? ' – Signatur geprüft' : ''}.`);
      onInstalled();
    } finally {
      setPending(null);
    }
  });

  // Erst ohne Bestätigung; verlangt der Server sie, erscheint die Rückfrage mit Code-Hinweis und neuen Domains
  const update = (item: StoreUpdateDto, confirm?: boolean) => act(async () => {
    try {
      const result = await (confirm ? api.updateFromStore(item.repoId, item.id, true) : api.updateFromStore(item.repoId, item.id));
      setPending(null);
      setNotice(`„${result.manifest.name}“ auf ${result.manifest.version} aktualisiert${result.verified ? ' – Signatur geprüft' : ''}.`);
      if (updates.state.status === 'ready') updates.set({ ...updates.state.data, updates: updateList.filter((u) => u !== item) });
      onInstalled();
    } catch (err) {
      if (!confirm && err instanceof ApiError && err.code === 'store-update-confirmation-required') {
        const details = err.details ?? {};
        setPending({ kind: 'update', update: item, hasCode: details.hasCode === true, domains: Array.isArray(details.newDomains) ? (details.newDomains as string[]) : [] });
        return;
      }
      setPending(null);
      throw err;
    }
  });

  // Lädt Index und Updates neu, ohne die Kacheln auszublenden
  const recheck = async () => {
    if (checking) return;
    setChecking(true);
    await Promise.all([index.refresh(), updates.refresh()]);
    setChecking(false);
    setRechecked(true);
  };

  const count = entries.length === 1 ? '1 Theme' : `${entries.length} Themes`;

  return (
    <div className="theme-store">
      {notice && <Notice tone="ok">{notice}</Notice>}
      {error && <p className="save-status error" role="alert">{error}</p>}
      {index.state.status === 'error' && <Notice tone="error">Store konnte nicht geladen werden: {index.state.message}</Notice>}

      {data && data.repos.length > 0 && (
        <StoreStatus repos={data.repos} checking={checking} lastChecked={rechecked ? 'gerade eben' : 'beim Öffnen des Dialogs'} onRecheck={() => void recheck()} />
      )}

      {updateList.length > 0 && (
        <section className="store-updates" aria-labelledby="store-updates">
          <span id="store-updates" className="theme-store-label">Updates</span>
          <ul className="store-update-list" aria-labelledby="store-updates">
            {updateList.map((u) => (
              <li key={`${u.repoId}/${u.id}`} className="store-update-row">
                <span className="store-update-name">{u.name}</span>
                <span className="muted store-update-versions">{u.installedVersion} → {u.availableVersion}</span>
                <button type="button" className="btn" disabled={busy} onClick={() => void update(u)}>Aktualisieren</button>
              </li>
            ))}
          </ul>
        </section>
      )}
      {/* Stores, die schon beim Index scheitern, erklärt ihre Zeile im Zustand; nur übrige Fehler der Update-Prüfung */}
      {updates.state.status === 'ready' && updates.state.data.errors.filter((e) => !data?.repos.some((r) => r.id === e.repoId && !r.ok)).map((e) => (
        <Notice key={`${e.repoId}/${e.code}`} tone="info">Update-Prüfung: {e.message}</Notice>
      ))}

      <div className="store-search-row">
        <label className="store-search">
          <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg>
          <input type="search" aria-label="Store durchsuchen" placeholder="Themes suchen …" value={query} onChange={(e) => setQuery(e.target.value)} />
        </label>
        {data && data.repos.length > 0 && <span className="store-count">{count}</span>}
      </div>

      {data && data.repos.length === 0 && (
        <div className="theme-store-empty">
          <strong>Noch kein Theme-Store eingetragen</strong>
          <p className="muted">Füge einen Store hinzu, um Themes zu finden.</p>
          {onManageRepos && (
            <button type="button" className="btn" onClick={onManageRepos}>
              <span className="material-icons" aria-hidden="true">add</span>
              <span>Store hinzufügen</span>
            </button>
          )}
        </div>
      )}
      {data && data.repos.length > 0 && entries.length === 0 && (
        <div className="theme-store-empty is-centered">
          <strong>Keine Themes gefunden</strong>
          {q && <span className="muted">Kein Theme passt zu „{query.trim()}“.</span>}
        </div>
      )}

      <ul className="store-grid">
        {entries.map((e) => (
          <li key={`${e.repoId}/${e.id}`} className="store-card">
            <div className="store-card-preview">
              {e.preview ? (
                <img src={previewUrl(e)} alt={`Vorschau: ${e.name}`} loading="lazy" />
              ) : (
                <div className="store-card-nopreview">
                  <span className="material-icons" aria-hidden="true">image</span>
                  Keine Vorschau
                </div>
              )}
            </div>
            <div className="store-card-body">
              <div className="store-card-title">
                <h3>{e.name}</h3>
                <span className="store-card-version">v{e.version}</span>
              </div>
              <span className="store-card-host">
                <StoreIcon />
                {repoLabel(e.repoUrl).name}
              </span>
              {e.description && <p className="store-card-desc">{e.description}</p>}
              {!e.compatible && <span className="store-pill is-warn">Nicht kompatibel</span>}
              {isInstalled(e) ? (
                <button type="button" className="store-card-btn is-installed" disabled>
                  <span className="material-icons" aria-hidden="true">check</span>
                  Installiert
                </button>
              ) : (
                <button type="button" className="store-card-btn" disabled={busy || !e.compatible} onClick={() => setPending({ kind: 'install', entry: e })}>Installieren</button>
              )}
            </div>
          </li>
        ))}
      </ul>

      {pending?.kind === 'install' && (
        <ThemeInstallDialog
          manifest={{ name: pending.entry.name, version: pending.entry.version }}
          hasCode="unknown"
          domains={null}
          busy={busy}
          onConfirm={() => void install(pending.entry)}
          onCancel={() => setPending(null)}
        />
      )}
      {pending?.kind === 'update' && (
        <ThemeInstallDialog
          title="Update installieren?"
          manifest={{ name: pending.update.name, version: pending.update.availableVersion }}
          hasCode={pending.hasCode}
          domains={pending.domains}
          busy={busy}
          onConfirm={() => void update(pending.update, true)}
          onCancel={() => setPending(null)}
        />
      )}
    </div>
  );
}

// Adresse des Vorschaubilds eines Store-Eintrags: der Server holt es beim Store ab, weil die CSP der
// Oberfläche keine Bilder fremder Hosts zulässt. `v` lässt den Browser nach einem Update neu laden.
function previewUrl(e: StoreIndexEntryDto): string {
  return serverUrl(`/api/themes/store/preview?repoId=${encodeURIComponent(e.repoId)}&id=${encodeURIComponent(e.id)}&v=${encodeURIComponent(e.version)}`);
}

// Zustand der eingetragenen Stores: Kopfzeile mit Punkten und Zusammenfassung, aufgeklappt je Store eine Zeile.
// Startet eingeklappt; Fehler zeigt die Kopfzeile (roter Punkt, Hinweis).
function StoreStatus({ repos, checking, lastChecked, onRecheck }: { repos: StoreIndexRepoDto[]; checking: boolean; lastChecked: string; onRecheck: () => void }) {
  const failed = repos.filter((r) => !r.ok).length;
  const [open, setOpen] = useState(false);
  const reachable = repos.length - failed;
  return (
    <section className="store-status">
      <button type="button" className="store-status-head" aria-expanded={open} onClick={() => setOpen(!open)}>
        <StoreIcon />
        <span className="store-status-title">Theme-Stores</span>
        <span className="store-status-dots" aria-hidden="true">
          {repos.map((r) => <span key={r.id} className={`store-dot${r.ok ? '' : ' is-error'}${checking && !r.ok ? ' is-checking' : ''}`} />)}
        </span>
        <span className="store-status-summary">{reachable} von {repos.length} erreichbar</span>
        {failed > 0 && <span className="store-pill is-error">{failed === 1 ? '1 Store nicht erreichbar' : `${failed} Stores nicht erreichbar`}</span>}
        <span className="store-status-toggle">{open ? 'Ausblenden' : 'Details'}</span>
        <svg className="store-status-chevron" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
      </button>
      {open && (
        <ul className="store-status-list" aria-label="Theme-Stores">
          {repos.map((r) => <StoreRow key={r.id} repo={r} checking={checking} lastChecked={lastChecked} onRecheck={onRecheck} />)}
        </ul>
      )}
    </section>
  );
}

function StoreRow({ repo, checking, lastChecked, onRecheck }: { repo: StoreIndexRepoDto; checking: boolean; lastChecked: string; onRecheck: () => void }) {
  const label = repoLabel(repo.url);
  if (repo.ok) {
    return (
      <li className="store-row">
        <span className="store-dot" aria-hidden="true" />
        <div className="store-row-text">
          <span className="store-row-name">{label.name}</span>
          <span className="store-row-url" title={repo.url}>{label.path}</span>
        </div>
        <span className="store-row-count">{repo.entryCount === 1 ? '1 Theme' : `${repo.entryCount} Themes`}</span>
        <span className="store-pill is-ok">Erreichbar</span>
      </li>
    );
  }
  const badge = checking ? 'prüfe …' : errorBadge(repo.error);
  const notFound = /\b404\b/.test(repo.error?.message ?? '');
  const text = notFound && label.releases
    ? 'Der Store antwortet mit „Nicht gefunden“. Meist fehlt im neuesten Release die Datei nexus-themes.json – oder das Repo ist privat.'
    : repo.error?.message ?? 'Der Store konnte nicht geladen werden.';
  return (
    <li className="store-row is-error">
      <span className={`store-dot is-error${checking ? ' is-checking' : ''}`} aria-hidden="true" />
      <div className="store-row-text">
        <div className="store-row-line">
          <span className="store-row-name">{label.name}</span>
          <span className={`store-row-badge${checking ? ' is-checking' : ''}`}>{badge}</span>
        </div>
        <span className="store-row-url" title={repo.url}>{label.path}</span>
        <p className="store-row-error">{text} Themes aus diesem Store und die Update-Prüfung sind solange nicht verfügbar.</p>
        <span className="store-row-checked">Zuletzt geprüft: {lastChecked}</span>
      </div>
      <div className="store-row-actions">
        {label.releases && (
          <a className="store-row-link" href={label.releases} target="_blank" rel="noreferrer">
            Releases öffnen
            <span className="material-icons" aria-hidden="true">open_in_new</span>
          </a>
        )}
        <button type="button" className="btn store-row-retry" disabled={checking} onClick={onRecheck}>
          <span className="material-icons" aria-hidden="true">refresh</span>
          {checking ? 'Prüfe …' : 'Erneut prüfen'}
        </button>
      </div>
    </li>
  );
}

// Kurzer Fehlerhinweis je Code; bei HTTP-Fehlern der Status aus der Meldung des Servers
function errorBadge(error: StoreIndexRepoDto['error']): string {
  if (!error) return 'Fehler';
  const status = /HTTP (\d{3})/.exec(error.message)?.[1] ?? /\b(\d{3})\b/.exec(error.message)?.[1];
  switch (error.code) {
    case 'store-index-http': return status ? `HTTP ${status}` : 'HTTP-Fehler';
    case 'store-index-timeout': return 'Zeitüberschreitung';
    case 'store-index-unreachable': return 'Nicht erreichbar';
    case 'store-index-too-large': return 'Zu groß';
    case 'store-index-invalid': return 'Ungültig';
    default: return error.code;
  }
}

// Anzeigename eines Store-Repos: GitHub-Releases als „github.com/owner/repo“ mit Link auf die Releases, sonst Host (mit Port)
function repoLabel(url: string): { name: string; path: string; releases?: string } {
  try {
    const u = new URL(url);
    const gh = u.hostname === 'github.com' ? /^\/([^/]+)\/([^/]+)\/(releases\/.*)$/.exec(u.pathname) : null;
    if (gh) return { name: `github.com/${gh[1]}/${gh[2]}`, path: `…/${gh[3]}`, releases: `https://github.com/${gh[1]}/${gh[2]}/releases` };
    return { name: u.host, path: url };
  } catch {
    return { name: url, path: url };
  }
}

function StoreIcon() {
  return (
    <svg className="store-icon" viewBox="0 0 24 24" aria-hidden="true">
      <rect x="3" y="4" width="18" height="7" rx="2" /><rect x="3" y="13" width="18" height="7" rx="2" /><path d="M7 7.5h.01" /><path d="M7 16.5h.01" />
    </svg>
  );
}
