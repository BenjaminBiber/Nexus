// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import type { FavoriteDto } from '../src/api/types.js';
import { FavoriteItem } from '../src/components/FavoriteItem.js';
import { SessionGroup } from '../src/components/SessionGroup.js';

afterEach(cleanup);

const ODD = 'A/B?#1';
const ENCODED = encodeURIComponent(ODD);

describe('Ticket-Links mit kodierter ID', () => {
  it('kodiert die Ticket-ID im Link der Session-Gruppe', () => {
    render(<MemoryRouter><SessionGroup pid="p1" ticket={{ id: ODD, title: 'Seltsam' }} sessions={[]} now={0} /></MemoryRouter>);
    expect(screen.getByRole('link', { name: 'Ticket öffnen' }).getAttribute('href')).toBe(`/p/p1/board/t/${ENCODED}`);
  });

  it('kodiert die Ticket-ID im Link des Favoriten', () => {
    const favorite = {
      sessionId: 'aaaaaaaa-1', note: '', tickets: [ODD], ticketInfo: [{ id: ODD, title: 'Seltsam', status: 'planned' }], cwd: 'D:/wt', cwdExists: true,
      title: null, markedAt: '2026-10-10T10:05:00.000Z', state: 'running', displayState: 'running', live: true,
      lastEventAt: '2026-10-10T10:05:00.000Z', resumedAs: [], command: 'claude --resume aaaaaaaa-1',
    } as unknown as FavoriteDto;
    render(<MemoryRouter><FavoriteItem pid="p1" favorite={favorite} now={0} onNote={async () => {}} onRemove={async () => {}} /></MemoryRouter>);
    expect(screen.getByRole('link', { name: /Seltsam/ }).getAttribute('href')).toBe(`/p/p1/board/t/${ENCODED}`);
  });
});
