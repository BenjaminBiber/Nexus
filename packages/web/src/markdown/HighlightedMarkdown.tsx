import { useEffect, useRef } from 'react';
import { MarkdownView } from './MarkdownView.js';

// Markdown mit hervorgehobenem Suchbegriff (Treffer in <mark>, ohne die Struktur zu verändern).
export function HighlightedMarkdown({ source, term }: { source: string; term: string }) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const root = ref.current;
    if (!root || term.trim().length < 2) return;
    const needle = term.trim().toLowerCase();
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    const nodes: Text[] = [];
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      if (node.textContent?.toLowerCase().includes(needle)) nodes.push(node as Text);
    }
    nodes.forEach((node) => wrapMatches(node, needle));
  }, [source, term]);

  return (
    <div ref={ref}>
      <MarkdownView source={source} />
    </div>
  );
}

// Ersetzt einen Textknoten durch Text und <mark>-Treffer
function wrapMatches(node: Text, needle: string): void {
  const text = node.textContent ?? '';
  const lower = text.toLowerCase();
  const fragment = document.createDocumentFragment();
  let from = 0;
  for (let at = lower.indexOf(needle); at >= 0; at = lower.indexOf(needle, from)) {
    fragment.append(text.slice(from, at));
    const mark = document.createElement('mark');
    mark.textContent = text.slice(at, at + needle.length);
    fragment.append(mark);
    from = at + needle.length;
  }
  fragment.append(text.slice(from));
  node.replaceWith(fragment);
}
