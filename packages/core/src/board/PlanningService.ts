import { HarnessError } from '../errors/HarnessError.js';
import type { BoardDatabase } from './BoardDatabase.js';
import { MAX_PERSON_NAME_LENGTH, PLANNING_CLAIM_TTL_MS, type BoardActor, type Ticket, type TicketStatus } from './BoardTypes.js';
import { BoardLimits } from './BoardLimits.js';
import { PlanningCandidate } from './PlanningCandidate.js';
import { TicketService, type TicketView } from './TicketService.js';

export interface PlanInput {
  plan?: string;
  handbook?: string;
  diagram?: string;
  /** Abnahme des Tickets; fehlt sie, bleibt die vorhandene */
  acceptance?: string;
  status?: TicketStatus;
}

// Planungslauf: Plan/Handbuch/Diagramm schreiben und das nächste Ticket mit Planungsbedarf holen.
export class PlanningService {
  private readonly tickets: TicketService;

  constructor(private readonly db: BoardDatabase, private readonly clock: () => Date) {
    this.tickets = new TicketService(db, clock);
  }

  // Schreibt die Texte, setzt den Status (Standard: Geplant), löscht Planungsbedarf und Claim.
  // Ohne Sperren für Abhängigkeiten und Abnahme (wie im alten Board).
  plan(id: string, input: PlanInput, actor: BoardActor): Promise<TicketView> {
    if (input.plan === undefined && input.handbook === undefined && input.diagram === undefined) {
      return Promise.reject(new HarnessError('plan-empty', 'Mindestens Plan, Handbuch oder Diagramm angeben.'));
    }
    return this.db.transaction(async (u) => {
      const before = await this.tickets.load(u, id, undefined, true);
      const status = input.status ?? 'planned';
      const after: Ticket = {
        ...before,
        plan: input.plan ?? before.plan,
        handbook: input.handbook ?? before.handbook,
        diagram: input.diagram === undefined ? before.diagram : this.stripFence(input.diagram),
        acceptance: input.acceptance?.trim() ? input.acceptance : before.acceptance,
        status,
        needsPlanning: false,
        planningClaimBy: null,
        planningClaimAt: null,
        plannedAt: this.clock().toISOString(),
        plannedBy: actor.name,
      };
      if (status !== before.status) after.sortOrder = await this.tickets.nextOrder(u, before.boardId, status);
      const note = status !== before.status ? 'geplant' : 'Plan aktualisiert';
      return this.tickets.view(u, await this.tickets.save(u, before, after, actor, note));
    });
  }

  // Nächstes Ticket mit Planungsbedarf (Anzeige-Reihenfolge); mit claimAgent wird es 15 Minuten beansprucht.
  // Beim Claim wird jeder Kandidat gesperrt und erneut geprüft; hat ihn inzwischen ein anderer beansprucht,
  // kommt der nächste dran. null, wenn keiner übrig bleibt.
  next(boardIds: number[], claimAgent: string | null, actor: BoardActor): Promise<TicketView | null> {
    const agent = claimAgent === null ? null : claimAgent.trim();
    if (agent === '') return Promise.reject(new HarnessError('agent-missing', 'Für den Claim fehlt der Name des Agenten (--claim).'));
    return this.db.transaction(async (u) => {
      const now = this.clock().getTime();
      if (agent) BoardLimits.text(agent, MAX_PERSON_NAME_LENGTH, 'Der Name des Agenten');
      // Innerhalb der Transaktion nur Methoden mit „u“ nutzen (list() würde eine zweite Transaktion öffnen)
      const views = this.tickets.sortViews(await this.tickets.views(u, await u.tickets.list({ boardIds })));
      const candidates = views.filter((t) => PlanningCandidate.matches(t, now, t.waitingOnQuestions.length));
      if (!agent) return candidates[0] ?? null;
      for (const candidate of candidates) {
        // Gesperrt neu lesen: gelöscht oder inzwischen beansprucht → nächster Kandidat
        const before = await u.tickets.byId(candidate.id, true);
        if (!before || !PlanningCandidate.matches(before, now, candidate.waitingOnQuestions.length)) continue;
        const after: Ticket = { ...before, planningClaimBy: agent, planningClaimAt: new Date(now).toISOString() };
        return this.tickets.view(u, await this.tickets.save(u, before, after, actor, null));
      }
      return null;
    });
  }

  // Beansprucht genau dieses Ticket 15 Minuten für die Planung („Planen“ im Ticket). Ein lebender Claim eines anderen
  // Agenten → HarnessError „plan-claimed“; der eigene wird erneuert
  claim(id: string, agent: string, actor: BoardActor): Promise<TicketView> {
    const name = agent.trim();
    if (!name) return Promise.reject(new HarnessError('agent-missing', 'Für den Claim fehlt der Name des Agenten.'));
    return this.db.transaction(async (u) => {
      BoardLimits.text(name, MAX_PERSON_NAME_LENGTH, 'Der Name des Agenten');
      const before = await this.tickets.load(u, id, undefined, true);
      const now = this.clock().getTime();
      const at = before.planningClaimAt ? Date.parse(before.planningClaimAt) : NaN;
      if (before.planningClaimBy && before.planningClaimBy !== name && now - at < PLANNING_CLAIM_TTL_MS) {
        throw new HarnessError('plan-claimed', `${before.id} wird gerade von ${before.planningClaimBy} geplant.`);
      }
      const after: Ticket = { ...before, planningClaimBy: name, planningClaimAt: new Date(now).toISOString() };
      return this.tickets.view(u, await this.tickets.save(u, before, after, actor, null));
    });
  }

  // Gibt den Planungs-Claim frei, wenn er (noch) diesem Agenten gehört; sonst bleibt alles, wie es ist
  release(id: string, agent: string, actor: BoardActor): Promise<void> {
    return this.db.transaction(async (u) => {
      const before = await this.tickets.load(u, id, undefined, true);
      if (before.planningClaimBy !== agent) return;
      await this.tickets.save(u, before, { ...before, planningClaimBy: null, planningClaimAt: null }, actor, null);
    });
  }

  // Entfernt einen umschließenden ```mermaid-Block
  private stripFence(diagram: string): string {
    const match = /^\s*```(?:mermaid)?\s*\n([\s\S]*?)\n\s*```\s*$/.exec(diagram);
    return match ? match[1] : diagram;
  }
}
