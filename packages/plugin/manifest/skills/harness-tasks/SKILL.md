---
name: harness-tasks
description: Schneidet aus Spec und Plan unter docs/superpowers die Aufgaben zu und legt je Aufgabe ein Ticket an. Jede Aufgabe ist selbsttragend, mit kurzem Titel, Beschreibung, Akzeptanzkriterien, Dateien, Schnittstellen und konkreten test-first-Schritten mit Code, Befehl und erwartetem Ergebnis. Verwenden, wenn aus einem Plan Aufgaben und Tickets entstehen sollen.
---

# Aufgaben zuschneiden und als Tickets anlegen

Du schneidest die Aufgaben einer Funktion zu und legst daraus Tickets auf dem Board an: der Titel der Aufgabe wird der Titel der Karte, `Beschreibung:` und `Akzeptanz:` werden Beschreibung und Abnahme des Tickets, die Schritte seine Arbeitsliste, der volle Aufgabentext sein Plan. Tickets änderst du nie von Hand, nur über `harness board …`.

**Leitgedanke**: Wer eine Aufgabe umsetzt, sieht nur dieses Ticket. Es muss selbsttragend sein: Was ist zu bauen, woran erkennt man „fertig“, welche Dateien, welcher Code, welcher Befehl, welches Ergebnis. Nichts bleibt abstrakt: kein „Logik ergänzen“, „Tests schreiben“ oder „Fehler behandeln“ ohne Angabe, wie genau.

## 1. Vorbereiten

- Lies den Plan unter `docs/superpowers/plans/…` (ohne Angabe des Benutzers den neuesten) und die Spec unter `docs/superpowers/specs/…`, auf die er sich bezieht (Pflicht).
- Falls vorhanden, lies auch `GLOSSARY.md` und die ADRs unter `docs/adr/`.
- Zusätzliche Wünsche des Benutzers beachtest du.

## Offene Kommentare einarbeiten

Der Benutzer kommentiert die Superpowers-Dokumente, Glossar und ADRs in den Docs des Boards. Nach dem Lesen von Spec und Plan, bevor du Aufgaben zuschneidest:

1. `harness spec comments <dokument>` (ohne Angabe: alle Dokumente; sonst ein Dokumentpfad oder ein Thema) listet die offenen Kommentare mit Datei, Abschnitt, Stelle und Text.
2. Arbeite jeden Kommentar in das genannte Dokument ein. Ist er unklar oder widerspricht er einem anderen Kommentar, frage den Benutzer, statt zu raten.
3. Markiere ihn danach mit `harness spec resolve K-… "Was du geändert hast, in einem Satz"`. Die `*.comments.json`-Dateien bearbeitest du nie von Hand.
4. Nenne dem Benutzer am Ende, welche Kommentare du eingearbeitet hast.

## 2. Dateistruktur zuerst

Lege vor dem Zuschneiden fest, welche Dateien entstehen oder sich ändern und wofür jede zuständig ist. Eine Verantwortung je Datei, eine Klasse oder Komponente je Datei, bestehende Muster des Projekts beibehalten. Aus dieser Struktur ergeben sich die Aufgaben.

## 3. Aufgaben zuschneiden

- Eine Aufgabe ist die kleinste Einheit mit eigenem Testzyklus. Setup, Konfiguration und Doku gehören in die Aufgabe, die sie braucht. Trenne nur dort, wo ein Reviewer die eine Aufgabe ablehnen und die Nachbarin annehmen könnte.
- Jede Aufgabe endet mit einem einzeln prüfbaren Ergebnis.
- Gliedere in Phasen: Setup, Grundlagen (blockieren alles), je eine Phase pro User Story in der Reihenfolge der Spec, zuletzt Feinschliff. In dieser Reihenfolge legst du die Tickets an und verkettest sie.
- Halte fest, welche Aufgaben parallel laufen können (andere Dateien, keine offene Abhängigkeit) – sie bekommen keine Voraussetzung aufeinander.

## 4. Vorgaben

Die Vorgaben gelten für jede Aufgabe. Sie stehen nicht in einem Dateikopf, sondern im Feld `handbook` jedes Tickets (aus Plan, Spec und den Regeln des Projekts, nichts erfinden):

```markdown
**Ziel**: ein Satz, was die Funktion leistet.
**Ansatz**: zwei, drei Sätze zur Architektur aus dem Plan.
**Technik**: Sprache, Bibliotheken, Testwerkzeug.

## Vorgaben

Regeln, die für jede Aufgabe gelten, je eine Zeile, Werte wörtlich aus Spec und Projektregeln
(zum Beispiel: Node-Version, Sprache der Texte, „eine Klasse pro Datei“, „jede Methode mit Kommentar“).
```

Ein Umsetzer liest die Vorgaben mit; sie gelten für jede Aufgabe, auch wenn sie dort nicht wiederholt werden.

## 5. Format einer Aufgabe

So sieht der Text einer Aufgabe aus; er wird später der Plan ihres Tickets (`plan` in `payload.json`). Die erste Zeile ist der Titel, darunter eingerückt (zwei Leerzeichen) die Details. Folgezeilen eines Details sind mit vier Leerzeichen eingerückt; so sind mehrzeilige Texte und Codeblöcke möglich.

````markdown
Ticket-Verlauf anzeigen
  - Beschreibung: Die Ticketseite zeigt unter der Beschreibung den Verlauf des Tickets, neueste Einträge oben.
    Ohne Einträge steht dort „Noch kein Verlauf“. Grundlage ist FR-004 der Spec.
  - Akzeptanz: Drei Einträge erscheinen in umgekehrter Reihenfolge der Entstehung; ein leerer Verlauf zeigt den Hinweis „Noch kein Verlauf“
    - der Autor und die Zeit stehen bei jedem Eintrag (FR-004, US1 Szenario 2)
  - Dateien: packages/web/src/components/TicketHistory.tsx (neu), packages/web/tests/TicketHistory.test.tsx (neu)
  - Nutzt: GET /api/tickets/:id/history; Typ TicketEntry
  - Liefert: <TicketHistory entries={TicketEntry[]}>
  - [ ] Test schreiben in packages/web/tests/TicketHistory.test.tsx
    ```tsx
    it('zeigt den Hinweis bei leerem Verlauf', () => {
      render(<TicketHistory entries={[]} />);
      expect(screen.getByText('Noch kein Verlauf')).toBeTruthy();
    });
    ```
  - [ ] Test ausführen, muss fehlschlagen
    Befehl: `npx vitest run tests/TicketHistory.test.tsx` im Ordner packages/web
    Erwartet: FAIL, `TicketHistory` ist nicht definiert
  - [ ] TicketHistory minimal umsetzen
    ```tsx
    export function TicketHistory({ entries }: { entries: TicketEntry[] }) {
      if (entries.length === 0) return <p>Noch kein Verlauf</p>;
      return <ol>{[...entries].reverse().map((e) => <li key={e.at}>{e.text}</li>)}</ol>;
    }
    ```
  - [ ] Test ausführen, muss bestehen
    Befehl: wie oben
    Erwartet: PASS, 1 Test
  - [ ] Committen
    Befehl: `git commit -m "feat(web): Verlauf auf der Ticketseite"`
````

**Titel** (die erste Zeile, später `--title` bzw. das erste Argument von `harness board create`): ein knapper Satz im Imperativ, höchstens etwa 60 Zeichen, beschreibt das Ergebnis und nicht die Dateiarbeit. Keine Pfade, Klassen-, Paket- oder Funktionsnamen im Titel. Richtig: „Server-Paket anlegen“. Falsch: „Paket `server` anlegen: `packages/server/package.json` (Abhängigkeiten …)“.

**Wohin die Teile gehen**: Titel → `harness board create "<Titel>"`, `Beschreibung:` → `--desc`, `Akzeptanz:` → `--acceptance`, jeder Schritt → ein `--step`, der volle Aufgabentext samt Code → `plan` in `payload.json`.

**Details** (genau diese Beschriftungen):

- `Beschreibung:` Pflicht. Ein bis vier Sätze: was die Aufgabe bewirkt, für wen, welches Verhalten sichtbar wird, und woher es kommt (Anforderung, Story). Jemand ohne Kenntnis der Spec muss danach wissen, was gebaut wird. Nicht den Titel wiederholen.
- `Akzeptanz:` Pflicht. Prüfbare Aussagen mit konkreten Werten („`[1, 2, 3]` ergibt 6“, nicht „Summe stimmt“). Mehrere mit Semikolon trennen oder als Unterpunkte darunter; am Ende der Anforderung der Spec nennen (`FR-002`, `US1 Szenario 1`).
- `Dateien:` kommagetrennt, genaue Pfade, „(neu)“ bei neuen Dateien.
- `Nutzt:` was die Aufgabe von früheren Aufgaben verwendet, mit genauen Namen und Typen; mehrere Einträge mit Semikolon trennen. Weglassen, wenn nichts genutzt wird.
- `Liefert:` was spätere Aufgaben von ihr erwarten, genaue Namen, Parameter- und Rückgabetypen; Semikolon-getrennt. Weglassen, wenn nichts geliefert wird.
- Schritte als `- [ ] …`, jeder eine Handlung von zwei bis fünf Minuten, jeder mit einem Text, der innerhalb der Aufgabe einmalig ist. Die Zeile nennt die Handlung, die Folgezeilen darunter die Anleitung dazu.
- Ein Mermaid-Diagramm (```` ```mermaid ````) ist erlaubt, wenn es einen Ablauf oder Zusammenhang klarer zeigt als Text. Es gehört in die Anleitung eines Schritts, nicht in `Beschreibung:` oder `Akzeptanz:`, dort geht die Struktur verloren. Die Schrittzeile nennt, was das Diagramm zeigt.

**Schritte für Code**: Test schreiben, Test ausführen (muss fehlschlagen), minimal umsetzen, Test ausführen (muss bestehen), committen.

- Test schreiben und Umsetzen enthalten den tatsächlichen Code in einem Codeblock, nicht eine Beschreibung davon. Der Test nennt die Testfälle samt Eingabe und erwartetem Wert.
- Test ausführen enthält den genauen `Befehl:` (mit Ordner, falls nötig) und die `Erwartet:`-Zeile mit dem Ergebnis („FAIL, `Greeter` ist nicht definiert“, „PASS, 2 Tests“).
- Committen enthält die Commit-Nachricht.
- Aufgaben ohne Code (Doku, Konfiguration) bekommen passende Schritte mit dem genauen Text oder der genauen Einstellung, einem Prüfbefehl und dem erwarteten Ergebnis.
- Größere Codeblöcke gehören als Codeblock in die Folgezeilen; die Schrittzeile bleibt kurz.

## 6. Keine Platzhalter

Das sind Fehler einer Aufgabe, schreibe sie nie:

- „TBD“, „später ergänzen“, „Details folgen“.
- „Fehlerbehandlung hinzufügen“, „Eingaben prüfen“, „Randfälle behandeln“, ohne zu sagen welche und mit welchem Ergebnis.
- „Tests schreiben“ ohne Testcode, „Test für das Obige“.
- Schritte, die sagen, was zu tun ist, aber nicht wie (bei Code fehlt der Codeblock).
- „wie in der vorigen Aufgabe“ statt der Angabe selbst (der Umsetzer liest nur dieses Ticket).
- Namen oder Typen, die in keiner Aufgabe definiert sind.
- Eine Beschreibung, die nur den Titel wiederholt, oder Akzeptanz ohne konkrete Werte.

## 7. Selbstprüfung

Gehe die fertigen Aufgaben einmal selbst durch, ohne Subagent, bevor du Tickets anlegst:

1. **Abdeckung**: Jede Anforderung und jede User Story der Spec hat Aufgaben. Lücken ergänzen.
2. **Selbsttragend**: Lies jede Aufgabe, als sähest du nur sie und die Vorgaben. Weißt du, was zu bauen ist, wie du es prüfst und welchen Befehl du ausführst? Sonst ergänzen.
3. **Platzhalter**: Suche nach den Mustern aus Abschnitt 6.
4. **Konsistenz**: Namen und Typen in Code, `Nutzt:` und `Liefert:` stimmen zwischen den Aufgaben überein.
5. **Titel**: Jeder Titel ist kurz, ohne Pfade und liest sich als Ergebnis. Sprich ihn einmal laut: Ist er doppeldeutig?

## 8. Tickets anlegen

- Committe zuerst Spec und Plan unter `docs/superpowers/`, bevor Tickets gestartet werden: Worktrees und Agenten sehen nur den letzten Commit; `harness board start` warnt sonst.
- Prüfe mit `harness board list` auf Dubletten: Gibt es zu einer Aufgabe schon ein Ticket, lege keins an, sondern nenne es.
- Lege dann je Aufgabe in der Reihenfolge der Phasen ein Ticket an:

```bash
harness board create "<Titel>" --desc "<Beschreibung>" --acceptance "<Akzeptanz>" --step "<Schritt 1>" --step "<Schritt 2>" --depends-on <ID der vorigen Aufgabe> --by claude
harness board plan-json <ID> --file payload.json --by claude
```

  `--depends-on` nennt das Ticket der vorigen Aufgabe (bei der ersten und bei parallelen Aufgaben entfällt es). In `payload.json` ist `plan` der volle Aufgabentext samt Code, `handbook` die Vorgaben und Testbefehle aus Abschnitt 4, `diagram` ein kleines Mermaid-Diagramm **ohne** Code-Zaun. Der Plan endet mit `## Aufwand` (siehe Skill `ai-harness:board`, „Tickets aus einem Superpowers-Plan“ – diesen Weg gehst du hier).
- Die Tickets landen auf dem Board, das diesem Repo zugeordnet ist; ist keines zugeordnet, nenne dem Benutzer `harness board boards list` und `harness board boards repo <id> <pfad>`.
- Kleinere Schnitte macht der Benutzer selbst auf der Seite „Docs“: Stelle markieren, „Zur Ticket-Auswahl“, Entwürfe erzeugen, prüfen und anlegen.
- Nenne dem Benutzer: die angelegten Tickets, die Anzahl der Aufgaben je Story, parallele Möglichkeiten, vorgeschlagenen Umfang für den ersten Schnitt.
- Schlage als Nächstes `/ai-harness:harness-implement` vor. Sollen die Tickets vom Dispatcher umgesetzt werden: `harness board set <id> --label <bisherige> --label auto` (`set --label` ersetzt die Labels, bisherige mit angeben).

## Regeln

- Tickets nie von Hand ändern; sie entstehen über `harness board create` und `harness board plan-json`.
- Keine Worktrees und keine Branches anlegen; das macht `harness board start`.
- Setze keine Kästchen von Aufgaben oder Schritten auf `[X]`; das tun `harness board step` und `harness board done`.
- Texte für Menschen in natürlichem Deutsch.
