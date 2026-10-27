import { useApi } from '../api/ApiContext.js';
import { useAsync } from './useAsync.js';

// Lädt die Liste der angemeldeten Projekte
export function useProjects() {
  const api = useApi();
  return useAsync(() => api.getProjects(), [api]);
}
