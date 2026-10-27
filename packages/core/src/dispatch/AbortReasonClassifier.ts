export type AbortCategory =
  | 'timeout' | 'max-turns' | 'context' | 'exit-code' | 'claude-error' | 'no-outcome' | 'usage-limit' | 'start-failed' | 'vanished' | 'other';

const LABELS: Record<AbortCategory, string> = {
  'timeout': 'Zeitlimit',
  'max-turns': 'Schrittlimit',
  'context': 'Kontext voll',
  'exit-code': 'Exit-Code',
  'claude-error': 'Fehler von Claude',
  'no-outcome': 'Ohne Ausgang',
  'usage-limit': 'Nutzungslimit',
  'start-failed': 'Start gescheitert',
  'vanished': 'Worker verschwunden',
  'other': 'Sonstiges',
};

// Ordnet die Abbruchgründe des Dispatchers (RunOutputReader, ResultHandler, TicketStarter, DispatchService) einer festen Kategorie zu.
export class AbortReasonClassifier {
  static readonly CATEGORIES = Object.keys(LABELS) as AbortCategory[];

  // Kategorie zum Grundtext; unbekannte Texte ergeben 'other'
  category(reason: string): AbortCategory {
    const text = reason.trim();
    if (/^Zeitlimit von /.test(text)) return 'timeout';
    if (/^Maximale Zahl an Schritten/.test(text)) return 'max-turns';
    if (/^Kontext voll/.test(text)) return 'context';
    if (/^Claude endete mit Exit-Code/.test(text)) return 'exit-code';
    if (/^Claude meldet einen Fehler/.test(text)) return 'claude-error';
    if (/^Lauf endete ohne Abschluss/.test(text)) return 'no-outcome';
    if (/^Nutzungslimit/.test(text)) return 'usage-limit';
    if (/^Worker konnte nicht gestartet werden/.test(text)) return 'start-failed';
    if (/^(Worker lieferte .* kein Ergebnis|Worker-Prozess endete ohne Ergebnis|Kein Lebenszeichen seit)/.test(text)) return 'vanished';
    return 'other';
  }

  // Deutsche Beschriftung einer Kategorie (unbekannte: der Name selbst)
  label(category: string): string {
    return LABELS[category as AbortCategory] ?? category;
  }
}
