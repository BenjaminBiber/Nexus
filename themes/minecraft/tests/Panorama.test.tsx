// @vitest-environment jsdom
// Panorama: die Flächen blenden erst ein, wenn alle sechs Bilder da sind; hinter Untermenüs unscharf,
// auf dem Titelbildschirm scharf mit dem dunklen Overlay-Verlauf
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ResourcesProvider } from '../src/context.js';
import { Panorama } from '../src/Panorama.js';
import { PANORAMA_OVERLAY, panorama } from '../src/resources.js';

// Alle Einträge aus dem Asset-Index, die das Panorama braucht
const ALL = Object.fromEntries([...[0, 1, 2, 3, 4, 5].map((i) => [panorama(i), `/p${i}`] as const), [PANORAMA_OVERLAY, '/ov'] as const]);

afterEach(cleanup);

describe('Panorama', () => {
  it('ist erst bereit, wenn alle sechs Bilder da sind', () => {
    const { container, rerender } = render(
      <ResourcesProvider urls={{ [panorama(0)]: '/p0' }}>
        <Panorama blur={false} />
      </ResourcesProvider>,
    );
    const view = () => container.querySelector('.mc-panorama')!;
    expect(view().classList.contains('mc-panorama--ready')).toBe(false);
    expect(view().querySelectorAll('.mc-panorama-face')).toHaveLength(6);

    rerender(
      <ResourcesProvider urls={ALL}>
        <Panorama blur={false} />
      </ResourcesProvider>,
    );
    expect(view().classList.contains('mc-panorama--ready')).toBe(true);
    expect(view().classList.contains('mc-panorama--blur')).toBe(false);
  });

  it('kennt Unschärfe und Overlay', () => {
    const { container } = render(
      <ResourcesProvider urls={ALL}>
        <Panorama blur />
      </ResourcesProvider>,
    );
    const view = container.querySelector('.mc-panorama')!;
    expect(view.classList.contains('mc-panorama--blur')).toBe(true);
    const overlay = container.querySelector('.mc-panorama-overlay') as HTMLElement;
    expect(overlay.style.getPropertyValue('--mc-panorama-overlay')).toContain('/ov');
  });

  it('ohne Overlay-Bild bleibt die Variable leer, die Flächen fehlen nicht', () => {
    const { container } = render(<Panorama blur={false} />);
    const overlay = container.querySelector('.mc-panorama-overlay') as HTMLElement;
    expect(overlay.style.getPropertyValue('--mc-panorama-overlay')).toBe('');
    expect(container.querySelectorAll('.mc-panorama-face')).toHaveLength(6);
  });
});
