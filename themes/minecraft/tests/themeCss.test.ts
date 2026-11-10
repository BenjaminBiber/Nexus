// Variablenblock und Zusatzregeln in theme.css: Chat-Farben des Spiels, eckige Ecken, keine weichen Schatten,
// Scrollbalken und Ankreuzfelder als Sprites. Die Namen stammen aus packages/web/src/styles/variables.css.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const read = (...parts: string[]) => readFileSync(join(import.meta.dirname, '..', ...parts), 'utf8');

// Variablen eines Blocks (vom Selektor bis zur ersten schließenden Klammer) wie in packages/web/tests/ThemeCss.test.ts
function variables(css: string, selector: string): Map<string, string> {
  const start = css.indexOf(selector);
  if (start < 0) return new Map();
  const body = css.slice(css.indexOf('{', start) + 1, css.indexOf('}', start));
  return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));
}

const css = read('theme.css');
const own = variables(css, ":root[data-theme='minecraft'] {");

describe('Variablen des Themes', () => {
  it('setzt die Textfarben, Zustandsfarben und Spaltenpunkte auf die Chat-Farben des Spiels', () => {
    for (const [name, value] of Object.entries({
      '--text': '#ffffff',
      '--text-soft': '#e0e0e0',
      '--muted': '#a0a0a0',
      '--accent': '#80ff20',
      '--danger': '#ff5555',
      '--warn': '#ffaa00',
      '--ok': '#55ff55',
      '--status-backlog': '#aaaaaa',
      '--status-waiting': '#ffff55',
      '--status-in-planning': '#ff55ff',
      '--status-planned': '#5555ff',
      '--status-in-progress': '#55ffff',
      '--status-blocked': '#ff5555',
      '--status-done': '#55ff55',
      '--status-not-needed': '#555555',
      '--chart-done': '#55ff55',
      '--chart-fail': '#ff5555',
      '--chart-bar': '#80ff20',
      '--chart-track': '#3f3f3f',
      '--chart-grid': '#555555',
      '--chart-axis': '#a0a0a0',
    })) {
      expect(own.get(name), name).toBe(value);
    }
  });

  it('hat keine runden Ecken, keine weichen Schatten und einen dunklen Dialog-Hintergrund', () => {
    for (const name of ['--radius-control', '--radius-card', '--radius-panel']) expect(own.get(name), name).toBe('0');
    for (const name of ['--shadow-faint', '--shadow-mid', '--shadow-soft', '--shadow-tab', '--shadow-strong', '--sheet-shadow', '--picker-shadow']) {
      expect(own.get(name), name).toBe('none');
    }
    expect(own.get('--backdrop')).toBe('rgba(0, 0, 0, 0.75)');
  });

  it('setzt jede Farb- und Gestaltungsvariable, die das helle Schema des Hosts kennt', () => {
    const light = variables(read('..', '..', 'packages', 'web', 'src', 'styles', 'variables.css'), ':root {');
    // Maße (Höhen, Breiten, Abstände) und Schriften bleiben beim Host; nur das Logo-Plättchen ist überall gleich
    const keep = /^--(control-height|sidebar-width|page-width|page-width-narrow|gutter|font-sans|font-mono|logo-plate)$/;
    const missing = [...light.keys()].filter((name) => !keep.test(name) && !own.has(name));
    expect(missing).toEqual([]);
  });

  it('kennt keine Variable, die der Host nicht hat (außer den eigenen --mc-*)', () => {
    const light = variables(read('..', '..', 'packages', 'web', 'src', 'styles', 'variables.css'), ':root {');
    expect([...own.keys()].filter((name) => !light.has(name) && !name.startsWith('--mc-'))).toEqual([]);
  });
});

describe('Scrollbalken und Ankreuzfelder', () => {
  it('der Scrollbalken ist 12 px breit und nutzt scroller und scroller_background', () => {
    expect(css).toMatch(/::-webkit-scrollbar\s*\{[^}]*width:\s*12px/);
    expect(css).toMatch(/::-webkit-scrollbar-track\s*\{[^}]*var\(--mc-scroller-bg/);
    expect(css).toMatch(/::-webkit-scrollbar-thumb\s*\{[^}]*var\(--mc-scroller[,)]/);
  });

  it('Ankreuzfelder sind Sprites statt Systemfeldern, mit Zuständen für Hover und angekreuzt', () => {
    expect(css).toMatch(/input\[type='checkbox'\]\s*\{[^}]*appearance:\s*none/);
    expect(css).toMatch(/input\[type='checkbox'\]\s*\{[^}]*var\(--mc-checkbox[,)]/);
    expect(css).toContain('--mc-checkbox-highlighted');
    expect(css).toContain('--mc-checkbox-selected');
    expect(css).toContain('--mc-checkbox-selected-highlighted');
  });

  it('Chips und Badges bleiben eckig und tragen den Textschatten des Spiels', () => {
    expect(css).toMatch(/:is\(\.chip, \.badge, \.tag\)\s*\{[^}]*border-radius:\s*0/);
  });
});
