// Kurz eingeblendete Werte: Zonenname nach Seitenwechsel, Board-Name nach Board-Wechsel, Anruf bei neuen Fragen.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

type Timer = ReturnType<typeof setTimeout> | undefined;

// Timer, der beim Abbauen sicher weggeräumt wird
function useTimer(): { start(ms: number, done: () => void): void; stop(): void } {
  const timer = useRef<Timer>(undefined);
  const stop = useCallback(() => clearTimeout(timer.current), []);
  const start = useCallback((ms: number, done: () => void) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(done, ms);
  }, []);
  useEffect(() => stop, [stop]);
  return useMemo(() => ({ start, stop }), [start, stop]);
}

/** Liefert den neuen Wert für `ms` Millisekunden, nachdem er sich geändert hat. Der erste definierte Wert löst nichts aus. */
export function useChanged<T>(value: T | null | undefined, ms: number): T | null {
  const [shown, setShown] = useState<{ value: T } | null>(null);
  // Letzter definierter Wert; null und undefined dazwischen zählen nicht als Änderung
  const known = useRef<{ value: T } | null>(null);
  const timer = useTimer();

  useEffect(() => {
    if (value === null || value === undefined) return;
    const before = known.current;
    known.current = { value };
    if (!before || Object.is(before.value, value)) return;
    setShown({ value });
    timer.start(ms, () => setShown(null));
  }, [value, ms, timer]);

  return shown ? shown.value : null;
}

/** Schalter, der nach `fire()` für `ms` Millisekunden an ist; ein weiteres `fire()` startet die Zeit neu */
export function useFlash(ms: number): { on: boolean; fire(): void } {
  const [on, setOn] = useState(false);
  const timer = useTimer();
  const fire = useCallback(() => {
    setOn(true);
    timer.start(ms, () => setOn(false));
  }, [ms, timer]);
  return useMemo(() => ({ on, fire }), [on, fire]);
}

/** Anlaufzeit nach dem Mount: Der Host liefert Zähler erst als 0, dann die echte Zahl – das ist kein Anstieg. */
export const SETTLE_MS = 5000;
/** So lange gilt nach einem Fall auf 0 der alte Stand weiter als Grundlinie (Projektwechsel, Neuladen). */
export const HOLD_MS = 30_000;

/** Meldet, wenn ein Zähler steigt (nicht in der Anlaufzeit); bleibt bis `clear()` oder `ms` Millisekunden */
export function useIncrease(count: number, ms: number, enabled: boolean): { value: number | null; clear(): void } {
  // Zahl der neuen seit dem Einblenden; weitere Anstiege zählen dazu
  const [value, setValue] = useState<number | null>(null);
  const known = useRef<number | null>(null);
  const settled = useRef(false);
  // Stand vor einem Fall auf 0, solange die Haltezeit läuft
  const held = useRef<number | null>(null);
  const timer = useTimer();
  const settle = useTimer();
  const hold = useTimer();

  const clear = useCallback(() => {
    timer.stop();
    setValue(null);
  }, [timer]);

  useEffect(() => {
    settle.start(SETTLE_MS, () => (settled.current = true));
  }, [settle]);

  useEffect(() => {
    const before = known.current;
    known.current = count;
    if (before === null || !settled.current || count === before) return;
    if (count === 0) {
      held.current = before;
      hold.start(HOLD_MS, () => (held.current = null));
      return;
    }
    const base = Math.max(before, held.current ?? 0);
    held.current = null;
    hold.stop();
    if (!enabled || count <= base) return;
    setValue((shown) => (shown ?? 0) + count - base);
    timer.start(ms, () => setValue(null));
  }, [count, enabled, ms, timer, hold]);

  useEffect(() => {
    if (!enabled) clear();
  }, [enabled, clear]);

  const shown = enabled ? value : null;
  return useMemo(() => ({ value: shown, clear }), [shown, clear]);
}
