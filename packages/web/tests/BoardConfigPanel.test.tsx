// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { BoardConfigDto, BoardConfigInput } from '../src/api/types.js';
import { BoardConfigPanel } from '../src/components/BoardConfigPanel.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const EMPTY: BoardConfigDto = {
  configured: false, file: '/home/.harness/board.yaml', provider: 'mssql', dbFile: '/home/.harness/board.db',
  server: '', port: 1433, database: 'AiHarness', user: '',
  hasPassword: false, encrypt: true, trustServerCertificate: true, userId: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa', userName: '', activeBoard: null,
};

// Rendert das Panel mit austauschbaren Aufrufen
function open(initial: BoardConfigDto, overrides: Record<string, unknown> = {}) {
  render(
    <ApiProvider api={fakeApi({ getBoardConfig: async () => initial, ...overrides })}>
      <BoardConfigPanel />
    </ApiProvider>,
  );
}

describe('BoardConfigPanel', () => {
  it('übernimmt Standardwerte und speichert mit Passwort', async () => {
    const save = vi.fn(async (input: BoardConfigInput) => ({ ...EMPTY, ...input, configured: true, hasPassword: true }));
    open(EMPTY, { saveBoardConfig: save });
    await userEvent.type(await screen.findByLabelText('Server'), 'dev-sql-5');
    await userEvent.type(screen.getByLabelText('Benutzer'), 'harness');
    await userEvent.type(screen.getByLabelText('Passwort'), 'geheim');
    await userEvent.type(screen.getByLabelText('Dein Name'), 'Florian');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({
      server: 'dev-sql-5', port: 1433, database: 'AiHarness', user: 'harness', password: 'geheim',
      userName: 'Florian', userId: EMPTY.userId,
    }));
    expect((await screen.findByRole('status')).textContent).toContain('Gespeichert');
  });

  it('schickt ohne neue Eingabe kein Passwort mit', async () => {
    const save = vi.fn(async (input: BoardConfigInput) => ({ ...EMPTY, ...input, configured: true, hasPassword: true }));
    open({ ...EMPTY, configured: true, server: 's', user: 'u', userName: 'F', hasPassword: true }, { saveBoardConfig: save });
    await userEvent.click(await screen.findByRole('button', { name: 'Speichern' }));
    expect(save.mock.calls[0][0]).not.toHaveProperty('password');
  });

  it('zeigt das Ergebnis des Verbindungstests', async () => {
    const test = vi.fn(async () => ({ ok: false, message: 'Board-Datenbank AiHarness auf s:1433 ist nicht erreichbar' }));
    open({ ...EMPTY, configured: true, server: 's', user: 'u', userName: 'F', hasPassword: true }, { testBoardConfig: test });
    await userEvent.click(await screen.findByRole('button', { name: 'Verbindung testen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('nicht erreichbar');
  });

  it('zeigt standardmäßig SQL Server und blendet beim Umschalten die Server-Felder aus', async () => {
    open(EMPTY);
    await screen.findByLabelText('Server');
    expect(screen.getByRole('button', { name: 'SQL Server' }).getAttribute('aria-pressed')).toBe('true');
    await userEvent.click(screen.getByRole('button', { name: 'SQLite (lokal)' }));
    expect(screen.queryByLabelText('Server')).toBeNull();
    expect(screen.queryByLabelText('Passwort')).toBeNull();
    expect((screen.getByLabelText('SQLite-Datei') as HTMLInputElement).value).toBe('/home/.harness/board.db');
    expect(screen.getByRole('note').textContent).toContain('Nur dieser Rechner');
    expect(screen.getByLabelText('Dein Name')).toBeTruthy();
  });

  it('behält die SQL-Server-Werte beim Zurückschalten', async () => {
    open(EMPTY);
    await userEvent.type(await screen.findByLabelText('Server'), 'dev-sql-5');
    await userEvent.click(screen.getByRole('button', { name: 'SQLite (lokal)' }));
    await userEvent.click(screen.getByRole('button', { name: 'SQL Server' }));
    expect((screen.getByLabelText('Server') as HTMLInputElement).value).toBe('dev-sql-5');
  });

  it('speichert mit SQLite Anbieter und Dateipfad', async () => {
    const save = vi.fn(async (input: BoardConfigInput) => ({ ...EMPTY, ...input, configured: true }));
    open(EMPTY, { saveBoardConfig: save });
    await userEvent.click(await screen.findByRole('button', { name: 'SQLite (lokal)' }));
    await userEvent.clear(screen.getByLabelText('SQLite-Datei'));
    await userEvent.type(screen.getByLabelText('SQLite-Datei'), 'C:/daten/board.db');
    await userEvent.type(screen.getByLabelText('Dein Name'), 'Florian');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({
      provider: 'sqlite', dbFile: 'C:/daten/board.db', userName: 'Florian',
    }));
    expect((await screen.findByRole('status')).textContent).toContain('Gespeichert');
  });

  it('testet mit SQLite und zeigt die Erfolgsmeldung', async () => {
    const test = vi.fn(async () => ({ ok: true, message: 'Datei geöffnet, Schema passt.' }));
    open(EMPTY, { testBoardConfig: test });
    await userEvent.click(await screen.findByRole('button', { name: 'SQLite (lokal)' }));
    await userEvent.click(screen.getByRole('button', { name: 'Verbindung testen' }));
    expect(test).toHaveBeenCalledWith(expect.objectContaining({ provider: 'sqlite', dbFile: '/home/.harness/board.db' }));
    expect((await screen.findByRole('status')).textContent).toContain('Datei geöffnet');
  });

  it('öffnet eine gespeicherte SQLite-Konfig mit aktivem SQLite', async () => {
    open({ ...EMPTY, configured: true, provider: 'sqlite', dbFile: 'D:/b.db', userName: 'F' });
    expect((await screen.findByRole('button', { name: 'SQLite (lokal)' })).getAttribute('aria-pressed')).toBe('true');
    expect((screen.getByLabelText('SQLite-Datei') as HTMLInputElement).value).toBe('D:/b.db');
    expect(screen.queryByLabelText('Server')).toBeNull();
  });
});
