# Changelog

Änderungen am Harness. Der Abschnitt „Vertrag“ nennt, was Anpassungen in `custom/` nutzen dürfen und was stabil bleibt. Änderungen daran stehen zuerst unter „Angekündigt“ und gelten frühestens eine Version später.

## Unveröffentlicht

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges

- Dispatcher: Die Account-Wahl rechnet den erwarteten Verbrauch gegen die Obergrenze – ein Account gilt als „knapp“, wenn Auslastung plus Reserve des Laufs und der noch ausstehenden Reserve laufender Läufe die Grenze überschreitet; setzt sich das 5-Stunden-Fenster innerhalb von `accountResetGraceMinutes` zurück, zählt die Reserve dafür nicht, für die Woche gibt es keine Ausnahme. Noch mit Reserve 0 verdrahtet, bis die Schätzung folgt – das Verhalten bleibt vorerst wie bisher.
- Einstellungen → Dispatcher: Reserve für Account-Limits einstellbar („Reserve 5 Stunden (%)“, „Reserve Woche (%)“, „Ohne Reserve, wenn 5 h zurücksetzt in (min)“).
- Dispatcher: Einstellung für die Reserve bei Account-Limits – neu sind `accountReserveFiveHour` (Vorgabe 10, 0–50), `accountReserveWeek` (Vorgabe 2, 0–50) und `accountResetGraceMinutes` (Vorgabe 30, 0–300). Die Felder stehen in der Einstellung und im Beispiel, wirken aber noch nicht.
- Mehrere Claude-Accounts im Dispatcher: neuer Bereich **Einstellungen → Accounts** (Datei `~/.nexus/accounts.json`) zum Anlegen, Anmelden, Plugin-Einrichten, Abschalten und Entfernen; je Account Anmeldung, Plugin, Limits und Pause. Profile und Regeln in `dispatch.yaml` nennen mit `accounts: [id, …]` die erlaubten Accounts (ohne Angabe: jeder aktive), das Label `account:<id>` überschreibt beide; neu sind `accountStrategy` (`most-free` | `order`) und `accountLimitPercent` (Vorgabe 90). Je Account schaltet `fallback` (Einstellung „Nur als Fallback“) ihn zum Ersatz, den der Dispatcher erst nimmt, wenn kein anderer erlaubter Account frei ist; `limitFiveHour` und `limitWeek` (1–100, leer = `accountLimitPercent`) setzen eigene Obergrenzen, ab denen er den Account nicht mehr wählt. Jeder Lauf – auch Doku-Überarbeitung, Entwürfe und Analyse – startet mit `CLAUDE_CONFIG_DIR` des gewählten Accounts. Ein Nutzungslimit pausiert nur den betroffenen Account (`accountPauses` in `dispatch/state.json` und im Status), der ganze Dispatcher erst, wenn alle aktiven pausiert sind. Anmelden per Knopf: `claude auth login` im Ordner des Accounts, Code-Eingabe auf der Seite, danach Name aus der E-Mail und Plugin-Einrichtung; den Ordner gibt man nur noch für vorhandene Profile an. Neue Routen `POST /api/accounts/login`, `POST/DELETE /api/accounts/:id/login`, `POST /api/accounts/:id/login/code`, `GET/POST /api/accounts`, `GET /api/accounts/names`, `PATCH/DELETE /api/accounts/:id`, `POST /api/accounts/:id/plugin` und `/resume` sowie `GET /api/claude-usage/accounts`; `RunSettings` tragen `account` und `accountWhy`. Mit nur dem Standard-Account bleibt alles wie bisher; die Limits des Rings liest der Server unter macOS jetzt auch aus dem Schlüsselbund.

## 0.1.3 – 2026-11-02

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges

- Windows-Installer: trägt den Installationsordner (mit `nexus.exe`) in den Benutzer-PATH ein und beim Deinstallieren wieder aus, damit `nexus` im Terminal gefunden wird.
- Windows-Installer im Nexus-Look: App-Icon, Seitenbild und Kopfbild mit Logo im Markenblau.
- Desktop-Entwicklung: `npm run desktop -- dev` lädt die Oberfläche von Vite mit Hot Reload, statt sie bei jedem Start komplett zu bauen. Der Server lässt die Herkunft von Vite in Debug-Builds der App wie die der App zu (nur mit Start-Token).
- Release-Workflow: Oberfläche und Server werden je Plattform nur noch einmal gebaut. Der Server teilt sich mit der App den Build unter `target/<ziel>/release`.

## 0.1.2 – 2026-11-02

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges

## 0.1.1 – 2026-11-02

### Vertrag

- Workflow: Typen `WorkflowPhase`, `WorkflowStep`, `WorkflowOption` aus `@harness/core`; Default-Export von `custom/workflow.ts` ist `WorkflowPhase[]`; Pflicht-Phasen `planen` und `umsetzen`; die Phasen-IDs `fragen` und `nebenbei` sind reserviert.
- Skills: `board`, `board-question`, `harness-grill`, `harness-wayfinder`, `harness-tasks`, `harness-implement`, `harness-setup`, `harness-customize`.
- Output-Styles: `software-engineer`.
- CLI: alle `harness`-Befehle samt Optionen.
- **Vertragsänderung:** `harness spec new` und `harness spec import` entfallen ersatzlos, samt dem automatischen Import aus `tasks.md` im Stop-Hook. Das Spec-Kit-Format (`specs/NNN-slug` mit `tasks.md`) wird nicht mehr unterstützt; Specs und Pläne liegen unter `docs/superpowers/{specs,plans}`, Tickets entstehen auf der Seite „Docs“ (markieren → Ticket-Auswahl → Entwürfe prüfen → anlegen) oder über „Tickets aus einem Superpowers-Plan“ (`harness board create` und `harness board plan-json`).

### Angekündigt

- Keine.

### Sonstiges

- Zielbranch im Ticket-Panel mit Vorschlagsliste: Das Feld schlägt beim Fokus die Branches des Board-Repos vor (lokale und die von `origin`, ohne den Namensraum der Tickets), filtert beim Tippen und übernimmt einen Vorschlag per Klick, Enter oder ↑/↓. Ein Name, den es noch nicht gibt, lässt sich weiterhin eintragen – angelegt wird der Branch erst beim Start des Tickets. Dazu neu: die lesende Route `GET /api/db/boards/:id/branches`; ohne Repo-Pfad oder ohne Repo auf diesem Rechner antwortet sie mit `{ branches: [], reason: 'no-repo' }`, und das Feld verhält sich wie bisher als reine Texteingabe.
- Docs-Seite im Editor-Layout: Dateibaum links, Inhalt in der Mitte, Werkzeuge rechts. Der Explorer ersetzt die Reiter „Specs/Glossar/ADRs“ und die Chip-Liste durch einen Baum (unter „Specs“ je Thema ein Knoten mit „Spec“ und „Plan“), bedienbar mit Maus und Tastatur. Rechts stehen Inhaltsverzeichnis, Kommentare, Tickets zum Dokument und die Ticket-Auswahl in klappbaren Abschnitten. Explorer und Werkzeuge lassen sich in der Breite ziehen; Breiten und offene Knoten merkt sich die Seite je Projekt. Suche, Badge „neu“, Spec-Fortschritt, Kommentieren, Bearbeiten, „Mit Claude ändern“ und Deep-Links `#abschnitt` bleiben unverändert.
- Dispatcher: Planungs-Automatik. Neue Einstellungen `planning.auto` (aus), `planning.wip` (1–5, Vorgabe 1) und
  `planning.maxPerDay` (Vorgabe 10) in `~/.harness/dispatch.yaml`, im Web unter Einstellungen → Dispatcher → Planungen.
  Ist sie an, plant der Tick Tickets mit Planungsbedarf auf eigenen Boards mit Repo-Pfad selbst – nur solange im WIP der
  Tickets ein Platz frei ist, je Ticket einmal, ohne Tickets mit offener Frage oder fremdem Planungs-Claim; Pause und
  Zeitfenster gelten, der Schalter für Tickets nicht. `harness board dispatch status` und der Dispatcher-Status zeigen die
  Automatik mit Tageszähler und letztem Fehler (`planningAuto`).
- Dispatcher-Optimierungen: „Tiefer analysieren“ sagt jetzt, was passiert und was herauskommt. Vor dem Start steht da, was die Analyse tut und was sie ungefähr kostet; während des Laufs läuft eine Statuszeile mit verstrichener Zeit und Zeitlimit mit; danach erscheint immer eine Meldung – auch „Claude sieht keine Verbesserung“ – samt Anzahl, Kosten und unbrauchbaren Antworten. Der Fußbereich nennt zusätzlich die Anzahl der Vorschläge der letzten Analyse. Das Feld `analysis.skipped` kommt neu in `GET /api/dispatch/suggestions`; vorhandene `~/.harness/dispatch-suggestions.json` ohne dieses Feld werden weiter gelesen (gilt als 0).
- Skills und Anleitung auf den neuen Spec-Ablauf umgestellt: Der Skill `harness-tasks` schneidet die Aufgaben aus Spec und Plan unter `docs/superpowers` zu und legt sie direkt als Tickets an (`harness board create` und `harness board plan-json`), statt eine `tasks.md` zu schreiben. `harness-implement` liest das Spec- und Plan-Dokument, auf das ein Ticket verweist; `harness-grill` und `harness-wayfinder` weisen auf die Seite „Docs“ hin. Kein Skill nennt mehr `spec import`, `spec new`, `tasks.md` oder `.specify`. Die Anleitung beschreibt in „Nebenbei“ den Weg vom markierten Text über „Bearbeiten“, „Mit Claude ändern“ und „Zur Ticket-Auswahl“ bis zum angelegten Ticket.
- Spec-Kit-Format entfernt: `harness spec comments [dokument]` listet jetzt die offenen Kommentare der Spec- und Plan-Dokumente aus `docs/superpowers` (dazu Glossar und ADRs); als Bereich lässt sich ein Dokumentpfad oder ein Thema (Dateiname ohne Datum und ohne `-design`, trifft Spec und Plan) angeben. `harness spec resolve` bleibt unverändert. `harness board start` warnt über das Spec-Dokument des Tickets statt über einen Spec-Ordner, `harness board done` hakt nichts mehr in `tasks.md` ab, und ein Merge-Konflikt in einer `tasks.md` wird wie jeder andere Konflikt gemeldet. Die Route `GET /api/projects/:pid/specs` entfällt (`…/spec-progress` bleibt). Die Zählerdatei führt nur noch `session`; vorhandene `spec`-Stände bleiben unangetastet in der Datei stehen.
- Skill `harness-wayfinder`: nennt zusätzlich die rein lesenden Board-Befehle `harness board question list --md` (schon getroffene Entscheidungen gelten für die Karte) und `harness board show <id>` (ein in einer Frage genanntes Ticket nachlesen). Der Skill ändert weiterhin nichts am Board.
- Board: SQLite als zweiter Anbieter der Board-Datenbank für einen einzelnen Rechner ohne DB-Server (Einstellungen →
  Board-Datenbank → „SQLite (lokal)“, oder `provider: sqlite` in `~/.harness/board.yaml`). Datei `~/.harness/board.db`, das
  Schema legt sich selbst an; Node 22.13 oder neuer. SQL Server bleibt Standard, kein Datenumzug.
- **Datenbank vor dem Update:** Skript `packages/db/sql/005_note_structure.sql` in `AiHarness_Test` und `AiHarness` einspielen (Spalten `Title`, `SeenCount`, `SeenIn` an `board.Notes`, Erweiterung `note-structure`). Ohne das Skript startet dieser Stand nicht (`board-schema-outdated`); die Schema-Version bleibt 3, ältere Stände laufen weiter. SQLite-Dateien bekommen die Stufe `006_note_structure` automatisch.
- Board: Notizen haben einen Titel (ein Satz, was auffällt, höchstens 200 Zeichen, eine Zeile) und getrennte Details. **Verhaltensänderung:** `harness board note add <titel>` nimmt den Titel statt des ganzen Texts, Details über `--details`; längere oder mehrzeilige Titel werden abgelehnt. Ältere Notizen behalten ihren Text als Inhalt, angezeigt wird dessen erste Zeile.
- Board: `note add` lehnt einen vermutlich schon notierten Befund ab (`note-similar`, offen oder schon als Ticket übernommen) und nennt die Kandidaten; `--force` legt trotzdem an. Neuer Befehl `harness board note seen <nr> [--from <ticket>]` zählt einen erneut aufgefallenen Befund hoch; Liste, Inbox und Ticket-Beschreibung bei `take` zeigen „N× aufgefallen“. Neue Route `POST /api/db/notes/:id/seen`; `POST /api/db/boards/:id/notes` erwartet `title` (statt `body` allein) und nimmt `force`. Im Notizen-Dialog: Felder „Neue Notiz“ und „Details“, bei Ablehnung „Ist derselbe – hochzählen“ oder „Trotzdem anlegen“.
- Board-Skill und Dispatch-Prompt: erst `note list` lesen, bekannte Befunde mit `note seen` melden; Notizen für Leser ohne Ticket-Kontext schreiben, ohne „unabhängig von …“.
- Nutzungslimits: Der Block in der Seitenleiste kommt ohne Neuladen der Seite zurück – die Oberfläche lädt beim Zurückkehren in den Tab neu und wiederholt einen fehlgeschlagenen Abruf nach rund 8 s; der Server merkt sich Fehlschläge nur rund 5 s statt 60 s.
- **Datenbank vor dem Update:** Skript `packages/db/sql/007_target_branch.sql` in `AiHarness_Test` und `AiHarness` einspielen (setzt 004 voraus, unabhängig von 005 und 006; legt die Spalte `board.Tickets.TargetBranch` an und trägt `target-branch` in `board.SchemaFeatures` ein). Mit DDL-Rechten spielt der Server das Skript selbst ein; ohne das Skript laufen Tickets ohne Zielbranch weiter, nur das Setzen eines Zielbranches meldet `board-schema-outdated`. Eine Datenbank, die das Skript schon unter dem früheren Namen `005_target_branch.sql` bekommen hat, braucht nichts weiter (geprüft wird der Eintrag `target-branch`). SQLite-Boards bekommen die Spalte beim Start automatisch (Schema-Stufe `008_target_branch`). Die Schema-Version bleibt 3: ältere Harness-Stände laufen mit der erweiterten Datenbank weiter.
- Dispatcher/Board: Zielbranch am Ticket. Ein Ticket kann einen Zielbranch tragen: der Ticket-Branch zweigt beim Start von ihm ab, und der Dispatcher mergt nach Erledigt in ihn; ohne Zielbranch bleibt alles beim Alten (Merge in den im Hauptordner ausgecheckten Branch). Fehlt der Branch lokal, legt der Harness ihn beim Start oder beim Merge an – von `origin/<Name>`, falls es diese Referenz gibt, sonst vom `HEAD` des Hauptordners – und pusht nie. Tickets treten einem Worktree nur mit demselben Zielbranch bei. Gemergt wird dort, wo der Zielbranch ausgecheckt ist (im Hauptordner mit den üblichen Prüfungen samt `mergeIgnore`, in einem anderen Worktree mit denselben Prüfungen ohne `mergeIgnore`), sonst in einem Merge-Worktree `<Repo>-worktrees/_merge/<Name>` (`/` wird zu `-`), den der Dispatcher nach dem Merge wieder entfernt. Review-Diff und Commit-Liste vergleichen gegen den Zielbranch. Änderbar nur vor dem Start des Tickets; Namen werden wie bei `git check-ref-format --branch` geprüft, `ticket/…` ist ausgeschlossen. Eingabe im Ticket-Panel (Feld „Zielbranch“), mit `harness board create|set --target-branch <name>` (`set … none` entfernt ihn); `harness board show` gibt ihn aus. Keine neuen Routen, die Ticket-Routen nehmen und liefern das Feld `targetBranch`.
- Dispatcher: „Planen“ im Ticket-Panel und `harness board plan-start <id>` lassen ein Ticket eines eigenen Boards sofort von einer Claude-Session `planner-<ID>` planen (headless, nur lesend, losgelöster Worktree `plan-<ID>`, Plan über `plan-json` oder eine Frage). Neue Routen `GET /api/dispatch/plan/:id`, `POST /api/dispatch/plan/:id`, neue Einstellung `planning.profile`, neuer Lauf-Typ `plan` (zählt nicht zum WIP), Status-Feld `planning`.
- Board: Neue Seite „Meine Aufgaben“ (Seitenleiste direkt nach „Board“, mit Zähler) – alles, was auf den eigenen Boards gerade auf dich wartet: gescheiterte Merges und Einrichtungen, offene eigene Fragen samt Zahl wartender Tickets, Abnahmen, sonst blockierte Tickets, Tickets in Planung und offene Notizen. Standardmäßig nach Dringlichkeit; per Ziehen und Ablegen umsortierbar, die Reihenfolge liegt je Rechner in `~/.harness/inbox-order.json` (neue Einträge oben). Filter nach Board und Art, direkte Aktionen „Abnehmen“, „Erneut mergen“, „Ticket daraus“. Auf dem Dashboard zeigt eine Kachel die Zahl je Art und die drei obersten Einträge (folgt dem Board-Filter bei eigenen Boards). Neuer Befehl `harness board inbox` (`--board`, `--kind`, `--md`, `--json`), neue Routen `GET /api/db/inbox`, `PUT`/`DELETE /api/db/inbox/order`. Kein Schemawechsel.
- Board: Der Reiter „Verlauf“ im Ticket-Panel ist eine Zeitleiste aller Ereignisse – Statuswechsel, Zuständige und Claims (jetzt auch der Planungs-Claim), Läufe des Dispatchers mit Profil und Ausgang, Fragen und Entscheidungen, Commits und Merge des Ticket-Branches, Kommentare und Änderungen an Plan, Handbuch, Abnahme und Schritten – mit Zeitpunkt und Person/Agent, filterbar nach Art, umschaltbar älteste/neueste zuerst. Lauf-Protokolle und Commits sind rechnerlokal; fehlen sie, nennt die Zeitleiste das. Neue Route `GET /api/db/tickets/:id/timeline`.
- Board: Suche über alle Boards und Projekte – „Suchen“ in der Seitenleiste oder Strg+K (Cmd+K). Findet Begriffe (Teilwörter, Groß/Klein egal, alle Wörter im selben Feld) in Titel, Beschreibung, Kommentaren, Plan, Handbuch, Abnahme, Fragen samt Entscheidung und Kommentaren sowie Notizen; Treffer mit Projekt, Board, Status und hervorgehobener Fundstelle, ein Klick öffnet Ticket, Frage oder Notizen. Ohne Volltextindex und ohne Schemaänderung (Schema bleibt 3). Neue Route `GET /api/db/search`.
- **Datenbank vor dem Update:** Skript `packages/db/sql/004_dispatch_runs.sql` in `AiHarness_Test` und `AiHarness` einspielen (legt `board.DispatchRuns` und `board.SchemaFeatures` an). Ohne das Skript startet dieser Stand nicht (`board-schema-outdated`). Die Schema-Version bleibt 3: ältere Harness-Stände laufen mit der erweiterten Datenbank weiter.
- Dashboard: neue Seite „Dashboard“ in der Seitenleiste (nach „Board“) mit Durchsatz, Erfolgsquote je Profil und Modell, Durchlaufzeiten (Median, P85), Versuchen bis Erledigt, Abbruchgründen, Eskalationen sowie Kosten und Tokens (Listenpreis) und den langsamsten und teuersten Tickets; Filter Board, Zeitraum und Labels. Neue Route `GET /api/db/stats/dispatch`.
- Dispatcher: erfasst jeden ausgewerteten Lauf in `board.DispatchRuns` (Profil, Eskalation, Versuch, Modell laut Ergebnis-Ereignis, Kosten, Tokens, Laufzeit, Ergebnis, Abbruchgrund). Läufe werden erst lokal vorgemerkt (`~/.harness/dispatch/runs/stats`) und dann geschrieben, damit bei einem Verbindungsabbruch nichts verloren geht.
- CLI: `harness board dispatch-runs backfill [--project KEY] [--dry-run]` trägt Läufe aus den `[dispatch]`-Kommentaren nach (ohne Kosten und tatsächliches Modell); beliebig oft ausführbar.
- Dispatcher: Freigabe vor dem Merge. Ein vom Agenten erledigtes Ticket wartet als Blockiert mit Grund `Human Interaction: Review vor dem Merge`; das Ticket-Panel zeigt den Diff gegen den Zielbranch und die Testläufe des Agenten, dazu „Übernehmen“ (Merge wie bisher) und „Zurück an den Agenten“ mit Rückmeldung (zurück nach Geplant, gleicher Worktree, kein Versuch). Neue Routen `GET /api/dispatch/review/:id`, `GET /api/dispatch/review/:id/file`, `POST /api/dispatch/return/:id`.
- **Verhaltensänderung:** Neue Einstellung `reviewBeforeMerge` in `dispatch.yaml`, Standard `true`. Nach dem Update mergt der Dispatcher erledigte Tickets erst nach Freigabe. Das bisherige Verhalten (sofort mergen) mit `reviewBeforeMerge: false` oder im Dispatcher-Panel über „Vor dem Merge auf Freigabe warten“.
- Anpassungen: Workflow, Skills und Output-Styles in `custom/` ersetzen oder ergänzen; Skill `/ai-harness:harness-customize`, `npm run customize`, `npm run setup -- --no-pull`, Panel „Anpassungen“ in den Einstellungen.
