// Claude-Theme „Synthwave“: setzt Farben und Szene aus look.ts mit den Einstellungen, die Nexus schreibt.
import type { Register } from 'claude-code'

import { lookFor } from './look'
import { registerLook } from './nexus-look'
import { settings } from './nexus-settings'

export const register: Register = on => {
  registerLook(on, lookFor(settings))
}
