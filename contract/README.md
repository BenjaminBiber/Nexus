# Vertragstest der HTTP-API

Die HTTP-API des Servers ist der Vertrag, den jede Umsetzung erfüllen muss – heute der TypeScript-Server,
später der Server in Rust. Dieser Ordner enthält das Werkzeug dafür: Es startet einen Server mit frischem
`HARNESS_HOME`, spielt Anfragen aus Szenario-Dateien ab, ersetzt die je Lauf wechselnden Werte durch
Platzhalter und vergleicht das Ergebnis mit einer Golden-Datei – oder zeichnet sie neu auf.

Alles hier ist plain Node (ESM, `.mjs`), ohne zusätzliche Abhängigkeiten und ohne eigenen Build-Schritt.
Nur der geprüfte Server muss gebaut sein (`npm run build`).

## Aufrufe

```sh
npm run build                      # Voraussetzung: packages/*/dist und packages/web/dist
npm run contract                   # alles vergleichen (= node contract/run.mjs)
node contract/run.mjs --update     # Golden-Dateien neu aufzeichnen
node contract/run.mjs --only basis/projects
node contract/run.mjs --coverage   # alle Routen mit abgedeckt/offen
npx vitest run contract            # Einheitentests (Normalisierer, Szenario-Lader)
```

| Option | Bedeutung |
| --- | --- |
| `--update` | Golden-Dateien neu schreiben; nur Dateien mit geändertem Inhalt werden angefasst. |
| `--only <teilstring>` | Nur Szenarien, deren Pfad den Teilstring enthält (z. B. `basis/health`). |
| `--suite <name>` | Ordner unter `contract/` mit den Szenarien (Standard `api`). |
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

- `title` ist Pflicht, `platforms` und `fixtures` sind freiwillig. Ohne `platforms` läuft das Szenario überall.
- Unbekannte Felder sind ein Fehler – Tippfehler fallen so beim Laden auf, nicht erst im Vergleich.
- Die Golden-Datei liegt daneben und heißt wie das Szenario: `projects.scenario.json` → `projects.golden.json`.

### Fixtures

- `repos`: temporäre Git-Repos. `name` in Großbuchstaben (`REPO1`), danach als `{{REPO1}}` verwendbar.
  `git` (Standard `true`) legt ein Repo mit erstem Commit an, `files` schreibt Dateien (Pfade mit `/`),
  `harnessInit` führt `<cli> init` im Repo aus (nötig, damit sich das Projekt anmelden lässt).
  Der Ordner heißt `<temp>/<NAME>/Mein Projekt ä` – Leerzeichen und Umlaut gehören zum Test.
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

Jeder Schritt hat einen eindeutigen `name`. `kind` wählt die Art (Standard `http`).

| Feld | Bedeutung |
| --- | --- |
| `method` | `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`. |
| `path` | Pfad ab `/`, mit Abfrageteil erlaubt. |
| `body` | Körper als JSON; bei `GET`/`HEAD` wird er nur aufgezeichnet, nicht gesendet. |
| `headers` | Zusätzliche Köpfe. Wert `null` lässt einen Standardkopf weg (für Tests des Schutzes). |
| `save` | Werte für späteren Gebrauch merken. |
| `compare` | `full` (Standard), `status` oder `shape`. |
| `ignore` | Felder, die vor dem Vergleich entfernt werden (`"json.a.b"`, relativ zur Antwort). |
| `record` | `false` führt den Schritt aus, zeichnet ihn aber nicht auf (Vorbereitung). |

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
5. **UUIDs** → `<UUID>`, außer `11111111-1111-1111-1111-111111111111` (der Vertrags-Benutzer).
6. **Git-Commits** in Feldern `sha`, `commit`, `head` (40 bzw. 7–12 Hex-Zeichen) → `<SHA>`.
7. **Zahlenfelder** mit Namen auf `Ms` oder `At` sowie `durationMs`, `pid` und `port` → `0`. Das trifft auch
   feste Werte wie den Standard-Port 1433 der Board-Konfiguration; dafür bleibt die Datei über Läufe gleich.

Jede Regel ist eine eigene Methode in `lib/Normalizer.mjs` und einzeln getestet
(`tests/Normalizer.test.mjs`).

## Routenliste und Abdeckung

`api/routes.json` enthält alle Routen des Servers (Methode, Muster, Quelldatei) und wird aus
`packages/server/src/routes/*.ts` erzeugt:

```sh
node contract/lib/extract-routes.mjs
```

`--coverage` liest die Golden-Dateien (es läuft kein Server), ordnet jeden aufgezeichneten Schritt über
Methode und Pfad einem Muster zu (`:param` → ein Pfadabschnitt, Abfrageteil abgeschnitten) und gibt je
Quelldatei eine Tabelle aus: `[x]` abgedeckt, `[ ]` offen. Schritte ohne passende Route stehen am Ende mit
`?` – dann ist `routes.json` veraltet. Mit `--strict` endet der Lauf mit Exit 1, solange eine Route offen ist
(gedacht für den Abschluss von Phase 0).

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
│  ├─ HttpStep.mjs            # Schritt-Art „http“
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
│  └─ board/                  # Board-Kern: Projekte, Boards, Tickets, Schritte (/api/db)
└─ tests/                     # Einheitentests (vitest, siehe vitest.config.ts)
```

## Neue Szenarien und neue Schritt-Arten

Ein neues Szenario ist eine Datei unter `contract/api/<gruppe>/`; die Golden-Datei entsteht mit
`node contract/run.mjs --only <gruppe>/<name> --update`. Danach einmal ohne `--update` laufen lassen –
`git status` darf nichts mehr zeigen.

Eine neue Schritt-Art (geplant: Ereignisstrom, CLI-Aufrufe) ist ein eigenes Modul mit einer Klasse und einer
Methode `run(step, context)`. `context` hält `base` (Adresse des Servers) und `variables`. Der Rückgabewert
ist `{ request, response }`; `response` darf alles enthalten, was die Art aufzeichnet, und `save` sowie
`ignore` greifen darauf zu (`json.…`, `headers.…`). Die Art meldet sich in der Registry `stepKinds` in
`lib/ScenarioRunner.mjs` an; der Lader prüft dann nur die gemeinsamen Felder.
