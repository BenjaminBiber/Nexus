import type { HarnessError } from '@harness/core';

// Sperren, die sich in der CLI mit --force übergehen lassen, und der Hinweis dazu; die Meldung aus dem Kern bleibt
// neutral (auch fürs Web)
const FORCE_HINTS: Record<string, string> = {
  'question-open': 'Mit --force übergehen.',
  'dependency-open': 'Mit --force übergehen.',
  'ticket-duplicate': 'Mit --force trotzdem anlegen.',
};

// Text eines erwartbaren Fehlers für die Kommandozeile: Kernmeldung plus CLI-eigene Hinweise.
export class CliErrorText {
  // Meldung mit CLI-Hinweisen, ohne Code
  of(error: HarnessError): string {
    const hint = FORCE_HINTS[error.code];
    return hint ? `${error.message} ${hint}` : error.message;
  }

  // Klartext für stderr: Meldung und Code in eckigen Klammern, z. B. „… [board-repo-mismatch]“
  line(error: HarnessError): string {
    return `${this.of(error)} [${error.code}]`;
  }
}
