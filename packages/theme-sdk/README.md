# @nexus/theme-sdk

Das Theme-SDK beschreibt den Vertrag zwischen der Nexus-Oberfläche (Host) und einem Theme: welche
Stellen ein Theme ersetzen darf (Slots), welche Daten und Bausteine es vom Host bekommt (Hooks und
Bausteine), welche CSS-Variablen es setzen kann und wie das Manifest aussieht.

Aktuelle Version: `SDK_VERSION` = **1.2.0** (gleich der Paketversion in `package.json`).

> Dieses README ist die maßgebliche Doku des SDK. `tests/readme.test.ts` prüft, dass jeder Slot,
> jeder Export und jede CSS-Variable aus `:root` hier genannt ist.

## Überblick

- **Reine CSS-Themes** bringen nur `theme.css` mit und setzen CSS-Variablen.
- **Themes mit Code** bringen zusätzlich `main.js` (ESM) mit und ersetzen Slots durch eigene
  React-Komponenten. Themes haben volles Vertrauen, es gibt keine Sandbox; installiert wird nur aus
  Repos, die der Nutzer selbst eingetragen hat, mit sha256-Prüfung.
- Ersetzbar ist nur, was ein Slot ist. Fehlt ein Slot, gilt die Standard-Komponente.
- Hell und Dunkel sind fest eingebaut; alle anderen Themes sind Pakete.

### Paketformat `.nexus`

Ein Theme ist ein ZIP mit der Endung `.nexus`:

| Datei | Pflicht | Inhalt |
|---|---|---|
| `manifest.json` | ja | Beschreibung des Themes (siehe [Manifest](#manifest)) |
| `theme.css` | ja | Werte der CSS-Variablen und eigene Regeln |
| `main.js` | nein | ESM-Modul mit der `ThemeDefinition` |
| `assets/**` | nein | Bilder, Schriften usw. |

Installierte Themes liegen unter `~/.nexus/themes/<id>/`. Gepackt wird mit `nexus theme pack`.

## Manifest

```json
{
  "id": "nacht",
  "name": "Nacht",
  "version": "1.2.3",
  "author": "Ada",
  "sdkVersion": "^1.0.0",
  "colorScheme": "dark",
  "startColor": "#0b0b10",
  "preview": "assets/preview.png",
  "slots": ["Shell", "Progress"],
  "permissions": { "network": ["fonts.example.com"] },
  "settingsSchema": {
    "sterne": { "type": "boolean", "label": "Sternenhimmel", "default": true }
  }
}
```

| Feld | Pflicht | Bedeutung |
|---|---|---|
| `id` | ja | Kennung, Muster `^[a-z0-9-]{2,40}$`; zugleich Wert von `data-theme` |
| `name` | ja | Anzeigename |
| `version` | ja | Version des Themes (semver) |
| `author` | ja | Autor |
| `sdkVersion` | ja | semver-Bereich der passenden SDK-Versionen, z. B. `^1.0.0` |
| `colorScheme` | ja | `light` oder `dark`: Grundschema, auf dem das Theme aufsetzt |
| `startColor` | nein | Hintergrundfarbe beim Start, bevor `theme.css` geladen ist |
| `preview` | nein | Pfad zu einem Vorschaubild im Paket |
| `slots` | nein | Liste der Slots, die `main.js` ersetzt |
| `permissions.network` | nein | Domains, die das Theme über den Server-Proxy laden darf |
| `settingsSchema` | nein | Eigene Einstellungen (siehe [Theme-Einstellungen](#theme-einstellungen)) |

## ThemeDefinition

`main.js` beschreibt das Theme als `ThemeDefinition`:

| Feld | Typ | Bedeutung |
|---|---|---|
| `id` | `string` | ID des Themes, gleich `id` im Manifest |
| `slots` | `ThemeSlots` | ersetzte Slots |
| `settingsSchema` | `ThemeSettingsSchema` (optional) | eigene Einstellungen |

```tsx
import type { ThemeDefinition, ProgressProps } from '@nexus/theme-sdk';

function Progress({ done, total, label }: ProgressProps) {
  return <progress aria-label={label} value={done} max={total} />;
}

const theme: ThemeDefinition = { id: 'nacht', slots: { Progress } };
export default theme;
```

## Slots

`ThemeSlots` listet alle Stellen, die ein Theme ersetzen darf. Jeder Slot ist optional.

| Slot | Props | Ersetzt |
|---|---|---|
| `Shell` | `ShellProps` | das ganze Layout um die Seiten |
| `NavIcon` | `{ name: NavIconName }` | Symbole der Hauptnavigation |
| `Progress` | `ProgressProps` | Fortschrittsanzeige (erledigte/alle Schritte) |
| `Button` | `ButtonProps` | Knöpfe |
| `Card` | `CardProps` | Inneres einer Karte |
| `Panel` | `PanelProps` | Inneres eines Bereichs (z. B. Einstellungen) |
| `Dialog` | `DialogProps` | sichtbare Hülle eines Dialogs |
| `Input` | `InputProps` | Eingabefelder (ein- und mehrzeilig) |
| `StartAnimation` | `StartAnimationProps` | Ladebildschirm beim Start |
| `page:dashboard` | `PageProps` | die ganze Dashboard-Seite |

**Fehler und abgesicherter Modus:** Jeder Slot sitzt in einer Error Boundary. Wirft eine
Theme-Komponente, zeigt der Host die Standard-Komponente und einen Hinweis. Mit dem
Adress-Parameter `?safe-theme` startet die Oberfläche ganz ohne Theme-Code.

### `ShellProps`

| Feld | Typ | Bedeutung |
|---|---|---|
| `pid` | `string` | Projekt-ID aus der Adresse |
| `nav` | `ReactNode` | Links der Hauptnavigation |
| `content` | `ReactNode` | Inhaltsbereich mit Verbindungshinweis und aktueller Seite |
| `parts` | `ShellParts` | weitere Bausteine des Rahmens |

`ShellParts` (auch über `useShellParts()`):

| Feld | Typ | Bedeutung |
|---|---|---|
| `search` | `ReactNode` | Knopf der globalen Suche (auch Strg+K) |
| `boardSwitcher` | `ReactNode` | Board-Auswahl samt Verbindungspunkt |
| `usage` | `ReactNode` | Claude-Nutzung (Ring/Balken) |
| `navItems` | `Partial<Record<NavIconName, ReactNode>>` | einzelne Links der Hauptnavigation |
| `outlet` | `ReactNode` | nur die aktuelle Seite |
| `connection` | `ReactNode` | Banner bei getrennter Live-Verbindung |
| `status` | `ReactNode` | Verbindungspunkt der Live-Verbindung |

`NavIconName`: `board`, `dashboard`, `inbox`, `sessions`, `questions`, `docs`, `search`, `guide`, `settings`.

### `ProgressProps`

`done: number`, `total: number`, `label: string` (zugängliche Bezeichnung), `caption: string`
(Beschriftung neben der Zahl).

### `ButtonProps`

`variant?: 'primary' | 'secondary' | 'danger' | 'ghost'` (Standard `secondary`), `type?: 'button' | 'submit'`,
`disabled?`, `onClick?`, `title?`, `autoFocus?`, `aria-label?`, `aria-pressed?`, `aria-expanded?`,
`className?`, `children?`. Das Theme rendert das `<button>` selbst und reicht die Props weiter.

### `CardProps` und `PanelProps`

`CardProps`: `title?`, `actions?`, `className?`, `children?`. `PanelProps`: `title?`, `className?`,
`children?`. Das äußere Element bleibt im Host und trägt `data-slot="Card"` bzw. `data-slot="Panel"`.

### `DialogProps`

`labelledBy: string` (id der Überschrift), `role: 'dialog' | 'alertdialog'`, `className?`, `children?`.
Rolle, Fokusfalle, Esc und Fokusrückgabe bleiben im Host (`data-slot="Dialog"`).

### `InputProps`

`value: string`, `onChange(value: string)`, `id?`, `name?`, `type?` (`text`, `search`, `password`,
`email`, `url`, `number`), `placeholder?`, `disabled?`, `required?`, `autoFocus?`, `multiline?`,
`rows?`, `aria-label?`, `aria-describedby?`, `className?`. Das Theme rendert `<input>` bzw. `<textarea>`.

### `StartAnimationProps`

`onDone(): void` – muss gerufen werden, wenn die Animation fertig ist. Liegt als Vollbild über der
App und wird spätestens nach 8 s beendet.

### Seiten-Slots: `PageName`, `PageProps`, `PageSlots`

`PageSlots` sind Schlüssel der Form `page:<name>` mit `<name>` aus `PageName` (derzeit nur
`dashboard`). `PageProps` enthält `pid: string`; Daten lädt die Theme-Seite selbst über die Hooks.

## Hooks

Alle Hooks funktionieren nur innerhalb der Nexus-Oberfläche. Außerhalb des `ThemeHostProvider`
werfen sie einen Fehler, der Hook und Provider nennt. `ThemeHostProvider` rendert nur der Host;
`ThemeHost` ist der Typ dessen, was er bereitstellt.

| Hook | Rückgabe | Bedeutung |
|---|---|---|
| `useNav()` | `NavItem[]` | Einträge der Hauptnavigation |
| `useBoards()` | `BoardsState` | Projekte, Boards und Board-Wahl |
| `useUsage()` | `UsageState` | Claude-Verbrauch |
| `useSettings()` | `SettingsState` | Nutzereinstellungen lesen und ändern |
| `useThemeSettings<S>()` | `ThemeSettingsState` mit `values: ThemeSettingValues<S>` | eigene Einstellungen des Themes |
| `useColorScheme()` | `ColorSchemeState` | angezeigtes Farbschema |
| `useConnection()` | `ConnectionState` | Zustand der Live-Verbindung |
| `useNavigate()` | `(to: string) => void` | Route wechseln |
| `useOpenSearch()` | `() => void` | globale Suche öffnen |
| `useShellParts()` | `ShellParts` | fertige Bausteine des Rahmens |
| `useThemeAsset(url, opts?)` | `ThemeAssetState` | fremde Ressource über den Server-Proxy (siehe [Fremde Ressourcen](#fremde-ressourcen-usethemeasset)) |

Typen:

- `NavItem`: `id: NavIconName`, `label`, `href`, `active`, `badge?`.
- `BoardsState`: `current: BoardRef | null`, `projects`, `boards`, `loading`, `choose(ref: BoardRef)`.
  `BoardRef`: `project: string`, `board: number | null`.
- `UsageState`: `enabled`, `available`, `reason`, `fiveHour`, `sevenDay` (je `UsageWindow | null`),
  `accounts` (`UsageAccount[]`, alle aktiven Accounts; leer ohne Einstellung oder ohne Werte) und
  `defaultLimit` (Vorgabe des Dispatchers in Prozent, ab der ein Account als voll gilt).
  `UsageWindow`: `percent`, `resetsAt` (ISO-Zeitpunkt oder null).
- `UsageAccount`: `id`, `name`, `fiveHour`, `sevenDay` (je `UsageWindow | null`),
  `limitFiveHour`, `limitWeek` (eigene Obergrenzen in Prozent; null = Vorgabe `defaultLimit`) und
  `reason` (Grund fehlender Werte, null wenn Werte da sind).
- `SettingsState`: `values: ThemeSettings | null`, `loading`, `update(patch)`.
  `ThemeSettings`: `theme`, `claudeUsage`, `compactCards`, `heroImages`.
- `ColorSchemeState`: `scheme: 'light' | 'dark'`, `themeId`, `mode`.
- `ConnectionState`: `connected` (true, solange der Ereignisstrom des Servers verbunden ist).

```tsx
import { useBoards, useNavigate } from '@nexus/theme-sdk';

function Boards() {
  const { boards } = useBoards();
  const navigate = useNavigate();
  return boards.map((b) => <button key={b.id} onClick={() => navigate(`/p/${b.project}/board`)}>{b.name}</button>);
}
```

## Fremde Ressourcen: `useThemeAsset`

Ein Theme lädt Bilder, Schriften oder Dateien aus einem JAR/ZIP nie direkt vom fremden Server
(CORS, Datenschutz), sondern über den Server-Proxy von Nexus. `useThemeAsset(url, opts?)` baut die
lokale Proxy-URL `/api/themes/<id>/fetch?url=<kodiert>` (plus `sha256` und `entry`, wenn gesetzt),
prüft sie per `fetch` und meldet Lade- und Fehlerzustand. Ein Theme darf nur Domains laden, die in
`permissions.network` des Manifests stehen; sonst antwortet der Proxy mit 403. Die Ressourcen liegen
im Cache je Theme.

```ts
function useThemeAsset(url: string | null | undefined, opts?: ThemeAssetOptions): ThemeAssetState;
function themeAssetUrl(themeId: string, url: string, opts?: Omit<ThemeAssetOptions, 'themeId'>): string;
```

`ThemeAssetOptions`:

| Feld | Bedeutung |
|---|---|
| `themeId?` | Theme, dessen Freigabe und Cache gelten; Standard ist das aktive Theme des Hosts (`useColorScheme().themeId`) |
| `sha256?` | erwartete Prüfsumme (hex); weicht sie ab, antwortet der Proxy mit 502 `theme_fetch_checksum` |
| `entry?` | Pfad eines Eintrags, wenn `url` ein ZIP oder JAR ist, z. B. `assets/minecraft/textures/block/dirt.png` |

`ThemeAssetState`:

| `status` | `url` | `error` | Bedeutung |
|---|---|---|---|
| `idle` | `null` | `null` | keine URL angegeben (`null`, `undefined`, leer), es wird nichts geladen |
| `loading` | `null` | `null` | Anfrage läuft |
| `ready` | Proxy-URL | `null` | Ressource erreichbar; die URL z. B. in `<img src>` oder `url(...)` verwenden |
| `error` | `null` | `ThemeAssetError` | Laden fehlgeschlagen |

Ändert sich `url` oder eine Option, lädt der Hook neu und bricht die alte Anfrage ab; beim Entfernen
der Komponente ebenfalls. Der Hook hält keine Bytes, das Bild lädt der Browser aus seinem HTTP-Cache.

`ThemeAssetError`: `status: number | null` (HTTP-Status, bei Netzfehler `null`), `code: string`,
`message: string`. Der `code` stammt aus der JSON-Antwort des Proxys:

| `code` | Status | Ursache |
|---|---|---|
| `theme_fetch_forbidden` | 403 | Domain steht nicht in `permissions.network` des Themes |
| `theme_fetch_too_large` | 413 | Ressource ist zu groß |
| `theme_fetch_checksum` | 502 | `sha256` passt nicht zum Inhalt |
| `theme_fetch_upstream` | 502 | fremder Server nicht erreichbar oder antwortet mit Fehler |
| `theme_fetch_entry_not_found` | 404 | `entry` fehlt im Archiv |
| `theme_fetch_not_archive` | 502 | `entry` gesetzt, aber der fremde Server liefert kein ZIP |
| `bad-request` | 400 | ungültige URL oder Parameter |
| `network` | – | Netzwerkfehler beim Aufruf des Proxys |
| `http_<status>` | je Antwort | Antwort ohne JSON-Fehler, z. B. `http_404` |

Ohne `ThemeHostProvider` und ohne `opts.themeId` wirft der Hook einen Fehler.

```tsx
import { useThemeAsset } from '@nexus/theme-sdk';

// Textur aus einem Client-JAR; Domain muss in permissions.network stehen
function Dirt() {
  const asset = useThemeAsset('https://piston-data.mojang.com/v1/objects/abc/client.jar', {
    sha256: '3f2a…',
    entry: 'assets/minecraft/textures/block/dirt.png',
  });
  if (asset.status === 'loading') return <span>lädt …</span>;
  if (asset.status === 'error') return <span title={asset.error?.message}>Bild nicht verfügbar ({asset.error?.code})</span>;
  return asset.url ? <img src={asset.url} alt="Erde" /> : null;
}
```

Für CSS ohne Zustand liefert `themeAssetUrl` nur die Proxy-URL, ohne etwas zu prüfen:

```tsx
import { themeAssetUrl, useColorScheme } from '@nexus/theme-sdk';

function Hintergrund() {
  const { themeId } = useColorScheme();
  const bild = themeAssetUrl(themeId, 'https://fonts.example.com/muster.png');
  return <div style={{ backgroundImage: `url(${bild})` }} />;
}
```

## Bausteine

Fertige Komponenten des Hosts für eigene Shells:

| Baustein | Bedeutung |
|---|---|
| `NavLink` | Link eines Navigationseintrags: `id`, `className?`, `role?`, `onClick?`, `children?: (item: NavItem) => ReactNode`; unbekannte ID rendert nichts |
| `BoardSwitcher` | Board-Auswahl |
| `SearchButton` | Knopf der globalen Suche |
| `UsageMeter` | Claude-Verbrauch |
| `Outlet` | aktuelle Seite ohne Standard-Wrapper |
| `ConnectionStatus` | Verbindungspunkt der Live-Verbindung |

```tsx
import { NavLink, Outlet, SearchButton } from '@nexus/theme-sdk';
import type { ShellProps } from '@nexus/theme-sdk';

function Shell(_: ShellProps) {
  return (
    <div className="meine-shell">
      <header><NavLink id="board" /><NavLink id="inbox" /><SearchButton /></header>
      <main><Outlet /></main>
    </div>
  );
}
```

## Theme-Einstellungen

Ein Theme kann eigene Einstellungen mitbringen (`settingsSchema` in Manifest und
`ThemeDefinition`). Sie erscheinen unter Einstellungen → Darstellung.

- `ThemeSettingsSchema`: `Record<string, ThemeSetting>`, Schlüssel `^[a-zA-Z][a-zA-Z0-9_-]{0,39}$`, höchstens 50.
- `ThemeSetting` = `ThemeSettingBoolean` (`type: 'boolean'`, `label`, `hint?`, `default: boolean`)
  oder `ThemeSettingSelect` (`type: 'select'`, `label`, `hint?`, `options: { value, label }[]`, `default: string`).
- `ThemeSettingValues<S>`: Werte zum Schema, Schalter als `boolean`, Auswahl als `string`.
- `ThemeSettingsState`: `values`, `loading`, `set(key, value)`; fehlende oder ungültige Werte ergeben den Default.

```tsx
const schema = { sterne: { type: 'boolean', label: 'Sternenhimmel', default: true } } as const;
const { values, set } = useThemeSettings<typeof schema>();
```

## CSS-Variablen

Ein Theme setzt Werte in `theme.css` unter dem Selektor `:root[data-theme='<id>']`. Die
Standardwerte stehen in `packages/web/src/styles/variables.css` (`:root` = hell). CSS-Schnipsel des
Nutzers liegen über dem aktiven Theme. Fremde Ressourcen (Schriften, Bilder) lädt ein Theme nur
über den Server-Proxy und nur von Domains aus `permissions.network`.

```css
:root[data-theme='nacht'] {
  color-scheme: dark;
  --bg: #0b0b10;
  --accent: #8b5cf6;
}
```

**Schrift:** `--font-sans`, `--font-mono`.

**Flächen:** `--bg`, `--surface`, `--surface-alt`, `--logo-plate` (helle Unterlage für Logos, in allen
Themes gleich), `--sidebar-bg`, `--nav-active-bg`, `--column-bg`, `--column-locked-a`,
`--column-locked-b` (gesperrte Spalte, Streifen).

**Linien:** `--line`, `--line-input`, `--line-soft`, `--line-strong`, `--line-dashed`.

**Text:** `--text`, `--text-soft`, `--muted`.

**Akzent und Primär:** `--accent`, `--accent-hover`, `--accent-soft`, `--accent-soft-hover`,
`--on-accent`, `--primary`, `--primary-hover`, `--on-primary`, `--chip-on-bg`, `--chip-on-fg`.

**Code:** `--code-bg`, `--code-fg`, `--code-muted`, `--code-btn-bg`, `--code-btn-line`.

**Zustände (Vorder-/Hintergrund):** `--status-bg`, `--status-fg`, `--running-fg`, `--running-bg`,
`--waiting-fg`, `--waiting-bg`, `--waiting-line`, `--idle-fg`, `--idle-bg`, `--blocked-fg`,
`--blocked-bg`, `--orphan-fg`, `--orphan-bg`, `--unread-fg`, `--unread-bg`, `--unread-row-bg`,
`--row-waiting-bg`, `--urgent-row-bg`.

**Hinweise und Meldungen:** `--note-bg`, `--note-line`, `--note-fg`, `--ok`, `--ok-fg`, `--ok-ring`,
`--danger`, `--warn`, `--connection-line`, `--connection-fg`, `--error-bg`, `--answered-bg`,
`--answered-line`, `--research-bg`, `--research-fg`.

**Claude-Nutzung:** `--usage-ok`, `--usage-warn`, `--usage-crit`, `--usage-over` (über der Grenze eines Accounts).

**Markierungen:** `--quote-line`, `--mark`, `--mark-comment`, `--mark-comment-hover`, `--tag-add-line`.

**Diff und Tests** (Diff-Zeilen stehen im dunklen Codeblock): `--diff-add-bg`, `--diff-add-fg`,
`--diff-del-bg`, `--diff-del-fg`, `--diff-hunk-fg`, `--test-ok`, `--test-fail`.

**Board-Spalten (Farbpunkt):** `--status-backlog`, `--status-waiting`, `--status-in-planning`,
`--status-planned`, `--status-in-progress`, `--status-blocked`, `--status-done`, `--status-not-needed`.

**Abhängigkeitsgraph:** `--graph-edge`, `--graph-critical` (kritischer Pfad), `--graph-upstream`, `--graph-downstream`.

**Diagramme des Dashboards:** `--chart-done`, `--chart-fail`, `--chart-bar`, `--chart-track`,
`--chart-grid`, `--chart-axis`, `--chart-other`, `--chart-escalated`; Abbruchgründe
`--reason-max-turns`, `--reason-timeout`, `--reason-context`, `--reason-exit-code`,
`--reason-claude-error`, `--reason-no-outcome`, `--reason-rest`.

**Schatten:** `--shadow-faint`, `--shadow-mid`, `--shadow-soft`, `--shadow-tab`, `--shadow-strong`,
`--backdrop` (Abdunkelung hinter Dialogen), `--sheet-shadow`, `--picker-shadow`.

**Formen und Maße:** `--radius-control`, `--radius-card`, `--radius-panel`, `--control-height`,
`--sidebar-width`, `--page-width`, `--page-width-narrow`, `--gutter`.

## Versionierung

Das SDK folgt ab 1.0.0 semver. `SDK_VERSION` ist immer gleich der Version in `package.json`.

- **Major:** Bruch an Slot-, Props-, Hook- oder Baustein-Typen (Feld entfernt oder umbenannt, Typ
  verengt, Slot entfernt) oder an CSS-Variablen.
- **Minor:** Neue Slots, Hooks, Bausteine, optionale Felder oder CSS-Variablen.
- **Patch:** Doku und Fehlerbehebungen ohne Änderung der Typen.

Änderungen:

| Version | Änderung |
|---|---|
| 1.2.0 | neu: `useConnection`, `ConnectionState`, `UsageAccount` sowie `accounts` und `defaultLimit` in `UsageState` |
| 1.1.0 | neu: `useThemeAsset`, `themeAssetUrl` und die Typen `ThemeAssetOptions`, `ThemeAssetState`, `ThemeAssetError` |
| 1.0.0 | erste stabile API |

Der Host lädt nur Themes, deren `sdkVersion`-Bereich die aktuelle `SDK_VERSION` enthält; andere
werden nicht geladen und im Store als unpassend ausgelassen. Empfehlung: `"sdkVersion": "^1.0.0"`.
