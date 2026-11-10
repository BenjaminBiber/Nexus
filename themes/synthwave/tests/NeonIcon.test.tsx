// @vitest-environment jsdom
// Neon-Symbole für die Slots NavIcon und Icon: Linien-SVG in currentColor, für Screenreader ausgeblendet
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import type { IconName, NavIconName } from '@nexus/theme-sdk';
import { NeonIcon } from '../src/NeonIcon.js';

afterEach(cleanup);

const NAV: NavIconName[] = ['board', 'dashboard', 'inbox', 'sessions', 'questions', 'docs', 'search', 'guide', 'settings'];
const ICONS: IconName[] = ['doc', 'plan', 'glossary', 'adr', 'file', 'folder', 'folderOpen', 'empty', 'edit', 'comment'];

describe('NeonIcon', () => {
  it.each([...NAV, ...ICONS])('liefert für „%s“ ein SVG mit mindestens einem Pfad, aria-hidden', (name) => {
    const { container } = render(<NeonIcon name={name} />);
    const svg = container.querySelector('svg');
    expect(svg).not.toBeNull();
    expect(svg!.getAttribute('aria-hidden')).toBe('true');
    expect(svg!.querySelectorAll('path').length).toBeGreaterThan(0);
    expect(svg!.getAttribute('stroke')).toBe('currentColor');
  });

  it('der Leerzustand zeigt „NO SIGNAL“', () => {
    const { container } = render(<NeonIcon name="empty" />);
    expect(container.querySelector('svg.synth-empty')?.textContent).toContain('NO SIGNAL');
  });

  it('unbekannte Namen ergeben nichts', () => {
    const { container } = render(<NeonIcon name={'gibtsnicht' as IconName} />);
    expect(container.innerHTML).toBe('');
  });
});
