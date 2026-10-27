import { createHash } from 'node:crypto';
import type { DispatchConfig } from './DispatchConfig.js';

// Teile der Einstellung, die in den Hash gehören; alles andere (Fenster, WIP, Boards, Namen) ändert das Ergebnis eines Laufs nicht
const KEYS: readonly (keyof DispatchConfig)[] = [
  'profiles', 'defaultProfile', 'rules', 'escalate', 'ladder', 'maxAttempts', 'allowFable', 'promptExtra', 'prompts',
  'mergeStrategy', 'reviewBeforeMerge', 'joinWorktrees', 'worktreeSetup', 'worktreeSetupTimeoutMinutes',
];

// Teile von claude, die ein Lauf tatsächlich spürt (command, useSubscription und gitBashPath gehören zur Maschine, nicht zum Lauf)
const CLAUDE_KEYS = ['model', 'effort', 'maxTurns', 'timeoutMinutes', 'permissionMode', 'allowedTools', 'extraArgs'] as const;

// Hash der wirksamen Dispatcher-Einstellung: benennt die Fassung, unter der ein Lauf gestartet ist.
// Nur Werte, die das Ergebnis beeinflussen – Fenster, WIP oder Boards ändern den Hash nicht.
export class DispatchConfigFingerprint {
  // Die beachteten Teile als JSON mit sortierten Schlüsseln; Arrays behalten ihre Reihenfolge (rules und ladder sind geordnet)
  content(cfg: DispatchConfig): string {
    const subset: Record<string, unknown> = {};
    for (const key of KEYS) subset[key] = cfg[key];
    const claude: Record<string, unknown> = {};
    for (const key of CLAUDE_KEYS) claude[key] = cfg.claude[key];
    subset.claude = claude;
    subset.research = { profile: cfg.research.profile };
    subset.planning = { profile: cfg.planning.profile };
    return JSON.stringify(this.sorted(subset));
  }

  // Die ersten 16 Hex-Zeichen des SHA-256 über content – kurz genug für Anzeige und Filter, lang genug gegen Kollisionen
  hash(cfg: DispatchConfig): string {
    return createHash('sha256').update(this.content(cfg)).digest('hex').slice(0, 16);
  }

  // Objekte mit sortierten Schlüsseln neu aufbauen, damit die Reihenfolge in der Datei den Hash nicht ändert
  private sorted(value: unknown): unknown {
    if (Array.isArray(value)) return value.map((v) => this.sorted(v));
    if (value === null || typeof value !== 'object') return value;
    const out: Record<string, unknown> = {};
    for (const key of Object.keys(value as Record<string, unknown>).sort()) out[key] = this.sorted((value as Record<string, unknown>)[key]);
    return out;
  }
}
