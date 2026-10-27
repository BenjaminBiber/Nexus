// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { DbColumnId, DbTicketDetailDto } from '../src/api/dbTypes.js';
import { DbDependencies } from '../src/components/db/DbDependencies.js';
import {
  CYCLE_REASON, canEditDependencies, dependentsClosure, pickerGroups, withDependency, withoutDependency, type PickerTicket,
} from '../src/dbboard/dependencyPicker.js';
import { dbCard, dbDetail, dbView } from './helpers/fakeApi.js';

afterEach(cleanup);

const COLUMNS = dbView([]).columns.map(({ id, title }) => ({ id, title }));

// Ticket für die reinen Funktionen
const t = (id: string, extra: Partial<PickerTicket> = {}): PickerTicket => ({ id, title: `Titel ${id}`, boardId: 1, status: 'backlog', column: 'backlog' as DbColumnId, dependsOn: [], ...extra });

describe('dependencyPicker', () => {
  it('bietet offene Tickets desselben Boards nach Spalte gruppiert an', () => {
    const me = t('A-1', { dependsOn: ['A-2'] });
    const groups = pickerGroups(me, [me, t('A-2'), t('A-3'), t('A-4', { status: 'in_progress', column: 'in_progress' }), t('A-5', { status: 'done', column: 'done' }), t('B-1', { boardId: 2 })], COLUMNS);
    expect(groups).toEqual([
      { id: 'backlog', title: 'Backlog', entries: [{ id: 'A-3', title: 'Titel A-3', disabled: false, reason: null }] },
      { id: 'in_progress', title: 'In Arbeit', entries: [{ id: 'A-4', title: 'Titel A-4', disabled: false, reason: null }] },
    ]);
  });

  it('sperrt Tickets, die einen Kreis ergäben, auch über Umwege', () => {
    const me = t('A-1');
    const tickets = [me, t('A-2', { dependsOn: ['A-1'] }), t('A-3', { dependsOn: ['A-2'], column: 'waiting' })];
    expect([...dependentsClosure('A-1', tickets)].sort()).toEqual(['A-2', 'A-3']);
    expect(pickerGroups(me, tickets, COLUMNS).flatMap((g) => g.entries)).toEqual([
      { id: 'A-2', title: 'Titel A-2', disabled: true, reason: CYCLE_REASON },
      { id: 'A-3', title: 'Titel A-3', disabled: true, reason: CYCLE_REASON },
    ]);
  });

  it('erlaubt das Bearbeiten nur bei offenen Tickets und fügt ohne Doppelte hinzu', () => {
    expect(canEditDependencies({ status: 'planned' })).toBe(true);
    expect(canEditDependencies({ status: 'done' })).toBe(false);
    expect(canEditDependencies({ status: 'not_needed' })).toBe(false);
    expect(withDependency(['A'], 'B')).toEqual(['A', 'B']);
    expect(withDependency(['A'], 'A')).toEqual(['A']);
    expect(withoutDependency(['A', 'B'], 'A')).toEqual(['B']);
  });
});

describe('DbDependencies', () => {
  const ME = dbDetail(dbCard('ARC-0001', 'Ich', 'backlog', { dependsOn: ['ARC-0002'], dependsOpen: ['ARC-0002'], dependents: ['ARC-0004'], column: 'waiting', waiting: true }));
  const BOARD = [
    dbCard('ARC-0001', 'Ich', 'backlog', { dependsOn: ['ARC-0002'], column: 'waiting', waiting: true }),
    dbCard('ARC-0002', 'Voraussetzung'),
    dbCard('ARC-0003', 'Kandidat', 'planned'),
    dbCard('ARC-0005', 'Zweiter Kandidat'),
    dbCard('ARC-0004', 'Abhängig', 'backlog', { dependsOn: ['ARC-0001'], column: 'waiting', waiting: true }),
  ];

  // Rendert den Picker mit austauschbarem Ticket
  function open(ticket: DbTicketDetailDto = ME, extra: { busy?: boolean; conflictOpen?: boolean } = {}) {
    const onChange = vi.fn(async (_update: (current: string[]) => string[]) => true);
    const onOpen = vi.fn();
    render(<DbDependencies ticket={ticket} boardTickets={BOARD} columns={COLUMNS} busy={extra.busy ?? false} conflictOpen={extra.conflictOpen ?? false} onChange={onChange} onOpen={onOpen} />);
    return { onChange, onOpen };
  }

  it('zeigt Voraussetzungen und Abhängige als Chips und öffnet sie', async () => {
    const { onOpen } = open();
    const needs = screen.getByRole('group', { name: 'Setzt voraus' });
    expect(within(needs).getByRole('button', { name: 'ARC-0002' }).className).toContain('is-open');
    await userEvent.click(within(screen.getByRole('group', { name: 'Voraussetzung für' })).getByRole('button', { name: 'ARC-0004' }));
    expect(onOpen).toHaveBeenCalledWith('ARC-0004');
  });

  it('nennt die Tickets, die mit diesem bereit werden, und öffnet auf Wunsch den Graphen', async () => {
    const onShowGraph = vi.fn();
    const onOpen = vi.fn();
    const ticket = { ...ME, unlocks: ['ARC-0004'] };
    render(<DbDependencies ticket={ticket} boardTickets={BOARD} columns={COLUMNS} busy={false} onChange={vi.fn(async () => true)} onOpen={onOpen} onShowGraph={onShowGraph} />);
    const ready = screen.getByRole('group', { name: 'Wird bereit, wenn erledigt' });
    await userEvent.click(within(ready).getByRole('button', { name: 'ARC-0004' }));
    expect(onOpen).toHaveBeenCalledWith('ARC-0004');
    await userEvent.click(screen.getByRole('button', { name: 'Graph anzeigen' }));
    expect(onShowGraph).toHaveBeenCalled();
  });

  it('zeigt ohne unlocks keinen Hinweis und ohne onShowGraph keinen Graph-Knopf', () => {
    open();
    expect(screen.queryByRole('group', { name: 'Wird bereit, wenn erledigt' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Graph anzeigen' })).toBeNull();
  });

  it('fügt über das Menü hinzu und sperrt Kreise', async () => {
    const { onChange } = open();
    await userEvent.click(screen.getByRole('button', { name: '+ Voraussetzung' }));
    const menu = screen.getByRole('menu', { name: 'Voraussetzung wählen' });
    expect((within(menu).getByRole('menuitem', { name: /^ARC-0004/ }) as HTMLButtonElement).disabled).toBe(true);
    expect(within(menu).getByRole('group', { name: 'Geplant' })).toBeTruthy();
    await userEvent.click(within(menu).getByRole('menuitem', { name: 'ARC-0003 Kandidat' }));
    expect(onChange.mock.calls[0][0](['ARC-0002'])).toEqual(['ARC-0002', 'ARC-0003']);
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('entfernt eine Voraussetzung', async () => {
    const { onChange } = open();
    await userEvent.click(screen.getByRole('button', { name: 'ARC-0002 entfernen' }));
    expect(onChange.mock.calls[0][0](['ARC-0002'])).toEqual([]);
  });

  it('zeigt bei erledigten Tickets nur lesend', () => {
    open({ ...ME, status: 'done', column: 'done', waiting: false });
    expect(screen.queryByRole('button', { name: '+ Voraussetzung' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'ARC-0002 entfernen' })).toBeNull();
    expect(screen.getByRole('button', { name: 'ARC-0002' })).toBeTruthy();
  });

  it('erklärt die Sperre bei offener Voraussetzung in Arbeit', () => {
    open({ ...ME, status: 'in_progress', column: 'in_progress', waiting: false });
    expect(screen.getByText(/Bleibt in „In Arbeit“ stehen/)).toBeTruthy();
  });

  describe('Tastatur', () => {
    const toggle = () => screen.getByRole('button', { name: '+ Voraussetzung' });

    it('öffnet mit Pfeil runter und fokussiert den ersten wählbaren Punkt (gesperrte werden übersprungen)', async () => {
      open();
      toggle().focus();
      await userEvent.keyboard('{ArrowDown}');
      const menu = screen.getByRole('menu');
      const enabled = within(menu).getAllByRole('menuitem').filter((i) => !(i as HTMLButtonElement).disabled);
      expect(enabled.length).toBeGreaterThan(0);
      expect(document.activeElement).toBe(enabled[0]);
    });

    it('wechselt zwischen mehreren Punkten', async () => {
      open({ ...ME, dependents: [] });
      toggle().focus();
      await userEvent.keyboard('{ArrowDown}');
      const items = within(screen.getByRole('menu')).getAllByRole('menuitem').filter((i) => !(i as HTMLButtonElement).disabled);
      expect(items.length).toBeGreaterThan(1);
      expect(document.activeElement).toBe(items[0]);
      await userEvent.keyboard('{ArrowDown}');
      expect(document.activeElement).toBe(items[1]);
      await userEvent.keyboard('{ArrowUp}');
      expect(document.activeElement).toBe(items[0]);
    });

    it('schließt mit Esc und gibt den Fokus an den Schalter zurück, ohne das Panel zu schließen', async () => {
      const onKey = vi.fn();
      render(
        <div onKeyDown={onKey}>
          <DbDependencies ticket={ME} boardTickets={BOARD} columns={COLUMNS} busy={false} onChange={vi.fn(async () => true)} onOpen={vi.fn()} />
        </div>,
      );
      toggle().focus();
      await userEvent.keyboard('{ArrowDown}{Escape}');
      expect(screen.queryByRole('menu')).toBeNull();
      expect(document.activeElement).toBe(toggle());
      expect(onKey.mock.calls.some(([e]) => e.key === 'Escape')).toBe(false);
    });

    it('schließt beim Verlassen per Tab und bei Klick außerhalb', async () => {
      open();
      await userEvent.click(toggle());
      expect(screen.getByRole('menu')).toBeTruthy();
      await userEvent.tab();
      await userEvent.tab();
      await userEvent.tab();
      await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
      await userEvent.click(toggle());
      expect(screen.getByRole('menu')).toBeTruthy();
      await userEvent.click(document.body);
      expect(screen.queryByRole('menu')).toBeNull();
    });
  });

  it('sperrt Schalter und Entfernen bei laufendem Speichern oder offenem Konflikt', () => {
    for (const extra of [{ busy: true }, { conflictOpen: true }]) {
      cleanup();
      open(ME, extra);
      expect((screen.getByRole('button', { name: '+ Voraussetzung' }) as HTMLButtonElement).disabled).toBe(true);
      expect((screen.getByRole('button', { name: 'ARC-0002 entfernen' }) as HTMLButtonElement).disabled).toBe(true);
    }
  });

  it('sperrt Menüpunkte, wenn das Speichern erst nach dem Öffnen beginnt', async () => {
    const props = { ticket: ME, boardTickets: BOARD, columns: COLUMNS, onChange: vi.fn(async () => true), onOpen: vi.fn() };
    const { rerender } = render(<DbDependencies {...props} busy={false} />);
    await userEvent.click(screen.getByRole('button', { name: '+ Voraussetzung' }));
    rerender(<DbDependencies {...props} busy />);
    expect((screen.getByRole('menuitem', { name: 'ARC-0003 Kandidat' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('schließt das Menü, wenn ein anderes Ticket gezeigt wird (key auf die ID)', async () => {
    const props = { boardTickets: BOARD, columns: COLUMNS, busy: false, onChange: vi.fn(async () => true), onOpen: vi.fn() };
    const { rerender } = render(<DbDependencies key={ME.id} ticket={ME} {...props} />);
    await userEvent.click(screen.getByRole('button', { name: '+ Voraussetzung' }));
    expect(screen.getByRole('menu')).toBeTruthy();
    const other = { ...ME, id: 'ARC-0003' };
    rerender(<DbDependencies key={other.id} ticket={other} {...props} />);
    expect(screen.queryByRole('menu')).toBeNull();
  });
});
