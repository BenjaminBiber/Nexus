# Vertragstest der HTTP-API

Die HTTP-API des Servers ist der Vertrag, den jede Umsetzung erfüllen muss – heute der TypeScript-Server,
später der Server in Rust. Dieser Ordner enthält das Werkzeug dafür: Es startet einen Server mit frischem
`HARNESS_HOME`, spielt Anfragen aus Szenario-Dateien ab, ersetzt die je Lauf wechselnden Werte durch
Platzhalter und vergleicht das Ergebnis mit einer Golden-Datei – oder zeichnet sie neu auf.

Es gibt zwei Sammlungen von Szenarien: `api/` für einzelne Anfragen und `sse/` für den Ereignisstrom
(Server-Sent Events) eines Projekts. Ohne `--suite` laufen beide.

Alles hier ist plain Node (ESM, `.mjs`), ohne zusätzliche Abhängigkeiten und ohne eigenen Build-Schritt.
Nur der geprüfte Server muss gebaut sein (`npm run build`).

## Aufrufe

```sh
npm run build                      # Voraussetzung: packages/*/dist und packages/web/dist
npm run contract                   # alles vergleichen (= node contract/run.mjs)
node contract/run.mjs --update     # Golden-Dateien neu aufzeichnen
node contract/run.mjs --only basis/projects
node contract/run.mjs --only sse   # nur die Szenarien des Ereignisstroms
node contract/run.mjs --coverage   # alle Routen mit abgedeckt/offen
npx vitest run contract            # Einheitentests (Normalisierer, Szenario-Lader, Ereignisstrom)
```

| Option | Bedeutung |
| --- | --- |
| `--update` | Golden-Dateien neu schreiben; nur Dateien mit geändertem Inhalt werden angefasst. |
| `--only <teilstring>` | Nur Szenarien, deren Pfad den Teilstring enthält (z. B. `basis/health`). |
| `--suite <name>` | Ordner unter `contract/` mit den Szenarien (Standard: alle, also `api` und `sse`). |
| `--server-cmd "<cmd {port}>"` | Eigener Startbefehl statt des TS-Servers; `{port}` wird ersetzt. Relative Pfade gelten ab der Projektwurzel. |
| `--base <url> --home <ordner>` | Gegen einen schon laufenden Server prüfen (siehe unten). |
| `--cli-cmd "<cmd>"` | Befehl der CLI für `harnessInit` (Standard `node packages/cli/dist/main.js`). |
| `--coverage` | Routen aus `api/routes.json` auflisten, Exit 0. |
| `--strict` | Zusammen mit `--coverage`: Exit 1, wenn eine Route offen ist. |
| `--verbose` | Ausgabe des gestarteten Servers mitschreiben. |

Exit-Codes: `0` alles grün, `1` Abweichung, fehlende Golden-Datei oder offene Route bei `--coverage --strict`,
`2` Fehler im Aufruf (unbekannte Option, `--base` ohne `--home`).

## Ablauf

Jedes Szenario läuft gegen einen **frischen Server mit frischem Home**. Deshalb sind Auto-Inkrement-IDs und
Ticket-IDs (`BEN-0001`) deterministisch und müssen nicht normalisiert werden.

1. Szenarien unter `contract/<suite>/**` suchen, nach `--only` und `platforms` filtern.
2. Je Szenario: temporäres `HARNESS_HOME`, temporäre `CLAUDE_CONFIG_DIR`, freier Port, Fixture-Repos anlegen.
3. Server starten (Standard `node contract/servers/ts-server.mjs --port {port}`) und auf `GET /api/health`
   warten (höchstens 15 s).
4. Fixture `boardConfig` anwenden, dann die Schritte ausführen und normalisieren.
5. Server beenden (unter Windows mit `taskkill /T /F`), Home und Repos löschen.
6. Vergleichen oder (mit `--update`) aufzeichnen; am Ende eine Zusammenfassung.

## Vertragsmodus des Servers

Jede Umsetzung des Servers muss diesen Modus anbieten; nur so sind die Antworten reproduzierbar.
Beim TypeScript-Server macht das `contract/servers/ts-server.mjs`.

- Version in `GET /api/health` ist `contract`.
- Kein Takt des Dispatchers; es wird **nie** ein Worker gestartet (`POST /api/dispatch/run` bleibt wirkungslos).
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

## Szenario-Format (`*.scenario.json`)

```json
{
  "title": "Projekte anmelden und auflisten",
  "platforms": ["darwin", "linux", "win32"],
  "fixtures": {
    "repos": [{ "name": "REPO1", "git": true, "harnessInit": false, "files": { "docs/a.md": "# A\n" } }],
    "boardConfig": "sqlite"
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
  Der Ordner heißt `<temp>/<NAME>/Mein Projekt ä` – Leerzeichen und Umlaut gehören zum Test.
  `{{REPO1}}` ist der aufgelöste Pfad (unter macOS `/private/var/…`, nicht `/var/…`), also genau der Text, den
  der Server als Projektwurzel führt. Nur so findet ein Board, dessen `repoPath` auf `{{REPO1}}` zeigt, sein
  Projekt wieder – der Vergleich in `RepoBoardResolver` ist ein reiner Textvergleich.
- `boardConfig: "sqlite"`: vor den Schritten `PUT /api/board-config` mit
  `{ provider: "sqlite", dbFile: "<HOME>/board.db", userId: "11111111-1111-1111-1111-111111111111", userName: "Vertrag" }`.
  Dieser Aufruf wird **nicht** aufgezeichnet.

### Variablen

`{{NAME}}` wird in `path`, `body` und `headers` eingesetzt, auch verschachtelt. Bekannt sind:

- die Repo-Namen der Fixtures (`{{REPO1}}` → Pfad des Repos),
- `{{HOME}}` (das `HARNESS_HOME` des Laufs) und `{{PORT}}`,
- alle mit `save` gemerkten Werte.

Besteht ein Wert nur aus einem Platzhalter (`"{{PORT}}"`), behält er seinen Typ; sonst wird er in den Text
eingesetzt. Ein unbekannter Name ist ein Fehler.

`save` liest Werte aus der Antwort über Punkt-Notation, beginnend mit `json.` oder `headers.`
(`"save": { "pid": "json.id", "ort": "headers.location" }`). Listen sprechen Indizes an: `json.items.0.id`.

### Schritte

Jeder Schritt hat einen eindeutigen `name`. `kind` wählt die Art (Standard `http`); bekannt sind `http`,
`httpStub`, `sseOpen`, `sseExpect` und `sseClose`, eine unbekannte Art ist ein Fehler. Diese Felder gelten
für jede Art:

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
4. **Zeit**: ISO-Zeitstempel → `<TIME>`; ein Datum `YYYY-MM-DD` nur dann → `<DATE>`, wenn es das heutige ist.
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
│  ├─ HttpStubStep.mjs        # Schritt-Art „httpStub“
│  ├─ HttpStub.mjs            # Mini-HTTP-Server der Attrappe
│  ├─ SseSteps.mjs            # Schritt-Arten „sseOpen“, „sseExpect“, „sseClose“
│  ├─ SseStream.mjs           # eine offene Verbindung: liest mit, gibt bis zum Treffer heraus
│  ├─ SseParser.mjs           # zerlegt den Textstrom in Ereignisse und Kommentare
│  ├─ SseEvents.mjs           # Herzschläge, Zusammenfassen, until als Teilobjekt
│  ├─ ServerHandle.mjs        # Server starten/beenden oder --base nutzen
│  ├─ Normalizer.mjs          # ersetzt die wechselnden Werte
│  ├─ GoldenStore.mjs         # lesen, schreiben, vergleichen
│  ├─ JsonText.mjs            # kanonisches JSON und zeilenweiser Diff
│  ├─ Variables.mjs           # {{NAME}} und save
│  ├─ Coverage.mjs            # routes.json gegen die aufgezeichneten Schritte
│  ├─ RepoFixture.mjs         # temporäre Git-Repos
│  └─ extract-routes.mjs      # erzeugt api/routes.json
├─ servers/
│  └─ ts-server.mjs           # TS-Server im Vertragsmodus
├─ api/
│  ├─ routes.json             # alle Routen des Servers
│  ├─ basis/                  # Beispielszenarien
│  ├─ board/                  # Board-Kern: Projekte, Boards, Tickets, Schritte (/api/db)
│  ├─ board-views/            # Notizen, Suche, Statistik, Graph, Inbox
│  ├─ docs/                   # Szenarien der Docs-Seite (Baum, Datei, Suche, Entwürfe, Kommentare)
│  ├─ questions/              # Fragen-Routen
│  └─ misc/                   # Sitzungen, Favoriten, Hook, Einstellungen, Usage, Workflow, Oberfläche
├─ sse/                       # Szenarien des Ereignisstroms
└─ tests/                     # Einheitentests (vitest, siehe vitest.config.ts)
```

## Neue Szenarien und neue Schritt-Arten

Ein neues Szenario ist eine Datei unter `contract/api/<gruppe>/` (oder `contract/sse/`); die Golden-Datei
entsteht mit `node contract/run.mjs --only <gruppe>/<name> --update`. Danach einmal ohne `--update` laufen
lassen – `git status` darf nichts mehr zeigen.

Eine neue Schritt-Art (geplant: CLI-Aufrufe) ist ein eigenes Modul mit einer Klasse und diesen Mitgliedern:

- `keys`: die Felder, die diese Art im Szenario kennt (alles andere ist dort ein unbekanntes Feld),
- `parse(value, index, fail)` (freiwillig): prüft diese Felder schon beim Lesen des Szenarios und liefert sie
  in der Form, die `run` erwartet. Ohne `parse` bekommt die Art den Schritt unverändert als `step.raw` und
  prüft ihre eigenen Felder selbst beim Ausführen (so arbeitet `httpStub`),
- `run(step, context)`: führt den Schritt aus. `context` hält `base` (Adresse des Servers), `variables`,
  `streams` (die offenen Ereignisströme) und `stubs` (die laufenden Attrappen). Der Rückgabewert ist
  `{ request, response }`; `response` darf alles enthalten, was die Art aufzeichnet, und `save` sowie `ignore`
  greifen darauf zu (`json.…`, `headers.…`).

Die Art meldet sich in der Registry `stepKinds` in `lib/StepKinds.mjs` an; der Lader prüft dann die
gemeinsamen Felder (`name`, `kind`, `save`, `compare`, `ignore`, `record`) und die aus `keys`.
