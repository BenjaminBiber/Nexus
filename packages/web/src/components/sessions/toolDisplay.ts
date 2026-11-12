// Darstellung eines Tool-Aufrufs im Verlauf: Kurzname, Symbol (Material Icons) und Kurzinfo ohne doppelten Namen.

const ICONS: Record<string, string> = {
  Bash: 'terminal',
  BashOutput: 'terminal',
  KillShell: 'cancel',
  PowerShell: 'terminal',
  Read: 'description',
  Edit: 'edit',
  MultiEdit: 'edit',
  Write: 'note_add',
  NotebookEdit: 'edit_note',
  Grep: 'search',
  Glob: 'folder_open',
  Task: 'smart_toy',
  Agent: 'smart_toy',
  WebFetch: 'public',
  WebSearch: 'travel_explore',
  TodoWrite: 'checklist',
  Skill: 'auto_awesome',
  ToolSearch: 'manage_search',
  ExitPlanMode: 'flag',
  AskUserQuestion: 'help',
};

// MCP-Tools heißen mcp__<server>__<tool>; angezeigt wird „server · tool“
export function toolDisplayName(name: string): string {
  const match = /^mcp__(.+?)__(.+)$/.exec(name);
  if (!match) return name;
  // Server-Kennungen sind teils UUIDs oder lange Plugin-Namen: nur den letzten Teil zeigen
  const server = match[1].split(/[_:]/).filter(Boolean).at(-1) ?? match[1];
  return `${server.length > 24 ? `${server.slice(0, 8)}…` : server} · ${match[2]}`;
}

export function toolIcon(name: string): string {
  if (name.startsWith('mcp__')) return 'extension';
  return ICONS[name] ?? 'build';
}

// Kurzinfo ohne vorangestellten Tool-Namen („Bash: npm test“ → „npm test“), da der Name daneben steht
export function toolDetail(name: string, summary: string): string {
  for (const prefix of [name, toolDisplayName(name)]) {
    if (summary.startsWith(prefix)) {
      const rest = summary.slice(prefix.length).replace(/^[:\s·-]+/, '');
      if (rest) return rest;
    }
  }
  return summary === name ? '' : summary;
}

// Eingabe als Objekt (Tools liefern fast immer eins); sonst null
export function inputObject(input: unknown): Record<string, unknown> | null {
  return input && typeof input === 'object' && !Array.isArray(input) ? (input as Record<string, unknown>) : null;
}

export function stringField(input: Record<string, unknown> | null, key: string): string | null {
  const value = input?.[key];
  return typeof value === 'string' ? value : null;
}
