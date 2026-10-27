---
name: board
description: Ticket-Tracking auf dem Board (SQL Server oder SQLite) – Tickets anlegen, übernehmen, kommentieren, blockieren, abschließen, planen, Fragen an den Nutzer stellen und Notizen für spätere Tickets festhalten. Nutze sie vor jeder mehrschrittigen Aufgabe, bei jeder Ticket-ID wie ARC-0007, sobald ein Plan freigegeben ist, bei Fragen nach Stand oder offenen Aufgaben, wenn dir ein Befund über etwas anderes auffällt, wenn eine Entscheidung des Nutzers fehlt, wenn ein Ticket für den Dispatcher vorbereitet werden soll, und ungefragt bei Fertigstellung oder Blocker. Nicht für Azure DevOps, Jira oder GitHub.
---

# Board bedienen

Das Board ist der Aufgaben-Tracker – nicht GitHub, nicht GitLab und nicht die Work Items des
Azure-DevOps-Servers (die werden nur gelesen, wenn der Nutzer ausdrücklich danach fragt).
Alle Daten liegen in der Board-Datenbank – gemeinsam auf SQL Server (jeder Entwickler hat eigene
Boards in Projekten, sieht aber alle) oder lokal als SQLite-Datei (nur dieser Rechner). Tickets und
Notizen werden nie von Hand in der Datenbank geändert, sondern nur über `harness board …`.

## Sieben Regeln für jede Aufgabe

1. **Vor einer mehrschrittigen Aufgabe nachsehen**, ob es schon ein Ticket gibt
   (`harness board list --q "stichwort" --md`), sonst eines anlegen. Kein doppeltes Ticket.
2. **Vor der Umsetzung übernehmen** (`harness board claim <id> --agent claude`), Zwischenstände
   als Kommentar festhalten, am Ende nach Erledigt mit Abnahme.
3. **Ein freigegebener Plan gehört aufs Ticket** – vor der ersten Codeänderung (siehe *Planen*).
4. **Ohne Abnahme kein Erledigt.** Am Ticket steht, *woran man sieht*, dass es fertig ist;
   sonst weist das Board den Wechsel ab.
5. **Ein Blocker gehört ins Board**, mit Grund. Fehlt eine Entscheidung des Nutzers, wird daraus
   eine **Frage** (`harness board question add … --ticket <id>`), kein Blockiert-Grund.
6. **Reihenfolge gehört in die Voraussetzungen** (depends-on), nicht in den Beschreibungstext.
7. **Befunde über etwas anderes werden eine Notiz**, kein Kommentar im laufenden Ticket und
   kein Ticket auf Verdacht. Gibt es den Befund schon, wird er hochgezählt (`note seen`), nicht neu notiert.

## Zugang

Die Verbindung steht in `~/.harness/board.yaml` und wird in der Oberfläche unter
**Einstellungen → Board-Datenbank** gepflegt. Meldet ein Befehl, dass die Datei fehlt oder die
Datenbank nicht erreichbar ist, sag das dem Nutzer und hör auf – nichts raten, keine
Zugangsdaten selbst eintragen.

Ohne `--board` arbeitet jeder Befehl im eingerichteten Repo auf dem **Board dieses Repos**
(Zuordnung mit `harness board boards repo <id> <pfad>`), sonst auf dem **aktiven Board** dieses Rechners.
Der Repo-Pfad ist **je Person lokal** (`~/.harness/board-repos.yaml`), nicht in der Board-Datenbank: jeder setzt
seinen eigenen Ordner, `harness board boards repo <id>` zeigt ihn, `--clear` entfernt ihn. Einen fremden Pfad nie übernehmen.
`harness board start` und `done` nehmen nur Tickets des Boards an, das dem Repo zugeordnet ist.

```bash
harness board boards list
harness board list --board all --md
```

Nur auf ausdrückliche Anweisung des Nutzers: `harness board boards use <id>`,
`harness board boards create <projekt> <name>`, `harness board boards delete <id>`,
`harness board project create <key> <name>`.

## Spalten

`backlog` → `in_planning` → `planned` → `in_progress` → `done`, quer dazu `blocked`, dahinter
`not_needed`. Deutsche Namen gehen auch: `in-planung`, `geplant`, `in-arbeit`, `blockiert`,
`erledigt`, `nicht mehr nötig`.

- **Nicht mehr nötig gehört dem Nutzer.** Leg dort nichts ab – `move` und `claim` weisen dich ab.
  Hältst du ein Ticket für überflüssig, schreib einen Kommentar. Nur auf ausdrückliche Anweisung
  `--force`, mit Kommentar warum.
- **In Planung gehört dem Nutzer.** `next-planning` überspringt diese Tickets.
- **Wartet** ist abgeleitet: Tickets in Backlog, In Planung oder Geplant mit offener
  Voraussetzung. Dorthin verschieben kann man nichts.
- Die Reihenfolge in einer Spalte ergibt sich aus der Priorität (`urgent` zuerst) und der
  Ticketnummer. Soll etwas weiter oben stehen: `harness board set <id> --priority high`.

## Abhängigkeiten

```bash
harness board create "Fassade" --depends-on ARC-0001,ARC-0002
harness board set ARC-0003 --depends-on-add ARC-0004
harness board set ARC-0003 --depends-on-rm ARC-0001
harness board set ARC-0003 --depends-on none
harness board list --status wartet --md
```

Solange eine Voraussetzung offen ist (nicht erledigt, nicht „nicht mehr nötig“), scheitern
`claim` und `move` nach In Arbeit oder Erledigt. `--force` übergeht das – nur mit Kommentar warum.
Voraussetzungen dürfen auf Tickets anderer Boards zeigen; Kreise werden abgelehnt.

## Erst lesen, dann handeln

```bash
harness board list --md
harness board list --status geplant --md
harness board list --needs-planning --md
harness board show ARC-0003
harness board show ARC-0003 --all-comments --history
harness board show ARC-0003 --field plan
harness --json board show ARC-0003
```

`--md` ist der Ausgang für Übersichten, `--json` liefert alles vollständig.
`show` ist standardmäßig kompakt: ohne Verlauf und ohne ältere Agenten-Kommentare (Kommentare von Menschen bleiben
immer stehen). `--history` und `--all-comments` holen den Rest dazu; `--json` und `--field` sind davon nicht betroffen.

## Meine Aufgaben

Fragt der Nutzer, was auf ihn wartet, oder will er seinen Eingang sehen:

```bash
harness board inbox --md
harness board inbox --kind merge,accept --md
harness board inbox --board 5 --md
```

Die Liste umfasst alle **eigenen** Boards: gescheiterte Merges und Einrichtungen, offene Fragen (ein Ticket mit offener
Frage erscheint nur über die Frage), Abnahmen (`Human Interaction`), sonst blockierte Tickets, Tickets in Planung und
offene Notizen. Jeder Eintrag nennt den Befehl, mit dem es weitergeht. Die Reihenfolge legt der Nutzer in der Oberfläche
fest (lokal in `~/.harness/inbox-order.json`); neue Einträge stehen oben. Abnehmen und Entscheiden bleiben beim Nutzer.

## Aufgabe aufnehmen

```bash
harness board create "Titel der Aufgabe" --desc "Kontext" --acceptance "Woran man es sieht" --priority high --label backend --by claude
```

`--label` und `--step "Text"` (Arbeitsschritt, in dieser Reihenfolge) dürfen mehrfach stehen. `--planned` setzt den
Planungsbedarf nicht (wenn du direkt planst oder kein Plan nötig ist). Gibt es auf dem Board schon ein offenes Ticket mit gleichem Titel (Groß-/Kleinschreibung
egal), lehnt `create` ab und nennt dessen ID (Code `ticket-duplicate`); nur wenn es wirklich ein eigenes Ticket sein
soll: `--force`, mit Kommentar warum. Ähnliche, aber anders formulierte Tickets erkennt `create` nicht – vorher mit
`harness board list` nachsehen.

## Planen

```bash
harness --json board next-planning --claim claude
harness board plan-json ARC-0001 --file payload.json --by claude
```

`payload.json` enthält `plan`, `handbook` und `diagram` (Mermaid **ohne** Code-Fence), optional `acceptance`
(Abnahme; fehlt sie, bleibt die vorhandene). Das setzt
Geplant, löscht den Planungsbedarf und den Claim. Ein Claim verfällt nach 15 Minuten.

Nur ein Feld schreiben, ohne die Planung als fertig zu melden:

```bash
harness board set ARC-0001 --plan-file plan.md --by claude
```

**Tickets aus einem Superpowers-Plan:** Sagt der Nutzer „Lege für die Tasks des Plans Tickets an“, gilt der Plan unter
`docs/superpowers/plans/…` (ohne Angabe der neueste). Zuerst mit `harness board list` auf Dubletten prüfen: Gibt es zu einer
Task schon ein Ticket, keins anlegen, sondern es nennen. Dann je Task in der Reihenfolge des Plans:

```bash
harness board create "<Task-Titel>" --desc "<Ziel der Task>" --acceptance "<woran man es sieht>" --step "<Task-Schritt 1>" --step "<Task-Schritt 2>" --depends-on <ID der vorigen Task> --by claude
harness board plan-json <ID> --file payload.json --by claude
```

`--step` steht je Task-Schritt in dieser Reihenfolge; `--depends-on` nennt das Ticket der vorigen Task (beim ersten
entfällt es). Nennt der Nutzer einen Zielbranch (z. B. `release/1`), steht `--target-branch <name>` bei **jedem**
`board create`: In diesen Branch mergt der Dispatcher, ein fehlender wird beim Start angelegt; ohne Angabe gilt der
ausgecheckte Branch des Hauptordners. Nach dem Start lässt sich der Zielbranch nicht mehr ändern. In
`payload.json` ist `plan` der vollständige Task-Text samt Code, `handbook` die globalen Vorgaben und
Testbefehle des Plans, `diagram` ein kleines Mermaid-Diagramm **ohne** Code-Zaun. Auch dieser Plan endet mit
`## Aufwand` (siehe „Aufwand schätzen“). `plan-json` setzt Geplant und löscht den Planungsbedarf. Plan und Handbuch stehen danach im
Ticket; Worktrees und Agenten brauchen die Plan-Datei nicht. Für den Dispatcher danach wie unter „Für den Dispatcher
vorbereiten“. Der Skill `ai-harness:harness-tasks` geht genau diesen Weg: Er schneidet die Aufgaben zu und legt sie
hierüber als Tickets an.

**Tickets aus einem Spec-Dokument:** Aus den Dokumenten unter `docs/superpowers` entstehen Tickets über die Seite
„Docs“ im Web: Stellen markieren, Entwürfe erzeugen lassen, prüfen und anlegen (serverseitig `POST
/api/projects/:pid/docs/tickets`). Die Tickets landen im **Backlog mit Planungsbedarf** und werden danach wie oben
geplant (`next-planning` / `plan-json`); Labels wie `auto` ergänzt `harness board set <ID> --label …` (ersetzt die
Labels, bisherige mit angeben). Ist das Spec-Dokument nicht (vollständig) committet, warnt `board start`: Worktrees und
Agenten sehen nur den letzten Commit – erst committen.

**Plan aus dem Plan-Modus:** Nach der Freigabe ist der erste Befehl das Schreiben des Plans
(`plan-json`, wenn das Ticket noch Planung braucht; `set --plan-file`, wenn es schon läuft;
gibt es kein Ticket, zuerst `create`). Den freigegebenen Text unverändert übernehmen.

**Aufwand schätzen:** Jeder Plan endet mit einem Abschnitt

```markdown
## Aufwand
standard – sechs Dateien, neue Route, Tests vorhanden
```

Erstes Wort `klein`, `standard` oder `schwer` (Namen der Profile in den Dispatcher-Einstellungen), danach eine kurze
Begründung. Du hast den Code gerade gelesen und kennst den Umfang besser als jede Regel; der Dispatcher wählt danach Modell
und Effort für die Umsetzung. Fehlt der Abschnitt, greifen nur seine groben Regeln.

## Umsetzen

```bash
harness board claim ARC-0001 --agent claude
harness board comment ARC-0001 "Schritt 2 von 4 fertig, Tests laufen." --author claude
harness board move ARC-0001 erledigt --acceptance "Test deckt den Fall ab" --by claude
```

Ist ein Ticket schon zugewiesen (auch an `claude`), endet `claim` mit Exit 1. Hast du es früher
in dieser Sitzung selbst übernommen, arbeite einfach weiter. Sonst nimm das nächste freie Ticket
oder frag nach. Nie `--force` ohne den Nutzer.

### Im eigenen Worktree (Repo mit Board)

Gehört das Repo zu einem Board (`harness board boards repo <id> <pfad>`), arbeitet jedes Ticket in seinem Worktree:

```bash
harness board start ARC-0001 --by claude
harness board step ARC-0001 1 --by claude
harness board done ARC-0001 --acceptance "Test deckt den Fall ab" --by claude
```

`start` legt einen eigenen Branch `ticket/<ID>-<Kurztitel>` vom Zielbranch des Tickets bzw. vom ausgecheckten Branch und einen eigenen Worktree an und setzt In Arbeit – mit denselben Sperren wie `move`. Mit `--join` tritt das Ticket stattdessen einem offenen Worktree derselben Spec bei, wenn sich die Bereiche nicht überschneiden (früheres Verhalten; `--in <ID>` wählt den Worktree selbst). `step` hakt Schritte ab, `done` setzt Erledigt (Abnahme Pflicht) und räumt fertige Worktrees auf. Gemergt wird nur vom Dispatcher (nach Erledigt eines von ihm umgesetzten Tickets, jedes Ticket einzeln, sobald die Tickets seines Worktrees fertig sind) oder vom Menschen. Gehören Branch und Worktree eines Tickets nicht mehr zu ihm, leert `harness board set <ID> --clear-branch` beide Felder (Branch und Ordner selbst bleiben).

### Schritte

Schritte sind die nummerierte Arbeitsliste eines Tickets (`harness board show` listet sie mit Nummer ab 1, optional mit
Anleitung). Jede Änderung erhöht die Version und steht im Verlauf.

```bash
harness board step ARC-0001 2 --by claude                      # abhaken (gleich: step done ARC-0001 2)
harness board step ARC-0001 2 --undo                           # Haken zurücknehmen
harness board step add ARC-0001 "Test schreiben" --detail "npm test – erwartet: rot" --by claude
harness board step add ARC-0001 "Migration prüfen" --at 2      # an Nummer 2 einfügen
harness board step edit ARC-0001 2 --text "Neuer Text"         # --detail "" entfernt die Anleitung
harness board step move ARC-0001 4 1                           # Schritt 4 an Nummer 1
harness board step rm ARC-0001 3                               # löschen, folgende rücken nach
```

Nummern sind nach `add --at`, `move` und `rm` neu – vor dem nächsten Befehl mit `show` nachsehen. Optional
schützt `--expect "<bisheriger Text>"` (bei `step`/`done`/`edit`/`rm`/`move`) bzw. `--rev <n>` vor gleichzeitigen
Änderungen: steht dort inzwischen etwas anderes, bricht der Befehl mit einem Konflikt ab – neu lesen und wiederholen.

### Fertig – aber wohin?

| Ausgang | wann |
| --- | --- |
| `erledigt` | fertig **und** selbst nachgewiesen; der Nachweis steht im Kommentar |
| `blockiert` + `Human Interaction: …` | fertig, aber der Nachweis war nicht selbst zu erbringen |
| Frage mit `--ticket <id>` | nicht fertig, eine Entscheidung des Nutzers fehlt |
| `blockiert` + Grund | nicht fertig, es fehlt etwas anderes (Zugang, Umgebung) |

```bash
harness board comment ARC-0001 "Umgesetzt. Zu prüfen: … Erwartung: … Nicht geprüft: …" --author claude
harness board set ARC-0001 --status blockiert --blocked-reason "Human Interaction: Oberfläche ansehen" --by claude
```

Ein `Human Interaction` nimmt nur der Nutzer ab, nie ein Agent: `harness board accept <ID>` (im Panel „Abnehmen“) setzt
Erledigt, leert den Grund und stößt auf einem eigenen Board den Merge an.

## Für den Dispatcher vorbereiten

Sagt der Nutzer „bereite ARC-0007 für den Dispatcher vor“ (oder „mach ARC-0007 fertig für den Dispatcher“), bringst du
das Ticket in den Zustand, in dem der Dispatcher es übernimmt, und legst das Profil fest. Ablauf:

1. **Lesen:** `harness board show ARC-0007` (Kommentare, Plan, Handbuch, Abnahme, Labels, Abhängigkeiten, Fragen;
   ältere Agenten-Kommentare und Verlauf bei Bedarf mit `--all-comments --history`) und
   `harness board dispatch status` (nennt den Grund, warum ein Ticket noch nicht bereit ist).
2. **Prüfen, was fehlt.** Bereit ist ein Ticket nur, wenn alles davon erfüllt ist:
   - Status Geplant, kein Zuständiger, kein Planungsbedarf;
   - Plan, Handbuch und Abnahme vorhanden;
   - keine offene Abhängigkeit, keine offene Frage; Voraussetzungen sind erledigt **und gemergt** (kein Branch mehr),
     sonst nennt `dispatch status` „wartet auf Merge von …“;
   - Label `auto`, nicht Label `manuell`;
   - das Board gehört dem Nutzer und hat seinen (lokalen) Repo-Pfad, den es gibt.
3. **Ergänzen, was fehlt** – im Code lesen, nicht raten:
   - Plan, Handbuch oder Diagramm fehlen: planen wie unter „Planen“ (`plan-json` setzt Geplant und löscht den
     Planungsbedarf), mit Abschnitt `## Aufwand` am Ende des Plans. Ein Ticket mit Plan nur ergänzen
     (`set --plan-file`), den vorhandenen Plan nicht wegwerfen.
   - Abnahme fehlt: `harness board set ARC-0007 --acceptance "…" --by claude`, prüfbar formuliert.
   - Schritte fehlen und der Plan hat erkennbare Arbeitsschritte: als Schritte anlegen (siehe „Schritte“).
   - Fehlt eine Entscheidung, die nur der Nutzer treffen kann: eine Frage anlegen (siehe „Fragen“) und dem Nutzer sagen,
     dass das Ticket bis zur Antwort wartet – nicht selbst entscheiden.
   - Offene Abhängigkeiten, Board ohne eigenen Repo-Pfad oder fremdes Board nur melden, nicht umbiegen.
4. **Aufwand einschätzen.** Nach dem Lesen von Ticket und Code ein Profil wählen: `klein`, `standard` oder `schwer`
   (Namen der Profile in den Dispatcher-Einstellungen; ein unbekannter Name blockiert das Ticket). Maßstab: Anzahl und
   Art der betroffenen Dateien, neue Schnittstellen, Risiko, vorhandene Tests. Die Einschätzung steht gleichlautend im
   Abschnitt `## Aufwand` des Plans (erstes Wort = Profil, danach die Begründung).
5. **Labels setzen.** Immer `auto` **und** `profil:<name>` zusammen. `set --label` ersetzt alle Labels: die bisherigen
   mitgeben, ein altes `profil:…` dabei weglassen:
   ```bash
   harness board set ARC-0007 --label frontend --label auto --label profil:standard --by claude
   ```
   `auto` nie ohne `profil:` setzen. `model:` und `effort:` nur, wenn der Nutzer es verlangt.
6. **Begründen und prüfen.** Die Einschätzung als Kommentar: `harness board comment ARC-0007 "Für den Dispatcher
   vorbereitet: profil:standard – <Begründung>; ergänzt: <was>" --author claude`. Dann noch einmal
   `harness board dispatch status`: Das Ticket darf dort keinen Grund mehr haben. Dem Nutzer Profil, Begründung,
   Ergänztes und Offenes nennen.

Mehrere Tickets („bereite ARC-0007 bis ARC-0010 vor“) nacheinander genauso. Starten musst du nichts – der Dispatcher
prüft jede Minute (sofort: `harness board dispatch run`, nur wenn der Nutzer es will).

## Vom Dispatcher gestartet

Der Harness-Server übernimmt bereite Tickets (Geplant, Label `auto`, Plan, Handbuch und Abnahme vorhanden) der eigenen
Boards und startet dafür `claude -p` im Worktree des Tickets. Wer so gestartet wurde, heißt `junior-<ID>` und hat sein
Ticket schon:

1. Das Ticket ist übernommen: **nicht erneut claimen**, kein anderes Ticket anfassen, nicht `start` aufrufen – Branch und
   Worktree stehen schon.
2. Zuerst `harness board show <ID>` lesen (Kommentare, Plan, Handbuch; die Ausgabe ist kompakt, ältere
   Agenten-Kommentare und der Verlauf kommen mit `--all-comments --history` dazu). Stehen in der Arbeitsanweisung Entscheidungen zu
   Fragen, gelten sie vor Plan und Handbuch. Vorhandene Commits auf dem Branch stammen aus einem früheren Lauf – dort
   weitermachen. Zwischenstände mit `step` und `comment --author junior-<ID>`.
3. Genau ein Ausgang, immer mit `--by junior-<ID>`:
   - **erledigt:** alles committet, Nachweis als Kommentar, dann `harness board done <ID> --acceptance "…" --by junior-<ID>`.
     Danach mergt der Dispatcher in den Zielbranch des Tickets bzw. den Branch, der im Hauptordner ausgecheckt ist, und räumt Worktree und Branch auf.
     Je nach Einstellung `mergeStrategy` als Merge-Commit (alle Commits bleiben) oder als Squash (ein Commit je Ticket) –
     Zwischencommits sind also unbedenklich, die Commit-Nachrichten trotzdem aussagekräftig halten.
     Mit `reviewBeforeMerge` (Standard) wartet der Dispatcher vorher auf die Freigabe eines Menschen: Das Ticket steht
     dann auf Blockiert mit Grund `Human Interaction: Review vor dem Merge`, im Panel mit Diff und deinen Testläufen.
     Tests, Build und Lint deshalb mit Bash oder PowerShell ausführen – nur so erscheinen sie im Review.
   - **Human Interaction:** `harness board set <ID> --status blockiert --blocked-reason "Human Interaction: …" --by junior-<ID>`.
     Gemergt wird erst, wenn der Nutzer abnimmt (`harness board accept`); andere Tickets warten nicht darauf.
   - **Frage:** `harness board question add "…" --description "…" --ticket <ID> --author junior-<ID>` (nach den Schreibregeln,
     siehe „Fragen“), dann den Lauf beenden.
     Das Ticket geht zurück nach Geplant und läuft nach der Entscheidung im selben Worktree weiter.
   - **blockiert mit Grund** (Zugang, Umgebung): `harness board set <ID> --status blockiert --blocked-reason "…" --by junior-<ID>`.
4. Steht in der Arbeitsanweisung eine **Rückmeldung aus dem Review**, hat ein Mensch die Umsetzung angesehen und
   zurückgegeben. Sie gilt vor Plan und Handbuch; es geht im selben Worktree und Branch weiter, danach wieder „erledigt“.
5. Ohne Ausgang gilt der Lauf als abgebrochen: `[dispatch] Abbruch`, zurück nach Geplant, nächster Versuch eine Stufe
   größer; nach dem letzten Versuch Blockiert. Auch ein gescheiterter Start zählt als Versuch.
6. Nicht pushen, nicht mergen, Branch nicht wechseln.
7. Keine Links, Junctions oder Symlinks auf den Hauptordner oder andere Worktrees anlegen (auch nicht für `node_modules`) –
   beim Aufräumen des Worktrees könnte sonst das Ziel leiden. Fehlen Abhängigkeiten, im Worktree selbst installieren
   (z. B. `npm ci`); geht das nicht, Ausgang „blockiert mit Grund“. Ist eine Einrichtung eingestellt (`worktreeSetup`), hat der
   Dispatcher sie im neuen Worktree vor dem Start ausgeführt; scheitert sie, startet kein Agent und das Ticket steht auf
   Blockiert mit Grund `Einrichtung fehlgeschlagen: …`.

Was du tust, sieht der Nutzer live im Protokoll des Laufs (Reiter „Agent“ im Ticket-Panel, bei Recherchen „Recherche-Läufe“
an der Frage): Texte, gelesene und geänderte Dateien, Befehle.

Für alle anderen gilt: Tickets mit Zuständigem `junior-…` gehören dem Dispatcher, solange sie In Arbeit sind – nicht
übernehmen. Kommentare, die mit `[dispatch]` beginnen, nie löschen oder nachahmen; der Dispatcher zählt an ihnen die
Versuche. Lokale Änderungen im Hauptordner halten den Merge nur an, wenn sie vorgemerkt sind (git add) oder Dateien
betreffen, die auch der Branch ändert; dann wartet er (Kommentar „Merge wartet auf den Hauptordner“) und läuft von selbst,
sobald sie committet oder verworfen sind. Steht ein Ticket auf Blockiert mit Grund `Merge: …`, ist der Merge gescheitert
(z. B. Detached HEAD, ungelöster Konflikt); nach dem Beheben stößt `harness board merge <ID>` ihn erneut an (im Panel „Erneut mergen“).
Jedes Ticket bekommt einen eigenen Worktree und wird einzeln gemergt (Einstellung `joinWorktrees: none`; `sameSpec`
bündelt wie früher die Tickets einer Spec in einem Worktree). Ein Ticket startet erst, wenn seine Voraussetzungen gemergt
sind – so steckt deren Code in seinem neuen Branch. Gehört der eingetragene Worktree eines fortgesetzten Tickets einem
anderen Ticket oder wurde er aufgeräumt, stellt der Dispatcher ihn nicht wieder her, sondern startet auf eigenem Branch
(Hinweis im Start-Kommentar). Ein Ticket mit Grund `Human Interaction: …` nimmt der Nutzer mit `harness board accept <ID>`
ab (im Panel „Abnehmen“): Erledigt, danach der Merge wie bei `merge`. Ein von Hand auf Erledigt gesetztes Ticket mergt
man mit „Mergen“ im Panel oder `harness board merge <ID>`. Wartet ein Ticket mit Branch auf einem eigenen Board auf
Freigabe (`Human Interaction: …`), zeigt das Panel den Review: Diff gegen den Zielbranch, Testläufe des Agenten,
„Übernehmen“ (wie `accept`: Erledigt und Merge) und „Zurück an den Agenten“ mit Rückmeldung (zurück nach Geplant, der
nächste Lauf setzt im selben Worktree fort, zählt nicht als Versuch). Das geht nur für Tickets eigener Boards; die Ausgabe nennt das Ergebnis für das Ticket
(gemergt mit Commit, blockiert mit Grund, wartet, Merge-Session gestartet).

Die Arbeitsanweisung je Laufart (Umsetzung, Planung, Recherche, Merge) kommt aus einer Vorlage. Standard ist die
mitgelieferte; unter Einstellungen → Dispatcher → Prompts lässt sich je Laufart eine eigene hinterlegen (gespeichert in
`~/.harness/dispatch.yaml`), wobei die Pflichtteile wie die Board-Befehle drin bleiben müssen – sonst lehnt der Server
sie ab. „Vorschau“ rendert dort den fertigen Prompt der noch ungespeicherten Vorlage, mit Beispielwerten oder mit den
echten Werten einer angegebenen Ticket- bzw. Frage-ID, und „Unterschiede zum Standard“ stellt ihn dem Standard-Prompt
zeilenweise gegenüber. Eine eigene Vorlage geht in den Konfig-Hash eines Laufs ein, so bleiben Läufe davor und danach
unterscheidbar.

```bash
harness board dispatch status        # Zustand, laufende Worker, warum Tickets (noch) nicht bereit sind
harness board dispatch run           # sofort prüfen (sonst jede Minute)
harness board dispatch pause --minutes 60
harness board dispatch resume        # enable / disable schalten den Dispatcher ein und aus
harness board merge ARC-0009
harness board accept ARC-0010 --by Florian   # Human Interaction abnehmen (nur der Nutzer)
```

## Vom Planen-Knopf gestartet

„Planen“ im Ticket-Panel (oder `harness board plan-start <ID>`) startet sofort eine Claude-Session `planner-<ID>` in einem
losgelösten Worktree `<repo>-worktrees/plan-<ID>` – unabhängig davon, ob der Dispatcher eingeschaltet ist; nur auf eigenen
Boards mit Repo-Pfad, in Backlog, In Planung, Geplant oder Blockiert, nicht bei offener Frage. Wer so gestartet wurde:

1. Das Ticket ist für die Planung beansprucht (Planungs-Claim): **nicht claimen, nicht verschieben, keine Labels setzen**.
2. Zuerst `harness board show <ID>` und `harness board question list --ticket <ID> --state alle --md` lesen;
   Entscheidungen gelten vor allem anderen. Dann im Code recherchieren (nur lesen).
3. Genau ein Ausgang, immer mit `--by`/`--author planner-<ID>`:
   - **Plan:** `plan.json` mit `plan`, `handbook`, `diagram` schreiben (Plan endet mit `## Aufwand`), dann
     `harness board plan-json <ID> --file plan.json --by planner-<ID>`. Das setzt Geplant.
   - **Frage:** fehlt eine Entscheidung des Nutzers, `harness board question add "…" --description "…" --ticket <ID> --author planner-<ID>`
     (nach den Schreibregeln, siehe „Fragen“) und den Lauf beenden.
4. Ohne Plan und ohne Frage schreibt der Dispatcher `[dispatch] Planung ohne Ergebnis: …` ans Ticket; einen automatischen
   Neuversuch gibt es nicht. Planungen zählen nicht zum WIP; ein Ticket startet nicht zur Umsetzung, solange es geplant wird.

## Fragen

Fehlt eine Entscheidung des Nutzers, stell eine **Frage** statt das Ticket zu blockieren. Die Frage
betrifft ein oder mehrere Tickets; solange sie offen ist, warten diese Tickets: Sie bleiben in ihrer
Spalte, `claim` und `move` nach In Arbeit oder Erledigt scheitern mit „wartet auf Frage …“, und
`next-planning` überspringt sie. Entscheiden (`answer`, `discard`, `reopen`) darf nur der Nutzer –
aus einer Session heraus lehnt das Board das ab. Fragen werden nie von Hand in der Datenbank geändert.

Titel und Beschreibung schreibst du für einen Außenstehenden, der Ticket und Code nicht kennt – nach den
Schreibregeln im Skill `board-question` (Abschnitt „Schreibregeln“): eine Entscheidung je Frage, Titel als Frage ohne
Ticketnummer, Beschreibung mit **Worum es geht**, **Heute** und **Zu entscheiden**.

```bash
harness board question add "Wo speichert der Export seine Zwischenstände?" --ticket ARC-0007 --description "**Worum es geht** …" --occasion "Planung ARC-0007" --kind session --author claude
harness board question list --md
harness board question list --ticket ARC-0007 --state alle --md
harness board question show ARC-F0001
harness board question comment ARC-F0001 "Neuer Befund: …" --kind session --author claude
harness board question link ARC-F0001 --add ARC-0008
```

Varianten und Empfehlung gehören als Recherche an die Frage (Vorgehen wie im Skill `board-question`:
`harness board question research ARC-F0001 --from recherche.json --kind session --author claude`),
nicht in einen Ticket-Kommentar. Ist die Frage entschieden, steht das im Verlauf des Tickets. Wer ein
Ticket aufnimmt, liest zuerst dessen Kommentare und die Fragen dazu
(`harness board question list --ticket ARC-0007 --state alle --md`) – eine Entscheidung gilt vor Plan und Handbuch.

## Notizen

```bash
harness board note list --md
harness board note add "Core-Unit-Tests scheitern, wenn TEMP ein Windows-Kurzpfad ist" \
  --details "GitPaths, LongPaths, WorktreeManagerDispatch: TEMP liefert C:/Users/BENJAM~1.BIB, git den langen Namen. Vorschlag: Pfade im Test mit realpath normalisieren." \
  --from ARC-0009 --label tests --author claude
harness board note seen 12 --from ARC-0014
harness board note list --state alle --md
harness board note take 12 --priority high
harness board note drop 12 --reason "erledigt sich mit ARC-0020"
```

| Was du hast | Wohin |
| --- | --- |
| Zwischenstand am laufenden Ticket | `comment` |
| das Ticket kommt nicht weiter | `set --status blockiert --blocked-reason` |
| Nacharbeit oder Befund über etwas **anderes** | `note add` |
| derselbe Befund steht schon in einer offenen Notiz | `note seen <id>` |

**Erst nachsehen, dann notieren.** Vor `note add` mit `note list --md` prüfen, ob es den Befund schon gibt. Wenn ja,
`note seen <id> --from <ticket>`: Das zählt hoch und merkt sich das Ticket; wie oft etwas auffällt, ist die wichtigste
Information für die Planung. `note add` lehnt einen vermutlich vorhandenen Befund ab (`note-similar`) und nennt die
Kandidaten; `--force` nur, wenn es wirklich etwas anderes ist. Ist der Befund schon als Ticket übernommen, dort kommentieren.

**Schreib für jemanden, der dein Ticket nicht kennt.**
- Titel (`<titel>`): ein Satz, was kaputt ist oder fehlt – ohne Ticket-ID, ohne „siehe oben“, höchstens 200 Zeichen.
- Details (`--details`): wo (Datei, Befehl, Seite), woran man es sieht, vermutete Ursache, Vorschlag. Kurz.
- Nicht hinein gehört, warum dein Ticket nichts damit zu tun hat („unabhängig von ARC-0009“, „nicht durch mich verursacht“,
  „betroffene Dateien: nur …“). Die Herkunft steht in `--from`.
- Labels: vorhandene aus `note list --md` wiederverwenden statt neue Schreibweisen zu erfinden.

Mit `--from` landet die Notiz auf dem Board des Herkunftstickets, und `note take` legt das Ticket dort an.
Ein `--board` aus einem anderen Projekt als das Ticket wird abgelehnt.

Vor dem Planungslauf `note list --md` lesen; was reif ist, wird mit `note take` ein Ticket.
Notizen nicht ungefragt massenhaft zu Tickets machen. Verwerfen nur mit Grund.

## Konflikte

`set` und `move` nehmen `--rev <n>`: Exit 1 heißt, jemand war schneller – neu lesen, neu
entscheiden. Mehrere Entwickler arbeiten auf derselben Datenbank.

## Regeln

1. **Nichts löschen.** `harness board delete` entfernt endgültig – nur auf ausdrückliche Anweisung.
2. **Von Hand geschriebene Inhalte nicht überschreiben.** Erst lesen, im Zweifel kommentieren.
3. **Status ehrlich pflegen.** Kein Erledigt, solange etwas fehlt oder ungetestet ist.
4. **`--by claude` / `--author claude` / `--agent claude` setzen**, damit der Verlauf zeigt, wer gehandelt hat.
5. **Fortschritt kommentieren** statt den Plan umzuschreiben.
6. **Board nicht ungefragt wechseln, anlegen oder löschen.**
7. **Fremde Boards nur bearbeiten, wenn der Nutzer es sagt** – technisch darf jeder alles.
8. **`--force` ist die Ausnahme** und braucht einen Kommentar.
