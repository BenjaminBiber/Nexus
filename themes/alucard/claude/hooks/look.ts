// Alucard für Claude: helles Dracula, Flieder-Grund mit fast weißen Flächen, Violett als Akzent.
// Werte aus theme.css; Warnung und Erfolg bleiben die dunklen Orange- und Grüntöne von dort.
import type { NexusLook, NexusSettings } from './nexus-look'

/** Einstellungen, die dieses Claude-Theme liest */
export const CLAUDE_SETTINGS = [] as const

export function lookFor(_settings: NexusSettings): NexusLook {
  return {
    colors: {
      label: 'Alucard',
      only: 'light',
      roles: { accent: '#644ac9', danger: '#cb3a2a', success: '#14710a', warning: '#a34d14', pro: '#644ac9' },
      light: {
        gray: {
          anchors: {
            'surface-0': '#f7f5fc',
            'surface-1': '#faf8fe',
            'surface-2': '#fefefe',
            'surface-3': '#fefefe',
            'text-primary': '#1f1f1f',
            'text-secondary': '#3d3a4c',
            'text-muted': '#635f80',
          },
        },
      },
    },
  }
}
