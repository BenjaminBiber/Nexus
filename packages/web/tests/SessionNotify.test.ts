import { describe, expect, it } from 'vitest';
import type { SessionDto } from '../src/api/types.js';
import { findReadySessions, readyTarget, rememberStates } from '../src/sessions/notify.js';

const session = (id: string, displayState: SessionDto['displayState']) => ({ id, displayState, tickets: [], cwd: '', title: null }) as unknown as SessionDto;

describe('findReadySessions', () => {
  it('meldet beim ersten Laden nichts', () => {
    expect(findReadySessions(new Map(), [session('a', 'waiting')])).toEqual([]);
  });

  it('meldet Wechsel von läuft nach bereit und nach wartet', () => {
    const known = rememberStates([session('a', 'running'), session('b', 'running'), session('c', 'running')]);
    const next = [session('a', 'idle'), session('b', 'waiting'), session('c', 'running')];
    expect(findReadySessions(known, next).map((s) => s.id)).toEqual(['a', 'b']);
  });

  it('meldet unveränderte oder beendete Sessions nicht', () => {
    const known = rememberStates([session('a', 'waiting'), session('b', 'running')]);
    expect(findReadySessions(known, [session('a', 'waiting'), session('b', 'ended')])).toEqual([]);
  });
});

describe('readyTarget', () => {
  it('führt zum ersten Ticket im Board, ohne Ticket zu den Sessions', () => {
    const withTicket = { ...session('a', 'waiting'), tickets: ['ARC-7', 'ARC-8'] } as SessionDto;
    expect(readyTarget('p1', withTicket)).toBe('/p/p1/board/t/ARC-7');
    expect(readyTarget('p1', session('b', 'idle'))).toBe('/p/p1/sessions');
  });
});
