// Spielschrift beim Start: Schrift-JSONs und Atlanten laden, die TrueType-Schrift bauen (oder aus der Cache API holen)
// und als Web-Schrift `Minecraft` registrieren. Scheitert etwas, bleibt die Rückfallschrift aus theme.css.
import { useEffect, useState } from 'react';
import { FONT_ACCENTED, FONT_ATLAS, FONT_DEFAULT_JSON, FONT_NONLATIN, FONT_SPACE_JSON, MINECRAFT_VERSION } from '../resources.js';
import { buildFont, FONT_FAMILY, type AtlasImage } from './buildFont.js';
import { fontCacheKey, readCachedFont, writeCachedFont } from './fontCache.js';
import { parseGlyphMap, type FontJson } from './glyphMap.js';

/** `idle` = noch nicht alle Quellen da, `building` = Bauen oder Laden läuft, `failed` = Rückfallschrift */
export type FontState = 'idle' | 'building' | 'ready' | 'failed';

/** Quellen der Schrift; die Reihenfolge bestimmt den Cache-Schlüssel (JSONs zuerst) */
const ENTRIES = [FONT_DEFAULT_JSON, FONT_SPACE_JSON, FONT_ATLAS, FONT_ACCENTED, FONT_NONLATIN] as const;

async function fetchJson(url: string, signal: AbortSignal): Promise<FontJson> {
  const res = await fetch(url, { signal });
  if (!res.ok) throw new Error(`HTTP ${res.status} für ${url}`);
  return (await res.json()) as FontJson;
}

// Atlas per `Image` und Canvas zu Pixeln; wirft, wenn das Bild nicht lädt oder kein 2D-Kontext da ist
function loadAtlas(url: string): Promise<AtlasImage> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      try {
        const canvas = document.createElement('canvas');
        canvas.width = img.naturalWidth;
        canvas.height = img.naturalHeight;
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('kein 2D-Kontext für den Schrift-Atlas');
        ctx.drawImage(img, 0, 0);
        resolve(ctx.getImageData(0, 0, canvas.width, canvas.height));
      } catch (e) {
        reject(e instanceof Error ? e : new Error(String(e)));
      }
    };
    img.onerror = () => reject(new Error(`Schrift-Atlas ${url} konnte nicht geladen werden`));
    img.src = url;
  });
}

/** Baut die Spielschrift aus den Proxy-URLs (`resources.urls`) und meldet den Zustand der Registrierung */
export function useMinecraftFont(urls: Record<string, string>): FontState {
  const [state, setState] = useState<FontState>('idle');
  // Nur die fünf Schrift-Quellen steuern den Lauf; andere Ressourcen kommen nebenher dazu
  const signature = ENTRIES.map((entry) => urls[entry] ?? '').join('|');

  useEffect(() => {
    const sources = signature.split('|');
    if (sources.some((url) => !url)) return;
    const controller = new AbortController();
    let live = true;
    let face: FontFace | null = null;
    setState('building');

    (async () => {
      const key = fontCacheKey(MINECRAFT_VERSION, sources);
      let bytes = await readCachedFont(key);
      if (!bytes) {
        const [defaultJson, spaceJson] = await Promise.all([
          fetchJson(sources[0], controller.signal),
          fetchJson(sources[1], controller.signal),
        ]);
        const map = parseGlyphMap(defaultJson, spaceJson);
        const urlOf = new Map(ENTRIES.map((entry, i) => [entry as string, sources[i]]));
        const atlases: Record<string, AtlasImage> = {};
        for (const atlas of map.atlases) {
          const url = urlOf.get(atlas);
          // Ein Atlas, der nicht unter den URLs steht (etwa ein weiterer Provider), wird übersprungen
          if (!url) continue;
          atlases[atlas] = await loadAtlas(url);
        }
        bytes = buildFont(atlases, map);
        await writeCachedFont(key, bytes);
      }
      if (!live) return;
      face = new FontFace(FONT_FAMILY, bytes);
      await face.load();
      if (!live) return;
      document.fonts.add(face);
      setState('ready');
    })().catch((e: unknown) => {
      if (!live) return;
      console.warn('[minecraft] Schrift', e);
      setState('failed');
    });

    return () => {
      live = false;
      controller.abort();
      if (face) document.fonts.delete(face);
    };
  }, [signature]);

  return state;
}
