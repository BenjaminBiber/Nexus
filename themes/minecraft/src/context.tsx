// Proxy-URLs der geladenen Minecraft-Ressourcen für Schrift, Logo, Item-Symbole und Töne unterhalb der Shell
import { createContext, useContext, useMemo, useState, type ReactNode } from 'react';
import type { SoundUrls } from './sounds.js';

/** Töne der Shell: Proxy-URLs und die je URL einmal angelegten Audio-Objekte */
interface Sounds {
  urls: SoundUrls;
  players: Map<string, HTMLAudioElement>;
}

const ResourcesContext = createContext<Record<string, string>>({});
const SoundsContext = createContext<Sounds>({ urls: {}, players: new Map() });

const NO_SOUNDS: SoundUrls = {};

export function ResourcesProvider({
  urls,
  soundUrls = NO_SOUNDS,
  children,
}: {
  urls: Record<string, string>;
  soundUrls?: SoundUrls;
  children?: ReactNode;
}) {
  const [players] = useState(() => new Map<string, HTMLAudioElement>());
  const sounds = useMemo(() => ({ urls: soundUrls, players }), [soundUrls, players]);
  return (
    <ResourcesContext.Provider value={urls}>
      <SoundsContext.Provider value={sounds}>{children}</SoundsContext.Provider>
    </ResourcesContext.Provider>
  );
}

/** Proxy-URL eines JAR-Eintrags oder undefined, solange er fehlt (Laden, offline, außerhalb der Shell) */
export function useResourceUrl(entry: string): string | undefined {
  return useContext(ResourcesContext)[entry];
}

/** Proxy-URLs der Töne (leer, solange der Asset-Index fehlt) und der Audio-Cache der Shell */
export function useSounds(): Sounds {
  return useContext(SoundsContext);
}
