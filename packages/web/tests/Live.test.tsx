// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbActive, fakeApi } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

// Rendert die Sessions-Seite der App mit gefälschtem Server (ohne Board-Projekt, damit der Strom nur Sessions meldet)
function openApp() {
  const getSessions = vi.fn(async () => []);
  const openEvents = vi.fn(() => new FakeEventSource());
  const api = fakeApi({ getSessions, openEvents, getDbProjects: async () => [], getDbActive: async () => dbActive(null) });
  render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={['/p/p1/sessions']}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
  return { getSessions, openEvents };
}

describe('Verbindungsanzeige', () => {
  it('zeigt "verbunden", sobald der Server grüßt', async () => {
    openApp();
    await screen.findByText('Keine Sessions');
    act(() => FakeEventSource.current().emit({ type: 'hello' }));
    expect(await screen.findByText('verbunden')).toBeTruthy();
  });

  it('zeigt bei Abbruch Meldung, Zeitpunkt des letzten Stands und "Erneut verbinden"', async () => {
    openApp();
    await screen.findByText('Keine Sessions');
    act(() => FakeEventSource.current().emit({ type: 'hello' }));
    act(() => FakeEventSource.current().fail());
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Server nicht erreichbar');
    expect(alert.textContent).toMatch(/Stand von \d{1,2}:\d{2} Uhr/);
    expect(screen.getByText('getrennt')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Erneut verbinden' })).toBeTruthy();
  });

  it('verbindet auf Wunsch neu und schließt die alte Verbindung', async () => {
    const { openEvents } = openApp();
    await screen.findByText('Keine Sessions');
    const first = FakeEventSource.current();
    act(() => first.fail());
    await userEvent.click(await screen.findByRole('button', { name: 'Erneut verbinden' }));
    expect(openEvents).toHaveBeenCalledTimes(2);
    expect(first.closed).toBe(true);
  });

  it('lädt nach der Wiederverbindung den vollen Stand neu und blendet die Meldung aus', async () => {
    const { getSessions } = openApp();
    await screen.findByText('Keine Sessions');
    act(() => FakeEventSource.current().emit({ type: 'hello' }));
    const before = getSessions.mock.calls.length;
    act(() => FakeEventSource.current().fail());
    await screen.findByRole('alert');
    act(() => FakeEventSource.current().emit({ type: 'hello' }));
    expect(await screen.findByText('verbunden')).toBeTruthy();
    expect(screen.queryByRole('alert')).toBeNull();
    expect(getSessions.mock.calls.length).toBeGreaterThan(before);
  });
});
