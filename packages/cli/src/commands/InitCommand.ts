import type { Command } from 'commander';
import { ClaudeImportApproval, HarnessError, HarnessProject, ProjectPluginSwitch, ProjectRules, type ApprovalResult } from '@harness/core';
import { BoardRuntime } from '@harness/db';
import type { CliContext } from '../CliContext.js';

// Längstens so lange wartet init auf die Board-Datenbank
const BOARD_TIMEOUT_MS = 3000;

interface BoardInfo {
  board: { id: number; name: string } | null;
  hint: string | null;
}

// `harness init`: richtet das Projekt einmalig ein (Token, Zähler, Plugin) und nennt das Board des Repos (wiederholbar).
export class InitCommand {
  constructor(private readonly context: CliContext) {}

  // Meldet den Befehl beim Programm an
  register(program: Command): void {
    program
      .command('init')
      .description('Richtet den Harness im aktuellen Git-Projekt ein (einmal pro Projekt, wiederholbar) und nennt das zugeordnete Board.')
      .addHelpText('after', '\nBeispiel:\n  harness init')
      .action(() => this.run());
  }

  // Legt Ordner, Zähler und Token an, schaltet das Plugin für dieses Projekt ein, legt die Rules für Doku- und
  // Board-Regeln an (fehlende werden bei Wiederholung ergänzt), gibt deren Import in Claude Code frei und nennt Harness-Ordner und Board
  private async run(): Promise<void> {
    const project = await HarnessProject.init(this.context.projectDir());
    const switched = await new ProjectPluginSwitch().enable(project.root);
    const rules = new ProjectRules();
    const written = await rules.ensure(project.root);
    const approval = await new ClaudeImportApproval().approve(project.root);
    const info = await this.board(project.root);
    const lines = [`Harness eingerichtet: ${project.layout.dir}`];
    if (switched) {
      lines.push('Plugin für dieses Projekt eingeschaltet. Claude Code neu starten oder /reload-plugins eingeben, dann laufen die Hooks.');
    }
    if (written.includes(rules.docs)) {
      lines.push(`Doku-Regeln eingebunden: ${rules.docs.path} (nicht versioniert) importiert ${rules.docs.importTarget()}.`);
    }
    if (written.includes(rules.board)) {
      lines.push(`Board-Regel eingebunden: ${rules.board.path} (nicht versioniert, immer geladen) importiert ${rules.board.importTarget()}.`);
    }
    lines.push(...this.approvalLines(approval));
    lines.push(info.board ? `Board dieses Repos: ${info.board.id} ${info.board.name}` : info.hint ?? '');
    this.context.printer().result(
      {
        dir: project.layout.dir, pluginEnabled: switched, docsRule: rules.docs.file(project.root), boardRule: rules.board.file(project.root), importApproval: approval,
        board: info.board, boardHint: info.hint,
      },
      lines.filter(Boolean),
    );
  }

  // Ausgabe zur Freigabe des Imports; bei unveränderter Freigabe nichts
  private approvalLines(approval: ApprovalResult): string[] {
    if (approval.status === 'approved') return [`Import der Doku- und Board-Regeln in Claude Code freigegeben: ${approval.file}`];
    if (approval.status === 'no-config') {
      return [`Claude Code lief auf diesem Rechner noch nie (${approval.file} fehlt). Nach dem ersten Start bitte \`harness init\` wiederholen, sonst laden die Doku- und Board-Regeln nicht.`];
    }
    return [];
  }

  // Board des Repos über den eigenen Repo-Pfad (board-repos.yaml, sonst Altbestand), nach bestem Bemühen: Fehler und Zeitüberschreitung werden zum Hinweis
  private async board(root: string): Promise<BoardInfo> {
    let timer: NodeJS.Timeout | undefined;
    const timeout = new Promise<BoardInfo>((resolve) => {
      timer = setTimeout(() => resolve({ board: null, hint: `Board-Datenbank antwortet nicht innerhalb von ${BOARD_TIMEOUT_MS / 1000} s; das Board dieses Repos zeigt „harness board boards list“.` }), BOARD_TIMEOUT_MS);
    });
    try {
      return await Promise.race([this.lookup(root), timeout]);
    } finally {
      clearTimeout(timer);
    }
  }

  // Sucht das Board; jeder Fehler ergibt einen Hinweis statt eines Abbruchs
  private async lookup(root: string): Promise<BoardInfo> {
    let runtime: BoardRuntime;
    try {
      runtime = await BoardRuntime.open(this.context.home(), { connectionTimeoutMs: BOARD_TIMEOUT_MS });
    } catch (error) {
      const missing = error instanceof HarnessError && error.code === 'board-config-missing';
      return {
        board: null,
        hint: missing
          ? 'Board-Datenbank ist noch nicht eingerichtet: mit „harness board“ die Oberfläche öffnen, Einstellungen → Board-Datenbank.'
          : `Board-Datenbank nicht verfügbar: ${(error as Error).message}`,
      };
    }
    try {
      const found = await runtime.services.boards.forRepo(root, runtime.config);
      return found
        ? { board: { id: found.id, name: found.name }, hint: null }
        : { board: null, hint: `Diesem Repo ist kein Board zugeordnet. Mit „harness board boards repo <id> ${root}“ zuordnen.` };
    } catch (error) {
      return { board: null, hint: `Board-Datenbank nicht verfügbar: ${(error as Error).message}` };
    } finally {
      await runtime.close().catch(() => undefined);
    }
  }
}
