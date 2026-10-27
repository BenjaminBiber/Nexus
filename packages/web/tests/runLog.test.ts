import { describe, expect, it } from 'vitest';
import { compactRunHeader, compactRunLog, detailedRunLog, runRoot } from '../src/dbboard/runLog.js';

const BODY = [
  '09:00:01  [start] Claude startet (opus)',
  '09:00:02  [say] Ich lese das Ticket.',
  '09:00:03  [run#t1] liest Ticket ARC-1\t"$ harness board show ARC-1"',
  '09:00:04  [ok#t1] ',
  '09:00:05  [edit#t2] ändert per Skript: a.tsx\t"$ node - <<\'EOF\'\\nfs.writeFileSync(p)\\nEOF"',
  '09:00:06  [fail#t2] Exit code 1\t"Exit code 1\\nError: miss"',
  '09:00:07  [test#t3] Tests (alle)\t"$ npm test"',
  '09:00:09  [ok#t3] 412 bestanden',
  '09:00:10  [test#t4] Tests: A.test.tsx\t"$ npx vitest run A.test.tsx"',
].join('\n');

describe('runLog', () => {
  it('kompakt: Schritte mit Farbe, Ergebnisse am Aufruf, offener Aufruf in Arbeit', () => {
    expect(compactRunLog(BODY, true)).toEqual([
      { type: 'step', time: '09:00:01', text: 'Claude startet (opus)', tone: 'ok' },
      { type: 'say', time: '09:00:02', text: 'Ich lese das Ticket.' },
      { type: 'step', time: '09:00:03', text: 'liest Ticket ARC-1', tone: 'ok' },
      { type: 'step', time: '09:00:05', text: 'ändert per Skript: a.tsx', tone: 'error', error: 'Exit code 1\nError: miss' },
      { type: 'step', time: '09:00:07', text: 'Tests (alle)', tone: 'ok', note: '412 bestanden' },
      { type: 'step', time: '09:00:10', text: 'Tests: A.test.tsx', tone: 'running' },
    ]);
    expect(compactRunLog(BODY, false).at(-1)).toMatchObject({ tone: 'error' });
  });

  it('kompakt: Dispatcher-Ende, alte Zeilen und Ausgabe ohne Uhrzeit', () => {
    const rows = compactRunLog('09:00:01  Claude startet (opus)\nnpm warn x\nnpm warn y\n[dispatch] 2026-10-21T12:00:00.000Z Ende: Exit-Code 1', false);
    expect(rows[0]).toEqual({ type: 'line', time: '09:00:01', text: 'Claude startet (opus)' });
    expect(rows[1]).toEqual({ type: 'output', lines: ['npm warn x', 'npm warn y'] });
    expect(rows[2]).toMatchObject({ type: 'step', text: 'Ende: Exit-Code 1', tone: 'error' });
  });

  it('detailliert: genaue Befehle, Fehler eingerückt, leere Erfolge weg', () => {
    expect(detailedRunLog(BODY).split('\n')).toEqual([
      '09:00:01  Claude startet (opus)',
      '09:00:02  Claude: Ich lese das Ticket.',
      '09:00:03  $ harness board show ARC-1',
      "09:00:05  $ node - <<'EOF'",
      '          fs.writeFileSync(p)',
      '          EOF',
      '            Fehler: Exit code 1',
      '            Error: miss',
      '09:00:07  $ npm test',
      '            → 412 bestanden',
      '09:00:10  $ npx vitest run A.test.tsx',
    ]);
  });

  it('detailliert: Pfade relativ zum Worktree aus dem Kopf, in jeder Schreibweise', () => {
    const root = runRoot('[dispatch] 2026-10-21T12:12:10.282Z junior-AIH-0043 startet in C:/Users/b/wt/AIH-0043 (Umsetzung)\n[dispatch] Profil standard: …');
    expect(root).toBe('C:/Users/b/wt/AIH-0043');
    const body = [
      '09:00:01  [run#a] Tests\t"$ cd /c/Users/b/wt/AIH-0043/packages/web && npx vitest run"',
      '09:00:02  [edit#b] ändert a.ts\t"Edit C:\\\\Users\\\\b\\\\wt\\\\AIH-0043\\\\src\\\\a.ts"',
      '09:00:03  $ cd /c/Users/b/wt/AIH-0043 && ls C:/Users/b/wt/AIH-00431/x',
    ].join('\n');
    expect(detailedRunLog(body, root).split('\n')).toEqual([
      '09:00:01  $ cd packages/web && npx vitest run',
      '09:00:02  Edit src\\a.ts',
      '09:00:03  $ cd . && ls C:/Users/b/wt/AIH-00431/x',
    ]);
    expect(detailedRunLog(body)).toContain('/c/Users/b/wt/AIH-0043/packages/web');
  });

  it('Kopf kompakt ohne Aufruf von Claude', () => {
    expect(compactRunHeader([
      '[dispatch] 2026-10-21T12:12:10.282Z junior-AIH-0043 startet in C:/wt/AIH-0043 (Umsetzung)',
      '[dispatch] Profil standard: Modell opus, Effort low, 250 Schritte, 90 min (Aufwand laut Plan: standard)',
      '[dispatch] claude -p --output-format stream-json --verbose',
    ].join('\n'))).toEqual([
      'junior-AIH-0043 · Umsetzung · AIH-0043',
      'Profil standard · Modell opus · Effort low · 250 Schritte · 90 min (Aufwand laut Plan: standard)',
    ]);
  });
});
