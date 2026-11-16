// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { OverflowTabList, type OverflowTab } from '../src/components/OverflowTabList.js';
import { overflowLayout as layout, resetOverflowLayout } from './helpers/overflowLayout.js';

afterEach(cleanup);
afterEach(resetOverflowLayout);

const TABS: OverflowTab[] = ['A', 'B', 'C', 'D', 'E'].map((label) => ({ key: label.toLowerCase(), label, content: label }));

// Reiterleiste mit eigenem Zustand; meldet jede Wahl an onSelect
function Strip({ tabs = TABS, initial = 'a', onSelect = () => {} }: { tabs?: OverflowTab[]; initial?: string; onSelect?: (key: string) => void }) {
  const [active, setActive] = useState(initial);
  return (
    <OverflowTabList
      label="Inhalt"
      tabs={tabs}
      active={active}
      tabId={(key) => `tab-${key}`}
      panelId="panel"
      onSelect={(key) => { onSelect(key); setActive(key); }}
    />
  );
}

const tabNames = () => screen.getAllByRole('tab').map((tab) => tab.textContent);

describe('OverflowTabList', () => {
  it('zeigt alle Reiter ohne ⋯-Knopf, solange nichts gemessen ist', () => {
    render(<Strip />);
    expect(tabNames()).toEqual(['A', 'B', 'C', 'D', 'E']);
    expect(screen.queryByRole('button', { name: /Weitere Reiter/ })).toBeNull();
  });

  it('legt Reiter, die nicht passen, ins ⋯-Menü und wählt sie von dort', async () => {
    layout(400);
    const onSelect = vi.fn();
    render(<Strip onSelect={onSelect} />);
    expect(tabNames()).toEqual(['A', 'B', 'C']);
    await userEvent.click(screen.getByRole('button', { name: 'Weitere Reiter' }));
    const menu = screen.getByRole('menu', { name: 'Weitere Reiter' });
    expect(within(menu).getAllByRole('menuitem').map((item) => item.textContent)).toEqual(['D', 'E']);
    await userEvent.click(within(menu).getByRole('menuitem', { name: 'E' }));
    expect(onSelect).toHaveBeenCalledWith('e');
    expect(screen.queryByRole('menu')).toBeNull();
    // der gewählte Reiter rückt an die Stelle des letzten sichtbaren
    expect(tabNames()).toEqual(['A', 'B', 'E']);
    expect(screen.getByRole('tab', { name: 'E', selected: true })).toBeTruthy();
  });

  it('hält den aktiven Reiter von Anfang an sichtbar', () => {
    layout(400);
    render(<Strip initial="d" />);
    expect(tabNames()).toEqual(['A', 'B', 'D']);
  });

  it('markiert den ⋯-Knopf, wenn ein ausgeblendeter Reiter Aufmerksamkeit braucht', () => {
    layout(400);
    const tabs = TABS.map((tab) => (tab.key === 'e' ? { ...tab, attention: true } : tab));
    render(<Strip tabs={tabs} />);
    expect(screen.getByRole('button', { name: 'Weitere Reiter (mit Aktivität)' })).toBeTruthy();
  });

  it('schließt das Menü mit Escape und gibt den Fokus an den Knopf zurück', async () => {
    layout(400);
    render(<Strip />);
    const more = screen.getByRole('button', { name: 'Weitere Reiter' });
    await userEvent.click(more);
    expect(screen.getByRole('menu')).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(more);
  });
});
