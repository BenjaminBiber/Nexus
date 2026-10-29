---
name: harness-customize
description: Verwaltet die Anpassungen des Harness für diesen Dev – Workflow, Skills und Output-Styles in custom/ im Harness-Clone; kopieren, neu anlegen, prüfen, zurücksetzen, anwenden. Verwenden, wenn der Benutzer den Ablauf des Harness ändern, einen Harness-Skill oder Output-Style ersetzen oder einen eigenen hinzufügen will.
---

# Harness anpassen

Anpassungen gelten für diesen Dev in allen Projekten. Sie liegen in `custom/` im Harness-Clone, den Git ignoriert, und überstehen Updates ohne Merge. Sie wirken erst nach `nexus install` und in einer neuen Session (oder nach `/reload-plugins`).

## 1. Clone finden

Lies `~/.nexus/customizations/overview.json`. Ist die Umgebungsvariable `NEXUS_HOME` gesetzt, liegt die Datei stattdessen unter `$NEXUS_HOME/customizations/overview.json`. Daraus brauchst du `cloneDir` und `customDir`.

Fehlt die Datei, sage: „Der Harness muss einmal im Harness-Ordner mit `nexus install` eingerichtet werden.“ Ändere dann nichts.

## 2. Vorgang wählen

Ist unklar, was der Benutzer will, frage nach. Alle Befehle laufen mit `nexus customize --clone "<cloneDir>"`:

| Vorgang | Befehl |
|---|---|
| Workflow anpassen | `nexus customize --clone "<cloneDir>" copy workflow`, danach `<customDir>/workflow.ts` bearbeiten |
| Skill ersetzen | `nexus customize --clone "<cloneDir>" copy skill <name>`, danach `<customDir>/skills/<name>/` bearbeiten |
| Output-Style ersetzen | `nexus customize --clone "<cloneDir>" copy output-style <name>`, danach `<customDir>/output-styles/<name>.md` bearbeiten |
| Neuen Skill anlegen | `<customDir>/skills/<name>/SKILL.md` selbst schreiben (Kopf mit `name: <name>` und `description`) |
| Neuen Output-Style anlegen | `<customDir>/output-styles/<name>.md` selbst schreiben (Kopf mit `name: <name>` und `description`) |
| Prüfen | `nexus customize --clone "<cloneDir>" check` |
| Zurück zum Standard | `nexus customize --clone "<cloneDir>" reset workflow` bzw. `reset skill <name>` oder `reset output-style <name>` |
| Anwenden | `npm --prefix "<cloneDir>" run build:plugin`, danach `<cloneDir>/packages/plugin/dist/nexus/bin/nexus install`; mit Timeout 600000 ms (10 Minuten) oder im Hintergrund (siehe Regeln) |

Ersetzen beginnt immer mit `copy`: so startet die Anpassung beim aktuellen Standard, und der Harness merkt sich die Basis, um später Änderungen am Standard zu melden. Gibt es die Anpassung schon, bearbeite sie direkt; `copy` ist nur für das erste Ersetzen. `--rebase` nur nach dem Übernehmen eines geänderten Standards.

## 3. Regeln

- Schreibe nur in `<customDir>` und nie in andere Dateien des Clones. Dateien in `<customDir>/.base/` änderst du nie von Hand; die Basis erneuert nur `copy … --rebase`.
- Namen: Kleinbuchstaben, Ziffern und einzelne Bindestriche, höchstens 64 Zeichen. Gleicher Name wie ein mitgelieferter Skill oder Output-Style ersetzt ihn, ein neuer Name kommt dazu. Aufruf eines Skills: `/nexus:<name>`.
- Der Workflow ist ein Default-Export vom Typ `WorkflowPhase[]` und muss die Phasen `planen` und `umsetzen` enthalten; ihr Inhalt ist frei. Die Phasen-IDs `fragen` und `nebenbei` sind für die Anleitung im Board reserviert:
  ```ts
  import type { WorkflowPhase } from '@harness/core';
  export default [
    { id: 'planen', title: '…', hint: '…', steps: [{ step: '…', command: '/…', when: '…' }] },
    { id: 'umsetzen', title: '…', hint: '…', steps: [] },
  ] satisfies WorkflowPhase[];
  ```
- Nach jeder Änderung: zuerst prüfen, dann anbieten anzuwenden. Ungültige Anpassungen fallen beim Anwenden auf den Standard zurück; die Ausgabe nennt Datei und Grund.
- Meldet die Prüfung „Der Standard hat sich seit deiner Kopie geändert“, zeige dem Benutzer den Diff, übernimm die gewünschten Änderungen in die Anpassung und erneuere danach die Basis mit `copy … --rebase` (gleiches Ziel wie beim Kopieren).
- Vor „Zurück zum Standard“ immer bestätigen lassen und sagen, was gelöscht wird; `custom/` hat keine Sicherung. Ist es ein eigener, neuer Output-Style, weise darauf hin, dass er danach nicht mehr auswählbar ist, falls er gerade gewählt ist.
- `npm run build:plugin` baut das Binary und das Plugin neu und legt dabei die Anpassungen über den Standard; das kann mehrere Minuten dauern. Führe es nur mit einem Timeout von 600000 ms (10 Minuten) aus, weil die üblichen zwei Minuten den Bau mitten im Lauf abbrechen, oder starte es im Hintergrund. Warte, bis die Ausgabe `Plugin gebaut: …` zeigt; erst danach `nexus install` ausführen.
- Nach dem Anwenden: neue Session starten oder `/reload-plugins` eingeben. Die aktiven Anpassungen zeigt auch das Board unter „Einstellungen“ im Panel „Anpassungen“.
