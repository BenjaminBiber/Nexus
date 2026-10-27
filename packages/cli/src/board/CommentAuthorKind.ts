// Namen der Agenten, die selbst Kommentare schreiben
const AGENT_NAMES = ['claude', 'dispatcher'];
// Präfixe der vom Dispatcher gestarteten Agenten (siehe DispatchService.agentPrefix, PlanRunner, ResearchRunner)
const AGENT_PREFIXES = ['junior-', 'planner-', 'research-'];

// Unterscheidet Kommentare von Agenten und von Menschen.
export class CommentAuthorKind {
  // Wahr, wenn der Name zu einem Agenten gehört
  isAgent(authorName: string): boolean {
    const name = authorName.trim().toLowerCase();
    if (!name) return false;
    return AGENT_NAMES.includes(name) || AGENT_PREFIXES.some((p) => name.startsWith(p));
  }
}
