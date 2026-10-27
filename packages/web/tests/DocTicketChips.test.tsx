// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, fireEvent, screen, within } from '@testing-library/react';
import type { DocFileDto, DocTicketRefDto } from '../src/api/types.js';
import { DocsPage } from '../src/pages/DocsPage.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

const SPEC = 'docs/superpowers/specs/2026-10-22-spec-docs-rework-design.md';
const CONTENT = '# Spec\n\n## Ziel\n\nEine Quelle für Docs.\n\n## Plan\n\nSchritt eins.\n';

const docs: DocFileDto[] = [
  { path: SPEC, title: 'Spec-Docs-Rework – Design', group: 'spec', kind: 'spec', topic: 'spec-docs-rework', date: '2026-10-22', mtimeMs: 300, exists: true },
];

const refs: DocTicketRefDto[] = [
  { id: 'ARC-0001', title: 'Ziel umsetzen', status: 'in_progress', column: 'in_progress', heading: 'Ziel', startLine: 3, endLine: 5 },
  { id: 'ARC-0002', title: 'Ohne Stelle', status: 'backlog', column: 'backlog', heading: 'Weg', startLine: 9, endLine: 9 },
];

// Öffnet das Dokument mit den übergebenen Tickets
function open(tickets: DocTicketRefDto[] = refs) {
  const api = fakeApi({
    getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs }),
    getDoc: async (_pid: string, p: string) => ({ path: p, content: CONTENT }),
    getDocComments: async () => [],
    getDocTickets: async () => tickets,
  });
  renderWithApp(<DocsPage />, { api, path: `/p/p1/docs/${SPEC}`, route: '/p/:pid/docs/*' });
}

describe('Ticket-Chips im Dokument', () => {
  it('setzt den Chip an die Überschrift, aus der das Ticket entstand', async () => {
    open();
    const chips = await screen.findByTestId('heading-tickets-ziel');
    const link = within(chips).getByRole('link', { name: /ARC-0001/ });
    expect(link.getAttribute('href')).toBe('/p/p1/board/t/ARC-0001');
    expect(link.getAttribute('data-status')).toBe('in_progress');
    expect(link.getAttribute('title')).toContain('Ziel umsetzen');
    expect(screen.queryByTestId('heading-tickets-plan')).toBeNull();
  });

  it('listet alle Tickets des Dokuments in der Seitenleiste, auch ohne passende Überschrift', async () => {
    open();
    await screen.findByTestId('heading-tickets-ziel');
    const list = screen.getByRole('list', { name: 'Tickets zu diesem Dokument' });
    const links = within(list).getAllByRole('link');
    expect(links.map((l) => l.textContent)).toEqual(['ARC-0001', 'ARC-0002']);
    expect(links[1].getAttribute('href')).toBe('/p/p1/board/t/ARC-0002');
    // Nur das Ticket mit vorhandener Überschrift springt ins Dokument
    expect(within(list).getByRole('button', { name: 'Ziel umsetzen' })).toBeTruthy();
    expect(within(list).queryByRole('button', { name: 'Ohne Stelle' })).toBeNull();
  });

  it('teilt die Tickets in Offen, Blockiert und Erledigt und zeigt den Fortschritt', async () => {
    open([
      ...refs,
      { id: 'ARC-0003', title: 'Hängt', status: 'blocked', column: 'blocked', heading: 'Ziel', startLine: 3, endLine: 5 },
      { id: 'ARC-0004', title: 'Fertig', status: 'done', column: 'done', heading: 'Plan', startLine: 7, endLine: 9 },
      { id: 'ARC-0005', title: 'Gestrichen', status: 'not_needed', column: 'not_needed', heading: 'Plan', startLine: 7, endLine: 9 },
    ]);
    const open_ = await screen.findByRole('button', { name: 'Offen (2)' });
    expect(open_.getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('img', { name: 'Fortschritt der Tickets: 40 %' })).toBeTruthy();
    expect(screen.getByText('Tickets erledigt')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Blockiert (1)' }));
    let list = screen.getByRole('list', { name: 'Tickets zu diesem Dokument' });
    expect(within(list).getAllByRole('link').map((l) => l.textContent)).toEqual(['ARC-0003']);

    // „entfällt“ zählt als erledigt
    fireEvent.click(screen.getByRole('button', { name: 'Erledigt (2)' }));
    list = screen.getByRole('list', { name: 'Tickets zu diesem Dokument' });
    expect(within(list).getAllByRole('link').map((l) => l.textContent)).toEqual(['ARC-0004', 'ARC-0005']);
  });

  it('zeigt ohne Tickets weder Chips noch Seitenleiste', async () => {
    open([]);
    expect(await screen.findByRole('heading', { name: 'Ziel' })).toBeTruthy();
    expect(screen.queryByTestId('heading-tickets-ziel')).toBeNull();
    expect(screen.queryByRole('list', { name: 'Tickets zu diesem Dokument' })).toBeNull();
  });
});
