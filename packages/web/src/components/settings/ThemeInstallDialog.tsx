import { useEffect } from 'react';
import type { ThemeManifest } from '../../api/generated/themes.js';
import { Button } from '../ui/Button.js';
import { Dialog } from '../ui/Dialog.js';

type Props = { manifest: ThemeManifest; hasCode: boolean; busy: boolean; onConfirm: () => void; onCancel: () => void };

// Rückfrage vor dem Installieren eines Themes (Vertrauensmodell D2): zeigt Herkunft, Code-Hinweis und angefragte Domains
export function ThemeInstallDialog({ manifest, hasCode, busy, onConfirm, onCancel }: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onCancel(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onCancel]);

  const domains = manifest.permissions.network;
  return (
    <Dialog role="alertdialog" labelledBy="theme-install-title" className="theme-install">
      <strong id="theme-install-title">Theme installieren?</strong>
      <div className="theme-install-head">
        <span className="theme-install-name">{manifest.name}</span>
        {manifest.author && <span className="muted">von {manifest.author}</span>}
        {manifest.version && <span className="muted">Version {manifest.version}</span>}
      </div>
      {hasCode && (
        <div className="notice warn theme-install-code">
          <strong>Dieses Theme führt Code aus</strong>
          <p>Der Code läuft mit vollen Rechten in Nexus. Installiere es nur aus einer Quelle, der du vertraust.</p>
        </div>
      )}
      <div>
        <span id="theme-install-domains">Angefragte Domains</span>
        {domains.length > 0 ? (
          <ul aria-labelledby="theme-install-domains">
            {domains.map((domain) => <li key={domain}>{domain}</li>)}
          </ul>
        ) : (
          <p className="muted">Keine fremden Domains</p>
        )}
      </div>
      <div className="actions">
        <Button variant="primary" disabled={busy} onClick={onConfirm}>
          Installieren
        </Button>
        <Button disabled={busy} onClick={onCancel}>
          Abbrechen
        </Button>
      </div>
    </Dialog>
  );
}
