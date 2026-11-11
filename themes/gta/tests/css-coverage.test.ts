// Abdeckung des Hosts: theme.css gestaltet die wichtigsten Selektoren des Kerns, und jede dieser Klassen gibt es im
// Quelltext von packages/web noch (so fällt eine umbenannte Klasse auf). Spätere Pakete hängen ihre Gruppe an HOST an.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = join(import.meta.dirname, '..');
const web = join(root, '..', '..', 'packages', 'web', 'src');

// Je Gruppe: Selektor so, wie er (ohne Präfix) in einer Regel von theme.css steht – Name zur Lesbarkeit im Testlauf
const HOST: Record<string, Record<string, string>> = {
  grundbausteine: {
    Knopf: '.btn',
    'primärer Knopf': '.btn.primary',
    'gefährlicher Knopf': '.btn.danger',
    'gesperrter Knopf': '.btn:disabled',
    'Knopf nur mit Icon': '.icon-btn',
    'Knopf als Link': '.link-btn',
    Eingabefeld: 'input',
    Textbereich: 'textarea',
    Auswahlliste: 'select',
    'Feld mit Beschriftung': '.field',
    Kontrollkästchen: "input[type='checkbox']",
    Schalter: ".setting-control > input[type='checkbox']",
    Registerkarten: '.tabs',
    Registerkarte: '.tab',
    'aktive Registerkarte': '.tab.on',
    Umschalter: '.segmented',
    Filterchip: '.fchip',
    Label: '.tag',
    Zustandsmarke: '.badge',
    Tooltip: '.info-pop',
    'Tooltip im Diagramm': '.runs-tip',
    Menü: '.doc-open-menu',
    'Menü im Theme-Store': '.theme-install-menu',
    'Auswahl-Popup': '.branch-combo-list',
    Dialog: '.db-modal',
    'kleiner Dialog': '.db-dialog',
    Abdunkelung: '.db-modal-backdrop',
    Hinweis: '.banner',
    'Hinweis erfolgreich': '.notice-ok',
    Fehlermeldung: '.field-error',
    'leerer Zustand': '.state.empty',
    Ladezustand: '.state.loading',
    Tabellenkopf: 'th',
    Taste: 'kbd',
    Verbindungsanzeige: '.connection-slot',
    Seitentitel: '.page-title',
    Werkzeugleiste: '.filters',
  },
  markdown: {
    'Markdown-Überschrift': '.markdown h2',
    'Markdown-Code-Block': '.markdown pre',
    'Markdown-Code inline': '.markdown :not(pre) > code',
    'Markdown-Zitat': '.markdown blockquote',
    'Markdown-Tabelle': '.markdown table',
    'Markdown-Link': '.markdown a',
  },
  suche: {
    Suche: '.search-dialog',
    Suchfeld: '.search-dialog-input',
    Treffer: '.search-hit',
    'markierter Treffer': '.search-hit.active',
    Trefferkopf: '.search-group-title',
    'Knopf Suchen': '.global-search',
  },
};

const ROOT = ":root[data-theme='gta']";
const stripComments = (text: string) => text.replace(/\/\*[\s\S]*?\*\//g, '');
const norm = (text: string) => text.replace(/"/g, "'").replace(/\s+/g, ' ').trim();
const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

// Teile einer Selektorliste, nur Kommas außerhalb von Klammern trennen
function topLevel(selector: string): string[] {
  const parts = [''];
  let depth = 0;
  for (const ch of selector) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (ch === ',' && depth === 0) parts.push('');
    else parts[parts.length - 1] += ch;
  }
  return parts.map(norm);
}

// Alle Selektoren aus Regeln mit Inhalt (ohne @-Regeln), ohne das Präfix des Themes und der Stile
function selectors(text: string): string[] {
  return [...stripComments(text).matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .filter((m) => m[2]!.trim() !== '' && !m[1]!.trim().startsWith('@'))
    .flatMap((m) => topLevel(m[1]!))
    .map((s) => s.replace(/^:root\[data-theme='gta'\](:not\(\[data-gta-style='sa'\]\)|\[data-gta-style='sa'\])?/, '').trim());
}

// Der Selektor steht in einer Regel: als Ganzes oder in einer :is()-Liste, nicht nur als Anfang eines längeren Namens
function covered(rules: string[], selector: string): boolean {
  const pattern = new RegExp(`(^|[\\s,(>+~])${escape(norm(selector))}(?![\\w-])`);
  return rules.some((rule) => pattern.test(rule));
}

// Klassen und Elemente eines Selektors (ohne Pseudoklassen und Attributwerte)
function names(selector: string) {
  const bare = selector.replace(/\[[^\]]*\]/g, '').replace(/::?[\w-]+(\([^)]*\))?/g, ' ');
  return {
    classes: [...bare.matchAll(/\.([\w-]+)/g)].map((m) => m[1]!),
    elements: [...bare.matchAll(/(?:^|[\s>+~(,])([a-z][a-z0-9]*)(?![\w-])/g)].map((m) => m[1]!),
  };
}

function sources(dir: string): string {
  return readdirSync(dir, { recursive: true, encoding: 'utf8' })
    .filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f))
    .map((f) => readFileSync(join(dir, f), 'utf8'))
    .join('\n');
}

const rules = selectors(readFileSync(join(root, 'theme.css'), 'utf8'));
const code = sources(web);
const entries = Object.entries(HOST).flatMap(([group, list]) => Object.entries(list).map(([name, selector]) => ({ group, name, selector })));

describe('Abdeckung der Host-Selektoren in theme.css', () => {
  it('die Liste ist nicht leer und die Regeln hängen alle am Theme', () => {
    expect(entries.length).toBeGreaterThan(40);
    expect(readFileSync(join(root, 'theme.css'), 'utf8')).toContain(ROOT);
  });

  it.each(entries)('$group: $name ($selector) hat eine Regel', ({ selector }) => {
    expect(covered(rules, selector)).toBe(true);
  });

  it.each(entries)('$group: $name ($selector) gibt es im Quelltext von packages/web', ({ selector }) => {
    const { classes, elements } = names(selector);
    expect(classes.length + elements.length).toBeGreaterThan(0);
    // Klassen stehen in className (auch in Vorlagen), Elemente als JSX-Tag oder in der Markdown-Ausgabe
    for (const name of classes) expect(code, `.${name}`).toMatch(new RegExp(`['"\`\\s{]${escape(name)}(?![\\w-])`));
    for (const name of elements.filter((e) => !['h2', 'pre', 'code', 'blockquote', 'table', 'a', 'th'].includes(e))) {
      expect(code, name).toMatch(new RegExp(`<${name}[\\s>/]`));
    }
  });

  it('die Prüfung erkennt fehlende und nur angefangene Selektoren', () => {
    expect(covered(['.btn.primary:hover'], '.btn.primary')).toBe(true);
    expect(covered([':is(.tab, .fchip).on'], '.tab')).toBe(true);
    expect(covered(['.btn-group'], '.btn')).toBe(false);
    expect(covered(['.search-hit-head'], '.search-hit')).toBe(false);
  });
});
