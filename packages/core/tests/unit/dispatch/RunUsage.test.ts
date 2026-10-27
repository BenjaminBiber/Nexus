import { describe, expect, it } from 'vitest';
import { RunOutputReader } from '../../../src/dispatch/RunOutputReader.js';

const reader = new RunOutputReader();

// Gekürzte echte Ergebniszeilen aus Lauf-Protokollen vom 15.10. (ARC-0011 mit einem Modell, ARC-0010 mit Opus und Haiku)
const single = JSON.stringify({
  type: 'result', subtype: 'success', is_error: false, duration_ms: 241000, num_turns: 17, result: 'fertig',
  total_cost_usd: 0.2369144,
  usage: { input_tokens: 18, cache_creation_input_tokens: 46039, cache_read_input_tokens: 349024, output_tokens: 1782 },
  modelUsage: {
    'claude-sonnet-5-5': { inputTokens: 18, outputTokens: 1782, cacheReadInputTokens: 349024, cacheCreationInputTokens: 46039, costUSD: 0.2369144 },
  },
});
const mixed = JSON.stringify({
  type: 'result', subtype: 'success', is_error: false, duration_ms: 60000, num_turns: 9, result: 'ok',
  total_cost_usd: 0.369613015,
  usage: { input_tokens: 14, cache_creation_input_tokens: 31384, cache_read_input_tokens: 275494, output_tokens: 2939 },
  modelUsage: {
    'claude-haiku-5-5': { inputTokens: 6, outputTokens: 830, cacheReadInputTokens: 54024, cacheCreationInputTokens: 29203, costUSD: 0.004606215 },
    'claude-opus-5-5': { inputTokens: 14, outputTokens: 2939, cacheReadInputTokens: 275494, cacheCreationInputTokens: 31384, costUSD: 0.3650068 },
  },
});

describe('RunOutputReader.usage', () => {
  it('liest Kosten, Tokens, Schritte, Dauer und Modell aus dem Ergebnis-Ereignis', () => {
    const out = [JSON.stringify({ type: 'system', subtype: 'init', model: 'sonnet' }), single].join('\n');
    expect(reader.usage(out)).toEqual({
      costUsd: 0.2369144, inputTokens: 18, outputTokens: 1782, cacheReadTokens: 349024, cacheWriteTokens: 46039,
      numTurns: 17, durationMs: 241000, model: 'claude-sonnet-5-5',
      modelUsage: { 'claude-sonnet-5-5': { inputTokens: 18, outputTokens: 1782, cacheReadTokens: 349024, cacheWriteTokens: 46039, costUsd: 0.2369144 } },
    });
  });

  it('nimmt als Hauptmodell das teuerste aus modelUsage', () => {
    const usage = reader.usage(mixed)!;
    expect(usage.model).toBe('claude-opus-5-5');
    expect(usage.costUsd).toBe(0.369613015);
    expect(Object.keys(usage.modelUsage)).toEqual(['claude-haiku-5-5', 'claude-opus-5-5']);
  });

  it('liefert null ohne Ergebnis-Ereignis oder bei kaputtem JSON', () => {
    expect(reader.usage('')).toBeNull();
    expect(reader.usage(JSON.stringify({ type: 'assistant', message: { content: [] } }))).toBeNull();
    expect(reader.usage('{"type":"result", kaputt')).toBeNull();
  });

  it('setzt fehlende Felder auf null (auch bei is_error)', () => {
    const usage = reader.usage(JSON.stringify({ type: 'result', subtype: 'error_during_execution', is_error: true }))!;
    expect(usage).toEqual({
      costUsd: null, inputTokens: null, outputTokens: null, cacheReadTokens: null, cacheWriteTokens: null,
      numTurns: null, durationMs: null, model: null, modelUsage: {},
    });
  });
});
