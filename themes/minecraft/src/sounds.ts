// Klick- und Menü-Töne: liegen nicht im Client-JAR, sondern im Asset-Index der Version auf resources.download.minecraft.net.
// Alles läuft über den Asset-Proxy; fehlt etwas, bleibt das Theme still.
import { themeAssetUrl } from '@nexus/theme-sdk';

/** Objekt-Server von Mojang für die Dateien aus dem Asset-Index */
export const RESOURCES_URL = 'https://resources.download.minecraft.net';

/** Schlüssel im Asset-Index je Ton */
export const MINECRAFT_SOUNDS = {
  click: 'minecraft/sounds/random/click.ogg',
  menuOpen: 'minecraft/sounds/random/wood_click.ogg',
  hotbar: 'minecraft/sounds/random/pop.ogg',
} as const;

export type SoundKey = keyof typeof MINECRAFT_SOUNDS;

/** Proxy-URL je Ton; fehlende Töne fehlen */
export type SoundUrls = Partial<Record<SoundKey, string>>;

/** Liest den Asset-Index aus der Versions-JSON und liefert die Proxy-URLs der Töne; jeder Fehler ergibt ein leeres Ergebnis */
export async function resolveSoundUrls(themeId: string, versionJson: { assetIndex?: { url?: string } }, signal?: AbortSignal): Promise<SoundUrls> {
  const index = versionJson.assetIndex?.url;
  if (!index) return {};
  try {
    const res = await fetch(themeAssetUrl(themeId, index), { signal });
    if (!res.ok) return {};
    const objects = ((await res.json()) as { objects?: Record<string, { hash?: string }> }).objects ?? {};
    const urls: SoundUrls = {};
    for (const [key, path] of Object.entries(MINECRAFT_SOUNDS) as [SoundKey, string][]) {
      const hash = objects[path]?.hash;
      if (hash && /^[0-9a-f]{40}$/.test(hash)) urls[key] = themeAssetUrl(themeId, `${RESOURCES_URL}/${hash.slice(0, 2)}/${hash}`);
    }
    return urls;
  } catch {
    return {};
  }
}
