import { useCallback, useContext, useId, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useApi } from '../../api/ApiContext.js';
import type { ThemeManifest } from '../../api/generated/themes.js';
import { useAsync } from '../../hooks/useAsync.js';
import { unregisterTheme } from '../../theme/registry.js';
import { applyTheme, displayedTheme, needsReload } from '../../theme/theme.js';
import { setThemeNotice } from '../../theme/themeNoticeStore.js';
import { ThemeSettingsSection } from '../../theme/ThemeSettingsSection.js';
import { inspectThemePackage, ThemePackageReadError } from '../../themes/inspectPackage.js';
import { DbModal } from '../db/DbModal.js';
import { Notice } from './Notice.js';
import { SaveStatusSlot, SettingsPanel } from './SettingsPanel.js';
import { StoreRepos } from './StoreRepos.js';
import { DARK_PALETTE, LIGHT_PALETTE, ThemeCard, type ThemeEntry } from './ThemeCard.js';
import { ThemeInstallDialog } from './ThemeInstallDialog.js';
import { ThemeStore } from './ThemeStore.js';
import { describeThemeError } from './themeErrors.js';
import { useUserSettings } from './useUserSettings.js';

// Fest eingebaute Modi; „System“ ist der Standard
const BUILTIN: ThemeEntry[] = [
  { id: 'system', name: 'System', meta: 'Nexus · folgt dem Betriebssystem', builtin: true, scheme: 'system', hasCode: false, palette: LIGHT_PALETTE },
  { id: 'light', name: 'Hell', meta: 'Nexus · immer hell', builtin: true, scheme: 'light', hasCode: false, palette: LIGHT_PALETTE },
  { id: 'dark', name: 'Dunkel', meta: 'Nexus · immer dunkel', builtin: true, scheme: 'dark', hasCode: false, palette: DARK_PALETTE },
];
const DEFAULT_THEME = 'system';

type Filter = 'all' | 'builtin' | 'installed';
const FILTERS: Array<{ id: Filter; label: string; match: (t: ThemeEntry) => boolean }> = [
  { id: 'all', label: 'Alle', match: () => true },
  { id: 'builtin', label: 'Eingebaut', match: (t) => t.builtin },
  { id: 'installed', label: 'Installiert', match: (t) => !t.builtin },
];

// Installiertes Paket als Kachel; ohne Vorschaubild die Farben seines Schemas mit der Startfarbe als Hintergrund
function fromManifest(m: ThemeManifest): ThemeEntry {
  const base = m.colorScheme === 'dark' ? DARK_PALETTE : LIGHT_PALETTE;
  return {
    id: m.id,
    name: m.name,
    meta: `${m.author} · v${m.version}`,
    builtin: false,
    scheme: m.colorScheme === 'dark' ? 'dark' : 'light',
    hasCode: (m.slots?.length ?? 0) > 0,
    image: m.preview ? `/themes/${encodeURIComponent(m.id)}/${m.preview}` : undefined,
    palette: m.startColor ? { ...base, bg: m.startColor, side: m.startColor } : base,
  };
}

// Darstellung › Themes: Kacheln aller Themes mit Filter und Suche, Anwenden, Entfernen und Einstellungen des aktiven Themes;
// Installieren (Store oder Datei) und die Verwaltung der Theme-Stores liegen in Dialogen
export function ThemesPanel() {
  const api = useApi();
  const { settings, save, state, run } = useUserSettings();
  // Installierte Themes; schlägt das Laden fehl, bleiben nur die eingebauten
  const installed = useAsync(() => api.listThemes(), [api]);
  const manifests = installed.state.status === 'ready' ? installed.state.data : [];
  const headSlot = useContext(SaveStatusSlot);
  const [storeKey, setStoreKey] = useState(0);
  const [filter, setFilter] = useState<Filter>('all');
  const [query, setQuery] = useState('');
  // Offener Dialog: Themes aus dem Store installieren bzw. Theme-Stores verwalten
  const [dialog, setDialog] = useState<'store' | 'repos' | null>(null);

  const builtinIds = new Set(BUILTIN.map((t) => t.id));
  const all = [...BUILTIN, ...manifests.filter((m) => !builtinIds.has(m.id)).map(fromManifest)];
  const q = query.trim().toLowerCase();
  const matchFilter = FILTERS.find((f) => f.id === filter)!.match;
  const shown = all.filter(matchFilter).filter((t) => !q || `${t.name} ${t.meta}`.toLowerCase().includes(q));
  const activeId = settings?.theme;

  // Speichert das Theme und setzt es sofort; kommt oder geht dabei ein Theme mit Code, lädt die Oberfläche neu (main.js lädt nur beim Start)
  const choose = async (id: string) => {
    const before = displayedTheme();
    const saved = await save({ theme: id });
    if (!saved) return;
    applyTheme(saved.theme, manifests);
    if (needsReload(before, saved.theme, manifests)) {
      setThemeNotice({ kind: 'info', message: 'Theme wird geladen …' });
      window.location.reload();
    }
  };

  // Das entfernte Theme wird abgemeldet (seine Slots greifen nicht mehr); war es aktiv, gilt danach der Standard
  const removed = async (id: string) => {
    unregisterTheme(id);
    if (activeId !== id) return;
    const saved = await save({ theme: DEFAULT_THEME });
    if (saved) applyTheme(saved.theme);
  };

  const upload = useThemeUpload(installed.reload, removed);
  const { cancel: cancelUpload } = upload;
  // Schließen verwirft auch eine offene Rückfrage zur gewählten Datei
  const closeDialog = useCallback(() => {
    setDialog(null);
    cancelUpload();
  }, [cancelUpload]);

  // Installation aus einer Datei im Store-Dialog; gelingt sie, schließt der Dialog und das Theme steht im Raster
  const installFile = async (file: File) => {
    if (await upload.install(file)) setDialog(null);
  };

  // Zurücksetzen und Installieren stehen in der Kopfzeile neben „Themes“ (ohne Kopfzeile, etwa in Tests, über den Kacheln)
  const actions = (
    <div className="theme-actions">
      <button
        type="button"
        className="icon-btn theme-reset"
        aria-label="Auf Standard zurücksetzen"
        title="Auf Standard zurücksetzen"
        disabled={!settings || activeId === DEFAULT_THEME}
        onClick={() => void choose(DEFAULT_THEME)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 12a9 9 0 1 0 3-6.7" /><path d="M3 4v5h5" /></svg>
      </button>
      <button type="button" className="btn primary theme-install-btn" aria-haspopup="dialog" onClick={() => setDialog('store')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v12" /><path d="M7 11l5 5 5-5" /><path d="M4 20h16" /></svg>
        <span>Theme installieren</span>
      </button>
    </div>
  );

  return (
    <SettingsPanel label="Themes" status={state} className="themes-panel">
      {headSlot ? createPortal(actions, headSlot) : actions}
      {dialog !== 'store' && upload.error && <UploadError error={upload.error} />}
      {installed.state.status === 'error' && <Notice tone="error">Installierte Themes konnten nicht geladen werden: {installed.state.message}</Notice>}

      <section className="theme-section" aria-labelledby="themes-installed-h">
        <div className="theme-toolbar">
          <h3 id="themes-installed-h">Installiert</h3>
          <div className="theme-toolbar-controls">
            <div role="group" aria-label="Filter" className="theme-filter">
              {FILTERS.map((f) => (
                <button key={f.id} type="button" aria-pressed={filter === f.id} onClick={() => setFilter(f.id)}>
                  {f.label} <span className="theme-filter-count">{all.filter(f.match).length}</span>
                </button>
              ))}
            </div>
            <button
              type="button"
              className="icon-btn theme-repos-btn"
              aria-label="Theme-Stores verwalten"
              title="Theme-Store hinzufügen oder verwalten"
              aria-haspopup="dialog"
              onClick={() => setDialog('repos')}
            >
              <span className="material-icons" aria-hidden="true">add</span>
            </button>
            <label className="theme-search">
              <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="M20 20l-3.5-3.5" /></svg>
              <span className="sr-only">Themes durchsuchen</span>
              <input type="search" placeholder="Name oder Autor" value={query} onChange={(e) => setQuery(e.target.value)} />
            </label>
          </div>
        </div>
        <ul className="theme-grid" aria-label="Themes">
          {shown.map((theme) => (
            <ThemeCard
              key={theme.id}
              theme={theme}
              active={theme.id === activeId}
              disabled={!settings || upload.busy}
              onApply={() => void choose(theme.id)}
              onRemove={theme.builtin ? undefined : () => upload.askRemove(theme)}
            />
          ))}
        </ul>
        {shown.length === 0 && <div className="theme-empty">Keine Themes für diesen Filter.</div>}
      </section>

      {upload.confirm && <RemoveDialog theme={upload.confirm} busy={upload.busy} onConfirm={() => void upload.remove(upload.confirm!.id)} onCancel={upload.cancelRemove} />}

      <ThemeSettingsSection run={run} />

      {dialog === 'repos' && (
        <ModalFrame title="Theme-Stores" hint="Nexus sucht Themes nur in Stores, die du hier selbst einträgst." onClose={closeDialog}>
          <StoreRepos onChanged={() => setStoreKey((k) => k + 1)} />
        </ModalFrame>
      )}
      {dialog === 'store' && (
        <ModalFrame
          title="Theme installieren"
          hint="Aus einem eingetragenen Store oder aus einer .nexus-Datei."
          wide
          onClose={closeDialog}
          action={
            <label className={`btn theme-file-btn${upload.busy ? ' disabled' : ''}`}>
              <span className="material-icons" aria-hidden="true">upload_file</span>
              <span>Aus Datei</span>
              <input ref={upload.input} type="file" accept=".nexus" aria-label="Aus Datei installieren" className="sr-only" disabled={upload.busy} onChange={(e) => void upload.choose(e.target.files?.[0])} />
            </label>
          }
        >
          {upload.pending && (
            <ThemeInstallDialog manifest={upload.pending.manifest} hasCode={upload.pending.hasCode} busy={upload.busy} onConfirm={() => void installFile(upload.pending!.file)} onCancel={upload.cancel} />
          )}
          {upload.error && <UploadError error={upload.error} />}
          <ThemeStore installed={manifests} onInstalled={installed.reload} reloadKey={storeKey} onManageRepos={() => setDialog('repos')} />
        </ModalFrame>
      )}
    </SettingsPanel>
  );
}

// Installieren aus einer .nexus-Datei (mit Rückfrage) und Entfernen (mit Bestätigung); Fehler mit den Feldern des Manifests
function useThemeUpload(reload: () => Promise<unknown>, onRemoved: (id: string) => Promise<unknown>) {
  const api = useApi();
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ message: string; fields: string[] } | null>(null);
  const [pending, setPending] = useState<{ file: File; manifest: ThemeManifest; hasCode: boolean } | null>(null);
  const [confirm, setConfirm] = useState<ThemeEntry | null>(null);

  const resetInput = () => {
    if (input.current) input.current.value = '';
  };

  // Liest das gewählte Paket und zeigt den Installationsdialog; ein Lesefehler installiert nichts
  const choose = async (file: File | undefined) => {
    if (!file) return;
    setError(null);
    try {
      setPending({ file, ...(await inspectThemePackage(file)) });
    } catch (err) {
      setError(err instanceof ThemePackageReadError ? { message: err.message, fields: [] } : describeThemeError(err));
      resetInput();
    }
  };

  const cancel = useCallback(() => {
    setPending(null);
    resetInput();
  }, []);

  // Lädt die bestätigte Datei hoch und aktualisiert die Liste
  const install = async (file: File): Promise<boolean> => {
    setBusy(true);
    setError(null);
    try {
      await api.installTheme(file);
      setPending(null);
      await reload();
      return true;
    } catch (err) {
      setPending(null);
      setError(describeThemeError(err));
      return false;
    } finally {
      setBusy(false);
      resetInput();
    }
  };

  // Entfernt das bestätigte Theme
  const remove = async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.removeTheme(id);
      setConfirm(null);
      await onRemoved(id);
      await reload();
    } catch (err) {
      setConfirm(null);
      setError(describeThemeError(err));
    } finally {
      setBusy(false);
    }
  };

  const cancelRemove = useCallback(() => setConfirm(null), []);

  return { input, busy, error, pending, confirm, choose, cancel, install, remove, askRemove: setConfirm, cancelRemove };
}

// Fehler beim Installieren aus einer Datei, bei Manifestfehlern mit den Feldern
function UploadError({ error }: { error: { message: string; fields: string[] } }) {
  return (
    <Notice tone="error">
      {error.message}
      {error.fields.length > 0 && (
        <ul>
          {error.fields.map((field) => <li key={field}>{field}</li>)}
        </ul>
      )}
    </Notice>
  );
}

// Dialog der Themes-Seite: Kopf mit Titel, Satz darunter, optionaler Aktion und Schließen; Esc und Klick daneben schließen.
// Steht darin eine Installations-Rückfrage, bricht Esc nur diese ab (sie hört selbst auf Esc)
function ModalFrame({ title, hint, action, wide, onClose, children }: { title: string; hint?: string; action?: ReactNode; wide?: boolean; onClose: () => void; children: ReactNode }) {
  const titleId = useId();
  return (
    <DbModal labelledBy={titleId} onEscape={() => { if (!document.querySelector('.theme-modal .theme-install')) onClose(); }} onBackdrop={onClose} className={`panel db-modal theme-modal${wide ? ' wide' : ''}`}>
      <header className="theme-modal-head">
        <div className="theme-modal-title">
          <h2 id={titleId}>{title}</h2>
          {hint && <p>{hint}</p>}
        </div>
        {action}
        <button type="button" className="icon-btn" aria-label="Schließen" onClick={onClose}>
          <span className="material-icons" aria-hidden="true">close</span>
        </button>
      </header>
      {children}
    </DbModal>
  );
}

// Rückfrage vor dem Entfernen eines installierten Themes
function RemoveDialog({ theme, busy, onConfirm, onCancel }: { theme: ThemeEntry; busy: boolean; onConfirm: () => void; onCancel: () => void }) {
  const titleId = useId();
  return (
    <DbModal role="alertdialog" labelledBy={titleId} onEscape={onCancel} onBackdrop={onCancel} className="panel db-modal theme-confirm">
      <div className="theme-confirm-icon" aria-hidden="true">
        <span className="material-icons">delete</span>
      </div>
      <h2 id={titleId}>Theme „{theme.name}“ entfernen?</h2>
      <p>Das Theme wird von diesem Rechner gelöscht. Ist es gerade aktiv, gilt danach wieder „System“.</p>
      <div className="actions">
        <button type="button" className="btn" disabled={busy} onClick={onCancel}>Abbrechen</button>
        <button type="button" className="btn danger" disabled={busy} onClick={onConfirm}>Endgültig entfernen</button>
      </div>
    </DbModal>
  );
}
