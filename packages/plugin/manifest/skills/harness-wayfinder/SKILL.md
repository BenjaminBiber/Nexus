---
name: harness-wayfinder
description: Klärt Anforderungen für große Vorhaben über mehrere Sessions, indem es Ziel und Klärungsfragen in einer Karte festhält und nacheinander beantwortet. Läuft vor /superpowers:writing-plans und legt selbst keine Tickets oder Branches an, Specs nur auf Zustimmung. Verwenden, wenn der Weg zum Ziel noch unklar ist und eine Session nicht reicht.
---

# Große Vorhaben klären (Wayfinder)

Diese Arbeitsweise folgt dem Skill „Wayfinder“ von Matt Pocock (MIT-Lizenz, github.com/mattpocock/skills), neu gefasst für den Harness. Du planst nur und baust nichts. Das Ergebnis ist eine Karte, aus der am Ende eine Zusammenfassung entsteht; aus ihr werden die Spec unter `docs/superpowers/specs/` und mit `/superpowers:writing-plans` der Plan. Tickets entstehen erst später aus den Tasks des Plans; Tickets änderst du nie von Hand.

Wähle diesen Ablauf, wenn der Weg zum Ziel unklar ist und mehrere Sessions nötig sind. Passt das Vorhaben in eine Session, ist `/ai-harness:harness-grill` die bessere Wahl.

## 0. Voraussetzungen prüfen

- Lies, was im Projekt schon vorliegt: `GLOSSARY.md`, `docs/adr/`, vorhandene Specs unter `docs/superpowers/specs/`, vorhandene Karten unter `docs/klaerung/`, die Tickets mit `harness board list --md` und die Notizen mit `harness board note list --md`.
- Sieh dir die offenen Fragen des Projekts mit `harness board question list --md` an: Was dort schon entschieden wurde, gilt für die Karte und wird nicht neu verhandelt. Die Board-Befehle liest du nur; du änderst nichts daran (kein `claim`, `set`, `move`, `comment` oder Beantworten einer Frage).
- Gibt es zum Vorhaben schon eine Karte, setze sie fort (Abschnitt 3) und lege keine zweite an.
- Meldet ein Befehl, dass das Projekt nicht eingerichtet ist, bitte den Benutzer, einmal `harness init` auszuführen, und warte.

## 1. Ziel benennen und Karte anlegen

- Klärt das Ziel in ein bis zwei Sätzen: Woran erkennt man, dass das Vorhaben fertig ist? Frage in Runden mit Empfehlung (Format wie in `harness-grill`, jede Frage nummeriert und mit empfohlener Antwort).
- Lege die Karte als **eine** Datei an: `docs/klaerung/<kurzname>.md` mit diesen Abschnitten:
  - **Ziel**: das Zielbild.
  - **Rahmen**: Randbedingungen, Hinweise, Quellen.
  - **Entscheidungen bisher**: eine Zeile je Entscheidung (Frage, Antwort in einem Satz).
  - **Klärungsfragen**: offene Fragen `K1`, `K2` … mit Art (Gespräch, Prototyp, Recherche, Vorarbeit), Stand (offen, in Arbeit, geklärt) und „wartet auf“.
  - **Noch nicht festgelegt**: Unklares, das noch nicht als Frage taugt („Nebel“).
  - **Außerhalb des Umfangs**: was bewusst nicht dazugehört.
- Nummern wie `K1` gehören nur der Karte. Es sind keine Tickets und haben keine Ticketnummern.

## 2. Nebel und Fragen

- Etwas wird erst dann zur Klärungsfrage, wenn du sie **präzise formulieren** kannst, nicht erst, wenn du sie beantworten kannst. Alles andere bleibt unter „Noch nicht festgelegt“.
- Sieht das Vorhaben schon klar aus, sage das und schlage `/ai-harness:harness-grill` vor, statt eine Karte aufzublasen.
- Lege Reihenfolge und Abhängigkeiten fest: Die **Grenze** sind alle offenen Fragen, die nicht auf eine andere warten.

## 3. Karte abarbeiten

Pro Session höchstens eine Frage, mit Ausnahme von Recherchen:

1. Lade die Karte und wähle eine Frage von der Grenze. Nenne sie dem Benutzer und markiere sie als „in Arbeit“.
2. Beantworte sie je nach Art:
   - **Gespräch**: Befragung in Runden mit Empfehlungen, wie in `harness-grill`.
   - **Recherche**: Fakten selbst nachlesen (Code, Dateien, Dokumentation); ein Subagent mit klarem Auftrag ist erlaubt. Nennt die Frage ein Ticket, lies es mit `harness board show <id>` nach — nur lesen, nichts daran ändern. Das Ergebnis steht als Abschnitt in der Karte, nicht auf einem eigenen Branch.
   - **Prototyp**: Wegwerf-Code nur im Arbeitsordner; er kommt nicht in den Hauptzweig. Fasse zusammen, was er beantwortet hat.
   - **Vorarbeit**: Was der Benutzer selbst erledigen muss; vermerke es in der Karte.
3. Trage die Antwort ein: Frage auf „geklärt“, eine Zeile unter „Entscheidungen bisher“, daraus folgende neue Fragen oder gelichteten Nebel ergänzen.
4. Halte Begriffe und Entscheidungen sofort fest (Abschnitt 4).

## 4. Begriffe und Entscheidungen festhalten

- **Begriffe**: Unscharfe oder widersprüchliche Wörter sagst du sofort an und schlägst einen eindeutigen Begriff vor. Geklärte Begriffe gehören in `GLOSSARY.md` (Begriff, ein bis zwei Sätze Definition, „Nicht verwenden“ für Synonyme).
- **ADR**: Nur wenn die Entscheidung **schwer rückgängig zu machen** ist, **überraschend** ohne Kontext und das **Ergebnis einer echten Abwägung**. Dann unter `docs/adr/NNNN-kurztitel.md`: Kontext, Entscheidung, Begründung.

## 5. Abschluss und Übergabe

Die Karte ist fertig, wenn Grenze und „Noch nicht festgelegt“ leer sind und der Benutzer es bestätigt.

1. Gib eine kompakte Zusammenfassung aus: Ziel, Entscheidungen mit Kernaussage, Außerhalb des Umfangs, geklärte Begriffe, offene Punkte.
2. Ergibt die Karte mehrere getrennte Funktionen, schlage sie als eigene Durchläufe vor: je Funktion eine Spec mit dem passenden Teil der Zusammenfassung und ein eigener Plan.
3. Weise auf den nächsten Schritt hin: Die Zusammenfassung wird die Spec unter `docs/superpowers/specs/JJJJ-MM-TT-<thema>-design.md` (committet), denn `/superpowers:writing-plans` plant aus einer Spec. Schreibe sie erst, wenn der Benutzer zustimmt, und starte `/superpowers:writing-plans` nicht selbst; der Benutzer entscheidet.
4. Nenne den weiteren Weg: `/superpowers:writing-plans` schreibt den Plan unter `docs/superpowers/plans/`. Dann „Lege für die Tasks des Plans Tickets an“: Claude legt je Task ein Ticket an und plant es (Skill `ai-harness:board`, „Tickets aus einem Superpowers-Plan“). Danach folgt `/ai-harness:harness-implement` oder der Dispatcher.
5. Weise auf die Seite „Docs“ der Oberfläche hin: Dort sieht der Benutzer Spec und Plan und kann eine markierte Stelle kommentieren, mit „Bearbeiten“ ändern, mit „Mit Claude ändern“ umschreiben lassen oder mit „Zur Ticket-Auswahl“ für Ticket-Entwürfe sammeln.

## Regeln

- Lege keine Tickets, Specs, Branches oder Worktrees an und pushe nichts; Tickets nie von Hand ändern. Es entstehen nur die Karte, bestätigte Glossareinträge und ADRs; einzige Ausnahme ist die Spec aus der Übergabe (Abschnitt 5), wenn der Benutzer zustimmt.
- Rufe keine anderen Skills auf; beschreibe die Arbeit selbst.
- Schreibe Texte für Menschen in natürlichem Deutsch ohne wörtlich übersetzte Fachwörter.
