import { useApi } from '../api/ApiContext.js';
import { useAsync } from './useAsync.js';

// Lädt die Projektliste und die Einstellungen des aktuellen Projekts
export function useSettings(pid: string) {
  const api = useApi();
  const projects = useAsync(() => api.getProjects(), [api]);
  const settings = useAsync(() => api.getSettings(pid), [api, pid]);
  return { projects, settings };
}
