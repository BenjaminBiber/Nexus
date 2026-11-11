// @vitest-environment jsdom
// Rahmen im Stil San Andreas: Szene, Menüspalte mit Radar, Kopf mit Titel und HUD, Inhalt, Untertitel und Zone
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen, within } from '@testing-library/react';
import { SaShell } from '../src/sa/SaShell.js';
import { fakeGame, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const part = (name: string) => document.querySelector(`[data-gta-part="${name}"]`) as HTMLElement | null;
const content = <main className="shell-main">seiten-inhalt</main>;

describe('SaShell', () => {
  it('trägt die Prüfmarke shell und zeigt Wortmarke, HUD, Menü und Inhalt', () => {
    renderWithGame(<SaShell content={content} />);
    const shell = part('shell')!;
    expect(shell.classList.contains('gta-sa')).toBe(true);
    expect(within(shell).getByRole('img', { name: 'Nexus' }).getAttribute('data-wordmark')).toBe('nexus');
    expect(shell.querySelector('header')?.contains(part('hud'))).toBe(true);
    expect(screen.getAllByRole('navigation').map((nav) => nav.getAttribute('aria-label'))).toEqual(['Hauptnavigation']);
    expect(screen.getAllByRole('main')).toHaveLength(1);
    expect(shell.querySelector('.gta-sa-main')?.textContent).toBe('seiten-inhalt');
  });

  it('legt die Szene als Schmuck hinter alles', () => {
    renderWithGame(<SaShell content={content} />);
    const scene = part('shell')!.querySelector('.gta-sa-scene') as HTMLElement;
    expect(scene.getAttribute('aria-hidden')).toBe('true');
    expect(scene.querySelector('svg')).not.toBeNull();
  });

  it('das Radar sitzt am Fuß der Menüspalte und folgt dem Schalter Radar', () => {
    const view = renderWithGame(<SaShell content={content} />);
    const side = part('shell')!.querySelector('.gta-sa-side') as HTMLElement;
    expect(side.contains(part('radar'))).toBe(true);
    expect(side.contains(screen.getByRole('navigation'))).toBe(true);
    view.unmount();
    const game = fakeGame();
    renderWithGame(<SaShell content={content} />, { ...game, settings: { ...game.settings, radar: false } });
    expect(part('radar')).toBeNull();
  });

  it('zeigt den Namen der Seite als Zone, eingeblendet nur nach einem Wechsel', () => {
    const view = renderWithGame(<SaShell content={content} />);
    expect(part('zone')!.textContent).toBe('Board');
    expect(part('zone')!.hasAttribute('data-flash')).toBe(false);
    view.unmount();
    renderWithGame(<SaShell content={content} />, fakeGame({ zone: 'Board' }));
    expect(part('zone')!.hasAttribute('data-flash')).toBe(true);
  });

  it('der Stats-Eintrag öffnet das Stats-Menü', () => {
    const game = fakeGame();
    renderWithGame(<SaShell content={content} />, game);
    fireEvent.click(part('stats-button')!);
    expect(game.setStatsOpen).toHaveBeenCalledWith(true);
  });

  it('Handy, Banner und Stats-Menü erscheinen nur, wenn das Spiel sie meldet', () => {
    const view = renderWithGame(<SaShell content={content} />);
    for (const name of ['phone', 'banner', 'stats', 'cheat']) expect(part(name), name).toBeNull();
    view.unmount();
    renderWithGame(
      <SaShell content={content} />,
      fakeGame({ phone: { count: 1 }, banner: { kind: 'passed', ticketId: 'NOVA-0003', title: 'Kasse', reward: 40_000 }, statsOpen: true, cheat: 'money' }),
    );
    for (const name of ['phone', 'banner', 'stats', 'cheat']) expect(part(name), name).not.toBeNull();
  });

  it('das Handy liegt im Fuß des Rahmens, nicht über dem Inhalt', () => {
    renderWithGame(<SaShell content={content} />, fakeGame({ phone: { count: 1 } }));
    const bottom = part('shell')!.querySelector('.gta-sa-bottom') as HTMLElement;
    expect(bottom.contains(part('phone'))).toBe(true);
    expect(part('shell')!.querySelector('.gta-sa-main')?.contains(part('phone'))).toBe(false);
  });

  it('Busted und Wasted dunkeln den Rahmen ab, Mission Passed nicht; <html> bleibt unberührt', () => {
    const events = [
      [{ kind: 'busted', ticketId: 'NOVA-0004', title: 'Zahlungsanbieter', reason: null }, true],
      [{ kind: 'wasted' }, true],
      [{ kind: 'passed', ticketId: 'NOVA-0003', title: 'Kasse', reward: 40_000 }, false],
    ] as const;
    for (const [banner, dead] of events) {
      const view = renderWithGame(<SaShell content={content} />, fakeGame({ banner }));
      expect(part('shell')!.hasAttribute('data-dead'), banner.kind).toBe(dead);
      view.unmount();
    }
    expect(document.documentElement.getAttribute('style')).toBeNull();
  });

  it('bei offenem Stats-Menü trägt der Rahmen data-stats', () => {
    renderWithGame(<SaShell content={content} />, fakeGame({ statsOpen: true }));
    expect(part('shell')!.hasAttribute('data-stats')).toBe(true);
  });
});
