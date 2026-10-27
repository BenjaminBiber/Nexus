import type { TimeWindow } from './DispatchConfig.js';

// Prüft, ob ein Zeitpunkt (Ortszeit) im Zeitfenster liegt; über Mitternacht erlaubt, ohne Fenster immer.
export class TimeWindowCheck {
  // true, wenn neue Worker jetzt starten dürfen
  contains(now: Date, window: TimeWindow | null): boolean {
    if (!window) return true;
    const t = now.getHours() * 60 + now.getMinutes();
    const from = this.minutes(window.from);
    const to = this.minutes(window.to);
    if (from === to) return true;
    return from < to ? t >= from && t < to : t >= from || t < to;
  }

  // Minuten seit Mitternacht aus HH:MM
  private minutes(hhmm: string): number {
    const [h, m] = hhmm.split(':').map(Number);
    return h * 60 + m;
  }
}
