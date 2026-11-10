import { useEffect, useRef, useState } from 'react';
import type { StartAnimationProps } from '@nexus/theme-sdk';

// Startanimation läuft vor dem Rahmen und außerhalb des ThemeHostProviders: Den Schalter „Startanimation“ spiegelt die
// Shell deshalb nach localStorage, den letzten Login merkt sich die Animation selbst. Beides darf fehlen.
export const BOOT_KEY = 'nexus-terminal-boot';
export const LAST_LOGIN_KEY = 'nexus-terminal-last-login';

/** Abstand zwischen zwei Boot-Zeilen und Pause am Ende (ms) */
export const BOOT_STEP_MS = 110;
export const BOOT_HOLD_MS = 350;

const BANNER = [
  String.raw` _ __   _____  ___   _ ___ `,
  String.raw`| '_ \ / _ \ \/ / | | / __|`,
  String.raw`| | | |  __/>  <| |_| \__ \ `,
  String.raw`|_| |_|\___/_/\_\\__,_|___/`,
];

const UNITS = ['Board-Datenbank', 'Live-Verbindung', 'Dispatcher', 'Claude-Nutzung', 'Docs-Index', 'Prompt-Zeile'];

function read(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // privates Fenster oder gesperrter Speicher: dann eben ohne
  }
}

/** Merkt sich, ob die Startanimation laufen soll (die Shell ruft das mit der Einstellung „boot“) */
export function rememberBoot(enabled: boolean): void {
  write(BOOT_KEY, enabled ? 'on' : 'off');
}

function lastLogin(): string | null {
  const stored = read(LAST_LOGIN_KEY);
  const date = stored ? new Date(stored) : null;
  if (!date || Number.isNaN(date.getTime())) return null;
  return date.toLocaleString('de-DE', { weekday: 'short', day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

// Startanimation „Terminal“: Schriftzug, Boot-Meldungen wie bei systemd und eine MOTD mit dem letzten Login.
// Jede Taste oder ein Klick beendet sie sofort; mit ausgeschalteter Einstellung endet sie, bevor etwas zu sehen ist.
export function BootScreen({ onDone }: StartAnimationProps) {
  const enabled = read(BOOT_KEY) !== 'off';
  const [previous] = useState(lastLogin);
  const [shown, setShown] = useState(0);
  const done = useRef(false);

  useEffect(() => {
    const finish = () => {
      if (done.current) return;
      done.current = true;
      onDone();
    };
    if (!enabled) {
      finish();
      return;
    }
    write(LAST_LOGIN_KEY, new Date().toISOString());
    const timers = UNITS.map((_, i) => window.setTimeout(() => setShown(i + 1), (i + 1) * BOOT_STEP_MS));
    timers.push(window.setTimeout(finish, UNITS.length * BOOT_STEP_MS + BOOT_HOLD_MS));
    window.addEventListener('keydown', finish);
    window.addEventListener('pointerdown', finish);
    return () => {
      timers.forEach((t) => window.clearTimeout(t));
      window.removeEventListener('keydown', finish);
      window.removeEventListener('pointerdown', finish);
    };
  }, [enabled, onDone]);

  if (!enabled) return null;
  return (
    <div className="term-boot" data-testid="term-boot">
      <pre className="term-boot-banner">{BANNER.join('\n')}</pre>
      <div className="term-boot-lines">
        {UNITS.slice(0, shown).map((unit) => (
          <div key={unit} className="term-boot-line">
            [<span className="term-boot-ok">  OK  </span>] Started {unit}.
          </div>
        ))}
      </div>
      {shown === UNITS.length && (
        <div className="term-boot-motd">
          {previous ? `Last login: ${previous} on tty1` : 'Willkommen bei Nexus.'}
          <span className="term-boot-cursor" />
        </div>
      )}
      <div className="term-boot-skip">Taste drücken zum Überspringen</div>
    </div>
  );
}
