import { useMemo } from 'react';
import DOMPurify from 'dompurify';
import { marked, type Token, type TokensList } from 'marked';
import { MermaidBlock } from './MermaidBlock.js';

// Links aus Beschreibungen öffnen sicher in neuem Tab
DOMPurify.addHook('afterSanitizeAttributes', (node) => {
  if (node.tagName === 'A') {
    node.setAttribute('target', '_blank');
    node.setAttribute('rel', 'noopener noreferrer');
  }
});

type Part = { kind: 'html'; html: string } | { kind: 'mermaid'; source: string };

// Zerlegt Markdown in HTML-Stücke und Mermaid-Blöcke; das HTML wird gesäubert (Skripte, Ereignisattribute).
function toParts(source: string): Part[] {
  const tokens = marked.lexer(source);
  const parts: Part[] = [];
  let buffer: Token[] = [];
  const flush = () => {
    if (!buffer.length) return;
    const chunk = Object.assign([...buffer], { links: tokens.links }) as TokensList;
    parts.push({ kind: 'html', html: DOMPurify.sanitize(marked.parser(chunk)) });
    buffer = [];
  };
  for (const token of tokens) {
    if (token.type === 'code' && (token as { lang?: string }).lang === 'mermaid') {
      flush();
      parts.push({ kind: 'mermaid', source: (token as { text: string }).text });
    } else {
      buffer.push(token);
    }
  }
  flush();
  return parts;
}

// Stellt Markdown dar; Codeblöcke mit der Sprache mermaid werden als Diagramm gezeichnet.
export function MarkdownView({ source }: { source: string }) {
  const parts = useMemo(() => toParts(source), [source]);
  return (
    <div className="markdown">
      {parts.map((part, index) =>
        part.kind === 'mermaid' ? (
          <MermaidBlock key={index} source={part.source} />
        ) : (
          <div key={index} dangerouslySetInnerHTML={{ __html: part.html }} />
        ),
      )}
    </div>
  );
}
