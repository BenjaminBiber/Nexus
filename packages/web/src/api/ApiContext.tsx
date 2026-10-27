import { createContext, useContext, type ReactNode } from 'react';
import type { ApiClient } from './ApiClient.js';

const Context = createContext<ApiClient | null>(null);

// Stellt den ApiClient allen Seiten zur Verfügung (in Tests austauschbar).
export function ApiProvider({ api, children }: { api: ApiClient; children: ReactNode }) {
  return <Context.Provider value={api}>{children}</Context.Provider>;
}

// Liefert den ApiClient der nächsten Provider-Ebene
export function useApi(): ApiClient {
  const api = useContext(Context);
  if (!api) throw new Error('ApiProvider fehlt');
  return api;
}
