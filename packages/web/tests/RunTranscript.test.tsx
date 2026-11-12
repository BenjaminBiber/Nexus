// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { DispatchRunShotDto } from '../src/api/dispatchTypes.js';
import type { TranscriptEntry } from '../src/api/sessionDetailTypes.js';
import type { RunItem } from '../src/dbboard/runItems.js';
import { RunTranscript } from '../src/components/db/run/RunTranscript.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(cleanup);

const AT = '2026-10-16T08:16:00.000Z';
const ok = { text: 'ok', isError: false, truncated: false };
const tool = (id: string, name = 'Read'): TranscriptEntry => ({ kind: 'tool', id, at: AT, name, summary: `${id}.ts`, input: null, result: ok });
const text = (id: string, body: string): TranscriptEntry => ({ kind: 'text', id, at: AT, text: body });
const shot = (file: string, toolUseId: string): DispatchRunShotDto => ({ file, toolUseId, at: AT, mediaType: 'image/png', bytes: 1 });
const entry = (e: TranscriptEntry, shots: DispatchRunShotDto[] = []): RunItem => ({ type: 'entry', key: e.id, entry: e, shots });
const group = (open: boolean): RunItem => ({ type: 'group', key: 'group:a', entries: [tool('a'), tool('b'), tool('c')], tools: 3, label: '3 Schritte · 3× Read', open, shots: [] });
const SHOTS = [shot('001.png', 'x'), shot('002.png', 's'), shot('003.png', 's')];
// Gruppe, an deren Aufrufen „s“ und „x“ alle drei Screenshots hängen
const shotGroup = (open: boolean): RunItem => ({
  type: 'group',
  key: 'group:a',
  entries: [tool('a'), tool('s', 'mcp__browser__screenshot'), tool('x')],
  tools: 3,
  label: '3 Schritte · 2× Read · 1× browser · screenshot',
  open,
  shots: SHOTS,
});

// Rendert den Verlauf und liefert die Zahl der sichtbaren Einträge
function show(items: RunItem[], extra: Partial<Parameters<typeof RunTranscript>[0]> = {}) {
  const onShot = vi.fn();
  const view = render(<RunTranscript items={items} shots={SHOTS} shotUrl={(file) => `/shots/run/${file}`} onShot={onShot} running={false} {...extra} />);
  const visible = () => view.container.querySelectorAll('li.tx-entry').length;
  return { ...view, onShot, visible };
}

describe('RunTranscript', () => {
  it('zeigt Einträge wie die Sessions-Seite und eine Gruppe zugeklappt', async () => {
    const { container, visible } = show([entry(text('t1', 'Ich lese das Ticket.')), group(false), entry(tool('d', 'Edit'))]);
    expect(await screen.findByText('Ich lese das Ticket.')).toBeTruthy();
    expect(container.querySelector('ol.transcript.run-transcript')).toBeTruthy();
    expect(visible()).toBe(2);
    const head = screen.getByRole('button', { name: '3 Schritte · 3× Read' });
    expect(head.getAttribute('aria-expanded')).toBe('false');

    await userEvent.click(head);
    expect(head.getAttribute('aria-expanded')).toBe('true');
    expect(visible()).toBe(5);

    await userEvent.click(head);
    expect(visible()).toBe(2);
  });

  it('öffnet die vorgegebene Gruppe und behält die Wahl des Nutzers', async () => {
    const { rerender, visible, onShot } = show([group(true)], { running: true });
    const head = screen.getByRole('button', { name: '3 Schritte · 3× Read' });
    expect(head.getAttribute('aria-expanded')).toBe('true');
    expect(visible()).toBe(3);

    await userEvent.click(head);
    expect(visible()).toBe(0);
    // Beim Nachladen kommt dieselbe Gruppe wieder als „offen“ – die Wahl des Nutzers gilt weiter
    rerender(<RunTranscript items={[group(true)]} shots={SHOTS} shotUrl={(file) => `/shots/run/${file}`} onShot={onShot} running />);
    expect(visible()).toBe(0);
  });

  it('zeigt die Screenshots eines Aufrufs klein am Ende seiner Zeile', async () => {
    const { container, onShot } = show([entry(tool('s', 'mcp__browser__screenshot'), [SHOTS[1]!, SHOTS[2]!]), entry(tool('d'))]);
    expect(container.querySelector('.run-shot-row')).toBeNull();
    const rows = container.querySelectorAll('li.tx-entry.has-aside');
    expect(rows).toHaveLength(1);
    // Links die Karte des Aufrufs, rechts die Vorschaubilder
    expect(rows[0]!.firstElementChild!.classList.contains('tx-tool')).toBe(true);
    const aside = rows[0]!.lastElementChild!;
    expect(aside.classList.contains('tx-aside')).toBe(true);
    expect(within(aside as HTMLElement).getByRole('group', { name: 'Screenshots des Aufrufs' })).toBeTruthy();
    expect(within(aside as HTMLElement).getAllByRole('img')).toHaveLength(2);
    // Ein Aufruf ohne Screenshots behält das Markup der Sessions-Seite
    const plain = [...container.querySelectorAll('li.tx-entry')].find((li) => !li.classList.contains('has-aside'))!;
    expect(plain.className).toBe('tx-entry tool');
    expect(plain.querySelector('.tx-aside')).toBeNull();
    const image = screen.getByAltText('Screenshot 2 von 3') as HTMLImageElement;
    expect(image.getAttribute('src')).toBe('/shots/run/002.png');
    expect(screen.getByAltText('Screenshot 3 von 3')).toBeTruthy();

    await userEvent.click(screen.getByRole('button', { name: 'Screenshot 3 von 3 vergrößern' }));
    expect(onShot).toHaveBeenCalledWith('003.png');
  });

  it('zeigt die Screenshots einer zugeklappten Gruppe am Ende ihrer Zeile, aufgeklappt an den Aufrufen', async () => {
    const { container, onShot } = show([shotGroup(false)]);
    const head = screen.getByRole('button', { name: '3 Schritte · 2× Read · 1× browser · screenshot' });
    const row = head.parentElement!;
    expect(row.classList.contains('run-group-row')).toBe(true);
    const thumbs = within(row).getByRole('group', { name: 'Screenshots der Schritte' });
    // Neben dem Knopf der Gruppe, nicht darin
    expect(head.contains(thumbs)).toBe(false);
    expect(head.querySelector('button')).toBeNull();
    expect(row.lastElementChild).toBe(thumbs);
    expect(within(thumbs).getAllByRole('img').map((image) => (image as HTMLImageElement).alt)).toEqual(['Screenshot 1 von 3', 'Screenshot 2 von 3', 'Screenshot 3 von 3']);

    // Ein Klick auf ein Bild öffnet die Großansicht und klappt die Gruppe nicht auf
    await userEvent.click(within(thumbs).getByRole('button', { name: 'Screenshot 2 von 3 vergrößern' }));
    expect(onShot).toHaveBeenCalledWith('002.png');
    expect(head.getAttribute('aria-expanded')).toBe('false');
    expect(container.querySelectorAll('li.tx-entry')).toHaveLength(0);

    // Aufgeklappt: keine Bilder in der Gruppenzeile, jeder Aufruf trägt seine eigenen
    await userEvent.click(head);
    expect(head.getAttribute('aria-expanded')).toBe('true');
    expect(within(row).queryByRole('group')).toBeNull();
    const rows = [...container.querySelectorAll<HTMLElement>('li.tx-entry.has-aside')];
    expect(rows).toHaveLength(2);
    const imagesOf = (li: HTMLElement) => within(within(li).getByRole('group', { name: 'Screenshots des Aufrufs' })).getAllByRole('img').map((image) => (image as HTMLImageElement).alt);
    expect(rows.map(imagesOf)).toEqual([['Screenshot 2 von 3', 'Screenshot 3 von 3'], ['Screenshot 1 von 3']]);
    expect(container.querySelectorAll('li.tx-entry:not(.has-aside)')).toHaveLength(1);
  });

  it('lässt die Zeile einer Gruppe ohne Screenshots ohne Vorschaubilder', () => {
    show([group(false)]);
    const row = screen.getByRole('button', { name: '3 Schritte · 3× Read' }).parentElement!;
    expect(row.classList.contains('run-group-row')).toBe(true);
    expect(row.children).toHaveLength(1);
  });

  it('sagt, wenn es keine Ausgabe gibt', () => {
    const { rerender, onShot } = show([], { running: true });
    expect(screen.getByText('Noch keine Ausgabe …')).toBeTruthy();
    rerender(<RunTranscript items={[]} shots={[]} shotUrl={(file) => file} onShot={onShot} running={false} />);
    expect(screen.getByText('(keine Ausgabe)')).toBeTruthy();
  });
});
