// Die Auswahl aus `--step`: welche Schritte eines Szenarios laufen und mit welchen Einträgen der
// Golden-Datei verglichen wird. So lässt sich eine Teilmenge der Befehle prüfen, solange die
// anderen noch nicht portiert sind.
export class StepFilter {
  // `step` ist der Wert von `--step` (kommagetrennte Teilstrings) oder null
  constructor(step) {
    const parts = (step ?? '').split(',').map((part) => part.trim()).filter(Boolean);
    this.parts = step === null || !parts.length ? null : parts;
  }

  // Wahr, wenn `--step` überhaupt etwas auswählt
  get active() {
    return this.parts !== null;
  }

  // Das Szenario mit den gewählten Schritten (ohne die Option: unverändert). Schritte mit
  // `record: false` bauen nur den Zustand auf (Projekt, Board, Tickets) und laufen immer mit.
  scenario(scenario) {
    if (!this.parts) return scenario;
    const keep = (step) => step.record === false || this.matches(step.name);
    return { ...scenario, steps: scenario.steps.filter(keep) };
  }

  // Die Einträge der Golden-Datei, die zu den gelaufenen Schritten gehören
  entries(entries) {
    return this.parts ? entries.filter((entry) => this.matches(entry.name)) : entries;
  }

  // Wahr, wenn der Name einen der Teilstrings enthält
  matches(name) {
    return this.parts.some((part) => name.includes(part));
  }
}
