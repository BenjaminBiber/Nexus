import { useEffect, useRef, useState } from 'react';

const MAX_MS = (99 * 3600 + 59 * 60 + 59) * 1000;

const pad = (value: number) => String(value).padStart(2, '0');

// Bandzähler als HH:MM:SS; über 99 Stunden bleibt er bei 99:59:59 stehen
export function formatCounter(ms: number): string {
  const total = Math.floor(Math.min(Math.max(ms, 0), MAX_MS) / 1000);
  return `${pad(Math.floor(total / 3600))}:${pad(Math.floor(total / 60) % 60)}:${pad(total % 60)}`;
}

// Einblendung im Display: mit VHS „PLAY ▶“ und die Zeit seit dem Laden, sonst nur die Uhrzeit
export function TapeCounter({ vhs }: { vhs: boolean }) {
  const start = useRef(Date.now());
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), vhs ? 1000 : 15_000);
    return () => clearInterval(timer);
  }, [vhs]);

  const date = new Date(now);
  if (!vhs) {
    return (
      <time className="synth-clock" dateTime={date.toISOString()}>
        {`${pad(date.getHours())}:${pad(date.getMinutes())}`}
      </time>
    );
  }
  return (
    <time className="synth-osd" dateTime={date.toISOString()} title={date.toLocaleString()}>
      <span aria-hidden="true">PLAY ▶</span> {formatCounter(now - start.current)}
    </time>
  );
}
