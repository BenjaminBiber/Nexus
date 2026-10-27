// Inhalt der Anleitung. Den Ablauf (planen, umsetzen) liefert der Core, damit Anleitung und Agent denselben Stand haben.
import { WORKFLOW_PHASES, type WorkflowOption, type WorkflowPhase, type WorkflowStep } from '../../../core/src/workflow/WorkflowPhases.js';

export type GuideOption = WorkflowOption;
export type GuideStep = WorkflowStep;
export type GuidePhase = WorkflowPhase;

// Oberflächenhilfen, die dem Ablauf des Cores folgen (Fragen, nebenbei)
export const EXTRA_PHASES: GuidePhase[] = [
  {
    id: 'fragen',
    title: 'Fragen',
    hint: 'Wenn eine Entscheidung noch nicht fällt: festhalten, recherchieren lassen, später selbst entscheiden.',
    steps: [
      { step: 'Entscheidung aufschieben', command: 'harness board question add "Titel" --ticket ARC-0042', when: 'Hält die Frage zum Ticket fest; das Ticket wartet, bis sie entschieden ist. Das geht auch im Ticket-Panel mit „Frage stellen“.' },
      { step: 'Recherchieren lassen', command: '/ai-harness:board-question ARC-F0007', when: 'Ein Agent sucht im Code die Fundstellen und schlägt Optionen vor. Entscheiden kannst nur du, auf der Seite „Fragen“.' },
      { step: 'Eigene Fragen sehen', place: 'Seite „Fragen“, Reiter „Eigene“', when: 'Zeigt die offenen Fragen, die dich betreffen: selbst gestellt (auch von deinen Agenten) oder zu Tickets auf deinen Boards. Nur sie zählt der rote Zähler an „Fragen“, und nur zu ihnen kommen Meldungen. Offene Fragen anderer stehen unter „Andere offene“.' },
      { step: 'Recherche starten lassen', place: 'Seite der Frage, „Recherche starten“', when: 'Startet sofort eine Claude-Session im eigenen Worktree, die die Frage recherchiert; das Ergebnis erscheint auf der Seite. Im Terminal: harness board question research-start ARC-F0007. Automatisch für neue Fragen zu deinen Boards: Einstellungen → Dispatcher → Recherchen.' },
    ],
  },
  {
    id: 'nebenbei',
    title: 'Nebenbei',
    hint: 'Bei Bedarf.',
    steps: [
      { step: 'Session fortsetzen', place: 'Seite „Sessions“, Favoritenliste', when: 'Session mit Stern merken, später den fertigen Befehl claude --resume kopieren.' },
      { step: 'Dokument finden', place: 'Seite „Docs“, Explorer links', when: 'Der Baum links zeigt Specs (je Thema Spec und Plan), Glossar und ADRs; die Suche darüber durchsucht alle Dokumente. In der Mitte steht das Dokument, rechts Inhaltsverzeichnis, Kommentare und Tickets. Die Seitenspalten lassen sich an ihrem Rand breiter ziehen. Das Symbol neben dem Pfad öffnet die Datei im Explorer oder in VS Code.' },
      { step: 'Stelle kommentieren', place: 'Seite „Docs“, Text markieren, „Kommentieren“', when: 'Den Kommentar im Fenster daneben schreiben. Ein Agent arbeitet ihn in das Dokument ein und erledigt ihn mit harness spec resolve.' },
      { step: 'Abschnitt bearbeiten', place: 'Seite „Docs“, Text markieren, „Bearbeiten“', when: 'Öffnet den Markdown-Editor für den markierten Abschnitt und speichert ihn zurück ins Dokument.' },
      { step: 'Mit Claude ändern', place: 'Seite „Docs“, Text markieren, „Mit Claude ändern“', when: 'Anweisung eingeben, Claude schlägt den geänderten Abschnitt vor; die Diff-Vorschau zeigt ihn, „Übernehmen“ schreibt ihn ins Dokument.' },
      { step: 'Tickets aus einem Dokument', place: 'Seite „Docs“, Knopf „Tickets erzeugen“ oben in der rechten Spalte', when: 'Claude zerlegt die ganze Spec bzw. den ganzen Plan in granulare Ticket-Entwürfe (je Task ein Ticket, bis zu 40). Das läuft auch weiter, wenn du die Seite wechselst: im Baum pulsiert dann das Claude-Symbol an der Datei, ein Hinweis unten rechts meldet, wenn die Entwürfe bereit sind. „Abbrechen“ (rechte Spalte oder Hinweis) beendet Claude und verwirft den Lauf. Der Dialog zeigt sie links als Kacheln, rechts die Details der gewählten zum Bearbeiten; „Akzeptieren“ legt sie im Backlog mit Planungsbedarf an (Abhängigkeiten in der Reihenfolge der Karten). Die Entwürfe werden je Datei gespeichert: Schließen merkt sich deine Änderungen, der Knopf heißt dann „Entwürfe öffnen“ (das Symbol daneben erzeugt sie neu und ersetzt sie, der Papierkorb oder „Entwürfe verwerfen“ im Dialog löscht sie, nach dem Anlegen sind sie weg). Der Filter „Entwürfe vorhanden“ über dem Baum zeigt nur Dateien mit gespeicherten Entwürfen. Sind aus dem Dokument schon Tickets entstanden, ist der Knopf gesperrt.' },
      { step: 'Repo und Board verbinden', place: 'Board, Verwaltung oder Terminal', when: 'Ein Repo arbeitet auf dem Board, dem du seinen Repo-Pfad zugeordnet hast (harness board boards repo <id> <pfad> oder „Mein Repo-Pfad“ in der Verwaltung); bei mehreren Boards eines Projekts auf dem eigenen. Der Repo-Pfad ist je Person lokal (~/.harness/board-repos.yaml), nicht in der gemeinsamen Datenbank: jeder trägt seinen eigenen Ordner ein. Nur dann ordnen die Hooks Sessions zu und importieren Aufgaben.' },
      { step: 'Von anderem Board holen', place: 'Board, „Von anderem Board holen“', when: 'Übernimmt Tickets eines anderen Boards desselben Projekts, zum Beispiel wenn ein Kollege krank ist. Tickets je Spalte ankreuzen.' },
      { step: 'Notizen festhalten', place: 'Board, „Notizen“', when: 'Hält Einfälle fest, macht aus einer Notiz ein Ticket oder verwirft sie mit Grund. Weg ist nichts.' },
      { step: 'Ticket planen lassen', place: 'Ticket-Panel, „Planen“', when: 'Lässt ein Ticket deines Boards sofort von einer Claude-Session planen (nur lesend, im Hintergrund). Der Plan erscheint am Ticket, fehlt eine Entscheidung, entsteht eine Frage. Im Terminal: harness board plan-start ARC-0042. Automatisch für neue Tickets mit Planungsbedarf, sobald im Dispatcher Platz ist: Einstellungen → Dispatcher → Planungen.' },
      { step: 'Merge anstoßen', place: 'Ticket-Panel, „Mergen“ bzw. „Erneut mergen“', when: 'Bei Blockiert mit „Merge: …“ den Merge nach der Korrektur erneut anstoßen. Im Terminal: harness board merge ARC-0042.' },
      { step: 'Abnehmen', place: 'Ticket-Panel, „Abnehmen“', when: 'Bei Blockiert mit „Human Interaction: …“ nach deiner Prüfung: Erledigt, danach mergt der Dispatcher. Im Terminal: harness board accept ARC-0042.' },
      { step: 'Agent zusehen', place: 'Ticket-Panel, Reiter „Agent“; Frage, „Recherche-Läufe“', when: 'Zeigt die Läufe des Dispatchers zum Ticket mit Protokoll: was Claude liest, ändert und ausführt, live alle 3 Sekunden, solange ein Lauf läuft (Punkt am Reiter). Die Arbeitsanweisung an den Agent lässt sich getrennt aufklappen.' },
    ],
  },
];

// Ablauf (Standard oder angewendet) gefolgt von den Oberflächenhilfen
export function guidePhases(workflow: GuidePhase[] = WORKFLOW_PHASES): GuidePhase[] {
  return [...workflow, ...EXTRA_PHASES];
}

// Befehle für das normale Terminal. Im Alltag nicht nötig, weil Claude sie bei Bedarf selbst aufruft.
export const TERMINAL: GuideStep[] = [
  { step: 'Board öffnen', command: 'harness board', when: 'Startet bei Bedarf den Server und öffnet diese Oberfläche.' },
  { step: 'Tickets ansehen', command: 'harness board list', when: 'Zeigt die Tickets des aktiven Boards je Spalte; --status geplant --md listet die bereiten.' },
  { step: 'Projekt einrichten', command: 'harness init', when: 'Richtet das Projekt ein bzw. frischt die Regeln auf (Doku- und Board-Regel); nach einem Update einmal je Projekt nötig.' },
  { step: 'Ticket mergen', command: 'harness board merge ARC-0042', when: 'Stößt den Merge eines erledigten Tickets an, auch erneut nach Blockiert „Merge: …“.' },
  { step: 'Ticket abnehmen', command: 'harness board accept ARC-0042', when: 'Nimmt ein Ticket mit Grund „Human Interaction: …“ ab: Erledigt, danach der Merge (nur auf eigenen Boards). Nur ein Mensch, nie ein Agent.' },
  { step: 'Branch am Ticket leeren', command: 'harness board set ARC-0042 --clear-branch', when: 'Leert Branch und Worktree am Ticket, wenn sie zu einem anderen oder aufgeräumten Worktree gehören; Branch und Ordner selbst bleiben.' },
  { step: 'Schritt anlegen', command: 'harness board step add ARC-0042 "Text"', when: 'Hängt einem Ticket einen Schritt an; edit, rm und move bearbeiten, löschen und verschieben ihn.' },
  { step: 'Recherche starten', command: 'harness board question research-start ARC-F0007', when: 'Startet die Recherche zu einer Frage sofort in einer eigenen Claude-Session.' },
  { step: 'Repo dem Board zuordnen', command: 'harness board boards repo 7 C:\\Projekte\\Demo', when: 'Verbindet ein Git-Repo mit einem Board – nur für dich auf diesem Rechner, der Repo-Pfad ist je Person lokal (Nummer und Pfad anpassen).' },
  { step: 'Dispatcher prüfen', command: 'harness board dispatch status', when: 'Zeigt laufende Worker, Merges und warum Geplant-Tickets noch nicht bereit sind; run prüft sofort, pause/resume halten an.' },
  { step: 'Kommentare zu Dokumenten lesen', command: 'harness spec comments spec-docs-rework', when: 'Listet die offenen Kommentare zu den Spec- und Plan-Dokumenten unter docs/superpowers, dazu Glossar und ADRs. Ohne Angabe alle, sonst ein Dokumentpfad oder ein Thema; harness spec resolve markiert einen Kommentar als erledigt.' },
];

// Abschnitt „Dispatcher“: was er prüft und tut, wie man ihn einrichtet und bedient (Stand: packages/core/src/dispatch, packages/server/src/dispatch)
export const DISPATCHER_READY: string[] = [
  'Es steht in Geplant, hat keinen Zuständigen und keinen Planungsbedarf.',
  'Es trägt das Label auto und nicht das Label manuell (beide in den Einstellungen änderbar).',
  'Plan, Handbuch und Abnahme sind ausgefüllt.',
  'Keine Voraussetzung und keine Frage ist offen. Voraussetzungen müssen erledigt und gemergt sein (kein Branch mehr) – sonst „wartet auf Merge von …“; so steckt ihr Code im neuen Branch.',
  'Die Labels profil:, model: und effort: nennen nur, was es gibt (sonst bleibt es stehen).',
  'Das Board gehört dir und hat deinen Repo-Pfad (je Person lokal), den es auf diesem Rechner gibt.',
];

export const DISPATCHER_OUTCOMES: GuideTerm[] = [
  { term: 'Start', meaning: 'Jedes Ticket bekommt einen eigenen Branch ticket/<ID>-<Kurztitel> vom Zielbranch des Tickets bzw. vom ausgecheckten Branch des Hauptordners und einen eigenen Worktree; unabhängige Tickets laufen so bis zur WIP-Grenze gleichzeitig. Die Einstellung „Worktrees“ → „Tickets einer Spec bündeln (alt)“ (joinWorktrees: sameSpec) teilt wie früher einen Worktree je Spec. Gehört der eingetragene Worktree eines fortgesetzten Tickets einem anderen Ticket oder wurde er aufgeräumt, stellt er ihn nicht wieder her, sondern startet auf eigenem Branch (Hinweis im Start-Kommentar).' },
  { term: 'Erledigt', meaning: 'Er mergt den Branch des Tickets einzeln in den Zielbranch des Tickets bzw. den Branch, der im Hauptordner des Repos ausgecheckt ist – als Merge-Commit oder Squash (Einstellung „Zurückmergen“), ohne auf andere Tickets der Spec zu warten. Danach räumt er Worktree und Branch auf.' },
  { term: 'Human Interaction', meaning: 'Fertig, aber ein Mensch muss es ansehen: Blockiert mit Grund „Human Interaction: …“. Der Dispatcher meldet es und mergt nichts, andere Tickets laufen weiter. „Abnehmen“ im Ticket-Panel (harness board accept ARC-0042) setzt Erledigt und mergt dann.' },
  { term: 'Blockiert', meaning: 'Der Agent setzt das Ticket auf Blockiert mit Grund (Zugang, Umgebung); der Dispatcher meldet es und mergt nichts.' },
  { term: 'Frage', meaning: 'Das Ticket geht zurück nach Geplant, Branch und Worktree bleiben. Nach deiner Entscheidung setzt er dort fort.' },
  { term: 'Abbruch', meaning: 'Ohne einen dieser Ausgänge geht das Ticket zurück nach Geplant; jeder weitere Versuch läuft eine Profilstufe größer (Eskalation nach Abbruch, abschaltbar). Nach maxAttempts Versuchen (Standard 2) geht es nach Blockiert.' },
  { term: 'Merge blockiert', meaning: 'Lokale Änderungen im Hauptordner halten den Merge nur an, wenn sie vorgemerkt sind (git add) oder Dateien betreffen, die auch der Branch ändert: dann wartet er und mergt von selbst, sobald sie committet oder verworfen sind. Blockiert mit „Merge: …“ wird z. B. bei Detached HEAD; nach dem Beheben „Erneut mergen“ im Ticket-Panel. Bei einem Konflikt startet er eine Merge-Session im Worktree; löst sie ihn nicht, geht das Ticket nach Blockiert.' },
  { term: 'Nutzungslimit', meaning: 'Das Ticket geht zurück nach Geplant, und der Dispatcher startet eine Weile nichts Neues (Standard 60 Minuten). Das zählt nicht als Versuch, erst das dritte Limit in Folge.' },
];

export const DISPATCHER_SETUP: GuideStep[] = [
  { step: 'Repo dem Board zuordnen', command: 'harness board boards repo 7 C:\\Projekte\\Demo', when: 'Ohne deinen eigenen, existierenden Repo-Pfad nimmt der Dispatcher keine Tickets des Boards; der Pfad ist je Person lokal.' },
  { step: 'Claude Code bereitstellen', place: 'Terminal', when: 'Claude Code installiert und angemeldet (Abo). API-Keys entfernt der Dispatcher aus der Umgebung des Laufs (useSubscription, Standard an). Unter Windows ist Git for Windows nötig: Die Git Bash sucht er selbst, sonst den Pfad im Feld „Git Bash“ eintragen (claude.gitBashPath). Der Agent ruft harness in der Git Bash auf; es muss dort auf dem PATH sein (nach npm run setup ein neues Terminal öffnen).' },
  { step: 'Einschalten und einstellen', place: 'Einstellungen → Dispatcher', when: 'Einschalten, Gleichzeitig (WIP), Zeitfenster, Profile und Regeln, Zurückmergen (Merge-Commit oder Squash), Worktrees (einer je Ticket oder je Spec gebündelt), Recherchen. Gespeichert in ~/.harness/dispatch.yaml.' },
  { step: 'Worktrees einrichten', place: 'Einstellungen → Dispatcher → Einrichtung neuer Worktrees', when: 'Befehle wie npm ci (Ordner relativ zum Worktree, z. B. BackofficeNextGen) führt der Dispatcher in jedem neu angelegten Worktree vor Claude aus, protokolliert im Reiter „Agent“; ein fortgesetztes Ticket wird nicht erneut eingerichtet. Scheitert ein Befehl, startet Claude nicht und das Ticket geht nach Blockiert mit „Einrichtung fehlgeschlagen: …“ (zählt nicht als Versuch). Links oder Junctions auf den Hauptordner legen Agents nicht an.' },
  { step: 'Prompts anpassen', place: 'Einstellungen → Dispatcher → Prompts', when: 'Je Laufart (Umsetzung, Planung, Recherche, Merge) eine eigene Vorlage statt der mitgelieferten; leer heißt: der Standard gilt und zieht mit neuen Versionen des Harness mit, „Auf Standard zurücksetzen“ stellt das wieder her. Pflichtteile wie die Board-Befehle müssen drin bleiben, sonst lehnt der Server die Vorlage ab. „Vorschau“ zeigt den fertigen Prompt der noch ungespeicherten Vorlage – mit Beispielwerten oder, mit einer Ticket- bzw. Frage-ID, mit deren echten Werten; „Unterschiede zum Standard“ stellt ihn dem Standard-Prompt zeilenweise gegenüber. Eine eigene Vorlage geht in den configHash eines Laufs ein: Läufe davor und danach sind im Dashboard unterscheidbar.' },
  { step: 'Tickets vorbereiten', place: 'In der Session sagen: „Bereite ARC-0042 für den Dispatcher vor“', when: 'Oder selbst die Labels auto und profil:<klein|standard|schwer> setzen.' },
  { step: 'Zusehen', place: 'Board, Knopf „Dispatcher: …“; Ticket-Panel, Reiter „Agent“', when: 'Zeigt Zustand, Gründe und Protokolle. Im Terminal: harness board dispatch status.' },
];

export const DISPATCHER_COMMANDS: GuideStep[] = [
  { step: 'Zustand', command: 'harness board dispatch status', when: 'Laufende Worker, Merges und warum Geplant-Tickets noch nicht bereit sind; --board <id> für ein anderes Board.' },
  { step: 'Sofort prüfen', command: 'harness board dispatch run', when: 'Führt gleich einen Durchlauf aus, statt auf die nächste Minute zu warten.' },
  { step: 'Pausieren', command: 'harness board dispatch pause --minutes 30', when: 'Startet so lange nichts Neues (Standard 60 Minuten); laufende Worker laufen weiter.' },
  { step: 'Pause aufheben', command: 'harness board dispatch resume', when: 'Hebt eine Pause auf, auch die nach einem Nutzungslimit.' },
  { step: 'Einschalten', command: 'harness board dispatch enable', when: 'Wie der Schalter in den Einstellungen; disable schaltet aus (laufende Worker laufen zu Ende).' },
  { step: 'Erneut mergen', command: 'harness board merge ARC-0042', when: 'Stößt den Merge eines erledigten Tickets nach „Blockiert: Merge: …“ erneut an und erlaubt wieder eine Merge-Session.' },
  { step: 'Abnehmen', command: 'harness board accept ARC-0042', when: 'Nimmt ein Ticket mit „Human Interaction: …“ ab (wie „Abnehmen“ im Ticket-Panel): Erledigt, danach der Merge.' },
];

export interface GuideTerm {
  term: string;
  meaning: string;
}

// Begriffe aus GLOSSARY.md
export const TERMS: GuideTerm[] = [
  { term: 'Ticket', meaning: 'Eine Arbeitseinheit auf dem Board mit Beschreibung, Status, Labels, Schritten, Kommentaren und Verlauf, gespeichert in der Board-Datenbank.' },
  { term: 'Projekt', meaning: 'Gruppiert Boards und Fragen unter einem Key, zum Beispiel ARC; legst du mit harness board project create <KEY> "<Name>" an.' },
  { term: 'Ticket-ID', meaning: 'Die eindeutige Kennung eines Tickets aus Projektkey und Nummer, zum Beispiel ARC-0042.' },
  { term: 'Board', meaning: 'Die Spaltenansicht der Tickets nach Status. Boards gehören einer Person innerhalb eines Projekts; ein Repo arbeitet auf dem Board, dem du seinen Repo-Pfad zugeordnet hast (bei mehreren das eigene).' },
  { term: 'Repo-Pfad', meaning: 'Der Ordner des Git-Repos eines Boards auf deinem Rechner. Er ist je Person lokal (~/.harness/board-repos.yaml) und steht nicht in der gemeinsamen Datenbank; jeder pflegt seinen eigenen, fremde Pfade siehst du nicht.' },
  { term: 'Spec', meaning: 'Die Beschreibung einer Funktion, die /superpowers:brainstorming unter docs/superpowers/specs/ ablegt; aus ihr entsteht der Plan, aus dessen Tasks die Tickets. Aus markierten Stellen der Spec lassen sich auf der Seite „Docs“ auch direkt Tickets anlegen.' },
  { term: 'Schritt', meaning: 'Ein abhakbarer Arbeitsschritt eines Tickets aus der Aufgabenliste oder im Ticket-Panel („+ Schritt“) bzw. mit harness board step add angelegt; der Fortschritt steht auf der Karte.' },
  { term: 'Kommentar', meaning: 'Eine datierte Wortmeldung an einem Ticket; Kommentare von Menschen bekommt die Session des Tickets bei der nächsten Eingabe einmal eingespeist.' },
  { term: 'Kommentar (Docs)', meaning: 'Eine Anmerkung zu einer markierten Textstelle in einem Superpowers-Dokument; ein Agent arbeitet sie ein und markiert sie mit harness spec resolve als erledigt.' },
  { term: 'Verlauf', meaning: 'Die zeitlich geordnete Folge aller Änderungen an einem Ticket.' },
  { term: 'Favorit', meaning: 'Eine mit Stern markierte Session, die dauerhaft aufbewahrt wird, damit man sie später fortsetzen kann.' },
  { term: 'Frage', meaning: 'Eine aufgeschobene Entscheidung zu Tickets eines Projekts. Solange sie offen ist, warten ihre Tickets.' },
  { term: 'Option', meaning: 'Ein Lösungsvorschlag zu einer Frage mit Vor- und Nachteilen; höchstens eine je Recherche ist empfohlen.' },
  { term: 'Recherche', meaning: 'Ein datierter Eintrag eines Agenten an einer Frage mit Fundstellen im Code und Optionen. Die neueste gilt als aktuell.' },
  { term: 'Entscheidung', meaning: 'Die Antwort auf eine Frage mit Person, Datum und wahlweise Begründung (Verwerfen verlangt eine). Sie trifft immer ein Mensch, nie ein Agent.' },
  { term: 'Session', meaning: 'Ein laufender Claude-Code-Lauf, der einem Ticket zugeordnet ist.' },
  { term: 'Dispatcher', meaning: 'Der Hintergrunddienst, der geplante Tickets mit Label auto selbst von Claude umsetzen und mergen lässt.' },
  { term: 'Worktree', meaning: 'Ein eigener Arbeitsordner zu einem Branch, in dem ein Ticket umgesetzt wird, ohne den Hauptordner zu berühren.' },
  { term: 'Planungsbedarf', meaning: 'Markiert Tickets im Backlog, die noch geplant werden müssen (Plan, Handbuch, Diagramm), bevor sie in Geplant landen.' },
  { term: 'Label auto', meaning: 'Gibt ein Ticket für den Dispatcher frei; ohne das Label setzt er es nicht um.' },
  { term: 'Subagent', meaning: 'Ein von einer Session gestarteter Hilfs-Agent; gehört zum Ticket seiner Session.' },
];

export interface GuideFaq {
  question: string;
  answer: string;
}

export const FAQ: GuideFaq[] = [
  { question: 'Kann ich kleine Aufgaben ohne Workflow erledigen?', answer: 'Ja, für schnelle Aufgaben oder Nebenaufgaben ohne Spec, Ticket und Worktree. Claude fragt dann zuerst „Ohne Workflow direkt umsetzen?“ und arbeitet erst nach deinem ausdrücklichen Ja so; sagst du selbst „ohne Workflow“, entfällt die Frage. Ohne Bestätigung bleibt es beim Ablauf.' },
  { question: 'Warum dürfen zwei Tickets in einem Worktree arbeiten?', answer: 'Standardmäßig bekommt jedes Ticket einen eigenen Worktree und wird einzeln gemergt. Ein gemeinsamer Worktree entsteht nur auf Wunsch: harness board start --join (oder --in <ID>) bzw. die Dispatcher-Einstellung „Tickets einer Spec bündeln (alt)“ – und nur, wenn sich die betroffenen Bereiche (touches) nicht überschneiden; sonst bekommt das Ticket trotzdem einen eigenen.' },
  { question: 'Wo liegen die Tickets?', answer: 'In der gemeinsamen Board-Datenbank (SQL Server). Gesichert wird sie vom Datenbank-Betrieb; der Harness legt keine eigenen Kopien an. Alte Ticketdateien unter .git/harness bleiben unberührt liegen.' },
  { question: 'Was bedeutet „Wartet“?', answer: 'Das Ticket hat eine offene Voraussetzung oder eine offene Frage. Der Hinweis entfällt von selbst, sobald sie erledigt bzw. entschieden ist.' },
  { question: 'Warum nimmt der Dispatcher mein Ticket nicht?', answer: 'Bereit ist es nur, wenn alles zutrifft: Geplant, ohne Zuständigen und ohne Planungsbedarf; Label auto, nicht manuell; Plan, Handbuch und Abnahme ausgefüllt; keine offene Voraussetzung oder Frage, und erledigte Voraussetzungen sind schon gemergt; gültige Labels profil:, model: und effort:; das Board gehört dir und hat deinen Repo-Pfad, den es gibt. Außerdem muss der Dispatcher eingeschaltet sein, nicht pausieren, im Zeitfenster liegen und unter der WIP-Grenze sein, und das Board muss in seiner Einstellung boards stehen (ohne Auswahl: das aktive Board). Den ersten fehlenden Punkt nennen die Übersicht „Dispatcher: …“ am Board und harness board dispatch status.' },
  { question: 'Warum sehe ich keine Sessions?', answer: 'Das Plugin ai-harness muss installiert sein (npm run setup im Harness-Ordner), das Projekt braucht harness init und danach /reload-plugins in Claude Code. Das Repo muss einem Board zugeordnet sein, und zwar mit deinem eigenen Repo-Pfad (harness board boards repo <id> <pfad> oder „Mein Repo-Pfad“ in der Verwaltung): der Pfad ist je Person lokal, der eines Kollegen gilt bei dir nicht. Die Session muss in diesem Projekt laufen. Sessions ohne erkennbares Ticket erscheinen unter „Nicht zugeordnet“; mit harness board claim ordnest du sie zu.' },
  { question: 'Wie aktualisiere ich den Harness?', answer: 'Im Ordner des Harness-Repositories npm run setup ausführen. Danach in laufenden Sessions /reload-plugins eingeben und harness board starten. Bestehende Projekte brauchen einmal harness init, damit die Board-Regel ankommt.' },
  { question: 'Wie setze ich eine Session fort?', answer: 'Markiere die Session in der Übersicht mit „Als Favorit merken“. In der Favoritenliste findest du den fertigen Befehl (cd in das Arbeitsverzeichnis, dann claude --resume mit der Kennung). Kopiere ihn in ein Terminal. Der Harness startet kein Terminal selbst.' },
];
