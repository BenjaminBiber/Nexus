// Slot „Dialog“: die sichtbare Hülle eines Dialogs als Fenster des Spiels (popup/background, 9-Slice Rand 6);
// Rolle und Überschrift kommen vom Host, Fokusfalle und Esc bleiben im Kern. Die Abdunkelung dahinter setzt der Host.
import type { DialogProps } from '@nexus/theme-sdk';
import { sliceStyle } from '../ui/Sprite.js';

export function McDialog({ labelledBy, role, className, children }: DialogProps) {
  return (
    <div role={role} aria-labelledby={labelledBy} className={['mc-popup', 'mc-dialog', className].filter(Boolean).join(' ')} style={sliceStyle('popup')}>
      {children}
    </div>
  );
}
