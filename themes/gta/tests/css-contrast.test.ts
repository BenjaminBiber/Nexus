// Kontraste nach WCAG aus den Tokens und Regeln von theme.css: Text mindestens 4,5:1, Fokusring und Flächen ohne Text
// mindestens 3:1 – in beiden Stilen und mit allen drei Akzentfarben. Die geprüften Paare stehen ausdrücklich unten.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const css = readFileSync(join(import.meta.dirname, '..', 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');

const ROOT = ":root[data-theme='gta']";
const V = ":root[data-theme='gta']:not([data-gta-style='sa'])";
const SA = ":root[data-theme='gta'][data-gta-style='sa']";
const STYLES = { v: V, sa: SA } as const;
const ACCENTS = ['blau', 'gruen', 'orange'] as const;
type Style = keyof typeof STYLES;
type Accent = (typeof ACCENTS)[number];

// ---- Regeln lesen ----

const norm = (s: string) => s.replace(/\s+/g, ' ').trim();

// Selektorliste an den Kommas der obersten Ebene teilen
function splitList(list: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let cur = '';
  for (const ch of list) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (ch === ',' && depth === 0) {
      out.push(norm(cur));
      cur = '';
    } else cur += ch;
  }
  out.push(norm(cur));
  return out;
}

const RULES = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(([, list, body]) => ({
  selectors: splitList(list!),
  decls: [...body!.matchAll(/(--[\w-]+|[a-z-]+)\s*:\s*([^;]+)/g)].map(([, prop, value]) => [prop!, value!.trim()] as const),
}));

/** Wert der Eigenschaft in der letzten Regel, deren Selektorliste genau diesen Selektor enthält */
function decl(selector: string, prop: string): string {
  let found: string | undefined;
  for (const rule of RULES) {
    if (!rule.selectors.includes(norm(selector))) continue;
    for (const [p, v] of rule.decls) if (p === prop) found = v;
  }
  if (found === undefined) throw new Error(`keine Regel „${selector} { ${prop} }“`);
  return found;
}

// Variablen eines Stils mit Akzent: Gemeinsam, Stil, Akzent und die Rahmen-Variablen (--sa-*) der Shell
function varsOf(style: Style, accent: Accent): Map<string, string> {
  const root = STYLES[style];
  const scopes = [ROOT, root, `${root}[data-gta-accent='${accent}']`, `${root} .gta-${style}`];
  const vars = new Map<string, string>();
  for (const scope of scopes) {
    for (const rule of RULES) {
      if (!rule.selectors.includes(scope)) continue;
      for (const [p, v] of rule.decls) if (p.startsWith('--')) vars.set(p, v);
    }
  }
  return vars;
}

// ---- Farben rechnen ----

interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

function args(inner: string): string[] {
  return splitList(inner);
}

function parse(value: string, vars: Map<string, string>): Rgba {
  const v = value.trim();
  const ref = /^var\((--[\w-]+)(?:,\s*(.+))?\)$/.exec(v);
  if (ref) {
    const next = vars.get(ref[1]!) ?? ref[2];
    if (next === undefined) throw new Error(`unbekannte Variable ${ref[1]}`);
    return parse(next, vars);
  }
  if (v === 'transparent') return { r: 0, g: 0, b: 0, a: 0 };
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(v);
  if (hex) {
    const h = hex[1]!.length === 3 ? [...hex[1]!].map((c) => c + c).join('') : hex[1]!;
    const n = (i: number) => parseInt(h.slice(i, i + 2), 16);
    return { r: n(0), g: n(2), b: n(4), a: h.length === 8 ? n(6) / 255 : 1 };
  }
  const rgb = /^rgba?\((.+)\)$/.exec(v);
  if (rgb) {
    const [r, g, b, a = '1'] = rgb[1]!.split(/[\s,/]+/).filter(Boolean);
    return { r: Number(r), g: Number(g), b: Number(b), a: Number(a) };
  }
  const mix = /^color-mix\(in srgb,(.+)\)$/.exec(v);
  if (mix) {
    const [first, second] = args(mix[1]!).map((part) => {
      const m = /^(.*?)(?:\s+([\d.]+)%)?$/.exec(part)!;
      return { color: parse(m[1]!, vars), p: m[2] === undefined ? undefined : Number(m[2]) / 100 };
    });
    const p1 = first!.p ?? (second!.p === undefined ? 0.5 : 1 - second!.p);
    const p2 = second!.p ?? 1 - p1;
    const a = first!.color.a * p1 + second!.color.a * p2;
    const ch = (k: 'r' | 'g' | 'b') => (a === 0 ? 0 : (first!.color[k] * first!.color.a * p1 + second!.color[k] * second!.color.a * p2) / a);
    return { r: ch('r'), g: ch('g'), b: ch('b'), a };
  }
  throw new Error(`Farbe nicht lesbar: ${v}`);
}

/** Legt Farben nacheinander übereinander; die erste ist die (deckende) Unterlage */
function stack(...layers: Rgba[]): Rgba {
  return layers.reduce((under, top) => ({
    r: top.r * top.a + under.r * (1 - top.a),
    g: top.g * top.a + under.g * (1 - top.a),
    b: top.b * top.a + under.b * (1 - top.a),
    a: 1,
  }));
}

function luminance({ r, g, b }: Rgba): number {
  const lin = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function contrast(fg: Rgba, bg: Rgba): number {
  const top = stack(bg, fg);
  const [hi, lo] = [luminance(top), luminance(bg)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
}

// ---- Unterlagen ----

// Hellste Stelle der Szene in San Andreas: die Sonne (#fff1c4 in SaScene.tsx) unter dem Filter brightness() der Welt
function saSceneBrightest(): Rgba {
  const factor = Number(/brightness\(([\d.]+)\)/.exec(decl(`${SA} .gta-sa-world`, 'filter'))![1]);
  const sun = parse('#fff1c4', new Map());
  return { r: sun.r * factor, g: sun.g * factor, b: sun.b * factor, a: 1 };
}

const black: Rgba = { r: 0, g: 0, b: 0, a: 1 };

function surface(style: Style, accent: Accent, token: string): Rgba {
  const vars = varsOf(style, accent);
  return stack(parse(vars.get('--gta-bg')!, vars), parse(`var(${token})`, vars));
}

// ---- Paare ----

interface Pair {
  name: string;
  fg: string | (() => string); // Farbe (Ausdruck) oder Wert einer Regel aus theme.css
  bg: (style: Style, accent: Accent) => Rgba;
  min: 4.5 | 3;
  styles: Style[];
}

const both: Style[] = ['v', 'sa'];
const onToken = (token: string) => (style: Style, accent: Accent) => surface(style, accent, token);

// Karte in San Andreas: Tafel (--gta-panel) über der hellsten Stelle der Szene, darüber die Tönung der Karte
function saCard(state: 'ruhig' | 'hover' | 'fokus') {
  return (_style: Style, accent: Accent) => {
    const vars = varsOf('sa', accent);
    const layers = [saSceneBrightest(), parse('var(--gta-panel)', vars)];
    if (state === 'hover') layers.push(parse(decl(`${SA} .db-card:is(:hover, :focus-within)`, 'background'), vars));
    if (state === 'fokus') layers.push(parse(decl(`${SA} .db-card:focus-within`, 'background'), vars));
    return stack(...layers);
  };
}

function saInbox(hover: boolean) {
  return (_style: Style, accent: Accent) => {
    const vars = varsOf('sa', accent);
    const layers = [saSceneBrightest(), parse('var(--gta-panel)', vars)];
    if (hover) layers.push(parse(decl(`${SA} .inbox-row:is(:hover, :focus-within)`, 'background'), vars));
    return stack(...layers);
  };
}

// Leisten über der Szene in San Andreas: ihre Unterlage über der hellsten Stelle der Szene
const SA_BARS = `${SA} .gta-sa-main :is(.page > .tabs, .page > .filters, .guide-nav, .dash-head .segmented, .page-head .fchip)`;
function saBar(_style: Style, accent: Accent): Rgba {
  return stack(saSceneBrightest(), parse(decl(SA_BARS, 'background'), varsOf('sa', accent)));
}

const white = () => ({ r: 255, g: 255, b: 255, a: 1 });
const RED_MARK = `${SA} .db-card :is(.badge.blocked, .badge.prio-high, .badge.prio-urgent)`;
const RED_MARK_ON = `${SA} .db-card:is(:hover, :focus-within) :is(.badge.blocked, .badge.prio-high, .badge.prio-urgent)`;

const PAIRS: Pair[] = [
  // Fehlertexte des Kerns
  { name: '--blocked-fg auf --blocked-bg', fg: 'var(--blocked-fg)', bg: onToken('--blocked-bg'), min: 4.5, styles: both },
  { name: '--connection-fg auf --error-bg', fg: 'var(--connection-fg)', bg: onToken('--error-bg'), min: 4.5, styles: both },
  { name: '--blocked-fg auf --surface', fg: 'var(--blocked-fg)', bg: onToken('--surface'), min: 4.5, styles: both },
  // Wo der Kern --danger als Textfarbe nimmt, steht der aufgehellte Ton
  ...['.usage-num.crit', '.kpi-value.tone-bad', '.ladder-summary .tone-bad'].map((sel) => ({
    name: `${sel} (Text in --danger) auf --surface`,
    fg: () => decl(`${ROOT} ${sel}`, 'color'),
    bg: onToken('--surface'),
    min: 4.5 as const,
    styles: both,
  })),
  // Weiße Schrift auf der roten Fläche
  { name: 'Gefahr-Knopf: --gta-danger-fg auf --gta-danger-bg', fg: 'var(--gta-danger-fg)', bg: onToken('--gta-danger-bg'), min: 4.5, styles: ['v'] },
  { name: 'Weiß auf --gta-danger', fg: '#ffffff', bg: onToken('--gta-danger'), min: 4.5, styles: both },
  // GTA V: Fokusring auf weißen Flächen schwarz
  ...[`${V} .gta-v-tab[aria-current='page']:focus-visible`, `${V} .gta-v-cat[aria-pressed='true']:focus-visible`, `${V} .db-card:is(:hover, :focus-within) :focus-visible`].map(
    (sel) => ({ name: `Fokusring ${sel.slice(V.length + 1)} auf Weiß`, fg: () => decl(sel, 'outline-color'), bg: white, min: 3 as const, styles: ['v'] as Style[] }),
  ),
  { name: 'Fokusring in der Akzentfarbe auf dem Grund', fg: 'var(--gta-accent)', bg: onToken('--gta-bg'), min: 3, styles: both },
  // San Andreas: Karte und Liste „Meine Aufgaben“
  { name: '.card-id auf der Karte', fg: () => decl(`${SA} .card-id`, 'color'), bg: saCard('ruhig'), min: 4.5, styles: ['sa'] },
  { name: '.card-id auf der Hover-Tönung', fg: () => decl(`${SA} .db-card:is(:hover, :focus-within) .card-id`, 'color'), bg: saCard('hover'), min: 4.5, styles: ['sa'] },
  { name: '.card-id auf der Fokus-Tönung', fg: () => decl(`${SA} .db-card:is(:hover, :focus-within) .card-id`, 'color'), bg: saCard('fokus'), min: 4.5, styles: ['sa'] },
  { name: 'rote Marke auf der Tafel', fg: () => decl(RED_MARK, 'color'), bg: saCard('ruhig'), min: 4.5, styles: ['sa'] },
  { name: 'rote Marke auf der Hover-Tönung', fg: () => decl(RED_MARK_ON, 'color'), bg: saCard('hover'), min: 4.5, styles: ['sa'] },
  { name: 'rote Marke auf der Fokus-Tönung', fg: () => decl(RED_MARK_ON, 'color'), bg: saCard('fokus'), min: 4.5, styles: ['sa'] },
  { name: '.inbox-id in der Liste', fg: () => decl(`${SA} .inbox-id`, 'color'), bg: saInbox(false), min: 4.5, styles: ['sa'] },
  { name: 'Reiter und Filter über der Szene', fg: 'var(--gta-tab-fg)', bg: saBar, min: 4.5, styles: ['sa'] },
  { name: 'aktiver Reiter über der Szene', fg: 'var(--gta-tab-on-fg)', bg: saBar, min: 4.5, styles: ['sa'] },
  { name: 'Sprungmarken der Anleitung über der Szene', fg: () => decl(`${SA} .guide-nav a`, 'color'), bg: saBar, min: 4.5, styles: ['sa'] },
  { name: '.inbox-id auf der Hover-Tönung', fg: () => decl(`${SA} .inbox-row:is(:hover, :focus-within) .inbox-id`, 'color'), bg: saInbox(true), min: 4.5, styles: ['sa'] },
];

describe('Farbrechnung', () => {
  it('liest Hex, rgba über einer Unterlage und color-mix', () => {
    const vars = new Map([['--a', '#ff0000']]);
    expect(parse('#fff', vars)).toEqual({ r: 255, g: 255, b: 255, a: 1 });
    expect(stack(black, parse('rgba(255, 255, 255, 0.5)', vars)).r).toBeCloseTo(127.5);
    const mixed = parse('color-mix(in srgb, var(--a) 70%, #fff)', vars);
    expect([mixed.r, mixed.g, mixed.b, mixed.a].map((n) => Math.round(n * 10) / 10)).toEqual([255, 76.5, 76.5, 1]);
    expect(parse('color-mix(in srgb, var(--a) 20%, transparent)', vars)).toMatchObject({ r: 255, g: 0, b: 0, a: 0.2 });
    expect(contrast(parse('#fff', vars), black)).toBeCloseTo(21);
    expect(contrast(parse('#777', vars), parse('#fff', vars))).toBeCloseTo(4.48, 2);
  });
});

describe('Kontraste in beiden Stilen und allen Akzentfarben', () => {
  for (const pair of PAIRS) {
    for (const style of pair.styles) {
      for (const accent of ACCENTS) {
        it(`${style}/${accent}: ${pair.name} ≥ ${pair.min}:1`, () => {
          const vars = varsOf(style, accent);
          const fg = typeof pair.fg === 'string' ? pair.fg : pair.fg();
          const value = contrast(parse(fg, vars), pair.bg(style, accent));
          expect(Math.round(value * 1000) / 1000).toBeGreaterThanOrEqual(pair.min);
        });
      }
    }
  }
});

describe('Schriftgröße der IDs in San Andreas', () => {
  for (const sel of ['.card-id', '.inbox-id']) {
    it(`${sel} ist mindestens 10,5 px groß`, () => {
      const size = Number(/([\d.]+)px/.exec(decl(`${SA} ${sel}`, 'font'))![1]);
      expect(size).toBeGreaterThanOrEqual(10.5);
    });
  }
});
