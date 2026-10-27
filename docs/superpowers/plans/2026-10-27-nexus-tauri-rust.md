# Plan: Nexus als Desktop-App mit Tauri und Rust

Stand: 27.10.2026 · Ausgangslage: AI Harness (TypeScript-Monorepo) in diesem Repo

## 1. Ziel

Aus dem AI Harness wird **Nexus**: eine kompakte, schnelle Desktop-App für macOS, Windows und Linux. Die Weboberfläche im Browser bleibt erhalten, CLI und Claude-Code-Plugin auch.

Die App darf in keinem Punkt langsamer werden als heute. Darum gibt es feste Grenzwerte, die vor jeder Phase gemessen werden (Phase 0 legt die Ausgangswerte fest):

| Messgröße | Grenzwert Ende Phase 5 | Mindestens |
|---|---|---|
| Installer macOS (`.dmg`, je Architektur) | ≤ 15 MB | – |
| Installer Windows (NSIS `.exe`) | ≤ 10 MB | – |
| Linux `.deb` / `.rpm` | ≤ 10 MB | – |
| Linux `.AppImage` | ≤ 90 MB (bringt WebKitGTK mit) | – |
| Kaltstart bis Board sichtbar | ≤ 500 ms | nicht schlechter als heute |
| RAM im Leerlauf (App + Server) | ≤ 120 MB | nicht schlechter als heute |
| Laufzeit eines Hooks (`session-start`, `user-prompt`) | ≤ 30 ms | nicht schlechter als heute |
| Antwortzeit der API (p95, Board laden) | ≤ heute | nicht schlechter als heute |

Kein Schritt wird übernommen, der eine Messgröße verschlechtert.

## 2. Ausgangslage im Code

| Paket | Zeilen (ca.) | Inhalt | Künftig |
|---|---|---|---|
| `packages/web` | 20.500 | React 19, Vite, Router, marked, mermaid, DOMPurify | **bleibt**, läuft in Tauri |
| `packages/core` | 14.500 | Board-Logik, Dispatch, Git/Worktrees, Docs, Anpassungen, Installation | → Rust (`nexus-core`) |
| `packages/server` | 8.900 | Fastify, 118 Routen, SSE, Dispatch-Worker, Benachrichtigungen | → Rust (`nexus-server`) |
| `packages/db` | 3.900 | SQLite (`node:sqlite`) und SQL Server (`mssql`), Schema 001–008 | → Rust (`nexus-db`) |
| `packages/cli` | 2.900 | `harness` (commander), Hooks | → Rust (`nexus-cli`) |
| `packages/plugin` | – | Hooks, Skills, Rules, Output-Styles | bleibt, ruft künftig das Rust-Programm auf |

Wichtige Punkte im heutigen Code, die der Plan berücksichtigen muss:

- **Hooks öffnen die Board-DB direkt** (`cli/src/hooks/BoardAccess.ts`) und starten heute für jeden Aufruf Node (`plugin/manifest/hooks/hooks.json`, Timeout 5 s).
- **`RequestGuard`** erlaubt nur `127.0.0.1`/`localhost` als Host und Origin und verlangt bei Schreibzugriffen `X-Harness: 1`. Eine Oberfläche mit Tauri-Origin (`tauri://localhost`, `http://tauri.localhost`) würde heute abgewiesen.
- **`ApiClient`** hat schon einen `base`-Parameter. Die Oberfläche kann also auch von einer anderen Origin aus mit dem Server reden.
- **Systemnahe Teile** laufen über Kindprozesse und sind teils nur für Windows gelöst: `WindowsToastSender` (PowerShell/WinRT), `FolderPicker`, `FileOpener`, `BrowserOpener`, `WindowsUserPath`, `GitBashLocator`, `LongPaths`.
- **Live-Updates** kommen per SSE (`/api/projects/:pid/events`). Dazu gehören Board-, Fragen- und Docs-Änderungen sowie `notice` statt Windows-Toast.
- **Benutzerdaten** liegen in `~/.harness` (`HarnessHome`, mit `HARNESS_HOME` überschreibbar): `board.db`, `*.yaml`, `*.json`, `dispatch/`, `rules/`, `customizations/`.
- **Tests:** rund 355 Testdateien. Die Prozess-Tests (`vitest.process.config.ts`) starten echte Server und CLI.

## 3. Zielarchitektur

```
nexus/
├─ Cargo.toml                    # Rust-Workspace
├─ crates/
│  ├─ nexus-core/                # Fachlogik ohne I/O-Framework (aus packages/core)
│  ├─ nexus-db/                  # Trait BoardDatabase + SQLite (rusqlite) + SQL Server (tiberius)
│  ├─ nexus-server/              # axum: HTTP-API, SSE, Dispatch-Worker, Watcher, Benachrichtigungen
│  ├─ nexus-cli/                 # clap: alle Befehle + Hooks; Binary „nexus“
│  └─ nexus-platform/            # Systemfunktionen hinter Traits (Benachrichtigung, Ordnerwahl, Öffnen, PATH)
├─ apps/
│  └─ desktop/                   # Tauri 2 (src-tauri/), nutzt nexus-server im selben Prozess
├─ packages/web/                 # React-Oberfläche (unverändert, Build landet in der App)
├─ packages/plugin/              # Claude-Code-Plugin (hooks.json ruft „nexus hook …“)
├─ contract/                     # API-, CLI- und Hook-Vertrag (Golden-Dateien), gilt für alt und neu
├─ scripts/release/bump.mjs      # setzt die Version an allen Stellen
└─ .github/workflows/            # ci.yml (jeder Push/PR), release.yml (manuell: patch/minor/major)
```

### Grundentscheidungen

1. **Die HTTP-API ist der Vertrag.** Die 118 Routen bleiben mit gleichen Pfaden, gleichen JSON-Formen und gleichen Fehlercodes erhalten. Dadurch merkt die Oberfläche nichts vom Wechsel und der Umbau kann Route für Route erfolgen.
2. **Ein Programm `nexus` für alles ohne Fenster.** CLI, Hooks und der Server ohne Oberfläche (`nexus serve`) stecken in einer Datei. Die Desktop-App bindet dieselben Crates ein und braucht keinen zweiten Prozess.
3. **Die DB-Schemata bleiben gleich.** Dieselbe `board.db` und dieselbe SQL-Server-Datenbank funktionieren mit alter und neuer Version. So können TS- und Rust-Teile während des Umbaus nebeneinander laufen, und ein Zurück ist jederzeit möglich. `SchemaVersionGuard` wird in Rust nachgebaut.
4. **Ein Server pro Benutzer.** Die App prüft `server.json` und `/api/health`. Läuft schon ein Server, verbindet sie sich mit ihm, sonst startet sie ihn im eigenen Prozess. Schließt man das Fenster, bleibt die App im Tray, damit laufende Dispatch-Läufe weiterlaufen. „Beenden“ fragt nach, wenn ein Lauf aktiv ist.
5. **Die Oberfläche bleibt Web.** Tauri nutzt WKWebView (macOS), WebView2 (Windows) bzw. WebKitGTK (Linux). Keine Neuentwicklung der Oberfläche.

### Rust-Bausteine

| Aufgabe | Heute | Rust |
|---|---|---|
| HTTP-Server, statische Dateien | fastify, @fastify/static | `axum`, `tower-http` (inkl. vorkomprimierte `.br`/`.gz`) |
| SSE | eigene Umsetzung in `EventRoutes` | `axum::response::sse` + `tokio::sync::broadcast` |
| SQLite | `node:sqlite` | `rusqlite` (Feature `bundled`), WAL wie heute |
| SQL Server | `mssql` | `tiberius` + `bb8-tiberius` (Pool) |
| Async-Laufzeit, Prozesse | Node | `tokio`, `tokio::process` |
| Git | `GitRunner` (ruft `git` auf) | weiter `git` aufrufen (`tokio::process`). `gix` nur für schnelle Lesezugriffe, falls eine Messung das rechtfertigt |
| Dateien beobachten | `BoardChangeWatcher`, `SpecDocWatcher` | `notify` + `notify-debouncer-full` |
| YAML/JSON | `yaml`, JSON | `serde`, `serde_yaml_ng`, `serde_json` |
| Markdown-Abschnitte, Front-Matter | eigene Parser | `pulldown-cmark` bzw. eigene Portierung |
| CLI | commander | `clap` (derive), deutsche Hilfe wie `GermanHelp` |
| Dateisperre, atomares Schreiben | `FileLock`, `AtomicWriter` | `fs4` + Schreiben in Temp-Datei und `rename` |
| Fehler | `HarnessError` | `thiserror` (Bibliothek), `anyhow` (Binary) |
| Logging | eigene Logdatei | `tracing` + `tracing-appender` |
| Benachrichtigungen | PowerShell/WinRT, ntfy | `tauri-plugin-notification` in der App. Ohne App: `notify-rust` bzw. `winrt-notification`. ntfy über `reqwest` |

### Tauri-Plugins der App

`tauri-plugin-single-instance`, `-notification`, `-dialog` (ersetzt `FolderPicker`), `-opener` (ersetzt `FileOpener`/`BrowserOpener`), `-deep-link` (`nexus://ticket/ARC-0042`), `-autostart`, `-updater`, `-window-state`, `-log`. Dazu ein Tray mit Statussymbol (bereit / wartet / Frage offen), angelehnt an `AttentionTracker` und `ToastModel`.

## 4. Phasen

Jede Phase endet mit einem lauffähigen Stand. Größenangaben sind grobe Schätzungen in Personentagen (PT) für eine Person mit KI-Unterstützung.

### Phase 0 – Messen und Vertrag festhalten (≈ 5 PT)

1. **Messskript** `scripts/bench/` für die Werte aus Abschnitt 1:
   - Kaltstart Server
   - `GET` Board mit 500 Tickets
   - SSE-Latenz von Änderung bis Ereignis
   - Hook-Laufzeit (`hyperfine "node …/cli.mjs hook user-prompt"`)
   - RAM von Server und Browser-Tab

   Ergebnisse als `contract/baseline.json` ablegen.
2. **API-Vertrag:** Ein Testlauf gegen den TS-Server schreibt Anfrage und Antwort aller 118 Routen als Golden-Dateien nach `contract/api/`. Variable Werte (Zeitstempel, IDs) werden ersetzt. Daraus wird ein Prüfprogramm, das später gegen jeden Server laufen kann (`contract/run --base http://127.0.0.1:<port>`).
3. **CLI- und Hook-Vertrag:** Ausgaben von `harness … --json` und `--md` sowie stdin/stdout aller sieben Hooks als Golden-Dateien.
4. **SSE-Vertrag:** Reihenfolge und Form der Ereignisse (`hello`, `sessions`, `dbboard`, `dbquestions`, `docs-changed`, `notice`, Herzschlag).

*Fertig, wenn:* Das Prüfprogramm läuft gegen den heutigen Server ohne Fehler, und die Ausgangswerte sind festgehalten.

### Phase 1 – Tauri-Hülle mit Node-Sidecar (≈ 8 PT)

Ziel: sofort eine echte App für macOS, Windows und Linux, ohne Code-Umbau und ohne dass etwas langsamer wird.

1. `apps/desktop` mit `npm create tauri-app` (Tauri 2) anlegen und in den Workspace aufnehmen.
2. **Sidecar:** Server und CLI mit esbuild zu einer Datei bündeln und als Node Single Executable (SEA) bauen. In `tauri.conf.json` unter `bundle.externalBin` eintragen, für `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc` und `x86_64-unknown-linux-gnu`.
3. **Start:** Die App liest `server.json` und ruft `/api/health` auf. Antwortet kein Server, startet sie den Sidecar mit freiem Port. Das Fenster lädt `http://127.0.0.1:<port>/`. Dadurch bleibt die Origin gleich, und `RequestGuard` muss sich nicht ändern.
4. **Systemfunktionen:**
   - Tauri-IPC für diese Origin über `capabilities` → `remote.urls` freigeben (`http://127.0.0.1:*`, `http://localhost:*`).
   - In `packages/web` eine kleine Schicht `platform.ts` einführen. Läuft die Oberfläche in Tauri (`window.__TAURI_INTERNALS__`), nutzt sie Dialog, Opener und Notification der App, sonst die bisherigen API-Aufrufe.
   - Die App meldet sich am SSE-Strom mit `notify=1` an, damit der Server `notice` statt PowerShell-Toast schickt.
5. Tray, Single-Instance, Fensterzustand, Deep-Links.
6. **Paketieren:** `.dmg` (getrennt arm64/x64), NSIS-Installer mit WebView2-Bootstrapper im Download-Modus, für Linux `.AppImage`, `.deb` und `.rpm`. Gebaut wird über den Release-Workflow aus Phase 6.

*Fertig, wenn:* Die App startet auf macOS, Windows und Linux, alle Funktionen der Weboberfläche laufen, der Vertrag aus Phase 0 ist grün und keine Messgröße ist schlechter als die Ausgangswerte.

> Diese Phase ist noch **nicht** kompakt (der Sidecar bringt Node mit, ~35–45 MB). Kompakt wird es erst nach Phase 5.

### Phase 2 – Rust-Workspace und Datenbank (≈ 15 PT)

1. `Cargo.toml` (Workspace), `crates/nexus-core`, `crates/nexus-db` anlegen. CI baut und testet Rust parallel zu TS.
2. **Typen:** DTOs und Fachtypen aus `core/src/board/*Types.ts`, `BoardQuestionTypes.ts`, `DispatchRunTypes.ts` usw. als `serde`-Structs. Mit `ts-rs` oder `specta` die TS-Typen für `packages/web` daraus erzeugen. Ab dann ist Rust die Quelle der Typen.
3. **Trait `BoardDatabase`** nach dem Vorbild von `core/src/board/BoardDatabase.ts` mit Repositories (Ticket, Step, Comment, Note, Question, QuestionComment, QuestionHistory, QuestionResearch, History, Deletion, DispatchRun, DispatchStats, Project, Search, TicketSession, User, Board) und Unit of Work.
4. **SQLite** (`rusqlite`): Schema-Schritte 001–008 aus `db/src/sqlite/schema/` 1:1 übernehmen. Tabelle und Nummern der Schema-Version bleiben gleich, damit eine von TS angelegte `board.db` ohne Änderung geöffnet wird. Die Volltextsuche wie in `SqliteSearchRepository` nachbauen.
5. **SQL Server** (`tiberius`): Repositories nach `db/src/Mssql*.ts`, `MssqlFeatureGate`, `SqlChunks`, `RowGroups`, `rowver`-Prüfung für gleichzeitige Änderungen. Die Skripte in `db/sql/` bleiben die Quelle.
6. **In-Memory-Datenbank** für Tests (wie `InMemoryBoardDatabase`).
7. **Tests:** `packages/db/tests` als Rust-Integrationstests portieren. Zusätzlich ein Gegenproben-Test: TS schreibt und Rust liest dieselbe DB, und umgekehrt.

*Fertig, wenn:* Alle portierten DB-Tests sind für SQLite und SQL Server grün und die Gegenprobe funktioniert in beide Richtungen.

### Phase 3 – CLI und Hooks in Rust, Umbenennung zu Nexus (≈ 20 PT)

Diese Phase bringt den ersten spürbaren Gewinn im Alltag, weil Hooks nicht mehr Node starten.

1. **Fachlogik nach `nexus-core`**, nur was CLI und Hooks brauchen:
   - `board/` (Services, Abhängigkeiten, IDs, Reihenfolge, Planung, Suche, Timeline)
   - `hooks/` (`ContextBuilder`, `HookEventFactory`, `TicketAssigner`, `QuestionContextMarks`, `EnvFileWriter`)
   - `workflow/`, `git/`, `worktrees/`, `sessions/`, `favorites/`, `settings/`, `registry/`, `locking/`, `install/`, `docs/` (Kommentare, Spec-Prüfung)
   - `customization/`
2. **`nexus-cli`** mit `clap`: alle Befehle aus `cli/src/commands/**` mit gleichen Optionen, deutscher Hilfe und Fehlermeldungen (`GermanHelp`, `GermanErrors`). Ausgabe über die Formatter (`BoardTextFormatter`, `BoardQuestionFormatter`, `DispatchStatusFormatter`).
3. **Hooks:** `nexus hook <event>` liest stdin und öffnet die DB direkt wie heute.
   - `hooks.json` ruft künftig `${CLAUDE_PLUGIN_ROOT}/bin/nexus` bzw. `nexus.exe` auf statt `node …/cli.mjs`.
   - `build-plugin.mjs` legt das passende Binary je Plattform ins Plugin.
   - Ziel: unter 30 ms je Hook.
4. **Serverstart aus der CLI:** `nexus board` startet bis Phase 5 weiter den Node-Server (bzw. die App, falls installiert) und öffnet Browser oder App.
5. **Umbenennung mit Übergangszeit:**
   - Datenordner `~/.nexus`, mit `NEXUS_HOME` überschreibbar. Fehlt er, aber `~/.harness` existiert, kopiert `nexus migrate` (und automatisch der erste Start) die Dateien. `board.db` wird mit `-wal`/`-shm` und nur bei gestopptem Server kopiert. `~/.harness` bleibt unverändert liegen.
   - `HARNESS_*`-Variablen werden noch gelesen, mit Hinweis.
   - Befehl `harness` bleibt als Alias, der eine Weile lang einen Hinweis ausgibt.
   - Plugin `nexus@nexus-local`. `nexus init` stellt bestehende Projekte um: `.claude/settings.local.json`, Rules `nexus-docs.md`/`nexus-board.md`, altes Plugin aus.
   - Ticket-IDs (`ARC-…`) und Board-Daten bleiben unverändert.
6. **Installation ohne Node:** `nexus install` übernimmt `npm run setup`. Es legt das Binary in den Benutzer-PATH (`UserPath`, `WindowsUserPath`), richtet das Plugin ein und schreibt die Rules nach `~/.nexus/rules`.

*Fertig, wenn:* Der CLI- und Hook-Vertrag ist gegen das Rust-Binary grün, die Hook-Laufzeit liegt unter dem Grenzwert und bestehende Projekte laufen nach `nexus init` ohne Datenverlust.

### Phase 4 – Server in Rust, Route für Route (≈ 35–45 PT)

**Vorgehen:** Der Rust-Server (`nexus-server`, axum) übernimmt den Port und leitet alle noch nicht portierten Routen an den Node-Sidecar weiter (Reverse-Proxy mit `hyper`, auch für SSE). Jede Gruppe wird erst umgeschaltet, wenn ihr Vertragstest gegen Rust grün ist.

Reihenfolge (Abhängigkeiten und Risiko aufsteigend):

| Schritt | Routen / Bausteine heute | Bemerkung |
|---|---|---|
| 4.1 | `HealthRoutes`, `StaticRoutes`, `RequestGuard`, `ErrorMapper`, `ServerConfigLoader`, `ServerInfoFile` | Grundgerüst, Proxy, Logdatei, `server.json` |
| 4.2 | `SettingsRoutes`, `UserSettingsRoutes`, `BoardConfigRoutes`, `ProjectRoutes` | einfache Datei- und Konfig-Zugriffe |
| 4.3 | `DbBoardRoutes`, `DbTicketRoutes`, `DbNoteRoutes`, `DbQuestionRoutes`, `DbSearchRoutes`, `DbStatsRoutes`, `DbGraphRoutes`, `DbInboxRoutes`, `TicketTimelineAssembler`, `BoardRuntimeHolder` | Kern des Boards, größter Nutzen für die Ladezeit |
| 4.4 | `EventRoutes`, `EventHub`, `BoardChangeWatcher`, `SpecDocWatcher`, `BrowserNoticeHub` | SSE muss mit dem Vertrag übereinstimmen, auch der Herzschlag |
| 4.5 | `DocsRoutes`, `SpecRoutes`, `DocCommentsRoutes`, `docs/*` (Revisionen, Entwürfe, Ticket-Entwürfe) | Markdown-Abschnitte, Doc-Hash |
| 4.6 | `SessionRoutes`, `FavoriteRoutes`, `HookRoutes`, `UsageRoutes` (`ClaudeUsageService`) | Sitzungen und Favoriten |
| 4.7 | `WorkflowRoutes`, `CustomizationRoutes` | Anpassungen, Workflow-Validierung |
| 4.8 | Benachrichtigungen: `NotificationService`, `AttentionTracker`, `DbQuestionWatch`, `QuestionChangeDetector`, `NtfySender`, Toasts | ohne App über `nexus-platform`, mit App über Tauri |
| 4.9 | Dispatch: `DispatchRoutes`, `DispatchSuggestionRoutes`, `dispatch/*` (Scheduler, Worker, `ClaudeProcess`, `WorkerLauncher`, `WorktreeSetupRunner`, `TicketStarter`, `PlanRunner`, `ResearchRunner`, `ReviewService`, `MergeService`, `ResultHandler`, `RunRecorder`) und `core/src/dispatch/**` | größtes Risiko, kommt zuletzt |

Hinweise zu Dispatch (4.9):

- **Prozesse:**
  - `claude` mit `tokio::process` starten und stream-json zeilenweise lesen (`StreamEventFormatter`, `RunOutputReader`, `ClaudeResultEnvelope`).
  - Abbruch über Prozessgruppen: unter Windows Job-Objekte, damit Kindprozesse mitbeendet werden; unter Unix `setsid` + `killpg`.
- **Windows:**
  - `CREATE_NO_WINDOW`, damit beim Start von `git` und `claude` keine Konsolenfenster aufblitzen.
  - Git Bash wie in `GitBashLocator` suchen.
  - Lange Pfade wie in `LongPaths`.
- **Zustand:** `dispatch/state.json`, `runs/`, `reviews/` und Logs bleiben im selben Format, damit Läufe über ein Update hinweg erhalten bleiben.
- Vor dem Umschalten laufen beide Umsetzungen eine Woche lang mit denselben Testtickets parallel (nur eine schreibt) und die Ergebnisse werden verglichen.

*Fertig, wenn:* Alle 118 Routen und der SSE-Vertrag sind gegen Rust grün, kein Aufruf geht mehr an den Sidecar, und kein Wert aus Abschnitt 1 ist schlechter als die Ausgangswerte.

### Phase 5 – Node entfernen, schlank machen (≈ 5 PT)

1. Sidecar, Proxy und `packages/{core,db,server,cli}` entfernen. Die TS-Tests sind bis dahin durch Rust-Tests und den Vertrag ersetzt.
2. **App-Prozess:** `nexus-server` läuft in der Tauri-App im selben Prozess (eigene `tokio`-Runtime). `nexus serve` bleibt für Betrieb ohne App.
3. **Oberfläche aus der App laden** statt vom Server:
   - `RequestGuard` erlaubt zusätzlich die Tauri-Origins (`tauri://localhost`, `http://tauri.localhost`).
   - `ApiClient` bekommt in der App die Server-Adresse als `base`.
   - Statt Port und Pflichtkopf wird für die App ein zufälliges Token beim Start übergeben.

   Im Browser bleibt alles wie heute.
4. **Größe:**
   - Release-Profil: `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.
   - Unnötige Features abschalten (`tokio` nur `rt-multi-thread`, `net`, `process`, `fs`, `sync`, `time`, `macros`). `rusqlite` mit `bundled`, `tiberius` mit `rustls` statt nativer TLS.
   - Größe mit `cargo bloat` prüfen.
   - Oberfläche: `mermaid` wird schon bei Bedarf geladen (`MermaidBlock.tsx`). Schriften auf die genutzten Schnitte begrenzen.
5. Abschlussmessung gegen Abschnitt 1.

### Phase 6 – Auslieferung über GitHub Actions (≈ 8 PT, parallel zu Phase 1)

Releases entstehen nur über einen **manuell gestarteten GitHub-Actions-Workflow**. Beim Start wählt man, ob die Änderungen ein `patch`, `minor` oder `major` sind. Der Workflow erhöht die Version, baut die Installer für macOS, Windows und Linux und hängt sie an ein GitHub-Release mit dieser Version.

Der Workflow wird schon in Phase 1 eingerichtet. Bis Phase 5 liefert er die App mit Node-Sidecar aus, danach die schlanke Rust-App, ohne dass sich am Ablauf etwas ändert.

#### Version

- **Eine Quelle:** `[workspace.package] version` in der `Cargo.toml` des Workspace. Die Crates und `apps/desktop/src-tauri` übernehmen sie mit `version.workspace = true`. Tauri liest die Version dann aus dem Crate, `tauri.conf.json` hat kein eigenes Feld `version`.
- **Mitgezogen** werden die Version im Plugin-Manifest (`packages/plugin`, `marketplace.json`) und, solange es sie gibt, in den `package.json`-Dateien. Das erledigt `scripts/release/bump.mjs`, das alle Stellen in einem Schritt setzt und prüft, dass danach überall dieselbe Version steht.
- Versionen folgen SemVer, Tags heißen `vX.Y.Z`.
- Ausgangswert ist das höchste vorhandene Tag `v*`. Gibt es keines, gilt die Version aus der `Cargo.toml`.

#### Workflow `.github/workflows/release.yml`

```yaml
name: Release
on:
  workflow_dispatch:
    inputs:
      bump:
        description: Art der Änderungen
        type: choice
        options: [patch, minor, major]
        required: true
concurrency: release          # nie zwei Releases gleichzeitig
permissions:
  contents: write
```

**Job 1 `version`** (ubuntu-latest), nur auf `main`:

1. Prüfen, dass der Lauf auf `main` gestartet wurde und die CI für den letzten Commit grün ist. Sonst abbrechen.
2. Nächste Version aus letztem Tag und `bump` berechnen, z. B. `1.4.2` + `minor` → `1.5.0`.
3. `bump.mjs` ausführen und den `CHANGELOG.md`-Abschnitt „Unveröffentlicht“ in `## X.Y.Z – <Datum>` umbenennen.
4. Commit „Release vX.Y.Z“ und Tag `vX.Y.Z` erstellen und beides pushen. Ist `main` geschützt, ist dafür ein Token einer GitHub-App mit Push-Recht nötig.
5. Ein **Entwurfs-Release** `vX.Y.Z` anlegen. Text: CHANGELOG-Abschnitt plus automatisch erzeugte Release Notes.
6. Ausgabe: `version`, `tag`, `release_id` für die folgenden Jobs.

**Job 2 `build`** (Matrix, `needs: version`, checkt das Tag aus):

| Runner | Rust-Ziel | Ergebnis |
|---|---|---|
| `macos-14` | `aarch64-apple-darwin` | `Nexus_X.Y.Z_aarch64.dmg`, `.app.tar.gz` (+ `.sig`) |
| `macos-14` | `x86_64-apple-darwin` (Cross-Build) | `Nexus_X.Y.Z_x64.dmg`, `.app.tar.gz` (+ `.sig`) |
| `windows-latest` | `x86_64-pc-windows-msvc` | `Nexus_X.Y.Z_x64-setup.exe` (NSIS, + `.sig`) |
| `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `.AppImage` (+ `.sig`), `.deb`, `.rpm` |
| `ubuntu-22.04-arm` (optional) | `aarch64-unknown-linux-gnu` | wie oben für ARM |

Linux wird bewusst auf Ubuntu 22.04 gebaut, damit die Pakete auch auf älteren Distributionen laufen (ältere glibc).

Schritte je Eintrag:

1. Rust-Toolchain mit Ziel installieren und `Swatinem/rust-cache` nutzen.
2. Node für den Build der Oberfläche einrichten, mit Cache für npm.
3. Nur auf Linux die Pakete `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` und `patchelf` installieren.
4. `npm ci` und Oberfläche bauen. Bis Phase 5 zusätzlich den Node-Sidecar für das Ziel bauen.
5. `tauri-apps/tauri-action` mit `releaseId` aus Job 1. Damit werden die Installer angehängt und `latest.json` für den Updater erzeugt (`includeUpdaterJson: true`).
6. Das CLI-Binary `nexus` zusätzlich als Archiv `nexus-X.Y.Z-<ziel>.tar.gz` bzw. `.zip` mit Plugin anhängen (für Betrieb ohne App und für Server).
7. Prüfsummen (`SHA256SUMS`) anhängen.

**Signieren im Build:**

| Plattform | Wie | Secrets |
|---|---|---|
| macOS | Developer-ID-Signatur, Hardened Runtime, Notarisierung über tauri-action. Entitlements nur Netzwerk-Client/-Server (localhost) | `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_PATH` |
| Windows | Azure Trusted Signing über `bundle.windows.signCommand` | `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` |
| Linux | AppImage mit Updater-Signatur. `.deb`/`.rpm` unsigniert, auf Wunsch später mit GPG | – |
| Updater (alle) | Tauri-Updater-Schlüssel | `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` |

**Job 3 `publish`** (`needs: build`, nur wenn alle Builds grün sind):

1. Prüfen, dass alle erwarteten Dateien am Release hängen.
2. Entwurf veröffentlichen und als „Latest“ markieren.

Schlägt ein Build fehl, bleibt das Release ein Entwurf. Tag und Versions-Commit sind dann schon auf `main`. Man behebt den Fehler und startet nur die fehlgeschlagenen Jobs neu („Re-run failed jobs“). Ist das nicht möglich, löscht man Entwurf und Tag von Hand und startet den Workflow erneut mit `patch`.

#### Updates

`tauri-plugin-updater` zeigt auf `https://github.com/BenjaminBiber/Nexus/releases/latest/download/latest.json`. Die App prüft beim Start und dann täglich. Nur signierte Pakete werden installiert. Unter Linux aktualisiert sich nur das AppImage selbst, `.deb`/`.rpm` laufen über die Paketverwaltung.

#### Weitere Workflows

- `.github/workflows/ci.yml` läuft bei jedem Push und PR auf allen drei Plattformen: Build, Tests, Vertragstests und Messskripte.
- Bis Phase 5 laufen dort auch die TS-Tests.

#### Installer und CLI

- macOS: getrennte arm64- und x64-Pakete, weil ein Universal-Binary fast doppelt so groß ist.
- Windows: NSIS-Installer pro Benutzer (ohne Adminrechte), WebView2 per Bootstrapper.
- Linux: AppImage für alle Distributionen, `.deb` und `.rpm` für Debian/Ubuntu bzw. Fedora/openSUSE.
- Die App legt `nexus` beim ersten Start in den Benutzer-PATH und hält das Plugin aktuell.

*Fertig, wenn:* Ein Lauf mit `patch` erzeugt Tag, Release und alle Installer. Die Installer lassen sich auf allen drei Plattformen ohne Warnung installieren (Linux: ohne Signaturwarnung beim AppImage-Update). Eine installierte Vorversion aktualisiert sich selbst.

## 5. Teststrategie

- **Vertragstests** (`contract/`) sind der Maßstab für Gleichheit. Sie laufen in CI gegen TS (bis Phase 5) und Rust.
- **Unit-Tests** aus `packages/*/tests/unit` werden mit der jeweiligen Portierung als Rust-Tests übernommen (`#[cfg(test)]` bzw. `tests/`). Testnamen und Fälle bleiben gleich, damit die Abdeckung vergleichbar ist.
- **Prozess-Tests** (heute `vitest.process.config.ts`) werden Rust-Integrationstests mit eigenem `NEXUS_HOME` und freiem Port je Test. Die SQL-Server-Tests laufen nacheinander auf der gemeinsamen Test-DB.
- **Oberfläche:** Die bestehenden Tests in `packages/web/tests` bleiben. Neu kommen E2E-Tests der App mit `tauri-driver` (WebDriver) auf Windows und Linux. Auf macOS gibt es keinen WKWebView-Treiber, dort Playwright gegen die Weboberfläche plus manuelle Abnahme.
- **Leistung:** Die Messskripte aus Phase 0 laufen in CI. Ein Rückschritt über 10 % lässt den Build scheitern.

## 6. Risiken

| Risiko | Gegenmaßnahme |
|---|---|
| Die Oberfläche verhält sich in WKWebView (Safari-Engine) anders als in Chrome | Ab Phase 1 auf macOS testen. Auffällig sind meist CSS-Details, `EventSource`, Datei-Uploads, Zwischenablage |
| WebKitGTK unter Linux ist langsamer und je nach Distribution unterschiedlich alt | Ab Phase 1 auf Ubuntu 22.04 und Fedora testen. Das AppImage bringt eine feste Version mit |
| Tray unter Linux hängt von der Desktop-Umgebung ab (GNOME braucht eine AppIndicator-Erweiterung) | Tray nur als Zusatz. Alle Funktionen sind auch im Fenster erreichbar |
| Signatur-Secrets gelangen in falsche Hände | Secrets nur in einem GitHub-Environment `release` mit Freigabe durch eine zweite Person. Forks bekommen keine Secrets |
| `tiberius` deckt eine genutzte SQL-Server-Funktion nicht ab (z. B. Anmeldeart) | In Phase 2 zuerst Verbindungsaufbau und Anmeldung wie im Team prüfen (SQL-Login / Windows-Anmeldung) |
| Dispatch verhält sich unter Windows anders (Prozesse, Git Bash, lange Pfade) | Zuletzt portieren, Parallelbetrieb, Job-Objekte, Tests auf Windows-Agent |
| Umbenennung bricht laufende Projekte | Übergangszeit mit Alias, `nexus migrate`, alte Daten bleiben liegen |
| Zwei Implementierungen gleichzeitig pflegen | Neue Funktionen ab Phase 2 nur noch in Rust. TS bekommt nur noch Fehlerbehebungen |
| Plattformabhängige Binaries im Plugin | `build-plugin.mjs` legt je Plattform das passende Binary ab. Der Hook-Aufruf wählt es über einen kleinen Starter (`bin/nexus` als Shell-Skript bzw. `nexus.cmd`) |
| Gemeinsame `board.db` von TS und Rust gleichzeitig | Gleiche WAL-Einstellungen und Busy-Timeout. Gegenproben-Tests aus Phase 2 |

## 7. Aufwand und Reihenfolge

| Phase | Ergebnis | Aufwand (grob) |
|---|---|---|
| 0 | Messwerte, Vertrag | 5 PT |
| 1 | App für macOS, Windows, Linux (noch mit Node) | 8 PT |
| 2 | Rust-DB-Schicht | 15 PT |
| 3 | Rust-CLI und -Hooks, Umbenennung | 20 PT |
| 4 | Rust-Server | 35–45 PT |
| 5 | Node entfernt, kompakte App | 5 PT |
| 6 | Release-Workflow (patch/minor/major), signierte Installer, Updates | 8 PT |
| **Summe** | | **≈ 96–106 PT** |

Phase 1 und 6 können früh parallel laufen. Phasen 2–4 bauen aufeinander auf. Innerhalb von Phase 4 sind die Schritte 4.2 und 4.5–4.7 voneinander unabhängig.

## 8. Nächste Schritte

1. Plan abnehmen und offene Entscheidungen klären:
   - Name des Datenordners (`~/.nexus`)
   - Signatur-Zertifikate (Apple Developer Account, Azure Trusted Signing) als Secrets im Environment `release`
   - Linux-Ziele: nur x64 oder auch ARM
2. Phase 0 und Phase 1 als Tickets auf dem Board anlegen.
3. Nach Phase 1 die Messwerte vergleichen und über den Start von Phase 2 entscheiden.
