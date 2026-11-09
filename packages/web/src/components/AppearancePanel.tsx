import { useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { ThemeMode, UserSettingsDto } from '../api/types.js';
import { COMPACT_CARDS_EVENT } from '../dbboard/compactCards.js';
import { useAsync } from '../hooks/useAsync.js';
import { unregisterTheme } from '../theme/registry.js';
import { applyTheme } from '../theme/theme.js';
import { ThemeSettingsSection } from '../theme/ThemeSettingsSection.js';
import { SettingsPanel, useSaveStatus } from './settings/SettingsPanel.js';
import { SettingRow, SettingsSection } from './settings/SettingRow.js';
import { SnippetsSection } from './settings/SnippetsSection.js';
import { StoreRepos } from './settings/StoreRepos.js';
import { ThemeStore } from './settings/ThemeStore.js';
import { ThemeManager } from './settings/ThemeManager.js';

const THEMES: Array<{ mode: ThemeMode; label: string; hint: string; icon: string }> = [
  { mode: 'system', label: 'System', hint: 'Folgt der Einstellung von Windows.', icon: 'M3 4h18v12H3zM8 20h8M12 16v4' },
  { mode: 'light', label: 'Hell', hint: 'Immer hell.', icon: 'M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4' },
  { mode: 'dark', label: 'Dunkel', hint: 'Immer dunkel.', icon: 'M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z' },
  { mode: 'signal', label: 'Signal', hint: 'Dunkle Seitenleiste, Spalten und Karten im Ton ihres Status.', icon: 'M4 4h4v16H4zM10 4h4v10h-4zM16 4h4v13h-4z' },
  { mode: 'pastel', label: 'Pastell', hint: 'Wie aus Papier gebastelt: Tonpapier und abgerissene Zettel.', icon: 'M12 3a9 9 0 1 0 0 18c1 0 1.5-.8 1.5-1.6 0-1.1-.9-1.4-.9-2.4 0-.9.7-1.5 1.6-1.5H17a4 4 0 0 0 4-4c0-4.7-4-8.5-9-8.5zM7.5 12.5h.01M9.5 8h.01M14.5 8h.01' },
  { mode: 'win95', label: 'Windows 95', hint: 'Desktop, Fenster mit Titelleiste, Startmenü und Taskleiste.', icon: 'M3 5.5l7-1v7H3zM12 4.2l9-1.2v8.5h-9zM3 13h7v7l-7-1zM12 13h9v8.5l-9-1.2z' },
];

/** Symbol für installierte Themes (Palette) */
const INSTALLED_ICON = 'M12 3a9 9 0 1 0 0 18c1 0 1.5-.8 1.5-1.6 0-1.1-.9-1.4-.9-2.4 0-.9.7-1.5 1.6-1.5H17a4 4 0 0 0 4-4c0-4.7-4-8.5-9-8.5z';

// Farbschema der ganzen Harness-Oberfläche und Aussehen des Boards (gespeichert in ~/.harness/settings.json).
export function AppearancePanel() {
  const api = useApi();
  const loaded = useAsync(() => api.getUserSettings(), [api]);
  // Installierte Themes; schlägt das Laden fehl, bleiben nur die eingebauten
  const installed = useAsync(() => api.listThemes(), [api]);
  const [storeKey, setStoreKey] = useState(0);
  const builtin = new Set(THEMES.map((theme) => theme.mode));
  const themes = [
    ...THEMES,
    ...(installed.state.status === 'ready' ? installed.state.data : [])
      .filter((manifest) => !builtin.has(manifest.id))
      .map((manifest) => ({ mode: manifest.id, label: manifest.name, hint: `${manifest.author} · Version ${manifest.version}`, icon: INSTALLED_ICON })),
  ];
  const { state, run } = useSaveStatus();
  const settings = loaded.state.status === 'ready' ? loaded.state.data : null;

  // Speichert eine Änderung und übernimmt den gespeicherten Stand
  const save = async (patch: Partial<UserSettingsDto>) => {
    const saved = await run(() => api.saveUserSettings(patch));
    if (saved) loaded.set(saved);
    return saved;
  };

  // Speichert das Schema und setzt es sofort
  const choose = async (mode: ThemeMode) => {
    const saved = await save({ theme: mode });
    if (saved) applyTheme(saved.theme, installed.state.status === 'ready' ? installed.state.data : undefined);
  };

  // Das entfernte Theme wird abgemeldet (seine Slots greifen nicht mehr); war es aktiv, gilt danach „System“
  const removed = async (id: string) => {
    unregisterTheme(id);
    if (settings?.theme !== id) return;
    const saved = await save({ theme: 'system' });
    if (saved) applyTheme(saved.theme);
  };

  // Speichert die kompakte Ansicht und lässt ein offenes Board sofort nachziehen
  const toggleCompact = async (compactCards: boolean) => {
    if (await save({ compactCards })) window.dispatchEvent(new Event(COMPACT_CARDS_EVENT));
  };

  return (
    <SettingsPanel label="Darstellung" status={state}>
      <SettingsSection title="Farbschema" bare>
        <fieldset className="choice-group" disabled={!settings}>
          <legend className="sr-only">Farbschema</legend>
          {themes.map(({ mode, label, hint, icon }) => (
            <label key={mode} className={`choice${settings?.theme === mode ? ' selected' : ''}`}>
              <input type="radio" name="theme" value={mode} aria-label={label} className="sr-only" checked={settings?.theme === mode} onChange={() => void choose(mode)} />
              <svg className="choice-icon" viewBox="0 0 24 24" aria-hidden="true">
                <path d={icon} />
              </svg>
              <span className="choice-label">{label}</span>
              <span className="choice-hint muted">{hint}</span>
            </label>
          ))}
        </fieldset>
      </SettingsSection>
      <ThemeManager installed={installed.state} onChanged={installed.reload} onRemoved={removed} />
      <ThemeSettingsSection run={run} />
      <StoreRepos onChanged={() => setStoreKey((k) => k + 1)} />
      <ThemeStore installed={installed.state.status === 'ready' ? installed.state.data : []} onInstalled={installed.reload} reloadKey={storeKey} />
      <SnippetsSection />
      <SettingsSection title="Board">
        <SettingRow label="Kompakte Ticket-Kacheln" hint="Tags eingeklappt, Agent nur als Claude-Logo.">
          <input
            type="checkbox"
            aria-label="Kompakte Ticket-Kacheln"
            disabled={!settings}
            checked={settings?.compactCards ?? false}
            onChange={(e) => void toggleCompact(e.target.checked)}
          />
        </SettingRow>
        <SettingRow label="Spalte „Nicht mehr nötig“ ausblenden" hint="Die Tickets darin bleiben erhalten, nur die Spalte fehlt auf dem Board.">
          <input type="checkbox" disabled={!settings} checked={settings?.hideNotNeeded ?? false} onChange={(e) => void save({ hideNotNeeded: e.target.checked })} />
        </SettingRow>
        <SettingRow label="Spalte „In Planung“ ausblenden" hint="Die Tickets darin bleiben erhalten, nur die Spalte fehlt auf dem Board.">
          <input type="checkbox" disabled={!settings} checked={settings?.hideInPlanning ?? false} onChange={(e) => void save({ hideInPlanning: e.target.checked })} />
        </SettingRow>
      </SettingsSection>
    </SettingsPanel>
  );
}
