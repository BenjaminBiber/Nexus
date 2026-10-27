import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { median, p95, round1 } from './Stats.mjs';

// Läufe vor der Messreihe, damit nicht der erste Aufruf (Dateien noch nicht im Cache) die Werte prägt
const WARMUP_RUNS = 3;

// Laufzeit eines Hooks: gemessen wird, was Claude Code wirklich startet – ein eigener Prozess je Ereignis,
// die Hook-Meldung auf der Standardeingabe. Die Zeit umfasst den gesamten Aufruf samt Start von Node.
export class HookBench {
  constructor({ command, args, cwd, env, tmp, log = () => {} }) {
    this.command = command;
    this.args = args;
    this.cwd = cwd;
    this.env = env;
    this.tmp = tmp;
    this.log = log;
  }

  // Messreihe für ein Ereignis: erst Aufwärmläufe, dann `runs` gemessene Läufe
  measure(kind, runs) {
    for (let warmup = 1; warmup <= WARMUP_RUNS; warmup += 1) this.once(kind, `bench-warm-${warmup}`);
    const values = [];
    for (let run = 1; run <= runs; run += 1) values.push(this.once(kind, `bench-${kind}-${run}`));
    return { median: round1(median(values)), p95: round1(p95(values)), runs: values.length };
  }

  // Ein Lauf; liefert die Zeit in Millisekunden. Ein Hook bleibt immer still und endet mit 0 – alles andere ist ein Fehler.
  once(kind, sessionId) {
    const input = JSON.stringify({ session_id: sessionId, cwd: this.cwd, prompt: 'hallo' });
    const begin = performance.now();
    const result = spawnSync(this.command, [...this.args, kind], {
      input,
      cwd: this.cwd,
      env: this.env,
      encoding: 'utf8',
      windowsHide: true,
    });
    const took = performance.now() - begin;
    if (result.error) throw new Error(`Der Hook ${kind} lässt sich nicht starten (${this.command}): ${result.error.message}`);
    if (result.status !== 0) {
      throw new Error(`Der Hook ${kind} endet mit Code ${result.status}: ${(result.stderr || result.stdout || '').slice(-300)}`);
    }
    return took;
  }

  // Messreihe über hyperfine (nur mit --hyperfine). Liefert null, wenn hyperfine fehlt oder nicht durchläuft;
  // dann gilt die eigene Messung. hyperfine schickt allen Läufen dieselbe Meldung, die Session-ID bleibt also gleich.
  hyperfine(kind, runs) {
    const payload = join(this.tmp, `hook-${kind}-eingabe.json`);
    const report = join(this.tmp, `hook-${kind}-hyperfine.json`);
    writeFileSync(payload, JSON.stringify({ session_id: `bench-hyperfine-${kind}`, cwd: this.cwd, prompt: 'hallo' }));
    const command = [this.command, ...this.args, kind].map((part) => (part.includes(' ') ? `"${part}"` : part)).join(' ');
    const result = spawnSync(
      'hyperfine',
      ['--shell=none', '--warmup', String(WARMUP_RUNS), '--runs', String(runs), '--input', payload, '--export-json', report, command],
      { cwd: this.cwd, env: this.env, encoding: 'utf8', windowsHide: true },
    );
    if (result.error || result.status !== 0) {
      const why = result.error ? result.error.message : `Code ${result.status}: ${(result.stderr || '').trim().slice(-200)}`;
      this.log(`  hyperfine nicht nutzbar (${why}); es gilt die eigene Messung.`);
      return null;
    }
    try {
      const times = JSON.parse(readFileSync(report, 'utf8')).results[0].times.map((seconds) => seconds * 1000);
      return { median: round1(median(times)), p95: round1(p95(times)), runs: times.length, tool: 'hyperfine' };
    } catch (error) {
      this.log(`  Bericht von hyperfine nicht lesbar (${error.message}); es gilt die eigene Messung.`);
      return null;
    }
  }
}
