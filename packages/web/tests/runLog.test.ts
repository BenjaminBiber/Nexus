import { describe, expect, it } from 'vitest';
import { compactRunHeader } from '../src/dbboard/runLog.js';

describe('runLog', () => {
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
