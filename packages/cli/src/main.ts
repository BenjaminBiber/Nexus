#!/usr/bin/env node
import type { Command } from 'commander';
import { HarnessError } from '@harness/core';
import { SqliteWarningFilter } from '@harness/db';
import { HarnessCommand } from './output/HarnessCommand.js';
import { CliContext } from './CliContext.js';
import { Printer } from './output/Printer.js';
import { CliErrorText } from './output/CliErrorText.js';
import { InitCommand } from './commands/InitCommand.js';
import { HookCommand } from './commands/HookCommand.js';
import { BoardCommand } from './commands/BoardCommand.js';
import { BoardProjectCommands } from './commands/board/BoardProjectCommands.js';
import { BoardBoardsCommands } from './commands/board/BoardBoardsCommands.js';
import { BoardTicketReadCommands } from './commands/board/BoardTicketReadCommands.js';
import { BoardTicketWriteCommands } from './commands/board/BoardTicketWriteCommands.js';
import { BoardClaimCommands } from './commands/board/BoardClaimCommands.js';
import { BoardWorkCommands } from './commands/board/BoardWorkCommands.js';
import { BoardStepCommands } from './commands/board/BoardStepCommands.js';
import { BoardPlanningCommands } from './commands/board/BoardPlanningCommands.js';
import { BoardCommentCommand } from './commands/board/BoardCommentCommand.js';
import { BoardNoteCommands } from './commands/board/BoardNoteCommands.js';
import { BoardInboxCommands } from './commands/board/BoardInboxCommands.js';
import { BoardQuestionCommands } from './commands/board/BoardQuestionCommands.js';
import { BoardDispatchCommands } from './commands/board/BoardDispatchCommands.js';
import { InstallCommand } from './commands/InstallCommand.js';
import { StopCommand } from './commands/StopCommand.js';
import { SpecCommentsCommand } from './commands/SpecCommentsCommand.js';
import { SpecResolveCommand } from './commands/SpecResolveCommand.js';

// Baut das Programm und bindet alle Befehle an
export function buildProgram(): Command {
  const program = new HarnessCommand();
  program
    .helpCommand('help [befehl]', 'Hilfe zu einem Befehl anzeigen')
    .name('harness')
    .description('AI Harness: Board, Specs, Worktrees und Sessions für parallele Claude-Sessions')
    .option('--json', 'Ausgabe als JSON')
    .option('--dir <pfad>', 'Projektordner (Standard: aktuelles Verzeichnis)');
  const context = new CliContext(program);
  new InitCommand(context).register(program);
  const board = new BoardCommand(context).register(program);
  new BoardProjectCommands(context).register(board);
  new BoardBoardsCommands(context).register(board);
  new BoardTicketReadCommands(context).register(board);
  new BoardTicketWriteCommands(context).register(board);
  new BoardClaimCommands(context).register(board);
  new BoardWorkCommands(context).register(board);
  new BoardStepCommands(context).register(board);
  new BoardPlanningCommands(context).register(board);
  new BoardCommentCommand(context).register(board);
  new BoardNoteCommands(context).register(board);
  new BoardInboxCommands(context).register(board);
  new BoardQuestionCommands(context).register(board);
  new BoardDispatchCommands(context).register(board);
  new InstallCommand(context).register(program);
  new StopCommand(context).register(program);
  new HookCommand().register(program);
  const spec = program.command('spec').description('Kommentare zu Spec- und Plan-Dokumenten abarbeiten');
  new SpecCommentsCommand(context).register(spec);
  new SpecResolveCommand(context).register(spec);
  return program;
}

// Startet das Programm und übersetzt Fehler in Rückgabecodes (1 erwartbar, 2 unerwartet); mit --json als JSON-Objekt auf stdout
async function main(): Promise<void> {
  SqliteWarningFilter.install();
  const printer = new Printer(process.argv.includes('--json'));
  try {
    await buildProgram().parseAsync(process.argv);
  } catch (error) {
    if (error instanceof HarnessError) {
      const text = new CliErrorText();
      printer.failure(error.code, text.of(error), text.line(error));
      process.exitCode = 1;
    } else {
      const message = `Unerwarteter Fehler: ${(error as Error).message}`;
      printer.failure('unexpected', message, message);
      process.exitCode = 2;
    }
  }
}

// Kein Top-Level-await: im gebündelten Sidecar ist das Ergebnis CommonJS, und das kennt keines.
// Der Rückgabecode steht in process.exitCode, der Prozess endet also wie vorher
void main();
