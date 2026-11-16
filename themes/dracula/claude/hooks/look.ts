// Dracula für Claude: Hintergrund, Seitenleiste, Fläche und Auswahl als vier Stufen, Purpur als Akzent.
// Werte aus theme.css; Rosa (--primary) als Pro-Farbe. surface-3 ist dunkler als --nav-active-bg (#44475a),
// sonst fällt grüne Schrift (text-success) auf dieser Fläche unter 3:1 und Desktop malt das Theme nicht.
import type { NexusLook, NexusSettings } from './nexus-look'

/** Einstellungen, die dieses Claude-Theme liest */
export const CLAUDE_SETTINGS = [] as const

export function lookFor(_settings: NexusSettings): NexusLook {
  return {
    colors: {
      label: 'Dracula',
      only: 'dark',
      roles: { accent: '#bd93f9', danger: '#ff5555', success: '#50fa7b', warning: '#ffb86c', pro: '#ff79c6' },
      dark: {
        gray: {
          anchors: {
            'surface-0': '#21222c',
            'surface-1': '#252631',
            'surface-2': '#282a36',
            'surface-3': '#383a4c',
            'text-primary': '#f8f8f2',
            'text-secondary': '#e2e2dc',
            'text-muted': '#a4acd4',
          },
        },
      },
      codeTheme: { dark: 'dracula' },
    },
  }
}
