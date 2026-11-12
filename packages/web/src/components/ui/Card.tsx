import type { CardProps } from '@nexus/theme-sdk';
import { useSlotted } from '../../theme/useSlotted.js';
import { SlotHost, type HostAttributes } from './SlotHost.js';

// Karte der Oberfläche: das Element (Klasse, Rolle, Ereignisse) bleibt im Kern, das Innere darf der Slot „Card“ des Themes gestalten
export function Card({ as, title, actions, className, children, ...attributes }: CardProps & HostAttributes) {
  const standard = (
    <>
      {title}
      {children}
      {actions}
    </>
  );
  const { node, replaced } = useSlotted('Card', { title, actions, className, children }, standard);
  return (
    <SlotHost as={as} className={className} slot="Card" replaced={replaced} attributes={attributes}>
      {node}
    </SlotHost>
  );
}
