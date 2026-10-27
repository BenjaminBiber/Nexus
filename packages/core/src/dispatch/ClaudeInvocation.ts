import {
  PLANNING_DENIED_TOOLS, PLANNING_TOOLS, RESEARCH_DENIED_TOOLS, RESEARCH_PERMISSION_MODE, RESEARCH_TOOLS, type DispatchConfig, type RunSettings,
} from './DispatchConfig.js';
import type { RunKind } from './state/RunStore.js';

const BASH_DEFAULT_TIMEOUT_MS = 15 * 60_000;
const BASH_MAX_TIMEOUT_MS = 30 * 60_000;

// Argumente und Umgebung für `claude -p`; die Arbeitsanweisung kommt über stdin.
export class ClaudeInvocation {
  // Argumentliste. --disallowedTools und --allowedTools nehmen beide alle folgenden Werte bis zur nächsten Option, daher stehen
  // sie am Ende (Verbote vor den Erlaubnissen, die Liste der Erlaubnisse zuletzt). Eine Recherche (`kind` research) übernimmt
  // aus claude.* nur den Aufruf: fester Rechte-Modus „default“, keine extraArgs, nur RESEARCH_TOOLS, dazu die Verbote.
  // Eine Planung (`kind` plan) genauso, mit PLANNING_TOOLS und PLANNING_DENIED_TOOLS
  args(cfg: DispatchConfig, run: RunSettings, extraTools: string[] = [], kind: RunKind = 'work'): string[] {
    const c = cfg.claude;
    const research = kind === 'research' || kind === 'plan';
    const base = kind === 'research' ? RESEARCH_TOOLS : kind === 'plan' ? PLANNING_TOOLS : c.allowedTools;
    const denied = kind === 'plan' ? PLANNING_DENIED_TOOLS : RESEARCH_DENIED_TOOLS;
    const tools = [...base, ...extraTools.filter((t) => !base.includes(t))];
    const mode = research ? RESEARCH_PERMISSION_MODE : c.permissionMode;
    return [
      '-p',
      // stream-json schreibt jedes Ereignis sofort, so wächst das Protokoll, während Claude arbeitet; ohne Terminal verlangt es --verbose
      '--output-format', 'stream-json',
      '--verbose',
      '--max-turns', String(run.maxTurns),
      ...(run.model ? ['--model', run.model] : []),
      ...(run.effort ? ['--effort', run.effort] : []),
      ...(mode ? ['--permission-mode', mode] : []),
      ...(research ? [] : c.extraArgs.filter((a) => a !== '--verbose')),
      ...(research ? ['--disallowedTools', ...denied] : []),
      ...(tools.length ? ['--allowedTools', ...tools] : []),
    ];
  }

  // Umgebung des Workers (gitBash: Pfad der Git Bash unter Windows, sonst null): ohne API-Key (Abrechnung über die Anmeldung), ohne fremde Session (auch keine Claude-Code-Kennung), mit HARNESS_TICKET (ohne Ticket, z. B. Recherche: ohne)
  env(env: Record<string, string | undefined>, cfg: DispatchConfig, ticketId: string | null, gitBash: string | null = null): Record<string, string | undefined> {
    const out = { ...env };
    if (cfg.claude.useSubscription) {
      delete out.ANTHROPIC_API_KEY;
      delete out.ANTHROPIC_AUTH_TOKEN;
    }
    // Der Server kann aus einer (Claude-)Session gestartet sein; der Worker ist eine eigene
    delete out.HARNESS_SESSION;
    delete out.CLAUDECODE;
    delete out.CLAUDE_CODE_ENTRYPOINT;
    if (ticketId) out.HARNESS_TICKET = ticketId;
    else delete out.HARNESS_TICKET;
    // Längere Bash-Zeitlimits (sonst 2 bzw. 10 min): ein Befehl darüber landet im Hintergrund, und mit `claude -p` endet der
    // Lauf, bevor er fertig ist. Eigene Werte aus der Umgebung des Servers bleiben
    out.BASH_DEFAULT_TIMEOUT_MS ??= String(BASH_DEFAULT_TIMEOUT_MS);
    out.BASH_MAX_TIMEOUT_MS ??= String(BASH_MAX_TIMEOUT_MS);
    // Unter Windows nutzt Claude Code sonst das PowerShell-Tool, für das die Bash-Regeln (`Bash(harness:*)`) nicht gelten
    if (gitBash) {
      out.CLAUDE_CODE_USE_POWERSHELL_TOOL = '0';
      out.CLAUDE_CODE_GIT_BASH_PATH = gitBash;
    }
    return out;
  }
}
