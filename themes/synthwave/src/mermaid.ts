import type { MermaidLook } from '@nexus/theme-sdk';

// Wert einer CSS-Variable auf <html> (beim Zeichnen sind Theme und Farbschema gesetzt); ohne Wert der Rückfall
function read(name: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

/** Cremefarbe der Bandetiketten (wie das Etikett in der Deck-Leiste) und ihre Schrift */
const LABEL_PAPER = '#f4ead2';
const LABEL_INK = '#2a1a3a';

// Mermaid im Neon-Look, in den Farben des gewählten Farbschemas (Rückfall: Outrun):
// - Knoten sind beleuchtete Schilder: tiefes Feld, Neonrand in Cyan mit doppeltem Schein (Cyan innen, Magenta außen)
// - Kanten sind Laserstrahlen in Cyan, auf denen ein Signal zum Ziel läuft (gestrichelt, wandert; ruht bei reduzierter
//   Bewegung); Pfeilspitzen in Magenta
// - Beschriftungen an Kanten und Notizen sind cremefarbene Bandetiketten mit dunkler Schrift, wie auf einer Kassette
// - Gruppen (subgraph) tragen ihren Titel in Orbitron-Versalien, Entscheidungen (Rauten) leuchten in Magenta
// - Sequenzen: Teilnehmer als Neonschilder, Lebenslinien gestrichelt in Violett, Aktivierungen in Magenta
export function synthMermaid(): MermaidLook {
  const bg = read('--synth-bg', '#120a24');
  const deep = read('--synth-bg-deep', '#0a0514');
  const surface = read('--synth-surface', '#1c1136');
  const fg = read('--synth-fg', '#f3eaff');
  const magenta = read('--synth-magenta', '#ff2bd6');
  const cyan = read('--synth-cyan', '#22e6ff');
  const violet = read('--synth-violet', '#8b5cff');
  const font = "'Exo 2', sans-serif";
  const display = "'Orbitron', sans-serif";
  // Farben als feste Werte: im SVG gelten die Variablen des Themes nicht sicher
  const themeCSS = [
    `.node rect, .node polygon, .node circle, .node path { stroke: ${cyan}; stroke-width: 1.5px; filter: drop-shadow(0 0 2px ${cyan}) drop-shadow(0 0 7px ${magenta}); }`,
    `.node polygon { stroke: ${magenta}; filter: drop-shadow(0 0 3px ${magenta}); }`,
    `.node .label, .nodeLabel { color: ${fg}; font-weight: 600; letter-spacing: 0.02em; }`,
    `.edgePath path, .flowchart-link { stroke: ${cyan}; stroke-width: 1.6px; stroke-dasharray: 10 4; filter: drop-shadow(0 0 3px ${cyan}); animation: synth-signal 1.2s linear infinite; }`,
    `@keyframes synth-signal { to { stroke-dashoffset: -14; } }`,
    `marker, .arrowheadPath, .marker { fill: ${magenta}; stroke: ${magenta}; }`,
    // nur Kanten mit Text bekommen ein Etikett; leere Beschriftungen bleiben unsichtbar
    `.edgeLabel, .edgeLabel span, .labelBkg { background-color: transparent; color: ${LABEL_INK}; }`,
    `.edgeLabel p:not(:empty) { background-color: ${LABEL_PAPER}; color: ${LABEL_INK}; padding: 0 6px; font-weight: 600; font-size: 12px; border-radius: 2px; }`,
    `.edgeLabel rect { fill: ${LABEL_PAPER}; opacity: 1; }`,
    `.cluster rect { stroke: ${violet}; stroke-dasharray: 6 3; fill: ${deep}; }`,
    `.cluster-label, .cluster text, .cluster-label span { font-family: ${display}; font-size: 12px; letter-spacing: 0.12em; text-transform: uppercase; color: ${cyan}; fill: ${cyan}; }`,
    `.note, rect.note { fill: ${LABEL_PAPER}; stroke: ${magenta}; }`,
    `.noteText, .noteText tspan { fill: ${LABEL_INK}; }`,
    `.actor { stroke: ${magenta}; fill: ${surface}; filter: drop-shadow(0 0 4px ${magenta}); }`,
    `text.actor, text.actor tspan { fill: ${fg}; font-family: ${display}; font-size: 13px; letter-spacing: 0.06em; }`,
    `.actor-line { stroke: ${violet}; stroke-dasharray: 4 4; }`,
    `.activation0, .activation1, .activation2 { fill: ${magenta}; stroke: ${magenta}; opacity: 0.6; }`,
    `.messageLine0, .messageLine1 { stroke: ${cyan}; filter: drop-shadow(0 0 2px ${cyan}); }`,
    `.messageText { fill: ${fg}; }`,
    `@media (prefers-reduced-motion: reduce) { .edgePath path, .flowchart-link { animation: none; } }`,
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
      edgeLabelBackground: LABEL_PAPER,
      noteBkgColor: LABEL_PAPER,
      noteTextColor: LABEL_INK,
      noteBorderColor: magenta,
      actorBorder: magenta,
      actorBkg: surface,
      actorTextColor: fg,
      signalColor: cyan,
      signalTextColor: fg,
      labelBoxBkgColor: surface,
      labelBoxBorderColor: magenta,
      labelTextColor: fg,
      loopTextColor: fg,
      activationBorderColor: magenta,
      fontFamily: font,
    },
    themeCSS,
  };
}
