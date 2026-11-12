// Höchste Wartezeit; ein Szenario soll Zeitstempel trennen, nicht auf Abläufe warten
const MAX_MS = 1000;

// Schritt-Art `wait`: wartet die genannten Millisekunden, ohne den Server anzusprechen. Der Server schreibt
// Zeitstempel auf die Millisekunde genau; zwei Anfragen direkt hintereinander landen auf einem schnellen Rechner
// in derselben Millisekunde, und Ereignisse aus verschiedenen Quellen (Verlauf, Kommentare) stehen dann in der
// Reihenfolge, in der der Server sie sammelt, statt in der, in der sie geschahen. Ein kurzes `wait` dazwischen
// trennt die Zeitstempel. Auf Ereignisse des Servers wartet `sseExpect` oder `httpStub` mit `await`.
export class WaitStep {
  // Die Felder, die diese Art im Szenario kennt
  get keys() {
    return ['ms'];
  }

  // Prüft die Felder des Schritts
  parse(value, index, fail) {
    if (!Number.isInteger(value.ms) || value.ms < 1 || value.ms > MAX_MS) {
      fail(`steps[${index}].ms muss eine ganze Zahl von 1 bis ${MAX_MS} sein.`);
    }
    return { ms: value.ms };
  }

  // Wartet; aufgezeichnet wird nur die Wartezeit (meist steht der Schritt ohnehin mit `record: false` da)
  async run(step) {
    await new Promise((resolve) => setTimeout(resolve, step.ms));
    return { request: { kind: 'wait', ms: step.ms }, response: {} };
  }
}
