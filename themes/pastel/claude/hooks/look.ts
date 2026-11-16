// Paper Notes für Claude: warmes Punktpapier, Fineliner-Tinte als Schrift, Indigo als Akzent.
// Abweichung von theme.css: --surface-alt (#f5f0df) ist dunkler als --bg, im hellen Modus muss surface-0 aber die dunkelste
// Fläche sein; daher Papierton als surface-0, --bg als surface-1, --surface für surface-2 und -3. Die gelbe
// --nav-active-bg bricht die Reihenfolge und entfällt; --primary (Marker-Gelb) ist die Pro-Farbe.
import type { NexusLook, NexusSettings } from './nexus-look'

/** Einstellungen, die dieses Claude-Theme liest */
export const CLAUDE_SETTINGS = [] as const

export function lookFor(_settings: NexusSettings): NexusLook {
  return {
    colors: {
      label: 'Paper Notes',
      only: 'light',
      roles: { accent: '#55518c', danger: '#a43b58', success: '#6ebd90', warning: '#966a20', pro: '#ffeb85' },
      light: {
        gray: {
          anchors: {
            'surface-0': '#f5f0df',
            'surface-1': '#faf8f0',
            'surface-2': '#fffdf5',
            'surface-3': '#fffdf5',
            'text-primary': '#292e43',
            'text-secondary': '#414557',
            'text-muted': '#676777',
          },
        },
      },
    },
  }
}
