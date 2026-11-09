// Menü-Knopf mit der Button-Textur (9-Slice in theme.css) und Beschriftung in der Minecraft-Schrift
import type { ComponentProps } from 'react';
import { McText } from './BitmapText.js';

export function McButton({ children, className, ...rest }: Omit<ComponentProps<'button'>, 'children'> & { children: string }) {
  return (
    <button type="button" {...rest} className={`mc-button${className ? ` ${className}` : ''}`}>
      <McText>{children}</McText>
    </button>
  );
}
