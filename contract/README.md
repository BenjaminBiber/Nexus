# Vertragstest von HTTP-API, CLI und Hooks

Die HTTP-API des Servers und das Verhalten von `harness` und seinen Hooks sind der Vertrag, den jede
Umsetzung erfüllen muss – heute der TypeScript-Server und die TypeScript-CLI, später beide in Rust.
Dieser Ordner enthält das Werkzeug dafür: Es startet einen Server mit frischem `HARNESS_HOME`, spielt
Anfragen und CLI-Aufrufe aus Szenario-Dateien ab, ersetzt die je Lauf wechselnden Werte durch Platzhalter
und vergleicht das Ergebnis mit einer Golden-Datei – oder zeichnet sie neu auf.

Es gibt drei Sammlungen von Szenarien: `api/` für einzelne Anfragen, `sse/` für den Ereignisstrom
(Server-Sent Events) eines Projekts und `cli/` für die Hooks, die `board`-Befehle und die Hilfetexte. Ohne
`--suite` laufen alle drei.

Alles hier ist plain Node (ESM, `.mjs`), ohne zusätzliche Abhängigkeiten und ohne eigenen Build-Schritt.
Nur der geprüfte Server und die geprüfte CLI müssen gebaut sein (`npm run build`).

## Aufrufe

```sh
npm run build                      # Voraussetzung: packages/*/dist und packages/web/dist
npm run contract                   # alles vergleichen (= node contract/run.mjs)
node contract/run.mjs --update     # Golden-Dateien neu aufzeichnen
node contract/run.mjs --only basis/projects
node contract/run.mjs --only sse   # nur die Szenarien des Ereignisstroms
node contract/run.mjs --coverage   # alle Routen mit abgedeckt/offen
node contract/run.mjs --suite cli             # Hooks, board-Befehle und Hilfetexte
node contract/run.mjs --suite cli --only cli/hooks
node contract/run.mjs --suite cli --only cli/board   # alle board-Unterbefehle
node contract/run.mjs --suite cli --only cli/board/boards-projects \
  --cli-cmd target/debug/nexus --init-cmd "node packages/cli/dist/main.js" \
  --step "board --no-open,board mit unbekanntem Unterbefehl,board mit kaputtem Port"
                                   # nur die schon portierten board-Fälle gegen das Rust-Binary
node contract/run.mjs --suite cli --only cli/board/dispatch \
  --cli-cmd target/debug/nexus --init-cmd "node packages/cli/dist/main.js" \
  --setup-cmd "node packages/cli/dist/main.js"
                                   # Aufbau mit der TS-CLI, geprüft wird das Rust-Binary
node contract/lib/gen-help-scenarios.mjs      # Szenario der Hilfetexte neu erzeugen
npx vitest run contract            # Einheitentests (Normalisierer, Szenario-Lader, Variablen,
                                   # Home-Dateien, Strom, CLI-Schritt, Commit, Home-Datei, Generator)
```

| Option | Bedeutung |
| --- | --- |
| `--update` | Golden-Dateien neu schreiben; nur Dateien mit geändertem Inhalt werden angefasst. |
| `--only <teilstring>` | Nur Szenarien, deren Pfad den Teilstring enthält (z. B. `basis/health`). |
| `--step <teilstrings>` | Nur Schritte, deren Name einen der kommagetrennten Teilstrings enthält; verglichen wird dann nur mit diesen Einträgen der Golden-Datei. Schritte mit `record: false` bauen nur den Zustand auf (Projekt, Board, Tickets) und laufen immer mit. Gedacht für eine Umsetzung, die erst einen Teil der Befehle kennt. Geht nicht mit `--update` zusammen. |
| `--suite <name>` | Ordner unter `contract/` mit den Szenarien (Standard: alle, also `api`, `sse` und `cli`). |
| `--server-cmd "<cmd {port}>"` | Eigener Startbefehl statt des TS-Servers; `{port}` wird ersetzt. Relative Pfade gelten ab der Projektwurzel. |
| `--base <url> --home <ordner>` | Gegen einen schon laufenden Server prüfen (siehe unten). |
| `--cli-cmd "<cmd>"` | Befehl der CLI für `harnessInit` und die `cli`-Schritte (Standard `node packages/cli/dist/main.js`). Auch als JSON-Array: `'["node","/Mein Ordner/main.js"]'` – der sichere Weg für Pfade mit Leerzeichen. Relative Pfade gelten ab der Projektwurzel. |
| `--init-cmd "<cmd>"` | Eigener Befehl nur für die Fixture `harnessInit` (Standard: `--cli-cmd`). Nötig, solange die geprüfte CLI noch kein `init` kennt. |
| `--setup-cmd "<cmd>"` | Eigener Befehl nur für `cli`-Schritte mit `record: false` (Standard: `--cli-cmd`). Diese Schritte bauen nur den Zustand auf; nötig, solange die geprüfte CLI die dafür benutzten Befehle noch nicht kennt. |
| `--coverage` | Routen aus `api/routes.json` auflisten, Exit 0. |
| `--strict` | Zusammen mit `--coverage`: Exit 1, wenn eine Route offen ist. |
| `--verbose` | Ausgabe des gestarteten Servers mitschreiben. |

Exit-Codes: `0` alles grün, `1` Abweichung, fehlende Golden-Datei oder offene Route bei `--coverage --strict`,
`2` Fehler im Aufruf (unbekannte Option, `--base` ohne `--home`).

## Ablauf

Jedes Szenario läuft gegen einen **frischen Server mit frischem Home**. Deshalb sind Auto-Inkrement-IDs und
Ticket-IDs (`BEN-0001`) deterministisch und müssen nicht normalisiert werden. Das gilt auch für die Suite
`cli`: Die Hooks melden sich am Server des Szenarios, nicht am echten unter Port 4710.

1. Szenarien unter `contract/<suite>/**` suchen, nach `--only` und `platforms` filtern.
2. Je Szenario: temporäres `HARNESS_HOME`, temporäre `CLAUDE_CONFIG_DIR`, freier Port, Fixture-Repos und
   Fixture-Dateien im Home (`homeFiles`) anlegen.
3. Server starten (Standard `node contract/servers/ts-server.mjs --port {port}`) und auf `GET /api/health`
   warten (höchstens 15 s).
4. Fixture `boardConfig` anwenden, dann die Schritte ausführen und normalisieren.
5. Server beenden (unter Windows mit `taskkill /T /F`), Home, Repos und Claude-Konfiguration löschen.
6. Vergleichen oder (mit `--update`) aufzeichnen; am Ende eine Zusammenfassung.

## Vertragsmodus des Servers

Jede Umsetzung des Servers muss diesen Modus anbieten; nur so sind die Antworten reproduzierbar.
Beim TypeScript-Server macht das `contract/servers/ts-server.mjs`.

- Version in `GET /api/health` ist `contract`.
- Kein Takt des Dispatchers von selbst, und es wird **nie** ein Prozess gestartet: der Starter des Workers
  liefert immer `null`. `POST /api/dispatch/run` löst den Durchlauf trotzdem aus – ein bereites Ticket wird
  gewählt und als Lauf vermerkt (`GET /api/dispatch/status` zeigt ihn mit `state: "running"`), nur arbeitet
  niemand daran. Der Lauf endet von selbst nie; der Zustand bleibt über alle weiteren Anfragen gleich. Ein
  Szenario kann das Ergebnis des Workers aber selbst hinlegen (Schritt-Art `homeFile`) – dann wertet der nächste
  Durchlauf den Lauf aus wie jeden anderen.
- Kein Ordnerdialog (`POST /api/db/pick-folder` antwortet mit `{ "path": null }`), kein Öffnen von Dateien im
  Explorer oder in VS Code.
- Keine Meldungen über neue Fragen (Abfrage aus).
- `GET /api/claude-usage` antwortet mit `available: false`, `reason: "no-login"` und sonst `null`.
- Claude-Aufrufe starten nie ein echtes Claude, sondern liefern feste Ergebnisse: Optimierungs-Vorschläge und
  Ticket-Entwürfe `[]`, das Umschreiben eines Dokument-Bereichs `Ersatztext`.
- `CONTRACT_HEARTBEAT_MS` steuert den Herzschlag des Ereignisstroms, `CONTRACT_POLL_MS` die Abfrage-Takte
  (Board-Änderungen, Dokumente).
- Daten liegen ausschließlich unter `HARNESS_HOME`; der echte `~/.harness` und Port 4710 werden nie benutzt.

Der Server wird mit `HARNESS_HOME`, `HARNESS_PORT`, `CLAUDE_CONFIG_DIR` und den `CONTRACT_*`-Variablen in der
Umgebung gestartet und hört auf `127.0.0.1`. Er schreibt `server.json` wie der normale Start und beendet sich
bei `SIGTERM`/`SIGINT` sauber.

### Rust vorn, Node dahinter

Im Betrieb nimmt der Rust-Server den Port an und hält den Node-Server als Kindprozess dahinter. Genau diesen
Weg prüft der ganze Vertrag mit:

```sh
npm run build && cargo build -p nexus-cli --bin nexus
node contract/run.mjs --server-cmd "node scripts/contract/rust-vor-node.mjs --port {port}"
```

Das Skript startet `nexus serve` mit dem Vertragsserver als Sidecar. Dafür kennt `contract/servers/ts-server.mjs`
denselben Sidecar-Modus wie der echte Node-Server: Mit `HARNESS_SIDECAR=1` ist Port 0 erlaubt (freier Port), der
gewählte Port geht als Zeile `HARNESS_SIDECAR_PORT=<port>` auf stdout, es entsteht keine `server.json` (die gehört
dem Rust-Prozess), Links in Meldungen zeigen auf `HARNESS_PUBLIC_PORT`, und ein Ende der stdin beendet ihn.

## Szenario-Format (`*.scenario.json`)

```json
{
  "title": "Projekte anmelden und auflisten",
  "platforms": ["darwin", "linux", "win32"],
  "fixtures": {
    "repos": [{ "name": "REPO1", "git": true, "harnessInit": false, "files": { "docs/a.md": "# A\n" } }],
    "boardConfig": "sqlite",
    "homeFiles": { "dispatch/state.json": { "paused": false }, "dispatch-logs/a.log": "eine Zeile\n" }
  },
  "steps": [
    { "name": "anmelden", "method": "POST", "path": "/api/projects", "body": { "path": "{{REPO1}}" },
      "save": { "pid": "json.id" } },
    { "name": "liste", "method": "GET", "path": "/api/projects" },
    { "name": "ohne Pflichtkopf", "method": "POST", "path": "/api/projects", "body": {},
      "headers": { "x-harness": null } }
  ]
}
```

- `title` ist Pflicht, `platforms`, `env` und `fixtures` sind freiwillig. Ohne `platforms` läuft das Szenario
  überall.
- Unbekannte Felder sind ein Fehler – Tippfehler fallen so beim Laden auf, nicht erst im Vergleich.
  Welche Felder ein Schritt haben darf, hängt von seiner `kind` ab (siehe „Schritte“).
- Die Golden-Datei liegt daneben und heißt wie das Szenario: `projects.scenario.json` → `projects.golden.json`.

### Umgebung

`env` setzt Umgebungsvariablen des Servers für dieses Szenario, erlaubt sind nur die `CONTRACT_*` des
Vertragsmodus. Szenarien mit Ereignisstrom drücken damit den Takt, damit sie in Sekunden fertig sind:

```json
"env": { "CONTRACT_HEARTBEAT_MS": "400", "CONTRACT_POLL_MS": "200" }
```

### Fixtures

- `repos`: temporäre Git-Repos. `name` in Großbuchstaben (`REPO1`), danach als `{{REPO1}}` verwendbar.
  `git` (Standard `true`) legt ein Repo mit erstem Commit an, `files` schreibt Dateien (Pfade mit `/`),
  `harnessInit` führt `<cli> init` im Repo aus (nötig, damit sich das Projekt anmelden lässt).
  Benutzer und Zeitpunkt des ersten Commits stehen fest (`Vertrag <vertrag@example.invalid>`, `2020-03-02`),
  deshalb ist sein Hash in jedem Lauf derselbe. Das zählt, sobald ein Befehl ihn ausgibt – `harness board
  merge` meldet „… gemergt in main (6f07eff)“.
  Der Ordner heißt `<temp>/<NAME>/Mein Projekt ä` – Leerzeichen und Umlaut gehören zum Test.
  `branches` legt Arbeitsbranches an, die vor dem Hauptbranch `main` stehen – je Branch `name` (nicht `main`),
  `files` und `message` (Standard `Arbeit auf <name>`); danach steht das Repo wieder auf `main`.
  Damit bekommt ein Szenario einen Zielbranch (`release/1`) oder einen fremden Stand, gegen den gemergt wird.

  ```json
  { "name": "REPO1", "files": { "src/a.ts": "alt\n" },
    "branches": [{ "name": "release/1", "message": "1.0", "files": { "src/a.ts": "release\n" } }] }
  ```

  Jeder Commit eines Szenarios – der der Fixtures ebenso wie der Merge-Commit des geprüften Servers – bekommt
  dieselbe Identität, dasselbe Datum und dieselben Git-Einstellungen (`lib/GitIdentity.mjs`). Deshalb sind die
  Commit-Kennungen über Läufe und Rechner hinweg gleich und stehen im Klartext in den Golden-Dateien.
  `{{REPO1}}` ist der aufgelöste Pfad (unter macOS `/private/var/…`, nicht `/var/…`), also genau der Text, den
  der Server als Projektwurzel führt. Nur so findet ein Board, dessen `repoPath` auf `{{REPO1}}` zeigt, sein
  Projekt wieder – der Vergleich in `RepoBoardResolver` ist ein reiner Textvergleich.
- `boardConfig: "sqlite"`: vor den Schritten `PUT /api/board-config` mit
  `{ provider: "sqlite", dbFile: "<HOME>/board.db", userId: "11111111-1111-1111-1111-111111111111", userName: "Vertrag" }`.
  Dieser Aufruf wird **nicht** aufgezeichnet.
- `claudeEnvFile: true`: setzt `CLAUDE_ENV_FILE` auf `<CLAUDE>/claude-env.sh` und zeichnet den Inhalt dieser
  Datei nach jedem `cli`-Schritt als `response.envFile` auf. `session-start` schreibt dort `HARNESS_SESSION`
  und `HARNESS_TICKET` (`EnvFileWriter` in `packages/core/src/hooks/`). Ohne die Fixture gibt es kein
  `CLAUDE_ENV_FILE` und kein `envFile` in der Golden-Datei.
- `homeFiles`: Dateien, die **vor dem Start des Servers** im `HARNESS_HOME` liegen (`lib/HomeFiles.mjs`).
  Schlüssel ist der Pfad relativ zum Home, immer mit `/` getrennt und ohne `..`; fehlende Ordner entstehen
  mit. Ein Text wird so geschrieben, wie er dasteht, jeder andere JSON-Wert als JSON mit zwei Leerzeichen
  Einrückung. Variablen (`{{HOME}}`, `{{NOW}}`, Repo-Namen) werden eingesetzt.

  Damit bekommt ein Szenario einen Zustand, den der Server sonst erst durch echte Arbeit hätte: Zustand des
  Dispatchers (`dispatch/state.json`), Läufe (`dispatch/runs/…`), vorgemerkte Zeilen für die Auswertung
  (`dispatch/runs/stats/…`), Lauf-Protokolle (`dispatch-logs/…`) und die Einstellung (`dispatch.yaml`).
  Diese Dateien sind Teil des Vertrags – jede Umsetzung des Servers muss sie lesen können.

### Variablen

`{{NAME}}` wird in `path`, `body`, `headers`, in `args`, `cwd`, `stdin` und `env` eines `cli`-Schritts und in
den Inhalten von `homeFiles` eingesetzt, auch verschachtelt. Bekannt sind:

- die Repo-Namen der Fixtures (`{{REPO1}}` → Pfad des Repos),
- `{{HOME}}` (das `HARNESS_HOME` des Laufs) und `{{PORT}}`,
- `{{NOW}}` (ISO-Zeit beim Start des Szenarios; für Daten, die in einem Zeitfenster liegen müssen, etwa
  Läufe in der Auswertung der Vorschläge – der Normalisierer macht daraus `<TIME>`),
- alle mit `save` gemerkten Werte.

Besteht ein Wert nur aus einem Platzhalter (`"{{PORT}}"`), behält er seinen Typ; sonst wird er in den Text
eingesetzt. Ein unbekannter Name ist ein Fehler.

`{{!name}}` ist ein **geschützter** Platzhalter: Er wird nicht eingesetzt, sondern zu `{{name}}`. So kommen
Texte durch, die selbst Platzhalter enthalten – etwa die Vorlagen der Prompts des Dispatchers
(`{{!agent}}`, `{{!ticketId}}`).

`save` liest Werte aus der Antwort über Punkt-Notation, beginnend mit `json.` oder `headers.`
(`"save": { "pid": "json.id", "ort": "headers.location" }`). Listen sprechen Indizes an: `json.items.0.id`.

### Schritte

Jeder Schritt hat einen eindeutigen `name`. `kind` wählt die Art (Standard `http`); bekannt sind `http`,
`cli`, `httpStub`, `sseOpen`, `sseExpect`, `sseClose`, `commit` und `homeFile`, eine unbekannte Art ist ein
Fehler. Diese Felder gelten für jede Art:

| Feld | Bedeutung |
| --- | --- |
| `save` | Werte für späteren Gebrauch merken. |
| `compare` | `full` (Standard), `status` oder `shape`. |
| `ignore` | Felder, die vor dem Vergleich entfernt werden (`"json.a.b"`, relativ zur Antwort). |
| `record` | `false` führt den Schritt aus, zeichnet ihn aber nicht auf (Vorbereitung). |

Dazu die eigenen Felder der Art `http`:

| Feld | Bedeutung |
| --- | --- |
| `method` | `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`. |
| `path` | Pfad ab `/`, mit Abfrageteil erlaubt. |
| `body` | Körper als JSON; bei `GET`/`HEAD` wird er nur aufgezeichnet, nicht gesendet. |
| `headers` | Zusätzliche Köpfe. Wert `null` lässt einen Standardkopf weg (für Tests des Schutzes). |

Standardköpfe bei allem außer `GET`/`HEAD`: `x-harness: 1` und `content-type: application/json`.
Der `Host`-Kopf ist immer `127.0.0.1:<port>`, weil die Anfragen an diese Adresse gehen (`localhost` würde der
`RequestGuard` nur dann annehmen, wenn er auch so angesprochen wird). `fetch` setzt keinen `origin`; für den
Test der fremden Herkunft steht er im Szenario.

`compare`-Arten:

- `full`: Status, Inhaltstyp und Körper genau vergleichen.
- `status`: nur den Status; Inhaltstyp und Körper fallen aus der Aufzeichnung.
- `shape`: Status und Inhaltstyp genau, der Körper nur als Schlüssel und JSON-Typen (`"<string>"`, `"<number>"`,
  `"<boolean>"`, `"<null>"`). Listen werden auf ihre verschiedenen Formen zusammengefasst, damit die Länge
  nichts ausmacht; ein Text-Körper wird zu `"<string>"`.

### Schritt-Art `cli`

Ruft die CLI als eigenen Prozess auf und zeichnet Rückgabecode, Standardausgabe und Standardfehler auf
(`lib/CliStep.mjs`).

```json
{ "name": "session-start", "kind": "cli", "args": ["hook", "session-start"], "cwd": "{{REPO1}}",
  "stdin": { "session_id": "s1", "cwd": "{{REPO1}}", "source": "startup" },
  "env": { "HARNESS_TICKET": "ARC-0001", "HARNESS_HOOK_DEADLINE_MS": "60000" } }
```

| Feld | Bedeutung |
| --- | --- |
| `args` | Argumente nach dem Programm, nicht leer. `{{NAME}}` wird eingesetzt. |
| `cwd` | Arbeitsordner; ohne Angabe das `HARNESS_HOME` des Laufs. |
| `stdin` | Objekt → als JSON hinein, Text → roh (so kommen kaputtes JSON und leere Eingabe in den Vertrag). |
| `env` | Zusätzliche Umgebungsvariablen; sie gewinnen gegen die des Rahmens. |

Das Programm kommt aus `--cli-cmd` und wird in Programm und Argumente zerlegt – **ohne Shell**, damit
Leerzeichen in Pfaden sicher sind. Die Umgebung ist die geerbte, aber:

- **gesetzt**: `HARNESS_HOME` (Home des Laufs), `HARNESS_PORT` (Port des Szenario-Servers),
  `CLAUDE_CONFIG_DIR` (leerer temporärer Ordner – der echte `~/.claude` wird nie angefasst),
  `NO_COLOR=1`, `LANG=C.UTF-8`;
- **entfernt**: `HARNESS_SESSION`, `HARNESS_TICKET`, `CLAUDE_ENV_FILE` – eine geerbte Zuordnung gilt nicht
  für den Vertrag. Wer sie braucht, setzt sie im `env` des Schritts.

Der Szenario-Server läuft schon auf `HARNESS_PORT`; `session-start` erkennt ihn und startet keinen zweiten
(`probe` in `packages/cli/src/hooks/HookRunner.ts`). `HARNESS_SERVER_ENTRY` wird deshalb nicht gesetzt.

Aufgezeichnet wird `{ code, stdout | json, stderr }`, dazu `envFile` mit der Fixture `claudeEnvFile`.
Ist die Standardausgabe ein JSON-Objekt oder eine JSON-Liste, steht sie als `json` statt als `stdout` –
die Hooks antworten mit JSON, die Hilfetexte mit Text. Zusätzlich zur Normalisierung der HTTP-Antworten
werden Zeilenenden `\r\n` zu `\n` und abschließende Leerzeilen weggelassen. `request.env` steht nur dort,
wo das Szenario Variablen nennt; die des Rahmens stehen hier im README und nicht in jeder Golden-Datei.

### Schritt-Art `httpStub`: Attrappe eines fremden Dienstes

Manche Routen rufen einen Dienst außerhalb des Rechners auf – `POST /api/user-settings/test-ntfy` schickt eine
Push-Meldung an einen ntfy-Server. Diese Art startet dafür einen Mini-HTTP-Server auf einem freien Port, der
jede Anfrage mit einem festen Status beantwortet und aufzeichnet, was ankam.

| Feld | Bedeutung |
| --- | --- |
| `stub` | Name der Attrappe in Großbuchstaben (`NTFY`), Pflicht. |
| `action` | `start` (Standard) startet sie, `requests` zeichnet die empfangenen Anfragen auf. |
| `status` | Antwortstatus der Attrappe, Standard `200` (nur bei `start`). |

```json
{ "name": "Attrappe starten", "kind": "httpStub", "stub": "NTFY", "save": { "ntfyUrl": "json.url" } },
{ "name": "einrichten", "method": "PUT", "path": "/api/user-settings",
  "body": { "ntfy": true, "ntfyServer": "{{ntfyUrl}}", "ntfyTopic": "harness-vertrag" } },
{ "name": "Push-Test", "method": "POST", "path": "/api/user-settings/test-ntfy", "body": {} },
{ "name": "Anfrage an der Attrappe", "kind": "httpStub", "action": "requests", "stub": "NTFY" }
```

- `start` liefert die Adresse als `json.url`; mit `save` wird sie zur Variable und – weil sie den wechselnden
  Port enthält – in der Golden-Datei zu `<var:name>`.
- `requests` liefert je Anfrage Methode, Pfad, die Köpfe `content-type` und `authorization` und den Körper als
  `json` oder `text`. Andere Köpfe hängen am HTTP-Client und gehören nicht zum Vertrag.
- Die Attrappen leben nur in ihrem Szenario; der Runner beendet sie danach.

### Schritt-Art `commit`: Arbeit im Repo

Der Vertragsmodus startet nie einen Worker. Der Branch, den der Dispatcher für ein Ticket anlegt, bliebe deshalb
leer – und ohne Änderungen hätten Review (Dateiliste, Patch) und Merge (Merge-Commit, Konflikt) nichts zu zeigen.
Diese Art schreibt und löscht Dateien in einem Ordner der Fixture-Repos und committet sie, so wie es der Agent
getan hätte. Committet wird nur innerhalb der Fixture-Repos.

| Feld | Bedeutung |
| --- | --- |
| `cwd` | Ordner des Commits, Pflicht; auch ein Worktree (`{{REPO1}}-worktrees/BEN-0001`). |
| `files` | Neue oder geänderte Dateien als Pfad → Text (Pfade mit `/`). |
| `remove` | Zu löschende Dateien; mit `files` zusammen ergibt das eine Umbenennung. |
| `message` | Commit-Nachricht, Pflicht. |

```json
{ "name": "Arbeit des Agenten", "kind": "commit", "cwd": "{{REPO1}}-worktrees/BEN-0001",
  "files": { "src/preis.ts": "neu\n" }, "remove": ["src/alt.ts"], "message": "BEN-0001: Rabatt" }
```

Aufgenommen werden nur die genannten Pfade – ein frischer Worktree enthält auch Dateien, die der Harness beim
Anlegen hineinschreibt (Plugin-Einstellung, Rules), und die gehören nicht in den Commit. Die Antwort nennt
`branch` und `commit` (kurze Kennung); beide sind wegen der festen Identität über Läufe hinweg gleich.

### Schritt-Art `homeFile`: Datei im Home während des Laufs

Die Fixture `homeFiles` legt Dateien vor dem Start des Servers an. Diese Art schreibt eine Datei, während das
Szenario läuft – für die, deren Name erst im Lauf feststeht. Der Hauptfall ist das Ergebnis eines Workers
(`dispatch/runs/<Lauf>.result.json`): die Lauf-ID vergibt erst `POST /api/dispatch/run`, und mit dem Ergebnis
daneben wertet der nächste Durchlauf den Lauf aus, als wäre der Worker fertig geworden. So kommt ein Szenario an
die Zustände nach einem beendeten Lauf (Freigabe vor dem Merge, Merge, Abbruch), ohne je einen Worker zu starten.

| Feld | Bedeutung |
| --- | --- |
| `path` | Pfad relativ zum Home, mit `/` getrennt und ohne `..`; Pflicht. |
| `content` | Inhalt: ein Text kommt wie er ist, jeder andere JSON-Wert als JSON. Pflicht. |

```json
{ "name": "Durchlauf", "method": "POST", "path": "/api/dispatch/run", "body": {},
  "save": { "lauf": "json.runs.0.id" } },
{ "name": "Ergebnis des Workers", "kind": "homeFile", "path": "dispatch/runs/{{lauf}}.result.json",
  "content": { "code": 0, "timedOut": false, "endedAt": "2020-03-02T08:45:00.000Z", "output": "…" } }
```

## Ereignisstrom (`sseOpen`, `sseExpect`, `sseClose`)

`GET /api/projects/:pid/events` liefert einen Ereignisstrom (Server-Sent Events): beim Verbinden
`{"type":"hello"}`, danach Änderungsmeldungen (`sessions`, `dbboard`, `dbquestions`, `docs-changed`,
`notice`) und im Takt `heartbeatMs` einen Herzschlag als Kommentarzeile `: ping`. Welche Meldungen ein Strom
bekommt, hängt am Abfrageteil: `?dbBoard=<id>` für `dbboard`, `?dbProject=<id>` für `dbquestions`,
`?notify=1` für `notice`.

Drei Schritt-Arten halten das als Vertrag fest. Ein Strom lebt nur innerhalb seines Szenarios und heißt dort
so, wie `as` ihn nennt:

```json
{ "name": "Strom öffnen", "kind": "sseOpen", "as": "s1",
  "path": "/api/projects/{{pid}}/events?dbBoard={{board}}&notify=1" },
{ "name": "Begrüßung", "kind": "sseExpect", "stream": "s1", "until": { "type": "hello" } },
{ "name": "Ticket ändern", "method": "PATCH", "path": "/api/db/tickets/ARC-0001",
  "body": { "title": "Anmeldung", "rev": "{{rev}}" }, "compare": "status" },
{ "name": "dbboard", "kind": "sseExpect", "stream": "s1", "until": { "type": "dbboard" }, "timeoutMs": 6000 },
{ "name": "Herzschlag", "kind": "sseExpect", "stream": "s1", "until": { "comment": "ping" } },
{ "name": "Strom schließen", "kind": "sseClose", "stream": "s1" }
```

| Art | Felder | Aufgezeichnet |
| --- | --- | --- |
| `sseOpen` | `as` (Name), `path` | `request`: `method`, `path`; `response`: `status`, `contentType`, `cacheControl`. |
| `sseExpect` | `stream`, `until`, `timeoutMs` (Standard 3000) | `request`: `stream`, `until`; `response`: `events`. |
| `sseClose` | `stream` | `request`: `stream`; `response`: `closed`. |

- `sseOpen` wartet nur auf die Köpfe der Antwort, nicht auf das erste Ereignis. Antwortet der Server nicht mit
  `text/event-stream` – etwa 404 bei einem unbekannten Projekt –, ist das ein ganz normaler Eintrag mit Status
  und Körper, und unter diesem Namen gibt es keinen Strom.
- `sseExpect` gibt alle Ereignisse heraus, die seit dem letzten `sseExpect` **dieses** Stroms angekommen sind,
  bis einschließlich des ersten Treffers von `until`. `until` wird als Teilobjekt verglichen: jedes genannte
  Feld muss gleich sein, weitere Felder des Ereignisses spielen keine Rolle (`{ "type": "dbboard" }` passt auf
  `{ "type": "dbboard", "boardId": 1 }`). Ohne Treffer innerhalb von `timeoutMs` scheitert der Schritt und
  nennt alles, was bisher angekommen ist.
- Herzschläge stehen als `{ "comment": "ping" }` im Strom. Wie viele davon ankommen, hängt am Takt, deshalb
  zeichnet `sseExpect` sie **nur** auf, wenn `until` selbst ein Herzschlag ist; sonst fallen sie aus der Liste.
- Aufeinanderfolgende gleiche Ereignisse werden zu einem zusammengefasst. Zwei Änderungen in einem Takt der
  Abfrage melden `dbboard` einmal oder zweimal – beides ergibt dieselbe Golden-Datei.
- `sseClose` bricht die Verbindung ab und gibt den Namen wieder frei. Am Ende jedes Szenarios werden alle noch
  offenen Ströme abgebrochen; ein offener Strom hielte sonst `run.mjs` am Leben. Das Beenden des Servers ist
  für einen Strom kein Fehler.

Zwei Muster halten die Aufzeichnung unabhängig vom Takt:

- **Takt abwarten.** Vor einem Auslöser ein `sseExpect` auf `{ "comment": "ping" }`: der Strom ist danach
  leer, und die Beobachter des Servers (Board, Dokumente) hatten Zeit, ihren Ausgangswert zu lesen.
- **Dass ein Strom nichts bekommt**, zeigt ein `sseExpect` auf den nächsten Herzschlag *nach* dem Auslöser.
  Aufgezeichnet wird dann nur `{ "comment": "ping" }` – käme die Meldung doch an, stünde sie davor.

Dass der Server einen Empfänger beim Schließen wirklich abmeldet, ist von außen nicht zu sehen (und in Rust
nichts, was der Vertrag prüfen könnte). Die Szenarien prüfen stattdessen das, was man sieht: nach einem
`sseClose` wird ein Ereignis ausgelöst, ein neuer Strom geöffnet – er bekommt seine Begrüßung, holt das
Verpasste **nicht** nach und erhält die folgenden Ereignisse wieder.

### Szenarien unter `sse/`

| Szenario | Auslöser |
| --- | --- |
| `hello-heartbeat` | Verbinden: `hello`, Herzschlag, zweiter Strom auf dasselbe Projekt, 404 für ein unbekanntes Projekt. |
| `sessions` | `POST /api/hook` mit dem Token aus `GET /api/projects/:pid/settings`; das zweite Projekt bekommt nichts. |
| `notice` | `POST /api/user-settings/test-notification`; nur Ströme mit `?notify=1` bekommen `notice`. |
| `dbboard` | Ticket anlegen und ändern; der Strom ohne `?dbBoard` bekommt nichts. |
| `dbquestions` | Frage stellen; der Strom ohne `?dbProject` bekommt nichts. |
| `docs-changed` | `PUT /api/projects/:pid/docs/section` im Repo des Boards. |

Die Testmeldung geht nur an einen Strom mit `?notify=1`, solange einer offen ist; ohne einen solchen Strom
würde der Server den Weg des Betriebssystems nehmen (Windows-Toast). Deshalb löst `notice` eine Testmeldung
immer nur bei offenem Strom aus.

## Golden-Format (`*.golden.json`)

```json
[
  { "name": "anmelden",
    "request": { "method": "POST", "path": "/api/projects", "body": { "path": "<REPO1>" } },
    "response": { "status": 201, "contentType": "application/json",
                  "json": { "id": "<var:pid>", "path": "<REPO1>" } } }
]
```

Ein `cli`-Schritt sieht so aus:

```json
[
  { "name": "session-start",
    "request": { "args": ["hook", "session-start"], "cwd": "<REPO1>",
                 "env": { "HARNESS_HOOK_DEADLINE_MS": "60000" }, "stdin": { "cwd": "<REPO1>", "session_id": "s1" } },
    "response": { "code": 0, "envFile": "export HARNESS_SESSION='s1'\n",
                  "json": { "hookSpecificOutput": { "hookEventName": "SessionStart" } }, "stderr": "" } }
]
```

- Eine Liste in der Reihenfolge der aufgezeichneten Schritte, je Schritt `name`, `request` und `response`.
- `request.headers` steht nur dort, wo das Szenario Köpfe nennt; `null` zeigt einen bewusst fehlenden Kopf.
- `contentType` ohne Parameter (`; charset=…` entfällt), `null` wenn die Antwort keinen hat.
- Der Körper steht als `json`, sonst als `text` (Zeilenenden auf `\n`); ein leerer Körper ist `"text": ""`.
- Objekt-Schlüssel sind sortiert (stabile Diffs), Listen behalten ihre Reihenfolge.
  2 Leerzeichen Einrückung, `\n` am Ende der Datei.

Abweichungen melden Szenario, Schritt und einen zeilenweisen Diff (`-` Golden-Datei, `+` aufgezeichnet).
Fehlt die Golden-Datei, scheitert der Lauf mit dem Hinweis auf `--update`.

## Normalisierung

Alle Textwerte – auch Objekt-Schlüssel – gehen durch diese Regeln, in dieser Reihenfolge:

1. **Gespeicherte Werte** aus `save` (nur Texte ab 6 Zeichen) → `<var:name>`.
2. **Pfade**: Repo-Ordner → `<REPO1>` …, `HARNESS_HOME` → `<HOME>`, `CLAUDE_CONFIG_DIR` → `<CLAUDE>`,
   der Ordner der Fixture-Repos → `<REPOS>`, der Temp-Ordner → `<TMP>`. Jeder Pfad in drei Schreibweisen
   (nativ, mit `/`, JSON-escaped mit `\\`), der längste Pfad zuerst; unter macOS zählt auch die aufgelöste
   Form (`/private/var/…` ↔ `/var/…`). Der Rest des Pfades hinter dem Platzhalter bekommt Schrägstriche,
   damit die Golden-Datei unter Windows dieselbe ist.
3. **Port** des Servers: `127.0.0.1:<port>` und `localhost:<port>` → `…:<PORT>`.
4. **Zeit**: ISO-Zeitstempel → `<TIME>`, ebenso Datum mit Uhrzeit in deutscher Schreibweise
   (`27.10.2026, 18:05`) – so gibt die CLI Kommentare und Verlauf aus. Ein Datum `YYYY-MM-DD` nur dann →
   `<DATE>`, wenn es das heutige ist.
5. **Datum relativ zu heute**: ein Datum bis zu einem Jahr vor heute → `<DATE-55>` (die Zahl ist der Abstand in
   Tagen). Das trifft Grenzen, die der Server aus „heute“ rechnet – etwa der Anfang des Standard-Zeitraums der
   Dispatch-Statistik (acht Wochen). Feste Daten eines Szenarios liegen deshalb weiter zurück (`2020-03-02`);
   sonst hinge die Golden-Datei am Tag der Aufzeichnung.
6. **UUIDs** → `<UUID>`, außer `11111111-1111-1111-1111-111111111111` (der Vertrags-Benutzer).
7. **Git-Commits** in Feldern `sha`, `commit`, `head` (40 bzw. 7–12 Hex-Zeichen) → `<SHA>`.
8. **Zahlenfelder** mit Namen auf `Ms` oder `At` sowie `durationMs`, `pid` und `port` → `0`. Das trifft auch
   feste Werte wie den Standard-Port 1433 der Board-Konfiguration; dafür bleibt die Datei über Läufe gleich.

Jede Regel ist eine eigene Methode in `lib/Normalizer.mjs` und einzeln getestet
(`tests/Normalizer.test.mjs`).

## Routenliste und Abdeckung

`api/routes.json` enthält alle Routen des Servers (Methode, Muster, Quelldatei) und wird aus
`packages/server/src/routes/*.ts` erzeugt:

```sh
node contract/lib/extract-routes.mjs
```

Der Rückfall der Oberfläche hat kein Muster im Quellcode (`app.setNotFoundHandler`); er steht als `GET /*`
in der Liste und deckt alles ab, was `StaticRoutes.ts` neben `/` beantwortet: Seiten der Oberfläche, Dateien
unter `assets/` und unbekannte Adressen.

`--coverage` liest die Golden-Dateien (es läuft kein Server), ordnet jeden aufgezeichneten Schritt über
Methode und Pfad einem Muster zu (`:param` → ein Pfadabschnitt, `*` → Rest des Pfades, Abfrageteil
abgeschnitten) und gibt je Quelldatei eine Tabelle aus: `[x]` abgedeckt, `[ ]` offen. Ein Muster mit `*`
zählt erst, wenn keine genauere Route passt. Schritte ohne passende Route stehen am Ende mit `?` – dann ist
`routes.json` veraltet. Mit `--strict` endet der Lauf mit Exit 1, solange eine Route offen ist (gedacht für
den Abschluss von Phase 0).

`--coverage` gilt nur für die Suite `api`; die Suite `cli` kennt keine Routenliste.

## Suite `cli`: Hooks, Board-Befehle und Hilfetexte

```sh
node contract/run.mjs --suite cli --only cli/hooks
node contract/run.mjs --suite cli --only cli/board
node contract/run.mjs --suite cli --only cli/help
```

### Hooks (`cli/hooks/`)

Je Hook-Ereignis ein Szenario: Fixture-Repo mit `harness init`, SQLite-Board, Board dem Repo zugeordnet,
ein Ticket in Arbeit mit einem ungelesenen Kommentar. Das Szenario ruft den Hook auf und liest danach
`GET /api/projects/<pid>/sessions` – so steht sowohl die Ausgabe an Claude Code als auch die Wirkung auf
den Server im Vertrag.

| Szenario | Was es festhält |
| --- | --- |
| `session-start` | Leitfaden und Kommentar als `additionalContext`, `HARNESS_SESSION` in der Umgebungsdatei, Session am Server. Beim zweiten Aufruf fehlt der Kommentar: er gilt als gelesen. |
| `user-prompt` | Kommentar genau einmal, Session-Name aus dem Prompt (`SE-001: …`). |
| `notification`, `stop`, `subagent-start`, `subagent-stop`, `session-end` | still (kein `additionalContext`), aber gemeldet: Zustand, Subagenten und Ende der Session. |
| `stille-eingaben` | kaputtes JSON, leere Eingabe, `[]`, `null`, fehlende und nicht-textuelle Sitzungskennung, unbekanntes Ereignis – alle Exit 0, keine Ausgabe, keine Session. |
| `ohne-board` | Ohne `board.yaml` gibt es keinen Kontext, die Session wird trotzdem gemeldet. Im Repo ohne `harness init` passiert nichts. |
| `zuordnung` | Ohne `HARNESS_TICKET` erst „keinem Ticket zugeordnet“, dann über die verknüpfte Session; ein unbekanntes `HARNESS_TICKET` fällt darauf zurück. |
| `worktree` | Hook aus `<REPO1>-worktrees/ARC-0001`: das Projekt bleibt das Hauptrepo (gemeinsames Git-Verzeichnis), das Ticket kommt aus dem Ordner. |

`HARNESS_HOOK_DEADLINE_MS` steht in jedem Szenario großzügig (60 s). Die Frist selbst ist **kein**
Vertragsbestandteil – sie ist nur hoch, damit ein langsamer CI-Rechner den Hook nicht abbricht.

### Board-Befehle (`cli/board/`)

Agenten und Skills arbeiten über `harness board …`; deren Ausgaben und Fehlermeldungen sind Vertrag. Jedes
Szenario legt sich seinen Stand selbst an (Projekt, Board, Tickets – als `http`-Schritte mit `record: false`)
und ruft danach die Unterbefehle auf: jeden mit mindestens einem Erfolgsfall je Ausgabeart, die er anbietet
(Text, `--md`, `--json`), und mindestens einem Fehlerfall. Ein Fehler meldet Exit-Code 1 und schreibt den
Text samt `[code]` auf die Standardfehlerausgabe; mit `--json` steht stattdessen
`{"error":{"code","message"}}` auf der Standardausgabe.

Die Fixture ist fast überall dieselbe: ein Repo mit `harnessInit: true`, `boardConfig: "sqlite"` und ein
Board, dessen `repoPath` auf `{{REPO1}}` zeigt – nur so findet ein Befehl aus dem Repo heraus „sein“ Board.
`boards-projects` braucht ein zweites Repo für den Repo-Pfad eines fremden Boards, `spec-stop` kommt ohne
Board-Datenbank aus und braucht drei Repos (Kommentare, beschädigte Kommentardatei, nicht eingerichtet).

| Szenario | Unterbefehle | Quelle unter `packages/cli/src/commands/` |
| --- | --- | --- |
| `tickets-read` | `list`, `show` (mit `--md`, `--json`, `--field`, `--history`, `--all-comments`, Filtern) | `board/BoardTicketReadCommands.ts` |
| `tickets-write` | `create`, `set`, `move`, `delete` (Labels, Voraussetzungen, `--rev`) | `board/BoardTicketWriteCommands.ts` |
| `steps` | `step <id> <nummer>`, `step done`, `step add`, `step edit`, `step rm`, `step move` | `board/BoardStepCommands.ts` |
| `comments` | `comment` | `board/BoardCommentCommand.ts` |
| `notes` | `note add`, `note seen`, `note list`, `note show`, `note take`, `note drop` | `board/BoardNoteCommands.ts` |
| `inbox` | `inbox` | `board/BoardInboxCommands.ts` |
| `planning` | `plan`, `plan-json`, `next-planning` | `board/BoardPlanningCommands.ts` |
| `boards-projects` | `project list/create/rename`, `boards list/create/rename/use/repo/delete`, `board --no-open` | `board/BoardProjectCommands.ts`, `board/BoardBoardsCommands.ts`, `BoardCommand.ts` |
| `questions` | `question add/research/comment/list/show/link/depends/answer/reopen/discard` | `board/BoardQuestionCommands.ts` |
| `claim-work` | `claim`, `release`, `start`, `done`, `merge`, `accept` | `board/BoardClaimCommands.ts`, `board/BoardWorkCommands.ts`, `board/BoardDispatchCommands.ts` |
| `dispatch` | `dispatch status/run/pause/resume/enable/disable`, `dispatch-runs backfill`, `plan-start`, `question research-start` | `board/BoardDispatchCommands.ts` |
| `spec-stop` | `spec comments`, `spec resolve`, `stop` | `SpecCommentsCommand.ts`, `SpecResolveCommand.ts`, `StopCommand.ts` |

Was beim Aufzeichnen zu beachten ist:

- **Kein Browser.** `harness board` ohne Unterbefehl öffnet das Board im Browser (`BrowserOpener`). Im
  Vertrag steht er deshalb nur als `harness board --no-open`: Server prüfen, Projekt anmelden, Adresse
  ausgeben. Ohne `--no-open` wird er in keinem Szenario aufgerufen.
- **Nie ein Port, auf dem nichts läuft.** `harness board` und die `dispatch`-Befehle starten den Server
  selbst, wenn der Port frei ist – ein solcher Prozess überlebte das Szenario. Die Fehlerfälle nehmen
  deshalb immer einen ungültigen Port (`--port abc`), der schon beim Prüfen auffällt.
- **Worktrees und Branches.** `start`, `done`, `merge`, `plan-start` und `question research-start` legen
  Worktrees neben dem Repo an (`<REPO1>-worktrees/ARC-0001`, `…/plan-ARC-0003`, `…/research-ARC-F0001`). Sie
  liegen unter dem Repo-Ordner der Fixture und werden mit ihm gelöscht. Weil Autor und Zeitpunkt des ersten
  Commits feststehen, ist auch der Hash in der Merge-Meldung in jedem Lauf derselbe.
- **Immer `--by`, `--author` oder `--agent` angeben.** Ohne sie nimmt die CLI den Git-Benutzernamen des
  Rechners – der steht nicht im Vertrag.
- **`stop` zuletzt.** `harness stop` beendet den Server des Szenarios. Danach folgt nur noch ein zweiter
  Aufruf, der „Es läuft kein Server.“ meldet; der Läufer merkt, dass der Server schon weg ist, und meldet
  deshalb keinen Fehler.
- **Keine Spaltenbreite.** Die Board-Ausgaben rechnen nicht mit der Breite des Terminals, nur commander
  bricht seine Hilfe bei 80 Zeichen um (Suite `cli/help`). `COLUMNS` muss deshalb nirgends gesetzt werden.
- **Läufe ohne Prozess.** Der Server läuft im Vertragsmodus und startet nie einen Worker: `dispatch run`,
  `plan-start` und `question research-start` legen einen Lauf an, der nie endet. Deshalb steht danach
  überall derselbe Zustand, und `claude` wird nie aufgerufen.

Drei Erfolgsfälle fehlen mit Absicht, weil ihre Textausgabe einen Wert aus der Uhr enthält, der kein
Vertragsbestandteil ist und sich nicht ersetzen lässt (der Normalisierer kennt nur ganze Zeitstempel):

| Befehl | Was in der Textausgabe steht | Stattdessen im Vertrag |
| --- | --- | --- |
| `dispatch pause` | „pausiert bis 14:35“ – Uhrzeit des Endes | `--json`; `message` steht in `ignore`, `pausedUntil` wird zu `<TIME>` |
| `plan-start` | „Protokoll ARC-0003-20261028-132938-plan.log“ | `--json` mit `save`; der Name des Laufs wird zu `<var:planlauf>` |
| `question research-start` | dasselbe für die Recherche | `--json` mit `save` → `<var:recherche>` |

Sobald ein Name des Laufs mit `save` gemerkt ist, ersetzt ihn der Normalisierer auch in den Textausgaben der
folgenden Schritte – `dispatch status` nennt ihn deshalb als `<var:lauf>`. Genauso hat `harness stop` keinen
Fehlerfall: ohne laufenden Server meldet er „Es läuft kein Server.“ mit Exit-Code 0.

### Hilfetexte (`cli/help/`)

`cli/help/help.scenario.json` ist **erzeugt**, nicht von Hand geschrieben:

```sh
node contract/lib/gen-help-scenarios.mjs
node contract/run.mjs --suite cli --only cli/help --update
```

Der Generator ruft `<cli> --help` auf, liest aus dem Abschnitt „Befehle“ rekursiv alle Unterbefehle
(Argumente und Aliasse werden abgeschnitten) und schreibt je Befehl einen Schritt `{"kind":"cli","args":[…,"--help"]}`.
Dazu kommen die Fehlermeldungen von commander (`GermanErrors`): unbekannter Befehl, mit Vorschlag
„Meinten Sie …?“, unbekannter Unterbefehl, fehlendes Pflichtargument, unbekannte Option am Programm und an
einem Befehl, Option ohne Wert. Der eingebaute Befehl `help` bekommt keinen eigenen Schritt – er zeigt die
Hilfe des übergeordneten Befehls; stattdessen prüft ein Schritt `harness help board`.

Zwei Dinge gehören zum Vertrag und sind leicht zu übersehen:

- **Zeilenumbruch bei 80 Zeichen.** Die Ausgabe geht in eine Pipe, nicht auf ein Terminal; commander bricht
  dann bei 80 Zeichen um. Der Port muss genauso umbrechen.
- **Der Programmname `harness`** steht in jedem Hilfetext (`Aufruf: harness board …`) und wird **nicht**
  normalisiert: Der Alias `harness` bleibt Vertrag. Wird der Port als `nexus` aufgerufen, darf er den Namen
  tauschen; dann kommt hier eine Normalisierungsregel dazu – vorher nicht.

## Gegen einen fremden Server prüfen

```sh
H=$(mktemp -d)
HARNESS_HOME=$H node contract/servers/ts-server.mjs --port 47999 &
node contract/run.mjs --base http://127.0.0.1:47999 --home $H --only basis/health
kill %1
```

Der fremde Server muss mit genau diesem leeren Home gestartet sein. Weil sein Zustand nicht zurückgesetzt
werden kann, ist **genau ein** Szenario erlaubt: `--only` muss eindeutig passen, sonst bricht der Lauf ab.
Dieselbe Form nimmt später der Server in Rust: entweder `--base` auf einen von Hand gestarteten Server oder
`--server-cmd "target/debug/harness-server --port {port}"`, damit `run.mjs` ihn selbst startet.

## Dateien

```
contract/
├─ README.md
├─ run.mjs                    # Einstieg: Argumente, Ablauf, Exit-Code
├─ lib/
│  ├─ ScenarioLoader.mjs      # liest *.scenario.json, prüft die Felder
│  ├─ ScenarioRunner.mjs      # Home, Fixtures, Variablen, Schritte, Normalisierung
│  ├─ StepKinds.mjs           # Registry der Schritt-Arten
│  ├─ HttpStep.mjs            # Schritt-Art „http“
│  ├─ CliStep.mjs             # Schritt-Art „cli“ (Programm, Umgebung, stdin)
│  ├─ HttpStubStep.mjs        # Schritt-Art „httpStub“
│  ├─ HttpStub.mjs            # Mini-HTTP-Server der Attrappe
│  ├─ SseSteps.mjs            # Schritt-Arten „sseOpen“, „sseExpect“, „sseClose“
│  ├─ SseStream.mjs           # eine offene Verbindung: liest mit, gibt bis zum Treffer heraus
│  ├─ SseParser.mjs           # zerlegt den Textstrom in Ereignisse und Kommentare
│  ├─ SseEvents.mjs           # Herzschläge, Zusammenfassen, until als Teilobjekt
│  ├─ CommitStep.mjs          # Schritt-Art „commit“
│  ├─ HomeFileStep.mjs        # Schritt-Art „homeFile“
│  ├─ ServerHandle.mjs        # Server starten/beenden oder --base nutzen
│  ├─ Normalizer.mjs          # ersetzt die wechselnden Werte
│  ├─ GoldenStore.mjs         # lesen, schreiben, vergleichen
│  ├─ JsonText.mjs            # kanonisches JSON und zeilenweiser Diff
│  ├─ Variables.mjs           # {{NAME}} und save
│  ├─ Coverage.mjs            # routes.json gegen die aufgezeichneten Schritte
│  ├─ RepoFixture.mjs         # temporäre Git-Repos
│  ├─ GitIdentity.mjs         # feste Identität, festes Datum und feste Einstellungen für jeden Commit
│  ├─ HomeFiles.mjs           # Fixture-Dateien im HARNESS_HOME
│  ├─ extract-routes.mjs      # erzeugt api/routes.json
│  └─ gen-help-scenarios.mjs  # erzeugt cli/help/help.scenario.json
├─ servers/
│  └─ ts-server.mjs           # TS-Server im Vertragsmodus
├─ api/
│  ├─ routes.json             # alle Routen des Servers
│  ├─ basis/                  # Beispielszenarien
│  ├─ board/                  # Board-Kern: Projekte, Boards, Tickets, Schritte (/api/db)
│  ├─ board-views/            # Notizen, Suche, Statistik, Graph, Inbox
│  ├─ docs/                   # Szenarien der Docs-Seite (Baum, Datei, Suche, Entwürfe, Kommentare)
│  ├─ questions/              # Fragen-Routen
│  ├─ dispatch/               # Dispatcher: Einstellung, Steuerung, Läufe, Vorschläge
│  ├─ dispatch-review/        # Dispatcher: Review, Zurückgeben, Abnehmen, Mergen, Planung, Recherche
│  └─ misc/                   # Sitzungen, Favoriten, Hook, Einstellungen, Usage, Workflow, Oberfläche
├─ sse/                       # Szenarien des Ereignisstroms
├─ cli/
│  ├─ hooks/                  # je Hook-Ereignis ein Szenario
│  ├─ board/                  # alle Unterbefehle von `harness board`, dazu `spec` und `stop`
│  └─ help/                   # Hilfetexte aller Befehle (erzeugt)
└─ tests/                     # Einheitentests (vitest, siehe vitest.config.ts)
```

## Neue Szenarien und neue Schritt-Arten

Ein neues Szenario ist eine Datei unter `contract/<suite>/<gruppe>/`; die Golden-Datei entsteht mit
`node contract/run.mjs --suite <suite> --only <gruppe>/<name> --update`. Danach einmal ohne `--update`
laufen lassen – `git status` darf nichts mehr zeigen.

Eine neue Schritt-Art ist ein eigenes Modul mit einer Klasse und diesen Mitgliedern:

- `keys`: die Felder, die diese Art im Szenario kennt (alles andere ist dort ein unbekanntes Feld),
- `parse(value, index, fail)` (freiwillig): prüft diese Felder schon beim Lesen des Szenarios und liefert sie
  in der Form, die `run` erwartet. Ohne `parse` bekommt die Art den Schritt unverändert als `step.raw` und
  prüft ihre eigenen Felder selbst beim Ausführen (so arbeiten `httpStub`, `commit` und `homeFile`),
- `run(step, context)`: führt den Schritt aus. `context` hält `base` (Adresse des Servers), `variables`,
  `streams` (die offenen Ereignisströme), `stubs` (die laufenden Attrappen), `home` (das `HARNESS_HOME` des
  Laufs) und `reposDir` (der Ordner der Fixture-Repos), für CLI-Aufrufe zusätzlich `cli` (das Programm als
  Liste), `claudeConfig`, `port` und `envFile`. Der Rückgabewert ist
  `{ request, response }`; `response` darf alles enthalten, was die Art aufzeichnet, und `save` sowie `ignore`
  greifen darauf zu (`json.…`, `headers.…`).

Die Art meldet sich in der Registry `stepKinds` in `lib/StepKinds.mjs` an; der Lader prüft dann die
gemeinsamen Felder (`name`, `kind`, `save`, `compare`, `ignore`, `record`) und die aus `keys`.
