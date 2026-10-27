import type { Ticket } from '../board/BoardTypes.js';
import type { DispatchProfileReason } from '../board/DispatchRunTypes.js';
import type { DispatchConfig, Rule, RunSettings } from './DispatchConfig.js';
import { DispatchConfigNormalizer } from './DispatchConfigNormalizer.js';
import { DispatchConfigFingerprint } from './DispatchConfigFingerprint.js';

type ProfileTicket = Pick<Ticket, 'labels' | 'priority' | 'plan'>;

export interface DispatchLabels {
  profile: string | null;
  model: string | null;
  effort: string | null;
}

// Wählt Modell, Effort, Schrittzahl und Zeitlimit eines Laufs: Label profil:, Aufwand im Plan, Regeln,
// Standardprofil; danach Eskalation je Abbruch und Labels model:/effort: (Port von chooseRun).
export class ProfileChooser {
  private readonly normalizer = new DispatchConfigNormalizer();
  private readonly fingerprint = new DispatchConfigFingerprint();
  // Hash der Einstellung dieses Choosers, einmal berechnet: jeder Lauf trägt die Fassung, die bei seiner Wahl galt
  private hashValue: string | null = null;

  constructor(private readonly cfg: DispatchConfig) {}

  // Hash der Einstellung, unter der dieser Chooser wählt (Version fürs Dashboard)
  get configHash(): string {
    return (this.hashValue ??= this.fingerprint.hash(this.cfg));
  }

  // Labels profil:, model: und effort: – jeweils das erste
  labels(ticket: Pick<Ticket, 'labels'>): DispatchLabels {
    const value = (prefix: string) => {
      const label = ticket.labels.find((l) => l.startsWith(prefix));
      return label ? label.slice(prefix.length).trim() || null : null;
    };
    return { profile: value('profil:'), model: value('model:'), effort: value('effort:') };
  }

  // Ein Label, mit dem nicht gestartet werden kann – lieber stehen lassen als raten
  labelProblem(ticket: Pick<Ticket, 'labels'>): string | null {
    const l = this.labels(ticket);
    if (l.profile && !this.cfg.profiles[l.profile]) {
      return `Label profil:${l.profile} – dieses Profil gibt es nicht (${Object.keys(this.cfg.profiles).join(', ')})`;
    }
    if (l.model) {
      const problem = this.normalizer.modelProblem(l.model, this.cfg.allowFable);
      if (problem) return `Label model:${l.model} – ${problem}`;
    }
    if (l.effort) {
      const problem = this.normalizer.effortProblem(l.effort);
      if (problem) return `Label effort:${l.effort} – ${problem}`;
    }
    return null;
  }

  // Zeilen der Tabelle unter „## Integration“ (ohne Kopf und Trenner)
  planFileCount(plan: string): number {
    const lines = plan.split(/\r?\n/);
    const start = lines.findIndex((l) => /^##\s+Integration\b/i.test(l.trim()));
    if (start < 0) return 0;
    let rows = 0;
    let separator = false;
    for (const line of lines.slice(start + 1)) {
      const t = line.trim();
      if (/^##\s/.test(t)) break;
      if (!t.startsWith('|')) continue;
      if (/^\|[\s:|-]+\|?$/.test(t)) {
        separator = true;
        continue;
      }
      if (separator) rows += 1;
    }
    return rows;
  }

  // Einschätzung des Planners: Zeile „Aufwand: <profil>“ (auch fett) oder Abschnitt „## Aufwand“; klein geschrieben
  planEstimate(plan: string): string | null {
    const line = /^[ \t]*\**Aufwand\**[ \t]*:\**[ \t]*\**([\p{L}\w-]+)/imu.exec(plan);
    if (line) return line[1].toLowerCase();
    const section = /^##[ \t]+Aufwand[ \t]*\r?\n\s*\**([\p{L}\w-]+)/imu.exec(plan);
    return section ? section[1].toLowerCase() : null;
  }

  // Einstellungen für Versuch `attempt` (ab 1)
  choose(ticket: ProfileTicket, attempt = 1): RunSettings {
    const labels = this.labels(ticket);
    let profile = this.cfg.defaultProfile;
    let why = `Standardprofil ${profile}`;
    // Grund der Wahl des Basisprofils fürs Dashboard; Eskalation und Labels model:/effort: ändern ihn nicht
    let reason: DispatchProfileReason = 'default';
    let ruleIndex: number | undefined;
    const estimate = this.planEstimate(ticket.plan);
    if (labels.profile && this.cfg.profiles[labels.profile]) {
      profile = labels.profile;
      why = `Label profil:${profile}`;
      reason = 'label';
    } else if (estimate && this.cfg.profiles[estimate]) {
      // Der Planner hat den Code gerade gelesen – seine Einschätzung schlägt jede Regel
      profile = estimate;
      why = `Aufwand laut Plan: ${profile}`;
      reason = 'plan';
    } else {
      const index = this.cfg.rules.findIndex((r) => this.matches(r, ticket));
      if (index >= 0) {
        profile = this.cfg.rules[index].profile;
        why = `Regel ${index + 1} → ${profile}`;
        reason = 'rule';
        ruleIndex = index + 1;
      }
      if (estimate) why += ` (Aufwand „${estimate}“ im Plan ist kein Profil)`;
    }
    let escalatedFrom: string | undefined;
    if (this.cfg.escalate && attempt > 1) {
      const at = this.cfg.ladder.indexOf(profile);
      if (at >= 0) {
        const target = this.cfg.ladder[Math.min(at + attempt - 1, this.cfg.ladder.length - 1)];
        if (target !== profile) {
          why += `, Versuch ${attempt}: eskaliert auf ${target}`;
          escalatedFrom = profile;
          profile = target;
        }
      }
    }
    const run: RunSettings = {
      ...this.cfg.profiles[profile], profile, why, reason, configHash: this.configHash, ...(ruleIndex != null ? { ruleIndex } : {}),
      ...(escalatedFrom ? { escalatedFrom } : {}),
    };
    if (labels.model && !this.normalizer.modelProblem(labels.model, this.cfg.allowFable)) {
      run.model = labels.model;
      run.why += `, Label model:${labels.model}`;
    }
    if (labels.effort && !this.normalizer.effortProblem(labels.effort)) {
      run.effort = labels.effort;
      run.why += `, Label effort:${labels.effort}`;
    }
    // Haiku kennt keinen Effort
    if (run.model && /(^|-)haiku/.test(run.model)) run.effort = null;
    return run;
  }

  // Einstellungen eines Recherche-Laufs: research.profile, sonst Profil „research“, sonst Standardprofil
  research(): RunSettings {
    const { cfg } = this;
    const [profile, why, reason] = cfg.research.profile
      ? [cfg.research.profile, `Einstellung research.profile: ${cfg.research.profile}`, 'setting' as const]
      : cfg.profiles.research
        ? ['research', 'Profil research', 'setting' as const]
        : [cfg.defaultProfile, `Standardprofil ${cfg.defaultProfile}`, 'default' as const];
    const run: RunSettings = { ...cfg.profiles[profile], profile, why, reason, configHash: this.configHash };
    if (run.model && /(^|-)haiku/.test(run.model)) run.effort = null;
    return run;
  }

  // Einstellungen eines Planungslaufs: planning.profile, sonst Profil „planning“, sonst Standardprofil
  planning(): RunSettings {
    const { cfg } = this;
    const [profile, why, reason] = cfg.planning.profile
      ? [cfg.planning.profile, `Einstellung planning.profile: ${cfg.planning.profile}`, 'setting' as const]
      : cfg.profiles.planning
        ? ['planning', 'Profil planning', 'setting' as const]
        : [cfg.defaultProfile, `Standardprofil ${cfg.defaultProfile}`, 'default' as const];
    const run: RunSettings = { ...cfg.profiles[profile], profile, why, reason, configHash: this.configHash };
    if (run.model && /(^|-)haiku/.test(run.model)) run.effort = null;
    return run;
  }

  // Trifft die Regel zu? Alle genannten Bedingungen müssen passen
  private matches(rule: Rule, ticket: ProfileTicket): boolean {
    const w = rule.when;
    if (w.labels && !w.labels.some((l) => ticket.labels.includes(l))) return false;
    if (w.priority && !w.priority.includes(ticket.priority)) return false;
    if (w.filesMin != null && this.planFileCount(ticket.plan) < w.filesMin) return false;
    if (w.planMinChars != null && ticket.plan.length < w.planMinChars) return false;
    return true;
  }
}
