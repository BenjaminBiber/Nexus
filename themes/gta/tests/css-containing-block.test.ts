// Bezugsrahmen: transform, filter, perspective, contain und will-change machen ein Element zum Bezugsrahmen aller fest
// positionierten Nachfahren. Liegt so eine Eigenschaft dauerhaft auf einer Hülle um Inhalte des Kerns, landen dessen
// Popups und Dialoge in der Hülle statt im Fenster (so war die Board-Auswahl in San Andreas kaputt). Ausnahme: Regeln,
// die nur während Busted/Wasted gelten ([data-dead]).
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const css = readFileSync(join(import.meta.dirname, '..', 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');

// Hüllen um Inhalte des Kerns: Wurzel und Rahmen der Shell, Inhaltsbereiche, Board-Auswahl samt ihrer Wurzel im Kern
const SHELLS = [/\.gta-frame(?![\w-])/, /\.gta-(sa|v)(?![\w-])/, /\.gta-(sa|v)-frame(?![\w-])/, /\.gta-(sa|v)-main(?![\w-])/];
const SWITCHER = /\.gta-(sa|v)-board(?![\w-])/;
const SWITCHER_ROOT = /\.(proj|board-picker)(?![\w-])/;
const PROPS = /(?:^|;)\s*(transform|filter|backdrop-filter|perspective|contain|will-change)\s*:\s*([^;]+)/g;

// Selektorliste an den Kommas der obersten Ebene teilen (nicht in :is(), :not(), :has())
function splitList(list: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let cur = '';
  for (const ch of list) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (ch === ',' && depth === 0) {
      out.push(cur.trim());
      cur = '';
    } else cur += ch;
  }
  out.push(cur.trim());
  return out;
}

// Letzter Teil des Selektors (das Element, auf dem die Regel liegt), Kombinatoren nur auf oberster Ebene
function subject(selector: string): string {
  let depth = 0;
  let start = 0;
  for (let i = 0; i < selector.length; i++) {
    const ch = selector[i];
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (depth === 0 && /[\s>+~]/.test(ch)) start = i + 1;
  }
  return selector.slice(start);
}

function wrapsCore(selector: string): boolean {
  const last = subject(selector);
  if (SHELLS.some((re) => re.test(last)) || SWITCHER.test(last)) return true;
  return SWITCHER.test(selector) && SWITCHER_ROOT.test(last);
}

function offenders(): string[] {
  const found: string[] = [];
  for (const [, list, body] of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const props = [...body.matchAll(PROPS)].filter(([, , value]) => value.trim() !== 'none');
    if (props.length === 0) continue;
    for (const selector of splitList(list)) {
      if (selector.includes('[data-dead]') || !wrapsCore(selector)) continue;
      found.push(`${selector} { ${props.map(([, p, v]) => `${p}: ${v.trim()}`).join('; ')} }`);
    }
  }
  return found;
}

describe('Bezugsrahmen der Hüllen um Inhalte des Kerns', () => {
  it('erkennt die Hüllen', () => {
    expect(wrapsCore(":root[data-theme='gta'][data-gta-style='sa'] .gta-sa-board")).toBe(true);
    expect(wrapsCore(":root[data-theme='gta'] .gta-v-board .proj")).toBe(true);
    expect(wrapsCore(":root[data-theme='gta'] .gta-sa-main")).toBe(true);
    expect(wrapsCore(":root[data-theme='gta'] .gta-sa-board .bp-field")).toBe(false);
    expect(wrapsCore(":root[data-theme='gta'] .gta-sa-main .db-card")).toBe(false);
  });

  it('keine Hülle wird dauerhaft zum Bezugsrahmen', () => {
    expect(offenders()).toEqual([]);
  });
});
