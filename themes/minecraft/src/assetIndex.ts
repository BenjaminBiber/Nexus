// Liest den Asset-Index einer Version (Versions-JSON → assetIndex.url → objects) und liefert die Proxy-URLs je Schlüssel.
// Alles läuft über den Asset-Proxy; jeder Fehler ergibt ein leeres Ergebnis, das Theme bleibt dann ohne diese Ressourcen bedienbar.
import { themeAssetUrl } from '@nexus/theme-sdk';

/** Objekt-Server von Mojang für die Dateien aus dem Asset-Index */
export const RESOURCES_URL = 'https://resources.download.minecraft.net';

/** Proxy-URL je Schlüssel aus `keys`; `keys` bildet Schlüssel auf den Pfad im Asset-Index ab */
export async function resolveIndexUrls<K extends string>(
  themeId: string,
  versionJson: { assetIndex?: { url?: string } },
  keys: Record<K, string>,
  signal?: AbortSignal,
): Promise<Partial<Record<K, string>>> {
  const index = versionJson.assetIndex?.url;
  if (!index) return {};
  try {
    const res = await fetch(themeAssetUrl(themeId, index), { signal });
    if (!res.ok) return {};
    const objects = ((await res.json()) as { objects?: Record<string, { hash?: string }> }).objects ?? {};
    const urls: Partial<Record<K, string>> = {};
    for (const [key, path] of Object.entries(keys) as [K, string][]) {
      const hash = objects[path]?.hash;
      if (hash && /^[0-9a-f]{40}$/.test(hash)) urls[key] = themeAssetUrl(themeId, `${RESOURCES_URL}/${hash.slice(0, 2)}/${hash}`);
    }
    return urls;
  } catch {
    return {};
  }
}
