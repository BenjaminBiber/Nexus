import { describe, expect, it } from 'vitest';
import { StreamEventFormatter } from '../../../src/dispatch/StreamEventFormatter.js';

const ev = (value: unknown) => JSON.stringify(value);
const assistant = (...content: unknown[]) => ev({ type: 'assistant', message: { content } });
const user = (...content: unknown[]) => ev({ type: 'user', message: { content } });

describe('StreamEventFormatter', () => {
  const f = new StreamEventFormatter();

  it('beschreibt Start, Text, Werkzeuge, Fehler und Ergebnis', () => {
    expect(f.describe(ev({ type: 'system', subtype: 'init', model: 'claude-opus' }))).toEqual(['Claude startet (claude-opus)']);
    expect(f.describe(ev({ type: 'system', subtype: 'hook' }))).toEqual([]);
    expect(f.describe(assistant({ type: 'text', text: 'Ich lese\n  zuerst  das Ticket.' }))).toEqual(['Claude: Ich lese zuerst das Ticket.']);
    expect(f.describe(assistant({ type: 'text', text: 'x'.repeat(500) }))[0]).toHaveLength('Claude: '.length + 400);
    expect(f.describe(assistant(
      { type: 'tool_use', name: 'Read', input: { file_path: 'src/a.ts' } },
      { type: 'tool_use', name: 'Edit', input: { file_path: 'src/b.ts' } },
      { type: 'tool_use', name: 'Write', input: { file_path: 'src/c.ts' } },
      { type: 'tool_use', name: 'Bash', input: { command: 'npm test' } },
      { type: 'tool_use', name: 'Grep', input: { pattern: 'TODO', path: 'src' } },
      { type: 'tool_use', name: 'Glob', input: { pattern: '**/*.ts' } },
      { type: 'tool_use', name: 'WebFetch', input: { url: 'x' } },
      { type: 'thinking', thinking: 'still' },
    ))).toEqual(['liest src/a.ts', 'ändert src/b.ts', 'schreibt src/c.ts', 'Tests (alle)', 'sucht „TODO“ in src', 'sucht Dateien **/*.ts', 'WebFetch']);
    expect(f.describe(user(
      { type: 'tool_result', is_error: true, content: 'Datei fehlt' },
      { type: 'tool_result', is_error: true, content: [{ type: 'text', text: 'Befehl' }, { type: 'text', text: 'abgelehnt' }] },
      { type: 'tool_result', content: 'ok' },
    ))).toEqual(['  Fehler: Datei fehlt', '  Fehler: Befehl abgelehnt']);
    expect(f.describe(ev({ type: 'result', subtype: 'success', num_turns: 12, duration_ms: 185_000 }))).toEqual(['fertig, 12 Schritte, 3 min']);
    expect(f.describe(ev({ type: 'result', subtype: 'error_max_turns' }))).toEqual(['beendet (error_max_turns)']);
  });

  it('liefert Einträge mit Art, Kurzfassung, genauem Aufruf und zugeordnetem Ergebnis', () => {
    const g = new StreamEventFormatter({ root: 'C:/wt/AIH-1' });
    expect(g.entries(assistant({ type: 'tool_use', id: 't1', name: 'Bash', input: { command: 'cd /c/wt/AIH-1/packages/web && npx vitest run tests/A.test.tsx', description: 'Web-Tests' } })))
      .toEqual([{ kind: 'test', ref: 't1', text: 'Web-Tests', detail: '$ cd /c/wt/AIH-1/packages/web && npx vitest run tests/A.test.tsx' }]);
    expect(g.entries(user({ type: 'tool_result', tool_use_id: 't1', is_error: true, content: 'Test Files  1 failed\n      Tests  2 failed | 40 passed (42)\nExit code 1' })))
      .toEqual([{ kind: 'fail', ref: 't1', text: '2 fehlgeschlagen, 40 bestanden', detail: 'Test Files  1 failed\n      Tests  2 failed | 40 passed (42)\nExit code 1' }]);
    expect(g.entries(assistant({ type: 'tool_use', id: 't2', name: 'Read', input: { file_path: 'C:\\wt\\AIH-1\\src\\a.ts', offset: 10, limit: 5 } })))
      .toEqual([{ kind: 'read', ref: 't2', text: 'liest src\\a.ts', detail: 'Read C:\\wt\\AIH-1\\src\\a.ts (Zeilen 10–14)' }]);
    expect(g.entries(user({ type: 'tool_result', tool_use_id: 't2', content: 'inhalt' }))).toEqual([{ kind: 'ok', ref: 't2', text: '' }]);
  });

  it('lässt unbekannte Ereignisse still weg und übernimmt Nicht-JSON unverändert', () => {
    expect(f.describe(ev({ type: 'stream_event' }))).toEqual([]);
    expect(f.describe('Fehler: kein Netz')).toBeNull();
    expect(f.describe('{kaputt')).toBeNull();
    expect(f.format(`Hinweis\n${ev({ type: 'rate_limit_event' })}\n${assistant({ type: 'text', text: 'Hallo' })}\n`)).toEqual(['Hinweis', 'Claude: Hallo']);
    expect(f.isEvent(ev({ type: 'assistant' }))).toBe(true);
    expect(f.isEvent('{"a":1}')).toBe(false);
    expect(f.isEvent('Text')).toBe(false);
  });

  it('setzt Teilzeilen über Blöcke hinweg zusammen', () => {
    const lines = new StreamEventFormatter();
    const text = `${assistant({ type: 'text', text: 'Hallo' })}\r\n${assistant({ type: 'tool_use', name: 'Bash', input: { command: 'ls' } })}\nRest`;
    const out: string[] = [];
    for (let i = 0; i < text.length; i += 7) out.push(...lines.push(text.slice(i, i + 7)).map((e) => e.text));
    expect(out).toEqual(['Hallo', 'prüft Dateien']);
    expect(lines.flush()).toEqual([{ kind: 'raw', text: 'Rest' }]);
    expect(lines.flush()).toEqual([]);
  });

  it('stempelt Protokollzeilen mit Uhrzeit, Art und genauem Aufruf', () => {
    const at = new Date(2026, 9, 16, 9, 5, 7);
    expect(f.stamped('Claude: Hallo', at)).toBe('09:05:07  Claude: Hallo\n');
    expect(f.stamped({ kind: 'raw', text: 'stderr' }, at)).toBe('09:05:07  stderr\n');
    expect(f.stamped({ kind: 'run', ref: 't1', text: 'git status', detail: '$ git status\n' }, at)).toBe('09:05:07  [run#t1] git status\t"$ git status\\n"\n');
    expect(f.stamped({ kind: 'ok', ref: 't1', text: '' }, at)).toBe('09:05:07  [ok#t1] \n');
  });
});
