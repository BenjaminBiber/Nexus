import { readFileSync } from 'node:fs';
import { HarnessError } from '../errors/HarnessError.js';
import type { ResearchInput } from './ResearchValidator.js';

// Liest die Eingabe einer Recherche aus einer JSON-Datei, der Standardeingabe oder einem Kontexttext.
export class ResearchInputReader {
  // Genau eine Quelle ist erlaubt: `from` (Datei oder „-“ für die Standardeingabe) oder `context`
  read(options: { from?: string; context?: string }): ResearchInput {
    if (options.from && options.context) {
      throw new HarnessError('research-input', 'Bitte entweder --from oder --context angeben, nicht beides.');
    }
    if (options.context) return { context: options.context };
    if (!options.from) {
      throw new HarnessError('research-input', 'Bitte --from <datei|-> oder --context <text> angeben.');
    }
    const text = this.readSource(options.from);
    try {
      return JSON.parse(text) as ResearchInput;
    } catch {
      throw new HarnessError('research-input', 'Die Eingabe ist kein gültiges JSON.');
    }
  }

  // Liest eine Datei oder die Standardeingabe
  private readSource(source: string): string {
    try {
      return readFileSync(source === '-' ? 0 : source, 'utf8');
    } catch {
      throw new HarnessError('research-input', `Die Eingabe ${source === '-' ? 'von der Standardeingabe' : source} lässt sich nicht lesen.`);
    }
  }
}
