import { COMPACT_CARDS_EVENT } from '../../dbboard/compactCards.js';
import { SettingsPanel } from './SettingsPanel.js';
import { SettingRow, SettingsSection } from './SettingRow.js';
import { SnippetsSection } from './SnippetsSection.js';
import { useUserSettings } from './useUserSettings.js';

// Auswahl für das Archiv der Spalte „Erledigt“ in Tagen; 0 heißt nie
const ARCHIVE_DAYS = [0, 7, 14, 30];

// Beschriftung einer Archiv-Dauer
function archiveLabel(days: number) {
  if (days === 0) return 'Nie';
  return days === 1 ? 'nach 1 Tag' : `nach ${days} Tagen`;
}

// Darstellung › Oberfläche: Aussehen des Boards und eigene CSS-Schnipsel, unabhängig vom gewählten Theme.
export function InterfacePanel() {
  const { settings, save, state } = useUserSettings();
  // Eine gespeicherte Dauer außerhalb der Auswahl (über die API gesetzt) erscheint als eigene Option
  const archiveDays = settings?.doneArchiveDays ?? 0;
  const archiveOptions = ARCHIVE_DAYS.includes(archiveDays) ? ARCHIVE_DAYS : [...ARCHIVE_DAYS, archiveDays].sort((a, b) => a - b);

  // Speichert die kompakte Ansicht und lässt ein offenes Board sofort nachziehen
  const toggleCompact = async (compactCards: boolean) => {
    if (await save({ compactCards })) window.dispatchEvent(new Event(COMPACT_CARDS_EVENT));
  };

  return (
    <SettingsPanel label="Oberfläche" status={state}>
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
        <SettingRow label="Erledigte Tickets archivieren" hint="Gemergte Tickets verschwinden danach vom Board. Die Suche findet sie weiter.">
          <select disabled={!settings} value={archiveDays} onChange={(e) => void save({ doneArchiveDays: Number(e.target.value) })}>
            {archiveOptions.map((days) => (
              <option key={days} value={days}>{archiveLabel(days)}</option>
            ))}
          </select>
        </SettingRow>
      </SettingsSection>
      <SnippetsSection />
    </SettingsPanel>
  );
}
