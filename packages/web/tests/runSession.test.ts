import { describe, expect, it } from 'vitest';
import type { TranscriptEntry } from '../src/api/sessionDetailTypes.js';
import { isRunFrameEntry, runChips, runSessionEnd, runStartLine, splitRunResult } from '../src/dbboard/runSession.js';

const AT = '2026-11-12T12:00:00.000Z';
const text = (id: string, value: string): TranscriptEntry => ({ kind: 'text', id, at: AT, text: value });
const tool = (id: string): TranscriptEntry => ({ kind: 'tool', id, at: AT, name: 'Bash', summary: id, input: null, result: null });
const system = (id: string, value: string): TranscriptEntry => ({ kind: 'system', id, at: AT, text: value, level: 'info' });

const HEADER = [
  '[dispatch] 2026-11-12T11:59:32.292Z junior-AIH-0211 startet in C:/wt/AIH-0211 (Umsetzung)',
  '[dispatch] Profil standard: Modell opus, Effort low, 150 Schritte, 90 min (Label profil:standard)',
  '[dispatch] claude -p --output-format stream-json --verbose --max-turns 150',
].join('\n');

const HOOK = "SessionEnd hook [${CLAUDE_PLUGIN_ROOT}/bin/nexus hook session-end] failed: Error occurred while executing hook command: ENOENT: no such file or directory, uv_spawn 'C:\\nexus/bin/nexus'";
const BODY = [
  '12:59:34  [start] Claude startet (claude-opus-5-5)',
  '12:59:35  [run#t1] Show ticket\t"$ nexus board show AIH-0211"',
  '12:59:37  [ok#t1] ',
  '13:10:15  [say] AIH-0211 ist umgesetzt.',
  '13:10:15  [done] fertig, 27 Schritte, 11 min',
  `13:10:15  ${HOOK}`,
  '',
  '[dispatch] 2026-11-12T12:10:15.892Z Ende: Exit-Code 0',
].join('\n');

describe('runChips', () => {
  it('macht aus der Profilzeile einzelne Angaben: Profil, Modell, Effort, Limit und den Grund der Wahl', () => {
    expect(runChips(HEADER)).toEqual(['Profil standard', 'Modell opus', 'Effort low', 'Limit 150 Schritte · 90 min', 'profil:standard']);
  });

  it('nennt jeden Teil des Grunds für sich, ohne Klammern', () => {
    const header = '[dispatch] Profil schwer: Modell opus, Effort high, 250 Schritte, 120 min (Label profil:klein, Versuch 2: eskaliert auf schwer)';
    expect(runChips(header)).toEqual(['Profil schwer', 'Modell opus', 'Effort high', 'Limit 250 Schritte · 120 min', 'profil:klein', 'Versuch 2: eskaliert auf schwer']);
  });

  it('kommt mit einer kürzeren Profilzeile und weiteren Zeilen zurecht', () => {
    const header = ['[dispatch] Profil standard: Modell opus, Effort low', '[dispatch] Account arbeit'].join('\r\n');
    expect(runChips(header)).toEqual(['Profil standard', 'Modell opus', 'Effort low', 'Account arbeit']);
  });

  it('hat ohne Profilzeile nichts zu zeigen', () => {
    expect(runChips('[dispatch] 2026-10-21T12:12:10.282Z junior-ARC-0001 startet in C:/wt (Umsetzung)')).toEqual([]);
    expect(runChips('')).toEqual([]);
  });
});

describe('runStartLine', () => {
  it('nimmt die Startzeile aus dem Protokoll, sonst das Modell des Laufs', () => {
    expect(runStartLine(BODY, 'claude-sonnet-5-5')).toBe('Claude startet (claude-opus-5-5)');
    expect(runStartLine('…13:10:15  [done] fertig', 'claude-sonnet-5-5')).toBe('Claude startet (claude-sonnet-5-5)');
    // Ältere Protokolle ohne Marken
    expect(runStartLine('12:00:01  Claude startet (opus)\n', null)).toBe('Claude startet (opus)');
    expect(runStartLine('', null)).toBeNull();
  });
});

describe('runSessionEnd', () => {
  it('liest Ergebnis, Schritte, Dauer, Exit-Code und Hook-Fehler am Sitzungsende', () => {
    expect(runSessionEnd(BODY)).toEqual({
      status: { label: 'fertig', ok: true },
      facts: ['27 Schritte', '11 min'],
      exit: { label: 'Exit-Code 0', ok: true },
      hooks: [{
        title: 'SessionEnd-Hook fehlgeschlagen',
        detail: "[${CLAUDE_PLUGIN_ROOT}/bin/nexus hook session-end] Error occurred while executing hook command: ENOENT: no such file or directory, uv_spawn 'C:\\nexus/bin/nexus'",
      }],
    });
  });

  it('meldet einen Abbruch, Zeitlimit und Exit-Code ungleich 0 als Fehler und nennt doppelte Hook-Fehler einmal', () => {
    const body = [
      '10:00:00  [abort] beendet (error_max_turns), 150 Schritte, 90 min',
      `10:00:01  ${HOOK}`,
      `10:00:01  ${HOOK}`,
      '[dispatch] 2026-11-12T09:00:02.000Z Ende: Exit-Code 1, Zeitlimit',
    ].join('\n');
    const end = runSessionEnd(body);
    expect(end.status).toEqual({ label: 'beendet (error_max_turns)', ok: false });
    expect(end.facts).toEqual(['150 Schritte', '90 min']);
    expect(end.exit).toEqual({ label: 'Exit-Code 1 · Zeitlimit', ok: false });
    expect(end.hooks).toHaveLength(1);
  });

  it('hat bei einem laufenden Lauf nichts zu melden; Hook-Fehler mitten im Lauf gehören nicht zum Ende', () => {
    const body = ['09:00:01  [start] Claude startet (opus)', '09:00:02  PreToolUse hook [x] failed: kaputt', '09:00:03  [say] Weiter.'].join('\n');
    expect(runSessionEnd(body)).toEqual({ status: null, facts: [], exit: null, hooks: [] });
  });
});

describe('isRunFrameEntry', () => {
  it('erkennt Start, Ende und Hook-Fehler am Sitzungsende – die stehen nicht im Verlauf', () => {
    expect(isRunFrameEntry(system('a', 'Claude startet (opus)'))).toBe(true);
    expect(isRunFrameEntry(system('b', 'fertig, 27 Schritte, 11 min'))).toBe(true);
    expect(isRunFrameEntry(system('c', 'beendet (error_max_turns), 150 Schritte, 90 min'))).toBe(true);
    expect(isRunFrameEntry(system('d', 'Ende: Exit-Code 0'))).toBe(true);
    expect(isRunFrameEntry(system('e', HOOK))).toBe(true);
    expect(isRunFrameEntry(system('f', 'Fehler: Worktree fehlt'))).toBe(false);
    expect(isRunFrameEntry(system('g', 'PreToolUse hook [x] failed: kaputt'))).toBe(false);
    expect(isRunFrameEntry(text('h', 'fertig, 27 Schritte, 11 min'))).toBe(false);
  });
});

describe('splitRunResult', () => {
  it('nimmt bei einem beendeten Lauf den letzten Text als Ergebnis aus dem Verlauf', () => {
    const entries = [text('a', 'Ich lese.'), tool('b'), text('c', 'Fertig: alles grün.'), system('d', 'Ende: Exit-Code 0')];
    const split = splitRunResult(entries, false);
    expect(split.result).toBe('Fertig: alles grün.');
    expect(split.entries.map((e) => e.id)).toEqual(['a', 'b', 'd']);
  });

  it('lässt den Verlauf, solange der Lauf läuft oder er mit einem Aufruf endet', () => {
    const entries = [text('a', 'Ich lese.'), tool('b')];
    expect(splitRunResult(entries, false)).toEqual({ entries, result: null });
    const running = [tool('b'), text('c', 'Zwischenstand')];
    expect(splitRunResult(running, true)).toEqual({ entries: running, result: null });
  });
});
