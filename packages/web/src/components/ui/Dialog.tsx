import type { DialogProps } from '@nexus/theme-sdk';
import { useSlotted } from '../../theme/useSlotted.js';
import { SlotHost, type HostAttributes } from './SlotHost.js';

// Sichtbare Hülle im Dialog über den Slot „Dialog“ des Themes; ohne Ersatz die Kinder unverändert (auch in DbModal genutzt)
export function useDialogBody(props: DialogProps) {
  return useSlotted('Dialog', props, props.children);
}

// Eigenständiger Dialog ohne Hintergrund (z. B. Konflikt im Formular): Rolle und Name bleiben im Kern, die Hülle darf das Theme gestalten
export function Dialog({ as, role, labelledBy, className, children, ...attributes }: DialogProps & Omit<HostAttributes, 'role'>) {
  const { node, replaced } = useDialogBody({ role, labelledBy, className, children });
  return (
    <SlotHost as={as} slot="Dialog" replaced={replaced} attributes={{ role, 'aria-labelledby': labelledBy, ...attributes }} className={className}>
      {node}
    </SlotHost>
  );
}
