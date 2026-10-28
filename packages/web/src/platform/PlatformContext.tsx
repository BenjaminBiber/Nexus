import { createContext, useContext, useMemo, type ReactNode } from 'react';
import type { ApiClient } from '../api/ApiClient.js';
import { useApiOrNull } from '../api/ApiContext.js';
import { createPlatform } from './createPlatform.js';
import type { Platform } from './Platform.js';

const Context = createContext<Platform | null>(null);

// Die Plattform der Umgebung; sie braucht den ApiClient, weil der Browser alles über den Server erledigt
function ofEnvironment(api: ApiClient | null): Platform {
  if (!api) throw new Error('PlatformProvider oder ApiProvider fehlt');
  return createPlatform(api);
}

// Stellt die Plattform allen Seiten zur Verfügung; ohne `value` die der Umgebung (in Tests austauschbar).
export function PlatformProvider({ value, children }: { value?: Platform; children: ReactNode }) {
  const api = useApiOrNull();
  const platform = useMemo(() => value ?? ofEnvironment(api), [value, api]);
  return <Context.Provider value={platform}>{children}</Context.Provider>;
}

// Liefert die Plattform der nächsten Provider-Ebene; ohne Provider die der Umgebung
export function usePlatform(): Platform {
  const api = useApiOrNull();
  const given = useContext(Context);
  return useMemo(() => given ?? ofEnvironment(api), [given, api]);
}
