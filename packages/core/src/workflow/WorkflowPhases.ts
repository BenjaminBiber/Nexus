// Gemeinsame Quelle des Ablaufs: Die Anleitung im Board und der Hinweis an den Agenten lesen beide hieraus.
// Nur reine Daten, keine Node-Importe, damit die Oberfläche die Datei direkt einbinden kann.

export interface WorkflowOption {
  label: string;
  command: string;
  when: string;
}

export interface WorkflowStep {
  step: string;
  /** Befehl der ersten (Standard-)Wahl, wenn `options` gesetzt ist; fehlt, wenn der Schritt keinen Befehl hat */
  command?: string;
  /** Statt eines Befehls: Ort in der Oberfläche oder ein Satz, den man Claude sagt (ohne Kopieren-Knopf) */
  place?: string;
  when: string;
  /** Gleichwertige Alternativen für diesen Schritt; die Oberfläche zeigt sie als Umschalter */
  options?: WorkflowOption[];
}

export interface WorkflowPhase {
  id: string;
  title: string;
  hint: string;
  steps: WorkflowStep[];
}

// Ablauf in Phasen, nur mit Befehlen für Claude Code. Die CLI ruft Claude im Hintergrund selbst auf.
export const WORKFLOW_PHASES: WorkflowPhase[] = [
  {
    id: 'planen',
    title: 'Neue Funktion planen',
    hint: 'In Claude Code, einmal pro Funktion. Für kleine Aufgaben genügt der erste Schritt.',
    steps: [
      {
        step: 'Anforderungen klären',
        command: '/superpowers:brainstorming',
        when: 'Einfach: erkundet Idee, Anforderungen und Entwurf im Dialog (Superpowers-Plugin) und schreibt die Spec nach docs/superpowers/specs/. Danach weiter mit /superpowers:writing-plans.',
        options: [
          { label: 'Brainstorming', command: '/superpowers:brainstorming', when: 'Einfach: erkundet Idee, Anforderungen und Entwurf im Dialog (Superpowers-Plugin) und schreibt die Spec nach docs/superpowers/specs/. Danach weiter mit /superpowers:writing-plans.' },
          { label: 'Grill', command: '/ai-harness:harness-grill', when: 'Mittel: fragt in Runden nach, pflegt Glossar und ADRs. Am Ende steht eine Zusammenfassung. Claude legt sie als Spec unter docs/superpowers/specs/ ab; danach weiter mit /superpowers:writing-plans.' },
          { label: 'Wayfinder', command: '/ai-harness:harness-wayfinder', when: 'Komplex: für große Vorhaben über mehrere Sessions (AI Hero). Hält Ziel und Klärungsfragen in einer Karte fest; am Ende steht eine Zusammenfassung. Claude legt sie als Spec unter docs/superpowers/specs/ ab (je Funktion eine); danach weiter mit /superpowers:writing-plans.' },
        ],
      },
      { step: 'Plan schreiben', command: '/superpowers:writing-plans', when: 'Zerlegt die Spec in Tasks mit Dateien, Code, Tests und Befehlen und speichert den Plan unter docs/superpowers/plans/. Fragt der Skill danach, wie umgesetzt werden soll: zuerst die Tickets anlegen lassen.' },
      { step: 'Tickets anlegen', place: 'In der Session sagen: „Lege für die Tasks des Plans Tickets an“', when: 'Claude legt je Task ein Ticket an (Titel, Abnahme, Schritte aus den Task-Schritten, Abhängigkeiten in Task-Reihenfolge) und schreibt Plan (Task-Text), Handbuch (Testbefehle, globale Vorgaben) und Diagramm mit harness board plan-json; danach stehen sie in Geplant. Plan und Handbuch stehen im Ticket – Worktrees und Agenten brauchen die Plan-Datei nicht. Kleinere Schnitte gehen auch auf der Seite „Docs“: Stelle markieren, „Zur Ticket-Auswahl“, Entwürfe prüfen, anlegen.' },
      { step: 'Für den Dispatcher vorbereiten', place: 'In der Session sagen: „Bereite ARC-0042 für den Dispatcher vor“', when: 'Optional. Claude prüft, was dem Ticket für den Dispatcher fehlt, ergänzt es, schätzt den Aufwand ein und setzt das Label auto mit profil:<klein|standard|schwer>.' },
    ],
  },
  {
    id: 'umsetzen',
    title: 'Ticket umsetzen',
    hint: 'In Claude Code, für jedes Ticket. Ohne Nummer nimmt Claude das nächste bereite Ticket.',
    steps: [
      {
        step: 'Umsetzen',
        command: '/ai-harness:harness-implement',
        when: 'Wählt das nächste Ticket, legt Branch und Worktree an (harness board start), setzt es test-first um und schließt mit Review ab. Ein bestimmtes Ticket: /ai-harness:harness-implement ARC-0042.',
        options: [
          { label: 'Ein Ticket', command: '/ai-harness:harness-implement', when: 'Wählt das nächste Ticket, legt Branch und Worktree an (harness board start), setzt es test-first um und schließt mit Review ab. Ein bestimmtes Ticket: /ai-harness:harness-implement ARC-0042.' },
          { label: 'Ganzer Plan', command: '/superpowers:subagent-driven-development', when: 'Setzt alle Tasks des Plans in dieser Session mit Subagenten um. Die Tickets dabei mit dem Board-Skill nachführen: übernehmen, Schritte abhaken, abschließen.' },
        ],
      },
      { step: 'Automatisch umsetzen lassen', place: 'Einstellungen → Dispatcher', when: 'Der Dispatcher setzt bereite Tickets mit Label auto selbst im eigenen Worktree um und mergt sie danach. Was er prüft und wie man ihn einrichtet, steht im Abschnitt „Dispatcher“ der Anleitung.' },
      { step: 'Schritte pflegen', place: 'Im Board im Ticket-Panel, „+ Schritt“', when: 'Schritte des Tickets lassen sich im Panel anlegen, bearbeiten, löschen und verschieben. Im Terminal: harness board step add|edit|rm|move.' },
      { step: 'Kommentare schreiben', place: 'Im Board im Ticket-Panel, Reiter „Kommentare“', when: 'Claude bekommt neue Kommentare automatisch bei der nächsten Eingabe und antwortet darauf.' },
      { step: 'Abschließen', place: 'In der Session sagen: „Schließe ARC-0042 ab“', when: 'Claude setzt das Ticket mit harness board done auf Erledigt. Gemergt wird mit „Mergen“ im Ticket-Panel oder harness board merge ARC-0042; der Dispatcher mergt seine Tickets selbst.' },
    ],
  },
];
