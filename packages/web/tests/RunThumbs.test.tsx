// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { DispatchRunShotDto } from '../src/api/dispatchTypes.js';
import { RunThumbs } from '../src/components/db/run/RunThumbs.js';

afterEach(cleanup);

const shot = (file: string): DispatchRunShotDto => ({ file, toolUseId: `call-${file}`, at: '2026-10-16T08:16:00.000Z', mediaType: 'image/png', bytes: 1 });
const FIVE = ['001.png', '002.png', '003.png', '004.png', '005.png'].map(shot);
const url = (file: string) => `/shots/run/${file}`;

// Rendert die Vorschaubilder einer Stelle; ohne `all` zählen die eigenen Screenshots
function show(shots: DispatchRunShotDto[], all = shots) {
  const onOpen = vi.fn();
  const view = render(<RunThumbs shots={shots} all={all} url={url} onOpen={onOpen} label="Screenshots des Laufs" />);
  return { ...view, onOpen };
}

describe('RunThumbs', () => {
  it('rendert ohne Screenshots nichts', () => {
    const { container } = show([]);
    expect(container.innerHTML).toBe('');
  });

  it('zeigt bis zu drei Screenshots ohne „+N“', () => {
    show(FIVE.slice(0, 3));
    const group = screen.getByRole('group', { name: 'Screenshots des Laufs' });
    expect(group.classList.contains('run-thumbs')).toBe(true);
    expect(within(group).getAllByRole('img')).toHaveLength(3);
    expect(group.querySelector('.run-thumbs-more')).toBeNull();
  });

  it('zeigt bei fünf Screenshots „+2“ vor den letzten drei', () => {
    show(FIVE);
    const group = screen.getByRole('group', { name: 'Screenshots des Laufs' });
    const images = within(group).getAllByRole('img') as HTMLImageElement[];
    expect(images.map((image) => image.alt)).toEqual(['Screenshot 3 von 5', 'Screenshot 4 von 5', 'Screenshot 5 von 5']);
    expect(images[0]!.getAttribute('src')).toBe('/shots/run/003.png');
    const more = within(group).getByRole('button', { name: '2 weitere Screenshots ansehen' });
    expect(more.textContent).toBe('+2');
    expect(more.classList.contains('run-thumbs-more')).toBe(true);
    // „+N“ steht vor den Bildern
    expect(group.firstElementChild).toBe(more);
  });

  it('nennt einen einzelnen weiteren Screenshot in der Einzahl', () => {
    show(FIVE.slice(0, 4));
    expect(screen.getByRole('button', { name: '1 weiterer Screenshot ansehen' }).textContent).toBe('+1');
  });

  it('öffnet das angeklickte Bild und mit „+N“ den ersten Screenshot der Stelle', async () => {
    const { onOpen } = show(FIVE);
    await userEvent.click(screen.getByRole('button', { name: 'Screenshot 4 von 5 vergrößern' }));
    expect(onOpen).toHaveBeenLastCalledWith('004.png');
    await userEvent.click(screen.getByRole('button', { name: '2 weitere Screenshots ansehen' }));
    expect(onOpen).toHaveBeenLastCalledWith('001.png');
  });

  it('zählt über alle Screenshots des Laufs', async () => {
    const { onOpen } = show(FIVE.slice(1, 2), FIVE);
    await userEvent.click(screen.getByRole('button', { name: 'Screenshot 2 von 5 vergrößern' }));
    expect(onOpen).toHaveBeenCalledWith('002.png');
  });

  it('klappt einen Lauf nicht auf oder zu, wenn die Bilder in dessen <summary> stehen', async () => {
    const onOpen = vi.fn();
    // Rendert einen Lauf mit den Vorschaubildern im Kopf
    const run = (open: boolean) => (
      <details open={open}>
        <summary>
          Lauf
          <RunThumbs shots={FIVE} all={FIVE} url={url} onOpen={onOpen} label="Screenshots des Laufs" />
        </summary>
        Inhalt
      </details>
    );
    // jsdom klappt <details> bei Klicks auf Knöpfe im <summary> nicht selbst um: geprüft wird darum auch, dass der Klick
    // die Standardaktion (Umklappen im Browser) unterdrückt – nach React, also am Dokument
    const prevented: boolean[] = [];
    const listen = (event: Event) => prevented.push(event.defaultPrevented);
    document.addEventListener('click', listen);
    try {
      const { container, rerender } = render(run(true));
      const details = container.querySelector('details')!;
      await userEvent.click(screen.getByRole('button', { name: 'Screenshot 5 von 5 vergrößern' }));
      await userEvent.click(screen.getByRole('button', { name: '2 weitere Screenshots ansehen' }));
      expect(details.open).toBe(true);
      rerender(run(false));
      await userEvent.click(screen.getByRole('button', { name: 'Screenshot 5 von 5 vergrößern' }));
      expect(details.open).toBe(false);
      expect(onOpen).toHaveBeenCalledTimes(3);
      expect(prevented).toEqual([true, true, true]);
    } finally {
      document.removeEventListener('click', listen);
    }
  });
});
