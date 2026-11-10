// Slot „Button“: ersetzt alle Knöpfe des Hosts durch einen Knopf im Spielstil (Textur als 9-Slice in theme.css)
// samt Klick-Ton. Die Beschriftung steht wie im Spiel weiß mit Schatten.
import { forwardRef } from 'react';
import type { ButtonProps } from '@nexus/theme-sdk';
import { useSound } from '../useSound.js';

/** Zusatz über ButtonProps hinaus: die Shell braucht `aria-haspopup` für den Menü-Knopf */
export type McButtonSlotProps = ButtonProps & { 'aria-haspopup'?: 'menu' | 'dialog' | 'listbox' | true };

// forwardRef, weil die Shell den Fokus an den Menü-Knopf zurückgibt
export const McButtonSlot = forwardRef<HTMLButtonElement, McButtonSlotProps>(function McButtonSlot(
  { variant, type, disabled, onClick, title, autoFocus, className, children, ...aria },
  ref,
) {
  const play = useSound();
  const classes = ['mc-button', variant === 'danger' && 'mc-button--danger', variant === 'ghost' && 'mc-button--ghost', className]
    .filter(Boolean)
    .join(' ');
  return (
    <button
      ref={ref}
      type={type ?? 'button'}
      disabled={disabled}
      title={title}
      autoFocus={autoFocus}
      className={classes}
      {...aria}
      onClick={(event) => {
        play('click');
        onClick?.(event);
      }}
    >
      {/* Kinder sind beliebige Knoten, darum kein McText (der verlangt Text) */}
      <span className="mc-text mc-shadow">{children}</span>
    </button>
  );
});
