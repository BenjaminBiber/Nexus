import type { SubagentDto } from '../api/types.js';
import { formatDuration } from '../sessions/format.js';
import { Badge } from './Badge.js';

// Subagenten einer Session als eingerückte Zeilen mit Typ und Laufzeit, älteste zuerst; beendete nur mit showEnded.
export function SubagentTree({ agents, label, now, showEnded = false }: { agents: SubagentDto[]; label: string; now: number; showEnded?: boolean }) {
  const active = agents
    .filter((agent) => agent.state === 'running' || showEnded)
    .sort((a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime());
  if (active.length === 0) return null;
  return (
    <ul className="subagents" aria-label={`Subagenten von ${label}`}>
      {active.map((agent) => {
        const end = agent.endedAt ? new Date(agent.endedAt).getTime() : now;
        return (
          <li key={agent.id}>
            <span aria-hidden="true" className="branch">
              └
            </span>
            {agent.state === 'running' ? <Badge kind="running">läuft</Badge> : <Badge>beendet</Badge>}
            <span className="mono">{formatDuration(end - new Date(agent.startedAt).getTime())}</span>
            <span>
              Subagent „<span className="mono">{agent.type}</span>“
            </span>
          </li>
        );
      })}
    </ul>
  );
}
