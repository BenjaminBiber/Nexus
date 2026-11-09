// Lädt die Minecraft-Ressourcen beim ersten Start über den Asset-Proxy von Nexus aus dem Client-JAR von Mojang.
// Mojang-Dateien liegen nie im Paket; der Proxy cacht sie unter ~/.nexus/themes/minecraft/cache/.
import { themeAssetUrl } from '@nexus/theme-sdk';
import { useCallback, useEffect, useState } from 'react';
import { resolveIndexUrls } from './assetIndex.js';
import { NAV_ITEM_TEXTURE } from './navIcons.js';
import { resolveSoundUrls, type SoundUrls } from './sounds.js';

/** id aus manifest.json; Domain-Freigabe und Cache des Proxys gelten je Theme */
export const THEME_ID = 'minecraft';

/** Feste Release-Version: die Pfade der Texturen im JAR ändern sich zwischen Versionen */
export const MINECRAFT_VERSION = '1.21.4';

/** Versionsliste von Mojang */
export const MANIFEST_URL = 'https://piston-meta.mojang.com/mc/game/version_manifest_v2.json';

export const FONT_ATLAS = 'assets/minecraft/textures/font/ascii.png';
export const BUTTON = 'assets/minecraft/textures/gui/sprites/widget/button.png';
export const BUTTON_HIGHLIGHTED = 'assets/minecraft/textures/gui/sprites/widget/button_highlighted.png';
export const HOTBAR = 'assets/minecraft/textures/gui/sprites/hud/hotbar.png';
export const HOTBAR_SELECTION = 'assets/minecraft/textures/gui/sprites/hud/hotbar_selection.png';
export const LOGO = 'assets/minecraft/textures/gui/title/minecraft.png';

/** Panorama und Overlay liegen im Asset-Index, nicht im JAR (dort nur 1×1-Platzhalter) */
export const panorama = (i: number) => `minecraft/textures/gui/title/background/panorama_${i}.png`;
export const PANORAMA_OVERLAY = 'minecraft/textures/gui/title/background/panorama_overlay.png';

/** Einträge des Asset-Index, die das Theme braucht: Schlüssel = Pfad im Index */
export const INDEX_ENTRIES: readonly string[] = [...[0, 1, 2, 3, 4, 5].map(panorama), PANORAMA_OVERLAY];

/** Einträge im Client-JAR, die das Theme braucht; nur sie zählen zum Fortschritt */
export const MINECRAFT_ENTRIES: readonly string[] = [
  FONT_ATLAS,
  BUTTON,
  BUTTON_HIGHLIGHTED,
  HOTBAR,
  HOTBAR_SELECTION,
  LOGO,
  ...Object.values(NAV_ITEM_TEXTURE),
];

/** Fehler beim Laden; `offline` heißt: Mojang nicht erreichbar, erneut versuchen hilft */
export interface MinecraftResourceError {
  status: number | null;
  code: string;
  message: string;
  offline: boolean;
}

export interface MinecraftResources {
  phase: 'manifest' | 'jar' | 'entries' | 'ready' | 'error';
  done: number;
  total: number;
  /** Proxy-URL je Eintrag aus MINECRAFT_ENTRIES, vollständig erst bei `ready`; die Einträge aus INDEX_ENTRIES kommen nebenher dazu */
  urls: Record<string, string>;
  /** Proxy-URL je Ton; leer, solange der Asset-Index fehlt (dann bleibt das Theme still) */
  soundUrls: SoundUrls;
  error:MinecraftResourceError | null;
  retry: () => void;
}

class ResourceError extends Error implements MinecraftResourceError {
  constructor(
    readonly status: number | null,
    readonly code: string,
    message: string,
    readonly offline: boolean,
  ) {
    super(message);
  }
}

// Fehlerantwort des Proxys (`{ code, message }`) in einen verständlichen Fehler übersetzen
async function proxyError(res: Response): Promise<ResourceError> {
  let code = `http_${res.status}`;
  let detail = res.statusText;
  try {
    const body = (await res.json()) as { code?: unknown; message?: unknown };
    if (typeof body.code === 'string') code = body.code;
    if (typeof body.message === 'string') detail = body.message;
  } catch {
    // kein JSON
  }
  if (res.status === 403) {
    return new ResourceError(403, code, `Fehlende Domain-Freigabe für Mojang im Manifest (permissions.network): ${detail}`, false);
  }
  if (res.status === 502 || res.status === 504) return new ResourceError(res.status, code, `Mojang nicht erreichbar: ${detail}`, true);
  return new ResourceError(res.status, code, detail || `HTTP ${res.status}`, false);
}

// Eine Ressource über den Proxy anfragen; loggt den Cache-Kopf als Beleg für Treffer/Fehlschlag
async function proxied(proxyUrl: string, label: string, signal?: AbortSignal): Promise<Response> {
  let res: Response;
  try {
    res = await fetch(proxyUrl, { signal });
  } catch (e) {
    if (signal?.aborted) throw e;
    throw new ResourceError(null, 'network', e instanceof Error ? e.message : 'Netzwerkfehler', true);
  }
  if (!res.ok) throw await proxyError(res);
  console.info(`[minecraft] ${label}`, { cache: res.headers.get('X-Nexus-Cache') });
  return res;
}

/** Versions-JSON, soweit das Theme sie liest */
export interface VersionJson {
  downloads?: { client?: { url?: string } };
  assetIndex?: { url?: string };
}

/** URL des Client-JARs einer Version: Versionsliste → Versions-JSON → `downloads.client.url` */
export async function resolveClientJar(themeId: string, version: string, signal?: AbortSignal): Promise<string> {
  return (await resolveVersion(themeId, version, signal)).jar;
}

// Versions-JSON und JAR-URL; die Versions-JSON dient danach auch der Sound-Auflösung
async function resolveVersion(themeId: string, version: string, signal?: AbortSignal): Promise<{ jar: string; meta: VersionJson }> {
  const manifest = (await (await proxied(themeAssetUrl(themeId, MANIFEST_URL), 'Versionsliste', signal)).json()) as {
    versions?: { id: string; url: string }[];
  };
  const entry = manifest.versions?.find((v) => v.id === version);
  if (!entry) throw new ResourceError(null, 'version_not_found', `Minecraft ${version} steht nicht in der Versionsliste von Mojang.`, false);
  const meta = (await (await proxied(themeAssetUrl(themeId, entry.url), `Versions-JSON ${version}`, signal)).json()) as VersionJson;
  const jar = meta.downloads?.client?.url;
  if (!jar) throw new ResourceError(null, 'client_jar_missing', `Versions-JSON ${version} nennt kein Client-JAR.`, false);
  return { jar, meta };
}

type State = Omit<MinecraftResources, 'retry'>;

const initial = (): State => ({ phase: 'manifest', done: 0, total: MINECRAFT_ENTRIES.length, urls: {}, soundUrls: {}, error: null });

/** Lädt alle Einträge; `retry` startet nach einem Fehler neu */
export function useMinecraftResources(): MinecraftResources {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<State>(initial);

  useEffect(() => {
    const controller = new AbortController();
    const signal = controller.signal;
    const update = (next: Partial<State>) => {
      if (!signal.aborted) setState((prev) => ({ ...prev, ...next }));
    };
    setState(initial());

    (async () => {
      const { jar, meta } = await resolveVersion(THEME_ID, MINECRAFT_VERSION, signal);
      // Töne nebenher: zählen nicht zum Fortschritt und halten das Gate nicht auf
      void resolveSoundUrls(THEME_ID, meta, signal).then((soundUrls) => update({ soundUrls }));
      update({ phase: 'entries' });
      const urls: Record<string, string> = {};
      // Panorama und Overlay nebenher aus dem Asset-Index; sie landen in demselben `urls`, damit die JAR-Einträge sie nicht überschreiben
      void resolveIndexUrls(THEME_ID, meta, Object.fromEntries(INDEX_ENTRIES.map((e) => [e, e])), signal).then((index) => {
        Object.assign(urls, index);
        update({ urls: { ...urls } });
      });
      // Nacheinander: der erste Eintrag lädt das JAR in den Proxy-Cache, die weiteren treffen ihn
      for (const [i, entry] of MINECRAFT_ENTRIES.entries()) {
        const url = themeAssetUrl(THEME_ID, jar, { entry });
        const res = await proxied(url, entry, signal);
        void res.body?.cancel().catch(() => {});
        urls[entry] = url;
        update({ done: i + 1, urls: { ...urls } });
      }
      update({ phase: 'ready' });
    })().catch((e: unknown) => {
      if (signal.aborted) return;
      const error =
        e instanceof ResourceError
          ? { status: e.status, code: e.code, message: e.message, offline: e.offline }
          : { status: null, code: 'unknown', message: e instanceof Error ? e.message : String(e), offline: false };
      update({ phase: 'error', error });
    });

    return () => controller.abort();
  }, [attempt]);

  const retry = useCallback(() => setAttempt((n) => n + 1), []);
  return { ...state, retry };
}
