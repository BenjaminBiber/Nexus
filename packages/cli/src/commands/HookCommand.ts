import type { Command } from 'commander';
import { HOOK_KINDS, type HookKind } from '@harness/core';
import { HookRunner } from '../hooks/HookRunner.js';
import { StdinReader } from '../hooks/StdinReader.js';

const DEFAULT_DEADLINE_MS = 3000;
const BOARD_RESERVE_MS = 1000;
const MIN_BOARD_BUDGET_MS = 500;

// `harness hook <ereignis>`: wird von den Hooks des Plugins aufgerufen; blockiert nie und bleibt still.
export class HookCommand {
  // Meldet den Befehl beim Programm an
  register(program: Command): void {
    program
      .command('hook <ereignis>')
      .description(`Wird von Claude Code aufgerufen (${HOOK_KINDS.join(', ')}); liest die Hook-Meldung von der Standardeingabe.`)
      .addHelpText('after', '\nBeispiel:\n  echo \'{"session_id":"abc","cwd":"."}\' | harness hook session-start')
      .action((ereignis: string) => this.run(ereignis));
  }

  // Liest die Eingabe, führt den Hook aus und gibt höchstens den Kontext aus; endet spätestens nach der Frist (3 s)
  private async run(name: string): Promise<void> {
    const limit = Number(process.env.HARNESS_HOOK_DEADLINE_MS) || DEFAULT_DEADLINE_MS;
    const deadline = setTimeout(() => process.exit(0), limit);
    try {
      if (!HOOK_KINDS.includes(name as HookKind)) return;
      const input = await new StdinReader().read();
      let payload: unknown = null;
      try {
        payload = JSON.parse(input);
      } catch {
        return;
      }
      // Die Board-DB bekommt die Frist abzüglich einer Sekunde, damit Umgebung, Ausgabe und Meldung sicher noch durchkommen
      const boardBudgetMs = Math.max(limit - BOARD_RESERVE_MS, MIN_BOARD_BUDGET_MS);
      await new HookRunner({ boardBudgetMs }).run(name as HookKind, payload, process.env, (output) => process.stdout.write(`${output}\n`));
    } finally {
      // Nicht löschen, nur lösen: Hält eine hängende DB-Verbindung den Prozess, beendet ihn die Frist trotzdem
      deadline.unref();
    }
  }
}
