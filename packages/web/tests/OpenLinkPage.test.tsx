// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup } from '@testing-library/react';
import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { ApiClient } from '../src/api/ApiClient.js';
import { OpenLinkPage } from '../src/pages/OpenLinkPage.js';
import { fakeApi } from './helpers/fakeApi.js';

// Zeigt die erreichte Adresse als Text, damit der Test die Weiterleitung prüfen kann.
function Ziel() {
  const location = useLocation();
  return <div>Ziel: {location.pathname}</div>;
}

// Rendert die Weiterleitungsseite unter /open/:kind/:id; jede andere Adresse zeigt ihren Pfad.
function renderOpen(path: string, api: ApiClient = fakeApi()) {
  return render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/open/:kind/:id" element={<OpenLinkPage />} />
          <Route path="*" element={<Ziel />} />
        </Routes>
      </MemoryRouter>
    </ApiProvider>,
  );
}

afterEach(() => cleanup());

describe('OpenLinkPage', () => {
  it('leitet ein Ticket auf das Board des ersten Projekts weiter', async () => {
    renderOpen('/open/ticket/ARC-0042');
    expect(await screen.findByText('Ziel: /p/p1/board/t/ARC-0042')).toBeTruthy();
  });

  it('leitet eine Frage auf die Fragenseite weiter', async () => {
    renderOpen('/open/question/ARC-F0001');
    expect(await screen.findByText('Ziel: /p/p1/questions/ARC-F0001')).toBeTruthy();
  });

  it('weist eine unbekannte Art mit einem Hinweis ab', async () => {
    renderOpen('/open/doc/ARC-0042');
    expect(await screen.findByText('Dieser Link führt nirgendwohin.')).toBeTruthy();
  });

  it('weist eine ungültige ID mit einem Hinweis ab', async () => {
    renderOpen('/open/ticket/..%2Fetc');
    expect(await screen.findByText('Dieser Link führt nirgendwohin.')).toBeTruthy();
  });

  it('weist eine Ticket-ID im Fragenformat ab', async () => {
    renderOpen('/open/question/ARC-0042');
    expect(await screen.findByText('Dieser Link führt nirgendwohin.')).toBeTruthy();
  });

  it('zeigt ohne Projekt den Leerzustand wie die Startseite', async () => {
    renderOpen('/open/ticket/ARC-0042', fakeApi({ getProjects: async () => [] }));
    expect(await screen.findByText('Noch kein Projekt angemeldet')).toBeTruthy();
  });
});
