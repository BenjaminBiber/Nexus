// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { DbAdminDialog } from '../src/components/db/DbAdminDialog.js';
import { DocsPage } from '../src/pages/DocsPage.js';
import type { MarkdownSectionDto } from '../src/api/types.js';
import type { Platform } from '../src/platform/Platform.js';
import { PlatformProvider } from '../src/platform/PlatformContext.js';
import { dbBoards, dbProjects, fakeApi } from './helpers/fakeApi.js';
import { fakePlatform } from './helpers/fakePlatform.js';
import { renderWithApp } from './helpers/renderWithApp.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(() => {
  document.removeEventListener('click', stopNavigation);
  cleanup();
});

describe('Ordnerauswahl über die Plattform', () => {
  it('übernimmt den Pfad aus dem Ordnerdialog der Plattform', async () => {
    const pickFolder = vi.fn(async () => '/Users/me/repo');
    const platform = fakePlatform({ pickFolder });
    render(
      <ApiProvider api={fakeApi()}>
        <PlatformProvider value={platform}>
          <DbAdminDialog projects={dbProjects} boards={dbBoards} onClose={vi.fn()} onChanged={vi.fn()} />
        </PlatformProvider>
      </ApiProvider>,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Board 1 bearbeiten' }));
    const dialog = screen.getByRole('dialog', { name: 'Board 1 bearbeiten' });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Ordner wählen' }));
    await waitFor(() => expect((within(dialog).getByLabelText('Mein Repo-Pfad') as HTMLInputElement).value).toBe('/Users/me/repo'));
    expect(pickFolder).toHaveBeenCalledWith(undefined);
  });

  it('zeigt einen Fehler der Plattform im Dialog', async () => {
    const platform = fakePlatform({ pickFolder: vi.fn(async () => { throw new Error('Die Ordnerauswahl ließ sich nicht öffnen. (abgelehnt)'); }) });
    render(
      <ApiProvider api={fakeApi()}>
        <PlatformProvider value={platform}>
          <DbAdminDialog projects={dbProjects} boards={dbBoards} onClose={vi.fn()} onChanged={vi.fn()} />
        </PlatformProvider>
      </ApiProvider>,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Board 1 bearbeiten' }));
    const dialog = screen.getByRole('dialog', { name: 'Board 1 bearbeiten' });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Ordner wählen' }));
    expect((await within(dialog).findByRole('alert')).textContent).toContain('ließ sich nicht öffnen');
  });
});

const PATH = 'docs/superpowers/plans/2026-10-27-x.md';
const SECTIONS: MarkdownSectionDto[] = [{ heading: 'Ziel', level: 2, startLine: 3, endLine: 4 }];
const docFile = { path: PATH, title: '001-x · plan.md', group: 'plan' as const, kind: 'plan' as const, topic: null, date: null, mtimeMs: 1, exists: true };

// Rendert die Docs-Seite mit einem Dokument des Boards „Sprint“ (Repo /repo)
function openDocs(platform: Platform) {
  const api = fakeApi({
    getDocs: async () => ({ source: { root: '/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs: [docFile] }),
    getDoc: async (_pid: string, p: string) => ({ path: p, content: '# Plan\n\n## Ziel\n\nText.\n', hash: 'h1', sections: SECTIONS }),
    getDocComments: async () => [],
  });
  renderWithApp(<DocsPage />, { api, path: `/p/p1/docs/${PATH}`, route: '/p/:pid/docs/*', platform });
}

describe('Dokument öffnen über die Plattform', () => {
  it('reicht den Repo-Ordner des Dokuments an die Plattform durch', async () => {
    const openDoc = vi.fn(async () => undefined);
    openDocs(fakePlatform({ openDoc }));
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(screen.getByRole('button', { name: 'Datei öffnen' }));
    await userEvent.click(screen.getByRole('menuitem', { name: /Im Explorer zeigen/ }));
    expect(openDoc).toHaveBeenCalledWith({ pid: 'p1', path: PATH, root: '/repo', board: undefined, target: 'explorer' });
    await userEvent.click(screen.getByRole('button', { name: 'Datei öffnen' }));
    await userEvent.click(screen.getByRole('menuitem', { name: /In VS Code öffnen/ }));
    expect(openDoc).toHaveBeenLastCalledWith({ pid: 'p1', path: PATH, root: '/repo', board: undefined, target: 'vscode' });
  });

  it('zeigt einen Fehler der Plattform unter dem Menü', async () => {
    openDocs(fakePlatform({ openDoc: vi.fn(async () => { throw new Error('Der Ordner des Dokuments ist unbekannt.'); }) }));
    await screen.findByRole('heading', { name: 'Ziel' });
    await userEvent.click(screen.getByRole('button', { name: 'Datei öffnen' }));
    await userEvent.click(screen.getByRole('menuitem', { name: /Im Explorer zeigen/ }));
    expect((await screen.findByRole('alert')).textContent).toContain('Der Ordner des Dokuments ist unbekannt.');
  });
});

// jsdom kann nicht navigieren; dieser Zuhörer läuft nach dem der App und hält den Klick am Ende auf
const stopNavigation = (event: MouseEvent) => event.preventDefault();

// Rendert die ganze App mit einer Plattform und einem Link auf die angegebene Adresse
function openAppWithLink(platform: Platform, href: string) {
  render(
    <ApiProvider api={fakeApi()}>
      <PlatformProvider value={platform}>
        <MemoryRouter initialEntries={['/p/p1/board']}>
          <App />
          <a href={href}>Ein Link</a>
        </MemoryRouter>
      </PlatformProvider>
    </ApiProvider>,
  );
  document.addEventListener('click', stopNavigation);
}

describe('Externe Links in der Desktop-App', () => {
  it('öffnet eine fremde Adresse im Standardbrowser statt im Fenster', async () => {
    const openExternal = vi.fn(async () => undefined);
    openAppWithLink(fakePlatform({ openExternal }), 'https://example.test/doku');
    await userEvent.click(screen.getByRole('link', { name: 'Ein Link' }));
    expect(openExternal).toHaveBeenCalledWith('https://example.test/doku');
  });

  it('lässt Links innerhalb der Oberfläche in Ruhe', async () => {
    const openExternal = vi.fn(async () => undefined);
    openAppWithLink(fakePlatform({ openExternal }), '/p/p1/docs');
    await userEvent.click(screen.getByRole('link', { name: 'Ein Link' }));
    expect(openExternal).not.toHaveBeenCalled();
  });

  it('übernimmt im Browser keine Klicks', async () => {
    const openExternal = vi.fn(async () => undefined);
    openAppWithLink(fakePlatform({ kind: 'browser', openExternal }), 'https://example.test/doku');
    await userEvent.click(screen.getByRole('link', { name: 'Ein Link' }));
    expect(openExternal).not.toHaveBeenCalled();
  });
});
