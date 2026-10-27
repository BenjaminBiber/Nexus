import { describe, expect, it } from 'vitest';
import { TimelineClassifier } from '../../../src/board/TimelineClassifier.js';
import { DISPATCH_MARKERS } from '../../../src/dispatch/DispatchConfig.js';
import type { Comment, HistoryEntry } from '../../../src/board/BoardTypes.js';

const AT = '2026-10-13T11:00:00.000Z';
const names = { dispatcherName: 'dispatcher', agentPrefix: 'junior-' };

function entry(field: string, from: string | null, to: string | null, byName = 'Florian', note: string | null = null): HistoryEntry {
  return { ticketId: 'ARC-0001', at: AT, byId: 'u', byName, field, from, to, note };
}

function comment(body: string, authorName = 'dispatcher'): Comment {
  return { id: 1, ticketId: 'ARC-0001', authorId: 'u', authorName, body, createdAt: AT, readByAgentAt: null };
}

describe('TimelineClassifier', () => {
  const c = new TimelineClassifier(names);

  it.each([
    ['status', 'status'], ['blockedReason', 'status'],
    ['assignee', 'claim'], ['planningClaimBy', 'claim'],
    ['question', 'question'], ['branch', 'git'], ['worktree', 'git'],
    ['plan', 'content'], ['handbook', 'content'], ['acceptance', 'content'], ['step', 'content'], ['steps', 'content'], ['description', 'content'],
    ['priority', 'other'], ['labels', 'other'], ['dependsOn', 'other'],
  ])('ordnet das Feld %s der Art %s zu', (field, kind) => {
    expect(c.history(entry(field, 'a', 'b'))).toMatchObject({ kind, type: `field:${field}`, field, from: 'a', to: 'b' });
  });

  it('erkennt Agenten an Dispatcher-Name, Vorsilbe und „claude“, sonst Person', () => {
    expect(c.actor('dispatcher')).toEqual({ name: 'dispatcher', type: 'agent' });
    expect(c.actor('junior-ARC-0001')).toEqual({ name: 'junior-ARC-0001', type: 'agent' });
    expect(c.actor('claude')).toEqual({ name: 'claude', type: 'agent' });
    expect(c.actor('Florian')).toEqual({ name: 'Florian', type: 'person' });
    expect(new TimelineClassifier(null).actor('junior-ARC-0001').type).toBe('person');
  });

  it('nimmt bei Fragen die Art des Autors', () => {
    expect(c.actor('Florian', 'session').type).toBe('agent');
    expect(c.actor('Florian', 'subagent').type).toBe('agent');
    expect(c.actor('junior-x', 'human').type).toBe('person');
  });

  it('liest Profil aus dem Start-Kommentar', () => {
    const body = `${DISPATCH_MARKERS.START}: an junior-ARC-0001 übergeben – Branch ticket/ARC-0001-x, Profil standard (Modell sonnet, Effort high, max. 80 Schritte, 60 min). Grund: Regel.`;
    expect(c.comment(comment(body))).toMatchObject({ kind: 'run', type: 'dispatch:start', ref: { profile: 'standard' }, actor: { type: 'agent' } });
  });

  it('liest Ziel und Commit aus dem Merge-Kommentar', () => {
    expect(c.comment(comment(`${DISPATCH_MARKERS.MERGED} in main (abc1234).`))).toMatchObject({ kind: 'git', type: 'merge', ref: { commit: 'abc1234' } });
  });

  it.each([
    [`${DISPATCH_MARKERS.ABORT} Versuch 1/3: Zeitlimit.`, 'run', 'dispatch:abort'],
    [`${DISPATCH_MARKERS.PAUSE}: Nutzungslimit`, 'run', 'dispatch:pause'],
    [`${DISPATCH_MARKERS.QUESTION}: wartet auf ARC-F0001`, 'run', 'dispatch:question'],
    [`${DISPATCH_MARKERS.SETUP} fehlgeschlagen: npm ci`, 'run', 'dispatch:setup'],
    [`${DISPATCH_MARKERS.REVIEW}: wartet auf Freigabe`, 'run', 'dispatch:review'],
    [`${DISPATCH_MARKERS.REVIEW_RETURN} (von Florian): bitte Tests`, 'run', 'dispatch:review-return'],
    [`${DISPATCH_MARKERS.MERGE_CONFLICT} in a.ts beim Merge in main – Merge-Session gestartet (Profil schwer).`, 'git', 'dispatch:merge-conflict'],
    [`${DISPATCH_MARKERS.MERGE_WAIT} auf ARC-0002`, 'git', 'dispatch:merge-wait'],
    [`${DISPATCH_MARKERS.MERGE} nicht möglich: lokale Änderungen`, 'git', 'dispatch:merge'],
  ])('ordnet „%s“ zu', (body, kind, type) => {
    expect(c.comment(comment(body))).toMatchObject({ kind, type, text: body });
  });

  it('lässt normale Kommentare als Kommentar, auch vom Dispatcher', () => {
    expect(c.comment(comment('Bitte prüfen', 'Florian'))).toMatchObject({ kind: 'comment', type: 'comment', actor: { type: 'person' } });
    expect(c.comment(comment('Hinweis ohne Marker'))).toMatchObject({ kind: 'comment', type: 'comment' });
  });

  it('liest die Fragen-ID aus dem Vermerk eines Frage-Eintrags', () => {
    expect(c.history(entry('question', 'open', 'answered', 'Florian', 'Entschieden: ARC-F0002 „Speicher?“'))).toMatchObject({ ref: { questionId: 'ARC-F0002' } });
  });
});
