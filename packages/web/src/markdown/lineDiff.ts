export interface DiffLine {
  kind: 'same' | 'add' | 'remove';
  text: string;
  /** Zeilennummer im Original (1-basiert), null bei neuen Zeilen */
  oldLine: number | null;
  /** Zeilennummer im Vorschlag (1-basiert), null bei entfernten Zeilen */
  newLine: number | null;
}

// Zeilenweiser Vergleich über die längste gemeinsame Teilfolge; bewusst ohne weitere Abhängigkeit,
// die verglichenen Bereiche sind Abschnitte und keine ganzen Dateien.
export function lineDiff(original: string, proposal: string): DiffLine[] {
  const a = original.split(/\r?\n/);
  const b = proposal.split(/\r?\n/);
  // lcs[i][j] = Länge der gemeinsamen Teilfolge von a ab i und b ab j
  const lcs: number[][] = Array.from({ length: a.length + 1 }, () => new Array<number>(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i -= 1) {
    for (let j = b.length - 1; j >= 0; j -= 1) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      out.push({ kind: 'same', text: a[i], oldLine: i + 1, newLine: j + 1 });
      i += 1;
      j += 1;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      // Entfernte Zeilen stehen vor den neuen, damit eine Ersetzung als Paar lesbar bleibt
      out.push({ kind: 'remove', text: a[i], oldLine: i + 1, newLine: null });
      i += 1;
    } else {
      out.push({ kind: 'add', text: b[j], oldLine: null, newLine: j + 1 });
      j += 1;
    }
  }
  for (; i < a.length; i += 1) out.push({ kind: 'remove', text: a[i], oldLine: i + 1, newLine: null });
  for (; j < b.length; j += 1) out.push({ kind: 'add', text: b[j], oldLine: null, newLine: j + 1 });
  return out;
}
