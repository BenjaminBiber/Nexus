// Ablauf-Vorlage für die Tests. Den echten Ablauf liefert der Server aus Rust
// (GET /api/workflow, Quelle `crates/nexus-core/src/workflow/phases.rs`); hier steht so viel
// davon, wie die Oberfläche zum Anzeigen braucht. Was im Ablauf selbst steht, prüft Rust.
import type { WorkflowPhase } from '../../src/api/generated/workflow.js';

export const TEST_WORKFLOW_PHASES: WorkflowPhase[] = [
  {
    id: 'planen',
    title: 'Neue Funktion planen',
    hint: 'In Claude Code, einmal pro Funktion.',
    steps: [
      {
        step: 'Anforderungen klären',
        command: '/superpowers:brainstorming',
        when: 'Einfach: erkundet Idee und Entwurf im Dialog.',
        options: [
          { label: 'Brainstorming', command: '/superpowers:brainstorming', when: 'Einfach: erkundet Idee und Entwurf im Dialog.' },
          { label: 'Grill', command: '/nexus:harness-grill', when: 'Mittel: fragt in Runden nach.' },
          { label: 'Wayfinder', command: '/nexus:harness-wayfinder', when: 'Komplex: für große Vorhaben über mehrere Sessions.' },
        ],
      },
      { step: 'Plan schreiben', command: '/superpowers:writing-plans', when: 'Zerlegt die Spec in Tasks.' },
      { step: 'Tickets anlegen', place: 'In der Session sagen: „Lege für die Tasks des Plans Tickets an“', when: 'Claude legt je Task ein Ticket an.' },
      { step: 'Für den Dispatcher vorbereiten', place: 'In der Session sagen: „Bereite ARC-0042 für den Dispatcher vor“', when: 'Optional. Claude ergänzt, was dem Ticket fehlt.' },
    ],
  },
  {
    id: 'umsetzen',
    title: 'Ticket umsetzen',
    hint: 'In Claude Code, für jedes Ticket.',
    steps: [
      {
        step: 'Umsetzen',
        command: '/nexus:harness-implement',
        when: 'Setzt das nächste Ticket test-first um.',
        options: [
          { label: 'Ein Ticket', command: '/nexus:harness-implement', when: 'Setzt das nächste Ticket test-first um.' },
          { label: 'Ganzer Plan', command: '/superpowers:subagent-driven-development', when: 'Setzt alle Tasks des Plans mit Subagenten um.' },
        ],
      },
      { step: 'Automatisch umsetzen lassen', place: 'Einstellungen → Dispatcher', when: 'Was er prüft, steht im Abschnitt „Dispatcher“ der Anleitung.' },
      { step: 'Abschließen', place: 'In der Session sagen: „Schließe ARC-0042 ab“', when: 'Claude setzt das Ticket mit nexus board done auf Erledigt.' },
    ],
  },
];
