// Synthwave für Claude: Flächen, Text und Rollen je Farbschema aus theme.css (--bg, --surface-alt, --surface,
// --nav-active-bg, --text, --text-soft, --muted; Akzent Cyan, Pro Magenta), dazu die Neon-Szene aus scene.ts.
import { choice, freezeScene, type NexusLook, type NexusSettings } from './nexus-look'
import { buildScene, type HorizonMode, type SceneColors, type SkyPhase } from './scene'

/** Einstellungen, die dieses Claude-Theme liest */
export const CLAUDE_SETTINGS = ['palette', 'sky', 'horizon'] as const

type PaletteId = 'outrun' | 'miami' | 'nightcall'

type Palette = {
  label: string
  /** Anker für makeColors: surface-0 … text-muted */
  gray: {
    'surface-0': string
    'surface-1': string
    'surface-2': string
    'surface-3': string
    'text-primary': string
    'text-secondary': string
    'text-muted': string
  }
  roles: { accent: string; danger: string; success: string; warning: string; pro: string }
  scene: SceneColors
}

// surface-1 = color-mix(surface 60 %, bg), text-secondary = color-mix(fg 75 %, muted), wie theme.css sie ableitet
const PALETTES: Record<PaletteId, Palette> = {
  outrun: {
    label: 'Synthwave Outrun',
    gray: {
      'surface-0': '#120a24',
      'surface-1': '#1e143b',
      'surface-2': '#261a4b',
      'surface-3': '#261a4b',
      'text-primary': '#f3eaff',
      'text-secondary': '#e4d9f6',
      'text-muted': '#b5a6db',
    },
    roles: { accent: '#22e6ff', danger: '#ff4d6d', success: '#3dffa8', warning: '#ffb347', pro: '#ff2bd6' },
    scene: {
      bg: '#120a24',
      deep: '#0a0514',
      fg: '#f3eaff',
      violet: '#8b5cff',
      magenta: '#ff2bd6',
      cyan: '#22e6ff',
      sunTop: '#ffd23f',
      sunBottom: '#ff3d81',
    },
  },
  miami: {
    label: 'Synthwave Miami',
    gray: {
      'surface-0': '#101a2b',
      'surface-1': '#192a43',
      'surface-2': '#1f3453',
      'surface-3': '#1f3453',
      'text-primary': '#f4fbff',
      'text-secondary': '#e3eef5',
      'text-muted': '#afc5d8',
    },
    roles: { accent: '#2ee6c8', danger: '#ff5470', success: '#52f2a0', warning: '#ffb347', pro: '#ff5fa2' },
    scene: {
      bg: '#101a2b',
      deep: '#0a111d',
      fg: '#f4fbff',
      violet: '#7f8cff',
      magenta: '#ff5fa2',
      cyan: '#2ee6c8',
      sunTop: '#ffe066',
      sunBottom: '#ff7a45',
    },
  },
  nightcall: {
    label: 'Synthwave Nightcall',
    gray: {
      'surface-0': '#070b1e',
      'surface-1': '#111938',
      'surface-2': '#182249',
      'surface-3': '#182249',
      'text-primary': '#e6ecff',
      'text-secondary': '#d5dcf5',
      'text-muted': '#a0acd8',
    },
    roles: { accent: '#3d8bff', danger: '#ff4d7a', success: '#45e0b0', warning: '#ffc266', pro: '#c44dff' },
    scene: {
      bg: '#070b1e',
      deep: '#03050f',
      fg: '#e6ecff',
      violet: '#5a4dff',
      magenta: '#c44dff',
      cyan: '#3d8bff',
      sunTop: '#ff9de1',
      sunBottom: '#7a3dff',
    },
  },
}

/** Phase zur lokalen Stunde wie src/sky.ts: 05–09 Morgenrot, 09–17 Tag, 17–21 Dämmerung, sonst Nacht */
function skyPhase(date: Date): SkyPhase {
  const hour = date.getHours()
  if (hour >= 5 && hour < 9) return 'dawn'
  if (hour >= 9 && hour < 17) return 'day'
  if (hour >= 17 && hour < 21) return 'dusk'
  return 'night'
}

/** „Nach Tageszeit“ gilt für die ganze Sitzung: Uhrzeit beim Laden des Moduls */
const AUTO_PHASE = skyPhase(new Date())

/** Himmel und Sonne je Phase für die Bildbeschreibung */
const ALT_SKY: Record<SkyPhase, string> = {
  dawn: 'Morgenrot, tief stehende gestreifte Sonne rechts vor einem rosa leuchtenden Grat',
  day: 'Heller violetter Himmel, hoch stehende gestreifte Sonne rechts vor einem rosa leuchtenden Grat',
  dusk: 'Violetter Nachthimmel mit funkelnden Sternen, große gestreifte Sonne rechts vor einem rosa leuchtenden Grat',
  night: 'Sternennacht mit Mondsichel, die gestreifte Sonne fast hinter dem Horizont versunken',
}

/** Bildbeschreibung der Szene */
function altOf(phase: SkyPhase, horizon: HorizonMode): string {
  const grid = horizon === 'off' ? '' : horizon === 'animated' ? ', unten ein Neon-Gitter, das auf den Betrachter zuläuft' : ', unten ein Neon-Gitter'
  return `${ALT_SKY[phase]}, links Drahtgitter-Berge mit Neonkanten${grid}`
}

export function lookFor(settings: NexusSettings): NexusLook {
  const palette = PALETTES[choice(settings, 'palette', ['outrun', 'miami', 'nightcall'] as const, 'outrun')]
  const skySetting = choice(settings, 'sky', ['auto', 'dawn', 'day', 'dusk', 'night'] as const, 'auto')
  const phase: SkyPhase = skySetting === 'auto' ? AUTO_PHASE : skySetting
  const horizon: HorizonMode = choice(settings, 'horizon', ['animated', 'still', 'off'] as const, 'animated')

  const svg = buildScene(palette.scene, phase, horizon)
  return {
    colors: {
      label: palette.label,
      only: 'dark',
      roles: palette.roles,
      dark: { gray: { anchors: palette.gray } },
      codeTheme: { dark: 'synthwave-84' },
    },
    scene: {
      svg: horizon === 'still' ? freezeScene(svg) : svg,
      alt: altOf(phase, horizon),
      glass: { blur: 16, tint: palette.gray['surface-0'], opacity: 0.58 },
    },
  }
}
