import { parse } from 'yaml';

export interface FrontMatterResult {
  /** Felder des Kopfes; ohne oder mit kaputtem Kopf ein leeres Objekt */
  fields: Record<string, unknown>;
  /** Erste Zeile der YAML-Fehlermeldung, wenn der Kopf vorhanden, aber nicht lesbar ist; sonst null */
  error: string | null;
}

// Liest den YAML-Kopf einer Markdown-Datei (zwischen den ersten beiden Zeilen „---“).
export class FrontMatter {
  // Felder des Kopfes; ohne oder mit kaputtem Kopf ein leeres Objekt
  read(text: string): Record<string, unknown> {
    return this.parse(text).fields;
  }

  // Felder des Kopfes und, falls das YAML nicht lesbar ist, die erste Zeile des Fehlers
  parse(text: string): FrontMatterResult {
    const match = /^---\r?\n([\s\S]*?)\r?\n---(\r?\n|$)/.exec(text);
    if (!match) return { fields: {}, error: null };
    try {
      const data: unknown = parse(match[1]);
      const fields = data && typeof data === 'object' && !Array.isArray(data) ? (data as Record<string, unknown>) : {};
      return { fields, error: null };
    } catch (error) {
      return { fields: {}, error: (error as Error).message.split(/\r?\n/)[0] };
    }
  }

  // Textfeld des Kopfes getrimmt; leer, fehlend oder kein Text ergibt null
  text(fields: Record<string, unknown>, key: string): string | null {
    const value = fields[key];
    return typeof value === 'string' && value.trim() ? value.trim() : null;
  }
}
