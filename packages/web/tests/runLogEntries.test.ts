import { describe, expect, it } from 'vitest';
import { runLogEntries } from '../src/dbboard/runLogEntries.js';

// Zeiten in Ortszeit bauen, damit die Erwartungen in jeder Zeitzone stimmen
const at = (h: number, m: number, s: number, day = 16) => new Date(2026, 9, day, h, m, s).toISOString();
const STARTED = at(10, 15, 0);
const NO_OUTPUT = 'Erfolgreich – die Ausgabe steht nicht im Protokoll.';
const log = (...lines: string[]) => lines.join('\n');

describe('runLogEntries', () => {
  it('liest Start, Text und einen Aufruf mit Ergebnis', () => {
    const entries = runLogEntries(
      log(
        '10:15:02  [start] Claude startet (fake-opus)',
        '10:15:03  [say] Ich lese das Ticket.',
        '10:15:04  [read#toolu_1] liest src/a.ts\t"Read C:/work/src/a.ts"',
        '10:15:05  [ok#toolu_1] ',
      ),
      STARTED,
    );
    expect(entries).toEqual([
      { kind: 'system', id: 'log-0', at: at(10, 15, 2), text: 'Claude startet (fake-opus)', level: 'info' },
      { kind: 'text', id: 'log-1', at: at(10, 15, 3), text: 'Ich lese das Ticket.' },
      {
        kind: 'tool', id: 'toolu_1', at: at(10, 15, 4), name: 'Read', summary: 'src/a.ts', input: 'Read C:/work/src/a.ts',
        result: { text: NO_OUTPUT, isError: false, truncated: false },
      },
    ]);
  });

  it('nimmt für Text das Detail, wenn die Kurzfassung gekürzt ist', () => {
    const [entry] = runLogEntries('10:15:03  [say] Kurz …\t"Kurz und dann lang"', STARTED);
    expect(entry).toMatchObject({ kind: 'text', text: 'Kurz und dann lang' });
  });

  it('liest Shell-Aufrufe mit Befehl, Fehler und Testzählung', () => {
    const entries = runLogEntries(
      log(
        '10:16:00  [run#toolu_2] liest Ticket ARC-0004\t"$ nexus board show ARC-0004"',
        '10:16:01  [fail#toolu_2] abgelehnt\t"abgelehnt: keine Berechtigung"',
        '10:16:02  [test#toolu_3] führt Tests aus\t"PS> npm test"',
        '10:16:30  [ok#toolu_3] 12 Tests bestanden',
      ),
      STARTED,
    );
    expect(entries).toEqual([
      {
        kind: 'tool', id: 'toolu_2', at: at(10, 16, 0), name: 'Bash', summary: 'liest Ticket ARC-0004', input: { command: 'nexus board show ARC-0004' },
        result: { text: 'abgelehnt: keine Berechtigung', isError: true, truncated: false },
      },
      {
        kind: 'tool', id: 'toolu_3', at: at(10, 16, 2), name: 'PowerShell', summary: 'führt Tests aus', input: { command: 'npm test' },
        result: { text: '12 Tests bestanden', isError: false, truncated: false },
      },
    ]);
  });

  it('benennt die Tools nach Marke und Text', () => {
    const entries = runLogEntries(
      log(
        '10:15:10  [edit#a] ändert src/a.ts\t"Edit C:/work/src/a.ts"',
        '10:15:11  [edit#b] schreibt src/b.ts\t"Write C:/work/src/b.ts"',
        '10:15:12  [search#c] sucht „foo“ in src\t"Grep \\"foo\\" src"',
        '10:15:13  [search#d] sucht Dateien *.ts\t"Glob *.ts"',
        '10:15:14  [todo#e] plant 3 Schritte',
        '10:15:15  [tool#f] ruft eine Seite ab\t"WebFetch https://example.org"',
        '10:15:16  [tool] Sonstiges',
      ),
      STARTED,
    );
    expect(entries.map((entry) => (entry.kind === 'tool' ? [entry.id, entry.name, entry.summary, entry.result] : null))).toEqual([
      ['a', 'Edit', 'src/a.ts', null],
      ['b', 'Write', 'src/b.ts', null],
      ['c', 'Grep', 'sucht „foo“ in src', null],
      ['d', 'Glob', 'sucht Dateien *.ts', null],
      ['e', 'TodoWrite', 'plant 3 Schritte', null],
      ['f', 'WebFetch', 'ruft eine Seite ab', null],
      ['log-6', 'Sonstiges', 'Sonstiges', null],
    ]);
    expect(entries[4]).toMatchObject({ input: null });
    expect(entries[2]).toMatchObject({ input: 'Grep "foo" src' });
  });

  it('macht Fehler ohne Aufruf, Abbruch und Ende zu Systemhinweisen', () => {
    const entries = runLogEntries(
      log(
        '10:20:00  [fail] abgelehnt\t"Bash: git push abgelehnt"',
        '10:20:01  [ok#unbekannt] ',
        '10:20:02  [abort] beendet (error_max_turns)',
        '10:20:03  [done] fertig, 3 Schritte, 1 min',
      ),
      STARTED,
    );
    expect(entries).toEqual([
      { kind: 'system', id: 'log-0', at: at(10, 20, 0), text: 'Bash: git push abgelehnt', level: 'error' },
      { kind: 'system', id: 'log-2', at: at(10, 20, 2), text: 'beendet (error_max_turns)', level: 'error' },
      { kind: 'system', id: 'log-3', at: at(10, 20, 3), text: 'fertig, 3 Schritte, 1 min', level: 'info' },
    ]);
  });

  it('stuft Meldungen des Dispatchers ein und nimmt ihre eigene Zeit', () => {
    const entries = runLogEntries(
      log(
        '10:20:00  [say] Fertig.',
        '[dispatch] 2026-10-16T08:30:00.000Z Ende: Exit-Code 0',
        '[dispatch] 2026-10-16T08:30:01.000Z Ende: Exit-Code 1',
        '[dispatch] Ende: Exit-Code 0 (Zeitlimit)',
        '[dispatch] Fehler: Worktree fehlt',
        '[dispatch] Einrichtung fehlgeschlagen',
        '[dispatch] Merge beginnt',
      ),
      STARTED,
    );
    expect(entries.slice(1).map((entry) => (entry.kind === 'system' ? [entry.level, entry.at, entry.text] : null))).toEqual([
      ['info', '2026-10-16T08:30:00.000Z', 'Ende: Exit-Code 0'],
      ['error', '2026-10-16T08:30:01.000Z', 'Ende: Exit-Code 1'],
      ['error', '2026-10-16T08:30:01.000Z', 'Ende: Exit-Code 0 (Zeitlimit)'],
      ['error', '2026-10-16T08:30:01.000Z', 'Fehler: Worktree fehlt'],
      ['error', '2026-10-16T08:30:01.000Z', 'Einrichtung fehlgeschlagen'],
      ['info', '2026-10-16T08:30:01.000Z', 'Merge beginnt'],
    ]);
  });

  it('fasst Zeilen ohne Uhrzeit zu einer Ausgabe zusammen', () => {
    const entries = runLogEntries(
      log('npm warn deprecated x', '', 'added 12 packages', 'audited 13 packages', '10:15:20  [say] Los.', 'stderr: eine Zeile'),
      STARTED,
    );
    expect(entries).toEqual([
      {
        kind: 'tool', id: 'log-0', at: STARTED, name: 'Ausgabe', summary: '3 Zeilen',
        input: 'Ausgabe ohne Zeitangabe (Einrichtung des Worktrees oder Fehlerausgabe)',
        result: { text: 'npm warn deprecated x\nadded 12 packages\naudited 13 packages', isError: false, truncated: false },
      },
      { kind: 'text', id: 'log-4', at: at(10, 15, 20), text: 'Los.' },
      {
        kind: 'tool', id: 'log-5', at: at(10, 15, 20), name: 'Ausgabe', summary: '1 Zeile',
        input: 'Ausgabe ohne Zeitangabe (Einrichtung des Worktrees oder Fehlerausgabe)',
        result: { text: 'stderr: eine Zeile', isError: false, truncated: false },
      },
    ]);
  });

  it('liest ältere Zeilen mit Uhrzeit ohne Marke', () => {
    const entries = runLogEntries(log('10:15:02  Claude: Hallo', '10:15:03  fertig, 3 Schritte'), STARTED);
    expect(entries).toEqual([
      { kind: 'text', id: 'log-0', at: at(10, 15, 2), text: 'Hallo' },
      { kind: 'system', id: 'log-1', at: at(10, 15, 3), text: 'fertig, 3 Schritte', level: 'info' },
    ]);
  });

  it('zählt nach Mitternacht den nächsten Tag', () => {
    const entries = runLogEntries(log('23:59:30  [say] spät', '00:00:10  [say] früh', '00:00:05  [say] gleich danach'), at(23, 59, 0));
    expect(entries.map((entry) => entry.at)).toEqual([at(23, 59, 30), at(0, 0, 10, 17), at(0, 0, 5, 17)]);
  });

  it('übergeht leere Protokolle, Windows-Zeilenenden und den Kürzungsvermerk', () => {
    expect(runLogEntries('', STARTED)).toEqual([]);
    const entries = runLogEntries('… es ist sehr lang\r\n10:15:03  [say] Ende.\r\n', STARTED);
    expect(entries).toEqual([{ kind: 'text', id: 'log-1', at: at(10, 15, 3), text: 'Ende.' }]);
  });

  it('kommt ohne Startzeit aus', () => {
    const [entry] = runLogEntries('10:15:03  [say] Hallo', null);
    expect(entry).toMatchObject({ kind: 'text', text: 'Hallo' });
    expect(Number.isNaN(new Date(entry!.at).getTime())).toBe(false);
  });
});
