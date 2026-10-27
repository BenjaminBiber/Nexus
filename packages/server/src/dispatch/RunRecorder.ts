import { randomBytes } from 'node:crypto';
import {
  AbortReasonClassifier, DispatchConfigFingerprint, HarnessError, isHumanInteraction,
  type DispatchRun, type DispatchRunOutcome, type OutputSummary, type RunEnd, type RunRecord, type RunSettings, type RunUsage,
} from '@harness/core';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { DispatchContext } from './DispatchContext.js';

/** Ergebnis der Auswertung eines Laufs: Ausgang für Bericht und Protokoll, dazu der Abbruch bzw. Blockiert-Grund */
export interface HandleResult {
  outcome: string;
  abort?: { reason: string; final: boolean };
  blockedReason?: string;
}

const OUTCOMES: Record<string, DispatchRunOutcome> = {
  'done': 'done', 'review': 'review', 'question': 'question', 'limit': 'pause-limit', 'claude-missing': 'pause-claude-missing',
  'released': 'abort', 'taken': 'taken', 'other': 'other', 'gone': 'gone', 'merge-session': 'merge-session', 'setup-failed': 'setup-failed',
};

// Erfasst ausgewertete Läufe für die Auswertung: erst lokal vorgemerkt (RunStore, Ordner stats), dann in board.DispatchRuns.
// So geht bei einem Verbindungsfehler kein Lauf verloren und keiner wird doppelt ausgewertet.
export class RunRecorder {
  private readonly classifier = new AbortReasonClassifier();
  private readonly fingerprint = new DispatchConfigFingerprint();
  /** Fehlende Erweiterung schon protokolliert (sonst jeden Tick derselbe Eintrag) */
  private outdatedNoted = false;

  constructor(private readonly ctx: DispatchContext) {}

  // Zeile zu einem beendeten Lauf vormerken; Fehler nur ins Protokoll (nie werfen: der Lauf ist schon ausgewertet und darf nicht\n  // ein zweites Mal ausgewertet werden)
  async keep(record: RunRecord, end: RunEnd, result: HandleResult): Promise<void> {
    // Planungen erfasst board.DispatchRuns (noch) nicht: die Spalte Kind kennt nur work, merge, research
    if (record.kind === 'plan') return;
    try {
      const board = await this.ctx.board(record.boardId);
      if (!board) return;
      const usage = this.ctx.output.usage(end.output);
      const summary = this.ctx.output.summarize(end.output);
      const row: DispatchRun = {
        ...this.base(this.id(record.id), record.kind, record.ticketId, board.projectId, board.id, record.agent, record.run, record.attempt ?? null, record.startedAt),
        model: usage?.model ?? null,
        endedAt: end.endedAt,
        durationMs: Math.max(0, Date.parse(end.endedAt) - Date.parse(record.startedAt)) || null,
        numTurns: usage?.numTurns ?? null,
        exitCode: end.code,
        timedOut: end.timedOut,
        ...this.outcome(record, result),
        ...this.denied(usage, summary),
        costUsd: usage?.costUsd ?? null,
        inputTokens: usage?.inputTokens ?? null,
        outputTokens: usage?.outputTokens ?? null,
        cacheReadTokens: usage?.cacheReadTokens ?? null,
        cacheWriteTokens: usage?.cacheWriteTokens ?? null,
        modelUsage: usage && Object.keys(usage.modelUsage).length ? JSON.stringify(usage.modelUsage) : null,
      };
      await this.ctx.stores.runs.keepStats(row.id, row);
    } catch (error) {
      this.ctx.log(`FEHLER ${record.ticketId}: Lauf nicht für die Auswertung vorgemerkt: ${(error as Error).message}`);
    }
  }

  // Gescheiterter Start (kein Lauf-Datensatz): Abbruch mit Kategorie start-failed vormerken
  async keepStartFailed(input: { ticketId: string; projectId: number; boardId: number; agent: string; run: RunSettings; attempt: number; reason: string; final: boolean }): Promise<void> {
    const at = this.ctx.now().toISOString();
    const id = this.id(`${this.ctx.stores.log.runName(input.ticketId, 'work', this.ctx.now())}-start`);
    try {
      await this.ctx.stores.runs.keepStats(id, {
        ...this.base(id, 'work', input.ticketId, input.projectId, input.boardId, input.agent, input.run, input.attempt, at),
        model: null, endedAt: at, durationMs: 0, numTurns: null, exitCode: null, timedOut: false,
        outcome: input.final ? 'abort-final' : 'abort', abortCategory: 'start-failed', abortReason: input.reason,
        costUsd: null, inputTokens: null, outputTokens: null, cacheReadTokens: null, cacheWriteTokens: null, modelUsage: null,
      } satisfies DispatchRun);
    } catch (error) {
      this.ctx.log(`FEHLER ${input.ticketId}: gescheiterter Start nicht für die Auswertung vorgemerkt: ${(error as Error).message}`);
    }
  }

  // Vorgemerkte Zeilen in die Datenbank schreiben; ein Verbindungsfehler lässt den Rest für den nächsten Tick liegen,
  // ebenso eine noch nicht eingespielte Erweiterung (die Zeilen gehen nach dem Einspielen hinein); eine Zeile, die die
  // Datenbank ablehnt, wird nach dem Protokolleintrag verworfen
  async flush(): Promise<number> {
    let written = 0;
    const kept = await this.ctx.stores.runs.keptStats<DispatchRun>();
    if (kept.length) await this.keepConfigVersion();
    for (const { id, row } of kept) {
      try {
        await this.ctx.services.dispatchRuns.record(row);
        written += 1;
      } catch (error) {
        if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        if (this.outdated(error)) return written;
        this.ctx.log(`FEHLER Lauf ${id} nicht in die Auswertung geschrieben: ${(error as Error).message}`);
      }
      await this.ctx.stores.runs.forgetStats(id);
    }
    return written;
  }

  // Die aktuelle Fassung der Einstellung einmal ablegen (idempotent über den Hash), damit das Dashboard Konfig-Änderungen benennen kann
  private async keepConfigVersion(): Promise<void> {
    const { cfg } = this.ctx;
    try {
      await this.ctx.services.dispatchRuns.recordConfigVersion({
        hash: this.fingerprint.hash(cfg), dispatcher: cfg.dispatcherName,
        firstSeenAt: this.ctx.now().toISOString(), content: this.fingerprint.content(cfg),
      });
    } catch (error) {
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      if (this.outdated(error)) return;
      this.ctx.log(`FEHLER Konfig-Version nicht gespeichert: ${(error as Error).message}`);
    }
  }

  // Fehlt der Datenbank die Erweiterung für die Läufe? Protokolliert das einmal je Prozess
  private outdated(error: unknown): boolean {
    if (!(error instanceof HarnessError) || error.code !== 'board-schema-outdated') return false;
    if (!this.outdatedNoted) this.ctx.log(`Läufe bleiben vorgemerkt, bis die Erweiterung eingespielt ist: ${error.message}`);
    this.outdatedNoted = true;
    return true;
  }

  // Lauf-IDs haben Sekunden-Auflösung (Abbruch und Neustart im selben Tick ergäben dieselbe): ein zufälliger Anhang macht die Zeile
  // eindeutig; beim Nachschreiben bleibt sie gleich, weil sie in der vorgemerkten Datei steht
  private id(runId: string): string {
    return `${runId}-${randomBytes(4).toString('hex')}`;
  }

  // Felder, die aus Lauf und Profil folgen
  private base(id: string, kind: DispatchRun['kind'], subjectId: string, projectId: number, boardId: number, agent: string, run: RunSettings, attempt: number | null, startedAt: string) {
    return {
      id, kind, subjectId, projectId, boardId, agent, attempt, profile: run.profile, escalatedFrom: run.escalatedFrom ?? null,
      requestedModel: run.model ?? null, effort: run.effort ?? null, startedAt, source: 'dispatcher' as const,
      // Limits und Grund der Profilwahl, wie sie für diesen Lauf galten; Läufe älterer Stände haben keinen Grund
      maxTurns: run.maxTurns ?? null, timeoutMinutes: run.timeoutMinutes ?? null,
      profileReason: run.reason ?? null, ruleIndex: run.ruleIndex ?? null,
      // Fassung der Einstellung, die beim Start galt; Läufe älterer Stände haben keine
      configHash: run.configHash ?? null,
      // Ohne Lauf-Ausgabe (gescheiterter Start) gibt es keine Ablehnungen zu zählen
      deniedCount: null, deniedTools: null,
    };
  }

  // Abgelehnte Werkzeugaufrufe: null ohne Ergebnis-Objekt (z. B. hart beendeter Lauf), sonst Anzahl und eindeutige
  // Werkzeugnamen (Reihenfolge des ersten Auftretens); `usage` ist bereits null, wenn kein Ergebnis gelesen werden konnte
  private denied(usage: RunUsage | null, summary: OutputSummary): Pick<DispatchRun, 'deniedCount' | 'deniedTools'> {
    if (!usage) return { deniedCount: null, deniedTools: null };
    if (!summary.denied.length) return { deniedCount: 0, deniedTools: null };
    const tools: string[] = [];
    for (const entry of summary.denied) {
      const tool = entry.split(': ')[0];
      if (!tools.includes(tool)) tools.push(tool);
    }
    return { deniedCount: summary.denied.length, deniedTools: tools.join(', ').slice(0, 500) };
  }

  // Feste Kategorie des Ausgangs, bei Abbruch mit Grund und Kategorie
  private outcome(record: RunRecord, result: HandleResult): Pick<DispatchRun, 'outcome' | 'abortCategory' | 'abortReason'> {
    if (record.kind === 'research') return { outcome: 'research', abortCategory: null, abortReason: null };
    if (result.abort) {
      return {
        outcome: result.abort.final ? 'abort-final' : 'abort', abortCategory: this.classifier.category(result.abort.reason),
        abortReason: result.abort.reason.slice(0, 500),
      };
    }
    if (result.outcome === 'blocked') {
      const human = isHumanInteraction(result.blockedReason);
      return { outcome: human ? 'human' : 'blocked', abortCategory: null, abortReason: result.blockedReason?.slice(0, 500) ?? null };
    }
    return { outcome: OUTCOMES[result.outcome] ?? 'other', abortCategory: null, abortReason: null };
  }
}
