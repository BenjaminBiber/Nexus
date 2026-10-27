// Thema einer Spec aus ihrem Wert: Dateiname ohne Ordner, ohne „.md“, ohne führendes Datum und ohne
// die Endung „-design“/„-plan“ (dieselben Regeln wie beim Einlesen der Superpowers-Dokumente).
// Bleibt nichts übrig, kommt der Eingabewert zurück – alte Werte wie „001-login“ bleiben so lesbar.
export function specTopic(spec: string): string {
  const file = spec.split(/[/\\]/).pop() ?? spec;
  const topic = file
    .replace(/\.md$/i, '')
    .replace(/^\d{4}-\d{2}-\d{2}-/, '')
    .replace(/-(design|plan)$/, '');
  return topic || spec;
}
