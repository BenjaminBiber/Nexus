import { describe, expect, it } from 'vitest';
import { runHeaderLines } from '../src/dbboard/runLog.js';

describe('runLog', () => {
  it('Kopf ohne Aufruf von Claude und ohne die Zeile „… startet in …“', () => {
    expect(runHeaderLines([
      '[dispatch] 2026-10-21T12:12:10.282Z junior-AIH-0043 startet in C:/wt/AIH-0043 (Umsetzung)',
      '[dispatch] Profil standard: Modell opus, Effort low, 250 Schritte, 90 min (Aufwand laut Plan: standard)',
      '[dispatch] claude -p --output-format stream-json --verbose',
    ].join('\n'))).toEqual([
      'Profil standard: Modell opus, Effort low, 250 Schritte, 90 min (Aufwand laut Plan: standard)',
    ]);
  });

  it('lässt nichts übrig, wenn der Kopf nur Start und Aufruf nennt', () => {
    expect(runHeaderLines([
      '[dispatch] 2026-10-21T12:12:10.282Z junior-AIH-0210 startet in C:\\wt\\AIH-0210 (Umsetzung)',
      '[dispatch] claude -p --output-format stream-json --verbose',
    ].join('\r\n'))).toEqual([]);
  });
});
