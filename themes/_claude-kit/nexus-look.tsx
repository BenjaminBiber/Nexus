// nexus-look.tsx: gemeinsamer Ablauf der Claude-Themes aus Nexus. Kopie aus themes/_claude-kit/,
// abgeglichen von scripts/themes/sync-claude-kit.mjs – hier nicht bearbeiten.
import type { Register, ThemeBackdropGlass, ThemeSetArgs } from 'claude-code'

import { makeColors } from './make-colors.js'

/** Was `makeColors` nimmt: Anker und Rollen je Modus */
export type ColorsInput = Parameters<typeof makeColors>[0]

/** Eine Szene hinter dem Gespräch: SVG-Dokument, Beschreibung und Glas der Panels darüber */
export type NexusScene = { svg: string; alt: string; glass?: ThemeBackdropGlass }

/** Was ein Claude-Theme aus Nexus zeichnet */
export type NexusLook = { colors: ColorsInput; scene?: NexusScene }

/** Werte der Theme-Einstellungen, wie Nexus sie in `nexus-settings.ts` schreibt */
export type NexusSettings = Readonly<Record<string, string | boolean>>

/** Name der Szene in `backdrop.scene`; der Backdrop-Hook zeichnet nur diese */
export const SCENE_NAME = 'nexus'

type On = Parameters<Register>[0]

/**
 * Setzt den Look bei jedem Sitzungsstart (und nach jedem Neuladen, wenn Nexus den Slot
 * ändert) und zeichnet die Szene, falls es eine gibt. Ein Theme ruft das genau einmal aus
 * `register` auf.
 */
export function registerLook(on: On, look: NexusLook): void {
  const { theme } = makeColors(look.colors)
  const scene = look.scene
  const args: ThemeSetArgs = scene
    ? { colors: theme, backdrop: { scene: SCENE_NAME, glass: scene.glass ?? {} } }
    : { colors: theme }

  on('session.start', async ($, e, next) => {
    await $.theme.set(args)
    return next(e)
  })

  if (scene === undefined) return

  on('ui.render', { component: 'Backdrop' }, ($, e, next) => {
    if (e.surface !== 'desktop' && e.surface !== 'mobile') return next(e)
    if (e.props.scene !== SCENE_NAME) return next(e)
    const { Svg } = $.ui.resolve(e)
    return <Svg source={scene.svg} alt={scene.alt} isInteractive />
  })
}

/** Wert einer Auswahl-Einstellung; Unbekanntes wird zum Standard */
export function choice<T extends string>(settings: NexusSettings, key: string, allowed: readonly T[], fallback: T): T {
  const value = settings[key]
  return typeof value === 'string' && (allowed as readonly string[]).includes(value) ? (value as T) : fallback
}

/** Wert eines Schalters; Unbekanntes wird zum Standard */
export function flag(settings: NexusSettings, key: string, fallback: boolean): boolean {
  const value = settings[key]
  return typeof value === 'boolean' ? value : fallback
}

/** Hält alle CSS-Animationen einer Szene an (Einstellung „still“); Szenen animieren nur über CSS */
export function freezeScene(svg: string): string {
  const open = svg.indexOf('>', svg.indexOf('<svg'))
  if (open < 0) return svg
  const style = '<style>*,*::before,*::after{animation-play-state:paused!important}</style>'
  return svg.slice(0, open + 1) + style + svg.slice(open + 1)
}
