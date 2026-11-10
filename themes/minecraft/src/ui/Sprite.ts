// 9-Slice-Ränder der GUI-Sprites aus den .mcmeta-Dateien des Spiels (gui.scaling.nine_slice), Maße in Spielpixeln;
// sliceStyle liefert border-image mit Rand × 2 (GUI-Maßstab 2). Die Textur selbst kommt aus der CSS-Variablen.
import type { CSSProperties } from 'react';

export type SpriteName =
  | 'button'
  | 'buttonHighlighted'
  | 'buttonDisabled'
  | 'textField'
  | 'textFieldHighlighted'
  | 'popup'
  | 'tooltipBackground'
  | 'tooltipFrame';

export const SPRITES: Record<SpriteName, { width: number; height: number; border: number; variable: string }> = {
  button: { width: 200, height: 20, border: 3, variable: '--mc-button' },
  buttonHighlighted: { width: 200, height: 20, border: 3, variable: '--mc-button-highlighted' },
  buttonDisabled: { width: 200, height: 20, border: 3, variable: '--mc-button-disabled' },
  textField: { width: 200, height: 20, border: 1, variable: '--mc-text-field' },
  textFieldHighlighted: { width: 200, height: 20, border: 1, variable: '--mc-text-field-highlighted' },
  popup: { width: 236, height: 34, border: 6, variable: '--mc-popup' },
  tooltipBackground: { width: 100, height: 100, border: 9, variable: '--mc-tooltip-bg' },
  tooltipFrame: { width: 100, height: 100, border: 10, variable: '--mc-tooltip-frame' },
};

/** border-image-Regeln eines Sprites; fehlt die Textur, greift `none` und damit der Rückfall aus theme.css */
export function sliceStyle(name: SpriteName): CSSProperties {
  const s = SPRITES[name];
  return {
    borderStyle: 'solid',
    borderWidth: `${s.border * 2}px`,
    borderColor: 'transparent',
    borderImageSource: `var(${s.variable}, none)`,
    borderImageSlice: `${s.border} fill`,
    borderImageWidth: `${s.border * 2}px`,
    borderImageRepeat: 'stretch',
  };
}
