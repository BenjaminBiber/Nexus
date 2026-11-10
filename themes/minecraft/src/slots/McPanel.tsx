// Slot „Panel“: Bereiche des Hosts (z. B. Einstellungen) auf dem Popup-Hintergrund des Spiels (popup/background, 9-Slice Rand 6)
import type { PanelProps } from '@nexus/theme-sdk';
import { sliceStyle } from '../ui/Sprite.js';

export function McPanel({ title, className, children }: PanelProps) {
  return (
    <div className={['mc-popup', className].filter(Boolean).join(' ')} style={sliceStyle('popup')}>
      {title && <h2 className="mc-popup-title mc-shadow">{title}</h2>}
      {children}
    </div>
  );
}
