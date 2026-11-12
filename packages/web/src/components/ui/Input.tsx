import type { InputProps } from '@nexus/theme-sdk';
import { useSlotted } from '../../theme/useSlotted.js';

// Eingabefeld der Oberfläche (input, mit multiline textarea); onChange liefert den Text. Der Slot „Input“ des Themes darf es ersetzen
export function Input(props: InputProps) {
  const { onChange, multiline, rows, type, ...rest } = props;
  const standard = multiline ? (
    <textarea rows={rows} {...rest} onChange={(e) => onChange(e.target.value)} />
  ) : (
    <input type={type} {...rest} onChange={(e) => onChange(e.target.value)} />
  );
  return useSlotted('Input', props, standard).node;
}
