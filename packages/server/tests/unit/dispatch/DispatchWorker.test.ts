import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { GitBashLocator, HarnessHome, RunOutputReader, RunStore } from '@harness/core';
import { WorktreeSetupRunner } from '../../../src/dispatch/WorktreeSetupRunner.js';
import { ClaudeProcess } from '../../../src/dispatch/ClaudeProcess.js';
import { DispatchWorker } from '../../../src/dispatch/DispatchWorker.js';
import { ProcessWorkerLauncher } from '../../../src/dispatch/WorkerLauncher.js';
import { FakeClaude } from '../../helpers/FakeClaude.js';

const RUN = { profile: 'standard', why: 'Standardprofil standard', model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 1 };

describe('Worker des Dispatchers', () => {
  let dir: string;
  let home: HarnessHome;
  let command: string;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-worker-'));
    home = new HarnessHome({ HARNESS_HOME: dir });
    command = FakeClaude.create(dir);
  });
  afterEach(async () => {
    delete process.env.FAKE_CLAUDE_MODE;
    // Beendete Prozesse geben den Ordner unter Windows erst einen Moment später frei (rm wiederholt bei EPERM, rmSync nicht)
    await rm(dir, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
  });

  it('ClaudeProcess gibt Anweisung über stdin, Argumente und Umgebung weiter', async () => {
    let seen = '';
    const result = await new ClaudeProcess().run({
      command, args: ['-p', '--allowedTools', 'Bash(harness:*)'], prompt: 'Hallo Welt', cwd: dir,
      env: { ...process.env, HARNESS_TICKET: 'ARC-0001', ANTHROPIC_API_KEY: undefined }, timeoutMs: 20000, onOutput: (c) => { seen += c; },
    });
    expect(result.code).toBe(0);
    expect(result.timedOut).toBe(false);
    expect(result.output).toContain('ticket=ARC-0001 key=- args=-p --allowedTools Bash(harness:*) prompt=Hallo Welt');
    expect(seen).toContain('"type":"result"');
  });

  it('ClaudeProcess kürzt lange Ausgabe auf ganze Zeilen; das Bruchstück der ersten Zeile zählt nicht als Nutzungslimit', async () => {
    const script = [
      `process.stdout.write('{"type":"user","x":"' + 'a'.repeat(150000) + 'rate_limit' + 'b'.repeat(100000) + '"}\\n');`,
      `process.stdout.write('{"type":"result","subtype":"error_max_turns","is_error":true,"result":""}\\n');`,
      'process.exit(1);',
    ].join('');
    const result = await new ClaudeProcess().run({
      command: process.execPath, args: ['-e', script], prompt: '', cwd: dir, env: process.env, timeoutMs: 20000, onOutput: () => undefined,
    });
    expect(result.output).not.toContain('rate_limit');
    expect(result.output.startsWith('{"type":"result"')).toBe(true);
    const reader = new RunOutputReader();
    expect(reader.hitUsageLimit(result)).toBe(false);
  });

  it('ClaudeProcess beendet nach dem Zeitlimit', async () => {
    const result = await new ClaudeProcess().run({
      command, args: ['-p'], prompt: 'x', cwd: dir, env: { ...process.env, FAKE_CLAUDE_MODE: 'sleep' }, timeoutMs: 1500, onOutput: () => undefined,
    });
    expect(result.timedOut).toBe(true);
  }, 30000);

  it('ClaudeProcess meldet einen fehlenden Befehl mit Exit-Code 127', async () => {
    const result = await new ClaudeProcess().run({
      command: join(dir, 'gibt-es-nicht.exe'), args: [], prompt: '', cwd: dir, env: process.env, timeoutMs: 5000, onOutput: () => undefined,
    });
    expect(result.code).toBe(127);
  });

  it('DispatchWorker schreibt Protokoll und Ergebnis', async () => {
    const runs = new RunStore(home);
    const logFile = join(dir, 'lauf.log');
    await runs.create({
      id: 'ARC-0001-x', kind: 'work', ticketId: 'ARC-0001', boardId: 1, title: 'T', agent: 'junior-ARC-0001', cwd: dir, branch: 'b', target: null,
      run: RUN, command, args: ['-p'], prompt: 'Arbeitsanweisung', logFile, startedAt: new Date().toISOString(), pid: null,
    });
    process.env.FAKE_CLAUDE_MODE = 'fail';
    await new DispatchWorker(home).run('ARC-0001-x');
    const end = await runs.result('ARC-0001-x');
    expect(end).toMatchObject({ code: 3, timedOut: false });
    expect(end!.output).toContain('kaputt');
    const log = readFileSync(logFile, 'utf8');
    expect(log).toContain('junior-ARC-0001 startet');
    expect(log).toContain('Arbeitsanweisung');
    expect(log).toContain('Ende: Exit-Code 3');
  });

  it('DispatchWorker führt die Einrichtung vor Claude nacheinander im jeweiligen Ordner aus und protokolliert sie', async () => {
    const runs = new RunStore(home);
    const logFile = join(dir, 'einrichtung.log');
    mkdirSync(join(dir, 'sub'));
    await runs.create({
      id: 'ARC-0005-x', kind: 'work', ticketId: 'ARC-0005', boardId: 1, title: 'T', agent: 'junior-ARC-0005', cwd: dir, branch: 'b', target: null,
      run: RUN, command, args: ['-p'], prompt: 'p', logFile, startedAt: new Date().toISOString(), pid: null,
      setup: [{ cwd: '.', run: 'echo eins>>order.txt' }, { cwd: 'sub', run: 'echo zwei>>../order.txt' }], setupTimeoutMinutes: 1,
    });
    await new DispatchWorker(home).run('ARC-0005-x');
    expect(readFileSync(join(dir, 'order.txt'), 'utf8').split(/\r?\n/).map((l) => l.trim()).filter(Boolean)).toEqual(['eins', 'zwei']);
    const end = await runs.result('ARC-0005-x');
    expect(end).toMatchObject({ code: 0, setupOk: true });
    expect(end!.setupFailed).toBeUndefined();
    expect(end!.output).toContain('prompt=p');
    const log = readFileSync(logFile, 'utf8');
    expect(log).toContain('[dispatch] Einrichtung: .$ echo eins>>order.txt');
    expect(log).toContain('[dispatch] Einrichtung: sub$ echo zwei>>../order.txt');
    expect(log.indexOf('Einrichtung: sub$')).toBeLessThan(log.indexOf('Ende: Exit-Code 0'));
  });

  it('DispatchWorker startet Claude nicht, wenn die Einrichtung scheitert, und meldet den Befehl im Ergebnis', async () => {
    const runs = new RunStore(home);
    const logFile = join(dir, 'kaputt.log');
    await runs.create({
      id: 'ARC-0006-x', kind: 'work', ticketId: 'ARC-0006', boardId: 1, title: 'T', agent: 'junior-ARC-0006', cwd: dir, branch: 'b', target: null,
      run: RUN, command, args: ['-p'], prompt: 'p', logFile, startedAt: new Date().toISOString(), pid: null,
      setup: [{ cwd: '.', run: 'echo kaputt&& exit 3' }, { cwd: '.', run: 'echo nie>>nie.txt' }], setupTimeoutMinutes: 1,
    });
    await new DispatchWorker(home).run('ARC-0006-x');
    const end = await runs.result('ARC-0006-x');
    expect(end).toMatchObject({ code: 3, setupFailed: { cwd: '.', run: 'echo kaputt&& exit 3', code: 3, timedOut: false } });
    expect(end!.setupOk).toBeUndefined();
    expect(end!.setupFailed!.output).toContain('kaputt');
    expect(end!.output).not.toContain('prompt=');
    expect(existsSync(join(dir, 'nie.txt'))).toBe(false);
    expect(readFileSync(logFile, 'utf8')).toContain('Einrichtung fehlgeschlagen – Claude wird nicht gestartet');
  });

  it.skipIf(process.platform !== 'win32' || !new GitBashLocator().locate(process.env))('Einrichtung läuft unter Windows in der Git Bash, wenn sie gefunden wurde', async () => {
    const bash = new GitBashLocator().locate(process.env)!;
    const result = await new WorktreeSetupRunner().run({
      steps: [{ cwd: '.', run: 'echo "$BASH_VERSION" > shell.txt' }], root: dir, env: { ...process.env, CLAUDE_CODE_GIT_BASH_PATH: bash },
      timeoutMs: 20000, onOutput: () => undefined,
    });
    expect(result).toEqual({ setupOk: true });
    expect(readFileSync(join(dir, 'shell.txt'), 'utf8').trim()).toMatch(/^\d+\./);
  });

  it('Einrichtung beendet einen Befehl nach dem Zeitlimit', async () => {
    const result = await new WorktreeSetupRunner().run({
      steps: [{ cwd: '.', run: `"${process.execPath}" -e "setInterval(() => {}, 1000)"` }], root: dir, env: process.env, timeoutMs: 1000, onOutput: () => undefined,
    });
    expect(result.setupFailed).toMatchObject({ timedOut: true });
  }, 30000);

  it('DispatchWorker schreibt stream-json lesbar mit Uhrzeit ins Protokoll; das Ergebnis bleibt auswertbar', async () => {
    const runs = new RunStore(home);
    const logFile = join(dir, 'strom.log');
    await runs.create({
      id: 'ARC-0004-x', kind: 'work', ticketId: 'ARC-0004', boardId: 1, title: 'T', agent: 'junior-ARC-0004', cwd: dir, branch: 'b', target: null,
      run: RUN, command, args: ['-p'], prompt: 'Zeile 1\n\nZeile 3', logFile, startedAt: new Date().toISOString(), pid: null,
    });
    process.env.FAKE_CLAUDE_MODE = 'stream';
    process.env.HARNESS_TICKET = 'ARC-0004';
    try {
      await new DispatchWorker(home).run('ARC-0004-x');
    } finally {
      delete process.env.HARNESS_TICKET;
    }
    const log = readFileSync(logFile, 'utf8');
    expect(log).toContain('[dispatch] Arbeitsanweisung:\nZeile 1\n\nZeile 3\n\n[dispatch] Ausgabe:\n');
    expect(log).toMatch(/\n\d\d:\d\d:\d\d {2}\[start\] Claude startet \(fake-opus\)\n\d\d:\d\d:\d\d {2}\[say\] Ich lese das Ticket\.\n\d\d:\d\d:\d\d {2}\[run(#\S+)?\] liest Ticket ARC-0004\t"\$ harness board show ARC-0004"\n/);
    expect(log).toMatch(/\d\d:\d\d:\d\d {2}\[fail(#\S+)?\] abgelehnt\t"abgelehnt"\n/);
    expect(log).toMatch(/\d\d:\d\d:\d\d {2}\[done\] fertig, 3 Schritte, 1 min\n/);
    expect(log).not.toContain('"type"');
    const end = await runs.result('ARC-0004-x');
    expect(end).toMatchObject({ code: 0, timedOut: false });
    const reader = new RunOutputReader();
    expect(reader.summarize(end!.output)).toEqual({ subtype: 'success', isError: false, text: 'erledigt', denied: ['Bash: git push'] });
  });

  it('ClaudeProcess liefert nach dem Zeitlimit auch dann ein Ergebnis, wenn das Beenden des Baums nicht greift', async () => {
    const proc = new ClaudeProcess(300);
    // Simuliert ein wirkungsloses taskkill/kill: nur der harte Abbruch bleibt
    let pid: number | undefined;
    (proc as unknown as { killTree: (pid?: number) => void }).killTree = (p) => { pid = p; };
    const result = await proc.run({
      command: process.execPath, args: [join(dir, 'fake-claude.mjs')], prompt: 'x', cwd: dir,
      env: { ...process.env, FAKE_CLAUDE_MODE: 'sleep' }, timeoutMs: 500, onOutput: () => undefined,
    });
    expect(result.timedOut).toBe(true);
    // Warten, bis der hart beendete Prozess wirklich weg ist (gibt sonst den Arbeitsordner nicht frei)
    const alive = () => { try { process.kill(pid!, 0); return true; } catch { return false; } };
    const deadline = Date.now() + 10000;
    while (pid && alive() && Date.now() < deadline) await new Promise((r) => setTimeout(r, 50));
    expect(pid && alive()).toBeFalsy();
  }, 20000);

  it('ClaudeProcess dekodiert UTF-8-Zeichen, die über zwei Blöcke verteilt ankommen', async () => {
    const script = join(dir, 'split.mjs');
    writeFileSync(script, 'process.stdout.write(Buffer.from([0xc3])); setTimeout(() => process.stdout.write(Buffer.from([0xa4])), 200);');
    let seen = '';
    const result = await new ClaudeProcess().run({
      command: process.execPath, args: [script], prompt: '', cwd: dir, env: process.env, timeoutMs: 10000, onOutput: (c) => { seen += c; },
    });
    expect(result.output).toBe('ä');
    expect(seen).toBe('ä');
  });

  it.skipIf(process.platform !== 'win32')('ClaudeProcess sucht Befehle zuerst nach PATH-Reihenfolge, dann nach Endung', () => {
    const first = join(dir, 'a');
    const second = join(dir, 'b');
    mkdirSync(first);
    mkdirSync(second);
    writeFileSync(join(first, 'claude.cmd'), '');
    writeFileSync(join(second, 'claude.exe'), '');
    const resolve = (new ClaudeProcess() as unknown as { resolveCommand: (c: string, e: NodeJS.ProcessEnv) => { file: string; shell: boolean } }).resolveCommand;
    expect(resolve.call(new ClaudeProcess(), 'claude', { PATH: [first, second].join(delimiter) })).toEqual({ file: join(first, 'claude.cmd'), shell: true });
  });

  it('DispatchWorker liest einen kurz danach geschriebenen Lauf', async () => {
    const runs = new RunStore(home);
    const logFile = join(dir, 'spaet.log');
    setTimeout(() => {
      void runs.create({
        id: 'ARC-0002-x', kind: 'work', ticketId: 'ARC-0002', boardId: 1, title: 'T', agent: 'junior-ARC-0002', cwd: dir, branch: 'b', target: null,
        run: RUN, command, args: ['-p'], prompt: 'p', logFile, startedAt: new Date().toISOString(), pid: null,
      });
    }, 300);
    await new DispatchWorker(home).run('ARC-0002-x');
    expect(await runs.result('ARC-0002-x')).toMatchObject({ code: 0, timedOut: false });
  });

  it('DispatchWorker schreibt auch ohne lesbaren Lauf ein Ergebnis und prüft die Lauf-ID', async () => {
    const runs = new RunStore(home);
    await expect(new DispatchWorker(home).run('fehlt')).rejects.toMatchObject({ code: 'run-not-found' });
    expect(await runs.result('fehlt')).toMatchObject({ code: 1, timedOut: false, output: expect.stringContaining('fehlt') });
    await expect(new DispatchWorker(home).run('../böse')).rejects.toMatchObject({ code: 'run-id-invalid' });
    expect(existsSync(join(dir, 'dispatch', 'böse.result.json'))).toBe(false);
  });

  it('DispatchWorker läuft weiter, wenn das Protokoll nicht geschrieben werden kann', async () => {
    const runs = new RunStore(home);
    const logFile = join(dir, 'ist-ein-ordner');
    mkdirSync(logFile);
    await runs.create({
      id: 'ARC-0003-x', kind: 'work', ticketId: 'ARC-0003', boardId: 1, title: 'T', agent: 'junior-ARC-0003', cwd: dir, branch: 'b', target: null,
      run: RUN, command, args: ['-p'], prompt: 'p', logFile, startedAt: new Date().toISOString(), pid: null,
    });
    await new DispatchWorker(home).run('ARC-0003-x');
    expect(await runs.result('ARC-0003-x')).toMatchObject({ code: 0, timedOut: false });
  });

  it('ProcessWorkerLauncher lässt Debug-Schalter des Servers weg', async () => {
    const entry = join(dir, 'entry.mjs');
    const out = join(dir, 'argv.json');
    writeFileSync(entry, `import { writeFileSync } from 'node:fs'; writeFileSync(${JSON.stringify(out)}, JSON.stringify(process.execArgv));`);
    // --debug lässt Node sofort abbrechen; ohne Filter entstünde keine Ausgabedatei
    const pid = new ProcessWorkerLauncher(entry, ['--debug', '--inspect-brk=0', '--no-warnings']).launch('ARC-0001-x', process.env);
    expect(typeof pid).toBe('number');
    const deadline = Date.now() + 10000;
    while (!existsSync(out) && Date.now() < deadline) await new Promise((r) => setTimeout(r, 50));
    expect(JSON.parse(readFileSync(out, 'utf8'))).toEqual(['--no-warnings']);
  });

  it('ProcessWorkerLauncher startet die Programmdatei losgelöst mit --dispatch-worker', async () => {
    const entry = join(dir, 'entry.mjs');
    const out = join(dir, 'argv.json');
    writeFileSync(entry, `import { writeFileSync } from 'node:fs'; writeFileSync(${JSON.stringify(out)}, JSON.stringify({ argv: process.argv.slice(2), t: process.env.HARNESS_TICKET }));`);
    const pid = new ProcessWorkerLauncher(entry, []).launch('ARC-0001-x', { ...process.env, HARNESS_TICKET: 'ARC-0001' });
    expect(typeof pid).toBe('number');
    const deadline = Date.now() + 10000;
    while (!existsSync(out) && Date.now() < deadline) await new Promise((r) => setTimeout(r, 50));
    expect(JSON.parse(readFileSync(out, 'utf8'))).toEqual({ argv: ['--dispatch-worker', 'ARC-0001-x'], t: 'ARC-0001' });
  });
});
