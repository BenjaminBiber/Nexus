import type { ReactNode } from 'react';

/** Neutrales Zeichen für unbekannte Namen: ein kleines Quadrat wie ein Blip */
export const NEUTRAL = <rect x="7" y="7" width="10" height="10" />;

// Flächiges Zeichen auf einem Raster 24 × 24 in der Textfarbe; die schwarze Kontur der HUD-Symbole setzt theme.css.
// Für Screenreader ausgeblendet.
export function SaGlyph({ name, children }: { name: string; children: ReactNode }) {
  return (
    <svg className="gta-sa-icon" data-icon={name} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" focusable="false">
      {children}
    </svg>
  );
}
