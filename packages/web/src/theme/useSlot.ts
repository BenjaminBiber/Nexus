import { getTheme } from './registry.js';
import type { ThemeSlots } from './slots.js';
import { useDisplayedTheme } from './useDisplayedTheme.js';

// Komponente des angezeigten Themes für einen Slot; undefined heißt: Standard-Komponente nehmen
export function useSlot<K extends keyof ThemeSlots>(slot: K): ThemeSlots[K] | undefined {
  const theme = useDisplayedTheme();
  return getTheme(theme)?.slots[slot];
}
