import type { Command } from 'commander';
import { HarnessError, TextSource, type BoardStepGuard, type BoardTicketDetail } from '@harness/core';
import type { CliContext } from '../../CliContext.js';
import { BoardOptionParser } from '../../board/BoardOptionParser.js';

interface GuardOptions { rev?: string; expect?: string }
interface StepOptions extends GuardOptions { undo?: boolean; by?: string }
interface AddOptions { detail?: string; detailFile?: string; at?: string; by?: string }
interface EditOptions extends GuardOptions { text?: string; detail?: string; detailFile?: string; by?: string }

// Hilfetexte der Schutz-Optionen gegen gleichzeitige Änderungen
const REV_HELP = 'nur ändern, wenn das Ticket noch diese Version hat (sonst Konflikt)';
const EXPECT_HELP = 'nur ändern, wenn der Schritt an dieser Nummer noch diesen Text hat (sonst Konflikt)';

// `harness board step …`: Schritte eines Tickets abhaken, anlegen, bearbeiten, löschen und verschieben.
// Die alte Form `harness board step <id> <nummer> [--undo]` hakt weiter ab; sie läuft, wenn das erste Argument
// kein Unterbefehl ist. Optionen von `step` gelten nur vor dem Unterbefehl (positional), damit `--by` beim
// Unterbefehl ankommt. --rev und --expect schützen optional vor gleichzeitigen Änderungen (ohne sie wie bisher).
export class BoardStepCommands {
  private readonly text = new TextSource();
  private readonly parse = new BoardOptionParser();

  constructor(private readonly context: CliContext) {}

  // Meldet `step` mit seinen Unterbefehlen unter `board` an
  register(board: Command): void {
    const step = board
      .command('step <id> <nummer>')
      .description('Hakt einen Schritt des Tickets ab (Nummer ab 1, siehe „harness board show“). Unterbefehle legen Schritte an, bearbeiten, löschen und verschieben sie.')
      .enablePositionalOptions()
      .option('--undo', 'nimmt den Haken zurück')
      .option('--by <name>', 'wer abhakt')
      .option('--rev <n>', REV_HELP)
      .option('--expect <text>', EXPECT_HELP)
      .addHelpText('after', '\nBeispiele:\n  harness board step ARC-0007 2 --by claude\n  harness board step ARC-0007 2 --undo\n  harness board step add ARC-0007 "Test schreiben" --detail "Erwartet: rot"\n  harness board step done ARC-0007 2')
      .action((id: string, nummer: string, o: StepOptions) => this.check(id, nummer, o));

    step
      .command('done <id> <nummer>')
      .description('Hakt einen Schritt ab (gleich wie „harness board step <id> <nummer>“).')
      .option('--undo', 'nimmt den Haken zurück')
      .option('--by <name>', 'wer abhakt')
      .option('--rev <n>', REV_HELP)
      .option('--expect <text>', EXPECT_HELP)
      .addHelpText('after', '\nBeispiele:\n  harness board step done ARC-0007 2 --by claude\n  harness board step done ARC-0007 2 --undo\n  harness board step done ARC-0007 2 --expect "Test schreiben"')
      .action((id: string, nummer: string, _o: StepOptions, cmd: Command) => this.check(id, nummer, cmd.optsWithGlobals<StepOptions>()));

    step
      .command('add <id> <text>')
      .description('Hängt einen Schritt an (oder fügt ihn mit --at an dieser Nummer ein); optional mit Anleitung.')
      .option('--detail <text>', 'Anleitung zum Schritt (Markdown: Befehl, erwartetes Ergebnis)')
      .option('--detail-file <datei>', 'Anleitung aus Datei („-“ = Standardeingabe)')
      .option('--at <n>', 'an dieser Nummer einfügen (ab 1; Standard: ans Ende)')
      .option('--by <name>', 'wer anlegt (z. B. claude)')
      .addHelpText('after', '\nBeispiele:\n  harness board step add ARC-0007 "Test schreiben" --detail "npm test – erwartet: rot" --by claude\n  harness board step add ARC-0007 "Migration prüfen" --at 2')
      .action((id: string, text: string, _o: AddOptions, cmd: Command) => this.context.withBoard(async (rt) => {
        const o = cmd.optsWithGlobals<AddOptions>();
        const at = o.at === undefined ? undefined : this.number(o.at);
        const detail = await rt.services.tickets.addStep(id, text, rt.actor(o.by), { detail: this.text.read(o.detail, o.detailFile), at });
        this.print(detail, `Schritt ${at ?? detail.stepsTotal} angelegt`);
      }));

    step
      .command('edit <id> <nummer>')
      .description('Ändert Text und/oder Anleitung eines Schritts; der Haken bleibt. Leere Anleitung („“) entfernt sie.')
      .option('--text <text>', 'neuer Text')
      .option('--detail <text>', 'neue Anleitung')
      .option('--detail-file <datei>', 'Anleitung aus Datei („-“ = Standardeingabe)')
      .option('--by <name>', 'wer ändert')
      .option('--rev <n>', REV_HELP)
      .option('--expect <text>', EXPECT_HELP)
      .addHelpText('after', '\nBeispiele:\n  harness board step edit ARC-0007 2 --text "Test für Login schreiben"\n  harness board step edit ARC-0007 2 --detail ""\n  harness board step edit ARC-0007 2 --text "Neu" --expect "Alt" --rev 5')
      .action((id: string, nummer: string, _o: EditOptions, cmd: Command) => this.context.withBoard(async (rt) => {
        const o = cmd.optsWithGlobals<EditOptions>();
        const number = this.number(nummer);
        const detail = await rt.services.tickets.editStep(id, number, { text: o.text, detail: this.text.read(o.detail, o.detailFile) }, rt.actor(o.by), this.guard(o));
        this.print(detail, `Schritt ${number} geändert`);
      }));

    step
      .command('rm <id> <nummer>')
      .description('Löscht einen Schritt; die folgenden rücken eine Nummer nach vorn.')
      .option('--by <name>', 'wer löscht')
      .option('--rev <n>', REV_HELP)
      .option('--expect <text>', EXPECT_HELP)
      .addHelpText('after', '\nBeispiele:\n  harness board step rm ARC-0007 3\n  harness board step rm ARC-0007 3 --expect "Alter Schritt"')
      .action((id: string, nummer: string, _o: StepOptions, cmd: Command) => this.context.withBoard(async (rt) => {
        const o = cmd.optsWithGlobals<StepOptions>();
        const number = this.number(nummer);
        this.print(await rt.services.tickets.removeStep(id, number, rt.actor(o.by), this.guard(o)), `Schritt ${number} gelöscht`);
      }));

    step
      .command('move <id> <von> <nach>')
      .description('Verschiebt einen Schritt auf eine andere Nummer (samt Haken und Anleitung).')
      .option('--by <name>', 'wer verschiebt')
      .option('--rev <n>', REV_HELP)
      .option('--expect <text>', 'nur verschieben, wenn der Schritt an Nummer <von> noch diesen Text hat (sonst Konflikt)')
      .addHelpText('after', '\nBeispiele:\n  harness board step move ARC-0007 4 1\n  harness board step move ARC-0007 4 1 --expect "Committen"')
      .action((id: string, von: string, nach: string, _o: StepOptions, cmd: Command) => this.context.withBoard(async (rt) => {
        const o = cmd.optsWithGlobals<StepOptions>();
        const from = this.number(von);
        const to = this.number(nach);
        this.print(await rt.services.tickets.moveStep(id, from, to, rt.actor(o.by), this.guard(o)), `Schritt ${from} → ${to}`);
      }));
  }

  // Hakt ab oder nimmt den Haken zurück
  private check(id: string, nummer: string, o: StepOptions): Promise<void> {
    return this.context.withBoard(async (rt) => {
      const detail = await rt.services.tickets.setStep(id, this.number(nummer), !o.undo, rt.actor(o.by), this.guard(o));
      this.context.printer().result(detail, [`${detail.id}: ${detail.stepsDone}/${detail.stepsTotal} Schritte erledigt.`]);
    });
  }

  // Schutz aus --rev/--expect; ohne beide Optionen ein leerer Schutz (Verhalten wie bisher)
  private guard(o: GuardOptions): BoardStepGuard {
    return { rev: this.parse.rev(o.rev), expectText: o.expect };
  }

  // Gibt das Ticket mit der nummerierten Schrittliste aus
  private print(detail: BoardTicketDetail, what: string): void {
    const lines = [`${detail.id}: ${what} (${detail.stepsDone}/${detail.stepsTotal} erledigt).`];
    detail.steps.forEach((s, i) => lines.push(`  ${i + 1}. [${s.done ? 'x' : ' '}] ${s.text}`));
    this.context.printer().result(detail, lines);
  }

  // Schrittnummer ab 1
  private number(value: string): number {
    const number = Number(value);
    if (!Number.isInteger(number) || number < 1) throw new HarnessError('step-invalid', `„${value}“ ist keine Schrittnummer (ab 1).`);
    return number;
  }
}
