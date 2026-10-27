export type PromptKind = 'work' | 'plan' | 'research' | 'merge';

export interface RequiredPart {
  id: string;
  label: string;
}

interface Rule extends RequiredPart {
  test: RegExp;
}

// Platzhalter in der Vorlage, nicht das fertige Ergebnis
const ph = (name: string) => `\\{\\{${name}\\}\\}`;
// Toleriert mehr Leerraum und zusätzliche Flags zwischen den Wörtern eines Befehls
const cmd = (...words: string[]) => new RegExp(words.join('\\s+(?:--?\\S+\\s+)*'));

const RULES: Record<PromptKind, Rule[]> = {
  work: [
    { id: 'ticketId', label: 'Platzhalter {{ticketId}}', test: new RegExp(ph('ticketId')) },
    { id: 'agent', label: 'Platzhalter {{agent}}', test: new RegExp(ph('agent')) },
    { id: 'done', label: 'Ausgang erledigt: harness board done {{ticketId}}', test: cmd('harness', 'board', 'done', ph('ticketId')) },
    { id: 'blocked', label: 'Ausgang blockiert: harness board set {{ticketId}} --status blocked', test: cmd('harness', 'board', 'set', ph('ticketId'), '--status', 'blocked') },
    { id: 'question', label: 'Ausgang Frage: harness board question add … --ticket {{ticketId}}', test: cmd('harness', 'board', 'question', 'add', '[\\s\\S]*?--ticket', ph('ticketId')) },
    { id: 'by-agent', label: 'Kennzeichnung --by {{agent}}', test: cmd('--by', ph('agent')) },
  ],
  plan: [
    { id: 'ticketId', label: 'Platzhalter {{ticketId}}', test: new RegExp(ph('ticketId')) },
    { id: 'agent', label: 'Platzhalter {{agent}}', test: new RegExp(ph('agent')) },
    { id: 'plan-json', label: 'Speichern: harness board plan-json {{ticketId}}', test: cmd('harness', 'board', 'plan-json', ph('ticketId')) },
    { id: 'plan-file', label: 'Datei plan.json', test: /plan\.json/ },
  ],
  research: [
    { id: 'questionId', label: 'Platzhalter {{questionId}}', test: new RegExp(ph('questionId')) },
    { id: 'agent', label: 'Platzhalter {{agent}}', test: new RegExp(ph('agent')) },
    { id: 'question-research', label: 'Speichern: harness board question research {{questionId}}', test: cmd('harness', 'board', 'question', 'research', ph('questionId')) },
    { id: 'research-file', label: 'Datei research.json', test: /research\.json/ },
  ],
  merge: [
    { id: 'ticketId', label: 'Platzhalter {{ticketId}}', test: new RegExp(ph('ticketId')) },
    { id: 'agent', label: 'Platzhalter {{agent}}', test: new RegExp(ph('agent')) },
    { id: 'branch', label: 'Platzhalter {{branch}}', test: new RegExp(ph('branch')) },
    { id: 'target', label: 'Platzhalter {{target}}', test: new RegExp(ph('target')) },
    { id: 'merge-abort', label: 'Rückzug: git merge --abort', test: cmd('git', 'merge', '--abort') },
  ],
};

// Prüft eine Prompt-Vorlage auf die Pflichtteile ihrer Laufart. Diese Teile sind der Vertrag
// zwischen Dispatcher und Agent; fehlen sie, enden Läufe still als „ohne Ergebnis“.
export class PromptTemplateCheck {
  static readonly KINDS: PromptKind[] = ['work', 'plan', 'research', 'merge'];

  // Alle Pflichtteile einer Laufart, für Oberfläche und Doku
  parts(kind: PromptKind): RequiredPart[] {
    return RULES[kind].map(({ id, label }) => ({ id, label }));
  }

  // Die Pflichtteile, die in der Vorlage nicht vorkommen
  missing(kind: PromptKind, template: string): RequiredPart[] {
    return RULES[kind].filter((rule) => !rule.test.test(template)).map(({ id, label }) => ({ id, label }));
  }
}
