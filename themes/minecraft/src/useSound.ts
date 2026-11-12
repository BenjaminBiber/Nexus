// Spielt Klick- und Menü-Töne, solange die Theme-Einstellung „Sounds“ an ist; fehlende Töne und Autoplay-Sperren bleiben still
import { useThemeSettings } from '@nexus/theme-sdk';
import { useCallback } from 'react';
import { useSounds } from './context.js';
import type { MINECRAFT_SETTINGS } from './settings.js';
import type { SoundKey } from './sounds.js';

/** Feste Lautstärke der Töne */
const VOLUME = 0.5;

/** Liefert `play(key)`; tut nichts, wenn Sounds aus sind, die Einstellungen noch laden oder der Ton fehlt */
export function useSound(): (key: SoundKey) => void {
  const { values, loading } = useThemeSettings<typeof MINECRAFT_SETTINGS>();
  // Ein Audio je URL, erst beim ersten Abspielen (nach einer Nutzergeste) angelegt
  const { urls, players } = useSounds();
  const enabled = !loading && values.sounds !== false;

  return useCallback(
    (key: SoundKey) => {
      const url = urls[key];
      if (!enabled || !url || typeof Audio === 'undefined') return;
      try {
        let audio = players.get(url);
        if (!audio) {
          audio = new Audio(url);
          audio.volume = VOLUME;
          players.set(url, audio);
        }
        audio.currentTime = 0;
        void audio.play()?.catch(() => {});
      } catch {
        // Ton ist Beiwerk; Fehler dürfen die Bedienung nicht stören
      }
    },
    [enabled, urls, players],
  );
}
