// Himmel nach Tageszeit: die Phase folgt der lokalen Uhr oder steht fest, je nach Einstellung „Himmel“
import { useEffect, useState } from 'react';
import type { SynthSettings } from './settings.js';

export type SkyPhase = 'dawn' | 'day' | 'dusk' | 'night';

/** Phase zur lokalen Stunde: 05–09 Morgenrot, 09–17 Tag, 17–21 Dämmerung, sonst Nacht (untere Grenze eingeschlossen) */
export function skyPhase(date: Date): SkyPhase {
  const hour = date.getHours();
  if (hour >= 5 && hour < 9) return 'dawn';
  if (hour >= 9 && hour < 17) return 'day';
  if (hour >= 17 && hour < 21) return 'dusk';
  return 'night';
}

/** Aufgelöste Phase; bei „auto“ jede Minute neu berechnet, sonst die feste Einstellung */
export function useSkyPhase(setting: SynthSettings['sky']): SkyPhase {
  const [now, setNow] = useState<SkyPhase>(() => skyPhase(new Date()));

  useEffect(() => {
    if (setting !== 'auto') return;
    setNow(skyPhase(new Date()));
    // setState mit gleichem Wert rendert nicht neu, die Phase ändert sich also nur beim Wechsel
    const timer = setInterval(() => setNow(skyPhase(new Date())), 60_000);
    return () => clearInterval(timer);
  }, [setting]);

  return setting === 'auto' ? now : setting;
}
