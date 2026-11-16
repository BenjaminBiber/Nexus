// Signal für Claude: kühles Grau mit weißen Flächen, Petrol als Akzent.
// Rollen erbt Signal aus den Basisvariablen. Werte aus theme.css; surface-3 gleich surface-2 (--nav-active-bg des hellen Schemas ist Weiß), wie im hellen Modus verlangt.
import type { NexusLook, NexusSettings } from './nexus-look'

/** Einstellungen, die dieses Claude-Theme liest */
export const CLAUDE_SETTINGS = [] as const

export function lookFor(_settings: NexusSettings): NexusLook {
  return {
    colors: {
      label: 'Signal',
      only: 'light',
      roles: { accent: '#0e7490', danger: '#f04438', success: '#12b76a', warning: '#f79009', pro: '#12343d' },
      light: {
        gray: {
          anchors: {
            'surface-0': '#f4f6f8',
            'surface-1': '#f8fafc',
            'surface-2': '#ffffff',
            'surface-3': '#ffffff',
            'text-primary': '#0f172a',
            'text-secondary': '#334155',
            'text-muted': '#64748b',
          },
        },
      },
      codeTheme: { light: 'github-light' },
    },
  }
}
