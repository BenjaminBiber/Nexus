// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { AccountsPanel } from '../src/components/AccountsPanel.js';
import { REFRESH_MS, RETRY_MS } from '../src/components/ClaudeUsageMeter.js';
import type { AccountDto, AccountListDto } from '../src/api/usageTypes.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const NO_USAGE = { available: false, reason: null, plan: null, fiveHour: null, sevenDay: null, fetchedAt: null } as const;

const account = (id: string, extra: Partial<AccountDto> = {}): AccountDto => ({
  id, name: id, configDir: null, effectiveDir: `/home/.claude/${id}`, enabled: true, fallback: false,
  limitFiveHour: null, limitWeek: null, loggedIn: true, email: null, plugin: true, runToken: false, usage: { ...NO_USAGE },
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
    expect(await screen.findByRole('article', { name: 'Standard' })).toBeTruthy();
    expect(screen.getByRole('article', { name: 'privat' })).toBeTruthy();
    expect(screen.getAllByText('Fallback').length).toBe(1);
    expect(screen.getAllByRole('button', { name: 'Nur als Fallback' }).map((b) => b.getAttribute('aria-pressed'))).toEqual(['false', 'true']);
  });

  it('zeigt die Limits eines Accounts als Ring mit Werten', async () => {
    const usage = { available: true, reason: null, plan: 'max', fiveHour: { percent: 22, resetsAt: null }, sevenDay: { percent: 35, resetsAt: null }, fetchedAt: null };
    open(list(account('standard', { name: 'Standard', usage, limitWeek: 80 })));
    expect(await screen.findByRole('img', { name: 'Sitzung 22 %, Woche 35 %' })).toBeTruthy();
    expect(screen.getByText('/ 80 %')).toBeTruthy();
  });

  it('zeigt ohne Limits einen leeren Ring und den Grund erst beim Darüberfahren', async () => {
    open(list(account('neu', { loggedIn: false })));
    const ring = await screen.findByRole('img', { name: 'Sitzung unbekannt, Woche unbekannt' });
    expect(ring.parentElement!.getAttribute('title')).toBe('Nicht angemeldet – keine Limits');
    expect(screen.queryByText('Nicht angemeldet – keine Limits')).toBeNull();
  });

  it('lädt nach einem vorübergehenden Fehlschlag schnell nach, sonst erst nach zwei Minuten', async () => {
    vi.useFakeTimers();
    try {
      const failed = { ...NO_USAGE, reason: 'token-expired' as const, plan: 'team' };
      const usage = { available: true, reason: null, plan: 'team', fiveHour: { percent: 43, resetsAt: null }, sevenDay: { percent: 32, resetsAt: null }, fetchedAt: null };
      let value = list(account('standard', { name: 'Standard', usage: failed }));
      let calls = 0;
      open(value, { getAccounts: async () => { calls++; return value; } });
      await act(() => vi.advanceTimersByTimeAsync(10));
      expect(screen.getByRole('img', { name: 'Sitzung unbekannt, Woche unbekannt' })).toBeTruthy();
      value = list(account('standard', { name: 'Standard', usage }));
      await act(() => vi.advanceTimersByTimeAsync(RETRY_MS + 10));
      expect(calls).toBe(2);
      expect(screen.getByRole('img', { name: 'Sitzung 43 %, Woche 32 %' })).toBeTruthy();
      // mit Werten erst nach REFRESH_MS
      await act(() => vi.advanceTimersByTimeAsync(RETRY_MS + 10));
      expect(calls).toBe(2);
      await act(() => vi.advanceTimersByTimeAsync(REFRESH_MS));
      expect(calls).toBe(3);
    } finally {
      vi.useRealTimers();
    }
  });

  it('legt im Dialog über „Erweitert“ einen Account aus einem vorhandenen Ordner an', async () => {
    const created = list(account('standard', { name: 'Standard' }), account('mcacc', { name: 'MCACC Firma', configDir: '~/.mcacc/profiles/firma/claude' }));
    const createAccount = vi.fn(async () => created);
    open(list(account('standard', { name: 'Standard' })), { createAccount });
    await userEvent.click(await screen.findByRole('button', { name: 'Account hinzufügen' }));
    const dialog = screen.getByRole('dialog', { name: 'Account hinzufügen' });
    await userEvent.type(within(dialog).getByLabelText('Name'), 'MCACC Firma');
    await userEvent.type(within(dialog).getByLabelText('Ordner'), '~/.mcacc/profiles/firma/claude');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Übernehmen' }));
    expect(createAccount).toHaveBeenCalledWith('MCACC Firma', '~/.mcacc/profiles/firma/claude');
    expect(await screen.findByRole('article', { name: 'MCACC Firma' })).toBeTruthy();
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('meldet im Dialog einen neuen Account mit Claude an', async () => {
    const loginNewAccount = vi.fn(async () => list(account('standard'), account('neu', { loggedIn: false })));
    open(list(account('standard')), { loginNewAccount });
    await userEvent.click(await screen.findByRole('button', { name: 'Account hinzufügen' }));
    await userEvent.type(screen.getByLabelText('E-Mail (optional)'), 'neu@example.com');
    await userEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Mit Claude anmelden' }));
    expect(loginNewAccount).toHaveBeenCalledWith('', 'neu@example.com');
    expect(await screen.findByRole('article', { name: 'neu' })).toBeTruthy();
  });

  it('schaltet einen Account ab und übernimmt die Antwort des Servers', async () => {
    const off = list(account('privat', { configDir: '~/privat', enabled: false }));
    const updateAccount = vi.fn(async () => off);
    open(list(account('privat', { configDir: '~/privat' })), { updateAccount });
    await userEvent.click(await screen.findByRole('button', { name: 'Aktiv' }));
    expect(updateAccount).toHaveBeenCalledWith('privat', { enabled: false });
    expect(await screen.findByText('abgeschaltet')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Aktiv' }).getAttribute('aria-pressed')).toBe('false');
  });

  it('benennt einen Account im Einstellungsdialog um', async () => {
    const updateAccount = vi.fn(async () => list(account('privat', { name: 'Privat Max' })));
    open(list(account('privat')), { updateAccount });
    await userEvent.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const input = within(screen.getByRole('dialog', { name: 'privat' })).getByLabelText('Name');
    await userEvent.clear(input);
    await userEvent.type(input, 'Privat Max{Enter}');
    expect(updateAccount).toHaveBeenCalledWith('privat', { name: 'Privat Max' });
    await userEvent.click(screen.getByRole('button', { name: 'Fertig' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(screen.getByRole('article', { name: 'Privat Max' })).toBeTruthy();
  });

  it('meldet einen angemeldeten Account über das Icon neu an', async () => {
    const loginAccount = vi.fn(async () => list(account('privat', { configDir: '~/privat' })));
    open(list(account('privat', { configDir: '~/privat' })), { loginAccount });
    await userEvent.click(await screen.findByRole('button', { name: 'Neu anmelden' }));
    expect(loginAccount).toHaveBeenCalledWith('privat');
  });

  it('setzt das Langzeit-Token im Einstellungsdialog und zeigt danach nur, dass es gesetzt ist', async () => {
    const setAccountRunToken = vi.fn(async () => list(account('standard', { runToken: true })));
    const removeAccountRunToken = vi.fn(async () => list(account('standard')));
    open(list(account('standard')), { setAccountRunToken, removeAccountRunToken });
    await userEvent.click(await screen.findByRole('button', { name: 'Einstellungen' }));
    const dialog = screen.getByRole('dialog', { name: 'standard' });
    expect(within(dialog).getByText('claude setup-token')).toBeTruthy();
    await userEvent.type(within(dialog).getByLabelText('Langzeit-Token'), ' sk-ant-oat01-geheim {Enter}');
    expect(setAccountRunToken).toHaveBeenCalledWith('standard', 'sk-ant-oat01-geheim');
    expect(await within(dialog).findByText('gesetzt')).toBeTruthy();
    expect(screen.queryByDisplayValue(/sk-ant/)).toBeNull();
    expect(screen.getByText('Langzeit-Token')).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Entfernen' }));
    expect(removeAccountRunToken).toHaveBeenCalledWith('standard');
  });

  it('entfernt einen Account erst nach Rückfrage und nennt dabei die Verweise', async () => {
    const deleteAccount = vi.fn(async () => list(account('standard', { name: 'Standard' })));
    open(list(account('standard', { name: 'Standard' }), account('privat', { configDir: '~/privat', references: ['Profil schwer'] })), { deleteAccount });
    const tile = await screen.findByRole('article', { name: 'privat' });
    await userEvent.click(within(tile).getByRole('button', { name: 'Einstellungen' }));
    const dialog = screen.getByRole('dialog', { name: 'privat' });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Entfernen …' }));
    expect(screen.getByText(/Profil schwer/)).toBeTruthy();
    await userEvent.click(screen.getByLabelText(/Auch den Ordner samt Anmeldung löschen/));
    await userEvent.click(screen.getByRole('button', { name: 'Entfernen' }));
    expect(deleteAccount).toHaveBeenCalledWith('privat', true);
    expect(screen.queryByRole('article', { name: 'privat' })).toBeNull();
  });
});
