import { useCallback, useRef, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { ThemeManifest } from '../../api/generated/themes.js';
import type { AsyncState } from '../../hooks/useAsync.js';
import { inspectThemePackage, ThemePackageReadError } from '../../themes/inspectPackage.js';
import { Notice } from './Notice.js';
import { SettingsSection } from './SettingRow.js';
import { ThemeInstallDialog } from './ThemeInstallDialog.js';
import { describeThemeError } from './themeErrors.js';

type Props = {
  installed: AsyncState<ThemeManifest[]>;
  /** Lädt die Liste nach Installieren oder Entfernen neu */
  onChanged: () => Promise<unknown>;
  /** Wurde ein Theme entfernt; der Aufrufer schaltet ggf. auf „System“ */
  onRemoved: (id: string) => Promise<unknown>;
};

// Installierte Themes als Kacheln mit Upload einer .nexus-Datei und Entfernen nach Rückfrage
export function ThemeManager({ installed, onChanged, onRemoved }: Props) {
  const api = useApi();
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ message: string; fields: string[] } | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);

  const [pending, setPending] = useState<{ file: File; manifest: ThemeManifest; hasCode: boolean } | null>(null);

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
  const upload = async (file: File) => {
    setBusy(true);
    setError(null);
    try {
      await api.installTheme(file);
      setPending(null);
      await onChanged();
    } catch (err) {
      setPending(null);
      setError(describeThemeError(err));
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
      await onChanged();
    } catch (err) {
      setError(describeThemeError(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <SettingsSection title="Installierte Themes">
      <div className="theme-upload">
        <label className={`btn${busy ? ' disabled' : ''}`}>
          <span className="material-icons" aria-hidden="true">upload_file</span>
          <span>Theme hochladen</span>
          <input ref={input} type="file" accept=".nexus" aria-label="Theme hochladen" className="sr-only" disabled={busy} onChange={(e) => void choose(e.target.files?.[0])} />
        </label>
      </div>
      {pending && <ThemeInstallDialog manifest={pending.manifest} hasCode={pending.hasCode} busy={busy} onConfirm={() => void upload(pending.file)} onCancel={cancel} />}
      {error && (
        <Notice tone="error">
          {error.message}
          {error.fields.length > 0 && (
            <ul>
              {error.fields.map((field) => <li key={field}>{field}</li>)}
            </ul>
          )}
        </Notice>
      )}
      {installed.status === 'error' && <Notice tone="error">Installierte Themes konnten nicht geladen werden: {installed.message}</Notice>}
      {installed.status === 'ready' && installed.data.length === 0 && <p className="muted">Noch keine Themes installiert.</p>}
      {installed.status === 'ready' && (
        <ul className="theme-tiles" aria-label="Installierte Themes">
          {installed.data.map((theme) => (
            <li key={theme.id} className="theme-tile" aria-label={theme.name}>
              {theme.preview ? (
                <img className="theme-tile-preview" src={`/themes/${encodeURIComponent(theme.id)}/${theme.preview}`} alt={`Vorschau ${theme.name}`} />
              ) : (
                <div className="theme-tile-preview" style={{ background: theme.startColor ?? 'var(--line)' }} aria-hidden="true" />
              )}
              <strong>{theme.name}</strong>
              <span className="muted">{theme.author} · Version {theme.version}</span>
              {confirm === theme.id ? (
                <div role="alertdialog" aria-label={`Theme ${theme.name} entfernen`} className="theme-tile-confirm">
                  <span>„{theme.name}“ wirklich entfernen?</span>
                  <button type="button" className="btn danger" disabled={busy} onClick={() => void remove(theme.id)}>Endgültig entfernen</button>
                  <button type="button" className="btn" disabled={busy} onClick={() => setConfirm(null)}>Abbrechen</button>
                </div>
              ) : (
                <button type="button" className="btn" aria-label={`${theme.name} entfernen`} disabled={busy} onClick={() => setConfirm(theme.id)}>
                  <span className="material-icons" aria-hidden="true">delete</span>
                  <span>Entfernen</span>
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </SettingsSection>
  );
}
