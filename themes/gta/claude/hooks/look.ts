// GTA für Claude: Flächen und Text aus den Tokens des gewählten Stils in theme.css („GTA V“ bzw. „San Andreas“),
// die color-mix-Stufen des gemeinsamen Abschnitts als feste Werte ausgerechnet. `accent` wählt die Rolle accent wie
// data-gta-accent. Pro ist das Violett der Statusspalten (V: --gta-c-in_planning, SA: --gta-c-planned) statt --primary:
// --primary ist hier das Weiß des Textes, und ein farbloser Seed macht Literale im Code und die Pro-Marke grau.
import type { NexusLook, NexusSettings } from './nexus-look'
import { choice } from './nexus-look'
import { sceneFor, type Style } from './scene'

/** Einstellungen, die dieses Claude-Theme liest */
export const CLAUDE_SETTINGS = ['style', 'accent'] as const

type Accent = 'blau' | 'gruen' | 'orange'

type Anchors = {
  'surface-0': string
  'surface-1': string
  'surface-2': string
  'surface-3': string
  'text-primary': string
  'text-secondary': string
  'text-muted': string
}

type StyleColors = {
  label: string
  anchors: Anchors
  danger: string
  success: string
  warning: string
  pro: string
  accents: Record<Accent, string>
}

const STYLES: Record<Style, StyleColors> = {
  // --gta-bg #07050c, --gta-text #fff; Flächen = Text zu 4 %, 7 % und 14 % in den Grund gemischt,
  // text-secondary 85 %, text-muted das Weiß zu 62 % Deckkraft auf dem Grund
  v: {
    label: 'GTA V',
    anchors: {
      'surface-0': '#07050c',
      'surface-1': '#110f16',
      'surface-2': '#18171d',
      'surface-3': '#2a282e',
      'text-primary': '#ffffff',
      'text-secondary': '#dadadb',
      'text-muted': '#a1a0a3',
    },
    danger: '#dc2f2f',
    success: '#72cc72',
    warning: '#f0c850',
    pro: '#a47ee8',
    accents: { blau: '#65b4d4', gruen: '#abedab', orange: '#ffa357' },
  },
  // --gta-bg #000, --gta-text #e1e1e1, --gta-text-dim #9fb0c2; Mischungen wie bei GTA V
  sa: {
    label: 'GTA San Andreas',
    anchors: {
      'surface-0': '#000000',
      'surface-1': '#090909',
      'surface-2': '#101010',
      'surface-3': '#202020',
      'text-primary': '#e1e1e1',
      'text-secondary': '#bfbfbf',
      'text-muted': '#9fb0c2',
    },
    danger: '#d0464a',
    success: '#5fbf4f',
    warning: '#e6c84b',
    pro: '#b06fd8',
    accents: { blau: '#accbf1', gruen: '#74c85a', orange: '#c79a2a' },
  },
}

export function lookFor(settings: NexusSettings): NexusLook {
  const style = choice<Style>(settings, 'style', ['v', 'sa'], 'v')
  const accentKey = choice<Accent>(settings, 'accent', ['blau', 'gruen', 'orange'], 'blau')
  const s = STYLES[style]
  const accent = s.accents[accentKey]
  return {
    colors: {
      label: s.label,
      only: 'dark',
      roles: { accent, danger: s.danger, success: s.success, warning: s.warning, pro: s.pro },
      dark: { gray: { anchors: s.anchors } },
    },
    scene: sceneFor(style, accent, s.anchors['surface-0']),
  }
}
