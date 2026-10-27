import { describe, expect, it } from 'vitest';
import type { BoardQuestionDetail } from '@harness/core';
import { BoardQuestionFormatter } from '../../src/board/BoardQuestionFormatter.js';

// Kleinste Frage, die showText ausgeben kann; der Verlauf kommt vom Test
function detail(history: Array<{ byKind: string | null; byName: string }>): BoardQuestionDetail {
  return {
    id: 'ARC-F0001', title: 'Welcher Speicher?', state: 'open', ready: false, rev: 1, tags: [], originName: 'claude', originKind: 'session',
    originSession: null, occasion: null, tickets: [], dependsOn: [], waitingOn: [], description: '', research: [], decision: null, comments: [],
    history: history.map((h) => ({ ...h, at: '2026-01-01', type: 'created', text: 'Frage angelegt' })),
  } as unknown as BoardQuestionDetail;
}

describe('BoardQuestionFormatter', () => {
  it('zeigt im Verlauf die deutsche Art und unbekannte Arten unverändert', () => {
    const lines = new BoardQuestionFormatter().showText(detail([
      { byKind: 'human', byName: 'Florian' }, { byKind: 'robot', byName: 'Bot' }, { byKind: null, byName: 'System' },
    ]));
    expect(lines).toContain('  [2026-01-01] Florian (Mensch) created: Frage angelegt');
    expect(lines).toContain('  [2026-01-01] Bot (robot) created: Frage angelegt');
    expect(lines).toContain('  [2026-01-01] System created: Frage angelegt');
  });
});
