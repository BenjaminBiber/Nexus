// Erwartbarer Fehler mit deutscher Meldung; das CLI gibt ihn als Rückgabecode 1 aus.
export class HarnessError extends Error {
  readonly code: string;
  /** Zusätzliche Angaben für Aufrufer (z. B. der Kreis einer Abhängigkeit) */
  readonly details: Record<string, unknown> | undefined;

  // Erzeugt den Fehler mit einem stabilen Code (für Tests und --json), der Meldung und optionalen Angaben
  constructor(code: string, message: string, details?: Record<string, unknown>) {
    super(message);
    this.name = 'HarnessError';
    this.code = code;
    this.details = details;
  }
}
