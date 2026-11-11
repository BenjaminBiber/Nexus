// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { gtaMermaid } from '../src/mermaid.js';
import gtaTheme from '../src/main.js';

const VARIABLEN = ['--gta-bg', '--gta-text', '--gta-text-dim', '--gta-line', '--gta-accent', '--gta-accent-ink', '--gta-warn', '--gta-font-ui', '--gta-font-title'];

afterEach(() => {
  for (const name of VARIABLEN) document.documentElement.style.removeProperty(name);
});

describe('gtaMermaid', () => {
  it('nutzt theme base, dunklen Modus und die Schrift des Stils', () => {
    const look = gtaMermaid();
    expect(look.theme).toBe('base');
    expect(look.themeVariables?.darkMode).toBe(true);
    expect(look.fontFamily).toContain('Barlow');
    expect(look.themeVariables?.fontFamily).toBe(look.fontFamily);
  });

  it('fällt ohne gesetzte Variablen auf die Tokens von GTA V zurück', () => {
    const look = gtaMermaid();
    expect(look.themeVariables?.background).toBe('#07050c');
    expect(look.themeVariables?.primaryColor).toBe('#000000');
    expect(look.themeVariables?.primaryTextColor).toBe('#ffffff');
    expect(look.themeVariables?.primaryBorderColor).toBe('#65b4d4');
    expect(look.themeVariables?.nodeBorder).toBe('#65b4d4');
    expect(look.themeVariables?.lineColor).toBe('#ffffff');
    expect(look.themeCSS).toContain('#65b4d4');
    expect(look.themeCSS).toContain('Barlow Condensed');
  });

  it('übernimmt die gesetzten --gta-* Tokens (Stil und Akzentfarbe) in Variablen und themeCSS', () => {
    const stil = document.documentElement.style;
    stil.setProperty('--gta-accent', '#74c85a');
    stil.setProperty('--gta-bg', '#010203');
    stil.setProperty('--gta-text', '#e1e1e1');
    stil.setProperty('--gta-font-ui', "'Arimo', sans-serif");
    stil.setProperty('--gta-font-title', "'Michroma', sans-serif");
    const look = gtaMermaid();
    expect(look.themeVariables?.primaryBorderColor).toBe('#74c85a');
    expect(look.themeVariables?.clusterBorder).toBe('#74c85a');
    expect(look.themeVariables?.background).toBe('#010203');
    expect(look.themeVariables?.lineColor).toBe('#e1e1e1');
    expect(look.fontFamily).toBe("'Arimo', sans-serif");
    expect(look.themeCSS).toContain('#74c85a');
    expect(look.themeCSS).toContain("'Michroma', sans-serif");
    expect(look.themeCSS).not.toContain('#65b4d4');
  });

  it('gestaltet Knoten als schwarze Kästen, Kanten, Beschriftungen und Gruppen', () => {
    const css = gtaMermaid().themeCSS ?? '';
    expect(css).toContain('.node rect');
    expect(css).toContain('.node polygon');
    expect(css).toContain('.edgePath path');
    expect(css).toContain('.flowchart-link');
    expect(css).toContain('.edgeLabel');
    expect(css).toContain('.cluster rect');
    expect(css).toContain('.cluster-label');
    expect(css).toMatch(/\.node rect[^}]*fill: #000000/);
  });

  it('lädt nichts von außen und bewegt sich nicht', () => {
    const css = gtaMermaid().themeCSS ?? '';
    expect(css).not.toMatch(/url\(|@import|https?:/);
    expect(css).not.toMatch(/animation|@keyframes|transition/);
  });

  it('ist im Theme als mermaid eingetragen', () => {
    expect(gtaTheme.mermaid).toBe(gtaMermaid);
  });
});
