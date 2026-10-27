import { AbortReasonClassifier } from '../dispatch/AbortReasonClassifier.js';
import type { DispatchProfileRow, DispatchRun, DispatchRunKind, DispatchStats } from './DispatchRunTypes.js';

/** Rohfakten der Auswertung, wie sie die Repositories liefern */
export interface DispatchStatsFacts {
  from: Date;
  /** ausschließlich */
  to: Date;
  kind: DispatchRunKind;
  /** Läufe aller Arten mit Start im Zeitraum */
  runs: DispatchRun[];
  runsSince: string | null;
  configChangedAt: string | null;
}

/** Erfolgreiche Ausgänge (Merge-Sitzung und Recherche zählen bei ihrer Art als Erfolg) */
const SUCCESS = new Set(['done', 'review', 'human', 'merge-session', 'research']);
const ABORTED = new Set(['abort', 'abort-final']);
/** Ausgänge, bei denen der Lauf auf einen Menschen wartet */
const WAITING = new Set(['question', 'blocked']);
/** Abbruchkategorien, die sich über die Einstellung beeinflussen lassen */
const CONFIGURABLE = new Set(['max-turns', 'timeout', 'context']);
/** Bis zu so vielen Tagen zeigt die Zeitreihe Tage statt Wochen */
const DAILY_MAX_DAYS = 28;

// Rechnet aus den Läufen die Kennzahlen des Dashboards (ohne Datenbank): Erfolg, Eskalation entlang der Profile, Abbruchgründe.
export class DispatchStatsCalculator {
  private readonly reasons = new AbortReasonClassifier();

  calculate(facts: DispatchStatsFacts): DispatchStats {
    const runs = facts.runs.filter((r) => r.kind === facts.kind);
    const done = runs.filter((r) => SUCCESS.has(r.outcome)).length;
    const aborted = runs.filter((r) => ABORTED.has(r.outcome)).length;
    const subjects = this.subjects(runs);
    const ladder = this.ladder(subjects);
    const solved = [...subjects.values()].filter((list) => list.some((r) => SUCCESS.has(r.outcome))).length;
    const abortReasons = this.abortReasons(runs);
    const configurable = abortReasons.filter((a) => CONFIGURABLE.has(a.category)).reduce((s, a) => s + a.count, 0);

    return {
      kind: facts.kind,
      runsSince: facts.runsSince,
      backfilledRuns: runs.filter((r) => r.source === 'comments').length,
      configChangedAt: facts.configChangedAt,
      kpis: {
        runs: runs.length,
        subjects: subjects.size,
        done, aborted, solved,
        successRate: this.rate(solved, ladder.finished),
        firstTryRate: ladder.firstTry,
        escalationRate: this.rate([...subjects.values()].filter((list) => list.some((r) => this.retried(r))).length, subjects.size),
        finalFailures: runs.filter((r) => r.outcome === 'abort-final').length,
      },
      series: this.series(facts, runs),
      byProfile: this.profiles(runs),
      ladder,
      abortReasons,
      configurableAbortShare: this.rate(configurable, aborted),
      usagePauses: facts.runs.filter((r) => r.outcome === 'pause-limit').length,
      staleRuns: facts.runs.filter((r) => r.abortCategory === 'vanished').length,
    };
  }

  // Läufe je Ticket bzw. Frage, älteste zuerst
  private subjects(runs: DispatchRun[]): Map<string, DispatchRun[]> {
    const map = new Map<string, DispatchRun[]>();
    for (const r of [...runs].sort((a, b) => a.startedAt.localeCompare(b.startedAt))) {
      map.set(r.subjectId, [...(map.get(r.subjectId) ?? []), r]);
    }
    return map;
  }

  // Lauf nach einem früheren Abbruch: eskaliert oder wiederholt
  private retried(r: DispatchRun): boolean {
    return r.escalatedFrom !== null || (r.attempt ?? 1) > 1;
  }

  // Abgeschlossen ist ein Gegenstand mit Erfolg oder endgültigem Abbruch; Erstversuch = erster Lauf erfolgreich und kein Wiederholungslauf
  private ladder(subjects: Map<string, DispatchRun[]>): DispatchStats['ladder'] {
    let firstTry = 0;
    let afterEscalation = 0;
    let final = 0;
    for (const list of subjects.values()) {
      const success = list.findIndex((r) => SUCCESS.has(r.outcome));
      if (success >= 0) {
        const clean = !list.slice(0, success + 1).some((r) => ABORTED.has(r.outcome) || this.retried(r));
        if (clean) firstTry += 1;
        else afterEscalation += 1;
      } else if (list.some((r) => r.outcome === 'abort-final')) {
        final += 1;
      }
    }
    const finished = firstTry + afterEscalation + final;
    return {
      finished,
      firstTry: this.rate(firstTry, finished),
      afterEscalation: this.rate(afterEscalation, finished),
      final: this.rate(final, finished),
    };
  }

  private rate(part: number, total: number): number | null {
    return total ? part / total : null;
  }

  // Zeitreihe vom Anfang bis zum Ende des Zeitraums, auch leere Abschnitte: bis DAILY_MAX_DAYS Tage je Tag, sonst je Woche
  // (Montag); Grenzen in Ortszeit
  private series(facts: DispatchStatsFacts, runs: DispatchRun[]): DispatchStats['series'] {
    const last = new Date(facts.to.getTime() - 1);
    const days = Math.round((this.midnight(last).getTime() - this.midnight(facts.from).getTime()) / 86_400_000) + 1;
    const unit = days <= DAILY_MAX_DAYS ? 'day' : 'week';
    const step = unit === 'day' ? 1 : 7;
    const startOf = (d: Date) => (unit === 'day' ? this.midnight(d) : this.monday(d));
    const buckets = new Map<string, { done: number; aborted: number; other: number }>();
    for (let d = startOf(facts.from); d <= last; d = new Date(d.getFullYear(), d.getMonth(), d.getDate() + step)) {
      buckets.set(this.day(d), { done: 0, aborted: 0, other: 0 });
    }
    for (const r of runs) {
      const b = buckets.get(this.day(startOf(new Date(r.startedAt))));
      if (!b) continue;
      if (SUCCESS.has(r.outcome)) b.done += 1;
      else if (ABORTED.has(r.outcome)) b.aborted += 1;
      else if (WAITING.has(r.outcome)) b.other += 1;
    }
    return { unit, points: [...buckets].map(([start, b]) => ({ start, ...b })) };
  }

  // 0:00 Ortszeit des Tages von `d`
  private midnight(d: Date): Date {
    return new Date(d.getFullYear(), d.getMonth(), d.getDate());
  }

  // Montag 0:00 Ortszeit der Woche von `d`
  private monday(d: Date): Date {
    const back = (d.getDay() + 6) % 7;
    return new Date(d.getFullYear(), d.getMonth(), d.getDate() - back);
  }

  // JJJJ-MM-TT in Ortszeit
  private day(d: Date): string {
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  }

  // Ausgänge je Profil, nach Zahl der Läufe, dann Name (die Reihenfolge der Leiter kennt erst die Anzeige)
  private profiles(runs: DispatchRun[]): DispatchProfileRow[] {
    const rows = new Map<string, DispatchProfileRow>();
    for (const r of runs) {
      const row = rows.get(r.profile) ?? { profile: r.profile, runs: 0, done: 0, escalated: 0, final: 0, other: 0 };
      row.runs += 1;
      if (SUCCESS.has(r.outcome)) row.done += 1;
      else if (r.outcome === 'abort') row.escalated += 1;
      else if (r.outcome === 'abort-final') row.final += 1;
      else row.other += 1;
      rows.set(r.profile, row);
    }
    return [...rows.values()].sort((a, b) => b.runs - a.runs || a.profile.localeCompare(b.profile));
  }

  private abortReasons(runs: DispatchRun[]): DispatchStats['abortReasons'] {
    const counts = new Map<string, number>();
    for (const r of runs) if (ABORTED.has(r.outcome)) counts.set(r.abortCategory ?? 'other', (counts.get(r.abortCategory ?? 'other') ?? 0) + 1);
    return [...counts].map(([category, count]) => ({ category, label: this.reasons.label(category), count }))
      .sort((a, b) => b.count - a.count || a.category.localeCompare(b.category));
  }
}
