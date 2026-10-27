import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BoardServices, HarnessHome, InboxOrderFile } from '@harness/core';
import { BoardInboxCommands } from '../../src/commands/board/BoardInboxCommands.js';
import { HarnessCommand } from '../../src/output/HarnessCommand.js';
import type { CliContext } from '../../src/CliContext.js';
import { BoardFixture } from '../../../core/tests/unit/board/helpers/BoardFixture.js';
import { Printer } from '../../src/output/Printer.js';

describe('harness board inbox (In-Memory-Board)', () => {
  const dirs: string[] = [];
  afterEach(() => {
    vi.restoreAllMocks();
    dirs.splice(0).forEach((d) => rmSync(d, { recursive: true, force: true }));
  });

  const setup = async (json = false) => {
    const f = await BoardFixture.create();
    const dir = mkdtempSync(join(tmpdir(), 'harness-inbox-cli-'));
    dirs.push(dir);
    const home = new HarnessHome({ HARNESS_HOME: dir });
    const services = new BoardServices(f.db, f.clock, f.repoPaths);
    const rt = { services, config: { activeBoard: f.boardId }, actor: (name?: string) => ({ userId: f.actor.userId, name: name ?? 'mensch' }) };
    const context = {
      withBoard: (work: (r: unknown) => Promise<unknown>) => work(rt),
      printer: () => new Printer(json),
      home: () => home,
      projectDir: () => process.cwd(),
    } as unknown as CliContext;
    const lines: string[] = [];
    vi.spyOn(console, 'log').mockImplementation((line: string) => { lines.push(line); });
    const run = (...args: string[]) => {
      const program = new HarnessCommand('harness').exitOverride();
      new BoardInboxCommands(context).register(program.command('board'));
      return program.parseAsync(['node', 'harness', 'board', ...args]);
    };
    const blocked = async (title: string, reason: string) => {
      const t = await f.tickets.create(f.boardId, { title }, f.actor);
      await f.tickets.update(t.id, { status: 'blocked', blockedReason: reason }, f.actor);
    };
    return { f, home, run, lines, blocked };
  };

  it('listet nummeriert mit Aktionshinweis als Markdown, in gespeicherter Reihenfolge', async () => {
    const { f, home, run, lines, blocked } = await setup();
    await blocked('Fertig', 'Human Interaction: ansehen');
    await blocked('Konflikt', 'Merge: a.ts');
    const note = await f.notes.add(f.boardId, { title: 'Cache prüfen' }, f.actor);
    await new InboxOrderFile(home).write(['ticket:ARC-0001', 'ticket:ARC-0002']);

    await run('inbox', '--md');
    expect(lines).toEqual([
      `1. **Notiz** \`Notiz ${note.id}\` Cache prüfen (ARC · Sprint) → \`harness board note take ${note.id}\``,
      '2. **Abnahme** `ARC-0001` Fertig (ARC · Sprint) – Human Interaction: ansehen → `harness board accept ARC-0001`',
      '3. **Merge gescheitert** `ARC-0002` Konflikt (ARC · Sprint) – Merge: a.ts → `harness board merge ARC-0002`',
    ]);
  });

  it('filtert nach Art und meldet eine leere Liste', async () => {
    const { run, lines, blocked } = await setup();
    await blocked('Konflikt', 'Merge: a.ts');
    await run('inbox', '--kind', 'accept,note');
    expect(lines).toEqual(['Nichts wartet auf dich.']);
    await expect(run('inbox', '--kind', 'quatsch')).rejects.toMatchObject({ code: 'input-invalid' });
  });

  it('liefert mit --json ein Array der Einträge', async () => {
    const { run, lines, blocked } = await setup(true);
    await blocked('Zugang', 'Zugang fehlt');
    await run('inbox');
    const parsed = JSON.parse(lines.join('\n'));
    expect(Array.isArray(parsed)).toBe(true);
    expect(parsed[0]).toMatchObject({ key: 'ticket:ARC-0001', kind: 'blocked', detail: 'Zugang fehlt' });
  });
});
