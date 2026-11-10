// Slot „Input“: ersetzt alle Eingabefelder des Hosts durch das Textfeld des Spiels
// (Rahmen aus widget/text_field als 9-Slice in theme.css, bei Fokus die hervorgehobene Textur).
import type { InputProps } from '@nexus/theme-sdk';

export function McInput({
  value,
  onChange,
  id,
  name,
  type,
  placeholder,
  disabled,
  required,
  autoFocus,
  multiline,
  rows,
  className,
  ...aria
}: InputProps) {
  const classes = ['mc-input', multiline && 'mc-input--multiline', className].filter(Boolean).join(' ');
  // Gemeinsame Eingaben beider Felder; `onChange` meldet dem Host nur den Text
  const common = {
    value,
    id,
    name,
    placeholder,
    disabled,
    required,
    autoFocus,
    className: classes,
    ...aria,
  };
  return multiline ? (
    <textarea {...common} rows={rows} onChange={(event) => onChange(event.target.value)} />
  ) : (
    <input {...common} type={type ?? 'text'} onChange={(event) => onChange(event.target.value)} />
  );
}
