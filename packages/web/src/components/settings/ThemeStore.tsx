import { useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { ApiError } from '../../api/ApiError.js';
import type { ThemeManifest } from '../../api/generated/themes.js';
import type { StoreIndexEntryDto, StoreUpdateDto } from '../../api/types.js';
import { useAsync } from '../../hooks/useAsync.js';
import { Notice } from './Notice.js';
import { SettingsSection } from './SettingRow.js';
import { ThemeInstallDialog } from './ThemeInstallDialog.js';

type Props = { installed: ThemeManifest[]; onInstalled: () => void; reloadKey?: number };

// Offene Rückfrage: Installation eines Store-Eintrags oder Update mit Code/neuen Domains
type Pending =
  | { kind: 'install'; entry: StoreIndexEntryDto }
  | { kind: 'update'; update: StoreUpdateDto; hasCode: boolean; domains: string[] };

// Store-Liste aller eingetragenen Repos mit Suche, Installieren (mit Rückfrage) und Updates (Bestätigung bei Code oder neuen Domains, D2)
export function ThemeStore({ installed, onInstalled, reloadKey = 0 }: Props) {
  const api = useApi();
  const index = useAsync(() => api.getStoreIndex(), [api, reloadKey]);
  const updates = useAsync(() => api.getStoreUpdates(), [api, reloadKey]);
  const [query, setQuery] = useState('');
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

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

  return (
    <SettingsSection title="Themes im Store">
      {updateList.length > 0 && (
        <>
          <span id="store-updates">Updates</span>
          <ul className="snippet-list" aria-labelledby="store-updates">
            {updateList.map((u) => (
              <li key={`${u.repoId}/${u.id}`} className="snippet-row">
                <span>{u.name}</span>
                <span className="muted">{u.installedVersion} → {u.availableVersion}</span>
                <button type="button" className="btn" disabled={busy} onClick={() => void update(u)}>Aktualisieren</button>
              </li>
            ))}
          </ul>
        </>
      )}
      {updates.state.status === 'ready' && updates.state.data.errors.map((e) => (
        <Notice key={`${e.repoId}/${e.code}`} tone="info">Update-Prüfung: {e.message}</Notice>
      ))}
      <input type="search" aria-label="Store durchsuchen" placeholder="Themes suchen …" value={query} onChange={(e) => setQuery(e.target.value)} />
      {index.state.status === 'error' && <Notice tone="error">Store konnte nicht geladen werden: {index.state.message}</Notice>}
      {data?.repos.filter((r) => !r.ok).map((r) => (
        <Notice key={r.id} tone="warn">{r.url}: {r.error?.message ?? 'Repo nicht ladbar'}</Notice>
      ))}
      {data && data.repos.length === 0 && <p className="muted">Trage oben ein Repo ein, um Themes zu finden.</p>}
      {data && data.repos.length > 0 && entries.length === 0 && <p className="muted">Keine Themes gefunden</p>}
      <ul className="store-grid">
        {entries.map((e) => (
          <li key={`${e.repoId}/${e.id}`} className="store-card">
            {e.preview && <img src={e.preview} alt={`Vorschau: ${e.name}`} />}
            <strong>{e.name}</strong>
            <span className="muted">Version {e.version}</span>
            {e.description && <p>{e.description}</p>}
            <span className="muted store-repo">{e.repoUrl}</span>
            {!e.compatible && <span className="notice warn">Nicht kompatibel</span>}
            {isInstalled(e) ? (
              <span className="muted">Installiert</span>
            ) : (
              <button type="button" className="btn primary" disabled={busy || !e.compatible} onClick={() => setPending({ kind: 'install', entry: e })}>Installieren</button>
            )}
          </li>
        ))}
      </ul>
      {notice && <Notice tone="ok">{notice}</Notice>}
      {error && <p className="save-status error" role="alert">{error}</p>}
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
    </SettingsSection>
  );
}
