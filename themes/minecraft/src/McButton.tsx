// Menü-Knopf der Shell: dünner Aufsatz auf dem Slot-Knopf (src/slots/McButtonSlot.tsx), Beschriftung ist immer Text
import { forwardRef } from 'react';
import { McButtonSlot, type McButtonSlotProps } from './slots/McButtonSlot.js';

export const McButton = forwardRef<HTMLButtonElement, Omit<McButtonSlotProps, 'children'> & { children: string }>(function McButton(
  props,
  ref,
) {
  return <McButtonSlot ref={ref} {...props} />;
});
