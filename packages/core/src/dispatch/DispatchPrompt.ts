import type { DispatchConfig } from './DispatchConfig.js';
import { ACCEPTANCE_RULE, DISPATCH_PROMPT_DEFAULTS, LINK_RULE, TOOL_RULE, type DispatchPromptKind } from './DispatchPromptDefaults.js';
import { PromptTemplate, type PromptValues } from './PromptTemplate.js';

/** Entschiedene Frage zum Ticket, die der nächste Lauf kennen muss */
export interface AnsweredQuestion {
  id: string;
  title: string;
  /** Entscheidung als Text: Option bzw. Freitext und Begründung */
  decision: string;
}

export interface WorkPromptInput {
  ticketId: string;
  title: string;
  agent: string;
  branch: string;
  answers: AnsweredQuestion[];
  /** Rückmeldungen aus dem Review („Zurück an den Agenten“) seit dem letzten Start */
  feedback?: string[];
  cfg: DispatchConfig;
}

export interface MergePromptInput {
  ticketId: string;
  title: string;
  agent: string;
  branch: string;
  target: string;
  cfg: DispatchConfig;
}

export interface ResearchPromptInput {
  questionId: string;
  title: string;
  agent: string;
  cfg: DispatchConfig;
}

export interface PlanPromptInput {
  ticketId: string;
  title: string;
  agent: string;
  cfg: DispatchConfig;
}

// Arbeitsanweisungen für den Junior-Agent (Umsetzung), die Merge-Session und die Recherche einer Frage.
// Die Texte stehen als Vorlagen in DispatchPromptDefaults; hier entstehen nur noch die Werte dafür.
export class DispatchPrompt {
  private readonly template = new PromptTemplate();

  // Anweisung für einen Umsetzungslauf (Port von buildPrompt, auf „harness board …“ umgestellt)
  work(input: WorkPromptInput): string {
    const { ticketId: id, agent, cfg } = input;
    return this.render('work', cfg, {
      ticketId: id,
      title: input.title,
      agent,
      branch: input.branch,
      worktreeSetup: cfg.worktreeSetup.length
        ? `- Die Abhängigkeiten sind laut Einrichtung des Dispatchers installiert (${cfg.worktreeSetup.map((s) => `${s.cwd}$ ${s.run}`).join('; ')}).`
        : '',
      linkRule: LINK_RULE,
      toolRule: TOOL_RULE,
      answers: this.answers(input.answers),
      feedback: this.feedback(input.feedback),
      promptExtra: this.promptExtra(cfg),
    });
  }

  // Anweisung für einen Recherche-Lauf zu einer offenen Frage (losgelöster Worktree, nur lesend)
  research(input: ResearchPromptInput): string {
    return this.render('research', input.cfg, {
      questionId: input.questionId,
      title: input.title,
      agent: input.agent,
      promptExtra: this.promptExtra(input.cfg),
    });
  }

  // Anweisung für einen Planungslauf an einem Ticket („Planen“; losgelöster Worktree, nur lesend)
  plan(input: PlanPromptInput): string {
    return this.render('plan', input.cfg, {
      ticketId: input.ticketId,
      title: input.title,
      agent: input.agent,
      acceptanceRule: input.cfg.planning?.acceptance === false ? '' : ACCEPTANCE_RULE,
      promptExtra: this.promptExtra(input.cfg),
    });
  }

  // Anweisung für die Merge-Session im Worktree des Tickets
  merge(input: MergePromptInput): string {
    return this.render('merge', input.cfg, {
      ticketId: input.ticketId,
      title: input.title,
      agent: input.agent,
      branch: input.branch,
      target: input.target,
      linkRule: LINK_RULE,
      toolRule: TOOL_RULE,
      promptExtra: this.promptExtra(input.cfg),
    });
  }

  // Rendert die eigene Vorlage der Laufart, sonst die Standardvorlage. Scheitert die eigene Vorlage
  // wider Erwarten doch, gilt der Standard – an einer Vorlage darf kein Lauf zerbrechen.
  private render(kind: DispatchPromptKind, cfg: DispatchConfig, values: PromptValues): string {
    const own = cfg.prompts?.[kind];
    if (own) {
      try {
        return this.template.render(own, values);
      } catch {
        // auf den Standard zurückfallen
      }
    }
    return this.template.render(DISPATCH_PROMPT_DEFAULTS[kind], values);
  }

  // Die entschiedenen Fragen als Block; die trennende Leerzeile gehört zum Wert, damit die
  // Vorlage mit einer einzigen Zeile auskommt, die im leeren Fall ganz wegfällt.
  private answers(answers: AnsweredQuestion[]): string {
    if (!answers.length) return '';
    return [
      '',
      'Entscheidungen zu Fragen dieses Tickets (gelten vor Plan und Handbuch, wo sie ihnen widersprechen):',
      ...answers.map((a) => `- ${a.id} „${a.title}“: ${a.decision}`),
    ].join('\n');
  }

  // Die Rückmeldungen aus dem Review als Block; mehrzeilige Rückmeldungen werden eingerückt
  private feedback(feedback: string[] | undefined): string {
    if (!feedback?.length) return '';
    return [
      '',
      'Rückmeldung aus dem Review (Arbeitsanweisung, gilt vor Plan und Handbuch):',
      ...feedback.map((f) => `- ${f.replace(/\n/g, '\n  ')}`),
      'Ein Mensch hat deine Umsetzung angesehen und zurückgegeben. Setze die Rückmeldung im selben Branch um, dann wieder „erledigt“.',
    ].join('\n');
  }

  // Die Zusatz-Anweisung aus der Einstellung samt trennender Leerzeile; leer, wenn keine eingestellt ist
  private promptExtra(cfg: DispatchConfig): string {
    return cfg.promptExtra.trim() ? `\n\n${cfg.promptExtra.trim()}` : '';
  }
}
