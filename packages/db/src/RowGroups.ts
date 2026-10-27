// Ordnet Kind-Zeilen (Labels, Voraussetzungen) ihrem Ticket zu – einmal gruppiert statt je Ticket alle Zeilen zu filtern.
export class RowGroups {
  // Werte je Schlüssel in der Reihenfolge der Zeilen
  static by<R, V>(rows: R[], key: (row: R) => string, value: (row: R) => V): Map<string, V[]> {
    const groups = new Map<string, V[]>();
    for (const row of rows) {
      const k = key(row);
      const list = groups.get(k);
      if (list) list.push(value(row));
      else groups.set(k, [value(row)]);
    }
    return groups;
  }
}
