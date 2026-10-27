import { describe, expect, it } from 'vitest';
import { RunOutputReader } from '../../../src/dispatch/RunOutputReader.js';

const reader = new RunOutputReader();
const run = { profile: 'standard', why: '', model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 90 };

describe('RunOutputReader', () => {
  it('findet das Ergebnisobjekt von --output-format json', () => {
    const out = 'Log\n{"type":"result","subtype":"error_max_turns","is_error":true,"result":"halb fertig"}';
    expect(reader.summarize(out)).toEqual({ subtype: 'error_max_turns', isError: true, text: 'halb fertig', denied: [] });
    expect(reader.summarize('kein json')).toEqual({ subtype: null, isError: false, text: 'kein json', denied: [] });
  });

  it('liest die Ausgabe von --output-format stream-json (Ergebnis-Ereignis zuletzt, abgelehnte Aufrufe, lesbarer Rest)', () => {
    const lines = [
      JSON.stringify({ type: 'system', subtype: 'init', model: 'opus' }),
      JSON.stringify({ type: 'assistant', message: { content: [{ type: 'text', text: 'Ich baue einen rate limit ein.' }] } }),
      JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: 'fertig', permission_denials: [{ tool_name: 'Bash', tool_input: { command: 'git push' } }] }),
    ];
    const out = lines.join('\n');
    expect(reader.summarize(out)).toEqual({ subtype: 'success', isError: false, text: 'fertig', denied: ['Bash: git push'] });
    // Ohne Ergebnis (z. B. Zeitlimit): die lesbaren Zeilen statt roher Ereignisse
    const cut = `${lines.slice(0, 2).join('\n')}\nstderr-Meldung`;
    expect(reader.summarize(cut).text).toBe('Claude startet (opus)\nClaude: Ich baue einen rate limit ein.\nstderr-Meldung');
    expect(reader.shown(cut, reader.summarize(cut))).toBe('Claude startet (opus)\nClaude: Ich baue einen rate limit ein.\nstderr-Meldung');
    // Leerer Ergebnistext: lesbare Zeilen statt roher Ereignisse
    const empty = [lines[1], JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: '' })].join('\n');
    expect(reader.shown(empty, reader.summarize(empty))).toBe('Claude: Ich baue einen rate limit ein.\nfertig');
  });

  it('erkennt im stream-json ein Nutzungslimit nur in stderr bzw. im Fehlerergebnis, nicht in Claudes Text', () => {
    const talk = JSON.stringify({ type: 'assistant', message: { content: [{ type: 'text', text: 'usage limit reached?' }] } });
    const ok = JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: 'ok' });
    expect(reader.hitUsageLimit({ code: 1, timedOut: false, output: `${talk}\n${ok}` })).toBe(false);
    expect(reader.hitUsageLimit({ code: 1, timedOut: false, output: talk })).toBe(false);
    expect(reader.hitUsageLimit({ code: 1, timedOut: false, output: `${talk}\nClaude AI usage limit reached|1760000000` })).toBe(true);
    const error = JSON.stringify({ type: 'result', subtype: 'error_during_execution', is_error: true, result: 'Usage limit reached' });
    expect(reader.hitUsageLimit({ code: 1, timedOut: false, output: `${talk}\n${error}` })).toBe(true);
  });

  it('nennt den Abbruchgrund in fester Reihenfolge', () => {
    const none = { subtype: null, isError: false, text: '', denied: [] };
    expect(reader.abortReason({ code: null, timedOut: true, output: '' }, none, run)).toBe('Zeitlimit von 90 min erreicht, Lauf abgebrochen');
    expect(reader.abortReason({ code: 1, timedOut: false, output: '' }, { ...none, subtype: 'error_max_turns' }, run)).toBe('Maximale Zahl an Schritten (80) erreicht');
    expect(reader.abortReason({ code: 2, timedOut: false, output: '' }, none, run)).toBe('Claude endete mit Exit-Code 2');
    expect(reader.abortReason({ code: 0, timedOut: false, output: '' }, { ...none, isError: true, subtype: 'error_x' }, run)).toBe('Claude meldet einen Fehler (error_x)');
    expect(reader.abortReason({ code: 0, timedOut: false, output: '' }, none, run)).toBe('Lauf endete ohne Abschluss – das Ticket steht weder auf Erledigt noch auf Blockiert und hat keine offene Frage');
  });

  it('erkennt ein volles Kontextfenster vor Exit-Code und Claude-Fehler, aber nach Zeit- und Schrittlimit', () => {
    const contextError = { subtype: 'error_during_execution', isError: true, text: 'Prompt is too long: 215000 tokens > 200000 maximum', denied: [] };
    expect(reader.abortReason({ code: 1, timedOut: false, output: '' }, contextError, run)).toBe('Kontext voll: Prompt is too long: 215000 tokens > 200000 maximum');
    // Zeitlimit und Schrittlimit gewinnen weiter
    expect(reader.abortReason({ code: 1, timedOut: true, output: '' }, contextError, run)).toBe('Zeitlimit von 90 min erreicht, Lauf abgebrochen');
    expect(reader.abortReason({ code: 1, timedOut: false, output: '' }, { ...contextError, subtype: 'error_max_turns' }, run)).toBe('Maximale Zahl an Schritten (80) erreicht');
    // Ein normaler Fehler ohne das Muster bleibt „Claude meldet einen Fehler“
    expect(reader.abortReason({ code: 0, timedOut: false, output: '' }, { ...contextError, text: 'irgendein Fehler' }, run)).toBe('Claude meldet einen Fehler (error_during_execution)');
  });

  it('erkennt volle Kontextfenster grob', () => {
    expect(reader.looksLikeContextOverflow('Prompt is too long')).toBe(true);
    expect(reader.looksLikeContextOverflow('context window exceeded')).toBe(true);
    expect(reader.looksLikeContextOverflow('alles gut')).toBe(false);
  });

  it('liest abgelehnte Werkzeugaufrufe (dedupliziert, höchstens 5) und hängt sie an den Abbruchgrund', () => {
    const d = (tool: string, command?: string) => ({ tool_name: tool, tool_input: command ? { command } : {} });
    const out = JSON.stringify({
      type: 'result', subtype: 'success', is_error: false, result: '',
      permission_denials: [d('PowerShell', 'harness board show ARC-0010'), d('PowerShell', 'harness board show ARC-0010'), d('Edit'), d('A', '1'), d('B', '2'), d('C', '3'), d('D', '4')],
    });
    const summary = reader.summarize(out);
    expect(summary.denied).toEqual(['PowerShell: harness board show ARC-0010', 'Edit', 'A: 1', 'B: 2', 'C: 3']);
    expect(reader.abortReason({ code: 0, timedOut: false, output: out }, { ...summary, denied: summary.denied.slice(0, 1) }, run))
      .toBe('Lauf endete ohne Abschluss – das Ticket steht weder auf Erledigt noch auf Blockiert und hat keine offene Frage; abgelehnte Werkzeugaufrufe: PowerShell: harness board show ARC-0010 (erlaubte Tools in den Dispatcher-Einstellungen prüfen)');
  });

  it('erkennt Nutzungslimits grob', () => {
    expect(reader.looksLikeUsageLimit('Claude usage limit reached')).toBe(true);
    expect(reader.looksLikeUsageLimit('rate-limit hit')).toBe(true);
    expect(reader.looksLikeUsageLimit('alles gut')).toBe(false);
  });

  it('erkennt die Limit-Meldungen neuerer Claude-Code-Versionen (Sitzung, Woche, Modell)', () => {
    expect(reader.looksLikeUsageLimit("You've hit your session limit · resets 12:10pm (Europe/Berlin)")).toBe(true);
    expect(reader.looksLikeUsageLimit("You've hit your weekly limit · resets Oct 23, 4pm")).toBe(true);
    expect(reader.looksLikeUsageLimit("You've hit your Opus limit · resets 6pm")).toBe(true);
    expect(reader.looksLikeUsageLimit("You've hit your limit · resets 3pm")).toBe(true);
    // Wie im Protokoll: Exit-Code 1, die Meldung steht allein in der Ausgabe
    expect(reader.hitUsageLimit({ code: 1, timedOut: false, output: "You've hit your session limit · resets 12:10pm (Europe/Berlin)" })).toBe(true);
    expect(reader.looksLikeUsageLimit('Test hit the breakpoint. Speicherlimit erhöht')).toBe(false);
  });

  it('erkennt ein Nutzungslimit nur bei fehlerhaftem Ende und nur im Fehlertext', () => {
    const ok = JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: 'rate limit der API eingebaut' });
    expect(reader.hitUsageLimit({ code: 0, timedOut: false, output: ok })).toBe(false);
    // Exit ≠ 0, aber „rate limit“ steht nur im erfolgreichen Ergebnistext
    expect(reader.hitUsageLimit({ code: 1, timedOut: false, output: ok })).toBe(false);
    expect(reader.hitUsageLimit({ code: 1, timedOut: false, output: `${ok}\nClaude usage limit reached` })).toBe(true);
    expect(reader.hitUsageLimit({ code: 1, timedOut: false, output: 'Claude usage limit reached' })).toBe(true);
    const error = JSON.stringify({ type: 'result', subtype: 'error_during_execution', is_error: true, result: 'Usage limit reached' });
    expect(reader.hitUsageLimit({ code: 0, timedOut: false, output: error })).toBe(true);
    expect(reader.hitUsageLimit({ code: null, timedOut: true, output: 'usage limit' })).toBe(false);
  });

  it('erkennt fehlendes Claude Code', () => {
    expect(reader.claudeMissing({ code: 127, timedOut: false, output: 'spawn claude ENOENT' })).toBe(true);
    expect(reader.claudeMissing({ code: 1, timedOut: false, output: 'claude-not-found' })).toBe(true);
    expect(reader.claudeMissing({ code: 1, timedOut: false, output: 'x' })).toBe(false);
  });

  it('kürzt das Ende und entschärft Codezäune', () => {
    expect(reader.tail('```x```', 100)).toBe("'''x'''");
    expect(reader.tail('abcdef', 3)).toBe('…def');
  });
});
