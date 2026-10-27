import { afterEach, describe, expect, it, vi } from 'vitest';
import { BoardServices } from '@harness/core';
import { BoardStepCommands } from '../../src/commands/board/BoardStepCommands.js';
import { BoardTicketWriteCommands } from '../../src/commands/board/BoardTicketWriteCommands.js';
import { HarnessCommand } from '../../src/output/HarnessCommand.js';
import type { CliContext } from '../../src/CliContext.js';
import { BoardFixture } from '../../../core/tests/unit/board/helpers/BoardFixture.js';
import { Printer } from '../../src/output/Printer.js';

describe('harness board step (Befehlsbaum mit In-Memory-Board)', () => {
  afterEach(() => vi.restoreAllMocks());

  // Programm mit `board step` und `board create`; der Kontext liefert In-Memory-Dienste, Handelnder = --by
  const setup = async () => {
    const f = await BoardFixture.create();
    const services = new BoardServices(f.db, f.clock, f.repoPaths);
    const rt = { services, config: { activeBoard: f.boardId }, actor: (name?: string) => ({ userId: f.actor.userId, name: name ?? 'mensch' }) };
    const context = {
      withBoard: (work: (r: unknown) => Promise<unknown>) => work(rt),
      printer: () => new Printer(false),
      projectDir: () => process.cwd(),
    } as unknown as CliContext;
    const lines: string[] = [];
    vi.spyOn(console, 'log').mockImplementation((line: string) => { lines.push(line); });
    // Je Aufruf ein frisches Programm wie bei einem echten Prozess (commander behält Optionswerte zwischen Läufen)
    const run = (...args: string[]) => {
      const program = new HarnessCommand('harness').exitOverride();
      const board = program.command('board');
      new BoardTicketWriteCommands(context).register(board);
      new BoardStepCommands(context).register(board);
      return program.parseAsync(['node', 'harness', 'board', ...args]);
    };
    return { f, run, lines };
  };

  it('legt mit create --step Schritte an und bearbeitet sie über die Unterbefehle', async () => {
    const { f, run, lines } = await setup();
    await run('create', 'Export', '--step', 'Test schreiben, rot', '--step', 'Umsetzen');
    expect((await f.tickets.get('ARC-0001')).steps.map((s) => s.text)).toEqual(['Test schreiben, rot', 'Umsetzen']);

    await run('step', 'add', 'ARC-0001', 'Committen', '--detail', 'git commit', '--by', 'claude');
    let detail = await f.tickets.get('ARC-0001');
    expect(detail.steps[2]).toEqual({ text: 'Committen', done: false, detail: 'git commit' });
    expect(detail.history.at(-1)).toMatchObject({ byName: 'claude', note: 'Schritt hinzugefügt' });
    expect(lines).toContain('  3. [ ] Committen');

    await run('step', 'add', 'ARC-0001', 'Lesen', '--at', '1');
    await run('step', 'edit', 'ARC-0001', '2', '--text', 'Test zuerst');
    await run('step', 'move', 'ARC-0001', '4', '1');
    await run('step', 'rm', 'ARC-0001', '3');
    detail = await f.tickets.get('ARC-0001');
    expect(detail.steps.map((s) => s.text)).toEqual(['Committen', 'Lesen', 'Umsetzen']);
  });

  it('hakt mit der alten Form und mit step done ab und zurück', async () => {
    const { f, run } = await setup();
    await run('create', 'Export', '--step', 'A', '--step', 'B');
    await run('step', 'ARC-0001', '2', '--by', 'claude');
    expect((await f.tickets.get('ARC-0001')).steps.map((s) => s.done)).toEqual([false, true]);
    expect((await f.tickets.get('ARC-0001')).history.at(-1)).toMatchObject({ byName: 'claude', field: 'step' });
    await run('step', 'ARC-0001', '2', '--undo');
    await run('step', 'done', 'ARC-0001', '1');
    expect((await f.tickets.get('ARC-0001')).steps.map((s) => s.done)).toEqual([true, false]);
    await run('step', 'done', 'ARC-0001', '1', '--undo', '--by', 'claude');
    const detail = await f.tickets.get('ARC-0001');
    expect(detail.steps.map((s) => s.done)).toEqual([false, false]);
    expect(detail.history.at(-1)).toMatchObject({ byName: 'claude', from: '1: erledigt', to: '1: offen' });
  });

  it('meldet ungültige Nummern', async () => {
    const { run } = await setup();
    await run('create', 'Export', '--step', 'A');
    await expect(run('step', 'rm', 'ARC-0001', 'x')).rejects.toMatchObject({ code: 'step-invalid' });
    await expect(run('step', 'move', 'ARC-0001', '1', '3')).rejects.toMatchObject({ code: 'step-not-found' });
  });

  it('schützt mit --expect und --rev vor gleichzeitigen Änderungen (alte Form und Unterbefehle)', async () => {
    const { f, run } = await setup();
    await run('create', 'Export', '--step', 'A', '--step', 'B');
    await run('step', 'move', 'ARC-0001', '2', '1');
    const conflict = { code: 'conflict', message: 'ARC-0001: Schritt 1 ist inzwischen „B“ – die Schritte wurden geändert.' };
    await expect(run('step', 'ARC-0001', '1', '--expect', 'A')).rejects.toMatchObject(conflict);
    await expect(run('step', 'done', 'ARC-0001', '1', '--expect', 'A')).rejects.toMatchObject(conflict);
    await expect(run('step', 'edit', 'ARC-0001', '1', '--text', 'X', '--expect', 'A')).rejects.toMatchObject(conflict);
    await expect(run('step', 'rm', 'ARC-0001', '1', '--expect', 'A')).rejects.toMatchObject(conflict);
    await expect(run('step', 'move', 'ARC-0001', '1', '2', '--expect', 'A')).rejects.toMatchObject(conflict);
    await expect(run('step', 'rm', 'ARC-0001', '1', '--rev', '1')).rejects.toMatchObject({ code: 'conflict' });
    await expect(run('step', 'rm', 'ARC-0001', '1', '--rev', 'x')).rejects.toMatchObject({ code: 'rev-invalid' });
    const rev = (await f.tickets.get('ARC-0001')).rev;
    await run('step', 'ARC-0001', '1', '--expect', 'B', '--rev', String(rev));
    await run('step', 'rm', 'ARC-0001', '2', '--expect', 'A', '--rev', String(rev + 1));
    const detail = await f.tickets.get('ARC-0001');
    expect(detail.steps).toEqual([{ text: 'B', done: true }]);
  });
});
