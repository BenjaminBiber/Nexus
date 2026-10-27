export type PromptValues = Record<string, string | string[]>;

// Platzhalter-Form: {{name}}, keine Leerzeichen innerhalb der Klammern.
const PLACEHOLDER = /\{\{([A-Za-z][A-Za-z0-9_]*)\}\}/g;
// Zeile, deren einziger Inhalt ein Platzhalter ist (Block-Zeile).
const BLOCK_LINE = /^(\s*)\{\{([A-Za-z][A-Za-z0-9_]*)\}\}(\s*)$/;

// Vorlagen-Engine für die Dispatcher-Prompts: ersetzt {{name}}, meldet unbekannte Platzhalter
// und lässt Zeilen wegfallen, die nur aus einem leeren Block-Platzhalter bestehen.
export class PromptTemplate {
  // Platzhalternamen der Vorlage in Reihenfolge des ersten Vorkommens, ohne Dubletten
  names(template: string): string[] {
    const found: string[] = [];
    for (const match of template.matchAll(PLACEHOLDER)) {
      if (!found.includes(match[1])) found.push(match[1]);
    }
    return found;
  }

  // Setzt die Werte ein; unbekannte Platzhalter führen zu einem Fehler, der alle fehlenden nennt
  render(template: string, values: PromptValues): string {
    const unknown = this.names(template).filter((name) => !(name in values));
    if (unknown.length > 0) {
      throw new Error(`Unbekannte Platzhalter in der Vorlage: ${unknown.map((n) => `{{${n}}}`).join(', ')}`);
    }

    const out: string[] = [];
    // Jede Zeile mit ihrem ursprünglichen Zeilenende, damit \r\n erhalten bleibt.
    for (const part of template.match(/[^\r\n]*(?:\r\n|\n|\r)?/g) ?? []) {
      if (part === '') continue;
      const end = part.match(/(?:\r\n|\n|\r)$/)?.[0] ?? '';
      const line = part.slice(0, part.length - end.length);

      const block = BLOCK_LINE.exec(line);
      if (block && this.value(values[block[2]]) === '') continue;

      out.push(line.replace(PLACEHOLDER, (_all, name: string) => this.value(values[name])) + end);
    }
    return out.join('');
  }

  // Array-Werte werden mit Zeilenumbruch verbunden; ein leeres Array zählt als leerer Wert
  private value(value: string | string[]): string {
    return Array.isArray(value) ? value.join('\n') : value;
  }
}
