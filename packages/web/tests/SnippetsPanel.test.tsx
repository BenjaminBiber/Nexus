// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { SnippetDto } from '../src/api/types.js';
import { SnippetsSection } from '../src/components/settings/SnippetsSection.js';
import { SNIPPETS_LINK_ID } from '../src/theme/snippets.js';
import { fakeApi } from './helpers/fakeApi.js';

const rot: SnippetDto = { id: 'rot', name: 'Rot', css: 'a { color: red }', enabled: true, order: 1, updatedAt: '' };
const link = () => document.getElementById(SNIPPETS_LINK_ID) as HTMLLinkElement | null;

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  document.head.innerHTML = '';
});

// Zeigt die Sektion mit einer veränderbaren Liste
function setup(extra: Record<string, unknown> = {}) {
  let list = [rot];
  const api = {
    listSnippets: vi.fn(async () => list),
    saveSnippet: vi.fn(async (id: string, s: { name: string; css: string }) => {
      const saved = { ...rot, id, ...s };
      list = [...list.filter((x) => x.id !== id), saved];
      return saved;
    }),
    toggleSnippet: vi.fn(async (id: string, enabled: boolean) => {
      list = list.map((x) => (x.id === id ? { ...x, enabled } : x));
      return list[0];
    }),
    deleteSnippet: vi.fn(async (id: string) => {
      list = list.filter((x) => x.id !== id);
    }),
    ...extra,
  };
  render(<ApiProvider api={fakeApi(api as never)}><SnippetsSection /></ApiProvider>);
  return api;
}

describe('Eigene CSS-Schnipsel', () => {
  it('listet Schnipsel mit Name und Schalter', async () => {
    setup();
    expect(await screen.findByText('Rot')).toBeTruthy();
    expect((screen.getByLabelText('Rot aktiv') as HTMLInputElement).checked).toBe(true);
  });

  it('schaltet aus und lädt das Schnipsel-CSS mit neuem v neu', async () => {
    const api = setup();
    await userEvent.click(await screen.findByLabelText('Rot aktiv'));
    expect(api.toggleSnippet).toHaveBeenCalledWith('rot', false);
    await waitFor(() => expect(link()?.getAttribute('href')).toMatch(/^\/snippets\/active\.css\?v=/));
    const first = link()!.getAttribute('href');
    await waitFor(() => expect((screen.getByLabelText('Rot aktiv') as HTMLInputElement).checked).toBe(false));
    await userEvent.click(screen.getByLabelText('Rot aktiv'));
    await waitFor(() => expect(link()!.getAttribute('href')).not.toBe(first));
  });

  it('bearbeitet Name und CSS', async () => {
    const api = setup();
    await userEvent.click(await screen.findByLabelText('Rot bearbeiten'));
    const css = screen.getByLabelText('CSS');
    await userEvent.clear(css);
    await userEvent.type(css, 'b {{ color: blue }');
    await userEvent.click(screen.getByText('Speichern'));
    expect(api.saveSnippet).toHaveBeenCalledWith('rot', { name: 'Rot', css: 'b { color: blue }' });
    await waitFor(() => expect(screen.queryByLabelText('CSS')).toBeNull());
  });

  it('legt einen neuen Schnipsel mit ID aus dem Namen an', async () => {
    const api = setup();
    await userEvent.click(await screen.findByText(/Neuer Schnipsel/));
    await userEvent.type(screen.getByLabelText('Name'), 'Mein Rot');
    await userEvent.type(screen.getByLabelText('CSS'), 'body {{ color: red }');
    await userEvent.click(screen.getByText('Speichern'));
    expect(api.saveSnippet).toHaveBeenCalledWith('mein-rot', { name: 'Mein Rot', css: 'body { color: red }' });
    expect(await screen.findByText('Mein Rot')).toBeTruthy();
  });

  it('löscht erst nach Bestätigung', async () => {
    const api = setup();
    const confirm = vi.spyOn(window, 'confirm').mockReturnValueOnce(false).mockReturnValueOnce(true);
    await userEvent.click(await screen.findByLabelText('Rot löschen'));
    expect(api.deleteSnippet).not.toHaveBeenCalled();
    await userEvent.click(screen.getByLabelText('Rot löschen'));
    expect(confirm).toHaveBeenCalledTimes(2);
    expect(api.deleteSnippet).toHaveBeenCalledWith('rot');
    await waitFor(() => expect(screen.queryByText('Rot')).toBeNull());
  });

  it('zeigt Fehler des Servers am Editor, Eingabe und Liste bleiben', async () => {
    setup({ saveSnippet: vi.fn(async () => { throw new ApiError('invalid_css', '@import ist nicht erlaubt', 400); }) });
    await userEvent.click(await screen.findByLabelText('Rot bearbeiten'));
    const css = screen.getByLabelText('CSS');
    await userEvent.clear(css);
    await userEvent.type(css, '@import "x.css";');
    await userEvent.click(screen.getByText('Speichern'));
    expect((await screen.findByRole('alert')).textContent).toContain('@import ist nicht erlaubt');
    expect((screen.getByLabelText('CSS') as HTMLTextAreaElement).value).toBe('@import "x.css";');
    expect(screen.getByLabelText('Rot aktiv')).toBeTruthy();
  });
});
