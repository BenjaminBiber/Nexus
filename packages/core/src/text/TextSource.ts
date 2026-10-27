import { readFileSync } from 'node:fs';
import { HarnessError } from '../errors/HarnessError.js';

// Text aus einer Option oder einer Datei („-“ = Standardeingabe); für CLI-Schalter wie --plan/--plan-file.
export class TextSource {
  // Datei hat Vorrang vor dem Inline-Text; fehlt beides, bleibt der Wert undefined
  read(inline: string | undefined, file: string | undefined): string | undefined {
    if (file === undefined) return inline;
    try {
      return readFileSync(file === '-' ? 0 : file, 'utf8');
    } catch {
      throw new HarnessError('input-unreadable', `Die Eingabe ${file === '-' ? 'von der Standardeingabe' : file} lässt sich nicht lesen.`);
    }
  }
}
