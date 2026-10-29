---
name: harness-implement
description: Setzt ein Ticket des Boards test-first um (nach ID oder das nächste geplante Ticket), arbeitet im von Nexus angelegten Worktree, hält Schritte, Kommentare und Status über nexus board aktuell und schließt mit Review und Abnahme ab. Verwenden, wenn ein Ticket umgesetzt werden soll.
---

# Ein Ticket umsetzen

Du setzt ein Ticket des Boards um. Status, Schritte und Kommentare änderst du ausschließlich über `nexus board …`; Tickets änderst du nie von Hand an der Datenbank vorbei. Worktrees legt Nexus an, nicht du.

## 1. Ticket bestimmen und verstehen

- Mit ID: nur dieses Ticket (`nexus board show ARC-0042`). Ohne ID: die geplanten Tickets der Spec nacheinander; `nexus board list --status geplant --md` zeigt sie. Nenne dem Benutzer das gefundene Ticket, ohne auf Antwort zu warten, und bleibe bei der Spec dieses Tickets.
- Gibt es kein geplantes Ticket, sage das und frage, wie es weitergehen soll. Neu angelegte Tickets einer Spec liegen im Backlog mit Planungsbedarf; schlage dann vor, sie zuerst zu planen (`nexus board next-planning` / `plan-json`, Skill `nexus:board`). Wartet das Ticket auf eine Voraussetzung oder eine Frage, nenne sie und schlage vor, zuerst diese zu erledigen. Entscheidungen zu Fragen triffst du nie selbst.
- Lies Beschreibung, Abnahme, Schritte, Plan, Handbuch und Kommentare des Tickets (`nexus board show`) und, falls vorhanden, das Spec-Dokument unter `docs/superpowers/specs/` und den Plan unter `docs/superpowers/plans/`, auf die es verweist. Der Aufgabentext selbst steht im Plan des Tickets, nicht in einer eigenen Datei. Nenne Titel und Ziel in einem Satz, bevor du arbeitest.

## Offene Kommentare einarbeiten

Der Benutzer kommentiert die Superpowers-Dokumente, Glossar und ADRs in den Docs des Boards. Bevor du am Dokument arbeitest:

1. `nexus spec comments <dokument>` (ohne Angabe: alle Dokumente; sonst ein Dokumentpfad oder ein Thema) listet die offenen Kommentare mit Datei, Abschnitt, Stelle und Text.
2. Arbeite jeden Kommentar in das genannte Dokument ein. Ist er unklar oder widerspricht er einem anderen Kommentar, frage den Benutzer, statt zu raten.
3. Markiere ihn danach mit `nexus spec resolve K-… "Was du geändert hast, in einem Satz"`. Die `*.comments.json`-Dateien bearbeitest du nie von Hand.
4. Nenne dem Benutzer am Ende, welche Kommentare du eingearbeitet hast.

Arbeite Kommentare vor `nexus board start` im Hauptordner ein, denn dort liest das Board. Superpowers-Dokumente, die du in einem Worktree änderst, erscheinen erst nach dem Merge in den Docs.

## 2. Arbeitsumgebung

- Übernimm das Ticket: `nexus board claim ARC-0042 --agent claude`. Innerhalb dieser Session verknüpft Nexus die Session dabei mit dem Ticket. Ist es schon zugewiesen und nicht deins, nimm das nächste geplante Ticket oder frag nach.
- Hat das Ticket noch keinen Branch, starte es mit `nexus board start ARC-0042 --by claude`. Jedes Ticket bekommt dabei einen eigenen Branch und Worktree vom Zielbranch des Tickets bzw. vom ausgecheckten Branch. Baut es auf einem Ticket derselben Spec auf, das noch nicht gemergt ist, starte es mit `--join`: dann tritt es dem offenen Worktree derselben Spec bei, wenn sich die Bereiche (`touches`) nicht mit laufenden Tickets überschneiden, und du setzt beide im selben Worktree um. Bei einer Überschneidungs-Warnung entsteht trotzdem ein eigener Worktree. Meldet der Befehl, dass das Ticket auf einem anderen Board liegt als dem dieses Repos, nenne dem Benutzer beide Boards und frag nach, statt weiterzumachen.
- Möchte der Benutzer ausdrücklich einen bestimmten Worktree, nutze `nexus board start ARC-0043 --in ARC-0042`.
- Wechsle in den Worktree, den der Befehl nennt, und arbeite dort. Lege nie selbst Worktrees oder Branches an; der Skill `superpowers:using-git-worktrees` ist ausgeschlossen, weil Nexus Worktrees verwaltet.

## 3. Umsetzen: Test zuerst

Falls vorhanden, nutze die Skills von Superpowers; sonst folge diesem kurzen Ablauf selbst:

1. **Plan**: Zerlege die Arbeit in kleine Schritte.
   **Umsetzen per Subagent (Pflicht)**: Rufe `superpowers:subagent-driven-development` auf. Ein Subagent mit frischem Kontext setzt jeden Teil um, auch bei aufeinander aufbauenden Tickets; nur das parallele Arbeiten setzt unabhängige Teile voraus. Setze nie selbst im Hauptlauf um. Ist Superpowers nicht installiert, darfst du ohne ihn vorgehen: Starte Subagenten mit klaren Aufträgen (Ziel, Dateien, Test, Befehl) und sage dem Benutzer, dass Superpowers fehlt. Subagenten ändern das Ticket nie von Hand.
2. **Test zuerst** (`superpowers:test-driven-development`, sonst selbst): erst einen fehlschlagenden Test schreiben und rot sehen, dann umsetzen, dann refaktorieren.
   Hat das Ticket Schritte (siehe `nexus board show`), arbeite sie der Reihe nach ab und hake jeden direkt nach dem Erledigen ab: `nexus board step ARC-0042 1 --by claude` (Nummer des Schritts; `--undo` nimmt den Haken zurück). „Nutzt“ und „Liefert“ nennen die Namen und Typen, an die du dich hältst.
3. **Kleine, lesbare Funktionen**, eine Klasse pro Datei, knappe Kommentare, wie es die Regeln des Projekts verlangen (siehe `CLAUDE.md` und Constitution).
4. **Prüfen**: Typprüfung und betroffene Tests regelmäßig, den gesamten Satz einmal am Ende (`superpowers:verification-before-completion`, sonst selbst). Behaupte nie „fertig“ ohne Ergebnis des Befehls.
5. **Review (Pflicht)**: Ein Subagent mit frischem Blick prüft den Diff, bevor das Ticket auf Erledigt geht (`superpowers:requesting-code-review`; ohne Superpowers ein Subagent mit klarem Auftrag).
6. Committe im Worktree mit sinnvollen Nachrichten.

## 4. Ticket aktuell halten

- Jede Änderung am Ticket begründest du mit einem Kommentar: `nexus board comment ARC-0042 "Schritt 2 von 4 fertig, Tests grün" --author claude`. Innerhalb der Session gilt dein Kommentar sofort als gelesen und kommt nicht in deine Session zurück.
- Kommentare des Benutzers speist Nexus bei der nächsten Eingabe ein; du berücksichtigst sie und antwortest per Kommentar. Ändert sich dadurch die Aufgabe, passe die Arbeit an und begründe es im Kommentar.
- Brauchst du eine Entscheidung des Benutzers, stelle eine Frage zum Ticket (Skill `board-question`, `nexus board question add "…" --ticket ARC-0042`); das Ticket wartet dann, bis sie entschieden ist.

## 5. Abschluss

- Sind Tests und Typprüfung grün und das Review erledigt, ist das Ticket fertig: Schreibe einen Kommentar mit Ergebnis, geänderten Bereichen, Prüfungen und offenen Punkten und schließe es mit `nexus board done ARC-0042 --acceptance "Woran man sieht, dass es fertig ist" --by claude` ab. Das setzt Erledigt (ohne Abnahme geht es nicht) und räumt fertige Worktrees auf; es wird nichts gemergt.
- Ist noch etwas offen (rote Tests, Anmerkungen aus dem Review, Rückfragen), bleibt das Ticket in Arbeit und der Kommentar nennt, was fehlt. Konntest du den Nachweis nicht selbst erbringen, setze es auf Blockiert mit Grund „Human Interaction“ (siehe Skill `board`).
- Gibt es ein weiteres geplantes Ticket derselben Spec (`nexus board list --status geplant --md`), setze es ohne Rückfrage um und beginne wieder bei Abschnitt 1. Das gilt, solange der Benutzer nichts anderes sagt. Tickets anderer Specs setzt du nie ungefragt um.
- Halte nur an, wenn in der Spec kein Ticket mehr bereit ist, ein Review offene Punkte hat, Tests rot bleiben oder eine Entscheidung des Benutzers nötig ist. Nenne dann Branch, Worktree und was als Nächstes ansteht.

## Regeln

- Tickets nie von Hand ändern; nur `nexus board …`.
- Keine Worktrees selbst anlegen; nur `nexus board start`.
- Nichts mergen, nichts auf Remote pushen, solange der Benutzer es nicht verlangt.
- Texte für Menschen in natürlichem Deutsch.
