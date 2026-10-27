import { existsSync } from 'node:fs';
import {
  BranchCommitReader, DispatchConfigStore, DispatchLog, RunLogOutcome, TimelineClassifier, sortTimeline,
  type HarnessHome, type TimelineAgentNames, type TimelineEvent,
} from '@harness/core';
import type { BoardRuntimeHolder } from './BoardRuntimeHolder.js';

// Antwort der Route: Ereignisse und je lokaler Quelle, ob sie auf diesem Rechner lesbar war
export interface TicketTimelineDto {
  ticketId: string;
  events: TimelineEvent[];
  sources: { runs: 'ok' | 'unavailable'; git: 'ok' | 'unavailable' | 'error'; gitMessage?: string };
}

const RUN_KIND_LABEL = { work: 'Umsetzung', merge: 'Merge-Session', research: 'Recherche', plan: 'Planung' } as const;

// Setzt die Zeitleiste eines Tickets zusammen: Ereignisse der Board-Datenbank (Kern) plus rechnerlokale Quellen –
// Lauf-Protokolle des Dispatchers (~/.harness/dispatch-logs) und Commits des Ticket-Branches im Repo des Boards.
export class TicketTimelineAssembler {
  private readonly logs: DispatchLog;
  private readonly outcome = new RunLogOutcome();
  private readonly commits = new BranchCommitReader();

  constructor(private readonly holder: BoardRuntimeHolder, private readonly home: HarnessHome) {
    this.logs = new DispatchLog(home);
  }

  async get(id: string): Promise<TicketTimelineDto> {
    const names = await this.names();
    const classifier = new TimelineClassifier(names);
    return this.holder.run(async (b) => {
      const timeline = await b.services.timeline(names).get(id);
      const ticket = await b.services.tickets.get(timeline.ticketId);
      const board = await b.services.boards.find(ticket.boardId);
      const runs = await this.runEvents(timeline.ticketId, names, ticket.createdAt);
      const mergeCommit = timeline.events.filter((e) => e.type === 'merge' && e.ref?.commit).at(-1)?.ref?.commit ?? null;
      const git = this.commits.read(board?.repoPath ?? null, { branch: ticket.branch, mergeCommit, targetBranch: ticket.targetBranch });
      const commitEvents: TimelineEvent[] = git.commits.map((c) => ({
        at: c.at, kind: 'git', type: 'commit', actor: classifier.actor(c.author), text: c.subject, ref: { commit: c.hash },
      }));
      const sources: TicketTimelineDto['sources'] = { runs: runs ? 'ok' : 'unavailable', git: git.status };
      if (git.message && git.status !== 'ok') sources.gitMessage = git.message;
      return { ticketId: timeline.ticketId, events: sortTimeline([...timeline.events, ...(runs ?? []), ...commitEvents]), sources };
    });
  }

  // Start und Ende je Lauf-Protokoll; null, wenn es auf diesem Rechner keinen Protokollordner gibt. Läufe von vor der
  // Anlage des Tickets gehören zu einem früheren Ticket mit derselben ID und fallen weg (der Name trägt nur ganze Sekunden)
  private async runEvents(ticketId: string, names: TimelineAgentNames | null, createdAt: string): Promise<TimelineEvent[] | null> {
    if (!existsSync(this.home.dispatchLogsDir)) return null;
    const dispatcher = names?.dispatcherName ?? 'dispatcher';
    const created = Math.floor(Date.parse(createdAt) / 1000) * 1000;
    const events: TimelineEvent[] = [];
    for (const run of await this.logs.runs(ticketId)) {
      if (!run.started || Date.parse(run.started) < created) continue;
      const outcome = this.outcome.read(await this.logs.read(run.name).catch(() => ''));
      const actor = run.kind === 'work'
        ? { name: `${names?.agentPrefix ?? 'junior-'}${ticketId}`, type: 'agent' as const }
        : { name: dispatcher, type: 'agent' as const };
      const label = RUN_KIND_LABEL[run.kind];
      const result = outcome.state === 'running'
        ? (outcome.timedOut ? 'Zeitlimit erreicht' : 'läuft oder ohne Ende im Protokoll')
        : `Exit-Code ${outcome.exitCode ?? '–'}${outcome.timedOut ? ', Zeitlimit' : ''}`;
      const ref = { runLog: run.name, exitCode: outcome.exitCode };
      const startText = outcome.state === 'running' ? `${label} gestartet (${result})` : `${label} gestartet`;
      events.push({ at: run.started, kind: 'run', type: 'run-log', actor, text: startText, ref });
      if (outcome.endedAt) events.push({ at: outcome.endedAt, kind: 'run', type: 'run-end', actor, text: `${label} beendet: ${result}`, ref });
    }
    return events;
  }

  // Namen aus der Dispatch-Konfiguration; ohne lesbare Datei die Standardwerte, bei Fehler null
  private async names(): Promise<TimelineAgentNames | null> {
    try {
      const { config } = await new DispatchConfigStore(this.home).read();
      return { dispatcherName: config.dispatcherName, agentPrefix: config.agentPrefix };
    } catch {
      return null;
    }
  }
}
