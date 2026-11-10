import type { ReactNode } from 'react';
import type { IconName } from '@nexus/theme-sdk';
import { SlotBoundary } from './SlotBoundary.js';
import { useDisplayedTheme } from './useDisplayedTheme.js';
import { useSlot } from './useSlot.js';

// Symbol im Kern, das der Slot „Icon“ des Themes ersetzen darf; ohne Slot (oder wenn er scheitert) gilt `children`
export function ThemeIcon({ name, children = null }: { name: IconName; children?: ReactNode }) {
  const theme = useDisplayedTheme();
  const Custom = useSlot('Icon');
  if (!Custom) return <>{children}</>;
  return (
    <SlotBoundary fallback={children} resetKey={theme}>
      <Custom name={name} />
    </SlotBoundary>
  );
}
