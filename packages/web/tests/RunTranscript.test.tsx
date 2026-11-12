// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
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
const group = (open: boolean): RunItem => ({ type: 'group', key: 'group:a', entries: [tool('a'), tool('b'), tool('c')], tools: 3, label: '3 Schritte · 3× Read', open });
const SHOTS = [shot('001.png', 'x'), shot('002.png', 's'), shot('003.png', 's')];

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

  it('zeigt die Screenshots eines Aufrufs direkt darunter', async () => {
    const { container, onShot } = show([entry(tool('s', 'mcp__browser__screenshot'), [SHOTS[1]!, SHOTS[2]!]), entry(tool('d'))]);
    const rows = container.querySelectorAll('li.run-shot-row');
    expect(rows).toHaveLength(1);
    expect(rows[0]!.previousElementSibling?.classList.contains('tx-entry')).toBe(true);
    const image = screen.getByAltText('Screenshot 2 von 3') as HTMLImageElement;
    expect(image.getAttribute('src')).toBe('/shots/run/002.png');
    expect(screen.getByAltText('Screenshot 3 von 3')).toBeTruthy();

    await userEvent.click(screen.getByRole('button', { name: 'Screenshot 3 von 3 vergrößern' }));
    expect(onShot).toHaveBeenCalledWith('003.png');
  });

  it('sagt, wenn es keine Ausgabe gibt', () => {
    const { rerender, onShot } = show([], { running: true });
    expect(screen.getByText('Noch keine Ausgabe …')).toBeTruthy();
    rerender(<RunTranscript items={[]} shots={[]} shotUrl={(file) => file} onShot={onShot} running={false} />);
    expect(screen.getByText('(keine Ausgabe)')).toBeTruthy();
  });
});
