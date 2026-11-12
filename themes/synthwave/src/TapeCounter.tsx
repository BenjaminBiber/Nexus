import { useEffect, useRef, useState } from 'react';
import { SIDE_MS, flipSide, useCassetteSide } from './cassette.js';

const MAX_MS = (99 * 3600 + 59 * 60 + 59) * 1000;

/** So lange zeigt das Display „AUTO REVERSE“, nachdem die Kassette sich gedreht hat (ms) */
export const REVERSE_MS = 5000;

const pad = (value: number) => String(value).padStart(2, '0');

// Bandzähler als HH:MM:SS; über 99 Stunden bleibt er bei 99:59:59 stehen
export function formatCounter(ms: number): string {
  const total = Math.floor(Math.min(Math.max(ms, 0), MAX_MS) / 1000);
  return `${pad(Math.floor(total / 3600))}:${pad(Math.floor(total / 60) % 60)}:${pad(total % 60)}`;
}

/** Wie oft die Kassette seit dem Laden schon ans Bandende gekommen ist */
export const sidesPlayed = (elapsed: number) => Math.floor(Math.max(elapsed, 0) / SIDE_MS);

// Einblendung im Display: mit VHS „PLAY ▶“ und die Zeit seit dem Laden, sonst nur die Uhrzeit. Auf Seite B läuft das
// Band rückwärts durchs Deck („◀ PLAY“, wie bei Auto-Reverse-Decks). Nach je 45 Minuten Spielzeit ist die Seite zu Ende:
// die Kassette dreht sich von selbst und das Display zeigt kurz „AUTO REVERSE“.
export function TapeCounter({ vhs }: { vhs: boolean }) {
  const start = useRef(Date.now());
  const [now, setNow] = useState(() => Date.now());
  const side = useCassetteSide();
  const played = sidesPlayed(now - start.current);
  const lastPlayed = useRef(played);
  const [reversing, setReversing] = useState(false);

  useEffect(() => {
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), vhs ? 1000 : 15_000);
    return () => clearInterval(timer);
  }, [vhs]);

  useEffect(() => {
    if (played === lastPlayed.current) return;
    lastPlayed.current = played;
    if (!vhs) return;
    flipSide();
    setReversing(true);
  }, [played, vhs]);

  useEffect(() => {
    if (!reversing) return;
    const timer = setTimeout(() => setReversing(false), REVERSE_MS);
    return () => clearTimeout(timer);
  }, [reversing]);

  const date = new Date(now);
  if (!vhs) {
    return (
      <time className="synth-clock" dateTime={date.toISOString()}>
        {`${pad(date.getHours())}:${pad(date.getMinutes())}`}
      </time>
    );
  }
  const mode = reversing ? 'AUTO REVERSE' : side === 'B' ? '◀ PLAY' : 'PLAY ▶';
  return (
    <time className="synth-osd" dateTime={date.toISOString()} title={date.toLocaleString()} data-reversing={reversing ? '' : undefined}>
      <span aria-hidden="true">{mode}</span> {formatCounter(now - start.current)}
    </time>
  );
}
