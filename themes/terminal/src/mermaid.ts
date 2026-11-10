import type { MermaidLook } from '@nexus/theme-sdk';

// Wert einer CSS-Variable auf <html> (beim Zeichnen sind Theme und Farbschema gesetzt); ohne Wert der Rückfall
function cssVar(name: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

// Mermaid im Terminal: Farben des gewählten Farbschemas, Festbreitenschrift, eckige Knoten und gerade Linien
export function terminalMermaid(): MermaidLook {
  const bg = cssVar('--bg', '#0d1117');
  const alt = cssVar('--surface-alt', '#161b22');
  const fg = cssVar('--text', '#c9d1d9');
  const line = cssVar('--line-strong', '#484f58');
  const accent = cssVar('--accent', '#3fb950');
  const path = cssVar('--term-path', '#58a6ff');
  const font = cssVar('--font-mono', 'monospace');
  return {
    theme: 'base',
    fontFamily: font,
    themeVariables: {
      darkMode: true,
      background: bg,
      fontFamily: font,
      fontSize: '13px',
      primaryColor: alt,
      primaryTextColor: fg,
      primaryBorderColor: accent,
      secondaryColor: bg,
      secondaryBorderColor: path,
      tertiaryColor: bg,
      tertiaryBorderColor: line,
      lineColor: line,
      textColor: fg,
      mainBkg: alt,
      nodeBorder: accent,
      clusterBkg: bg,
      clusterBorder: line,
      edgeLabelBackground: bg,
      noteBkgColor: alt,
      noteBorderColor: path,
      noteTextColor: fg,
    },
    themeCSS: 'rect, polygon, .node rect, .cluster rect { rx: 0 !important; ry: 0 !important; } .edgePath path, .flowchart-link { stroke-linecap: square; }',
  };
}
