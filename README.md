# AI Harness

Lokaler Harness für mehrere Claude-Code-Sessions gleichzeitig: Kanban-Board mit Tickets, Übersicht der Sessions, konfliktfreie Parallelarbeit mit Branches und Worktrees, Favoriten zum Fortsetzen von Sessions. Tickets, Fragen und Boards liegen in einer Board-Datenbank: gemeinsam im Team auf SQL Server oder als lokale SQLite-Datei für einen einzelnen Rechner.

Bestandteile: CLI `harness`, lokaler Server mit Weboberfläche und das Claude-Code-Plugin `ai-harness` (Hooks, Skills).

## Einrichten (einmal pro Rechner)

Voraussetzungen: Rust (stable) und cargo, Node.js 22.13 oder neuer (nur für den Bau der Oberfläche), Git 2.5 oder neuer, Claude Code.

```sh
git clone https://github.com/BenjaminBiber/Nexus.git harness
cd harness
npm install
npm run build          # Oberfläche
npm run build:plugin   # Binary und Plugin nach packages/plugin/dist/nexus
packages/plugin/dist/nexus/bin/nexus install
```

`nexus install` installiert das Plugin in Claude Code (lokaler Marktplatz), macht `nexus` (und den Alias `harness`) im Terminal verfügbar (`~/.nexus`, Benutzer-PATH) und legt die Doku-Regeln unter `~/.nexus/rules` ab. Das Plugin ist danach **ausgeschaltet** und wird nur in Projekten mit `harness init` eingeschaltet. Danach ein **neues** Terminal öffnen.

## Aktualisieren

```sh
git pull
npm install            # nur bei geänderter package.json
npm run build
npm run build:plugin
packages/plugin/dist/nexus/bin/nexus install
```

Baut neu und aktualisiert Plugin, `nexus` und Doku-Regeln. Danach in laufenden Sessions `/reload-plugins` eingeben; bestehende Projekte brauchen einmal `harness init` (Board-Regel).

## Verwenden

Im Projektordner (Git-Repository):

```sh
harness init     # einmal: richtet das Projekt ein, schaltet das Plugin ein, bindet die Doku- und Board-Regel ein
harness board    # startet bei Bedarf den Server und öffnet das Board im Browser
```

Danach `/reload-plugins` eingeben oder Claude Code neu starten.

Jedes Repo gehört zu einem Board: einmal `harness board boards repo <id> <pfad>` (Pfad = Hauptordner des Repos; `harness board boards list` zeigt die Boards). Im eingerichteten Repo arbeiten `harness board list`, `create`, `next-planning` und `note` ohne `--board` auf diesem Board, sonst auf dem aktiven Board aus `~/.harness/board.yaml`. `harness board start` und `done` nehmen nur Tickets des Boards an, das dem Repo zugeordnet ist.

**Ablauf:** Der Workflow von der Anforderung bis zum fertigen Ticket steht im Board im Tab **Anleitung**, mit allen Befehlen zum Kopieren. Zusätzlich gibt ihn ein Hook zu Beginn jeder Session an Claude: Claude kennt damit den Ablauf und den Stand der Tickets, schlägt nach jedem Schritt den nächsten vor und führt dich so durch den Workflow. Beides folgt deinem angepassten Workflow, falls es einen gibt (siehe „Anpassen“).

| Schritt | Befehl |
|---|---|
| Anforderungen klären | `/superpowers:brainstorming` (einfach), `/ai-harness:harness-grill` (eine Session, mittel) oder `/ai-harness:harness-wayfinder` (komplex, mehrere Sessions) |
| Spec | `/superpowers:brainstorming` schreibt sie nach `docs/superpowers/specs/`; nach Grill oder Wayfinder legt Claude die Zusammenfassung dort als Spec ab |
| Plan schreiben | `/superpowers:writing-plans` (Tasks mit Dateien, Code, Tests und Befehlen unter `docs/superpowers/plans/`) |
| Tickets anlegen | in der Session „Lege für die Tasks des Plans Tickets an“: je Task ein Ticket mit Abnahme, Schritten und Abhängigkeit zur vorigen Task; Plan, Handbuch und Diagramm per `harness board plan-json ARC-0042 --file payload.json` (setzt Geplant) |
| Für den Dispatcher vorbereiten | optional: „Bereite ARC-0042 für den Dispatcher vor“ (Labels `auto` und `profil:…`) |
| Nächstes Ticket wählen | `harness board list --status geplant --md` (alle: `harness board list`) |
| Ticket übernehmen | `harness board claim ARC-0042 --agent claude` |
| Ticket starten | `harness board start ARC-0042 --by claude` (Branch und Worktree) |
| Umsetzen | `/ai-harness:harness-implement` (ganzer Plan in einer Session: `/superpowers:subagent-driven-development`, Tickets dabei nachführen; automatisch: Dispatcher); Schritte abhaken mit `harness board step ARC-0042 2`, anlegen/ändern mit `harness board step add|edit|rm|move` (oder im Ticket-Panel) |
| Rückfragen | `harness board question add "…" --ticket ARC-0042`, Recherche mit `/ai-harness:board-question <ID>`, „Recherche starten“ auf der Seite der Frage oder `harness board question research-start <ID>`; entscheiden nur Menschen (Board → Fragen) |
| Spec-Dateien kommentieren | im Board unter Docs Text markieren und kommentieren; der Agent liest die Kommentare mit `harness spec comments` und erledigt sie mit `harness spec resolve` |
| Kommentare | im Board schreiben; sie erreichen die Session bei der nächsten Eingabe |
| Abschluss | `harness board done ARC-0042 --acceptance "Woran man sieht, dass es fertig ist"`, dann Merge von Hand |

**Spec-Kit (Altbestand):** Ältere Specs unter `specs/` bleiben nutzbar, stehen aber nicht mehr im Ablauf: `/speckit-specify`, `/speckit-plan` und `/ai-harness:harness-tasks` (kurze Titel, Dateien, Schritte); Spec-Ordner committen, dann entstehen die Tickets automatisch auf dem Board des Repos – im Backlog mit Planungsbedarf (von Hand: `harness spec import 001-name`, mit `--label auto` für den Dispatcher – nur auf neu angelegte Tickets; da der automatische Import nach `/ai-harness:harness-tasks` die Tickets schon ohne Label anlegt, bestehende mit `harness board set <ID> --label auto` ergänzen; nicht committete Specs meldet der Befehl als Warnung). Geplant werden sie mit `harness --json board next-planning --claim claude`, dann `harness board plan-json ARC-0042 --file payload.json` (setzt Geplant).

Sicherung: Bei SQL Server sichert der DBA die Board-Datenbank (SQL-Server-Sicherung). Bei SQLite: Server mit
`harness stop` beenden, keine Session und kein Dispatcher-Lauf aktiv, dann `board.db` zusammen mit `board.db-wal` und
`board.db-shm` (falls vorhanden) kopieren. Der Harness legt keine eigenen Sicherungen oder Exporte an. Altes Datei-Board: Ordner aus früheren Versionen unter `<git-common-dir>/harness` (z. B. `tickets/`, `questions/`) bleiben unangetastet liegen und werden **nicht** in die Board-Datenbank importiert.

**Schnelle Aufgaben ohne Workflow:** Für kleine Aufgaben oder Nebenaufgaben kannst du ohne Spec, Ticket und Worktree arbeiten. Claude fragt dann zuerst „Ohne Workflow direkt umsetzen?“ und arbeitet erst nach deinem ausdrücklichen Ja so; ohne Bestätigung bleibt es beim Workflow. Du kannst es auch direkt sagen, z. B. „ohne Workflow“.

Hinweise:

- Skills (`/ai-harness:…`) und Hooks gibt es nur in Projekten mit `harness init`; `harness` im Terminal überall.
- Claude Code im Projektstamm starten (bei Worktrees im Worktree-Ordner). `harness board start` richtet neue Worktrees selbst ein (Plugin-Einstellung, Doku- und Board-Rule); ältere Worktrees bekommen `.claude/settings.local.json` durch Kopieren aus dem Hauptprojekt.
- `harness init` schreibt `.claude/settings.local.json` und die Rules `.claude/rules/ai-harness-docs.md` (lädt die Doku-Regeln beim Zugriff auf `docs/`) und `.claude/rules/ai-harness-board.md` (immer geladen: Tickets, Board, Fragen, Notizen und IDs wie `ARC-0042` meinen immer das AI-Harness-Board, nie ein altes Kanban oder GitHub/GitLab-Issues); alles ist lokal und wird nicht committet. Die persönliche `CLAUDE.md` bleibt unberührt. Hintergrund zu den Doku-Regeln: [ADR 0004](docs/adr/0004-doku-regeln-zentral-per-rule-import.md).
- Schon eingerichtete Repos und ihre Hauptordner bekommen die Board-Regel durch einmaliges erneutes `harness init` (wiederholbar, ergänzt nur Fehlendes); `nexus install` aktualisiert nur den zentralen Inhalt unter `~/.nexus/rules`, legt aber keine Rules in Projekten an. Neue Worktrees aus `harness board start` bekommen beide Rules automatisch.
- Kontrolle: `claude plugin list` in einem Projekt ohne Harness muss `ai-harness@ai-harness-local` als ausgeschaltet zeigen; sonst einmal `claude plugin disable ai-harness@ai-harness-local --scope user` (macht auch `nexus install`).
- Doku-Regeln ändern nur in `packages/plugin/manifest/rules/docs.md`, die Board-Regel nur in `packages/plugin/manifest/rules/board.md`, danach `npm run build:plugin` und `nexus install`. Projektspezifisches gehört in die `CLAUDE.md` des Projekts.

### Board-Datenbank

Zwei Varianten: **SQL Server** für das Team (Standard) oder **SQLite** für einen einzelnen Rechner ohne DB-Server.

#### Variante A: SQL Server (Team)

1. Einmalig (Administrator): `crates/nexus-db/sql/000_create_databases.sql` und danach
   `crates/nexus-db/sql/001_initial.sql` in `AiHarness` und `AiHarness_Test` ausführen
   (Details in `crates/nexus-db/sql/README.md`).
2. Pro Rechner: `harness board` öffnen → **Einstellungen → Board-Datenbank**: Server, Benutzer,
   Passwort, eigenen Namen eintragen, „Verbindung testen“, „Speichern“. Die Benutzer-ID (GUID)
   wird vorgeschlagen und kann im Team abgestimmt werden.

Schema-Änderungen kommen als neues Skript `crates/nexus-db/sql/00N_….sql`; `harness` meldet, welches fehlt.

#### Variante B: SQLite (Einzelplatz)

1. `harness board` öffnen → **Einstellungen → Board-Datenbank** → „SQLite (lokal)“. Dateipfad übernehmen
   (Vorschlag `~/.harness/board.db`) oder ändern, eigenen Namen eintragen, „Verbindung testen“, „Speichern“.
2. Ohne Oberfläche gleichwertig: `~/.harness/board.yaml` anlegen mit
   ```yaml
   provider: sqlite
   file: board.db          # optional; relativ zu ~/.harness, Standard board.db
   userId: <eigene GUID>
   userName: <Anzeigename>
   ```
3. Die Datei und ihr Schema legt der Harness beim ersten Zugriff selbst an; SQL-Skripte sind nicht nötig.

Grenzen: nur dieser Rechner (kein Teambetrieb), nicht auf Netzlaufwerk oder in synchronisierten Ordnern
(OneDrive, Dropbox), kein Umzug von Daten zwischen SQL Server und SQLite (ein Wechsel startet mit leerem Board).

Erstes Board (beide Varianten): `harness board project create ARC "Arches"`, dann
`harness board boards create ARC "Mein Board" --use`.

### Dispatcher (Tickets automatisch umsetzen lassen)

Der Server prüft jede Minute die Spalte **Geplant** der eigenen Boards (Auswahl in den Einstellungen, sonst das aktive
Board). Bereit ist ein Ticket mit Label `auto` (abschaltbar), Plan, Handbuch und Abnahme, ohne offene Voraussetzung oder
Frage, auf einem Board mit Repo-Pfad. Der Dispatcher übernimmt es als `junior-<ID>`, legt Branch und Worktree an und
startet `claude -p` mit deiner Claude-Anmeldung (ohne `ANTHROPIC_API_KEY`). Danach:

- **Erledigt:** Merge in den Zielbranch des Tickets bzw. den Branch, der im Hauptordner ausgecheckt ist (`git merge --no-ff`), danach Worktree und Branch
  aufräumen. Teilen mehrere Tickets einen Worktree, wird erst gemergt, wenn alle fertig sind. Lokale Änderungen im
  Hauptordner verhindern den Merge (Ausnahmen per Muster, z. B. `*.yaml`). Bei Konflikten löst eine Merge-Session im
  Worktree sie; sie startet nur, wenn der Dispatcher eingeschaltet, im Zeitfenster und nicht pausiert ist.
- **Merge gescheitert:** Das Ticket steht auf Blockiert mit Grund „Merge: …“. Nach dem Beheben im Panel
  „Erneut mergen“ oder `harness board merge <ID>`.
- **Von Hand erledigt** (z. B. nach Human Interaction): Ein von Hand auf Erledigt gesetztes Ticket mergt man mit
  „Mergen“ im Panel oder `harness board merge <ID>`.
- **Frage:** zurück nach Geplant; nach deiner Entscheidung geht es im selben Worktree weiter.
- **Abbruch, Zeitlimit oder gescheiterter Start:** zählt als Versuch, das Ticket geht zurück nach Geplant, der nächste
  Versuch nutzt ein größeres Profil; nach dem letzten Versuch Blockiert.
- **Nutzungslimit:** Pause (Standard 60 min), zählt nicht als Versuch – ab der dritten Limit-Pause in Folge für dasselbe
  Ticket aber schon. Wird Claude Code nicht gefunden, pausiert der Dispatcher ebenso, ohne Versuch.

Der Worker nutzt unter Windows die Git Bash (wird automatisch gefunden, sonst `claude.gitBashPath` in den Dispatcher-Einstellungen setzen).

Einschalten und einstellen unter **Einstellungen → Dispatcher** (Datei `~/.harness/dispatch.yaml`); Zustand und
Protokolle über den Knopf „Dispatcher: …“ in der Board-Kopfzeile oder im Terminal:

```bash
harness board dispatch status          # Zustand, Worker, Merges, warum Tickets nicht bereit sind
harness board dispatch run             # sofort prüfen
harness board dispatch pause --minutes 60
harness board dispatch resume
harness board dispatch enable          # bzw. disable
harness board merge ARC-0009           # Merge erneut anstoßen
```

**Recherchen zu Fragen:** Auf der Seite einer offenen Frage startet „Recherche starten“ (oder
`harness board question research-start ARC-F0007`) sofort eine Claude-Session `research-<ID>`, die die Frage nach dem
Skill `ai-harness:board-question` recherchiert und das Ergebnis an die Frage schreibt – unabhängig davon, ob der
Dispatcher für Tickets eingeschaltet ist; nur eine Pause (z. B. Nutzungslimit) verhindert den Start. Sie läuft im Repo
des eigenen Boards, zu dem die Frage gehört (Board eines verknüpften Tickets, sonst das aktive Board des Projekts, sonst
das eigene Board des Projekts mit Repo-Pfad und kleinster ID), in einem eigenen, losgelösten Worktree
`<repo>-worktrees/research-<ID>`, der danach wieder entfernt wird (verwaiste räumt jeder Tick auf). Die Session läuft
unabhängig von `claude.permissionMode` und `claude.extraArgs` mit `--permission-mode default`, darf nur lesen (Read,
Glob, Grep, `git log/show/diff`) und von der Board-CLI `question show`, `question research`, `show` und `list` nutzen;
Edit/Write (außer der Datei `research.json` im Recherche-Worktree), Commits, Branchwechsel, `npm`/`node` und Entscheidungen sind per `--disallowedTools` verboten (gilt auch
gegen Erlaubnisse in `~/.claude/settings.json`). Das Ergebnis schreibt der Agent in `research.json` und speichert es mit
`harness board question research <ID> --from research.json`. Als Ergebnis zählt nur eine Recherche des Agents `research-<ID>` ab dem
Start; sonst steht `[dispatch] Recherche ohne Ergebnis: …` an der Frage, ein Neuversuch erfolgt nicht automatisch. Unter
**Einstellungen → Dispatcher → Recherchen** lässt sich einschalten, dass neue Fragen zu deinen Boards automatisch
recherchiert werden: nur Fragen, die nach dem Einschalten entstehen (Aus- und Einschalten beginnt neu), die du (oder ein
Agent unter deinem Benutzer) gestellt hast oder die mit Tickets deiner Boards verknüpft sind, je Frage einmal; Grenzen
„Gleichzeitige Recherchen“ (1–5) und „Automatische Recherchen je Tag“ (Vorgabe 10); Profil wählbar (ohne Wahl das Profil
`research`, falls vorhanden, sonst das Standardprofil). Fremde Fragen startest du von Hand. Umgebungsfehler (z. B. Git Bash
fehlt, Repo-Pfad kein Git-Repo) setzen die Automatik 30 min aus und stehen einmal im Protokoll und in der Übersicht. In
`dispatch.yaml`: `research: { auto: true, wip: 1, maxPerDay: 10, profile: klein }`.

**Planen am Ticket:** Im Ticket-Panel startet „Planen“ (oder `harness board plan-start ARC-0007`) sofort eine
Claude-Session `planner-<ID>`, die das Ticket nach dem Skill `ai-harness:board` plant – wie eine Recherche unabhängig davon,
ob der Dispatcher eingeschaltet ist, nur nicht während einer Pause. Das geht für Tickets eigener Boards mit Repo-Pfad in
Backlog, In Planung, Geplant oder Blockiert ohne offene Frage. Die Session läuft in einem losgelösten Worktree
`<repo>-worktrees/plan-<ID>` (danach entfernt), mit `--permission-mode default`, darf nur lesen, `plan.json` schreiben und
von der Board-CLI `show`, `list`, `plan-json`, `comment`, `note list` sowie `question show/list/add` nutzen; Status, Labels
und Zuständigkeit ändert sie nicht von Hand. Sie speichert den Plan mit `harness board plan-json` (Ticket danach Geplant)
oder legt eine Frage an, wenn eine Entscheidung fehlt. Ohne beides steht `[dispatch] Planung ohne Ergebnis: …` am Ticket.
Planungen zählen nicht zum WIP. Profil unter **Einstellungen → Dispatcher → Recherchen → Planungen** (ohne Wahl das Profil
`planning`, falls vorhanden, sonst das Standardprofil); in `dispatch.yaml`: `planning: { profile: schwer }`.

Lauf-, Pause- und Merge-Zustand liegen unter `~/.harness/dispatch/`, Protokolle unter `~/.harness/dispatch-logs/`. Die
Worker laufen losgelöst weiter, wenn der Server neu startet. Erlaubte Werkzeuge stehen in der Einstellung „Erlaubte
Tools“; braucht ein Projekt z. B. `dotnet test`, gehört `Bash(dotnet test:*)` dazu. Arbeitet derselbe Nutzer auf
mehreren Rechnern, den Dispatcher nur auf einem davon einschalten – sonst übernehmen beide Rechner Tickets derselben Boards.

## Anpassen

Workflow, Skills und Output-Styles lassen sich pro Dev ersetzen oder ergänzen. Die Anpassungen liegen in `custom/` in diesem Ordner; Git ignoriert ihn, Updates laufen ohne Merge.

In einer Claude-Session in einem eingerichteten Projekt:

```text
/ai-harness:harness-customize
```

Der Skill kopiert den Standard als Ausgangspunkt, legt Neues an, prüft und setzt auf Wunsch zurück. Anpassungen wirken erst nach dem Anwenden in diesem Ordner (Pflicht), danach `/reload-plugins`:

```sh
nexus customize check          # prüfen, ohne etwas zu ändern
npm run build:plugin           # anwenden: baut das Plugin mit den Anpassungen
packages/plugin/dist/nexus/bin/nexus install
```

Ungültige Anpassungen fallen mit Warnung auf den Standard zurück; die Ausgabe von `build:plugin` und das Panel „Anpassungen“ in den Einstellungen des Boards zeigen, was gilt. Was stabil bleibt, steht in `CHANGELOG.md`.

## Daten und Einstellungen

| Was | Ort |
|---|---|
| Token, Zähler, Worktree-Verzeichnis | `<git-common-dir>/harness/` im Projekt (nicht versioniert) |
| Projekte, Sessions, Favoriten, Server-Protokoll | `~/.harness/` |
| Tickets, Fragen, Projekte, Boards, Notizen (SQL Server) | Datenbank `AiHarness` (Schema `board`); Sicherung durch den DBA |
| Tickets, Fragen, Projekte, Boards, Notizen (SQLite) | `~/.harness/board.db` (oder `file` aus `board.yaml`) samt `-wal`/`-shm`; Sicherung siehe oben |
| Anbieter und Verbindung der Board-Datenbank, eigene Benutzer-ID, aktives Board | `~/.harness/board.yaml` |
| Einstellung des Dispatchers | `~/.harness/dispatch.yaml` |
| Läufe, Pause und Merges des Dispatchers, Protokolle | `~/.harness/dispatch/`, `~/.harness/dispatch-logs/` |
| Doku-Regeln (Importziel der Projekte) | `~/.harness/rules/`, Quelle `packages/plugin/manifest/rules/` |
| Angewendete Anpassungen | `~/.harness/customizations/`, Quelle `custom/` |
| Freigabe des Imports je Projekt | `~/.claude.json` (oder `CLAUDE_CONFIG_DIR/.claude.json`), Eintrag unter `projects` |

Umgebungsvariablen: `HARNESS_HOME` (statt `~/.harness`), `HARNESS_PORT` (statt 4710), `HARNESS_TICKET` (Ticket der Session fest vorgeben), `HARNESS_TEST_DB` (nur für Tests: Verbindungs-JSON zur Test-DB).

## Fehlersuche

| Symptom | Abhilfe |
|---|---|
| `harness` im Terminal „nicht gefunden“ | `nexus install` aus dem gebauten Plugin ausführen, neues Terminal öffnen |
| Hooks oder Skills fehlen | Im Projekt `harness init`, dann `/reload-plugins`; prüfen mit `claude plugin list` (`ai-harness@ai-harness-local`) |
| `git worktree add` scheitert mit „Filename too long“ (Windows) | Der Harness schaltet dafür `core.longpaths` in der lokalen Konfiguration des Repos ein (Hinweis beim Anlegen); notfalls von Hand `git config core.longpaths true` im Repo |
| Hooks fehlen in einem Worktree | `harness init` im Hauptprojekt oder `.claude/settings.local.json` aus dem Hauptprojekt kopieren; neue Worktrees legt `harness board start` richtig an |
| Doku-Regeln greifen nicht | Werden erst beim Lesen oder Schreiben einer Datei unter `docs/` geladen. `harness init` ausführen und neue Session starten; in `~/.claude.json` müssen für das Projekt `hasClaudeMdExternalIncludesApproved` und `hasClaudeMdExternalIncludesWarningShown` auf `true` stehen |
| Anpassung wirkt nicht | `nexus customize check` ausführen, dann `npm run build:plugin`, `nexus install` und `/reload-plugins` |
| „Port 4710 ist belegt“ | Es läuft schon ein Server; beenden (`harness stop`) oder `HARNESS_PORT` setzen |
| Projektliste im Board leer | `harness board` im Projektordner ausführen |
| „Kein Board ist dem Repo … zugeordnet“ oder „… gehört aber zu Board …“ | `harness board boards list`, dann `harness board boards repo <id> <pfad>`; oder das Ticket im Repo seines Boards bearbeiten |
| „Board-Datei ist nicht erreichbar“ / „lässt sich nicht öffnen“ (SQLite) | Pfad in Einstellungen → Board-Datenbank prüfen; Ordner beschreibbar? Datei nicht auf Netzlaufwerk/OneDrive; anderes Programm hält die Datei offen → schließen |

Für ein Projekt ausschalten: `claude plugin disable ai-harness@ai-harness-local --scope local`. Deinstallieren: `claude plugin uninstall ai-harness@ai-harness-local`, dann `~/.harness/app`, `~/.harness/bin` und `~/.harness/rules` löschen und `~/.harness/bin` aus dem PATH nehmen.

## Entwicklung am Harness

```sh
npm run dev                # Rust-Server (4710) und Vite (5173) zusammen; Strg+C beendet beide
npm run build              # Weboberfläche (Typprüfung und Bündeln)
npm run lint               # Typprüfung der Oberfläche (= npm run typecheck)
npm test                   # Tests der Oberfläche und des Plugins
npm run build:plugin       # Binary und Plugin nach packages/plugin/dist/nexus
npm run contract           # Vertrag (API, SSE, CLI, Hooks) gegen das Rust-Binary
cargo fmt --all --check    # Rust: Format
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace     # Rust: alle Tests (Fachlogik, Datenbank, Server, CLI)
```

Plugin ohne Installation laden: `claude --plugin-dir packages/plugin/dist/nexus`.

Regeln: Tickets nie direkt in der Datenbank ändern (nur über CLI, Server oder `nexus-core`); neue Features mit Spec und Tests zuerst; deutsche Kommentare und Texte für Menschen; die Oberfläche bezieht ihre Typen aus den erzeugten Bindings unter `packages/web/src/api/generated` (ts-rs, `cargo test`).

```text
crates/nexus-core      Fachlogik: Board-Dienste, Zähler, Sperren, Worktrees, Sessions, Favoriten, Dispatcher, Installation, Anpassungen
crates/nexus-db        Board-Datenbank (SQL Server mit SQL-Skripten, SQLite mit eingebautem Schema)
crates/nexus-server    HTTP-Server auf 127.0.0.1 mit Livemeldungen
crates/nexus-cli       Befehl nexus (Alias harness)
crates/nexus-platform  Systemfunktionen je Betriebssystem
apps/desktop           Tauri-App (Nexus)
packages/web           React-Oberfläche (Vite)
packages/plugin        Manifest, Hooks, Skills, Output-Styles, Doku-Regeln und Build des Plugins
GLOSSARY.md            Begriffe
```
