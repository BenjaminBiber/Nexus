const PLACEHOLDER = /\{\{([A-Za-z][A-Za-z0-9_]*)\}\}/g;

// Die Werte, die ein Szenario in Pfade, Körper und Köpfe einsetzen kann (`{{REPO1}}`, `{{HOME}}`, `{{PORT}}`, `save`).
export class Variables {
  constructor(values = {}) {
    this.values = { ...values };
  }

  // Legt einen Wert ab (aus `save` eines Schritts)
  set(name, value) {
    this.values[name] = value;
  }

  // Alle Werte als einfaches Objekt (für den Normalisierer)
  get all() {
    return { ...this.values };
  }

  // Ersetzt `{{NAME}}` in Texten, Listen und Objekten; besteht ein Text nur aus einem Platzhalter,
  // bleibt der Wert sein eigener Typ (Zahl, Objekt)
  apply(input) {
    if (typeof input === 'string') {
      const single = input.match(/^\{\{([A-Za-z][A-Za-z0-9_]*)\}\}$/);
      if (single) return this.lookup(single[1]);
      return input.replace(PLACEHOLDER, (_match, name) => String(this.lookup(name)));
    }
    if (Array.isArray(input)) return input.map((entry) => this.apply(entry));
    if (input !== null && typeof input === 'object') {
      const out = {};
      for (const [key, value] of Object.entries(input)) out[this.apply(key)] = this.apply(value);
      return out;
    }
    return input;
  }

  // Liefert einen Wert; ein unbekannter Name ist ein Fehler im Szenario
  lookup(name) {
    if (!(name in this.values)) {
      throw new Error(`Unbekannte Variable {{${name}}} (bekannt: ${Object.keys(this.values).sort().join(', ')}).`);
    }
    return this.values[name];
  }

  // Liest einen Wert über Punkt-Notation aus einem Eintrag (`json.id`, `json.items.0.id`, `headers.location`)
  static pick(source, path) {
    let current = source;
    for (const part of path.split('.')) {
      if (current === null || current === undefined) return undefined;
      current = current[part];
    }
    return current;
  }

  // Entfernt einen Wert über Punkt-Notation (für `ignore`)
  static drop(source, path) {
    const parts = path.split('.');
    const last = parts.pop();
    let current = source;
    for (const part of parts) {
      if (current === null || typeof current !== 'object') return;
      current = current[part];
    }
    if (current !== null && typeof current === 'object') delete current[last];
  }
}
