// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, createEvent, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DispatchConfigViewDto, RawDispatchConfigDto } from '../src/api/dispatchTypes.js';
import { DispatchPanel } from '../src/components/DispatchPanel.js';
import { dbBoards, fakeApi, suggestion, suggestionList } from './helpers/fakeApi.js';

afterEach(cleanup);

const RAW: RawDispatchConfigDto = {
  enabled: false, boards: null, wip: 1, window: null, optInLabel: 'auto', optOutLabel: 'manuell', maxAttempts: 2, pauseMinutesOnLimit: 60,
  profiles: { standard: { model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 90 } }, defaultProfile: 'standard', rules: [], escalate: true, allowFable: false,
  claude: { command: 'claude', permissionMode: 'acceptEdits', allowedTools: ['Read'] }, promptExtra: '',
};
const VIEW: DispatchConfigViewDto = { file: 'C:/h/dispatch.yaml', exists: false, raw: RAW, config: { enabled: false, staleMinutes: 120, ladder: ['standard'] }, error: null };
const BOARDS = dbBoards.map((b) => (b.id === 2 ? { ...b, repoPath: 'C:/repo' } : b));

// Rendert das Panel mit austauschbaren Aufrufen
function open(overrides: Record<string, unknown> = {}) {
  const api = fakeApi({ getDispatchConfig: async () => VIEW, getDbBoards: async () => BOARDS, ...overrides });
  render(<ApiProvider api={api}><DispatchPanel /></ApiProvider>);
  return api;
}

describe('DispatchPanel', () => {
  it('speichert Schalter, Boards, Grenzen und Profile', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...VIEW, exists: true, raw }));
    open({ saveDispatchConfig: save });
    await userEvent.click(await screen.findByLabelText('Dispatcher eingeschaltet'));
    await userEvent.click(screen.getByLabelText('ARC · Nächster'));
    expect((screen.getByLabelText(/ARC · Sprint/) as HTMLInputElement).disabled).toBe(true);
    await userEvent.clear(screen.getByLabelText('Gleichzeitig (WIP)'));
    await userEvent.type(screen.getByLabelText('Gleichzeitig (WIP)'), '2');
    await userEvent.click(screen.getByRole('button', { name: '+ Profil' }));
    await userEvent.type(screen.getByLabelText('Profil 2 Name'), 'klein');
    await userEvent.type(screen.getByLabelText('Profil 2 Modell'), 'sonnet');
    await userEvent.click(screen.getByRole('button', { name: '+ Regel' }));
    await userEvent.type(screen.getByLabelText('Regel 1 Labels'), 'doku');
    await userEvent.selectOptions(screen.getByLabelText('Regel 1 Profil'), 'klein');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({
      enabled: true, boards: [2], wip: 2,
      profiles: { standard: RAW.profiles!.standard, klein: { model: 'sonnet', effort: null, maxTurns: undefined, timeoutMinutes: undefined } },
      rules: [{ when: { labels: ['doku'] }, profile: 'klein' }],
    }));
    expect((await screen.findByRole('status')).textContent).toContain('Gespeichert in C:/h/dispatch.yaml');
  });

  it('speichert kurz nach der letzten Eingabe automatisch, unfertige Eingaben aber nicht', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...VIEW, exists: true, raw }));
    open({ saveDispatchConfig: save });
    const wip = await screen.findByLabelText('Gleichzeitig (WIP)');
    await userEvent.clear(wip);
    await userEvent.type(wip, 'x');
    expect(await screen.findByText(/Nicht automatisch gespeichert/, {}, { timeout: 3000 })).toBeTruthy();
    expect(save).not.toHaveBeenCalled();
    await userEvent.clear(wip);
    await userEvent.type(wip, '3');
    expect(await screen.findByText(/Automatisch gespeichert um/, {}, { timeout: 3000 })).toBeTruthy();
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ wip: 3 }));
  });

  describe('Auto-Speichern mit unfertigen Zeilen', () => {
    // Wie der Server: Regeln brauchen Bedingung und Profil, Einrichtungsbefehle einen Befehl, Zeitfenster HH:MM
    const strict = vi.fn(async (raw: RawDispatchConfigDto) => {
      raw.rules?.forEach((r, i) => {
        if (!Object.keys(r.when ?? {}).length) throw new ApiError('dispatch-config-invalid', `rules[${i}]: „when“ braucht mindestens eine Bedingung.`, 422);
      });
      raw.worktreeSetup?.forEach((s, i) => {
        if (!s.run) throw new ApiError('dispatch-config-invalid', `worktreeSetup[${i}]: run (Befehl) fehlt.`, 422);
      });
      return { ...VIEW, exists: true, raw };
    });
    const settle = (text: RegExp) => screen.findByText(text, {}, { timeout: 3000 });

    it('behält eine neue leere Zeile (Profil, Regel, Befehl), statt sie zu löschen oder zu bemängeln', async () => {
      strict.mockClear();
      open({ saveDispatchConfig: strict });
      await userEvent.click(await screen.findByRole('button', { name: '+ Profil' }));
      await userEvent.click(screen.getByRole('button', { name: '+ Regel' }));
      await userEvent.click(screen.getByRole('button', { name: '+ Befehl' }));
      await settle(/Nicht automatisch gespeichert/);
      expect(screen.queryByRole('alert')).toBeNull();
      expect(screen.getByLabelText('Profil 2 Name')).toBeTruthy();
      expect(screen.getByLabelText('Regel 1 Labels')).toBeTruthy();
      expect(screen.getByLabelText('Einrichtung 1 Befehl')).toBeTruthy();
    });

    it('lässt eine leere Profilzeile stehen, auch wenn die übrigen Eingaben automatisch gespeichert werden', async () => {
      strict.mockClear();
      open({ saveDispatchConfig: strict });
      await userEvent.click(await screen.findByRole('button', { name: '+ Profil' }));
      await userEvent.clear(screen.getByLabelText('Gleichzeitig (WIP)'));
      await userEvent.type(screen.getByLabelText('Gleichzeitig (WIP)'), '3');
      await settle(/Automatisch gespeichert um/);
      expect(strict).toHaveBeenCalledWith(expect.objectContaining({ wip: 3 }));
      expect(screen.getByLabelText('Profil 2 Name')).toBeTruthy();
    });

    it('zeigt beim Auto-Speichern keinen Fehler, wenn der Server eine halbfertige Eingabe ablehnt', async () => {
      const save = vi.fn(async () => { throw new ApiError('dispatch-config-invalid', 'window braucht from und to im Format HH:MM.', 422); });
      open({ saveDispatchConfig: save });
      await userEvent.type(await screen.findByLabelText('Von (HH:MM)'), '18:');
      expect((await settle(/Nicht automatisch gespeichert: window braucht/)).textContent).toContain('HH:MM');
      expect(screen.queryByRole('alert')).toBeNull();
    });

    it('speichert die Zeile, sobald sie vollständig ist', async () => {
      strict.mockClear();
      open({ saveDispatchConfig: strict });
      await userEvent.click(await screen.findByRole('button', { name: '+ Regel' }));
      await userEvent.type(screen.getByLabelText('Regel 1 Labels'), 'doku');
      await userEvent.selectOptions(screen.getByLabelText('Regel 1 Profil'), 'standard');
      await settle(/Automatisch gespeichert um/);
      expect(strict).toHaveBeenLastCalledWith(expect.objectContaining({ rules: [{ when: { labels: ['doku'] }, profile: 'standard' }] }));
    });

    it('eine manuell gespeicherte unvollständige Regel zeigt weiterhin den Fehler', async () => {
      strict.mockClear();
      open({ saveDispatchConfig: strict });
      await userEvent.click(await screen.findByRole('button', { name: '+ Regel' }));
      await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
      expect((await screen.findByRole('alert')).textContent).toContain('rules[0]');
    });

    it('speichert ein Profil mit Modell, aber ohne Namen nicht automatisch', async () => {
      strict.mockClear();
      open({ saveDispatchConfig: strict });
      await userEvent.click(await screen.findByRole('button', { name: '+ Profil' }));
      await userEvent.type(screen.getByLabelText('Profil 2 Modell'), 'sonnet');
      await settle(/Nicht automatisch gespeichert: Profil 2 braucht einen Namen/);
      expect(strict).not.toHaveBeenCalled();
      expect((screen.getByLabelText('Profil 2 Modell') as HTMLInputElement).value).toBe('sonnet');
    });
  });

  it('speichert die Merge-Strategie', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...VIEW, exists: true, raw }));
    open({ saveDispatchConfig: save });
    const select = (await screen.findByLabelText('Zurückmergen')) as HTMLSelectElement;
    expect(select.value).toBe('merge');
    expect([...select.options].map((o) => o.textContent)).toEqual(['Merge-Commit (alle Commits bleiben)', 'Squash (ein Commit je Ticket)']);
    await userEvent.selectOptions(select, 'squash');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ mergeStrategy: 'squash' }));
  });

  it('Freigabe vor dem Merge ist standardmäßig an; ausschalten speichert reviewBeforeMerge: false', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...VIEW, exists: true, raw }));
    open({ saveDispatchConfig: save });
    const box = (await screen.findByLabelText('Vor dem Merge auf Freigabe warten')) as HTMLInputElement;
    expect(box.checked).toBe(true);
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save.mock.calls[0][0]).not.toHaveProperty('reviewBeforeMerge');
    await userEvent.click(box);
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ reviewBeforeMerge: false }));
  });

  it('pflegt die Einrichtung neuer Worktrees als Liste aus Ordner und Befehl', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...VIEW, exists: true, raw }));
    open({ saveDispatchConfig: save });
    await userEvent.click(await screen.findByRole('button', { name: '+ Befehl' }));
    await userEvent.click(screen.getByRole('button', { name: '+ Befehl' }));
    await userEvent.type(screen.getByLabelText('Einrichtung 1 Ordner'), 'BackofficeNextGen');
    await userEvent.type(screen.getByLabelText('Einrichtung 1 Befehl'), 'npm ci');
    await userEvent.type(screen.getByLabelText('Einrichtung 2 Befehl'), 'weg');
    await userEvent.click(screen.getByRole('button', { name: 'Einrichtung 2 entfernen' }));
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ worktreeSetup: [{ cwd: 'BackofficeNextGen', run: 'npm ci' }] }));
  });

  it('speichert, ob Tickets einer Spec einen Worktree teilen', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...VIEW, exists: true, raw }));
    open({ saveDispatchConfig: save });
    const select = (await screen.findByLabelText('Worktrees')) as HTMLSelectElement;
    expect(select.value).toBe('none');
    expect([...select.options].map((o) => o.textContent)).toEqual(['Ein Worktree je Ticket', 'Tickets einer Spec bündeln (alt)']);
    await userEvent.selectOptions(select, 'sameSpec');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ joinWorktrees: 'sameSpec' }));
  });

  it('speichert die Einstellungen der Recherchen', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...VIEW, exists: true, raw }));
    open({ saveDispatchConfig: save });
    await userEvent.click(await screen.findByLabelText('Neue Fragen zu meinen Boards automatisch recherchieren lassen'));
    expect(screen.getByLabelText('Automatische Recherchen je Tag')).toHaveProperty('value', '10');
    await userEvent.clear(screen.getByLabelText('Gleichzeitige Recherchen (1–5)'));
    await userEvent.type(screen.getByLabelText('Gleichzeitige Recherchen (1–5)'), '6');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Gleichzeitige Recherchen muss eine ganze Zahl von 1 bis 5 sein.');
    expect(save).not.toHaveBeenCalled();
    await userEvent.clear(screen.getByLabelText('Gleichzeitige Recherchen (1–5)'));
    await userEvent.type(screen.getByLabelText('Gleichzeitige Recherchen (1–5)'), '2');
    await userEvent.clear(screen.getByLabelText('Automatische Recherchen je Tag'));
    await userEvent.type(screen.getByLabelText('Automatische Recherchen je Tag'), '4');
    await userEvent.selectOptions(screen.getByLabelText('Profil der Recherche'), 'standard');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ research: { auto: true, wip: 2, maxPerDay: 4, profile: 'standard' } }));
  });

  it('zeigt Prüffehler des Servers', async () => {
    open({ saveDispatchConfig: async () => { throw new ApiError('dispatch-config-invalid', 'wip muss eine ganze Zahl ab 0 sein.', 422); } });
    await userEvent.click(await screen.findByRole('button', { name: 'Speichern' }));
    expect((await screen.findByRole('alert')).textContent).toContain('wip muss eine ganze Zahl ab 0 sein.');
  });

  it('meldet eine kaputte Datei', async () => {
    open({ getDispatchConfig: async () => ({ ...VIEW, exists: true, config: null, error: 'dispatch.yaml ist kein gültiges YAML.' }) });
    const panel = await screen.findByRole('region', { name: 'Dispatcher' });
    expect(within(panel).getByRole('alert').textContent).toContain('kein gültiges YAML');
  });
});

describe('DispatchPanel Prüfungen', () => {
  const rawWith = (extra: Partial<RawDispatchConfigDto>): RawDispatchConfigDto => ({ ...RAW, ...extra });

  it('wählt die Accounts eines Profils im Dropdown aus und behält unbekannte IDs', async () => {
    const raw = rawWith({ profiles: { standard: { accounts: ['alt'] } } });
    const save = vi.fn(async (r: RawDispatchConfigDto) => ({ ...VIEW, raw: r }));
    const account = (id: string, name: string, enabled = true) => ({ id, name, configDir: null, effectiveDir: '', enabled, fallback: false });
    open({
      getDispatchConfig: async () => ({ ...VIEW, raw }),
      saveDispatchConfig: save,
      getAccounts: async () => ({ accounts: [account('max', 'Max'), account('pro', 'Pro', false)], multi: true, accountsDir: '', suggestions: [], pluginCommands: [] }),
    });
    const button = await screen.findByRole('button', { name: 'Profil 1 Accounts' });
    expect(button.textContent).toBe('alt');
    await userEvent.click(button);
    const menu = screen.getByRole('group', { name: 'Profil 1 Accounts' });
    expect(within(menu).getByText('aus')).toBeTruthy();
    expect(within(menu).getByText('unbekannt')).toBeTruthy();
    await userEvent.click(within(menu).getByLabelText('Max'));
    await userEvent.click(within(menu).getByLabelText('alt'));
    expect(button.textContent).toBe('Max');
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('group', { name: 'Profil 1 Accounts' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ profiles: { standard: expect.objectContaining({ accounts: ['max'] }) } }));
  });

  it('wählt die Accounts einer Regel im Dropdown aus', async () => {
    const raw = rawWith({ rules: [{ when: { labels: ['doku'] }, profile: 'standard' }] });
    const save = vi.fn(async (r: RawDispatchConfigDto) => ({ ...VIEW, raw: r }));
    const account = (id: string, name: string) => ({ id, name, configDir: null, effectiveDir: '', enabled: true, fallback: false });
    open({
      getDispatchConfig: async () => ({ ...VIEW, raw }),
      saveDispatchConfig: save,
      getAccounts: async () => ({ accounts: [account('max', 'Max'), account('pro', 'Pro')], multi: true, accountsDir: '', suggestions: [], pluginCommands: [] }),
    });
    const button = await screen.findByRole('button', { name: 'Regel 1 Accounts' });
    expect(button.textContent).toBe('wie Profil');
    await userEvent.click(button);
    const menu = screen.getByRole('group', { name: 'Regel 1 Accounts' });
    await userEvent.click(await within(menu).findByLabelText('Pro'));
    await userEvent.click(within(menu).getByLabelText('Max'));
    expect(button.textContent).toBe('Pro, Max');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ rules: [{ when: { labels: ['doku'] }, profile: 'standard', accounts: ['pro', 'max'] }] }));
  });

  it('sortiert Profile per Ziehen und Pfeiltasten um, ohne Verweise zu verändern', async () => {
    const raw = rawWith({ profiles: { klein: {}, standard: {}, schwer: {} }, defaultProfile: 'standard', rules: [{ when: { labels: ['doku'] }, profile: 'klein' }] });
    const save = vi.fn(async (r: RawDispatchConfigDto) => ({ ...VIEW, raw: r }));
    open({ getDispatchConfig: async () => ({ ...VIEW, raw }), saveDispatchConfig: save });
    const names = () => [1, 2, 3].map((n) => (screen.getByLabelText(`Profil ${n} Name`) as HTMLInputElement).value);
    await screen.findByLabelText('Profil 1 Name');
    expect(names()).toEqual(['klein', 'standard', 'schwer']);

    // Ziehen: erstes Profil auf die dritte Zeile fallen lassen
    const dataTransfer = { setData: vi.fn(), setDragImage: vi.fn(), effectAllowed: '', dropEffect: '' };
    const drag = (el: Element, type: 'dragStart' | 'dragOver' | 'drop') => fireEvent(el, Object.assign(createEvent[type](el), { dataTransfer }));
    drag(screen.getByRole('button', { name: 'Profil 1 verschieben' }), 'dragStart');
    const target = screen.getByLabelText('Profil 3 Name').closest('.dispatch-row')!;
    drag(target, 'dragOver');
    drag(target, 'drop');
    expect(names()).toEqual(['standard', 'schwer', 'klein']);

    // Tastatur: letztes Profil eine Stelle nach oben
    screen.getByRole('button', { name: 'Profil 3 verschieben' }).focus();
    await userEvent.keyboard('{ArrowUp}');
    expect(names()).toEqual(['standard', 'klein', 'schwer']);

    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    const saved = save.mock.calls[0][0];
    expect(Object.keys(saved.profiles ?? {})).toEqual(['standard', 'klein', 'schwer']);
    expect(saved.defaultProfile).toBe('standard');
    expect(saved.rules).toEqual([{ when: { labels: ['doku'] }, profile: 'klein' }]);
  });

  it('zieht Umbenennung in Standardprofil und Regeln nach und leert beim Löschen', async () => {
    const raw = rawWith({ profiles: { standard: {}, klein: {} }, rules: [{ when: { labels: ['doku'] }, profile: 'klein' }] });
    const save = vi.fn(async (r: RawDispatchConfigDto) => ({ ...VIEW, raw: r }));
    open({ getDispatchConfig: async () => ({ ...VIEW, raw }), saveDispatchConfig: save });
    await userEvent.type(await screen.findByLabelText('Profil 2 Name'), 'er');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ defaultProfile: 'standard', rules: [{ when: { labels: ['doku'] }, profile: 'kleiner' }] }));
    await userEvent.click(screen.getAllByRole('button', { name: 'Entfernen' })[1]);
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ rules: [{ when: { labels: ['doku'] }, profile: '' }] }));
  });

  it('zeigt einen unbekannten Wert als ungültig', async () => {
    open({ getDispatchConfig: async () => ({ ...VIEW, raw: rawWith({ defaultProfile: 'weg' }) }) });
    expect(await screen.findByRole('option', { name: '(ungültig: weg)' })).toBeTruthy();
  });

  it('meldet doppelte Profilnamen und ungültige Zahlen ohne zu senden', async () => {
    const save = vi.fn(async (r: RawDispatchConfigDto) => ({ ...VIEW, raw: r }));
    open({ saveDispatchConfig: save });
    await userEvent.click(await screen.findByRole('button', { name: '+ Profil' }));
    await userEvent.type(screen.getByLabelText('Profil 2 Name'), ' standard ');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Profilname standard doppelt');
    await userEvent.clear(screen.getByLabelText('Profil 2 Name'));
    await userEvent.type(screen.getByLabelText('Gleichzeitig (WIP)'), 'x');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Gleichzeitig (WIP) muss eine ganze Zahl sein.');
    expect(save).not.toHaveBeenCalled();
  });

  it('erlaubt das Abwählen eines Boards ohne Repo-Pfad', async () => {
    open({ getDispatchConfig: async () => ({ ...VIEW, raw: rawWith({ boards: [1] }) }) });
    const boxes = (await screen.findAllByRole('checkbox')).filter((c) => (c as HTMLInputElement).checked && !(c as HTMLInputElement).disabled);
    expect(boxes.length).toBeGreaterThan(0);
  });

  it('führt „Optimierungen“ als Unterbereich direkt nach „Profile & Regeln“', async () => {
    const { DISPATCH_SECTIONS } = await import('../src/components/DispatchPanel.js');
    const ids = DISPATCH_SECTIONS.map((s) => s.id);
    expect(ids.indexOf('optimize')).toBe(ids.indexOf('profiles') + 1);
    const api = fakeApi({ getDispatchConfig: async () => VIEW, getDispatchSuggestions: async () => suggestionList() });
    render(<ApiProvider api={api}><DispatchPanel sub="optimize" /></ApiProvider>);
    expect(await screen.findByRole('heading', { name: 'Optimierungen' })).toBeTruthy();
    expect(screen.queryByRole('heading', { name: 'Profile' })).toBeNull();
  });

  it('übernimmt einen Vorschlag ins Formular, ohne danach automatisch zu speichern', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...VIEW, exists: true, raw }));
    const appliedRaw = { ...RAW, profiles: { standard: { ...RAW.profiles!.standard!, maxTurns: 120 } } };
    open({
      saveDispatchConfig: save,
      getDispatchSuggestions: async () => suggestionList({ suggestions: [suggestion()] }),
      applyDispatchSuggestion: async () => ({ view: { ...VIEW, exists: true, raw: appliedRaw }, list: suggestionList() }),
    });
    const card = await screen.findByRole('article', { name: 'Schritte zu knapp · standard' });
    await userEvent.click(within(card).getByRole('button', { name: 'Übernehmen' }));
    expect(((await screen.findByLabelText('Profil 1 Schritte')) as HTMLInputElement).value).toBe('120');
    await new Promise((r) => setTimeout(r, 1500));
    expect(save).not.toHaveBeenCalled();
  });

  describe('Prompt-Vorlagen', () => {
    const PROMPT_INFO = [
      { kind: 'work' as const, label: 'Umsetzung', default: 'Standard-Umsetzung {{ticketId}}', required: ['harness board done'], placeholders: [{ name: 'ticketId', description: 'Ticket-ID' }, { name: 'agentName', description: 'Name des Agenten' }] },
      { kind: 'plan' as const, label: 'Planung', default: 'Standard-Planung', required: [], placeholders: [] },
      { kind: 'research' as const, label: 'Recherche', default: 'Standard-Recherche', required: [], placeholders: [] },
      { kind: 'merge' as const, label: 'Merge', default: 'Standard-Merge', required: [], placeholders: [] },
    ];
    const view = (raw: RawDispatchConfigDto = RAW): DispatchConfigViewDto => ({ ...VIEW, raw, promptInfo: PROMPT_INFO });
    const prompts = (overrides: Record<string, unknown> = {}, raw?: RawDispatchConfigDto) => {
      const api = fakeApi({ getDispatchConfig: async () => view(raw), getDbBoards: async () => BOARDS, ...overrides });
      render(<ApiProvider api={api}><DispatchPanel sub="prompts" /></ApiProvider>);
      return api;
    };
    const editor = (label: string) => screen.getByRole('group', { name: `Prompt-Vorlage ${label}` });

    it('führt „Prompts“ als Unterbereich mit einem Editor je Laufart', async () => {
      const { DISPATCH_SECTIONS } = await import('../src/components/DispatchPanel.js');
      expect(DISPATCH_SECTIONS.map((s) => s.id)).toContain('prompts');
      prompts();
      expect(await screen.findByLabelText('Vorlage Umsetzung')).toBeTruthy();
      for (const label of ['Umsetzung', 'Planung', 'Recherche', 'Merge']) {
        expect(within(editor(label)).getByText('Standard')).toBeTruthy();
      }
      expect(screen.queryByRole('heading', { name: 'Claude' })).toBeNull();
    });

    it('speichert eine geänderte Vorlage als prompts.<art>, die übrigen als null', async () => {
      const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...view(raw), exists: true }));
      prompts({ saveDispatchConfig: save });
      const area = await screen.findByLabelText('Vorlage Umsetzung');
      fireEvent.change(area, { target: { value: '  Eigene Vorlage {{ticketId}}\n' } });
      expect(within(editor('Umsetzung')).getByText('angepasst')).toBeTruthy();
      await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
      expect(save).toHaveBeenCalledWith(expect.objectContaining({
        prompts: { work: '  Eigene Vorlage {{ticketId}}\n', plan: null, research: null, merge: null },
      }));
    });

    it('setzt eine Vorlage auf Standard zurück und speichert null', async () => {
      const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...view(raw), exists: true }));
      prompts({ saveDispatchConfig: save }, rawWith({ prompts: { work: 'eigene Vorlage' } }));
      await screen.findByLabelText('Vorlage Umsetzung');
      expect(within(editor('Umsetzung')).getByText('angepasst')).toBeTruthy();
      await userEvent.click(screen.getByRole('button', { name: 'Umsetzung auf Standard zurücksetzen' }));
      expect(within(editor('Umsetzung')).getByText('Standard')).toBeTruthy();
      await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
      expect(save).toHaveBeenCalledWith(expect.objectContaining({ prompts: { work: null, plan: null, research: null, merge: null } }));
    });

    it('fügt einen Platzhalter an der Cursorposition ein', async () => {
      prompts();
      const area = (await screen.findByLabelText('Vorlage Umsetzung')) as HTMLTextAreaElement;
      fireEvent.change(area, { target: { value: 'ab' } });
      area.focus();
      area.setSelectionRange(1, 1);
      await userEvent.click(within(editor('Umsetzung')).getByRole('button', { name: /\{\{ticketId\}\}/ }));
      expect(area.value).toBe('a{{ticketId}}b');
      expect(area.selectionStart).toBe('a{{ticketId}}'.length);
    });

    it('zeigt eine Ablehnung des Servers am betroffenen Editor statt oben', async () => {
      const save = vi.fn(async () => {
        throw new ApiError('dispatch-prompt-invalid', 'Vorlage „work“: fehlende Pflichtteile.', 400, undefined, {
          kind: 'work', missing: ['harness board done'], unknown: [],
        });
      });
      prompts({ saveDispatchConfig: save }, rawWith({ prompts: { work: 'eigene Vorlage' } }));
      await screen.findByLabelText('Vorlage Umsetzung');
      await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
      const alert = await within(editor('Umsetzung')).findByRole('alert');
      expect(alert.textContent).toContain('harness board done');
      expect(within(editor('Planung')).queryByRole('alert')).toBeNull();
      expect(screen.getAllByRole('alert')).toHaveLength(1);
    });

    it('nimmt die Meldung nach erfolgreichem Speichern wieder weg', async () => {
      let fail = true;
      const save = vi.fn(async (raw: RawDispatchConfigDto) => {
        if (fail) throw new ApiError('dispatch-prompt-invalid', 'Vorlage „work“ unvollständig.', 400, undefined, { kind: 'work', missing: ['harness board done'], unknown: [] });
        return { ...view(raw), exists: true };
      });
      prompts({ saveDispatchConfig: save }, rawWith({ prompts: { work: 'eigene Vorlage' } }));
      await screen.findByLabelText('Vorlage Umsetzung');
      await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
      expect(await within(editor('Umsetzung')).findByRole('alert')).toBeTruthy();
      fail = false;
      await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
      await screen.findByText(/Gespeichert in/);
      expect(within(editor('Umsetzung')).queryByRole('alert')).toBeNull();
    });

    it('meldet eine abgelehnte Vorlage beim Auto-Speichern nur als „nicht automatisch gespeichert“', async () => {
      const save = vi.fn(async () => {
        throw new ApiError('dispatch-prompt-invalid', 'Vorlage „work“ unvollständig.', 400, undefined, { kind: 'work', missing: ['harness board done'], unknown: [] });
      });
      prompts({ saveDispatchConfig: save });
      const area = await screen.findByLabelText('Vorlage Umsetzung');
      fireEvent.change(area, { target: { value: 'halbfertig' } });
      expect(await screen.findByText(/Nicht automatisch gespeichert/, {}, { timeout: 3000 })).toBeTruthy();
      expect(await within(editor('Umsetzung')).findByRole('alert')).toBeTruthy();
    });
  });

  it('beschriftet promptExtra als „Zusatz für alle Prompts“ und speichert ihn weiter', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...VIEW, exists: true, raw }));
    open({ saveDispatchConfig: save });
    await userEvent.type(await screen.findByLabelText('Zusatz für alle Prompts'), 'kurz fassen');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ promptExtra: 'kurz fassen' }));
  });

  it('sperrt Übernehmen bei ungespeicherten Änderungen im Formular', async () => {
    open({ getDispatchSuggestions: async () => suggestionList({ suggestions: [suggestion()] }) });
    const wip = await screen.findByLabelText('Gleichzeitig (WIP)');
    await userEvent.type(wip, '3');
    const card = await screen.findByRole('article', { name: 'Schritte zu knapp · standard' });
    expect((within(card).getByRole('button', { name: 'Übernehmen' }) as HTMLButtonElement).disabled).toBe(true);
  });
});
