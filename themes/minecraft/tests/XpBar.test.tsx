// @vitest-environment jsdom
// XP-Leiste als Baustein: Füllung nach Prozent, Level-Zahl darüber, Grenzstrich und Farbzustände
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { XpBar } from '../src/usage/XpBar.js';

afterEach(cleanup);

describe('XpBar', () => {
  it('füllt nach Prozent und zeigt die Level-Zahl', () => {
    const { container } = render(<XpBar percent={42} limit={90} width={182} label="Sitzung" />);
    expect(screen.getByRole('img', { name: 'Sitzung: 42 %' })).toBeTruthy();
    expect((container.querySelector('.mc-xp-fill') as HTMLElement).style.clipPath).toBe('inset(0 58% 0 0)');
    expect(container.querySelector('.mc-xp-level')?.textContent).toBe('42');
    expect((container.querySelector('.mc-xp-limit') as HTMLElement).style.left).toBe('90%');
  });

  it('zeigt – ohne Wert und kennt die Farbzustände', () => {
    const { container, rerender } = render(<XpBar percent={null} limit={null} width={182} label="Woche" />);
    expect(container.querySelector('.mc-xp-level')?.textContent).toBe('–');
    expect(container.querySelector('.mc-xp-limit')).toBeNull();
    rerender(<XpBar percent={92} limit={90} width={182} label="Woche" tone="over" />);
    expect(container.querySelector('.mc-xp')?.classList.contains('mc-xp--over')).toBe(true);
  });

  it('rechnet die Breite in Spielpixel um und kennt die volle Breite', () => {
    const { container } = render(<XpBar percent={10} limit={null} width={182} label="Sitzung" />);
    expect((container.querySelector('.mc-xp') as HTMLElement).style.width).toBe('364px');
    cleanup();
    const full = render(<XpBar percent={10} limit={null} width="full" label="Sitzung" />);
    expect((full.container.querySelector('.mc-xp') as HTMLElement).style.width).toBe('');
  });

  it('kann dekorativ sein und eine eigene Level-Anzeige tragen', () => {
    const { container } = render(<XpBar percent={75} limit={null} width="full" label="Schritte" decorative level={3} />);
    expect(screen.queryByRole('img')).toBeNull();
    expect(container.querySelector('.mc-xp')?.getAttribute('aria-hidden')).toBe('true');
    expect(container.querySelector('.mc-xp-level')?.textContent).toBe('3');
  });

  it('begrenzt Werte außerhalb von 0 bis 100 auf die Leiste', () => {
    const { container, rerender } = render(<XpBar percent={140} limit={null} width="full" label="Woche" />);
    expect((container.querySelector('.mc-xp-fill') as HTMLElement).style.clipPath).toBe('inset(0 0% 0 0)');
    rerender(<XpBar percent={-5} limit={null} width="full" label="Woche" />);
    expect((container.querySelector('.mc-xp-fill') as HTMLElement).style.clipPath).toBe('inset(0 100% 0 0)');
  });
});
