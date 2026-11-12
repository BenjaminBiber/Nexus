// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, screen } from '@testing-library/react';
import type { DocFileDto } from '../src/api/types.js';
import { DocsNavLink } from '../src/components/DocsNavLink.js';
import { NavCountsProvider } from '../src/hooks/useNavCounts.js';
import { fakeApi } from './helpers/fakeApi.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { renderWithApp } from './helpers/renderWithApp.js';

afterEach(() => {
  cleanup();
  FakeEventSource.reset();
  window.localStorage.clear();
});

const docs: DocFileDto[] = [
  { path: 'docs/superpowers/specs/2026-10-22-a-design.md', title: 'A – Design', group: 'spec', kind: 'spec', topic: 'a', date: '2026-10-22', mtimeMs: 300, exists: true },
  { path: 'docs/superpowers/specs/2026-09-01-b-design.md', title: 'B – Design', group: 'spec', kind: 'spec', topic: 'b', date: '2026-09-01', mtimeMs: 100, exists: true },
];

// Rendert den Menüpunkt „Docs“ mit gefälschter API
function open() {
  const api = fakeApi({ getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs }) });
  renderWithApp(
    <NavCountsProvider pid="p1">
      <DocsNavLink pid="p1" />
    </NavCountsProvider>,
{ api, path: '/p/p1/board', route: '/p/:pid/board' });
}

describe('DocsNavLink', () => {
  it('zählt die Dokumente, die seit dem gemerkten Stand neu sind', async () => {
    window.localStorage.setItem('harness.docsSeen.p1', '100');
    open();
    expect(await screen.findByText('1')).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Docs, 1 neu' })).toBeTruthy();
  });

  it('zeigt ohne neue Dokumente nur den Namen', async () => {
    window.localStorage.setItem('harness.docsSeen.p1', '300');
    open();
    expect(await screen.findByText('Docs')).toBeTruthy();
    expect(screen.queryByText('1')).toBeNull();
    expect(screen.queryByText('2')).toBeNull();
  });
});
