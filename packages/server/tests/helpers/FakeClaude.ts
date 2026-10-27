import { writeFileSync } from 'node:fs';
import { join } from 'node:path';

const SCRIPT = `
let input = '';
process.stdin.on('data', (d) => { input += d; });
process.stdin.on('end', () => {
  const mode = process.env.FAKE_CLAUDE_MODE || 'ok';
  if (mode === 'sleep') { setInterval(() => {}, 1000); return; }
  if (mode === 'fail') { process.stderr.write('kaputt'); process.exit(3); }
  if (mode === 'stream') {
    // Wie --output-format stream-json: je Ereignis eine Zeile, hier absichtlich mitten in der Zeile geteilt
    const events = [
      { type: 'system', subtype: 'init', model: 'fake-opus' },
      { type: 'assistant', message: { content: [{ type: 'text', text: 'Ich lese das Ticket.' }, { type: 'tool_use', name: 'Bash', input: { command: 'harness board show ' + process.env.HARNESS_TICKET } }] } },
      { type: 'user', message: { content: [{ type: 'tool_result', is_error: true, content: 'abgelehnt' }] } },
      { type: 'result', subtype: 'success', is_error: false, num_turns: 3, duration_ms: 60000, result: 'erledigt', permission_denials: [{ tool_name: 'Bash', tool_input: { command: 'git push' } }] },
    ].map((e) => JSON.stringify(e) + '\\n').join('');
    const half = Math.floor(events.length / 2);
    process.stdout.write(events.slice(0, half));
    setTimeout(() => process.stdout.write(events.slice(half)), 100);
    return;
  }
  const text = mode === 'limit' ? 'Claude usage limit reached' :
    'ticket=' + process.env.HARNESS_TICKET + ' key=' + (process.env.ANTHROPIC_API_KEY || '-') + ' args=' + process.argv.slice(2).join(' ') + ' prompt=' + input;
  process.stdout.write(JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: text }));
});
`;

// Ersatz für Claude Code in Tests: liest die Anweisung von stdin und antwortet mit einem JSON-Ergebnis.
export class FakeClaude {
  // Legt Skript und Aufruf (unter Windows .cmd) im Ordner an und liefert den Befehl
  static create(dir: string): string {
    const script = join(dir, 'fake-claude.mjs');
    writeFileSync(script, SCRIPT);
    if (process.platform === 'win32') {
      const cmd = join(dir, 'fake-claude.cmd');
      writeFileSync(cmd, `@echo off\r\n"${process.execPath}" "${script}" %*\r\n`);
      return cmd;
    }
    const sh = join(dir, 'fake-claude');
    writeFileSync(sh, `#!/bin/sh\nexec "${process.execPath}" "${script}" "$@"\n`, { mode: 0o755 });
    return sh;
  }
}
