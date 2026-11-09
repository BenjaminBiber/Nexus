// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { StoreRepoDto } from '../src/api/types.js';
import { StoreRepos } from '../src/components/settings/StoreRepos.js';
import { fakeApi } from './helpers/fakeApi.js';

const repo: StoreRepoDto = { id: 'r1', url: 'https://example.com/a/nexus-themes.json', addedAt: '' };

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

// Zeigt den Abschnitt mit einer veränderbaren Liste
function setup(initial: StoreRepoDto[] = [repo], extra: Record<string, unknown> = {}) {
  let list = initial;
  const api = {
    listStoreRepos: vi.fn(async () => list),
    addStoreRepo: vi.fn(async (url: string) => {
      const added = { id: 'r2', url, addedAt: '' };
      list = [...list, added];
      return added;
    }),
    removeStoreRepo: vi.fn(async (id: string) => {
      list = list.filter((x) => x.id !== id);
    }),
    ...extra,
  };
  render(<ApiProvider api={fakeApi(api as never)}><StoreRepos /></ApiProvider>);
  return api;
}

describe('Store-Repos', () => {
  it('listet die eingetragenen Repos', async () => {
    setup();
    expect(await screen.findByText(repo.url)).toBeTruthy();
  });

  it('zeigt bei leerer Liste einen Hinweis', async () => {
    setup([]);
    expect(await screen.findByText('Noch keine Repos eingetragen.')).toBeTruthy();
  });

  it('fügt ein Repo hinzu und leert das Feld', async () => {
    const api = setup([]);
    const input = (await screen.findByLabelText('Repo-URL')) as HTMLInputElement;
    expect((screen.getByRole('button', { name: 'Hinzufügen' }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.type(input, 'https://example.com/b.json');
    await userEvent.click(screen.getByRole('button', { name: 'Hinzufügen' }));
    expect(api.addStoreRepo).toHaveBeenCalledWith('https://example.com/b.json');
    expect(await screen.findByText('https://example.com/b.json')).toBeTruthy();
    expect(input.value).toBe('');
  });

  it('zeigt die Meldung des Servers und lässt die Eingabe stehen', async () => {
    const api = setup([repo], {
      addStoreRepo: vi.fn(async () => {
        throw new ApiError('store-repo-exists', 'Dieses Repo ist schon eingetragen.', 409);
      }),
    });
    const input = (await screen.findByLabelText('Repo-URL')) as HTMLInputElement;
    await userEvent.type(input, repo.url);
    await userEvent.click(screen.getByRole('button', { name: 'Hinzufügen' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Dieses Repo ist schon eingetragen.');
    expect(input.value).toBe(repo.url);
    expect(api.listStoreRepos).toHaveBeenCalledTimes(1);
    expect(screen.getAllByRole('listitem')).toHaveLength(1);
  });

  it('entfernt nach Bestätigung', async () => {
    const api = setup();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    await userEvent.click(await screen.findByLabelText(`Repo entfernen: ${repo.url}`));
    expect(api.removeStoreRepo).toHaveBeenCalledWith('r1');
    await waitFor(() => expect(screen.queryByText(repo.url)).toBeNull());
  });

  it('tut bei Abbruch nichts', async () => {
    const api = setup();
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    await userEvent.click(await screen.findByLabelText(`Repo entfernen: ${repo.url}`));
    expect(api.removeStoreRepo).not.toHaveBeenCalled();
    expect(screen.getByText(repo.url)).toBeTruthy();
  });
});
