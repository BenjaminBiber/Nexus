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
  board: {
    Board: '.page.db-board',
    'Kopf des Boards': '.db-board-head',
    Spaltenreihe: '.db-columns',
    Spalte: '.db-column',
    Spaltenkopf: '.db-column .column-head',
    Spaltenpunkt: '.db-column .column-dot',
    Spaltenzähler: '.column-head .count',
    'leere Spalte': '.column-empty',
    'gesperrte Spalte': '.db-column.locked',
    Ablagefläche: '.db-column.drop',
    'gesperrte Ablage': '.db-column.drop-blocked',
    Karte: '.db-card',
    'Karte beim Überfahren': '.db-card:is(:hover, :focus-within)',
    'ausgewählte Karte': '.db-card:focus-within',
    'blockierte Karte': '.db-card:has(.badge.blocked)',
    'wartende Karte': '.db-card.is-waiting',
    'Karte beim Ziehen': '.db-card:active',
    Kartennummer: '.card-id',
    Kartentitel: '.card-title',
    Wartehinweis: '.card-waiting',
    Fortschrittsbalken: '.card-steps-bar',
    Fortschritt: '.card-steps-fill',
    'Marke Frage auf der Karte': '.db-card .badge.question',
    'Marke Planung auf der Karte': '.db-card .badge.waiting',
    'Knopf Ticket anlegen': '.db-fab-dot',
  },
  'ticket-panel': {
    'Ticket-Panel': '.db-panel',
    'Kopf des Panels': '.db-panel-head',
    Titel: '.db-title',
    Felder: '.db-fields',
    'Feld zum Ankreuzen': '.db-field-check',
    Seitenleiste: '.db-panel-side',
    'Titel der Seitenleiste': '.db-side-title',
    Reiterleiste: '.db-tablist',
    Reiter: ".db-tablist [role='tab']",
    'aktiver Reiter': ".db-tablist [role='tab'].active",
    Schritt: '.db-step',
    'erledigter Schritt': '.db-step.done',
    Kommentar: '.db-entries > .entry',
    Verlaufseintrag: '.tl-body',
    Verlaufspunkt: '.tl-dot',
    Abhängigkeit: '.dep',
    'offene Abhängigkeit': '.dep.is-open',
    'Titel eines Bereichs': '.db-drawer-title',
    Review: '.db-review',
    'Diff im Review': '.db-review-patch',
  },
  'meine-aufgaben': {
    Seitentitel: '.questions-head > h1',
    Liste: '.inbox-list',
    Eintrag: '.inbox-row',
    'Eintrag beim Überfahren': '.inbox-row:is(:hover, :focus-within)',
    'Eintrag zum Ziehen': ".inbox-row[draggable='true']",
    'Ablage beim Ziehen': '.inbox-row.drop-target',
    'Marke der Art': '.badge.inbox-badge',
    Titel: '.inbox-title',
    Nummer: '.inbox-id',
    Detail: '.inbox-detail',
    'Zähler im Filter': '.tab-count',
  },
  dashboard: {
    Dashboard: '.page.dashboard',
    Seitentitel: '.dash-title > h1',
    Kopf: '.dash-head',
    Kennzahl: '.kpi',
    'Name der Kennzahl': '.kpi-label',
    'Wert der Kennzahl': '.kpi-value',
    Kachel: '.dash-card',
    'Titel der Kachel': '.dash-card h2',
    Säulendiagramm: '.runs-plot',
    Säule: '.runs-seg',
    Legende: '.chart-legend .dot',
    Leiter: '.ladder-bar',
    Abbruchgrund: '.reason-track',
    Hindernis: '.obstacle-count',
    Aufgabengruppe: '.todo-group',
    'Kopf der Gruppe': '.todo-head',
    'Zähler der Gruppe': '.todo-count',
    Aufgabe: '.todo-row',
    'Knopf der Aufgabe': '.todo-action',
    'Hinweis des Dashboards': '.dashboard-note',
    'Notiz als Seitenblatt': '.note-slide',
  },
  sessions: {
    Gruppe: '.card.group',
    'Kopf der Gruppe': '.group-head',
    Session: '.session-row',
    'wartende Session': '.session-row.waiting',
    Zustandspunkt: '.state-dot',
    'laufende Session': '.state-dot.running',
    Titel: '.session-title',
    Subagent: '.subagents li',
    Detailansicht: '.sheet.session-panel',
    'Kopf der Detailansicht': '.sp-head',
    'Titel der Session': '.sheet-head.sp-head h2',
    Eingabe: '.tx-bubble',
    Werkzeugaufruf: '.tx-tool',
    'Kopf des Werkzeugaufrufs': '.tx-tool-head',
    Ausgabe: '.tx-code',
    Befehl: '.tx-command',
    Nachdenken: '.tx-thinking-head',
    Systemmeldung: '.tx-system',
    'läuft noch': '.tx-live-dot',
    Dateiliste: '.fc-list',
    'Kennzahl des Laufs': '.ri-stat',
    'Werte des Laufs': '.ri-list',
  },
  fragen: {
    Liste: '.question-list',
    Tabellenkopf: '.question-head',
    Frage: '.question-row',
    'neue Frage': '.question-row.fresh',
    Titel: '.question-title',
    'Verweis auf ein Ticket': '.qlink',
    'Marke bereit': '.badge.q-ready',
    'Titel der Detailseite': '.question-detail-head h1',
    'wartende Tickets': '.db-question-relations h2',
    Lösungsoption: '.option-card',
    'empfohlene Option': '.option-card.recommended',
    'gewählte Option': '.option-card.picked',
    'Titel der Option': '.option-title',
    Vorteil: '.option-list .pro',
    Entscheidung: '.decision-form',
    'gewählte Antwort': '.decision-choice',
    'getroffene Entscheidung': '.decision-box',
    'frühere Recherche': '.research-earlier',
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

// Steht vor der Fundstelle ein Vorfahre (Kombinator außerhalb von Klammern)? Dann gestaltet die Regel nur einen Sonderfall
function nested(prefix: string): boolean {
  let depth = 0;
  for (const ch of prefix) {
    if (ch === '(') depth++;
    else if (ch === ')') depth--;
    else if (depth === 0 && /[\s>+~]/.test(ch)) return true;
  }
  return false;
}

// Der Selektor beginnt eine Regel (auch innerhalb einer :is()-Liste), nicht nur als Anfang eines längeren Namens und
// nicht nur unter einem Vorfahren wie „.db-search input“ oder „.gta-v kbd“
function covered(rules: string[], selector: string): boolean {
  const pattern = new RegExp(`(^|[\\s,(])${escape(norm(selector))}(?![\\w-])`, 'g');
  return rules.some((rule) => [...rule.matchAll(pattern)].some((m) => !nested(rule.slice(0, m.index + m[1]!.length))));
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

// Selektoren nur aus dem Abschnitt eines Stils (ohne dessen Präfix)
const STYLE_PREFIX = { v: ":root[data-theme='gta']:not([data-gta-style='sa'])", sa: ":root[data-theme='gta'][data-gta-style='sa']" };
function styleSelectors(text: string, style: keyof typeof STYLE_PREFIX): string[] {
  const prefix = STYLE_PREFIX[style];
  return [...stripComments(text).matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .filter((m) => m[2]!.trim() !== '')
    .flatMap((m) => topLevel(m[1]!))
    .filter((s) => s.startsWith(prefix) && !s.startsWith(`${prefix}[`))
    .map((s) => s.slice(prefix.length).trim());
}

// Board und Ticket-Panel bekommen in jedem Stil einen eigenen Look, nicht nur die gemeinsame Form
const PER_STYLE = ['.db-column .column-head', '.column-head .count', '.db-card', '.db-card:is(:hover, :focus-within)', '.card-id', '.card-title', '.card-steps-bar', '.card-waiting', '.db-fab-dot', '.db-title', ".db-tablist [role='tab'].active",
  // Meine Aufgaben, Dashboard, Sessions und Fragen
  '.inbox-row', '.badge.inbox-badge', '.kpi-value', '.dash-card', '.todo-head', '.session-row', '.tx-tool', '.question-row', '.option-card', '.option-card.picked'];

// Arten der Einträge unter „Meine Aufgaben“ (aus dem Kern) und die Farbe, die theme.css je Art setzt
const inboxKinds = [...readFileSync(join(web, 'api', 'inboxTypes.ts'), 'utf8').match(/INBOX_KINDS = \[([^\]]*)\]/)![1]!.matchAll(/'([\w-]+)'/g)].map((m) => m[1]!);
function kindColor(text: string, kind: string, prefix = ''): string | undefined {
  const rule = stripComments(text).match(new RegExp(`${escape(prefix)}\\s*\\.inbox-row\\.kind-${kind}(?![\\w-])[^{]*\\{([^}]*)\\}`));
  return rule?.[1]!.match(/--inbox-color:\s*([^;]+)/)?.[1]!.trim();
}

// Farbe einer Art im Stil: Regel des Stils vor der gemeinsamen, Tokens aus dem ersten Block des Stils aufgelöst
function kindValue(text: string, kind: string, style: keyof typeof STYLE_PREFIX): string | undefined {
  const prefix = STYLE_PREFIX[style];
  const tokens = stripComments(text).match(new RegExp(`${escape(prefix)}\\s*\\{([^}]*)\\}`))![1]!;
  const color = kindColor(text, kind, prefix) ?? kindColor(text, kind);
  const name = color?.match(/^var\((--gta-[\w-]+)\)$/)?.[1];
  return name ? tokens.match(new RegExp(`${name}:\\s*([^;]+);`))?.[1]!.trim().toLowerCase() : color;
}

const rules = selectors(readFileSync(join(root, 'theme.css'), 'utf8'));
const code = sources(web);
const entries = Object.entries(HOST).flatMap(([group, list]) => Object.entries(list).map(([name, selector]) => ({ group, name, selector })));

describe('Abdeckung der Host-Selektoren in theme.css', () => {
  it('die Liste ist nicht leer, theme.css hat Regeln am Theme', () => {
    expect(entries.length).toBeGreaterThan(40);
    expect(rules.length).toBeGreaterThan(100);
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

  it.each((['v', 'sa'] as const).flatMap((style) => PER_STYLE.map((selector) => ({ style, selector }))))('Stil $style gestaltet $selector', ({ style, selector }) => {
    expect(covered(styleSelectors(readFileSync(join(root, 'theme.css'), 'utf8'), style), selector)).toBe(true);
  });

  it('jede Art unter „Meine Aufgaben“ hat eine Kennfarbe, die sich in jedem Stil von den anderen unterscheidet', () => {
    const css = readFileSync(join(root, 'theme.css'), 'utf8');
    expect(inboxKinds).toEqual(expect.arrayContaining(['question', 'accept', 'merge', 'blocked', 'planning', 'note']));
    for (const kind of inboxKinds) expect(kindColor(css, kind), kind).toMatch(/^var\(--gta-/);
    for (const style of ['v', 'sa'] as const) {
      const colors = inboxKinds.map((kind) => kindValue(css, kind, style));
      for (const color of colors) expect(color, style).toMatch(/^#[0-9a-f]{6}$/);
      expect(new Set(colors).size, `${style}: ${colors.join(', ')}`).toBe(inboxKinds.length);
    }
  });

  it('die Abschnitte der Stile werden getrennt gelesen', () => {
    const css = ":root[data-theme='gta']:not([data-gta-style='sa']) .a { color: red; } :root[data-theme='gta'][data-gta-style='sa'] .b { color: red; } :root[data-theme='gta'] .c { color: red; }";
    expect(styleSelectors(css, 'v')).toEqual(['.a']);
    expect(styleSelectors(css, 'sa')).toEqual(['.b']);
  });

  it('die Prüfung erkennt fehlende und nur angefangene Selektoren', () => {
    expect(covered(['.btn.primary:hover'], '.btn.primary')).toBe(true);
    expect(covered([':is(.tab, .fchip).on'], '.tab')).toBe(true);
    expect(covered(['.btn-group'], '.btn')).toBe(false);
    expect(covered(['.search-hit-head'], '.search-hit')).toBe(false);
    expect(covered(['.db-search input:focus'], 'input')).toBe(false);
    expect(covered(['.gta-v kbd'], 'kbd')).toBe(false);
    expect(covered([':is(.a, .b) > .c', '.markdown h2'], '.markdown h2')).toBe(true);
  });
});
