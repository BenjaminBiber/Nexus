import { useState } from 'react';
import { SettingsPanel } from './settings/SettingsPanel.js';
import { SettingsSection } from './settings/SettingRow.js';

// Zugriffs-Token des Projekts: erst auf Wunsch sichtbar, mit Kopieren-Knopf.
export function TokenPanel({ token }: { token: string }) {
  const [shown, setShown] = useState(false);

  // Kopiert das Token in die Zwischenablage
  const copy = () => void navigator.clipboard?.writeText(token);

  return (
    <SettingsPanel label="Token">
      <SettingsSection title="Zugriffs-Token" description="Mit diesem Token melden sich deine Claude-Sitzungen beim Harness an. Gib es nicht weiter.">
        <div className="token-row">
          <span className="mono token-value">{shown ? token : '••••••••••••••••••••'}</span>
          {shown && (
            <button type="button" className="btn" aria-label="Token kopieren" onClick={copy}>
              Kopieren
            </button>
          )}
          <button type="button" className="btn" aria-label={shown ? 'Token verbergen' : 'Token anzeigen'} onClick={() => setShown(!shown)}>
            {shown ? 'Verbergen' : 'Anzeigen'}
          </button>
        </div>
      </SettingsSection>
    </SettingsPanel>
  );
}
