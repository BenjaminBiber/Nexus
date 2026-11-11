import { serverUrl } from '../api/nexusBridge.js';

/** ID des Links auf die aktiven CSS-Schnipsel des Nutzers (liegt über dem Theme, D8) */
export const SNIPPETS_LINK_ID = 'nexus-snippets-css';

/** Laufende Nummer, damit zwei Aufrufe in derselben Millisekunde trotzdem neu laden */
let counter = 0;

// Bindet die aktiven Schnipsel als letztes Stylesheet ein und lädt sie mit neuem v neu
export function applySnippets(): void {
  const href = serverUrl(`/snippets/active.css?v=${Date.now()}-${++counter}`);
  const existing = document.getElementById(SNIPPETS_LINK_ID);
  const link = existing instanceof HTMLLinkElement ? existing : document.createElement('link');
  link.id = SNIPPETS_LINK_ID;
  link.rel = 'stylesheet';
  link.href = href;
  document.head.appendChild(link);
}

// Macht aus einem Namen eine gültige Schnipsel-ID (a-z, 0-9, „-“, 2–40 Zeichen)
export function snippetSlug(name: string): string {
  const slug = name
    .toLowerCase()
    .replace(/ä/g, 'ae').replace(/ö/g, 'oe').replace(/ü/g, 'ue').replace(/ß/g, 'ss')
    .normalize('NFKD').replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 40)
    .replace(/-+$/, '');
  return slug.length >= 2 ? slug : `schnipsel-${Date.now().toString(36)}`;
}
