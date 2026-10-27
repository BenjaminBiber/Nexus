import type { Command } from 'commander';
import { DocCommentStore, type OpenDocComment } from '@harness/core';
import type { CliContext } from '../CliContext.js';

// `harness spec comments`: listet offene Kommentare zu Spec- und Plan-Dokumenten, Glossar und ADRs für die Einarbeitung.
export class SpecCommentsCommand {
  constructor(private readonly context: CliContext) {}

  // Meldet den Befehl in der Gruppe `spec` an
  register(spec: Command): void {
    spec
      .command('comments [dokument]')
      .description('Listet offene Kommentare zu Spec- und Plan-Dokumenten, Glossar und ADRs (optional eines Dokuments oder Themas).')
      .addHelpText('after', '\nBeispiele:\n  harness spec comments\n  harness spec comments spec-docs-rework\n  harness spec comments docs/superpowers/specs/2026-10-22-spec-docs-rework-design.md')
      .action((dokument?: string) => this.run(dokument));
  }

  // Liest die offenen Kommentare und gibt sie als Text oder JSON aus
  private async run(dokument?: string): Promise<void> {
    const project = await this.context.openProject();
    const open = await new DocCommentStore(project.root).listOpen(dokument);
    const lines = open.length ? open.flatMap((o) => this.format(o)) : ['Keine offenen Kommentare.'];
    this.context.printer().result(open, lines);
  }

  // Ein Kommentar als lesbarer Block
  private format({ path, comment }: OpenDocComment): string[] {
    const section = comment.heading ? ` · Abschnitt „${comment.heading}“` : '';
    const line = comment.lines ? ` · Zeile ${comment.lines[0]}` : '';
    return [`${comment.id} · ${path}${section}${line}`, `  Stelle: „${comment.quote}“`, `  Kommentar: ${comment.text}`];
  }
}
