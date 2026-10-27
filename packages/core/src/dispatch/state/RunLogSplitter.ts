/** Zeile im Lauf-Protokoll vor der Arbeitsanweisung */
export const RUN_LOG_PROMPT_MARKER = '[dispatch] Arbeitsanweisung:';
/** Zeile im Lauf-Protokoll zwischen Arbeitsanweisung und Ausgabe von Claude */
export const RUN_LOG_OUTPUT_MARKER = '[dispatch] Ausgabe:';

/** Teile eines Lauf-Protokolls */
export interface RunLogParts {
  /** Kopf des Dispatchers (Start, Profil, Aufruf) */
  header: string;
  /** Arbeitsanweisung; null, wenn sie nicht (mehr) im Text steht, z. B. weil nur das Ende gelesen wurde */
  prompt: string | null;
  /** Ausgabe von Claude und Ende des Laufs */
  body: string;
}

// Erste Ausgabe in Protokollen ohne Marker: Zeile mit Uhrzeit (stream-json), JSON (älteres json-Format) oder Meldung des Dispatchers
const FIRST_OUTPUT = /^(\d\d:\d\d:\d\d {2}|\{|\[dispatch\] )/;

// Zerlegt ein Lauf-Protokoll in Kopf, Arbeitsanweisung und Ausgabe, damit die Oberfläche die Anweisung getrennt zeigen kann.
export class RunLogSplitter {
  // Teile des Textes; ohne Anweisung ist alles Ausgabe
  split(text: string): RunLogParts {
    const lines = text.split('\n');
    const clean = lines.map((l) => l.replace(/\r$/, ''));
    const start = clean.indexOf(RUN_LOG_PROMPT_MARKER);
    if (start < 0) return { header: '', prompt: null, body: text };
    let end = clean.indexOf(RUN_LOG_OUTPUT_MARKER, start + 1);
    let bodyFrom = end + 1;
    if (end < 0) {
      const found = clean.findIndex((l, i) => i > start && FIRST_OUTPUT.test(l));
      end = found < 0 ? clean.length : found;
      bodyFrom = end;
    }
    return {
      header: clean.slice(0, start).join('\n').trimEnd(),
      prompt: clean.slice(start + 1, end).join('\n').replace(/\s+$/, ''),
      body: lines.slice(bodyFrom).join('\n'),
    };
  }
}
