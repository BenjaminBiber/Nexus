import type { WriteStream } from 'node:fs';
import { DispatchLog, HarnessError, OrphanSweeper, RUN_LOG_OUTPUT_MARKER as OUTPUT_MARKER, RunStore, StreamEventFormatter, type HarnessHome, type RunLogLine, type RunRecord, type RunResult } from '@harness/core';
import { ClaudeProcess } from './ClaudeProcess.js';
import { WorktreeSetupRunner } from './WorktreeSetupRunner.js';

const RUN_ID = /^[A-Za-z0-9._-]+$/;
const READ_ATTEMPTS = 5;
const READ_PAUSE_MS = 200;

// Läuft im losgelösten Worker-Prozess: liest den Lauf, startet Claude im Worktree, schreibt Protokoll und Ergebnis.
// Kennt keine Datenbank – ausgewertet wird im Server. Bei gültiger Lauf-ID entsteht immer eine Ergebnisdatei.
export class DispatchWorker {
  private readonly runs: RunStore;
  private readonly logs: DispatchLog;

  constructor(home: HarnessHome, private readonly sweeper: OrphanSweeper = new OrphanSweeper()) {
    this.runs = new RunStore(home);
    this.logs = new DispatchLog(home);
  }

  // Führt den Lauf `runId` aus; Fehler landen als Exit-Code 1 im Ergebnis und werden danach weitergereicht
  async run(runId: string): Promise<void> {
    if (!RUN_ID.test(runId)) throw new HarnessError('run-id-invalid', `„${runId}“ ist keine gültige Lauf-ID.`);
    let result: RunResult = { code: 1, timedOut: false, output: `Lauf ${runId} wurde nicht ausgeführt.` };
    let log: WriteStream | null = null;
    let record: RunRecord | null = null;
    const write = (text: string) => {
      if (log && !log.destroyed) log.write(text);
    };
    try {
      record = await this.readRecord(runId);
      if (!record) throw new HarnessError('run-not-found', `Lauf ${runId} gibt es nicht.`);
      log = this.openLog(record.logFile);
      const r = record.run;
      write(
        `[dispatch] ${new Date().toISOString()} ${record.agent} startet in ${record.cwd} (${{ work: 'Umsetzung', merge: 'Merge-Session', research: 'Recherche', plan: 'Planung' }[record.kind]})\n` +
          `[dispatch] Profil ${r.profile}: Modell ${r.model ?? 'Standard'}, Effort ${r.effort ?? 'Standard'}, ${r.maxTurns} Schritte, ${r.timeoutMinutes} min (${r.why})\n` +
          `[dispatch] ${record.command} ${record.args.join(' ')}\n` +
          `[dispatch] Arbeitsanweisung:\n${record.prompt}\n\n${OUTPUT_MARKER}\n`,
      );
      // Neu angelegter Worktree: erst die Einrichtung (z. B. npm ci); scheitert sie, startet Claude nicht
      let setup: Pick<RunResult, 'setupOk' | 'setupFailed'> = {};
      if (record.kind === 'work' && record.setup?.length) {
        setup = await new WorktreeSetupRunner().run({
          steps: record.setup, root: record.cwd, env: process.env, timeoutMs: (record.setupTimeoutMinutes ?? 15) * 60_000, onOutput: write,
        });
        if (setup.setupFailed) {
          const f = setup.setupFailed;
          result = { code: f.code, timedOut: f.timedOut, output: f.output, ...setup };
          write(`\n[dispatch] ${new Date().toISOString()} Einrichtung fehlgeschlagen – Claude wird nicht gestartet\n`);
          return;
        }
      }
      // stream-json lesbar und mit Uhrzeit ins Protokoll; je Strom ein eigener Formatierer (Teilzeilen bleiben getrennt).
      // Ausgewertet wird die rohe Ausgabe im Ergebnis (RunResult.output, zuletzt das Ergebnis-Ereignis)
      const streams = { stdout: new StreamEventFormatter({ root: record.cwd }), stderr: new StreamEventFormatter({ root: record.cwd }) };
      const readable = (entries: RunLogLine[], f: StreamEventFormatter) => entries.forEach((e) => write(f.stamped(e, new Date())));
      result = await new ClaudeProcess().run({
        command: record.command, args: record.args, prompt: record.prompt, cwd: record.cwd, env: process.env,
        timeoutMs: r.timeoutMinutes * 60_000,
        onOutput: (chunk, source) => (source === 'dispatch' ? write(chunk) : readable(streams[source].push(chunk), streams[source])),
      });
      result = { ...result, ...setup };
      for (const f of Object.values(streams)) readable(f.flush(), f);
      write(`\n[dispatch] ${new Date().toISOString()} Ende: Exit-Code ${result.code}${result.timedOut ? ', Zeitlimit' : ''}\n`);
    } catch (error) {
      result = { code: 1, timedOut: false, output: (error as Error).message };
      write(`\n[dispatch] ${new Date().toISOString()} Fehler: ${(error as Error).message}\n`);
      throw error;
    } finally {
      if (record) await this.sweep(runId, record.cwd, write);
      await this.closeLog(log);
      await this.runs.finish(runId, { ...result, endedAt: new Date().toISOString() });
    }
  }

  // Beendet Prozesse, die der Lauf in seinem Worktree verwaist zurückgelassen hat (z. B. ein `nexus serve … &`).
  // Nur in einem verknüpften Worktree, den kein anderer laufender Lauf nutzt; Fehler beenden den Lauf nicht
  private async sweep(runId: string, cwd: string, write: (text: string) => void): Promise<void> {
    try {
      if (!(await OrphanSweeper.linkedWorktree(cwd))) return;
      for (const other of await this.runs.list()) {
        if (other.id === runId || other.cwd !== cwd || (await this.runs.result(other.id))) continue;
        if (other.pid === null || this.alive(other.pid)) return;
      }
      const pids = await this.sweeper.sweep(cwd);
      if (pids.length) {
        write(`[dispatch] ${new Date().toISOString()} Aufgeräumt: ${pids.length} übrig gebliebene Prozesse beendet (PID ${pids.join(', ')})\n`);
      }
    } catch {
      // Aufräumen ist Kür
    }
  }

  // Läuft der Prozess noch?
  private alive(pid: number): boolean {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  }

  // Liest den Lauf; der Server schreibt ihn kurz vor dem Start, daher mehrere Versuche mit Pause
  private async readRecord(runId: string): Promise<RunRecord | null> {
    for (let attempt = 1; attempt <= READ_ATTEMPTS; attempt++) {
      const record = await this.runs.read(runId);
      if (record) return record;
      if (attempt < READ_ATTEMPTS) await new Promise((resolve) => setTimeout(resolve, READ_PAUSE_MS));
    }
    return null;
  }

  // Öffnet das Lauf-Protokoll; Fehler beim Protokoll beenden den Lauf nicht
  private openLog(file: string): WriteStream | null {
    try {
      const log = this.logs.openRunLog(file);
      log.on('error', () => undefined);
      return log;
    } catch {
      return null;
    }
  }

  // Schließt das Protokoll und wartet, bis alles geschrieben ist (oder der Stream schon kaputt ist)
  private closeLog(log: WriteStream | null): Promise<void> {
    if (!log || log.destroyed) return Promise.resolve();
    return new Promise<void>((resolve) => {
      log.once('close', () => resolve());
      log.end(() => resolve());
    });
  }
}
