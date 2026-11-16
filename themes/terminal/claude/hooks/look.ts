// Terminal für Claude: jedes der acht Farbschemata aus theme.css als eigene Variante, dazu die Szene aus scene.ts.
//
// Die Farbschemata setzen in theme.css nur Grundwerte (--term-*), der Kern leitet daraus ab:
// --bg = --surface = --term-bg, --surface-alt = --nav-active-bg = --term-bg-alt, --accent = --primary = --ok = ANSI-Grün.
// Weil --surface dunkler ist als --surface-alt und --primary gleich --accent, gilt hier:
//   surface-0      ← --term-bg (--bg)
//   surface-1..3   ← --term-bg-alt (--surface-alt, --nav-active-bg); hellere Stufen wie --term-line machen
//                    gedämpften Text auf Menüs unlesbar
//   text-primary   ← --term-fg (--text), text-secondary ← --term-fg-soft (--text-soft), text-muted ← --term-muted (--muted)
//   accent         ← --term-accent (ANSI-Grün, der Prompt), success ← ANSI-Grün (--ok)
//   danger         ← ANSI-Rot (--danger), warning ← ANSI-Gelb (--warn)
//   pro            ← ANSI-Magenta statt --primary (= Akzent), damit Literale im Code nicht dieselbe Farbe wie Schlüsselwörter tragen
//
// Nachjustiert für Claude Desktop (sonst „Desktop won't paint“), alles andere unverändert aus theme.css:
//   dracula     text-muted #8c95c4 → #9aa3d0
//   tokyonight  text-muted #737aa2 → #868eb8
//   solarized   text-primary #93a1a1 → #c4cdcd, text-secondary #839496 → #a3b1b2, text-muted #6c8289 → #8aa0a5
//   nord        text-muted #8a93a5 → #a0a8ba, success-Stufe 400 heller (#c0dba8), weil die Flächen hell sind
//   green/amber Phosphor bleibt einfarbig in Akzent und Text; Fehler, Warnung und Erfolg bekommen trotzdem
//               einen roten, bernsteinfarbenen und grünen Ton (theme.css macht sie nur heller, Claude bräuchte die Bedeutung)
import type { ColorsInput, NexusLook, NexusSettings } from './nexus-look'
import { choice, flag } from './nexus-look'
import { sceneFor, type SceneColors } from './scene'

/** Einstellungen, die dieses Claude-Theme liest */
export const CLAUDE_SETTINGS = ['palette', 'crt'] as const

const PALETTES = ['github', 'dracula', 'gruvbox', 'tokyonight', 'solarized', 'nord', 'green', 'amber'] as const
type Palette = (typeof PALETTES)[number]

type Role = NonNullable<ColorsInput['roles']>['success']

/** Grundwerte eines Farbschemas aus theme.css, Claude-Anpassungen schon eingerechnet */
type Variant = {
  label: string
  bg: string
  bgAlt: string
  fg: string
  fgSoft: string
  muted: string
  red: string
  green: string
  yellow: string
  blue: string
  magenta: string
  cyan: string
  /** Rollen, die vom ANSI-Wert abweichen (Phosphor, Nord) */
  roles?: Partial<Record<'accent' | 'danger' | 'success' | 'warning' | 'pro', Role>>
  /** Highlighting in Claude, wo es ein passendes gibt */
  codeTheme?: string
}

const VARIANTS: Record<Palette, Variant> = {
  github: {
    label: 'GitHub Dark',
    bg: '#0d1117', bgAlt: '#161b22', fg: '#c9d1d9', fgSoft: '#b1bac4', muted: '#8b949e',
    red: '#ff7b72', green: '#3fb950', yellow: '#d29922', blue: '#58a6ff', magenta: '#bc8cff', cyan: '#39c5cf',
    codeTheme: 'github-dark',
  },
  dracula: {
    label: 'Dracula',
    bg: '#282a36', bgAlt: '#343746', fg: '#f8f8f2', fgSoft: '#e2e2dc', muted: '#9aa3d0',
    red: '#ff5555', green: '#50fa7b', yellow: '#f1fa8c', blue: '#bd93f9', magenta: '#ff79c6', cyan: '#8be9fd',
    codeTheme: 'dracula',
  },
  gruvbox: {
    label: 'Gruvbox',
    bg: '#282828', bgAlt: '#32302f', fg: '#ebdbb2', fgSoft: '#d5c4a1', muted: '#a89984',
    red: '#fb4934', green: '#b8bb26', yellow: '#fabd2f', blue: '#83a598', magenta: '#d3869b', cyan: '#8ec07c',
    codeTheme: 'gruvbox-dark-medium',
  },
  tokyonight: {
    label: 'Tokyo Night',
    bg: '#1a1b26', bgAlt: '#24283b', fg: '#c0caf5', fgSoft: '#a9b1d6', muted: '#868eb8',
    red: '#f7768e', green: '#9ece6a', yellow: '#e0af68', blue: '#7aa2f7', magenta: '#bb9af7', cyan: '#7dcfff',
    codeTheme: 'tokyo-night',
  },
  solarized: {
    label: 'Solarized Dark',
    bg: '#002b36', bgAlt: '#073642', fg: '#c4cdcd', fgSoft: '#a3b1b2', muted: '#8aa0a5',
    red: '#e0504d', green: '#859900', yellow: '#b58900', blue: '#268bd2', magenta: '#6c71c4', cyan: '#2aa198',
    codeTheme: 'solarized-dark',
  },
  nord: {
    label: 'Nord',
    bg: '#2e3440', bgAlt: '#3b4252', fg: '#e5e9f0', fgSoft: '#d8dee9', muted: '#a0a8ba',
    red: '#bf616a', green: '#a3be8c', yellow: '#ebcb8b', blue: '#81a1c1', magenta: '#b48ead', cyan: '#88c0d0',
    roles: { success: { seed: '#a3be8c', stops: { 400: '#c0dba8' } } },
    codeTheme: 'nord',
  },
  green: {
    label: 'Phosphor grün',
    bg: '#0a0f0a', bgAlt: '#0f1a0f', fg: '#33ff66', fgSoft: '#2bd957', muted: '#20a443',
    red: '#c6ffc6', green: '#33ff66', yellow: '#99ff99', blue: '#66ff8c', magenta: '#80ffa0', cyan: '#4dff79',
    roles: { danger: '#ff6b57', success: '#66ff8c', warning: '#e6d64d', pro: '#4dffc3' },
  },
  amber: {
    label: 'Phosphor bernstein',
    bg: '#120b00', bgAlt: '#1c1200', fg: '#ffb000', fgSoft: '#e69e00', muted: '#b07a00',
    red: '#ffdc99', green: '#ffb000', yellow: '#ffcc4d', blue: '#ffc233', magenta: '#ffd280', cyan: '#ffbf26',
    roles: { danger: '#ff6a3d', success: '#c8d64a', warning: '#ffcc4d', pro: '#ffe0a3' },
  },
}

function colorsOf(v: Variant): ColorsInput {
  return {
    label: `Terminal · ${v.label}`,
    only: 'dark',
    roles: { accent: v.green, danger: v.red, success: v.green, warning: v.yellow, pro: v.magenta, ...v.roles },
    dark: {
      gray: {
        anchors: {
          'surface-0': v.bg,
          'surface-1': v.bgAlt,
          'surface-2': v.bgAlt,
          'surface-3': v.bgAlt,
          'text-primary': v.fg,
          'text-secondary': v.fgSoft,
          'text-muted': v.muted,
        },
      },
    },
    ...(v.codeTheme ? { codeTheme: { dark: v.codeTheme } } : {}),
  }
}

/** Farben der Szene: die rohen Werte aus theme.css (Phosphor bleibt dort ganz einfarbig) */
function sceneColorsOf(v: Variant): SceneColors {
  return { bg: v.bg, fg: v.fg, muted: v.muted, accent: v.green, ok: v.green, warn: v.yellow, hex: v.cyan, path: v.blue }
}

export function lookFor(settings: NexusSettings): NexusLook {
  const palette = choice(settings, 'palette', PALETTES, 'github')
  const crt = flag(settings, 'crt', false)
  const variant = VARIANTS[palette]
  return {
    colors: colorsOf(variant),
    scene: {
      svg: sceneFor(sceneColorsOf(variant), crt),
      alt: crt
        ? 'Dunkles Terminal wie auf einem Röhrenmonitor: Scanlines, dunkle Ränder, gedimmte Protokollzeilen ziehen am rechten Rand langsam nach oben, oben links blinkt ein Block-Cursor'
        : 'Dunkles Terminal: gedimmte Protokollzeilen ziehen am rechten Rand langsam nach oben, oben links blinkt ein Block-Cursor',
      glass: { blur: 12, tint: variant.bg, opacity: 0.62 },
    },
  }
}
