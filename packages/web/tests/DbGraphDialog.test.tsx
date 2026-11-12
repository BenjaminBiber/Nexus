// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DbGraphDto, DbGraphScope } from '../src/api/dbTypes.js';
import { DbGraphDialog } from '../src/components/db/DbGraphDialog.js';
import { LiveContext, type LiveHandler, type LiveValue } from '../src/live/liveContext.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const node = (id: string, column: DbGraphDto['nodes'][number]['column'], extra = {}) => ({
  id, title: `Titel ${id}`, status: column === 'waiting' ? 'planned' as const : column, column, boardId: 1, boardName: 'Sprint', spec: null, external: false,
  stepsDone: 0, stepsTotal: 0, ...extra,
});

// A ← B ← C, D extern; kritischer Pfad A, B, C
const GRAPH: DbGraphDto = {
  scope: { kind: 'board', boardId: 1 },
  nodes: [node('ARC-0001', 'in_progress'), node('ARC-0002', 'waiting'), node('ARC-0003', 'waiting'), node('ARC-0004', 'done', { external: true, boardName: 'Zweites' })],
  edges: [{ from: 'ARC-0001', to: 'ARC-0002' }, { from: 'ARC-0002', to: 'ARC-0003' }, { from: 'ARC-0004', to: 'ARC-0003' }],
  criticalPath: ['ARC-0001', 'ARC-0002', 'ARC-0003'],
  cycleEdges: [],
  hiddenWithoutEdges: 2,
  hiddenDone: 0,
};

const SCOPES = [
  { label: 'Ticket', scope: { kind: 'ticket', id: 'ARC-0002' } as DbGraphScope },
  { label: 'Spec 001-login', scope: { kind: 'spec', project: 'ARC', spec: '001-login' } as DbGraphScope },
  { label: 'Board', scope: { kind: 'board', boardId: 1 } as DbGraphScope },
];

function open(graph: DbGraphDto = GRAPH, initial = 2) {
  const getDbGraph = vi.fn(async () => graph);
  const onOpenTicket = vi.fn();
  const onClose = vi.fn();
  render(
    <ApiProvider api={fakeApi({ getDbGraph })}>
      <DbGraphDialog scopes={SCOPES} initial={initial} onClose={onClose} onOpenTicket={onOpenTicket} />
    </ApiProvider>,
  );
  return { getDbGraph, onOpenTicket, onClose };
}

const nodeEl = (id: string) => document.querySelector<SVGGElement>(`.dep-graph-node[data-id='${id}']`)!;
const steps = (id: string) => nodeEl(id).querySelector('.dep-graph-steps');
const zoomValue = () => screen.getByRole('button', { name: 'Zoom auf 100 % zurücksetzen' }).textContent;

// Dialog mit einer Live-Verbindung, deren Meldungen der Test selbst auslöst
function openLive(getDbGraph: ReturnType<typeof vi.fn>) {
  const handlers: LiveHandler[] = [];
  const live: LiveValue = {
    state: 'connected', lastSync: null, reconnect: () => undefined,
    register: (handler) => {
      handlers.push(handler);
      return () => void handlers.splice(handlers.indexOf(handler), 1);
    },
    watchDbBoard: () => undefined, watchDbProject: () => () => undefined,
  };
  render(
    <ApiProvider api={fakeApi({ getDbGraph })}>
      <LiveContext.Provider value={live}>
        <DbGraphDialog scopes={SCOPES} initial={2} onClose={vi.fn()} onOpenTicket={vi.fn()} />
      </LiveContext.Provider>
    </ApiProvider>,
  );
  return { emit: (event: (handler: LiveHandler) => void) => act(() => handlers.forEach(event)) };
}

describe('DbGraphDialog', () => {
  it('färbt Knoten nach der Board-Spalte und hebt den kritischen Pfad hervor', async () => {
    const { getDbGraph } = open();
    await waitFor(() => expect(nodeEl('ARC-0001')).toBeTruthy());
    expect(getDbGraph).toHaveBeenCalledWith({ kind: 'board', boardId: 1 }, false);
    expect(['ARC-0001', 'ARC-0002', 'ARC-0003', 'ARC-0004'].map((id) => nodeEl(id).getAttribute('data-status')))
      .toEqual(['in_progress', 'waiting', 'waiting', 'done']);
    expect(['ARC-0001', 'ARC-0002', 'ARC-0003'].every((id) => nodeEl(id).classList.contains('is-critical'))).toBe(true);
    expect(nodeEl('ARC-0004').classList.contains('is-critical')).toBe(false);
    expect(nodeEl('ARC-0004').classList.contains('is-external')).toBe(true);
    const edge = (key: string) => document.querySelector(`[data-edge='${key}']`)!;
    expect(edge('ARC-0001>ARC-0002').classList.contains('is-critical')).toBe(true);
    expect(edge('ARC-0004>ARC-0003').classList.contains('is-critical')).toBe(false);
    expect(screen.getByText(/2 ohne Abhängigkeiten ausgeblendet/)).toBeTruthy();
    expect(nodeEl('ARC-0002').getAttribute('aria-label')).toBe('ARC-0002 Titel ARC-0002 – Wartet, kritischer Pfad');
  });

  it('öffnet ein Ticket per Klick, Enter und Leertaste', async () => {
    const { onOpenTicket } = open();
    await waitFor(() => expect(nodeEl('ARC-0003')).toBeTruthy());
    await userEvent.click(nodeEl('ARC-0003'));
    expect(onOpenTicket).toHaveBeenLastCalledWith('ARC-0003');
    fireEvent.keyDown(nodeEl('ARC-0001'), { key: 'Enter' });
    expect(onOpenTicket).toHaveBeenLastCalledWith('ARC-0001');
    fireEvent.keyDown(nodeEl('ARC-0004'), { key: ' ' });
    expect(onOpenTicket).toHaveBeenLastCalledWith('ARC-0004');
  });

  it('hebt beim Hovern oder Fokussieren eines Knotens seine Kanten und Nachbarn hervor', async () => {
    open();
    await waitFor(() => expect(nodeEl('ARC-0002')).toBeTruthy());
    const edge = (key: string) => document.querySelector(`[data-edge='${key}']`)!.classList;
    fireEvent.mouseEnter(nodeEl('ARC-0002'));
    expect(edge('ARC-0001>ARC-0002').contains('is-upstream')).toBe(true);
    expect(edge('ARC-0002>ARC-0003').contains('is-downstream')).toBe(true);
    expect(edge('ARC-0004>ARC-0003').contains('is-dimmed')).toBe(true);
    expect(nodeEl('ARC-0002').classList.contains('is-hovered')).toBe(true);
    expect(nodeEl('ARC-0001').classList.contains('is-neighbor')).toBe(true);
    expect(nodeEl('ARC-0004').classList.contains('is-dimmed')).toBe(true);
    expect(document.querySelector('.dep-graph')!.classList.contains('has-hover')).toBe(true);

    fireEvent.mouseLeave(nodeEl('ARC-0002'));
    expect(edge('ARC-0001>ARC-0002').contains('is-upstream')).toBe(false);
    expect(edge('ARC-0004>ARC-0003').contains('is-dimmed')).toBe(false);

    fireEvent.focus(nodeEl('ARC-0003'));
    expect(edge('ARC-0004>ARC-0003').contains('is-upstream')).toBe(true);
    fireEvent.blur(nodeEl('ARC-0003'));
    expect(edge('ARC-0004>ARC-0003').contains('is-upstream')).toBe(false);
  });

  it('lädt beim Umschalten den passenden Umfang mit dessen Standard für Erledigte', async () => {
    const { getDbGraph } = open();
    await waitFor(() => expect(getDbGraph).toHaveBeenCalledTimes(1));
    await userEvent.click(screen.getByRole('button', { name: 'Ticket' }));
    await waitFor(() => expect(getDbGraph).toHaveBeenLastCalledWith({ kind: 'ticket', id: 'ARC-0002' }, true));
    await userEvent.click(screen.getByRole('button', { name: 'Spec 001-login' }));
    await waitFor(() => expect(getDbGraph).toHaveBeenLastCalledWith({ kind: 'spec', project: 'ARC', spec: '001-login' }, true));
    await userEvent.click(screen.getByRole('switch', { name: 'Erledigte' }));
    await waitFor(() => expect(getDbGraph).toHaveBeenLastCalledWith({ kind: 'spec', project: 'ARC', spec: '001-login' }, false));
    expect(screen.getByRole('button', { name: 'Spec 001-login' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('zeigt Kennzahlen als Chips und blendet die Legende über den Knopf aus und ein', async () => {
    open();
    expect(await screen.findByText('Kritischer Pfad: 3 offen')).toBeTruthy();
    expect(screen.getByText('4 Tickets')).toBeTruthy();
    const toggle = screen.getByRole('button', { name: 'Legende ein-/ausblenden' });
    expect(toggle.getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('group', { name: 'Legende' })).toBeTruthy();
    await userEvent.click(toggle);
    expect(toggle.getAttribute('aria-pressed')).toBe('false');
    expect(screen.queryByRole('group', { name: 'Legende' })).toBeNull();
    await userEvent.click(toggle);
    expect(screen.getByRole('group', { name: 'Legende' })).toBeTruthy();
  });

  it('meldet einen Kreis in den Daten und markiert die Kante', async () => {
    open({ ...GRAPH, cycleEdges: [{ from: 'ARC-0003', to: 'ARC-0001' }] });
    expect((await screen.findByRole('alert')).textContent).toContain('Kreis in den Daten: ARC-0001 setzt ARC-0003 voraus');
    expect(document.querySelector("[data-edge='ARC-0003>ARC-0001']")!.classList.contains('is-cycle')).toBe(true);
  });

  it('zeigt den Fortschritt der Schritte als Zahl und Balken, ohne Schritte nichts', async () => {
    open({
      ...GRAPH,
      nodes: [
        node('ARC-0001', 'in_progress', { stepsDone: 3, stepsTotal: 7 }),
        node('ARC-0002', 'waiting'),
        node('ARC-0003', 'waiting', { stepsTotal: 4 }),
        node('ARC-0004', 'done', { external: true, boardName: 'Zweites', stepsDone: 7, stepsTotal: 7 }),
      ],
    });
    await waitFor(() => expect(nodeEl('ARC-0001')).toBeTruthy());
    const width = (id: string, part: string) => Number(steps(id)!.querySelector(`.dep-graph-steps-${part}`)!.getAttribute('width'));
    const track = width('ARC-0001', 'track');
    expect(track).toBeGreaterThan(0);
    expect(steps('ARC-0001')!.textContent).toBe('3/7');
    expect(width('ARC-0001', 'fill')).toBe(Math.round((track * 3) / 7));
    expect(steps('ARC-0001')!.classList.contains('is-complete')).toBe(false);
    expect(steps('ARC-0003')!.textContent).toBe('0/4');
    expect(width('ARC-0003', 'fill')).toBe(0);
    expect(steps('ARC-0004')!.textContent).toBe('7/7');
    expect(width('ARC-0004', 'fill')).toBe(track);
    expect(steps('ARC-0004')!.classList.contains('is-complete')).toBe(true);
    expect(steps('ARC-0002')).toBeNull();
    expect(nodeEl('ARC-0001').getAttribute('aria-label')).toBe('ARC-0001 Titel ARC-0001 – In Arbeit, kritischer Pfad, 3 von 7 Schritten erledigt');
    expect(nodeEl('ARC-0004').getAttribute('aria-label')).toBe('ARC-0004 Titel ARC-0004 – Erledigt, 7 von 7 Schritten erledigt – Board Zweites');
    expect(nodeEl('ARC-0001').querySelector('title')!.textContent).toContain('3 von 7 Schritten erledigt');
  });

  it('lädt bei einer Live-Meldung des Boards ohne Ladeanzeige nach und behält den Zoom', async () => {
    const updated = { ...GRAPH, nodes: GRAPH.nodes.map((n) => (n.id === 'ARC-0001' ? { ...n, stepsDone: 4, stepsTotal: 7 } : n)) };
    const getDbGraph = vi.fn().mockResolvedValueOnce(GRAPH).mockResolvedValue(updated);
    const { emit } = openLive(getDbGraph);
    await waitFor(() => expect(nodeEl('ARC-0001')).toBeTruthy());
    const fitted = zoomValue();
    await userEvent.click(screen.getByRole('button', { name: 'Vergrößern' }));
    const zoomed = zoomValue();
    expect(zoomed).not.toBe(fitted);

    emit((handler) => handler.onDbBoard?.(1));
    expect(screen.queryByText('Graph wird geladen …')).toBeNull();
    await waitFor(() => expect(steps('ARC-0001')?.textContent).toBe('4/7'));
    expect(getDbGraph).toHaveBeenCalledTimes(2);
    expect(getDbGraph).toHaveBeenLastCalledWith({ kind: 'board', boardId: 1 }, false);
    expect(zoomValue()).toBe(zoomed);

    emit((handler) => handler.onReset?.());
    await waitFor(() => expect(getDbGraph).toHaveBeenCalledTimes(3));
  });

  it('behält den Graphen, wenn das Nachladen nach einer Live-Meldung fehlschlägt', async () => {
    const getDbGraph = vi.fn().mockResolvedValueOnce(GRAPH).mockRejectedValue(new Error('Server weg'));
    const { emit } = openLive(getDbGraph);
    await waitFor(() => expect(nodeEl('ARC-0001')).toBeTruthy());
    emit((handler) => handler.onDbBoard?.(1));
    await waitFor(() => expect(getDbGraph).toHaveBeenCalledTimes(2));
    await act(async () => undefined);
    expect(nodeEl('ARC-0001')).toBeTruthy();
    expect(screen.queryByText('Server weg')).toBeNull();
  });

  it('zeigt einen Hinweis ohne Knoten und schließt per Knopf', async () => {
    const { onClose } = open({ ...GRAPH, nodes: [], edges: [], criticalPath: [] });
    expect(await screen.findByText('Keine Tickets mit Abhängigkeiten.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(onClose).toHaveBeenCalled();
  });
});
