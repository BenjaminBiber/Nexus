import type { ReactNode } from 'react';
import type { NavIconName } from '@nexus/theme-sdk';
import { f, type Motion } from './motion.js';

// Linien-Symbole der Navigation im 24er-Raster mit ihrer Bewegung beim Überfahren. `body` zeigt den Ruhezustand,
// bewegte Teile tragen `data-n`; `draw` schreibt die Zahlen aus `target` in genau diese Teile. Jede Bewegung ist
// die Handlung des Gegenstands selbst und endet ruhig
export interface NavIconDef {
  body: ReactNode;
  motion: Motion;
}

const clamp = (n: number) => Math.min(1, Math.max(0, n));
const set = (el: SVGElement, attr: string, v: string | number) => el.setAttribute(attr, String(v));
// Textzeilen der aufgeschlagenen Anleitung: Höhe und rechtes Ende, alle beginnen bei x 12.5
const PAGE_LINES: [number, number][] = [[8, 16], [11, 16], [14, 14.5]];

export const NAV_ICONS: Record<NavIconName, NavIconDef> = {
  // Board: Arbeit wandert in die andere Spalte, die Spalten tauschen ihre Höhe
  board: {
    body: (
      <>
        <path data-n="l" d="M4 4h6v16H4z" />
        <path data-n="r" d="M14 4h6v10h-6z" />
      </>
    ),
    motion: {
      target: (h) => (h ? { l: 10, r: 16 } : { l: 16, r: 10 }),
      draw: (p, e) => {
        set(e.l, 'd', `M4 4h6v${f(p.l)}H4z`);
        set(e.r, 'd', `M14 4h6v${f(p.r)}h-6z`);
      },
    },
  },
  // Dashboard: die Balken ordnen sich zu einem steigenden Verlauf
  dashboard: {
    body: (
      <>
        <path data-n="b" d="M4 20V10M10 20V4M16 20V13" />
        <path d="M21 20H3" />
      </>
    ),
    motion: {
      target: (h) => (h ? { a: 13, b: 8, c: 4 } : { a: 10, b: 4, c: 13 }),
      draw: (p, e) => set(e.b, 'd', `M4 20V${f(p.a)}M10 20V${f(p.b)}M16 20V${f(p.c)}`),
    },
  },
  // Eingang: ein Pfeil fällt von oben in die Ablage
  inbox: {
    body: (
      <>
        <path d="M4 13l2.5-8h11L20 13v6H4zM4 13h5l1 2h4l1-2h5" />
        <g data-n="a" transform="translate(0 -6)" opacity="0">
          <path d="M12 7.1v3.8M10 8.9l2 2 2-2" />
        </g>
      </>
    ),
    motion: {
      target: (h) => (h ? { y: 0, o: 1 } : { y: -6, o: 0 }),
      draw: (p, e) => {
        set(e.a, 'transform', `translate(0 ${f(p.y)})`);
        set(e.a, 'opacity', f(p.o));
      },
    },
  },
  // Sitzungen: der Befehl läuft, die Eingabezeile rückt hoch und darunter erscheint die Ausgabe
  sessions: {
    body: (
      <>
        <path d="M4 5h16v14H4z" />
        <g data-n="p">
          <path d="M8 10l3 2-3 2M13 14h3" />
        </g>
        <path data-n="o" d="M8 16h0" opacity="0" />
      </>
    ),
    motion: {
      target: (h) => (h ? { y: -2, w: 7, o: 1 } : { y: 0, w: 0, o: 0 }),
      draw: (p, e) => {
        set(e.p, 'transform', `translate(0 ${f(p.y)})`);
        set(e.o, 'd', `M8 16h${f(p.w)}`);
        set(e.o, 'opacity', f(p.o));
      },
    },
  },
  // Fragen: das Fragezeichen legt grübelnd den Kopf schief
  questions: {
    body: (
      <>
        <path d="M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z" />
        <g data-n="q">
          <path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17h.01" />
        </g>
      </>
    ),
    motion: {
      target: (h) => (h ? { a: 14, y: -0.6 } : { a: 0, y: 0 }),
      draw: (p, e) => set(e.q, 'transform', `translate(0 ${f(p.y)}) rotate(${f(p.a)} 12 13)`),
    },
  },
  // Doku: die Ecke der Seite knickt weiter um
  docs: {
    body: (
      <>
        <path data-n="pg" d="M6 3H14L18 7V21H6Z" />
        <path data-n="fd" d="M14 3V7H18" />
        <path d="M9 12h6M9 16h6" />
      </>
    ),
    motion: {
      target: (h) => ({ c: h ? 6.5 : 4 }),
      draw: (p, e) => {
        const x = f(18 - p.c), y = f(3 + p.c);
        set(e.pg, 'd', `M6 3H${x}L18 ${y}V21H6Z`);
        set(e.fd, 'd', `M${x} 3V${y}H18`);
      },
    },
  },
  // Suche: die Lupe schwenkt suchend nach oben links
  search: {
    body: (
      <g data-n="m">
        <path d="M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4" />
      </g>
    ),
    motion: {
      target: (h) => (h ? { x: -1.5, y: -1.5, a: -14 } : { x: 0, y: 0, a: 0 }),
      draw: (p, e) => set(e.m, 'transform', `translate(${f(p.x)} ${f(p.y)}) rotate(${f(p.a)} 11 11)`),
    },
  },
  // Anleitung: der Buchdeckel schwingt auf (die freie Kante kommt perspektivisch näher), dahinter liegt die Seite mit Text
  guide: {
    body: (
      <>
        <path d="M4 21V5a2 2 0 0 1 2-2M6 19a2 2 0 0 0-2 2" />
        <path data-n="pg" d="M19 3H19V19H19" opacity="0" />
        <path data-n="ln" d="" />
        <path data-n="cv" d="M6 3L19 3V19L6 19" />
        <path data-n="tt" d="M9 7h6" />
      </>
    ),
    motion: {
      target: (h) => (h ? { x: 10, d: 1.5 } : { x: 19, d: 0 }),
      draw: (p, e) => {
        const { x, d } = p;
        // Punkt auf dem Deckel: u quer von der Bindung (0) zur freien Kante (1), v von oben (0) nach unten (1)
        const at = (u: number, v: number) => {
          const top = 3 - d * u, bottom = 19 + d * u;
          return `${f(6 + u * (x - 6))} ${f(top + v * (bottom - top))}`;
        };
        set(e.cv, 'd', `M6 3L${f(x)} ${f(3 - d)}V${f(19 + d)}L6 19`);
        set(e.tt, 'd', `M${at(3 / 13, 0.25)}L${at(9 / 13, 0.25)}`);
        set(e.tt, 'opacity', f(clamp((x - 15) / 3)));
        set(e.pg, 'd', `M${f(x)} 3H19V19H${f(x)}`);
        set(e.pg, 'opacity', f(clamp((19 - x) / 2)));
        // Textzeilen der Seite kommen erst hinter der Deckelkante zum Vorschein, statt durch den Deckel zu scheinen
        const from = x + 1.75;
        set(e.ln, 'd', PAGE_LINES.filter(([, end]) => from < end).map(([y, end]) => `M${f(Math.max(12.5, from))} ${y}H${end}`).join(''));
      },
    },
  },
  // Einstellungen: das Zahnrad dreht sich eine Vierteldrehung weiter
  settings: {
    body: (
      <>
        <g data-n="g">
          <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
        </g>
        <circle cx="12" cy="12" r="3" />
      </>
    ),
    motion: {
      target: (h) => ({ a: h ? 90 : 0 }),
      draw: (p, e) => set(e.g, 'transform', `rotate(${f(p.a)} 12 12)`),
    },
  },
};
