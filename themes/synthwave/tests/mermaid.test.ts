// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { synthMermaid } from '../src/mermaid.js';
import synthwaveTheme from '../src/main.js';

const VARIABLEN = ['--synth-bg', '--synth-bg-deep', '--synth-surface', '--synth-fg', '--synth-magenta', '--synth-cyan', '--synth-violet'];

afterEach(() => {
  for (const name of VARIABLEN) document.documentElement.style.removeProperty(name);
});

describe('synthMermaid', () => {
  it('nutzt theme base und Exo 2 als Schrift', () => {
    const look = synthMermaid();
    expect(look.theme).toBe('base');
    expect(look.fontFamily).toContain('Exo 2');
    expect(look.themeVariables?.fontFamily).toContain('Exo 2');
    expect(look.themeVariables?.darkMode).toBe(true);
  });

  it('fällt ohne gesetzte Variablen auf die Outrun-Werte zurück', () => {
    const look = synthMermaid();
    expect(look.themeVariables?.lineColor).toBe('#22e6ff');
    expect(look.themeVariables?.primaryBorderColor).toBe('#22e6ff');
    expect(look.themeVariables?.background).toBe('#120a24');
    expect(look.themeVariables?.primaryColor).toBe('#1c1136');
    expect(look.themeVariables?.primaryTextColor).toBe('#f3eaff');
    expect(look.themeVariables?.secondaryColor).toBe('#0a0514');
    expect(look.themeVariables?.clusterBorder).toBe('#8b5cff');
    expect(look.themeVariables?.noteBorderColor).toBe('#ff2bd6');
    expect(look.themeCSS).toContain('#22e6ff');
  });

  it('übernimmt die gesetzten --synth-* Farben in Variablen und themeCSS', () => {
    const stil = document.documentElement.style;
    stil.setProperty('--synth-cyan', '#123456');
    stil.setProperty('--synth-magenta', '#654321');
    stil.setProperty('--synth-bg', '#010203');
    const look = synthMermaid();
    expect(look.themeVariables?.lineColor).toBe('#123456');
    expect(look.themeVariables?.nodeBorder).toBe('#123456');
    expect(look.themeVariables?.signalColor).toBe('#123456');
    expect(look.themeVariables?.actorBorder).toBe('#654321');
    expect(look.themeVariables?.background).toBe('#010203');
    expect(look.themeCSS).toContain('#123456');
    expect(look.themeCSS).toContain('#654321');
    expect(look.themeCSS).not.toContain('#22e6ff');
  });

  it('liefert themeCSS mit Regeln für Knoten, Kanten und Pfeilspitzen', () => {
    const css = synthMermaid().themeCSS ?? '';
    expect(css).toContain('.node rect');
    expect(css).toContain('.node polygon');
    expect(css).toContain('.node circle');
    expect(css).toContain('drop-shadow');
    expect(css).toContain('.edgePath path');
    expect(css).toContain('.flowchart-link');
    expect(css).toContain('marker');
    expect(css).toContain('Orbitron');
  });

  it('ist im Theme als mermaid eingetragen', () => {
    expect(synthwaveTheme.mermaid).toBe(synthMermaid);
  });
});
