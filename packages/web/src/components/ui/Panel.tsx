import type { PanelProps } from '@nexus/theme-sdk';
import { useSlotted } from '../../theme/useSlotted.js';
import { SlotHost, type HostAttributes } from './SlotHost.js';

// Bereich der Oberfläche (z. B. Einstellungen): das Element bleibt im Kern, das Innere darf der Slot „Panel“ des Themes gestalten
export function Panel({ as, title, className, children, ...attributes }: PanelProps & HostAttributes) {
  const standard = (
    <>
      {title}
      {children}
    </>
  );
  const { node, replaced } = useSlotted('Panel', { title, className, children }, standard);
  return (
    <SlotHost as={as} className={className} slot="Panel" replaced={replaced} attributes={attributes}>
      {node}
    </SlotHost>
  );
}
