// Cheats und Tasten über einen keydown-Lauscher auf window. Felder und schon behandelte Ereignisse bleiben unberührt,
// und der Lauscher ruft nie preventDefault oder stopPropagation: Tab, Pfeiltasten und Kürzel des Kerns gehen immer durch.
import { useEffect, useRef } from 'react';

export type CheatId = 'money' | 'clear' | 'wanted';

export const CHEATS: Record<string, CheatId> = {
  HESOYAM: 'money',
  TURTLE: 'money',
  AEZAKMI: 'clear',
  LAWYERUP: 'clear',
  BRINGITON: 'wanted',
  FUGITIVE: 'wanted',
};

const CODES = Object.keys(CHEATS);
const BUFFER = 12;
const PAUSE_MS = 3000;
const FIELDS = 'input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="textbox"]';
// Offene Ebenen des Kerns, in denen Buchstaben ruhen: Dialoge (DbModal mit aria-modal, Suche), Seitenblätter wie das
// Ticket-Panel (role="dialog", aria-modal="false"), Popups wie die Board-Auswahl, Menüs und Auswahllisten. Das eigene
// Stats-Menü zählt nicht dazu; dort wirken Q/E weiter.
const OVERLAYS =
  ':is([role="dialog"], [role="alertdialog"], [aria-modal="true"], dialog[open], [role="menu"], [role="listbox"]):not([hidden], [data-gta-part="stats"])';
// Ladebildschirm: wer ihn mit einem Buchstaben überspringt, soll nicht zugleich den Reiter wechseln
const BOOT = '[data-gta-part="boot"]';

// Geprüft wird das ganze Dokument, nicht nur das Ziel der Taste – der Fokus kann auf body liegen
function busy(): boolean {
  return document.querySelector(`${OVERLAYS}, ${BOOT}`) !== null;
}

/** Länge des längsten Endes von buffer, das Anfang eines Cheat-Codes ist */
export function cheatProgress(buffer: string): number {
  for (let length = Math.min(buffer.length, BUFFER); length > 0; length--) {
    const tail = buffer.slice(-length);
    if (CODES.some((code) => code.startsWith(tail))) return length;
  }
  return 0;
}

/** Endet buffer mit einem Cheat-Code? */
export function matchCheat(buffer: string): CheatId | null {
  const code = CODES.find((c) => buffer.endsWith(c));
  return code ? CHEATS[code]! : null;
}

// Taste für uns? Nur einzelne Buchstaben ohne Strg/Alt/Meta, nicht in Feldern, nicht schon behandelt, nicht bei offenem
// Dialog oder Ladebildschirm; gehaltene Tasten wiederholen nichts (sonst rast ein gehaltenes E durch die Navigation)
function letterOf(event: KeyboardEvent): string | null {
  if (event.defaultPrevented || event.repeat || event.ctrlKey || event.altKey || event.metaKey) return null;
  if (!/^[a-z]$/i.test(event.key)) return null;
  if (event.target instanceof Element && event.target.closest(FIELDS)) return null;
  if (busy()) return null;
  return event.key.toUpperCase();
}

export function useGameKeys(options: { cheats: boolean; onCheat(id: CheatId): void; keys?: Record<string, () => void> }): void {
  // Immer die aktuellen Optionen, ohne den Lauscher neu anzumelden
  const latest = useRef(options);
  useEffect(() => {
    latest.current = options;
  });

  useEffect(() => {
    let buffer = '';
    let pause: ReturnType<typeof setTimeout> | undefined;

    const onKeyDown = (event: KeyboardEvent) => {
      const letter = letterOf(event);
      if (!letter) return;
      const { cheats, onCheat, keys } = latest.current;
      buffer = (buffer + letter).slice(-BUFFER);
      clearTimeout(pause);
      pause = setTimeout(() => (buffer = ''), PAUSE_MS);

      const cheat = cheats ? matchCheat(buffer) : null;
      if (cheat) {
        buffer = '';
        onCheat(cheat);
        return;
      }
      // Mitten in einem Cheat (HESOYAM, AEZAKMI, TURTLE …) lösen dieselben Buchstaben keine Aktion aus
      if (cheats && cheatProgress(buffer) >= 2) return;
      keys?.[letter.toLowerCase()]?.();
    };

    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      clearTimeout(pause);
    };
  }, []);
}
