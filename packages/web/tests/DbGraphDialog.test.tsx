// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DbGraphDto, DbGraphScope } from '../src/api/dbTypes.js';
import { DbGraphDialog } from '../src/components/db/DbGraphDialog.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const node = (id: string, column: DbGraphDto['nodes'][number]['column'], extra = {}) => ({
  id, title: `Titel ${id}`, status: column === 'waiting' ? 'planned' as const : column, column, boardId: 1, boardName: 'Sprint', spec: null, external: false, ...extra,
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
    await userEvent.click(screen.getByRole('checkbox', { name: 'Erledigte zeigen' }));
    await waitFor(() => expect(getDbGraph).toHaveBeenLastCalledWith({ kind: 'spec', project: 'ARC', spec: '001-login' }, false));
    expect(screen.getByRole('button', { name: 'Spec 001-login' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('meldet einen Kreis in den Daten und markiert die Kante', async () => {
    open({ ...GRAPH, cycleEdges: [{ from: 'ARC-0003', to: 'ARC-0001' }] });
    expect((await screen.findByRole('alert')).textContent).toContain('Kreis in den Daten: ARC-0001 setzt ARC-0003 voraus');
    expect(document.querySelector("[data-edge='ARC-0003>ARC-0001']")!.classList.contains('is-cycle')).toBe(true);
  });

  it('zeigt einen Hinweis ohne Knoten und schließt per Knopf', async () => {
    const { onClose } = open({ ...GRAPH, nodes: [], edges: [], criticalPath: [] });
    expect(await screen.findByText('Keine Tickets mit Abhängigkeiten.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(onClose).toHaveBeenCalled();
  });
});
