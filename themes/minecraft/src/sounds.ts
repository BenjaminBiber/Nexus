// Klick- und Menü-Töne: liegen nicht im Client-JAR, sondern im Asset-Index der Version auf resources.download.minecraft.net.
// Den Index liest assetIndex.ts; fehlt etwas, bleibt das Theme still.
import { resolveIndexUrls } from './assetIndex.js';

export { RESOURCES_URL } from './assetIndex.js';

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
  return resolveIndexUrls(themeId, versionJson, MINECRAFT_SOUNDS, signal);
}
