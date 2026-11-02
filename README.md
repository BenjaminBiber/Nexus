# Nexus

Lokale Werkbank für mehrere Claude-Code-Sessions gleichzeitig (früher „AI Harness“): Kanban-Board mit Tickets, Fragen und Notizen, Übersicht der Sessions, konfliktfreie Parallelarbeit mit Branches und Worktrees, ein Dispatcher, der geplante Tickets selbst umsetzen lässt, und Favoriten zum Fortsetzen von Sessions. Tickets, Fragen und Boards liegen in einer Board-Datenbank: gemeinsam im Team auf SQL Server oder als lokale SQLite-Datei für einen einzelnen Rechner.

Bestandteile:

- CLI `nexus` (Alias `harness`, alte Befehle laufen weiter)
- lokaler Server mit Weboberfläche (http://127.0.0.1:4710)
- Desktop-App (Tauri), die Server und Oberfläche in einem Fenster startet
- Claude-Code-Plugin `nexus` (Hooks, Skills, Output-Style, Doku- und Board-Regel)

Inhalt: [Einrichten](#einrichten-einmal-pro-rechner) · [Aktualisieren](#aktualisieren) · [Umstieg vom AI Harness](#umstieg-vom-ai-harness) · [Verwenden](#verwenden) · [Board-Datenbank](#board-datenbank) · [Dispatcher](#dispatcher-tickets-automatisch-umsetzen-lassen) · [Anpassen](#anpassen) · [Daten und Einstellungen](#daten-und-einstellungen) · [Fehlersuche](#fehlersuche) · [Entwicklung](#entwicklung-an-nexus)

## Einrichten (einmal pro Rechner)

Voraussetzungen:

- Rust über rustup; die Version legt `rust-toolchain.toml` fest, rustup lädt sie beim ersten `cargo`-Aufruf
- Node.js 22.13 oder neuer (nur für den Bau der Oberfläche und des Plugins)
- Git 2.5 oder neuer; unter Windows Git for Windows (der Dispatcher braucht die Git Bash)
- Claude Code, angemeldet

```sh
git clone https://github.com/BenjaminBiber/Nexus.git nexus
cd nexus
npm install
npm run build          # Oberfläche nach packages/web/dist
npm run build:plugin   # Binary und Plugin nach packages/plugin/dist/nexus
packages/plugin/dist/nexus/bin/nexus install
```

`nexus install`

- kopiert das Programm nach `~/.nexus` und trägt `~/.nexus/bin` in den PATH des Benutzers ein (dort liegen `nexus` und der Alias `harness`),
- installiert das Plugin `nexus@nexus-local` über einen lokalen Marktplatz in Claude Code und schaltet es auf Benutzerebene **aus**; eingeschaltet wird es nur in Projekten mit `nexus init`,
- legt die Doku- und Board-Regel unter `~/.nexus/rules` ab.

Danach ein **neues** Terminal öffnen.

## Aktualisieren

```sh
git pull
npm install            # nur bei geänderter package.json
npm run build
npm run build:plugin
nexus stop             # laufenden Server beenden, damit die neue Version startet
packages/plugin/dist/nexus/bin/nexus install
```

Danach in laufenden Sessions `/reload-plugins` eingeben. Bringt ein Update neue Regeln mit, je Projekt einmal `nexus init`. Steht im `CHANGELOG.md` unter „Datenbank vor dem Update“ ein SQL-Skript, muss es vorher in die SQL-Server-Datenbanken (siehe [Board-Datenbank](#board-datenbank)).

## Umstieg vom AI Harness

Wer den Harness schon unter dem alten Namen nutzt:

```sh
nexus migrate --dry-run   # zeigt, was aus ~/.harness kopiert würde
nexus migrate             # kopiert nach ~/.nexus; ~/.harness bleibt unverändert liegen
```

- Solange es kein `~/.nexus` gibt, liest Nexus weiter `~/.harness`. Die alten Umgebungsvariablen (`HARNESS_HOME`, `HARNESS_PORT`, …) gelten weiter, die neuen (`NEXUS_…`) haben Vorrang.
- `nexus install` schaltet das alte Plugin `ai-harness@ai-harness-local` aus. Skills heißen jetzt `/nexus:…` statt `/ai-harness:…`.
- `nexus init` im Projekt ersetzt die alten Regeln `.claude/rules/ai-harness-*.md` durch `nexus-docs.md` und `nexus-board.md` (von Hand geänderte alte Regeln bleiben liegen) und trägt das alte Plugin aus `.claude/settings.local.json` aus.
- Das alte Plugin ganz entfernen: `claude plugin uninstall ai-harness@ai-harness-local`.

## Verwenden

Im Projektordner (Git-Repository):

```sh
nexus init     # einmal: richtet das Projekt ein, schaltet das Plugin ein, bindet Doku- und Board-Regel ein
nexus board    # startet bei Bedarf den Server und öffnet das Board im Browser
```

Danach `/reload-plugins` eingeben oder Claude Code neu starten.

Jedes Repo gehört zu einem Board: einmal `nexus board boards repo <id> <pfad>` (Pfad = Hauptordner des Repos; `nexus board boards list` zeigt die Boards) oder „Mein Repo-Pfad“ in der Verwaltung. Der Repo-Pfad ist je Person lokal (`~/.nexus/board-repos.yaml`), nicht in der gemeinsamen Datenbank. Im eingerichteten Repo arbeiten `nexus board list`, `create` und `note` ohne `--board` auf diesem Board, sonst auf dem aktiven Board aus `~/.nexus/board.yaml`. `nexus board start` und `done` nehmen nur Tickets des Boards an, das dem Repo zugeordnet ist.

Die Oberfläche hat in der Seitenleiste u. a. **Board**, **Meine Aufgaben** (alles, was gerade auf dich wartet: gescheiterte Merges, eigene Fragen, Abnahmen, Tickets in Planung, Notizen), **Dashboard** (Durchsatz, Erfolgsquote, Durchlaufzeiten und Kosten des Dispatchers), **Fragen**, **Docs**, **Sessions** und **Anleitung**; **Suchen** bzw. Strg+K (Cmd+K) durchsucht alle Boards und Projekte.

**Ablauf:** Der Workflow von der Anforderung bis zum fertigen Ticket steht im Board im Tab **Anleitung**, mit allen Befehlen zum Kopieren. Zusätzlich gibt ihn ein Hook zu Beginn jeder Session an Claude: Claude kennt damit den Ablauf und den Stand der Tickets, schlägt nach jedem Schritt den nächsten vor und führt dich so durch den Workflow. Beides folgt deinem angepassten Workflow, falls es einen gibt (siehe [Anpassen](#anpassen)).

| Schritt | Befehl |
|---|---|
| Anforderungen klären | `/superpowers:brainstorming` (einfach), `/nexus:harness-grill` (eine Session, mittel) oder `/nexus:harness-wayfinder` (komplex, mehrere Sessions) |
| Spec | `/superpowers:brainstorming` schreibt sie nach `docs/superpowers/specs/`; nach Grill oder Wayfinder legt Claude die Zusammenfassung dort als Spec ab |
| Plan schreiben | `/superpowers:writing-plans` (Tasks mit Dateien, Code, Tests und Befehlen unter `docs/superpowers/plans/`) |
| Tickets anlegen | in der Session „Lege für die Tasks des Plans Tickets an“ (je Task ein Ticket mit Abnahme, Schritten und Abhängigkeit zur vorigen Task; Plan, Handbuch und Diagramm per `nexus board plan-json ARC-0042 --file payload.json`, setzt Geplant) oder im Board unter **Docs** mit „Tickets erzeugen“ bzw. Text markieren → Ticket-Auswahl |
| Für den Dispatcher vorbereiten | optional: „Bereite ARC-0042 für den Dispatcher vor“ (Labels `auto` und `profil:<klein\|standard\|schwer>`) |
| Nächstes Ticket wählen | `nexus board list --status geplant --md` (alle: `nexus board list`) |
| Ticket übernehmen | `nexus board claim ARC-0042 --agent claude` |
| Ticket starten | `nexus board start ARC-0042 --by claude` (Branch und Worktree; optional mit Zielbranch, siehe unten) |
| Umsetzen | `/nexus:harness-implement` (ganzer Plan in einer Session: `/superpowers:subagent-driven-development`, Tickets dabei nachführen; automatisch: Dispatcher); Schritte abhaken mit `nexus board step ARC-0042 2`, anlegen/ändern mit `nexus board step add\|edit\|rm\|move` (oder im Ticket-Panel) |
| Rückfragen | `nexus board question add "…" --ticket ARC-0042`, Recherche mit `/nexus:board-question <ID>`, „Recherche starten“ auf der Seite der Frage oder `nexus board question research-start <ID>`; entscheiden nur Menschen (Board → Fragen) |
| Spec- und Plan-Dokumente kommentieren | im Board unter **Docs** Text markieren und kommentieren, bearbeiten oder „Mit Claude ändern“; der Agent liest Kommentare mit `nexus spec comments [dokument]` und erledigt sie mit `nexus spec resolve` |
| Notizen | Einfälle nebenbei: `nexus board note add "<titel>" --details "…"` oder Board → Notizen; daraus später ein Ticket |
| Kommentare | im Board schreiben; sie erreichen die Session bei der nächsten Eingabe |
| Abschluss | `nexus board done ARC-0042 --acceptance "Woran man sieht, dass es fertig ist"`, dann „Mergen“ im Ticket-Panel oder `nexus board merge ARC-0042` |

**Zielbranch:** Ein Ticket kann einen Zielbranch tragen (Ticket-Panel, Feld „Zielbranch“, oder `nexus board create|set --target-branch <name>`). Der Ticket-Branch zweigt dann von ihm ab, und gemergt wird in ihn; ohne Zielbranch in den Branch, der im Hauptordner ausgecheckt ist. Fehlt der Branch lokal, legt Nexus ihn an (von `origin/<name>`, sonst vom `HEAD`) und pusht nie.

**Schnelle Aufgaben ohne Workflow:** Für kleine Aufgaben oder Nebenaufgaben kannst du ohne Spec, Ticket und Worktree arbeiten. Claude fragt dann zuerst „Ohne Workflow direkt umsetzen?“ und arbeitet erst nach deinem ausdrücklichen Ja so; ohne Bestätigung bleibt es beim Workflow. Du kannst es auch direkt sagen, z. B. „ohne Workflow“.

Hinweise:

- Skills (`/nexus:…`) und Hooks gibt es nur in Projekten mit `nexus init`; `nexus` im Terminal überall.
- Claude Code im Projektstamm starten (bei Worktrees im Worktree-Ordner). `nexus board start` richtet neue Worktrees selbst ein (Plugin-Einstellung, Doku- und Board-Regel); ältere Worktrees bekommen `.claude/settings.local.json` durch Kopieren aus dem Hauptprojekt.
- `nexus init` schreibt `.claude/settings.local.json` und die Regeln `.claude/rules/nexus-docs.md` (lädt die Doku-Regeln beim Zugriff auf `docs/`) und `.claude/rules/nexus-board.md` (immer geladen: Tickets, Board, Fragen, Notizen und IDs wie `ARC-0042` meinen immer das Nexus-Board, nie ein altes Kanban oder GitHub/GitLab-Issues); alles ist lokal und wird nicht committet. Die persönliche `CLAUDE.md` bleibt unberührt.
- `nexus init` ist wiederholbar und ergänzt nur Fehlendes; `nexus install` aktualisiert nur den zentralen Inhalt unter `~/.nexus/rules`, legt aber keine Regeln in Projekten an.
- Kontrolle: `claude plugin list` in einem Projekt ohne Nexus muss `nexus@nexus-local` als ausgeschaltet zeigen; sonst einmal `claude plugin disable nexus@nexus-local --scope user` (macht auch `nexus install`).
- Doku-Regeln ändern nur in `packages/plugin/manifest/rules/docs.md`, die Board-Regel nur in `packages/plugin/manifest/rules/board.md`, danach `npm run build:plugin` und `nexus install`. Projektspezifisches gehört in die `CLAUDE.md` des Projekts.

**Sicherung:** Bei SQL Server sichert der DBA die Board-Datenbank. Bei SQLite: Server mit `nexus stop` beenden, keine Session und kein Dispatcher-Lauf aktiv, dann `board.db` zusammen mit `board.db-wal` und `board.db-shm` (falls vorhanden) kopieren. Nexus legt keine eigenen Sicherungen oder Exporte an. Ordner des alten Datei-Boards unter `<git-common-dir>/harness` (z. B. `tickets/`, `questions/`) bleiben unangetastet liegen und werden **nicht** importiert.

### Board-Datenbank

Zwei Varianten: **SQL Server** für das Team (Standard) oder **SQLite** für einen einzelnen Rechner ohne DB-Server.

#### Variante A: SQL Server (Team)

1. Einmalig (Administrator): `crates/nexus-db/sql/000_create_databases.sql`, danach die Skripte `001_…` bis zum höchsten `00N_…` der Reihe nach in `AiHarness` und `AiHarness_Test` ausführen (Details in `crates/nexus-db/sql/README.md`).
2. Pro Rechner: `nexus board` öffnen → **Einstellungen → Board-Datenbank**: Server, Benutzer, Passwort, eigenen Namen eintragen, „Verbindung testen“, „Speichern“. Die Benutzer-ID (GUID) wird vorgeschlagen und kann im Team abgestimmt werden.

Schema-Änderungen kommen als neues Skript `crates/nexus-db/sql/00N_….sql` und stehen im `CHANGELOG.md`; `nexus` meldet, welches fehlt (`board-schema-outdated`). Mit DDL-Rechten spielt der Server manche Skripte selbst ein.

#### Variante B: SQLite (Einzelplatz)

1. `nexus board` öffnen → **Einstellungen → Board-Datenbank** → „SQLite (lokal)“. Dateipfad übernehmen (Vorschlag `~/.nexus/board.db`) oder ändern, eigenen Namen eintragen, „Verbindung testen“, „Speichern“.
2. Ohne Oberfläche gleichwertig: `~/.nexus/board.yaml` anlegen mit
   ```yaml
   provider: sqlite
   file: board.db          # optional; relativ zu ~/.nexus, Standard board.db
   userId: <eigene GUID>
   userName: <Anzeigename>
   ```
3. Die Datei und ihr Schema legt Nexus beim ersten Zugriff selbst an und hält es aktuell; SQL-Skripte sind nicht nötig.

Grenzen: nur dieser Rechner (kein Teambetrieb), nicht auf Netzlaufwerk oder in synchronisierten Ordnern (OneDrive, Dropbox), kein Umzug von Daten zwischen SQL Server und SQLite (ein Wechsel startet mit leerem Board).

Erstes Board (beide Varianten): `nexus board project create ARC "Arches"`, dann `nexus board boards create ARC "Mein Board" --use`.

### Dispatcher (Tickets automatisch umsetzen lassen)

Der Server prüft jede Minute die Spalte **Geplant** der eigenen Boards (Auswahl in den Einstellungen, sonst das aktive Board). Bereit ist ein Ticket mit Label `auto` (abschaltbar), Plan, Handbuch und Abnahme, ohne offene Voraussetzung oder Frage, auf einem Board mit eigenem Repo-Pfad. Der Dispatcher übernimmt es als `junior-<ID>`, legt Branch und Worktree an, führt die eingestellten Einrichtungsbefehle (z. B. `npm ci`) aus und startet `claude -p` mit deiner Claude-Anmeldung (ohne `ANTHROPIC_API_KEY`). Danach:

- **Erledigt:** Standardmäßig wartet das Ticket als Blockiert mit Grund „Human Interaction: Review vor dem Merge“. Das Ticket-Panel zeigt den Diff gegen den Zielbranch und die Testläufe des Agenten; „Übernehmen“ (oder `nexus board accept <ID>`) mergt, „Zurück an den Agenten“ schickt es mit Rückmeldung nach Geplant (gleicher Worktree, kein Versuch). Mit `reviewBeforeMerge: false` mergt der Dispatcher sofort.
- **Merge:** in den Zielbranch des Tickets bzw. den Branch, der im Hauptordner ausgecheckt ist (Merge-Commit oder Squash, einstellbar), danach Worktree und Branch aufräumen. Teilen mehrere Tickets einen Worktree, wird erst gemergt, wenn alle fertig sind. Lokale Änderungen im Hauptordner verhindern den Merge (Ausnahmen per Muster, z. B. `*.yaml`). Bei Konflikten löst eine Merge-Session im Worktree sie; sie startet nur, wenn der Dispatcher eingeschaltet, im Zeitfenster und nicht pausiert ist.
- **Merge gescheitert:** Das Ticket steht auf Blockiert mit Grund „Merge: …“. Nach dem Beheben im Panel „Erneut mergen“ oder `nexus board merge <ID>`.
- **Von Hand erledigt:** Ein von Hand auf Erledigt gesetztes Ticket mergt man mit „Mergen“ im Panel oder `nexus board merge <ID>`.
- **Frage:** zurück nach Geplant; nach deiner Entscheidung geht es im selben Worktree weiter.
- **Abbruch, Zeitlimit oder gescheiterter Start:** zählt als Versuch, das Ticket geht zurück nach Geplant, der nächste Versuch nutzt ein größeres Profil; nach dem letzten Versuch Blockiert.
- **Nutzungslimit:** Pause (Standard 60 min), zählt nicht als Versuch – ab der dritten Limit-Pause in Folge für dasselbe Ticket aber schon. Wird Claude Code nicht gefunden, pausiert der Dispatcher ebenso, ohne Versuch.

Der Worker nutzt unter Windows die Git Bash (wird automatisch gefunden, sonst `claude.gitBashPath` in den Dispatcher-Einstellungen setzen).

Einschalten und einstellen unter **Einstellungen → Dispatcher** (Datei `~/.nexus/dispatch.yaml`): WIP, Zeitfenster, Profile und Regeln, Zurückmergen, Worktrees (einer je Ticket oder gebündelt), Einrichtung neuer Worktrees, eigene Prompts je Laufart, Recherchen und Planungen. Zustand und Protokolle über den Knopf „Dispatcher: …“ in der Board-Kopfzeile, den Reiter „Agent“ im Ticket-Panel oder im Terminal:

```bash
nexus board dispatch status          # Zustand, Worker, Merges, warum Tickets nicht bereit sind
nexus board dispatch run             # sofort prüfen
nexus board dispatch pause --minutes 60
nexus board dispatch resume
nexus board dispatch enable          # bzw. disable
nexus board merge ARC-0009           # Merge erneut anstoßen
nexus board accept ARC-0009          # Review abnehmen: Erledigt, danach Merge
```

**Recherchen zu Fragen:** Auf der Seite einer offenen Frage startet „Recherche starten“ (oder `nexus board question research-start ARC-F0007`) sofort eine Claude-Session `research-<ID>`, die die Frage nach dem Skill `nexus:board-question` recherchiert und das Ergebnis an die Frage schreibt – unabhängig davon, ob der Dispatcher für Tickets eingeschaltet ist; nur eine Pause (z. B. Nutzungslimit) verhindert den Start. Sie läuft im Repo des eigenen Boards, zu dem die Frage gehört (Board eines verknüpften Tickets, sonst das aktive Board des Projekts, sonst das eigene Board des Projekts mit Repo-Pfad und kleinster ID), in einem eigenen, losgelösten Worktree `<repo>-worktrees/research-<ID>`, der danach wieder entfernt wird (verwaiste räumt jeder Tick auf). Die Session läuft unabhängig von `claude.permissionMode` und `claude.extraArgs` mit `--permission-mode default`, darf nur lesen (Read, Glob, Grep, `git log/show/diff`) und von der Board-CLI `question show`, `question research`, `show` und `list` nutzen; Edit/Write (außer der Datei `research.json` im Recherche-Worktree), Commits, Branchwechsel, `npm`/`node` und Entscheidungen sind per `--disallowedTools` verboten (gilt auch gegen Erlaubnisse in `~/.claude/settings.json`). Das Ergebnis schreibt der Agent in `research.json` und speichert es mit `nexus board question research <ID> --from research.json`. Als Ergebnis zählt nur eine Recherche des Agents `research-<ID>` ab dem Start; sonst steht `[dispatch] Recherche ohne Ergebnis: …` an der Frage, ein Neuversuch erfolgt nicht automatisch.

Unter **Einstellungen → Dispatcher → Recherchen** lässt sich einschalten, dass neue Fragen zu deinen Boards automatisch recherchiert werden: nur Fragen, die nach dem Einschalten entstehen (Aus- und Einschalten beginnt neu), die du (oder ein Agent unter deinem Benutzer) gestellt hast oder die mit Tickets deiner Boards verknüpft sind, je Frage einmal; Grenzen „Gleichzeitige Recherchen“ (1–5) und „Automatische Recherchen je Tag“ (Vorgabe 10); Profil wählbar (ohne Wahl das Profil `research`, falls vorhanden, sonst das Standardprofil). Fremde Fragen startest du von Hand. Umgebungsfehler (z. B. Git Bash fehlt, Repo-Pfad kein Git-Repo) setzen die Automatik 30 min aus und stehen einmal im Protokoll und in der Übersicht. In `dispatch.yaml`: `research: { auto: true, wip: 1, maxPerDay: 10, profile: klein }`.

**Planen am Ticket:** Im Ticket-Panel startet „Planen“ (oder `nexus board plan-start ARC-0007`) sofort eine Claude-Session `planner-<ID>`, die das Ticket nach dem Skill `nexus:board` plant – wie eine Recherche unabhängig davon, ob der Dispatcher eingeschaltet ist, nur nicht während einer Pause. Das geht für Tickets eigener Boards mit Repo-Pfad in Backlog, In Planung, Geplant oder Blockiert ohne offene Frage. Die Session läuft in einem losgelösten Worktree `<repo>-worktrees/plan-<ID>` (danach entfernt), mit `--permission-mode default`, darf nur lesen, `plan.json` schreiben und von der Board-CLI `show`, `list`, `plan-json`, `comment`, `note list` sowie `question show/list/add` nutzen; Status, Labels und Zuständigkeit ändert sie nicht von Hand. Sie speichert den Plan mit `nexus board plan-json` (Ticket danach Geplant) oder legt eine Frage an, wenn eine Entscheidung fehlt. Ohne beides steht `[dispatch] Planung ohne Ergebnis: …` am Ticket. Planungen zählen nicht zum WIP der Tickets.

**Planungs-Automatik:** Unter **Einstellungen → Dispatcher → Planungen** lässt sich einschalten, dass Tickets mit Planungsbedarf auf eigenen Boards selbst geplant werden – nur solange im WIP der Tickets ein Platz frei ist, je Ticket einmal, ohne Tickets mit offener Frage. Mit `acceptance: true` füllt die Planung auch die Abnahme. Profil ohne Wahl `planning`, falls vorhanden, sonst das Standardprofil. In `dispatch.yaml`: `planning: { auto: true, wip: 1, acceptance: true, profile: schwer }`.

Lauf-, Pause- und Merge-Zustand liegen unter `~/.nexus/dispatch/`, Protokolle unter `~/.nexus/dispatch-logs/`; ausgewertete Läufe landen zusätzlich in der Board-Datenbank (Dashboard). Die Worker laufen losgelöst weiter, wenn der Server neu startet. Erlaubte Werkzeuge stehen in der Einstellung „Erlaubte Tools“; braucht ein Projekt z. B. `dotnet test`, gehört `Bash(dotnet test:*)` dazu. Arbeitet derselbe Nutzer auf mehreren Rechnern, den Dispatcher nur auf einem davon einschalten – sonst übernehmen beide Rechner Tickets derselben Boards.

## Anpassen

Workflow, Skills und Output-Styles lassen sich pro Dev ersetzen oder ergänzen. Die Anpassungen liegen in `custom/` in diesem Ordner; Git ignoriert ihn, Updates laufen ohne Merge.

In einer Claude-Session in einem eingerichteten Projekt:

```text
/nexus:harness-customize
```

Der Skill kopiert den Standard als Ausgangspunkt, legt Neues an, prüft und setzt auf Wunsch zurück. Anpassungen wirken erst nach dem Anwenden in diesem Ordner (Pflicht), danach `/reload-plugins`:

```sh
nexus customize check          # prüfen, ohne etwas zu ändern
npm run build:plugin           # anwenden: baut das Plugin mit den Anpassungen
packages/plugin/dist/nexus/bin/nexus install
```

Ungültige Anpassungen fallen mit Warnung auf den Standard zurück; die Ausgabe von `build:plugin` und das Panel „Anpassungen“ in den Einstellungen des Boards zeigen, was gilt. Was stabil bleibt (Vertrag), steht in `CHANGELOG.md`.

## Daten und Einstellungen

| Was | Ort |
|---|---|
| Token, Zähler, Worktree-Verzeichnis | `<git-common-dir>/harness/` im Projekt (nicht versioniert) |
| Projekte, Sessions, Favoriten, Einstellungen, Server-Protokoll | `~/.nexus/` |
| Repo-Pfade der Boards (je Person) | `~/.nexus/board-repos.yaml` |
| Tickets, Fragen, Projekte, Boards, Notizen, Dispatcher-Läufe (SQL Server) | Datenbank `AiHarness` (Schema `board`); Sicherung durch den DBA |
| dasselbe (SQLite) | `~/.nexus/board.db` (oder `file` aus `board.yaml`) samt `-wal`/`-shm`; Sicherung siehe oben |
| Anbieter und Verbindung der Board-Datenbank, eigene Benutzer-ID, aktives Board | `~/.nexus/board.yaml` |
| Einstellung des Dispatchers | `~/.nexus/dispatch.yaml` |
| Läufe, Pause, Reviews und Merges des Dispatchers, Protokolle | `~/.nexus/dispatch/`, `~/.nexus/dispatch-logs/` |
| Reihenfolge in „Meine Aufgaben“ | `~/.nexus/inbox-order.json` |
| Programm und Aufrufe | `~/.nexus/app/`, `~/.nexus/bin/` |
| Doku- und Board-Regel (Importziel der Projekte) | `~/.nexus/rules/`, Quelle `packages/plugin/manifest/rules/` |
| Angewendete Anpassungen | `~/.nexus/customizations/`, Quelle `custom/` |
| Freigabe des Imports je Projekt | `~/.claude.json` (oder `CLAUDE_CONFIG_DIR/.claude.json`), Eintrag unter `projects` |

Ohne `~/.nexus` gilt während der Übergangszeit noch `~/.harness` (siehe [Umstieg](#umstieg-vom-ai-harness)).

Umgebungsvariablen (die alten Namen mit `HARNESS_` statt `NEXUS_` gelten weiter, die neuen haben Vorrang):

| Variable | Wirkung |
|---|---|
| `NEXUS_HOME` | Ordner statt `~/.nexus` (z. B. für eine Test-Umgebung) |
| `NEXUS_PORT` | Port des Servers statt 4710 |
| `NEXUS_TICKET` | Ticket der Session fest vorgeben |
| `NEXUS_WEB_DIR` | Ordner der gebauten Oberfläche für `nexus serve` |
| `HARNESS_TEST_DB` | nur für Tests: Verbindungs-JSON zur SQL-Server-Test-DB |

## Fehlersuche

| Symptom | Abhilfe |
|---|---|
| `nexus` im Terminal „nicht gefunden“ | Neues Terminal öffnen (der PATH gilt erst dort); sonst `nexus install` aus dem gebauten Plugin ausführen. Windows-Installer bis 0.1.2 tragen keinen PATH ein: neu installieren oder `%LOCALAPPDATA%\Nexus\nexus.exe` direkt aufrufen |
| Hooks oder Skills fehlen | Im Projekt `nexus init`, dann `/reload-plugins`; prüfen mit `claude plugin list` (`nexus@nexus-local`) |
| Skills heißen noch `/ai-harness:…` oder laufen doppelt | Altes Plugin aktiv: `nexus install` erneut, im Projekt `nexus init`; sonst `claude plugin disable ai-harness@ai-harness-local --scope user` |
| `git worktree add` scheitert mit „Filename too long“ (Windows) | Nexus schaltet dafür `core.longpaths` in der lokalen Konfiguration des Repos ein (Hinweis beim Anlegen); notfalls von Hand `git config core.longpaths true` im Repo |
| Hooks fehlen in einem Worktree | `nexus init` im Hauptprojekt oder `.claude/settings.local.json` aus dem Hauptprojekt kopieren; neue Worktrees legt `nexus board start` richtig an |
| Doku-Regeln greifen nicht | Werden erst beim Lesen oder Schreiben einer Datei unter `docs/` geladen. `nexus init` ausführen und neue Session starten; in `~/.claude.json` müssen für das Projekt `hasClaudeMdExternalIncludesApproved` und `hasClaudeMdExternalIncludesWarningShown` auf `true` stehen |
| Anpassung wirkt nicht | `nexus customize check` ausführen, dann `npm run build:plugin`, `nexus install` und `/reload-plugins` |
| „Port 4710 ist belegt“ | Es läuft schon ein Server; beenden (`nexus stop`) oder `NEXUS_PORT` setzen |
| Nach dem Update läuft noch die alte Version | `nexus stop`, dann `nexus board` |
| Projektliste im Board leer | `nexus board` im Projektordner ausführen |
| „Kein Board ist dem Repo … zugeordnet“ oder „… gehört aber zu Board …“ | `nexus board boards list`, dann `nexus board boards repo <id> <pfad>`; oder das Ticket im Repo seines Boards bearbeiten |
| `board-schema-outdated` (SQL Server) | Fehlendes Skript aus `crates/nexus-db/sql/` einspielen (Meldung nennt es) |
| „Board-Datei ist nicht erreichbar“ / „lässt sich nicht öffnen“ (SQLite) | Pfad in Einstellungen → Board-Datenbank prüfen; Ordner beschreibbar? Datei nicht auf Netzlaufwerk/OneDrive; anderes Programm hält die Datei offen → schließen |

Für ein Projekt ausschalten: `claude plugin disable nexus@nexus-local --scope local`. Deinstallieren: `nexus stop`, `claude plugin uninstall nexus@nexus-local`, dann `~/.nexus/app`, `~/.nexus/bin` und `~/.nexus/rules` löschen und `~/.nexus/bin` aus dem PATH nehmen.

## Entwicklung an Nexus

### Lokal testen

Die Tests legen eigene Git-Repos an und vergleichen Golden-Dateien mit `\n`; deshalb einmal:

```sh
git config --global core.autocrlf false
git config --global core.longpaths true      # nur Windows
```

**1. Einmal bauen** (in dieser Reihenfolge, wie in der CI):

```sh
npm install
npm run build                                   # Oberfläche nach packages/web/dist (brauchen Plugin-Build und Server)
cargo build -p nexus-cli --bin nexus            # target/debug/nexus (für Vertragstests und Smoke-Test)
cargo build --release -p nexus-cli --bin nexus  # target/release/nexus (für Plugin-Build und Plugin-Tests)
```

**2. Von Hand ausprobieren, ohne die eigene Installation zu berühren.** Mit `NEXUS_HOME` arbeiten Server und CLI in einem eigenen Ordner (statt `~/.nexus`) und damit auf einem eigenen Board; mit einer SQLite-Datei braucht es keinen SQL Server:

```sh
export NEXUS_HOME="$HOME/nexus-dev"           # PowerShell: $env:NEXUS_HOME = "$HOME\nexus-dev"
mkdir -p "$NEXUS_HOME"
cat > "$NEXUS_HOME/board.yaml" <<'EOF'
provider: sqlite
userId: 11111111-1111-1111-1111-111111111111
userName: Dev
EOF
nexus stop                                    # laufenden installierten Server beenden (Port 4710)
npm run dev                                   # Rust-Server (4710) und Vite mit Hot Reload (http://localhost:5173)
```

Vite leitet `/api` fest an Port 4710 weiter; ein installierter Server muss für `npm run dev` deshalb aus sein. Der Server liest dieselbe Umgebung, also auch `NEXUS_HOME`.

Die CLI aus dem Quellstand (gleiche Umgebung, zweites Terminal):

```sh
cargo run -q -p nexus-cli -- board project create ARC "Arches"
cargo run -q -p nexus-cli -- board boards create ARC "Test-Board" --use
cargo run -q -p nexus-cli -- board create "Erstes Ticket"
cargo run -q -p nexus-cli -- board list
```

Statt `cargo run …` geht nach dem Bauen auch direkt `target/debug/nexus …`. Nur den Server ohne Vite (gebaute Oberfläche, eigener Port): `cargo run -q -p nexus-cli -- serve --port 4800 --web-dir packages/web/dist`, dann http://127.0.0.1:4800.

Zum Aufräumen genügt es, den Ordner aus `NEXUS_HOME` zu löschen und die Variable zu entfernen.

**3. Plugin und Hooks in Claude Code testen**, ohne `nexus install`:

```sh
npm run build:plugin                          # baut nach packages/plugin/dist/nexus
claude --plugin-dir packages/plugin/dist/nexus
```

Am besten in einem Wegwerf-Repo (`git init`) mit gesetztem `NEXUS_HOME`. Nach Änderungen an Skills, Hooks oder Regeln erneut `npm run build:plugin` und in der Session `/reload-plugins`.

**4. Desktop-App (Tauri):**

```sh
npm run desktop -- dev                        # baut Oberfläche und Server-Sidecar, startet die App
npm run desktop -- build --debug --no-bundle  # Debug-Build für den Smoke-Test
npm run smoke -w apps/desktop                 # Smoke-Test: Start, Server-Erkennung, Beenden (eigenes HOME, freier Port)
```

Unter Linux braucht Tauri Systempakete (`libwebkit2gtk-4.1-dev` u. a., siehe `.github/workflows/ci.yml`). Paketierung und Signatur: [docs/desktop/packaging.md](docs/desktop/packaging.md).

### Automatische Tests

| Befehl | Prüft | Voraussetzung |
|---|---|---|
| `npm run lint` | Typprüfung der Oberfläche (= `npm run typecheck`) | – |
| `npm test` | Vitest: Oberfläche und Plugin (Plugin-Build in einem eigenen Ordner je Lauf) | `npm run build`, Release-Binary |
| `npm run contract` | Vertrag von API, SSE, CLI und Hooks gegen das Rust-Binary (Golden-Dateien unter `contract/`) | Debug-Binary |
| `cargo fmt --all --check` | Rust-Format | – |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rust-Lints | – |
| `cargo test --workspace --no-fail-fast` | Rust: Fachlogik, Datenbank, Server, CLI | – |
| `cargo test -p nexus-core export_bindings` | erzeugt die TypeScript-Typen unter `packages/web/src/api/generated` neu; danach darf `git status` dort nichts zeigen | – |

Einzelne Tests gezielt:

```sh
npx vitest run packages/web/tests/ResearchBlock.test.tsx   # eine Testdatei
npx vitest packages/web                                    # Watch-Modus
cargo test -p nexus-core dispatch::                        # Rust-Tests nach Namensteil
npm run contract -- --only board                           # Vertrags-Szenarien nach Pfadteil
npm run contract -- --update --only board                  # Golden-Dateien neu aufzeichnen (Diff danach prüfen!)
```

**Tests gegen SQL Server:** Ohne Test-Datenbank melden die SQL-Server-Tests in `crates/nexus-db` „übersprungen“ und sind grün; SQLite wird immer getestet. Für den vollen Lauf eine Datenbank mit Endung `_Test` (z. B. `AiHarness_Test`, Skripte unter `crates/nexus-db/sql/`) bereitstellen und setzen:

```sh
export HARNESS_TEST_DB='{"server":"localhost","port":1433,"database":"AiHarness_Test","user":"sa","password":"…","encrypt":true,"trustServerCertificate":true}'
cargo test -p nexus-db
```

Die Tests leeren alle Tabellen; deshalb verweigern sie jede Datenbank ohne `_Test` am Ende.

**Vor dem Push** dieselben Schritte wie die CI:

```sh
npm run build && npm run typecheck && npm test && npm run contract
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast
```

### Release

Releases startet man von Hand über den GitHub-Workflow „Release“ (`.github/workflows/release.yml`, Auswahl patch/minor/major). Er erhöht die Version an allen Stellen (`scripts/release/bump.mjs`), benennt den Abschnitt „Unveröffentlicht“ im `CHANGELOG.md` um, baut Installer der Desktop-App (Windows, macOS, Linux) und CLI-Archive und veröffentlicht sie als GitHub-Release. Details: [scripts/release/README.md](scripts/release/README.md).

### Regeln und Aufbau

Tickets nie direkt in der Datenbank ändern (nur über CLI, Server oder `nexus-core`); neue Features mit Spec und Tests zuerst; deutsche Kommentare und Texte für Menschen; Änderungen unter „Unveröffentlicht“ im `CHANGELOG.md` eintragen; die Oberfläche bezieht ihre Typen aus den erzeugten Bindings unter `packages/web/src/api/generated` (ts-rs, `cargo test`).

```text
crates/nexus-core      Fachlogik: Board-Dienste, Zähler, Sperren, Worktrees, Sessions, Favoriten, Dispatcher, Installation, Anpassungen
crates/nexus-db        Board-Datenbank (SQL Server mit SQL-Skripten, SQLite mit eingebautem Schema)
crates/nexus-server    HTTP-Server auf 127.0.0.1 mit Livemeldungen
crates/nexus-cli       Befehl nexus (Alias harness)
crates/nexus-platform  Systemfunktionen je Betriebssystem
apps/desktop           Tauri-App (Nexus)
packages/web           React-Oberfläche (Vite)
packages/plugin        Manifest, Hooks, Skills, Output-Styles, Doku-Regeln und Build des Plugins
contract/              Vertragstests (API, SSE, CLI, Hooks) mit Golden-Dateien
scripts/               Entwicklungsstart, Benchmarks, Release
docs/                  ADRs, Specs und Pläne (docs/superpowers), Design-Notizen
GLOSSARY.md            Begriffe
```
