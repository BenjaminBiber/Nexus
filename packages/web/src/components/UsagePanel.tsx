import { useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import { useAsync } from '../hooks/useAsync.js';
import { USAGE_SETTING_EVENT } from './ClaudeUsageMeter.js';
import { Notice } from './settings/Notice.js';
import { SettingsPanel, useSaveStatus } from './settings/SettingsPanel.js';
import { SettingRow, SettingsSection } from './settings/SettingRow.js';

// Schalter für das Auslesen der Claude-Nutzungslimits. Aus, bis jemand die Warnung bestätigt; ein ausgeschalteter Zustand liest
// weder die Anmeldedatei noch fragt er Anthropic.
export function UsagePanel() {
  const api = useApi();
  const loaded = useAsync(() => api.getUserSettings(), [api]);
  const { state, run } = useSaveStatus();
  const [understood, setUnderstood] = useState(false);
  const settings = loaded.state.status === 'ready' ? loaded.state.data : null;
  const on = settings?.claudeUsage ?? false;

  // Speichert den Schalter und lässt den Ring in der Seitenleiste sofort nachziehen
  const save = async (claudeUsage: boolean) => {
    const saved = await run(() => api.saveUserSettings({ claudeUsage }));
    if (!saved) return;
    loaded.set(saved);
    setUnderstood(false);
    window.dispatchEvent(new Event(USAGE_SETTING_EVENT));
  };

  return (
    <SettingsPanel label="Nutzungslimits" status={state}>
      <SettingsSection>
        <SettingRow
          label="Nutzungslimits auslesen"
          hint={on ? 'Eingeschaltet: Sitzungs- und Wochenlimit stehen als Doppelring neben der Suche.' : 'Sitzungslimit (5 Stunden) und Wochenlimit als Doppelring neben der Suche.'}
          info={DETAILS}
          group
        >
          {on && (
            <button type="button" className="btn small" onClick={() => void save(false)}>
              Ausschalten
            </button>
          )}
        </SettingRow>
        {/* Vor dem Einschalten steht die Warnung offen da, damit niemand ungelesen zustimmt; Einzelheiten stehen im (i) */}
        {!on && (
          <div className="settings-card-body usage-consent">
            <Notice tone="warn" role="note" label="Warnung">
              Dafür ruft das Harness mit dem Anmelde-Token aus <code>~/.claude/.credentials.json</code> einen nicht dokumentierten Anthropic-Endpunkt auf. Anthropic kann das als unzulässig werten; bei einem Firmenkonto vorher mit IT oder Rechtsabteilung klären.
            </Notice>
            <div className="usage-consent-actions">
              <label className="option-row ack">
                <input type="checkbox" checked={understood} disabled={!settings} onChange={(e) => setUnderstood(e.target.checked)} />
                <span>Verstanden, ich möchte die Nutzungslimits auslesen.</span>
              </label>
              <button type="button" className="btn small primary" disabled={!settings || !understood} onClick={() => void save(true)}>
                Einschalten
              </button>
            </div>
          </div>
        )}
      </SettingsSection>
    </SettingsPanel>
  );
}

// Einzelheiten zum Auslesen hinter dem (i) der Zeile
const DETAILS = (
  <ul>
    <li>Das Harness liest das Anmelde-Token von Claude Code aus <code>~/.claude/.credentials.json</code> und ruft damit <code>api.anthropic.com/api/oauth/usage</code> auf.</li>
    <li>Der Endpunkt ist nicht dokumentiert. Ändert er sich oder wird er gesperrt, verschwindet der Ring.</li>
    <li>Anthropic sieht diese Anmeldung nur für Claude Code und die eigenen Apps vor und behält sich Maßnahmen ohne Vorwarnung vor.</li>
    <li>Das Token wird nur gelesen: nicht gespeichert, nicht erneuert, nicht weitergegeben. Abgefragt wird höchstens einmal pro Minute.</li>
  </ul>
);
