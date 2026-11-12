import { useId } from 'react';
import { createPortal } from 'react-dom';
import { DbModal } from '../db/DbModal.js';
import { Button } from '../ui/Button.js';

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

// Rückfrage vor dem Installieren eines Themes (Vertrauensmodell D2) als eigener Dialog über dem Store: zeigt Herkunft,
// Code-Hinweis und angefragte Domains. Liegt per Portal im body; Tasten bleiben in ihm, damit Esc und Tab den
// darunterliegenden Dialog nicht erreichen.
export function ThemeInstallDialog({ manifest, hasCode, domains: given, title = 'Theme installieren?', busy, onConfirm, onCancel }: Props) {
  const titleId = useId();
  const domainsId = useId();
  const domains = given === undefined ? manifest.permissions?.network ?? [] : given;
  const isUpdate = title !== 'Theme installieren?';
  const meta = [manifest.author && `von ${manifest.author}`, manifest.version && `Version ${manifest.version}`].filter(Boolean) as string[];

  return createPortal(
    <div className="theme-install-layer" onKeyDown={(e) => e.stopPropagation()}>
      <DbModal role="alertdialog" labelledBy={titleId} onEscape={onCancel} onBackdrop={busy ? undefined : onCancel} className="panel db-modal theme-confirm theme-install-modal">
        <div className={`theme-confirm-icon${hasCode === true ? ' is-code' : ' is-install'}`} aria-hidden="true">
          <span className="material-icons">{hasCode === true ? 'code' : 'download'}</span>
        </div>
        <h2 id={titleId}>{title}</h2>
        <div className="theme-install-head">
          <span className="theme-install-name">{manifest.name}</span>
          {meta.map((m) => <span key={m} className="muted">{m}</span>)}
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
        <div className="theme-install-domains">
          <span id={domainsId} className="theme-install-label">{isUpdate ? 'Neue Domains' : 'Angefragte Domains'}</span>
          {domains === null ? (
            <p className="muted">Domains werden nach dem Download geprüft</p>
          ) : domains.length > 0 ? (
            <ul aria-labelledby={domainsId}>
              {domains.map((domain) => <li key={domain}>{domain}</li>)}
            </ul>
          ) : (
            <p className="muted">Keine fremden Domains</p>
          )}
        </div>
        <div className="actions">
          <Button disabled={busy} onClick={onCancel}>
            Abbrechen
          </Button>
          <Button variant="primary" disabled={busy} onClick={onConfirm}>
            {busy ? 'Installiere …' : 'Installieren'}
          </Button>
        </div>
      </DbModal>
    </div>,
    document.body,
  );
}
