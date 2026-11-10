import { createElement, type ComponentType, type ReactNode } from 'react';
import type { ThemeSlots } from '@nexus/theme-sdk';
import { SlotBoundary } from './SlotBoundary.js';
import { useDisplayedTheme } from './useDisplayedTheme.js';
import { useSlot } from './useSlot.js';

/** Eingaben der Komponente eines Slots */
export type SlotProps<K extends keyof ThemeSlots> = NonNullable<ThemeSlots[K]> extends ComponentType<infer P> ? P : never;

// Komponente des Themes für einen Slot in einer SlotBoundary (Rückfall auf den Standard); ohne Ersatz der Standard selbst.
// replaced sagt, ob ein Theme den Slot ersetzt (z. B. für data-slot am Element des Kerns).
export function useSlotted<K extends keyof ThemeSlots>(slot: K, props: SlotProps<K>, standard: ReactNode): { node: ReactNode; replaced: boolean } {
  const theme = useDisplayedTheme();
  const Custom = useSlot(slot) as ComponentType<SlotProps<K>> | undefined;
  if (!Custom) return { node: standard, replaced: false };
  return {
    node: (
      <SlotBoundary fallback={standard} resetKey={theme}>
        {createElement(Custom as ComponentType<object>, props as object)}
      </SlotBoundary>
    ),
    replaced: true,
  };
}
