// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ResearchDto } from '../src/api/questionTypes.js';
import { ResearchBlock } from '../src/components/ResearchBlock.js';

afterEach(cleanup);

const older: ResearchDto = {
  at: '2026-10-08T16:20:00.000Z', author: 'explore', kind: 'subagent', context: 'Erste Sichtung: nur eine Stelle.',
  options: [{ id: 'O1', title: 'Alte Idee', text: 'Nur lesbar', pros: ['klein'], cons: ['alt'], recommended: false }],
};

const newest: ResearchDto = {
  at: '2026-10-09T14:05:00.000Z', author: 'Claude', kind: 'session',
  context: 'Der Export schreibt CSV in `src/export/Exporter.ts:42`.',
  options: [
    { id: 'O2', title: 'Eigene CSV', text: 'Weiter selbst schreiben', pros: ['keine Abhängigkeit'], cons: ['kein Excel'], recommended: false },
    { id: 'O3', title: 'ExcelJS', text: 'Mit Formaten', pros: ['Formate', 'gepflegt'], cons: ['groß'], recommended: true },
  ],
};

describe('ResearchBlock', () => {
  it('zeigt die neueste Recherche mit Kontext und vergleichbaren Optionen', () => {
    render(<ResearchBlock questionId="ARC-F0007" research={[older, newest]} />);
    const current = screen.getByRole('region', { name: 'Aktuelle Recherche' });
    expect(within(current).getByText('neueste')).toBeTruthy();
    expect(within(current).getByText(/Exporter\.ts:42/)).toBeTruthy();
    const cards = within(current).getAllByRole('article');
    expect(cards).toHaveLength(2);
    expect(within(cards[1]).getByText('Option O3')).toBeTruthy();
    expect(within(cards[1]).getByText('empfohlen')).toBeTruthy();
    expect(within(cards[1]).getByText('Formate')).toBeTruthy();
    expect(within(cards[1]).getByText('groß')).toBeTruthy();
    expect(within(cards[0]).queryByText('empfohlen')).toBeNull();
  });

  it('klappt ältere Recherchen ein und macht ihre Optionen nur lesbar', async () => {
    render(<ResearchBlock questionId="ARC-F0007" research={[older, newest]} selectable picked={null} onPick={() => undefined} />);
    const earlier = screen.getByText(/ältere Recherche/).closest('details')!;
    expect(earlier.open).toBe(false);
    await userEvent.click(within(earlier).getByText(/ältere Recherche/));
    expect(earlier.open).toBe(true);
    expect(within(earlier).getByText('Option O1')).toBeTruthy();
    expect(within(earlier).queryByRole('button', { name: /wählen/ })).toBeNull();
  });

  it('lässt nur Optionen der neuesten Recherche wählen und meldet die Wahl', async () => {
    const onPick = vi.fn();
    render(<ResearchBlock questionId="ARC-F0007" research={[older, newest]} selectable picked="O3" onPick={onPick} />);
    const current = screen.getByRole('region', { name: 'Aktuelle Recherche' });
    expect(within(current).getByRole('button', { name: 'Gewählt' }).getAttribute('aria-pressed')).toBe('true');
    await userEvent.click(within(current).getByRole('button', { name: 'Diese Option wählen' }));
    expect(onPick).toHaveBeenCalledWith('O2');
    await userEvent.click(within(current).getByRole('button', { name: 'Gewählt' }));
    expect(onPick).toHaveBeenLastCalledWith(null);
  });

  it('bietet ohne Wahlmöglichkeit keine Knöpfe an', () => {
    render(<ResearchBlock questionId="ARC-F0007" research={[newest]} />);
    expect(screen.queryByRole('button', { name: /wählen|Gewählt/ })).toBeNull();
  });

  it('zeigt ohne Recherche den Befehl mit Plugin-Namen; ohne start gibt es keinen Knopf zum Starten', () => {
    render(<ResearchBlock questionId="ARC-F0008" research={[]} />);
    const card = screen.getByRole('region', { name: 'Recherche anfordern' });
    expect(within(card).getByText('/ai-harness:board-question ARC-F0008')).toBeTruthy();
    expect(within(card).getByText(/Befehl in einer Session ausführen/)).toBeTruthy();
    expect(within(card).getByRole('button', { name: /Befehl kopieren/ })).toBeTruthy();
    expect(within(card).queryByRole('button', { name: 'Recherche starten' })).toBeNull();
  });

  it('„Recherche starten“: startet, sperrt bei laufender Recherche oder nicht offener Frage mit Hinweis, zeigt Fehler', async () => {
    const onStart = vi.fn();
    const onShowLog = vi.fn();
    const base = { open: true, running: null, busy: false, error: null, onStart, onShowLog };
    const { rerender } = render(<ResearchBlock questionId="ARC-F0008" research={[]} start={base} />);
    const card = screen.getByRole('region', { name: 'Recherche anfordern' });
    await userEvent.click(within(card).getByRole('button', { name: 'Recherche starten' }));
    expect(onStart).toHaveBeenCalledTimes(1);
    const run = { id: 'r', kind: 'research' as const, ticketId: 'ARC-F0008', questionId: 'ARC-F0008', title: 't', profile: 'standard', model: null, effort: null, startedAt: '2026-10-15T07:00:00.000Z', log: 'ARC-F0008-20261015-090000-research.log' };
    rerender(<ResearchBlock questionId="ARC-F0008" research={[]} start={{ ...base, running: run }} />);
    expect((within(card).getByRole('button', { name: 'Recherche starten' }) as HTMLButtonElement).disabled).toBe(true);
    expect(within(card).getByText('Für diese Frage läuft schon eine Recherche.')).toBeTruthy();
    expect(within(card).getByRole('status').textContent).toContain('Recherche läuft – Ergebnis erscheint hier automatisch');
    await userEvent.click(within(card).getByRole('button', { name: run.log }));
    expect(onShowLog).toHaveBeenCalledWith(run.log);
    rerender(<ResearchBlock questionId="ARC-F0008" research={[]} start={{ ...base, open: false, error: 'Der Dispatcher pausiert bis 14:30' }} />);
    expect((within(card).getByRole('button', { name: 'Recherche starten' }) as HTMLButtonElement).disabled).toBe(true);
    expect(within(card).getByText('Nur offene Fragen lassen sich recherchieren.')).toBeTruthy();
    expect(within(card).getByRole('alert').textContent).toBe('Der Dispatcher pausiert bis 14:30');
  });

  it('erklärt bei vorhandener Recherche, dass der Befehl eine neue anfordert', () => {
    render(<ResearchBlock questionId="ARC-F0007" research={[newest]} />);
    const card = screen.getByRole('region', { name: 'Recherche anfordern' });
    expect(within(card).getByText(/Eine neue Recherche kommt dazu/)).toBeTruthy();
    expect(within(card).getByText('/ai-harness:board-question ARC-F0007')).toBeTruthy();
  });
});
