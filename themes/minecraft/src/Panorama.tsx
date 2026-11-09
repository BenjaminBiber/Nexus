// Hintergrund wie im Titelbildschirm: die 6 Panorama-Bilder aus dem Asset-Index als Würfel um den Betrachter,
// langsam drehend (theme.css). Die Flächen blenden erst ein, wenn alle sechs Bilder da sind; vorher bleibt der
// Himmel-Erde-Verlauf der Shell zu sehen. Hinter Untermenüs wird das Panorama unscharf gezeichnet, auf dem
// Titelbildschirm scharf mit dem dunklen Overlay-Verlauf darüber.
import type { CSSProperties } from 'react';
import { useResourceUrl } from './context.js';
import { PANORAMA_OVERLAY, panorama } from './resources.js';

const FACES = ['front', 'right', 'back', 'left', 'top', 'bottom'] as const;

export function Panorama({ blur }: { blur: boolean }) {
  // Reihenfolge der Flächen = Reihenfolge der Bilder panorama_0..5 (theme.css ordnet sie dem Würfel zu)
  const urls = FACES.map((_, i) => useResourceUrl(panorama(i)));
  const overlay = useResourceUrl(PANORAMA_OVERLAY);
  const ready = urls.every(Boolean);
  const classes = ['mc-panorama', ready ? 'mc-panorama--ready' : '', blur ? 'mc-panorama--blur' : ''].filter(Boolean).join(' ');
  return (
    <div className={classes} aria-hidden="true">
      <div className="mc-panorama-cube">
        {FACES.map((face) => (
          <div key={face} className={`mc-panorama-face mc-panorama-face--${face}`} />
        ))}
      </div>
      {/* Das Overlay steht immer im Baum und wird bei Unschärfe per CSS ausgeblendet; die Variable auch hier,
          damit das Panorama außerhalb der Shell (Tests, einzelner Slot) ohne die Variablen des Gates auskommt. */}
      <div className="mc-panorama-overlay" style={overlay ? ({ '--mc-panorama-overlay': `url("${overlay}")` } as CSSProperties) : undefined} />
    </div>
  );
}
