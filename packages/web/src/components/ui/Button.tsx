import type { ButtonProps } from '@nexus/theme-sdk';
import { useSlotted } from '../../theme/useSlotted.js';

// Klasse je Variante; „secondary“ ist der schlichte Knopf
const VARIANT_CLASS: Record<NonNullable<ButtonProps['variant']>, string> = {
  primary: ' primary',
  secondary: '',
  danger: ' danger',
  ghost: ' ghost',
};

// Knopf der Oberfläche (Klasse „btn“ mit Variante); der Slot „Button“ des Themes darf ihn ersetzen
export function Button(props: ButtonProps) {
  const { variant = 'secondary', type = 'button', className, children, ...rest } = props;
  const standard = (
    <button type={type} className={`btn${VARIANT_CLASS[variant]}${className ? ` ${className}` : ''}`} {...rest}>
      {children}
    </button>
  );
  return useSlotted('Button', { ...props, variant, type }, standard).node;
}
