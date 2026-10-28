// Ändert einen Wert in JSON-Text an genau seiner Stelle und lässt den Rest der Datei Zeichen für Zeichen stehen.
// `JSON.parse` + `JSON.stringify` wäre kürzer, formatiert aber die ganze Datei neu (einzeilige Objekte wie
// `"author": { "name": "User" }` würden auseinandergezogen, Einrückungen vereinheitlicht). Ein Versionswechsel
// soll im Diff nur die Versionszeilen zeigen, deshalb dieser Weg.

// Pfad einer Stelle als Schlüssel einer Map; als JSON-Text, damit Schlüssel mit Sonderzeichen eindeutig bleiben
const asKey = (path) => JSON.stringify(path);

// Liest den Text einmal durch und merkt sich zu jedem Mitglied (Objektschlüssel, Array-Eintrag) seine Textstelle
function scan(text) {
  const members = new Map();
  let at = 0;

  const fail = (what) => {
    throw new Error(`JSON lässt sich nicht lesen (${what} an Zeichen ${at}).`);
  };

  const skipSpace = () => {
    while (at < text.length && (text[at] === ' ' || text[at] === '\t' || text[at] === '\n' || text[at] === '\r')) at += 1;
  };

  // Steht am öffnenden Anführungszeichen und liefert den gelesenen Text
  const readString = () => {
    const start = at;
    at += 1;
    while (at < text.length) {
      if (text[at] === '\\') at += 2;
      else if (text[at] === '"') {
        at += 1;
        return JSON.parse(text.slice(start, at));
      } else at += 1;
    }
    return fail('Zeichenkette ohne Ende');
  };

  // Zahl, true, false, null – alles bis zum nächsten Trennzeichen
  const readLiteral = () => {
    const start = at;
    while (at < text.length && !',}] \t\n\r'.includes(text[at])) at += 1;
    const raw = text.slice(start, at);
    try {
      return JSON.parse(raw);
    } catch {
      return fail(`unbekannter Wert „${raw}“`);
    }
  };

  const readValue = (path) => {
    skipSpace();
    const start = at;
    const char = text[at];
    let value;
    if (char === undefined) return fail('Wert erwartet');
    else if (char === '{') value = readObject(path);
    else if (char === '[') value = readArray(path);
    else if (char === '"') value = readString();
    else value = readLiteral();
    return { value, start, end: at };
  };

  // Merkt sich ein Mitglied samt seiner Textstelle
  const remember = (path, keyStart, keyEnd, read) => {
    members.set(asKey(path), { path, keyStart, keyEnd, valueStart: read.start, valueEnd: read.end, value: read.value });
  };

  function readObject(path) {
    at += 1;
    const object = {};
    skipSpace();
    if (text[at] === '}') {
      at += 1;
      return object;
    }
    for (;;) {
      skipSpace();
      if (text[at] !== '"') return fail('Schlüssel erwartet');
      const keyStart = at;
      const name = readString();
      const keyEnd = at;
      skipSpace();
      if (text[at] !== ':') return fail('Doppelpunkt erwartet');
      at += 1;
      const inner = [...path, name];
      const read = readValue(inner);
      remember(inner, keyStart, keyEnd, read);
      object[name] = read.value;
      skipSpace();
      if (text[at] === ',') {
        at += 1;
        continue;
      }
      if (text[at] === '}') {
        at += 1;
        return object;
      }
      return fail('Komma oder } erwartet');
    }
  }

  function readArray(path) {
    at += 1;
    const list = [];
    skipSpace();
    if (text[at] === ']') {
      at += 1;
      return list;
    }
    for (;;) {
      const inner = [...path, list.length];
      const read = readValue(inner);
      // Ein Array-Eintrag hat keinen Schlüssel; als Stelle gilt der Anfang seines Werts
      remember(inner, read.start, read.start, read);
      list.push(read.value);
      skipSpace();
      if (text[at] === ',') {
        at += 1;
        continue;
      }
      if (text[at] === ']') {
        at += 1;
        return list;
      }
      return fail('Komma oder ] erwartet');
    }
  }

  const root = readValue([]);
  skipSpace();
  if (at !== text.length) fail('überzähliger Text hinter dem Ende');
  return { members, root };
}

// JSON-Text mit gezielten Änderungen an einzelnen Werten.
export class JsonText {
  constructor(text) {
    // Zeilenende der Datei – eingefügte Zeilen sollen dazu passen
    this.eol = text.includes('\r\n') ? '\r\n' : '\n';
    this.read(text);
  }

  // Liest den Text ein; nach jeder Änderung erneut, damit die Textstellen stimmen
  read(text) {
    const scanned = scan(text);
    this.text = text;
    this.members = scanned.members;
    this.root = scanned.root;
  }

  // Der gelesene Inhalt als gewöhnliches Objekt
  get data() {
    return this.root.value;
  }

  // Gibt es die Stelle?
  has(path) {
    return this.members.has(asKey(path));
  }

  // Wert einer Stelle; `undefined`, wenn es sie nicht gibt
  value(path) {
    return this.members.get(asKey(path))?.value;
  }

  // Ersetzt eine Zeichenkette an ihrer Stelle
  setString(path, value) {
    const member = this.member(path);
    this.replace(member.valueStart, member.valueEnd, JSON.stringify(value));
    return this;
  }

  // Fügt `"name": "wert"` hinter einem vorhandenen Mitglied ein; Einrückung wie dieses Mitglied
  insertStringAfter(path, name, value) {
    const member = this.member(path);
    const indent = this.indentOf(member.keyStart);
    this.replace(member.valueEnd, member.valueEnd, `,${this.eol}${indent}${JSON.stringify(name)}: ${JSON.stringify(value)}`);
    return this;
  }

  // Entfernt ein Mitglied samt dem Komma, das es von seinen Geschwistern trennt
  remove(path) {
    const member = this.member(path);
    const siblings = this.children(path.slice(0, -1));
    const previous = siblings[siblings.findIndex((entry) => asKey(entry.path) === asKey(path)) - 1];
    let to = member.valueEnd;
    if (previous) {
      // Nach einem Geschwister steht das Komma davor und verschwindet mit der Zeile
      this.replace(previous.valueEnd, to, '');
      return this;
    }
    // Als erstes Mitglied gehört das Komma dahinter dazu
    while (to < this.text.length && ' \t'.includes(this.text[to])) to += 1;
    if (this.text[to] === ',') to += 1;
    this.replace(this.spanOf(path.slice(0, -1)).valueStart + 1, to, '');
    return this;
  }

  // Mitglieder eines Objekts oder Arrays in der Reihenfolge der Datei
  children(path) {
    const prefix = asKey(path);
    const depth = path.length + 1;
    return [...this.members.values()]
      .filter((member) => member.path.length === depth && asKey(member.path.slice(0, path.length)) === prefix)
      .sort((a, b) => a.valueStart - b.valueStart);
  }

  // Die Stelle eines Mitglieds; wirft, wenn es sie nicht gibt
  member(path) {
    const member = this.members.get(asKey(path));
    if (!member) throw new Error(`Die Stelle „${path.join('.')}“ gibt es in dieser Datei nicht.`);
    return member;
  }

  // Wie member, aber der leere Pfad meint den ganzen Inhalt der Datei
  spanOf(path) {
    return path.length === 0 ? { valueStart: this.root.start, valueEnd: this.root.end } : this.member(path);
  }

  // Einrückung der Zeile, in der diese Stelle beginnt
  indentOf(start) {
    let at = start;
    while (at > 0 && ' \t'.includes(this.text[at - 1])) at -= 1;
    return this.text.slice(at, start);
  }

  // Ersetzt einen Textabschnitt und liest danach neu ein
  replace(from, to, insert) {
    this.read(this.text.slice(0, from) + insert + this.text.slice(to));
  }
}
