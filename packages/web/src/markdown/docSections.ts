// Schneidet den 1-basierten, einschließenden Zeilenbereich aus dem Rohtext;
// Bereiche über das Dateiende werden auf die vorhandenen Zeilen geklemmt
export function sliceLines(source: string, startLine: number, endLine: number): string {
  const lines = source.split('\n');
  const from = Math.max(1, startLine);
  const to = Math.min(lines.length, endLine);
  if (to < from) return '';
  return lines.slice(from - 1, to).join('\n');
}
