const PREFIX = /^SE-\d+/;
// Geschlossene Blöcke, die nicht vom Nutzer stammen: system-reminder, browser_instruction, ide_*, command-*, local-command-*
const INJECTED_BLOCK = /<(system-reminder|browser_instruction|ide_[\w-]+|command-[\w-]+|local-command-[\w-]+)(?:\s[^>]*)?>[\s\S]*?<\/\1>/g;

// Bildet den Sitzungsnamen „SE-001: Titel“ (nur „SE-001“, wenn es noch keinen Titel gibt).
export class SessionTitle {
  // Setzt die Nummer (dreistellig) vor den Titel; ein bereits nummerierter Titel bleibt unverändert
  format(number: number, title?: string): string {
    const text = title?.trim();
    if (text && PREFIX.test(text)) return text;
    const label = `SE-${String(number).padStart(3, '0')}`;
    return text ? `${label}: ${text}` : label;
  }

  // Entfernt von Claude Code oder der IDE eingefügte Blöcke (z. B. <system-reminder>), damit sie nie im Titel landen
  private stripInjected(prompt: string): string {
    return prompt.replace(INJECTED_BLOCK, '');
  }

  // Kurzer Titel aus dem Prompt: erste Zeile, die ersten Wörter, ohne Zeilenumbrüche; leer, wenn nichts übrig bleibt
  fromPrompt(prompt: string | undefined, maxWords = 6, maxLength = 50): string {
    const firstLine = this.stripInjected(prompt ?? '').split(/\r?\n/).find((line) => line.trim()) ?? '';
    const words = firstLine.trim().split(/\s+/).filter(Boolean);
    const text = words.slice(0, maxWords).join(' ');
    const cut = text.length > maxLength ? text.slice(0, maxLength).trimEnd() : text;
    return cut && (words.length > maxWords || cut.length < text.length) ? `${cut} …` : cut;
  }
}
