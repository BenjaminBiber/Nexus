// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { AccountsPanel } from '../src/components/AccountsPanel.js';
import type { AccountDto, AccountListDto } from '../src/api/usageTypes.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const NO_USAGE = { available: false, reason: null, plan: null, fiveHour: null, sevenDay: null, fetchedAt: null } as const;

const account = (id: string, extra: Partial<AccountDto> = {}): AccountDto => ({
  id, name: id, configDir: null, effectiveDir: `/home/.claude/${id}`, enabled: true, fallback: false,
  limitFiveHour: null, limitWeek: null, loggedIn: true, email: null, plugin: true, usage: { ...NO_USAGE },
  pausedUntil: null, references: [], login: null, ...extra,
});

const list = (...accounts: AccountDto[]): AccountListDto => ({
  accounts, multi: accounts.filter((a) => a.enabled).length > 1,
  accountsDir: '/home/.nexus/accounts', suggestions: ['~/.mcacc/profiles/firma/claude'], pluginCommands: [],
});

// Rendert das Panel mit austauschbaren Aufrufen; die erste Liste kommt aus getAccounts
function open(initial: AccountListDto, overrides: Record<string, unknown> = {}) {
  render(
    <ApiProvider api={fakeApi({ getAccounts: async () => initial, ...overrides })}>
      <AccountsPanel />
    </ApiProvider>,
  );
}

describe('AccountsPanel', () => {
  it('zeigt die Accounts des Servers samt Ordner neuer Accounts', async () => {
    open(list(account('standard', { name: 'Standard', email: 'ich@example.com' }), account('privat', { configDir: '~/privat', fallback: true })));
    expect(await screen.findByRole('button', { name: /Standard\s+standard/ })).toBeTruthy();
    expect(screen.getByRole('button', { name: /privat/ })).toBeTruthy();
    expect(screen.getByText(/Mehrere Accounts aktiv/)).toBeTruthy();
    expect(screen.getAllByText('Fallback').length).toBe(1);
  });

  it('legt über „Erweitert“ einen Account aus einem vorhandenen Ordner an', async () => {
    const created = list(account('standard', { name: 'Standard' }), account('mcacc', { name: 'MCACC Firma', configDir: '~/.mcacc/profiles/firma/claude' }));
    const createAccount = vi.fn(async () => created);
    open(list(account('standard', { name: 'Standard' })), { createAccount });
    await userEvent.type(await screen.findByLabelText('Name'), 'MCACC Firma');
    await userEvent.type(screen.getByLabelText('Ordner'), '~/.mcacc/profiles/firma/claude');
    await userEvent.click(screen.getByRole('button', { name: 'Übernehmen' }));
    expect(createAccount).toHaveBeenCalledWith('MCACC Firma', '~/.mcacc/profiles/firma/claude');
    expect(await screen.findByText('MCACC Firma')).toBeTruthy();
  });

  it('schaltet einen Account ab und übernimmt die Antwort des Servers', async () => {
    const off = list(account('privat', { configDir: '~/privat', enabled: false }));
    const updateAccount = vi.fn(async () => off);
    open(list(account('privat', { configDir: '~/privat' })), { updateAccount });
    await userEvent.click(await screen.findByRole('button', { name: /privat/ }));
    await userEvent.click(screen.getByLabelText('Aktiv'));
    expect(updateAccount).toHaveBeenCalledWith('privat', { enabled: false });
    expect(await screen.findByText('abgeschaltet')).toBeTruthy();
  });

  it('entfernt einen Account erst nach Rückfrage und nennt dabei die Verweise', async () => {
    const deleteAccount = vi.fn(async () => list(account('standard', { name: 'Standard' })));
    open(list(account('standard', { name: 'Standard' }), account('privat', { configDir: '~/privat', references: ['Profil schwer'] })), { deleteAccount });
    await userEvent.click(await screen.findByRole('button', { name: /privat/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Entfernen …' }));
    expect(screen.getByText(/Profil schwer/)).toBeTruthy();
    await userEvent.click(screen.getByLabelText(/Auch den Ordner samt Anmeldung löschen/));
    await userEvent.click(screen.getByRole('button', { name: 'Entfernen' }));
    expect(deleteAccount).toHaveBeenCalledWith('privat', true);
    expect(screen.queryByRole('button', { name: /privat/ })).toBeNull();
  });
});
