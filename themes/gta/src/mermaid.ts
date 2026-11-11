import type { MermaidLook } from '@nexus/theme-sdk';

// Wert eines Tokens auf <html> (beim Zeichnen sind Stil und Akzentfarbe gesetzt); ohne Wert der Rückfall
function read(name: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

/** Knoten und Beschriftungen sind schwarze Kästen wie die Paneele der Menüs */
const BOX = '#000000';

// Mermaid im Look des aktiven Stils; die Tokens kommen von <html>, Rückfall sind die Werte von GTA V:
// - Knoten sind schwarze Kästen mit Linie in der Akzentfarbe, Entscheidungen (Rauten) in Gelb wie die Marker der Karte
// - Kanten und Pfeilspitzen hell in der Textfarbe, Beschriftungen an Kanten als schwarze Kästen mit heller Schrift
// - Gruppen (subgraph) gestrichelt in der Akzentfarbe, Titel in der Titelschrift des Stils in Versalien
// - Sequenzen: Teilnehmer als schwarze Kästen, Lebenslinien gestrichelt
// Keine Animation: Diagramme stehen still, auch ohne reduzierte Bewegung
export function gtaMermaid(): MermaidLook {
  const bg = read('--gta-bg', '#07050c');
  const text = read('--gta-text', '#ffffff');
  const dim = read('--gta-text-dim', 'rgba(255, 255, 255, 0.62)');
  const line = read('--gta-line', 'rgba(255, 255, 255, 0.22)');
  const accent = read('--gta-accent', '#65b4d4');
  const ink = read('--gta-accent-ink', '#000000');
  const warn = read('--gta-warn', '#f0c850');
  const font = read('--gta-font-ui', "'Barlow', 'Segoe UI', Arial, sans-serif");
  const title = read('--gta-font-title', "'Barlow Condensed', 'Arial Narrow', Impact, sans-serif");
  // Farben als feste Werte: im SVG gelten die Variablen des Themes nicht sicher
  const themeCSS = [
    `.node rect, .node polygon, .node circle, .node path { fill: ${BOX}; stroke: ${accent}; stroke-width: 2px; }`,
    `.node polygon { stroke: ${warn}; }`,
    `.node .label, .nodeLabel { color: ${text}; font-weight: 600; }`,
    `.edgePath path, .flowchart-link { stroke: ${text}; stroke-width: 1.6px; }`,
    `marker, .arrowheadPath, .marker { fill: ${text}; stroke: ${text}; }`,
    // nur Kanten mit Text bekommen einen Kasten; leere Beschriftungen bleiben unsichtbar
    `.edgeLabel, .edgeLabel span, .labelBkg { background-color: transparent; color: ${text}; }`,
    `.edgeLabel p:not(:empty) { background-color: ${BOX}; color: ${text}; padding: 0 6px; font-size: 12px; }`,
    `.edgeLabel rect { fill: ${BOX}; opacity: 1; }`,
    `.cluster rect { fill: ${bg}; stroke: ${accent}; stroke-dasharray: 6 3; }`,
    `.cluster-label, .cluster text, .cluster-label span { font-family: ${title}; font-size: 13px; letter-spacing: 0.08em; text-transform: uppercase; color: ${accent}; fill: ${accent}; }`,
    `.note, rect.note { fill: ${warn}; stroke: ${BOX}; }`,
    `.noteText, .noteText tspan { fill: ${BOX}; }`,
    `.actor { fill: ${BOX}; stroke: ${accent}; }`,
    `text.actor, text.actor tspan { fill: ${text}; font-family: ${title}; font-size: 13px; letter-spacing: 0.06em; }`,
    `.actor-line { stroke: ${dim}; stroke-dasharray: 4 4; }`,
    `.activation0, .activation1, .activation2 { fill: ${accent}; stroke: ${accent}; }`,
    `.messageLine0, .messageLine1 { stroke: ${text}; }`,
    `.messageText { fill: ${text}; }`,
    `.labelBox { fill: ${accent}; stroke: ${accent}; }`,
    `.labelText, .labelText tspan { fill: ${ink}; }`,
    `.loopLine { stroke: ${line}; }`,
  ].join(' ');
  return {
    theme: 'base',
    fontFamily: font,
    themeVariables: {
      darkMode: true,
      background: bg,
      primaryColor: BOX,
      primaryTextColor: text,
      primaryBorderColor: accent,
      secondaryColor: BOX,
      tertiaryColor: bg,
      lineColor: text,
      textColor: text,
      mainBkg: BOX,
      nodeBorder: accent,
      clusterBkg: bg,
      clusterBorder: accent,
      titleColor: accent,
      edgeLabelBackground: BOX,
      noteBkgColor: warn,
      noteTextColor: BOX,
      noteBorderColor: BOX,
      actorBorder: accent,
      actorBkg: BOX,
      actorTextColor: text,
      signalColor: text,
      signalTextColor: text,
      labelBoxBkgColor: accent,
      labelBoxBorderColor: accent,
      labelTextColor: ink,
      loopTextColor: text,
      activationBorderColor: accent,
      fontFamily: font,
    },
    themeCSS,
  };
}
