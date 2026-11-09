// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { SNIPPETS_LINK_ID, applySnippets } from '../src/theme/snippets.js';
import { THEME_LINK_ID, applyTheme } from '../src/theme/theme.js';

const links = () => document.head.querySelectorAll<HTMLLinkElement>(`link#${SNIPPETS_LINK_ID}`);
const version = (link: HTMLLinkElement) => new URL(link.href, 'http://x').searchParams.get('v');

afterEach(() => {
  document.head.innerHTML = '';
  applyTheme('light');
});

describe('CSS-Schnipsel laden', () => {
  it('legt genau einen Link an und lädt ihn mit neuem v neu', async () => {
    applySnippets();
    const first = version(links()[0]);
    await new Promise((r) => setTimeout(r, 2));
    applySnippets();
    expect(links()).toHaveLength(1);
    expect(links()[0].getAttribute('href')).toMatch(/^\/snippets\/active\.css\?v=/);
    expect(version(links()[0])).not.toBe(first);
  });

  it('steht nach Basis-CSS und Theme-CSS', () => {
    const base = document.createElement('link');
    base.rel = 'stylesheet';
    document.head.appendChild(base);
    const theme = document.createElement('link');
    theme.id = THEME_LINK_ID;
    document.head.appendChild(theme);
    applySnippets();
    expect(document.head.lastElementChild).toBe(links()[0]);
    expect(theme.compareDocumentPosition(links()[0]) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('bleibt nach einem Theme-Wechsel über dem Theme', () => {
    applySnippets();
    applyTheme('ocean', ['ocean']);
    const theme = document.getElementById(THEME_LINK_ID)!;
    expect(theme.compareDocumentPosition(links()[0]) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    applyTheme('wald', ['wald']);
    expect(document.head.querySelectorAll(`#${THEME_LINK_ID}`)).toHaveLength(1);
    expect(document.getElementById(THEME_LINK_ID)!.nextElementSibling).toBe(links()[0]);
    expect(links()).toHaveLength(1);
  });
});
