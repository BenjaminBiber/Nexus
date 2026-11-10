import type { MermaidLook } from '@nexus/theme-sdk';

// Wert einer CSS-Variable auf <html> (beim Zeichnen sind Theme und Farbschema gesetzt); ohne Wert der Rückfall
function read(name: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

// Mermaid im Neon-Look: dunkle Knoten mit Neonrand, Kanten in Cyan, Pfeilspitzen in Magenta, Schrift Exo 2 –
// in den Farben des gewählten Farbschemas (Rückfall: Outrun)
export function synthMermaid(): MermaidLook {
  const bg = read('--synth-bg', '#120a24');
  const deep = read('--synth-bg-deep', '#0a0514');
  const surface = read('--synth-surface', '#1c1136');
  const fg = read('--synth-fg', '#f3eaff');
  const magenta = read('--synth-magenta', '#ff2bd6');
  const cyan = read('--synth-cyan', '#22e6ff');
  const violet = read('--synth-violet', '#8b5cff');
  const font = "'Exo 2', sans-serif";
  // Farben als feste Werte: im SVG gelten die Variablen des Themes nicht sicher
  const themeCSS = [
    `.node rect, .node polygon, .node circle { stroke-width: 1.5px; filter: drop-shadow(0 0 4px ${cyan}); }`,
    `.edgePath path, .flowchart-link { stroke: ${cyan}; }`,
    `marker, .arrowheadPath, .marker { fill: ${magenta}; stroke: ${magenta}; }`,
    `.cluster-label, .cluster text { font-family: 'Orbitron', sans-serif; font-size: 12px; }`,
  ].join(' ');
  return {
    theme: 'base',
    fontFamily: font,
    themeVariables: {
      darkMode: true,
      background: bg,
      primaryColor: surface,
      primaryTextColor: fg,
      primaryBorderColor: cyan,
      secondaryColor: deep,
      tertiaryColor: deep,
      lineColor: cyan,
      textColor: fg,
      mainBkg: surface,
      nodeBorder: cyan,
      clusterBkg: deep,
      clusterBorder: violet,
      edgeLabelBackground: bg,
      noteBkgColor: surface,
      noteBorderColor: magenta,
      actorBorder: magenta,
      signalColor: cyan,
      fontFamily: font,
    },
    themeCSS,
  };
}
