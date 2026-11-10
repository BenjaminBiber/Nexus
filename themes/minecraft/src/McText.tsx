// Text in der Spielschrift mit Schatten wie im Spiel. Die Schrift selbst ist die Web-Schrift `Minecraft`
// (src/font/useMinecraftFont.ts); fehlt sie, greift die Rückfallschrift aus theme.css und der Schatten entfällt.
import type { ReactNode } from 'react';

export function McText({ children, className }: { children: ReactNode; className?: string }) {
  return <span className={['mc-text', 'mc-shadow', className].filter(Boolean).join(' ')}>{children}</span>;
}
