// Proxy-URLs der geladenen Minecraft-Ressourcen für Schrift, Logo und Item-Symbole unterhalb der Shell
import { createContext, useContext, type ReactNode } from 'react';

const ResourcesContext = createContext<Record<string, string>>({});

export function ResourcesProvider({ urls, children }: { urls: Record<string, string>; children?: ReactNode }) {
  return <ResourcesContext.Provider value={urls}>{children}</ResourcesContext.Provider>;
}

/** Proxy-URL eines JAR-Eintrags oder undefined, solange er fehlt (Laden, offline, außerhalb der Shell) */
export function useResourceUrl(entry: string): string | undefined {
  return useContext(ResourcesContext)[entry];
}
