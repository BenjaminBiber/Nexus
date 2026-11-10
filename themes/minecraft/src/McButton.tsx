// Menü-Knopf mit der Button-Textur (9-Slice in theme.css), Beschriftung in der Minecraft-Schrift und Klick-Ton
import type { ComponentProps } from 'react';
import { McText } from './McText.js';
import { useSound } from './useSound.js';

export function McButton({ children, className, onClick, ...rest }: Omit<ComponentProps<'button'>, 'children'> & { children: string }) {
  const play = useSound();
  return (
    <button
      type="button"
      {...rest}
      className={`mc-button${className ? ` ${className}` : ''}`}
      onClick={(event) => {
        play('click');
        onClick?.(event);
      }}
    >
      <McText>{children}</McText>
    </button>
  );
}
