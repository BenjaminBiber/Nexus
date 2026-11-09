import { SettingRow, SettingsSection } from '../components/settings/SettingRow.js';
import { getTheme } from './registry.js';
import { useDisplayedTheme } from './useDisplayedTheme.js';
import { useThemeSettingsState } from './useThemeSettingsState.js';

// Eigene Einstellungen des angezeigten Themes als Schalter bzw. Auswahl; ohne settingsSchema nichts.
// `run` ist der Speicher-Status des Panels
export function ThemeSettingsSection({ run }: { run: <T>(task: () => Promise<T>) => Promise<T | undefined> }) {
  const themeId = useDisplayedTheme();
  const schema = getTheme(themeId)?.settingsSchema;
  const { values, loading, set } = useThemeSettingsState(themeId);
  if (!schema || Object.keys(schema).length === 0) return null;
  const save = (key: string, value: boolean | string) => void run(() => set(key, value));
  return (
    <SettingsSection title="Einstellungen des Themes">
      {Object.entries(schema).map(([key, setting]) => (
        <SettingRow key={key} label={setting.label} hint={setting.hint}>
          {setting.type === 'boolean' ? (
            <input type="checkbox" aria-label={setting.label} disabled={loading} checked={values[key] === true} onChange={(e) => save(key, e.target.checked)} />
          ) : (
            <select aria-label={setting.label} disabled={loading} value={String(values[key] ?? setting.default)} onChange={(e) => save(key, e.target.value)}>
              {setting.options.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          )}
        </SettingRow>
      ))}
    </SettingsSection>
  );
}
