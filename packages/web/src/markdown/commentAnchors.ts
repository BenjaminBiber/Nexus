export interface Selected {
  quote: string;
  heading: string | null;
}

const MARK_CLASS = 'doc-comment';
const BLOCKS = 'p, li, h1, h2, h3, h4, h5, h6, td, th, div, blockquote, pre, dt, dd, figcaption';

// Alle Textknoten unterhalb von root in Dokumentreihenfolge
function textNodes(root: HTMLElement): Text[] {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) nodes.push(n as Text);
  return nodes;
}

// Nächster Blockvorfahre eines Textknotens (Absatz, Listenpunkt, ...)
function blockOf(node: Text, root: HTMLElement): Element {
  return node.parentElement?.closest(BLOCKS) ?? root;
}

// Text aller Knoten mit zu einem Leerzeichen gefaltetem Leerraum; map[i] = Originalposition von text[i]
function normalizedText(nodes: Text[], root: HTMLElement): { text: string; map: number[] } {
  let text = '';
  const map: number[] = [];
  let offset = 0;
  let block: Element | null = null;
  for (const node of nodes) {
    const own = blockOf(node, root);
    // Blockwechsel zählt als Leerraum, denn die Auswahl setzt dort einen Zeilenumbruch
    if (block && own !== block && !text.endsWith(' ')) {
      text += ' ';
      map.push(offset);
    }
    block = own;
    for (let i = 0; i < node.data.length; i++) {
      const isSpace = /\s/.test(node.data[i]);
      if (isSpace && text.endsWith(' ')) continue;
      text += isSpace ? ' ' : node.data[i];
      map.push(offset + i);
    }
    offset += node.data.length;
  }
  return { text, map };
}

// Entfernt alle Kommentar-Hervorhebungen und fügt die Textknoten wieder zusammen
export function clearHighlights(root: HTMLElement): void {
  root.querySelectorAll(`mark.${MARK_CLASS}`).forEach((mark) => mark.replaceWith(...Array.from(mark.childNodes)));
  root.normalize();
}

// Hebt das Zitat hervor (auch über Elementgrenzen); false, wenn es im Text nicht vorkommt
export function highlightQuote(root: HTMLElement, quote: string, id: string): boolean {
  const needle = quote.replace(/\s+/g, ' ').trim();
  if (!needle) return false;
  const nodes = textNodes(root);
  const { text, map } = normalizedText(nodes, root);
  const found = text.indexOf(needle);
  if (found < 0) return false;
  // Zurück auf Positionen im unveränderten Text abbilden
  const start = map[found];
  const end = map[found + needle.length - 1] + 1;
  let offset = 0;
  for (const node of nodes) {
    const length = node.data.length; // vor dem Teilen merken
    const from = Math.max(start - offset, 0);
    const to = Math.min(end - offset, length);
    // Reiner Leerraum liegt zwischen Blöcken (ul, table) und bleibt ohne Hervorhebung
    if (from < to && /\S/.test(node.data.slice(from, to))) wrapPart(node, from, to, id);
    offset += length;
  }
  return true;
}

// Umschließt den Bereich [from, to) eines Textknotens mit einer Hervorhebung
function wrapPart(node: Text, from: number, to: number, id: string): void {
  const part = from > 0 ? node.splitText(from) : node;
  if (to - from < part.data.length) part.splitText(to - from);
  const mark = document.createElement('mark');
  mark.className = MARK_CLASS;
  mark.dataset.commentId = id;
  part.replaceWith(mark);
  mark.append(part);
}

// Liest die Auswahl innerhalb von root: Zitat und die Überschrift davor
export function readSelection(root: HTMLElement): Selected | null {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return null;
  const range = selection.getRangeAt(0);
  if (!root.contains(range.commonAncestorContainer)) return null;
  const quote = selection.toString().replace(/\s+/g, ' ').trim();
  if (!quote) return null;
  return { quote, heading: headingBefore(root, range.startContainer) };
}

// Text der letzten Überschrift (h2–h3, wie im Verzeichnis), die vor dem Knoten steht
function headingBefore(root: HTMLElement, node: Node): string | null {
  let found: string | null = null;
  root.querySelectorAll('h2, h3').forEach((h) => {
    if (h.compareDocumentPosition(node) & Node.DOCUMENT_POSITION_FOLLOWING) found = h.textContent?.trim() ?? null;
  });
  return found;
}

// Zeichen der Inline-Auszeichnung (Code, Fett/Kursiv, Durchgestrichen, Tabellen, Escapes, Linkklammern); sie fehlen im
// gerenderten Text und werden beim Vergleich auf beiden Seiten übergangen
const INLINE_MARKUP = /[`*_~|\\[\]]/;
// Blockmarken am Zeilenanfang: Einrückung, Überschrift, Zitat, Listenpunkt (auch Aufgaben), Nummer
const LINE_MARKUP = /^[ \t]*(?:(?:#{1,6}|>|[-*+](?:[ \t]+\[[ xX]\])?|\d+[.)])[ \t]+)*/;

// Das Markdown als Text, wie ihn die gerenderte Seite zeigt (ohne Auszeichnung, Links nur mit ihrem Text), ganz ohne Leerraum:
// Inline-Code und Umbrüche erzeugen in der Auswahl unterschiedlich viel davon. map[i] = Position von text[i] im Markdown
function plainOf(source: string): { text: string; map: number[] } {
  let text = '';
  const map: number[] = [];
  for (let i = 0; i < source.length; i++) {
    if (i === 0 || source[i - 1] === '\n') {
      i += LINE_MARKUP.exec(source.slice(i))?.[0].length ?? 0;
      if (i >= source.length) break;
    }
    const c = source[i];
    // Das Ziel eines Links „[Text](Ziel)“ steht nicht im gerenderten Text
    if (c === ']' && source[i + 1] === '(') {
      const close = source.indexOf(')', i);
      if (close > 0) i = close;
      continue;
    }
    if (INLINE_MARKUP.test(c) || /\s/.test(c)) continue;
    text += c;
    map.push(i);
  }
  return { text, map };
}

// Zeilenbereich (1-basiert) des Zitats im Markdown; null, wenn nicht auffindbar. Das Zitat ist gerenderter Text: Auszeichnung
// wie `Code` oder **fett** und Linkziele fehlen darin, Leerraum zählt nicht.
export function lineRangeOf(source: string, quote: string): [number, number] | null {
  const needle = Array.from(quote).filter((c) => !INLINE_MARKUP.test(c) && !/\s/.test(c)).join('');
  if (!needle) return null;
  const { text, map } = plainOf(source);
  let length = needle.length;
  let at = text.indexOf(needle);
  // Notfalls genügt der Anfang des Zitats (etwa wenn es über ein Bild oder HTML läuft); der Bereich ist dann nur dieser Anfang
  if (at < 0) {
    length = Math.min(needle.length, 60);
    at = text.indexOf(needle.slice(0, length));
  }
  if (at < 0) return null;
  const lineAt = (pos: number) => source.slice(0, pos).split('\n').length;
  return [lineAt(map[at]), lineAt(map[at + length - 1])];
}

const POPOVER_WIDTH = 320;

// Punkt am Ende der Auswahl (rechts der letzten Zeile) als Rechteck ohne Breite; null, wo nicht ermittelbar (z. B. jsdom)
export function selectionRect(): DOMRect | null {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);
  if (typeof range.getClientRects !== 'function') return null;
  // Nur Rechtecke mit Breite zählen; leere Rechtecke an Blockgrenzen würden den Punkt verschieben
  const rects = Array.from(range.getClientRects()).filter((r) => r.width > 0);
  const last = rects[rects.length - 1];
  return last ? new DOMRect(last.right, last.top, 0, last.height) : null;
}

// Position unter der Auswahl, relativ zum Layout und innerhalb dessen Breite
export function anchorIn(layout: HTMLElement, rect: DOMRect | null): { top: number; left: number } {
  if (!rect) return { top: 0, left: 0 };
  const box = layout.getBoundingClientRect();
  const left = Math.min(rect.left - box.left, box.width - POPOVER_WIDTH);
  return { top: rect.bottom - box.top + 6, left: Math.max(left, 0) };
}
