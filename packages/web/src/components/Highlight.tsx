import type { ReactNode } from 'react';

// Text mit hervorgehobenen Bereichen [Anfang, Ende) als <mark>; Bereiche außerhalb des Textes oder überlappend werden übergangen.
// Kein HTML aus Daten: alles bleibt Text.
export function Highlight({ text, ranges }: { text: string; ranges: Array<[number, number]> }) {
  const parts: ReactNode[] = [];
  let pos = 0;
  for (const [start, end] of [...ranges].sort((a, b) => a[0] - b[0])) {
    if (start < pos || end <= start || end > text.length) continue;
    if (start > pos) parts.push(text.slice(pos, start));
    parts.push(<mark key={start}>{text.slice(start, end)}</mark>);
    pos = end;
  }
  if (pos < text.length) parts.push(text.slice(pos));
  return <>{parts}</>;
}
