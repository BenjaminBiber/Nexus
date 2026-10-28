// Bearbeitet TOML zeilenweise, ohne Bibliothek: Abschnitt merken, nur die gesuchte Zeile ersetzen, alles andere
// bleibt stehen (Kommentare, Reihenfolge, Leerzeilen). Gelesen wird nur, was dieses Skript braucht –
// Zeichenketten, `true`/`false` und einfache Listen aus Zeichenketten.

// `version = "0.1.0"` oder `version.workspace = true`, auch mit Kommentar dahinter
const KEY_LINE = /^\s*((?:[A-Za-z0-9_-]+|"[^"]*")(?:\.(?:[A-Za-z0-9_-]+|"[^"]*"))*)\s*=\s*(.*)$/;
// `[abschnitt]` oder `[[abschnitt]]`
const SECTION_LINE = /^\s*(\[\[?)\s*([^\]]+?)\s*(\]\]?)\s*(?:#.*)?$/;
// Eine Zeichenkette am Anfang eines Werts
const STRING_VALUE = /^"((?:[^"\\]|\\.)*)"/;

// TOML-Text mit gezielten Änderungen an einzelnen Schlüsseln.
export class TomlText {
  constructor(text) {
    // Zeilenende der Datei; gemischte Zeilenenden werden auf das häufigere vereinheitlicht
    this.eol = text.includes('\r\n') ? '\r\n' : '\n';
    this.lines = text.split(/\r?\n/);
  }

  // Der aktuelle Text
  get text() {
    return this.lines.join(this.eol);
  }

  // Alle Abschnitte der Datei: Name, ob Tabellen-Liste (`[[…]]`) und die Zeilen dazwischen
  sections() {
    const found = [{ name: '', array: false, from: 0, to: this.lines.length }];
    for (let index = 0; index < this.lines.length; index += 1) {
      const match = SECTION_LINE.exec(this.lines[index]);
      if (!match) continue;
      found[found.length - 1].to = index;
      found.push({ name: match[2], array: match[1] === '[[', from: index + 1, to: this.lines.length });
    }
    return found;
  }

  // Gibt es diesen Abschnitt?
  hasSection(name) {
    return this.sections().some((section) => section.name === name);
  }

  // Nummer der Zeile mit diesem Schlüssel im Abschnitt; -1, wenn es sie nicht gibt
  lineOf(section, key, range = null) {
    const area = range ?? this.sections().find((entry) => entry.name === section);
    if (!area) return -1;
    for (let index = area.from; index < area.to; index += 1) {
      const match = KEY_LINE.exec(this.lines[index]);
      if (match && match[1] === key) return index;
    }
    return -1;
  }

  // Wert eines Schlüssels als Rohtext ohne einen Kommentar dahinter; `undefined`, wenn es ihn nicht gibt
  raw(section, key, range = null) {
    const index = this.lineOf(section, key, range);
    if (index === -1) return undefined;
    const after = KEY_LINE.exec(this.lines[index])[2].trim();
    // Eine Zeichenkette darf ein `#` enthalten, ein nackter Wert nicht
    return after.startsWith('"') ? after : after.split('#')[0].trim();
  }

  // Wert eines Schlüssels als Zeichenkette; `undefined`, wenn es ihn nicht gibt oder er keine ist
  string(section, key, range = null) {
    const match = STRING_VALUE.exec(this.raw(section, key, range) ?? '');
    return match ? JSON.parse(`"${match[1]}"`) : undefined;
  }

  // Setzt eine vorhandene Zeichenkette; wirft, wenn es den Schlüssel nicht gibt
  setString(section, key, value, range = null) {
    const index = this.lineOf(section, key, range);
    if (index === -1) throw new Error(`„${key}“ gibt es im Abschnitt [${section}] nicht.`);
    const line = this.lines[index];
    const after = KEY_LINE.exec(line)[2];
    const start = line.length - after.length;
    const match = STRING_VALUE.exec(after);
    if (!match) throw new Error(`„${key}“ im Abschnitt [${section}] ist keine Zeichenkette.`);
    // Nur die Zeichenkette selbst tauschen: Einrückung, Schlüssel und ein Kommentar dahinter bleiben
    this.lines[index] = line.slice(0, start) + JSON.stringify(value) + line.slice(start + match[0].length);
    return this;
  }

  // Zeichenketten einer Liste (`members = ["a", "b"]`), auch über mehrere Zeilen
  array(section, key) {
    const index = this.lineOf(section, key);
    if (index === -1) return [];
    let text = KEY_LINE.exec(this.lines[index])[2];
    // Weiterlesen, bis die Klammer zu ist – Listen stehen oft über mehrere Zeilen
    for (let next = index + 1; next < this.lines.length && !text.includes(']'); next += 1) text += `\n${this.lines[next]}`;
    const list = [];
    for (const match of text.matchAll(/"((?:[^"\\]|\\.)*)"/g)) list.push(JSON.parse(`"${match[1]}"`));
    return list;
  }

  // Abschnitte einer Tabellen-Liste (`[[package]]`) als { name, from, to } – für Cargo.lock.
  // `name` ist der Wert des Schlüssels `name` darin, nicht der Name des Abschnitts.
  tables(section, nameKey = 'name') {
    return this.sections()
      .filter((entry) => entry.array && entry.name === section)
      .map((entry) => ({ name: this.string(section, nameKey, entry), from: entry.from, to: entry.to }));
  }
}
