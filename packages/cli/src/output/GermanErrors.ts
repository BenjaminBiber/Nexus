const RULES: Array<[RegExp, string]> = [
  [/^error: missing required argument '(.+)'/, "Fehler: Das Pflichtargument '$1' fehlt."],
  [/^error: unknown option '(.+?)'/, "Fehler: Unbekannte Option '$1'."],
  [/^error: unknown command '(.+?)'/, "Fehler: Unbekannter Befehl '$1'."],
  [/^error: option '(.+?)' argument missing/, "Fehler: Zur Option '$1' fehlt ein Wert."],
  [/^error: too many arguments.*/, 'Fehler: Zu viele Argumente.'],
  [/\(Did you mean (.+?)\?\)/, '(Meinten Sie $1?)'],
  [/^error:/, 'Fehler:'],
];

// Übersetzt die englischen Fehlermeldungen von commander ins Deutsche.
export class GermanErrors {
  // Wendet die Regeln der Reihe nach an
  translate(message: string): string {
    return RULES.reduce((text, [pattern, german]) => text.replace(pattern, german), message);
  }
}
