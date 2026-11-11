import type { ReactNode } from 'react';

/** Neutrales Zeichen für unbekannte Namen: eine kleine Raute */
export const NEUTRAL = <path d="M12 5l7 7-7 7-7-7z" />;

// Gefülltes Zeichen auf einem Raster 24 × 24 in der Textfarbe, wie die Symbole der Preview; für Screenreader ausgeblendet
export function VGlyph({ name, children }: { name: string; children: ReactNode }) {
  return (
    <svg className="gta-v-icon" data-icon={name} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" focusable="false">
      {children}
    </svg>
  );
}
