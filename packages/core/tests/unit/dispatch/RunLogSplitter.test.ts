import { describe, expect, it } from 'vitest';
import { RUN_LOG_OUTPUT_MARKER, RunLogSplitter } from '../../../src/dispatch/state/RunLogSplitter.js';

const HEAD = '[dispatch] 2026-10-16T07:00:00.000Z junior-ARC-0001 startet in C:/w (Umsetzung)\n[dispatch] Profil standard: …\n[dispatch] claude -p\n';

describe('RunLogSplitter', () => {
  const splitter = new RunLogSplitter();

  it('trennt Kopf, Arbeitsanweisung (mit Leerzeilen) und Ausgabe am Marker', () => {
    const text = `${HEAD}[dispatch] Arbeitsanweisung:\nDu bist junior.\n\nSetze um.\n\n${RUN_LOG_OUTPUT_MARKER}\n09:00:01  Claude startet (opus)\n\n[dispatch] Ende: Exit-Code 0\n`;
    expect(splitter.split(text)).toEqual({
      header: HEAD.trimEnd(),
      prompt: 'Du bist junior.\n\nSetze um.',
      body: '09:00:01  Claude startet (opus)\n\n[dispatch] Ende: Exit-Code 0\n',
    });
  });

  it('erkennt das Ende der Anweisung in älteren Protokollen an der ersten Ausgabe', () => {
    const json = `${HEAD}[dispatch] Arbeitsanweisung:\nTu es.\n\n{"type":"result","result":"ok"}\n[dispatch] Ende: Exit-Code 0\n`;
    expect(splitter.split(json)).toEqual({ header: HEAD.trimEnd(), prompt: 'Tu es.', body: '{"type":"result","result":"ok"}\n[dispatch] Ende: Exit-Code 0\n' });
    const running = `${HEAD}[dispatch] Arbeitsanweisung:\nTu es.\n\n`;
    expect(splitter.split(running)).toEqual({ header: HEAD.trimEnd(), prompt: 'Tu es.', body: '' });
    const stamped = `${HEAD}[dispatch] Arbeitsanweisung:\nTu es.\n\n09:00:01  Claude startet (opus)\n`;
    expect(splitter.split(stamped).body).toBe('09:00:01  Claude startet (opus)\n');
  });

  it('liefert ohne Anweisung (abgeschnittenes Ende) den ganzen Text als Ausgabe', () => {
    expect(splitter.split('…09:00:01  Claude: weiter\n')).toEqual({ header: '', prompt: null, body: '…09:00:01  Claude: weiter\n' });
  });

  it('kommt mit CRLF zurecht', () => {
    const text = `[dispatch] x\r\n[dispatch] Arbeitsanweisung:\r\nA\r\n\r\n${RUN_LOG_OUTPUT_MARKER}\r\n09:00:01  B\r\n`;
    expect(splitter.split(text)).toEqual({ header: '[dispatch] x', prompt: 'A', body: '09:00:01  B\r\n' });
  });
});
