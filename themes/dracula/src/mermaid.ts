// Gleiche Datei in themes/dracula und themes/alucard (ein Test vergleicht beide); nur palette.ts unterscheidet sich
import type { MermaidLook } from '@nexus/theme-sdk';

/** Palettenwerte für die Diagramme; theme.css setzt sie als --dracula-<name> */
export interface DraculaPalette {
  bg: string;
  'bg-deep': string;
  node: string;
  note: string;
  current: string;
  fg: string;
  comment: string;
  cyan: string;
  green: string;
  orange: string;
  pink: string;
  purple: string;
  red: string;
  yellow: string;
}

// Wert einer CSS-Variable auf <html> (beim Zeichnen sind Theme und Farbschema gesetzt); ohne Wert der Rückfall
function cssVar(name: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

/** Palette aus theme.css, fehlende Werte aus `fallback` */
export function readPalette(fallback: DraculaPalette): DraculaPalette {
  const out = {} as DraculaPalette;
  for (const key of Object.keys(fallback) as (keyof DraculaPalette)[]) out[key] = cssVar(`--dracula-${key}`, fallback[key]);
  return out;
}

// Mermaid in Dracula-Farben: Knoten mit Purple-Rand, Kanten in Cyan, Notizen in Yellow, Kreisdiagramme und Git-Graphen in
// den sieben Akzentfarben; Schrift wie in der App. Farben als feste Werte, weil im SVG die Variablen nicht sicher gelten.
export function draculaMermaid(fallback: DraculaPalette, darkMode: boolean): MermaidLook {
  const p = readPalette(fallback);
  const font = cssVar('--font-sans', 'system-ui, sans-serif');
  const accents = [p.purple, p.pink, p.cyan, p.green, p.orange, p.yellow, p.red, p.comment];
  const scale: Record<string, string> = {};
  accents.forEach((color, i) => {
    scale[`pie${i + 1}`] = color;
    scale[`git${i}`] = color;
    scale[`gitBranchLabel${i}`] = p.bg;
  });
  return {
    theme: 'base',
    fontFamily: font,
    themeVariables: {
      darkMode,
      fontFamily: font,
      fontSize: '14px',
      background: p.bg,
      textColor: p.fg,
      // Flussdiagramme, Zustände, Klassen
      mainBkg: p.node,
      primaryColor: p.node,
      primaryTextColor: p.fg,
      primaryBorderColor: p.purple,
      nodeBorder: p.purple,
      nodeTextColor: p.fg,
      secondaryColor: p.node,
      secondaryTextColor: p.fg,
      secondaryBorderColor: p.pink,
      tertiaryColor: p['bg-deep'],
      tertiaryTextColor: p.fg,
      tertiaryBorderColor: p.cyan,
      lineColor: p.cyan,
      edgeLabelBackground: p.bg,
      clusterBkg: p['bg-deep'],
      clusterBorder: p.comment,
      titleColor: p.fg,
      // Notizen
      noteBkgColor: p.note,
      noteBorderColor: p.yellow,
      noteTextColor: p.fg,
      // Sequenzdiagramme
      actorBkg: p.node,
      actorBorder: p.purple,
      actorTextColor: p.fg,
      actorLineColor: p.comment,
      signalColor: p.fg,
      signalTextColor: p.fg,
      labelBoxBkgColor: p.node,
      labelBoxBorderColor: p.pink,
      labelTextColor: p.fg,
      loopTextColor: p.fg,
      activationBkgColor: p['bg-deep'],
      activationBorderColor: p.pink,
      sequenceNumberColor: p.bg,
      // Gantt
      sectionBkgColor: p.current,
      altSectionBkgColor: p.bg,
      sectionBkgColor2: p['bg-deep'],
      taskBkgColor: p.purple,
      taskBorderColor: p.purple,
      taskTextColor: p.bg,
      taskTextOutsideColor: p.fg,
      taskTextLightColor: p.bg,
      taskTextDarkColor: p.fg,
      activeTaskBkgColor: p.pink,
      activeTaskBorderColor: p.pink,
      doneTaskBkgColor: p.green,
      doneTaskBorderColor: p.green,
      critBkgColor: p.red,
      critBorderColor: p.red,
      todayLineColor: p.orange,
      gridColor: p.current,
      // Kreisdiagramme und Git-Graphen
      ...scale,
      pieStrokeColor: p.bg,
      pieOuterStrokeColor: p.comment,
      pieTitleTextColor: p.fg,
      pieSectionTextColor: p.bg,
      pieLegendTextColor: p.fg,
      commitLabelColor: p.fg,
      commitLabelBackground: p.node,
      // Fehler
      errorBkgColor: p.red,
      errorTextColor: p.bg,
    },
    themeCSS: [
      '.node rect, .cluster rect, .actor, .note rect, rect.note { rx: 6px; ry: 6px; }',
      `.edgeLabel, .edgeLabel p { color: ${p.fg}; background-color: ${p.bg}; }`,
      `.cluster-label, .cluster span { color: ${p.comment}; }`,
      `marker path, .arrowheadPath { fill: ${p.cyan}; stroke: ${p.cyan}; }`,
    ].join(' '),
  };
}
