// Windows 95 für Claude: Silbergrau als Flächen, Marineblau als Akzent, dazu die Petrol-Szene aus scene.ts.
// Werte aus theme.css, für das helle Schema nachjustiert: Karten und Panels eine Spur heller als die
// Seite (#c0c0c0), weil im hellen Modus surface-0 die dunkelste Fläche ist und surface-2 = surface-3
// sein muss (--nav-active-bg ist Navy und taugt nicht als Fläche). --muted (#808080) ist auf Silbergrau
// zu schwach, daher dunkler. pro ist Magenta statt --primary (Grau ohne Farbton).
import type { NexusLook, NexusSettings } from './nexus-look'
import { SCENE_SVG } from './scene'

/** Einstellungen, die dieses Claude-Theme liest */
export const CLAUDE_SETTINGS = [] as const

export function lookFor(_settings: NexusSettings): NexusLook {
  return {
    colors: {
      label: 'Windows 95',
      only: 'light',
      roles: { accent: '#000080', danger: '#c00000', success: '#008000', warning: '#a07800', pro: '#800080' },
      light: {
        gray: {
          anchors: {
            'surface-0': '#c0c0c0',
            'surface-1': '#c8c8c8',
            'surface-2': '#d4d4d4',
            'surface-3': '#d4d4d4',
            'text-primary': '#000000',
            'text-secondary': '#202020',
            'text-muted': '#505050',
          },
        },
        // Dunkler als die Standardfarben, damit Diff-Text auf Silbergrau lesbar bleibt
        git: {
          added: '#006000',
          removed: '#900000',
          modified: '#705000',
          merged: '#700070',
          closed: '#900000',
          conflicting: '#904000',
          draft: '#484848',
        },
      },
    },
    scene: {
      svg: SCENE_SVG,
      alt: 'Petrolfarbene Fläche wie ein Desktop der Neunziger, nach oben fein gerastert, mit langsam ziehenden Pixelwolken',
      // Flaches, fast deckendes Silbergrau statt starkem Milchglas
      glass: { blur: 2, tint: '#c0c0c0', opacity: 0.8 },
    },
  }
}
