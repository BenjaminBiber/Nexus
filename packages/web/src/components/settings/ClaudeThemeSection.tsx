import { useEffect, useRef } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { ClaudeThemeStatus } from '../../api/generated/themes.js';
import { useAsync } from '../../hooks/useAsync.js';
import { SettingRow, SettingsSection } from './SettingRow.js';

type Props = {
  /** Einstellung `claudeTheme` (fehlt sie, gilt an) */
  enabled: boolean;
  disabled: boolean;
  onToggle: (on: boolean) => void;
  /** Ändert sich der Wert, wird der Zustand neu geladen (Theme-Wechsel, Schalter, Theme-Einstellungen) */
  reloadKey: unknown;
};

/** Satz unter dem Schalter zum Zustand des Claude-Themes */
export function claudeThemeHint(status: ClaudeThemeStatus | undefined, enabled: boolean): string {
  if (!enabled) return 'Aus: Claude Desktop behält sein eigenes Aussehen.';
  if (!status) return 'Zustand wird geladen …';
  switch (status.state) {
    case 'active':
      return `In Claude aktiv: ${status.themeName ?? status.themeId ?? 'Theme'}`;
    case 'none':
      return 'Dieses Theme bringt kein Claude-Theme mit.';
    case 'foreign':
      return `${status.path} gehört nicht Nexus – umbenennen oder löschen.`;
    case 'error':
      return status.message ?? 'Das Claude-Theme konnte nicht abgelegt werden.';
  }
}

// Darstellung › Themes › Claude Desktop: Schalter, ob Claude Desktop das Claude-Theme des aktiven Themes übernimmt,
// darunter der Zustand aus GET /api/claude-theme
export function ClaudeThemeSection({ enabled, disabled, onToggle, reloadKey }: Props) {
  const api = useApi();
  const status = useAsync(() => api.getClaudeTheme(), [api]);
  const { refresh } = status;
  // Nach Theme-Wechsel, Schalter oder Theme-Einstellungen still nachladen (die erste Ladung macht useAsync)
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    void refresh();
  }, [reloadKey, refresh]);
  const data = status.state.status === 'ready' ? status.state.data : undefined;
  const hint = status.state.status === 'error' ? `Zustand nicht abrufbar: ${status.state.message}` : claudeThemeHint(data, enabled);
  const warn = enabled && (data?.state === 'foreign' || data?.state === 'error');
  return (
    <SettingsSection
      title="Claude Desktop"
      info="Bringt das aktive Theme ein Claude-Theme mit, legt Nexus es als Mod nach skills/nexus-theme im Claude-Ordner. Claude Desktop lädt es von selbst neu. Wechselst du das Theme, zieht Claude mit; die Einstellungen des Themes gelten dort ebenfalls."
    >
      <SettingRow label="Claude übernimmt das Theme" hint={<span className={warn ? 'claude-theme-warn' : undefined}>{hint}</span>}>
        <input type="checkbox" role="switch" disabled={disabled} checked={enabled} onChange={(e) => onToggle(e.target.checked)} />
      </SettingRow>
    </SettingsSection>
  );
}
