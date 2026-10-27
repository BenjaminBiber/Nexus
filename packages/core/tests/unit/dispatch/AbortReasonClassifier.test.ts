import { describe, expect, it } from 'vitest';
import { AbortReasonClassifier } from '../../../src/dispatch/AbortReasonClassifier.js';
import { RunOutputReader } from '../../../src/dispatch/RunOutputReader.js';

const reader = new RunOutputReader();
const classifier = new AbortReasonClassifier();
const run = { profile: 'standard', why: '', model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 90 };
const result = (over: Partial<{ code: number | null; timedOut: boolean; output: string }>) => ({ code: 0, timedOut: false, output: '', ...over });
// Grund so, wie der RunOutputReader ihn formuliert (nicht abgetippt)
const reason = (r: ReturnType<typeof result>) => reader.abortReason(r, reader.summarize(r.output), run);

describe('AbortReasonClassifier', () => {
  it('ordnet die Gründe des RunOutputReader genau einer Kategorie zu', () => {
    expect(classifier.category(reason(result({ timedOut: true, code: null })))).toBe('timeout');
    expect(classifier.category(reason(result({ output: JSON.stringify({ type: 'result', subtype: 'error_max_turns', is_error: true }) })))).toBe('max-turns');
    expect(classifier.category(reason(result({ code: 1, output: JSON.stringify({ type: 'result', subtype: 'error_during_execution', is_error: true, result: 'Prompt is too long' }) })))).toBe('context');
    expect(classifier.category(reason(result({ code: 2 })))).toBe('exit-code');
    expect(classifier.category(reason(result({ output: JSON.stringify({ type: 'result', subtype: 'error_during_execution', is_error: true }) })))).toBe('claude-error');
    expect(classifier.category(reason(result({})))).toBe('no-outcome');
  });

  it('erkennt abgelehnte Werkzeugaufrufe nicht als eigene Kategorie, sondern am Grund davor', () => {
    const out = JSON.stringify({ type: 'result', subtype: 'success', is_error: false, permission_denials: [{ tool_name: 'Bash' }] });
    expect(classifier.category(reason(result({ output: out })))).toBe('no-outcome');
  });

  it('kennt die Gründe von ResultHandler, TicketStarter und DispatchService', () => {
    expect(classifier.category('Nutzungslimit zum 3. Mal in Folge')).toBe('usage-limit');
    expect(classifier.category('Worker konnte nicht gestartet werden: git-bash-missing')).toBe('start-failed');
    expect(classifier.category('Worker lieferte 10 min nach dem Zeitlimit noch kein Ergebnis (Prozess hängt oder Prozess-ID neu vergeben)')).toBe('vanished');
    expect(classifier.category('Worker-Prozess endete ohne Ergebnis (abgestürzt oder Rechner neu gestartet)')).toBe('vanished');
    expect(classifier.category('Kein Lebenszeichen seit mehr als 30 min (Worker vermutlich abgestürzt oder Rechner neu gestartet)')).toBe('vanished');
  });

  it('liefert other für Unbekanntes und ein Label je Kategorie', () => {
    expect(classifier.category('irgendwas')).toBe('other');
    for (const c of AbortReasonClassifier.CATEGORIES) expect(classifier.label(c)).toMatch(/\S/);
  });

  it('kennt die Kategorie „Kontext voll“', () => {
    expect(AbortReasonClassifier.CATEGORIES).toContain('context');
    expect(classifier.category('Kontext voll: Prompt is too long')).toBe('context');
    expect(classifier.label('context')).toBe('Kontext voll');
  });
});
