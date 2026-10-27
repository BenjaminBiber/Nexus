import { describe, expect, it } from 'vitest';
import type { DbTimelineEventDto } from '../src/api/dbTypes.js';
import { dayLabel, timelineView } from '../src/dbboard/timelineView.js';

const AT = '2026-10-16T12:00:00.000Z';
const agent = { name: 'dispatcher', type: 'agent' as const };
const person = { name: 'Florian', type: 'person' as const };

function ev(over: Partial<DbTimelineEventDto>): DbTimelineEventDto {
  return { at: AT, kind: 'other', type: 'x', actor: person, ...over };
}

describe('timelineView', () => {
  it('färbt Erledigt grün und Blockiert rot, beide hervorgehoben', () => {
    expect(timelineView(ev({ kind: 'status', type: 'field:status', field: 'status', from: 'in_progress', to: 'done' })))
      .toMatchObject({ tone: 'ok', title: 'In Arbeit → Erledigt', highlight: true });
    expect(timelineView(ev({ kind: 'status', type: 'field:status', field: 'status', from: 'planned', to: 'in_progress' })))
      .toMatchObject({ tone: 'plain', title: 'Geplant → In Arbeit', highlight: false });
  });

  it('zeigt einen Abbruch als Fehler mit Grund', () => {
    const v = timelineView(ev({ kind: 'run', type: 'dispatch:abort', actor: agent, text: '[dispatch] Abbruch Versuch 1/2: Claude endete mit Exit-Code 1.' }));
    expect(v).toMatchObject({ tone: 'error', title: 'Lauf abgebrochen (Versuch 1/2)', error: 'Claude endete mit Exit-Code 1.', highlight: true });
  });

  it('zeigt Start mit Profil, Agent, Branch und Grund', () => {
    const v = timelineView(ev({
      kind: 'run', type: 'dispatch:start', actor: agent, ref: { profile: 'schwer' },
      text: '[dispatch] Start: an junior-ARC-0003 übergeben – Branch ticket/ARC-0003-x, Profil schwer (Modell opus). Grund: Regel Plan > 10000.',
    }));
    expect(v).toMatchObject({ tone: 'info', title: 'Lauf gestartet – Profil schwer', detail: 'Regel Plan > 10000.' });
    expect(v.meta).toEqual(['dispatcher · Agent', 'an junior-ARC-0003', 'Branch ticket/ARC-0003-x']);
  });

  it('zeigt Merge grün mit Ziel und Commit', () => {
    expect(timelineView(ev({ kind: 'git', type: 'merge', actor: agent, text: '[dispatch] Gemergt in main (abc1234).', ref: { commit: 'abc1234' } })))
      .toMatchObject({ tone: 'ok', title: 'Gemergt in main', meta: ['dispatcher · Agent', 'Commit abc1234'] });
  });

  it('zeigt Human Interaction als Warten, anderen Blockiert-Grund als Fehler', () => {
    expect(timelineView(ev({ field: 'blockedReason', type: 'field:blockedReason', from: null, to: 'Human Interaction: ansehen' }))).toMatchObject({ tone: 'warn', title: 'Wartet auf Abnahme', warning: 'Human Interaction: ansehen' });
    expect(timelineView(ev({ field: 'blockedReason', type: 'field:blockedReason', from: null, to: 'DB fehlt' }))).toMatchObject({ tone: 'error', title: 'Blockiert' });
  });

  it('beschreibt die Änderung des Zielbranches mit deutschem Feldnamen', () => {
    expect(timelineView(ev({ field: 'targetBranch', type: 'field:targetBranch', from: null, to: 'release/1' }))).toMatchObject({ title: 'Zielbranch: – → release/1' });
  });

  it('beschreibt lange Texte, Listen und Laufende', () => {
    expect(timelineView(ev({ field: 'plan', type: 'field:plan', from: '0 Zeichen', to: '120 Zeichen' }))).toMatchObject({ title: 'Plan geschrieben' });
    expect(timelineView(ev({ field: 'labels', type: 'field:labels', from: '[]', to: '["auto","ui"]' }))).toMatchObject({ title: 'Labels: auto, ui' });
    expect(timelineView(ev({ kind: 'run', type: 'run-end', actor: agent, text: 'Umsetzung beendet: Exit-Code 1', ref: { exitCode: 1, runLog: 'a.log' } })))
      .toMatchObject({ tone: 'error', title: 'Umsetzung beendet', error: 'Exit-Code 1' });
    const ok = timelineView(ev({ kind: 'run', type: 'run-end', actor: agent, text: 'Umsetzung beendet: Exit-Code 0', ref: { exitCode: 0 } }));
    expect(ok.tone).toBe('ok');
    expect(ok.error).toBeUndefined();
  });

  it('zeigt eine Entscheidung grün mit Link auf die Frage', () => {
    expect(timelineView(ev({ kind: 'question', type: 'question:decided', text: 'ARC-F0001 „Speicher?“: Antwort SQL', ref: { questionId: 'ARC-F0001' } })))
      .toMatchObject({ tone: 'ok', title: 'Frage entschieden: ARC-F0001 „Speicher?“', detail: 'Antwort SQL', questionId: 'ARC-F0001', highlight: true });
  });
});

describe('dayLabel', () => {
  const now = new Date(2026, 9, 19, 15, 0);
  it('nennt Heute, Gestern und sonst Tag und Monat (Jahr nur, wenn anders)', () => {
    expect(dayLabel(new Date(2026, 9, 19, 8, 0).toISOString(), now)).toBe('Heute');
    expect(dayLabel(new Date(2026, 9, 18, 23, 0).toISOString(), now)).toBe('Gestern');
    expect(dayLabel(new Date(2026, 9, 5, 9, 0).toISOString(), now)).toBe('5. Oktober');
    expect(dayLabel(new Date(2025, 11, 31, 9, 0).toISOString(), now)).toBe('31. Dezember 2025');
  });
});
