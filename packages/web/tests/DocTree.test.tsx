// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DocFileDto } from '../src/api/types.js';
import type { DocTreeFilter } from '../src/board/docsTree.js';
import { DocTree } from '../src/components/DocTree.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

const SPEC = 'docs/superpowers/specs/2026-10-22-a-design.md';
const PLAN = 'docs/superpowers/plans/2026-10-22-a-plan.md';

const docs: DocFileDto[] = [
  { path: SPEC, title: 'Thema A – Design', group: 'spec', kind: 'spec', topic: 'a', date: '2026-10-22', mtimeMs: 300, exists: true },
  { path: PLAN, title: 'Thema A – Plan', group: 'plan', kind: 'plan', topic: 'a', date: '2026-10-22', mtimeMs: 200, exists: true },
  { path: 'GLOSSARY.md', title: 'Glossar', group: 'glossary', kind: null, topic: null, date: null, mtimeMs: 50, exists: true },
];

// Zeigt die aktuelle Adresse zur Prüfung an
function Address() {
  const l = useLocation();
  return <span data-testid="address">{l.pathname}</span>;
}

// Rendert den Baum mit gefälschtem ApiClient unter /p/p1/docs
// `seenMs` liegt voreingestellt über allen Dateien, damit das Badge „neu“ die Namen der Knoten nicht verändert
function open(current: string | null = null, overrides: Parameters<typeof fakeApi>[0] = {}, seenMs = 1000, filter?: DocTreeFilter, list = docs) {
  const api = fakeApi(overrides);
  render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={['/p/p1/docs']}>
        <Routes>
          <Route path="*" element={<DocTree docs={list} pid="p1" current={current} seenMs={seenMs} filter={filter} />} />
        </Routes>
        <Address />
      </MemoryRouter>
    </ApiProvider>,
  );
}

// Der Knoten mit dieser Beschriftung
const item = (name: string | RegExp) => screen.getByRole('treeitem', { name });

describe('DocTree', () => {
  it('zeigt die Ordner offen, ohne Hüllordner wie docs/superpowers, Ordner vor Dateien', async () => {
    open();
    const tree = await screen.findByRole('tree', { name: 'Dokumente' });
    const top = within(tree).getAllByRole('treeitem').filter((li) => li.getAttribute('aria-level') === '1');
    expect(top.map((li) => li.getAttribute('aria-label'))).toEqual(['plans', 'specs', 'GLOSSARY.md']);
    expect(item('plans').getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('link', { name: /2026-10-22-a-design\.md/ }).getAttribute('href')).toBe(`/p/p1/docs/${SPEC}`);
    expect(screen.getByRole('link', { name: /2026-10-22-a-plan\.md/ }).getAttribute('href')).toBe(`/p/p1/docs/${PLAN}`);
  });

  it('gibt Plänen und Docs eigene Symbole', async () => {
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    expect(item('2026-10-22-a-plan.md').querySelector('.doc-tree-icon.is-plan')).not.toBeNull();
    expect(item('2026-10-22-a-design.md').querySelector('.doc-tree-icon.is-doc')).not.toBeNull();
    expect(item('GLOSSARY.md').querySelector('.doc-tree-icon.is-glossary')).not.toBeNull();
    // Ordner tragen einen Pfeil, aber kein Symbol
    expect(item('plans').querySelector(':scope > .doc-tree-row .doc-tree-icon')).toBeNull();
  });

  it('öffnet beim Klick auf eine Datei das Dokument und klappt Ordner per Klick zu', async () => {
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    await userEvent.click(screen.getByText('2026-10-22-a-design.md'));
    expect(screen.getByTestId('address').textContent).toBe(`/p/p1/docs/${SPEC}`);
    await userEvent.click(screen.getByText('plans'));
    expect(item('plans').getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByText('2026-10-22-a-plan.md')).toBeNull();
  });

  it('klappt den Pfad zum geöffneten Dokument auf und hebt es hervor', async () => {
    window.localStorage.setItem('harness.docsTreeOpen.p1', JSON.stringify(['dir:docs/superpowers/plans']));
    open(PLAN);
    await screen.findByRole('tree', { name: 'Dokumente' });
    const plan = item('2026-10-22-a-plan.md');
    expect(item('plans').getAttribute('aria-expanded')).toBe('true');
    expect(plan.getAttribute('aria-selected')).toBe('true');
    expect(within(plan).getByRole('link').getAttribute('aria-current')).toBe('page');
    expect(item('2026-10-22-a-design.md').getAttribute('aria-selected')).toBe('false');
  });

  it('lässt sich mit der Tastatur bedienen', async () => {
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    item('plans').focus();
    // links klappt zu, rechts wieder auf, runter auf die erste Datei
    await userEvent.keyboard('{ArrowLeft}');
    expect(item('plans').getAttribute('aria-expanded')).toBe('false');
    await userEvent.keyboard('{ArrowRight}');
    expect(item('plans').getAttribute('aria-expanded')).toBe('true');
    await userEvent.keyboard('{ArrowDown}');
    expect(document.activeElement).toBe(item('2026-10-22-a-plan.md'));
    // Enter öffnet das Dokument, links springt zurück zum Ordner
    await userEvent.keyboard('{Enter}');
    expect(screen.getByTestId('address').textContent).toBe(`/p/p1/docs/${PLAN}`);
    await userEvent.keyboard('{ArrowLeft}');
    expect(document.activeElement).toBe(item('plans'));
  });

  it('zeigt den Fortschritt der Spec unter ihrer Datei', async () => {
    const getSpecProgress = vi.fn(async () => [{ spec: SPEC, total: 2, done: 1, blocked: 0, waiting: 0, inProgress: 0, open: 1, notNeeded: 0 }]);
    open(null, { getSpecProgress });
    const bar = await screen.findByRole('img', { name: (n) => n.endsWith('1 von 2 erledigt, 0 blockiert, 0 wartet, 0 in Arbeit, 1 offen') });
    expect(item('2026-10-22-a-design.md').contains(bar)).toBe(true);
    expect(getSpecProgress).toHaveBeenCalledWith('p1');
  });

  it('trägt „neu“ an Dateien, die jünger sind als der gemerkte Stand', async () => {
    open(null, {}, 100);
    await screen.findByRole('tree', { name: 'Dokumente' });
    // Spec (300 ms) und Plan (200 ms) sind neuer als 100; das Glossar (50 ms) nicht. Offene Ordner tragen kein Badge.
    expect(screen.getAllByText('neu')).toHaveLength(2);
    expect(item('2026-10-22-a-design.md, neu')).toBeTruthy();
    await userEvent.click(screen.getByText('specs'));
    expect(screen.getAllByText('neu')).toHaveLength(2);
    expect(item('specs, neu')).toBeTruthy();
  });

  it('merkt zugeklappte Ordner über ein Neuladen hinweg', async () => {
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    await userEvent.click(screen.getByText('specs'));
    expect(JSON.parse(window.localStorage.getItem('harness.docsTreeOpen.p1') ?? '[]')).toContain('dir:docs/superpowers/specs');
    cleanup();
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    expect(item('specs').getAttribute('aria-expanded')).toBe('false');
    expect(item('plans').getAttribute('aria-expanded')).toBe('true');
  });

  it('zeigt den Titel der Datei als Tooltip', async () => {
    open();
    await screen.findByRole('tree', { name: 'Dokumente' });
    expect(within(item('2026-10-22-a-design.md')).getByRole('link').getAttribute('title')).toBe('Thema A – Design');
  });

  it('filtert nach Titel, hebt Treffer im Dateinamen hervor und blendet leere Ordner aus', async () => {
    open(null, {}, 1000, { query: 'plan', kind: 'all' });
    await screen.findByRole('tree', { name: 'Dokumente' });
    expect(screen.queryByRole('treeitem', { name: 'GLOSSARY.md' })).toBeNull();
    expect(screen.queryByRole('treeitem', { name: 'specs' })).toBeNull();
    expect(within(item('2026-10-22-a-plan.md')).getByText('plan').tagName).toBe('MARK');
  });

  it('meldet einen Filter ohne Treffer', async () => {
    open(null, {}, 1000, { query: 'gibtsnicht', kind: 'all' });
    expect(await screen.findByText('Keine Docs für „gibtsnicht“.')).toBeTruthy();
  });
});
