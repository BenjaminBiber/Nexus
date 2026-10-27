// Gibt Ergebnisse als Text oder, mit --json, als JSON aus.
export class Printer {
  constructor(private readonly json: boolean) {}

  // Gibt ein Ergebnis aus: JSON-Objekt im JSON-Modus, sonst die Textzeilen
  result(data: unknown, lines: string[]): void {
    if (this.json) console.log(JSON.stringify(data, null, 2));
    else for (const line of lines) console.log(line);
  }

  // Gibt eine Warnung auf stderr aus (auch im JSON-Modus, damit stdout sauber bleibt)
  warn(message: string): void {
    console.error(`Warnung: ${message}`);
  }

  // Gibt einen Fehler aus: im JSON-Modus `{"error":{"code","message"}}` auf stdout, sonst `text` auf stderr
  failure(code: string, message: string, text: string): void {
    if (this.json) console.log(JSON.stringify({ error: { code, message } }, null, 2));
    else console.error(text);
  }
}
