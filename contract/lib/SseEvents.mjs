// Bearbeitet die Liste der Ereignisse eines Stroms, bevor sie in die Golden-Datei geht:
// Herzschläge entfernen, aufeinanderfolgende gleiche Einträge zusammenfassen, `until` als Teilobjekt prüfen.
// Beides hält die Aufzeichnung unabhängig vom Takt des Servers (reine Funktionen, kein Zustand).
export class SseEvents {
  // Ist der Eintrag ein Kommentar (Herzschlag `: ping`)?
  static isComment(entry) {
    return entry !== null && typeof entry === 'object' && 'comment' in entry;
  }

  // Alle Einträge außer den Kommentaren; ihre Anzahl hängt am Takt und gehört nicht in die Golden-Datei
  static withoutComments(entries) {
    return entries.filter((entry) => !SseEvents.isComment(entry));
  }

  // Fasst aufeinanderfolgende gleiche Einträge zu einem zusammen (zwei Änderungen in einem Takt melden
  // der Server einmal oder zweimal – beides ergibt denselben Eintrag)
  static fold(entries) {
    const out = [];
    for (const entry of entries) {
      const last = out[out.length - 1];
      if (last !== undefined && JSON.stringify(last) === JSON.stringify(entry)) continue;
      out.push(entry);
    }
    return out;
  }

  // Passt der Eintrag auf `until`? Verglichen wird als Teilobjekt: jedes Feld aus `until` muss im Eintrag
  // stehen und gleich sein, verschachtelte Objekte wieder als Teilobjekt. Felder, die `until` nicht nennt,
  // spielen keine Rolle.
  static matches(entry, until) {
    if (until === null || typeof until !== 'object' || Array.isArray(until)) return false;
    if (entry === null || typeof entry !== 'object' || Array.isArray(entry)) return false;
    return Object.entries(until).every(([key, value]) => {
      if (value !== null && typeof value === 'object' && !Array.isArray(value)) return SseEvents.matches(entry[key], value);
      return JSON.stringify(entry[key]) === JSON.stringify(value);
    });
  }
}
