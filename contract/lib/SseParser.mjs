// Zerlegt den Textstrom eines Ereignisstroms (Server-Sent Events) in Einträge.
// Ein Block endet mit einer leeren Zeile; `data:` liefert das JSON des Ereignisses, eine Zeile mit `:`
// einen Kommentar (`: ping` → `{ comment: 'ping' }`). Blöcke dürfen über mehrere Chunks verteilt sein.
export class SseParser {
  constructor() {
    this.rest = '';
  }

  // Nimmt den nächsten Teil des Stroms und liefert alle damit vollständigen Einträge
  push(chunk) {
    this.rest += chunk.replace(/\r\n/g, '\n');
    const blocks = this.rest.split('\n\n');
    // Der letzte Teil ist noch nicht durch eine leere Zeile abgeschlossen
    this.rest = blocks.pop();
    const out = [];
    for (const block of blocks) out.push(...SseParser.entries(block));
    return out;
  }

  // Die Einträge eines abgeschlossenen Blocks; ein Block ohne `data` und ohne Kommentar liefert nichts
  static entries(block) {
    const out = [];
    const data = [];
    for (const line of block.split('\n')) {
      if (line.startsWith(':')) out.push({ comment: line.slice(1).trim() });
      else if (line.startsWith('data:')) data.push(line.slice('data:'.length).trim());
    }
    if (data.length) out.push(SseParser.payload(data.join('\n')));
    return out;
  }

  // Der Inhalt eines `data`-Blocks; der Vertrag schickt immer JSON, alles andere bleibt als Text sichtbar
  static payload(text) {
    try {
      return JSON.parse(text);
    } catch {
      return { data: text };
    }
  }
}
