import { createElement, type HTMLAttributes, type ReactNode, type Ref } from 'react';

/** Element des Kerns um einen Hüllen-Slot (Card, Panel, Dialog) */
export type HostTag = 'div' | 'article' | 'section' | 'form' | 'aside';

/** Eigenschaften des Elements, die im Kern bleiben: Rolle, Ref, Ereignisse, Stil, data-/aria-Attribute */
export type HostAttributes = Omit<HTMLAttributes<HTMLElement>, 'title' | 'className' | 'children'> & {
  as?: HostTag;
  ref?: Ref<HTMLElement>;
};

// Rendert das Element des Kerns mit Klasse und Attributen; ersetzt ein Theme das Innere, trägt es data-slot mit dem Slot-Namen
export function SlotHost({ as = 'div', className, slot, replaced, attributes, children }: {
  as?: HostTag;
  className?: string;
  slot: string;
  replaced: boolean;
  attributes: Omit<HostAttributes, 'as'>;
  children: ReactNode;
}) {
  return createElement(as, { className, ...attributes, 'data-slot': replaced ? slot : undefined }, children);
}
