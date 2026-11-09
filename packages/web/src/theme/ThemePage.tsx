import type { ReactNode } from 'react';
import { useParams } from 'react-router-dom';
import type { PageName } from '@nexus/theme-sdk';
import { SlotBoundary } from './SlotBoundary.js';
import { useDisplayedTheme } from './useDisplayedTheme.js';
import { useSlot } from './useSlot.js';

// Seite des Themes für page:<name>, sonst die Standard-Seite; Fehler der Theme-Seite fallen auf die Standard-Seite zurück
export function ThemePage({ name, standard }: { name: PageName; standard: ReactNode }) {
  const theme = useDisplayedTheme();
  const { pid = '' } = useParams();
  const Custom = useSlot(`page:${name}` as const);
  if (!Custom) return <>{standard}</>;
  return (
    <SlotBoundary fallback={standard} resetKey={theme}>
      <Custom pid={pid} />
    </SlotBoundary>
  );
}
