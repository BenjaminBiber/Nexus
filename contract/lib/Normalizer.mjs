import { realpathSync } from 'node:fs';

// Zeitstempel mit Zone (`2026-10-27T18:05:00.123Z`, `…+02:00`)
const ISO_TIME = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})/g;
// Datum und Uhrzeit, deutsch (`27.10.2026, 18:05`): so gibt die CLI Kommentare und Verlauf aus
const GERMAN_TIME = /(?<!\d)\d{2}\.\d{2}\.\d{4}, \d{2}:\d{2}(?::\d{2})?(?!\d)/g;
const UUID = /[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}/g;
// Datum `2026-09-02` für sich, nicht als Teil einer längeren Zahlenfolge
const DATE = /(?<!\d)\d{4}-\d{2}-\d{2}(?!\d)/g;
// Weiter zurückliegende Tage sind keine berechneten Grenzen mehr, sondern feste Daten der Szenarien
const MAX_RELATIVE_DAYS = 366;
// Diese GUID steht fest in den Szenarien (Benutzer der Board-Konfiguration) und bleibt lesbar
const STABLE_UUID = '11111111-1111-1111-1111-111111111111';
// Rest eines Pfades hinter einem Platzhalter (`<HOME>\board.db`, `<REPO1>\\.git`); Leerzeichen beenden ihn
const PATH_TAIL = /(<[A-Z][A-Z0-9_]*>)((?:\\{1,2}|\/)(?:[^\s"'<>]|\\{1,2})*)/g;
// Felder, in denen ein Hex-Wort ein Git-Commit ist
const SHA_FIELDS = new Set(['sha', 'commit', 'head']);
const SHA_FULL = /\b[0-9a-f]{40}\b/g;
const SHA_SHORT = /\b[0-9a-f]{7,12}\b/g;
// Zahlenfelder, die je Lauf anders aussehen
const ZERO_FIELDS = new Set(['pid', 'port', 'durationMs']);
// Mindestlänge eines gespeicherten Werts; kurze Werte („1“, „ok“) würden überall ersetzt
const MIN_SAVED = 6;

// Ersetzt die je Lauf wechselnden Werte einer Antwort durch feste Platzhalter (reine Funktionen, kein Zustand).
export class Normalizer {
  // `paths`: Name → Ordner (`REPO1`, `HOME`, `TMP`), `saved`: Name → gespeicherter Wert,
  // `port`: Port des Servers, `today`: heutiges Datum als `YYYY-MM-DD`
  constructor(context = {}) {
    this.port = context.port ?? null;
    this.today = context.today ?? Normalizer.today();
    this.saved = context.saved ?? {};
    this.pathRules = Normalizer.pathRules(context.paths ?? {});
  }

  // Heutiges Datum in Ortszeit als `YYYY-MM-DD`
  static today() {
    const now = new Date();
    const pad = (value) => String(value).padStart(2, '0');
    return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  }

  // Ersetzungsregeln für alle Ordner, längster Pfad zuerst (damit HOME vor TMP greift)
  static pathRules(paths) {
    const rules = [];
    for (const [name, dir] of Object.entries(paths)) {
      if (typeof dir !== 'string' || !dir) continue;
      for (const from of Normalizer.spellings(dir)) rules.push({ from, to: `<${name}>` });
    }
    return rules.sort((a, b) => b.from.length - a.from.length);
  }

  // Alle Schreibweisen eines Ordners: nativ, mit Schrägstrichen, JSON-escaped; dazu die aufgelöste Form
  // (macOS legt temporäre Ordner unter /var ab, `realpath` meldet /private/var)
  static spellings(dir) {
    const bases = new Set([dir]);
    try {
      bases.add(realpathSync(dir));
    } catch {
      /* Ordner kann schon gelöscht sein */
    }
    for (const base of [...bases]) {
      if (base.startsWith('/private/var/')) bases.add(base.slice('/private'.length));
    }
    const out = new Set();
    for (const base of bases) {
      out.add(base);
      out.add(base.replace(/\\/g, '/'));
      out.add(base.replace(/\\/g, '\\\\'));
    }
    return [...out];
  }

  // Normalisiert einen beliebigen Wert; `key` ist der Feldname, unter dem er steht
  value(input, key = '') {
    if (typeof input === 'string') return this.text(input, key);
    if (typeof input === 'number') return this.number(input, key);
    if (Array.isArray(input)) return input.map((entry) => this.value(entry, key));
    if (input !== null && typeof input === 'object') {
      const out = {};
      for (const [name, entry] of Object.entries(input)) out[this.text(name, '')] = this.value(entry, name);
      return out;
    }
    return input;
  }

  // Alle Regeln auf einen Text, in der Reihenfolge des Vertrags
  text(input, key = '') {
    let out = this.savedValues(input);
    out = this.paths(out);
    out = this.ports(out);
    out = this.times(out);
    out = this.relativeDates(out);
    out = this.uuids(out);
    return this.shas(out, key);
  }

  // 1. Gespeicherte Werte aus `save` → `<var:name>`
  savedValues(input) {
    let out = input;
    for (const [name, value] of Object.entries(this.saved)) {
      if (typeof value !== 'string' || value.length < MIN_SAVED) continue;
      out = out.split(value).join(`<var:${name}>`);
    }
    return out;
  }

  // 2. Repo-Pfade, Home und Temp-Ordner → `<REPO1>`, `<HOME>`, `<TMP>`; der Rest des Pfades
  // bekommt Schrägstriche, damit die Golden-Datei unter Windows dieselbe ist
  paths(input) {
    let out = input;
    for (const rule of this.pathRules) out = out.split(rule.from).join(rule.to);
    return out.replace(PATH_TAIL, (_match, name, tail) => name + tail.replace(/\\{1,2}/g, '/'));
  }

  // 3. Port des Servers → `<PORT>`
  ports(input) {
    if (this.port === null) return input;
    return input.replace(new RegExp(`(127\\.0\\.0\\.1|localhost):${this.port}\\b`, 'g'), '$1:<PORT>');
  }

  // 4. Zeitstempel → `<TIME>`, auch als deutsches Datum mit Uhrzeit; heutiges Datum → `<DATE>`
  times(input) {
    const out = input.replace(ISO_TIME, '<TIME>').replace(GERMAN_TIME, '<TIME>');
    return out.split(this.today).join('<DATE>');
  }

  // 5. Datum vor heute, höchstens ein Jahr zurück → `<DATE-55>`: Zeiträume, die der Server aus „heute“ berechnet
  // (etwa der Anfang der Standard-Auswertung), bleiben so über Tage hinweg dieselbe Golden-Datei
  relativeDates(input) {
    return input.replace(DATE, (match) => {
      const days = this.daysBefore(match);
      return days === null ? match : `<DATE-${days}>`;
    });
  }

  // Ganze Tage zwischen einem Datum und heute; null, wenn es kein Tag im letzten Jahr ist
  daysBefore(date) {
    const day = Date.parse(`${date}T00:00:00Z`);
    const today = Date.parse(`${this.today}T00:00:00Z`);
    if (Number.isNaN(day) || Number.isNaN(today)) return null;
    const days = (today - day) / 86_400_000;
    return Number.isInteger(days) && days >= 1 && days <= MAX_RELATIVE_DAYS ? days : null;
  }

  // 6. UUIDs außer der festen GUID der Szenarien → `<UUID>`
  uuids(input) {
    return input.replace(UUID, (match) => (match.toLowerCase() === STABLE_UUID ? match : '<UUID>'));
  }

  // 7. Git-Commits in Feldern `sha`, `commit`, `head` → `<SHA>`
  shas(input, key) {
    if (!SHA_FIELDS.has(key)) return input;
    return input.replace(SHA_FULL, '<SHA>').replace(SHA_SHORT, '<SHA>');
  }

  // 8. Zahlenfelder auf `Ms` oder `At` sowie `durationMs`, `pid`, `port` → 0
  number(input, key) {
    return ZERO_FIELDS.has(key) || /(?:Ms|At)$/.test(key) ? 0 : input;
  }
}
