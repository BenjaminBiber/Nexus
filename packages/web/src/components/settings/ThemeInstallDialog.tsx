import { useEffect } from 'react';
import { Button } from '../ui/Button.js';
import { Dialog } from '../ui/Dialog.js';

// Was der Dialog vom Theme kennt; im Store ist vor dem Download nur der Index-Eintrag bekannt
type ManifestInfo = { name: string; author?: string; version?: string; permissions?: { network: string[] } };

type Props = {
  manifest: ManifestInfo;
  // 'unknown': Store-Eintrag vor dem Download, ob Code dabei ist, steht noch nicht fest
  hasCode: boolean | 'unknown';
  // Abweichende Domainliste (Update: neue Domains); null = erst nach dem Download bekannt
  domains?: string[] | null;
  title?: string;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
};

// Rückfrage vor dem Installieren eines Themes (Vertrauensmodell D2): zeigt Herkunft, Code-Hinweis und angefragte Domains
export function ThemeInstallDialog({ manifest, hasCode, domains: given, title = 'Theme installieren?', busy, onConfirm, onCancel }: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onCancel(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onCancel]);

  const domains = given === undefined ? manifest.permissions?.network ?? [] : given;
  const isUpdate = title !== 'Theme installieren?';
  return (
    <Dialog role="alertdialog" labelledBy="theme-install-title" className="theme-install">
      <strong id="theme-install-title">{title}</strong>
      <div className="theme-install-head">
        <span className="theme-install-name">{manifest.name}</span>
        {manifest.author && <span className="muted">von {manifest.author}</span>}
        {manifest.version && <span className="muted">Version {manifest.version}</span>}
      </div>
      {hasCode === true && (
        <div className="notice warn theme-install-code">
          <strong>{isUpdate ? 'Dieses Update führt Code aus' : 'Dieses Theme führt Code aus'}</strong>
          <p>Der Code läuft mit vollen Rechten in Nexus. Installiere es nur aus einer Quelle, der du vertraust.</p>
        </div>
      )}
      {hasCode === 'unknown' && (
        <div className="notice warn theme-install-code">
          <p>Themes aus dem Store können Code ausführen. Installiere nur aus Repos, denen du vertraust.</p>
        </div>
      )}
      <div>
        <span id="theme-install-domains">{isUpdate ? 'Neue Domains' : 'Angefragte Domains'}</span>
        {domains === null ? (
          <p className="muted">Domains werden nach dem Download geprüft</p>
        ) : domains.length > 0 ? (
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
