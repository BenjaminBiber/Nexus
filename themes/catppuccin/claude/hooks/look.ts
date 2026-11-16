// Catppuccin Mocha für Claude: Crust, Mantle, Base und Surface 0 als Flächen, Mauve als Akzent.
// Werte aus theme.css; Warnung in Yellow statt Peach, damit sie bernsteinfarben bleibt.
import type { NexusLook, NexusSettings } from './nexus-look'

/** Einstellungen, die dieses Claude-Theme liest */
export const CLAUDE_SETTINGS = [] as const

export function lookFor(_settings: NexusSettings): NexusLook {
  return {
    colors: {
      label: 'Catppuccin',
      only: 'dark',
      roles: { accent: '#cba6f7', danger: '#f38ba8', success: '#a6e3a1', warning: '#f9e2af', pro: '#b4befe' },
      dark: {
        gray: {
          anchors: {
            'surface-0': '#11111b',
            'surface-1': '#181825',
            'surface-2': '#1e1e2e',
            'surface-3': '#313244',
            'text-primary': '#cdd6f4',
            'text-secondary': '#bac2de',
            'text-muted': '#9399b2',
          },
        },
      },
      codeTheme: { dark: 'catppuccin-mocha' },
    },
  }
}
