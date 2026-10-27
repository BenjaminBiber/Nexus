---
name: harness-grill
description: Klärt Anforderungen mit dem Benutzer in Befragungsrunden mit Empfehlungen und pflegt Glossar und ADRs. Läuft vor /superpowers:writing-plans und legt selbst keine Tickets oder Worktrees an, eine Spec nur auf Zustimmung. Verwenden, wenn ein Plan, eine Idee oder eine Entscheidung geklärt werden soll.
---

# Anforderungen klären

Du befragst den Benutzer so lange, bis ihr ein gemeinsames Verständnis habt. Das Ergebnis ist eine Zusammenfassung, aus der die Spec unter `docs/superpowers/specs/` und danach mit `/superpowers:writing-plans` der Plan entsteht. Du legst weder Tickets noch Worktrees an und die Spec erst bei der Übergabe; Tickets entstehen später aus den Tasks des Plans.

## 0. Voraussetzungen prüfen

- Lies, was im Projekt schon vorliegt: `GLOSSARY.md`, `docs/adr/`, vorhandene Specs unter `docs/superpowers/specs/`, die Tickets mit `harness board list --md`.
- Meldet ein Befehl, dass das Projekt nicht eingerichtet ist, bitte den Benutzer, einmal `harness init` auszuführen, und warte.

## 1. Befragung in Runden

Arbeite die Entscheidungen als Baum ab: Jede Entscheidung führt zu weiteren, die davon abhängen. Die **Grenze** sind alle Entscheidungen, deren Voraussetzungen schon geklärt sind.

- Stelle in jeder Runde die ganze Grenze auf einmal, nummeriert, jede Frage mit einer **Empfehlung**.
- Formuliere so, dass „Ja“ die Empfehlung annimmt.
- Warte die Antworten ab, bevor du die nächste Runde stellst. Antworten verschieben die Grenze und schalten Folgefragen frei.
- Fakten suchst du selbst (Code, Dateien, Dokumentation); frage den Benutzer nie nach etwas, das du nachlesen kannst. Entscheidungen gehören dem Benutzer.
- Die Befragung ist beendet, wenn die Grenze leer ist: Jeder Zweig ist besucht, nichts wurde stillschweigend angenommen. Fasse dann das Verständnis kurz zusammen und lass es bestätigen.

Format einer Frage:

```
❓ **Q1** - **<Titel>**: <Frage, ggf. mit Auswahlmöglichkeiten>

➡️ <deine empfohlene Antwort>
```

## 2. Begriffe und Entscheidungen festhalten

Während des Gesprächs, nicht erst am Ende:

- **Begriffe**: Widerspricht ein Wort dem Glossar oder ist es unscharf, sage es sofort und schlage einen eindeutigen Begriff vor. Geklärte Begriffe schreibst du sofort in `GLOSSARY.md` (Begriff, ein bis zwei Sätze Definition, „Nicht verwenden“ für Synonyme). Das Glossar enthält nur Begriffe dieses Projekts, keine Implementierungsdetails.
- **ADR**: Biete nur dann einen ADR unter `docs/adr/NNNN-kurztitel.md` an, wenn die Entscheidung **schwer rückgängig zu machen** ist, **überraschend** ohne Kontext und das **Ergebnis einer echten Abwägung**. Fehlt eines davon, kein ADR. Ein ADR darf ein Absatz sein: Kontext, Entscheidung, Begründung.
- **Szenarien**: Prüfe Beziehungen mit konkreten Beispielen und Randfällen, damit der Benutzer Grenzen genau benennt.

## Offene Kommentare einarbeiten

Der Benutzer kommentiert die Superpowers-Dokumente, Glossar und ADRs in den Docs des Boards. Bevor du am Dokument arbeitest:

1. `harness spec comments <dokument>` (ohne Angabe: alle Dokumente; sonst ein Dokumentpfad oder ein Thema) listet die offenen Kommentare mit Datei, Abschnitt, Stelle und Text.
2. Arbeite jeden Kommentar in das genannte Dokument ein. Ist er unklar oder widerspricht er einem anderen Kommentar, frage den Benutzer, statt zu raten.
3. Markiere ihn danach mit `harness spec resolve K-… "Was du geändert hast, in einem Satz"`. Die `*.comments.json`-Dateien bearbeitest du nie von Hand.
4. Nenne dem Benutzer am Ende, welche Kommentare du eingearbeitet hast.

## 3. Übergabe an die Spezifikation

Nach der bestätigten Zusammenfassung:

1. Gib das Ergebnis kompakt aus: Ziel, Entscheidungen, Randfälle, geklärte Begriffe, offene Punkte.
2. Weise auf den nächsten Schritt hin: Die Zusammenfassung wird die Spec unter `docs/superpowers/specs/JJJJ-MM-TT-<thema>-design.md` (committet), denn `/superpowers:writing-plans` plant aus einer Spec. Schreibe sie erst, wenn der Benutzer zustimmt, und starte `/superpowers:writing-plans` nicht selbst; der Benutzer entscheidet.
3. Nenne danach den weiteren Weg: `/superpowers:writing-plans` schreibt den Plan unter `docs/superpowers/plans/`. Dann „Lege für die Tasks des Plans Tickets an“: Claude legt je Task ein Ticket an und plant es (Skill `ai-harness:board`, „Tickets aus einem Superpowers-Plan“). Danach folgt `/ai-harness:harness-implement` oder der Dispatcher.
4. Weise auf die Seite „Docs“ der Oberfläche hin: Dort sieht der Benutzer Spec und Plan und kann eine markierte Stelle kommentieren, mit „Bearbeiten“ ändern, mit „Mit Claude ändern“ umschreiben lassen oder mit „Zur Ticket-Auswahl“ für Ticket-Entwürfe sammeln.

## Regeln

- Lege keine Tickets, Specs, Branches oder Worktrees an und ändere Tickets nie von Hand; nur Glossareinträge und bestätigte ADRs entstehen hier. Einzige Ausnahme ist die Spec aus der Übergabe (Abschnitt 3), wenn der Benutzer zustimmt.
- Schreibe Texte für Menschen in natürlichem Deutsch ohne wörtlich übersetzte Fachwörter.
