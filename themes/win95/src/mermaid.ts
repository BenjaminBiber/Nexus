import type { MermaidLook } from '@nexus/theme-sdk';

// Mermaid im Schema „Windows 95“: Knoten silbergrau mit grauem Rand und schwarzem Schlagschatten wie ein erhabener Knopf,
// Gruppen weiß mit grauem Rand, Linien und Schrift schwarz in der Systemschrift. Farben aus den Variablen in theme.css.
export function win95Mermaid(): MermaidLook {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string) => css.getPropertyValue(name).trim();
  const [face, field, shadow, dark, tooltip, navy] = ['--w95-face', '--w95-field', '--w95-shadow', '--w95-dark', '--w95-tooltip', '--w95-navy'].map(v);
  return {
    theme: 'base',
    themeVariables: {
      // Ohne Anführungszeichen: Mermaid misst die Beschriftungen mit dem Namen als SVG-Attribut, dort brechen Anführungszeichen ihn
      fontFamily: v('--font-sans').replace(/['"]/g, ''), fontSize: '12px', background: field,
      primaryColor: face, primaryBorderColor: shadow, primaryTextColor: dark, mainBkg: face, nodeBorder: shadow,
      secondaryColor: face, secondaryBorderColor: shadow, tertiaryColor: field, tertiaryBorderColor: shadow,
      lineColor: dark, textColor: dark, titleColor: dark, edgeLabelBackground: face,
      clusterBkg: field, clusterBorder: shadow, noteBkgColor: tooltip, noteBorderColor: dark, noteTextColor: dark,
      actorBkg: face, actorBorder: shadow, actorTextColor: dark, signalColor: dark, signalTextColor: dark, labelBoxBkgColor: face,
    },
    // Beschriftungen stecken in <p>; ohne eigene Größe greift die Absatzschrift des Dokuments und sprengt die gemessenen Kästen
    themeCSS: `
      foreignObject p { margin: 0; font-size: 12px; line-height: 1.35; }
      .node rect, .node polygon, .node circle, .node ellipse, .node path, rect.actor, .note { filter: drop-shadow(1px 1px 0 ${dark}); }
      .cluster rect { rx: 0; ry: 0; }
      .cluster-label, .cluster-label span, .cluster text { color: ${navy}; fill: ${navy}; font-weight: 700; }
      .edgeLabel, .edgeLabel p, .edgeLabel rect { background-color: ${face}; fill: ${face}; }
    `,
  };
}
