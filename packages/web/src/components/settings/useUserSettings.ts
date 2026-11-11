import { useApi } from '../../api/ApiContext.js';
import type { UserSettingsDto } from '../../api/types.js';
import { useAsync } from '../../hooks/useAsync.js';
import { useSaveStatus } from './SettingsPanel.js';

// Persönliche Einstellungen (~/.nexus/settings.json) mit Speicherstand: save übernimmt den gespeicherten Stand
export function useUserSettings() {
  const api = useApi();
  const loaded = useAsync(() => api.getUserSettings(), [api]);
  const { state, run } = useSaveStatus();
  const settings = loaded.state.status === 'ready' ? loaded.state.data : null;

  const save = async (patch: Partial<UserSettingsDto>) => {
    const saved = await run(() => api.saveUserSettings(patch));
    if (saved) loaded.set(saved);
    return saved;
  };

  return { settings, save, state, run };
}
