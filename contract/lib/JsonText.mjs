// Kanonische JSON-Schreibweise der Golden-Dateien und zeilenweiser Vergleich (eigene LCS-Funktion).
export class JsonText {
  // Serialisiert mit sortierten Objekt-Schlüsseln, 2 Leerzeichen Einrückung, ohne Zeilenumbruch am Ende
  static canonical(value) {
    return JSON.stringify(JsonText.sorted(value), null, 2);
  }

  // Baut den Wert mit sortierten Objekt-Schlüsseln neu auf; Arrays behalten ihre Reihenfolge
  static sorted(value) {
    if (Array.isArray(value)) return value.map((entry) => JsonText.sorted(entry));
    if (value !== null && typeof value === 'object') {
      const out = {};
      for (const key of Object.keys(value).sort()) out[key] = JsonText.sorted(value[key]);
      return out;
    }
    return value;
  }

  // Zeilenweiser Unterschied zweier Werte: `-` erwartet, `+` tatsächlich; leerer Text bei Gleichheit
  static diff(expected, actual, context = 2) {
    const left = JsonText.canonical(expected).split('\n');
    const right = JsonText.canonical(actual).split('\n');
    const ops = JsonText.operations(left, right);
    if (!ops.some((op) => op.mark !== ' ')) return '';
    return JsonText.withContext(ops, context).join('\n');
  }

  // Vergleichsschritte beider Zeilenlisten über die längste gemeinsame Teilfolge
  static operations(left, right) {
    // Sehr große Dateien vergleichen wir nur Zeile für Zeile; die LCS-Tabelle wäre unnötig teuer
    if (left.length * right.length > 1_000_000) return JsonText.positional(left, right);
    const table = JsonText.table(left, right);
    const ops = [];
    let i = 0;
    let j = 0;
    while (i < left.length || j < right.length) {
      if (i < left.length && j < right.length && left[i] === right[j]) {
        ops.push({ mark: ' ', text: left[i] });
        i += 1;
        j += 1;
      } else if (i < left.length && (j === right.length || table[i + 1][j] >= table[i][j + 1])) {
        // Bei gleicher Länge der Teilfolge zuerst die erwartete Zeile, damit `-` vor `+` steht
        ops.push({ mark: '-', text: left[i] });
        i += 1;
      } else {
        ops.push({ mark: '+', text: right[j] });
        j += 1;
      }
    }
    return ops;
  }

  // Tabelle der Länge der längsten gemeinsamen Teilfolge ab Position (i, j)
  static table(left, right) {
    const table = Array.from({ length: left.length + 1 }, () => new Int32Array(right.length + 1));
    for (let i = left.length - 1; i >= 0; i -= 1) {
      for (let j = right.length - 1; j >= 0; j -= 1) {
        table[i][j] = left[i] === right[j] ? table[i + 1][j + 1] + 1 : Math.max(table[i + 1][j], table[i][j + 1]);
      }
    }
    return table;
  }

  // Rückfall für sehr große Listen: Zeilen der gleichen Position vergleichen
  static positional(left, right) {
    const ops = [];
    for (let i = 0; i < Math.max(left.length, right.length); i += 1) {
      if (left[i] === right[i]) ops.push({ mark: ' ', text: left[i] });
      else {
        if (i < left.length) ops.push({ mark: '-', text: left[i] });
        if (i < right.length) ops.push({ mark: '+', text: right[i] });
      }
    }
    return ops;
  }

  // Nur Zeilen rund um die Abweichungen ausgeben; dazwischen eine Auslassung
  static withContext(ops, context) {
    const keep = new Set();
    ops.forEach((op, index) => {
      if (op.mark === ' ') return;
      for (let k = index - context; k <= index + context; k += 1) if (k >= 0 && k < ops.length) keep.add(k);
    });
    const lines = [];
    let skipped = false;
    ops.forEach((op, index) => {
      if (!keep.has(index)) {
        skipped = true;
        return;
      }
      if (skipped) lines.push('  …');
      skipped = false;
      lines.push(`${op.mark} ${op.text}`);
    });
    return lines;
  }
}
