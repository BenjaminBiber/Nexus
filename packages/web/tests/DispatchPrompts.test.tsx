// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DispatchConfigViewDto, DispatchPromptKindDto, RawDispatchConfigDto } from '../src/api/dispatchTypes.js';
import { DispatchPanel } from '../src/components/DispatchPanel.js';
import { dbBoards, fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const RAW: RawDispatchConfigDto = {
  enabled: false, boards: null, wip: 1, window: null, optInLabel: 'auto', optOutLabel: 'manuell', maxAttempts: 2, pauseMinutesOnLimit: 60,
  profiles: { standard: { model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 90 } }, defaultProfile: 'standard', rules: [], escalate: true, allowFable: false,
  claude: { command: 'claude', permissionMode: 'acceptEdits', allowedTools: ['Read'] }, promptExtra: '',
};
const VIEW: DispatchConfigViewDto = { file: 'C:/h/dispatch.yaml', exists: false, raw: RAW, config: { enabled: false, staleMinutes: 120, ladder: ['standard'] }, error: null };

const PROMPT_INFO = [
  { kind: 'work' as const, label: 'Umsetzung', default: 'Standard\nzweite Zeile\nletzte', required: [], placeholders: [] },
  { kind: 'plan' as const, label: 'Planung', default: 'Standard-Planung', required: [], placeholders: [] },
  { kind: 'research' as const, label: 'Recherche', default: 'Standard-Recherche', required: [], placeholders: [] },
  { kind: 'merge' as const, label: 'Merge', default: 'Standard-Merge', required: [], placeholders: [] },
];

const view = (raw: RawDispatchConfigDto = RAW): DispatchConfigViewDto => ({ ...VIEW, raw, promptInfo: PROMPT_INFO });
const rawWith = (extra: Partial<RawDispatchConfigDto>): RawDispatchConfigDto => ({ ...RAW, ...extra });

// Antwort des Vorschau-Endpunkts; `template` entscheidet, ob eigene Vorlage oder Standard gerendert wurde
function preview(kind: DispatchPromptKindDto, template: string | null | undefined, id?: string) {
  return { kind, id: id ?? null, sample: !id, prompt: `${template ?? 'Standard\nzweite Zeile\nletzte'} [${id ?? 'Beispiel'}]` };
}

function open(overrides: Record<string, unknown> = {}, raw?: RawDispatchConfigDto) {
  const api = fakeApi({ getDispatchConfig: async () => view(raw), getDbBoards: async () => dbBoards, ...overrides });
  render(<ApiProvider api={api}><DispatchPanel sub="prompts" /></ApiProvider>);
  return api;
}

const editor = (label: string) => screen.getByRole('group', { name: `Prompt-Vorlage ${label}` });
const inEditor = (label: string) => within(editor(label));

describe('Vorschau der Prompt-Vorlagen', () => {
  it('rendert ohne ID mit Beispielwerten', async () => {
    const run = vi.fn(async (kind: DispatchPromptKindDto, template?: string | null, id?: string) => preview(kind, template, id));
    open({ dispatchPromptPreview: run }, rawWith({ prompts: { work: 'Eigene Vorlage' } }));
    await screen.findByLabelText('Vorlage Umsetzung');
    await userEvent.click(inEditor('Umsetzung').getByRole('button', { name: 'Vorschau' }));
    expect(run).toHaveBeenCalledWith('work', 'Eigene Vorlage', undefined);
    expect((await inEditor('Umsetzung').findByLabelText('Gerenderter Prompt')).textContent).toBe('Eigene Vorlage [Beispiel]');
    expect(inEditor('Umsetzung').getByText(/^Beispielwerte · /)).toBeTruthy();
  });

  it('schickt eine eingetippte Ticket-ID mit', async () => {
    const run = vi.fn(async (kind: DispatchPromptKindDto, template?: string | null, id?: string) => preview(kind, template, id));
    open({ dispatchPromptPreview: run }, rawWith({ prompts: { work: 'Eigene Vorlage' } }));
    await screen.findByLabelText('Vorlage Umsetzung');
    await userEvent.type(inEditor('Umsetzung').getByLabelText(/Ticket-\/Frage-ID/), 'ARC-0042');
    await userEvent.click(inEditor('Umsetzung').getByRole('button', { name: 'Vorschau' }));
    expect(run).toHaveBeenCalledWith('work', 'Eigene Vorlage', 'ARC-0042');
    expect((await inEditor('Umsetzung').findByLabelText('Gerenderter Prompt')).textContent).toContain('[ARC-0042]');
  });

  it('nimmt den ungespeicherten Stand des Editors', async () => {
    const run = vi.fn(async (kind: DispatchPromptKindDto, template?: string | null, id?: string) => preview(kind, template, id));
    open({ dispatchPromptPreview: run });
    const area = await screen.findByLabelText('Vorlage Umsetzung');
    fireEvent.change(area, { target: { value: 'gerade erst getippt' } });
    await userEvent.click(inEditor('Umsetzung').getByRole('button', { name: 'Vorschau' }));
    expect(run).toHaveBeenCalledWith('work', 'gerade erst getippt', undefined);
  });

  it('zeigt einen Fehler des Servers an der Vorschau', async () => {
    const run = vi.fn(async () => {
      throw new ApiError('not-found', 'Ticket ARC-0999 gibt es nicht.', 404);
    });
    open({ dispatchPromptPreview: run }, rawWith({ prompts: { work: 'Eigene Vorlage' } }));
    await screen.findByLabelText('Vorlage Umsetzung');
    await userEvent.type(inEditor('Umsetzung').getByLabelText(/Ticket-\/Frage-ID/), 'ARC-0999');
    await userEvent.click(inEditor('Umsetzung').getByRole('button', { name: 'Vorschau' }));
    const alert = await inEditor('Umsetzung').findByRole('alert');
    expect(alert.textContent).toContain('ARC-0999 gibt es nicht');
    expect(inEditor('Umsetzung').queryByLabelText('Gerenderter Prompt')).toBeNull();
    // Der Panel-Zustand bleibt unberührt: nichts gespeichert, keine weitere Meldung
    expect(screen.getAllByRole('alert')).toHaveLength(1);
  });

  it('stellt mit „Unterschiede zum Standard“ beide Fassungen gegenüber', async () => {
    const run = vi.fn(async (kind: DispatchPromptKindDto, template?: string | null, id?: string) => preview(kind, template, id));
    open({ dispatchPromptPreview: run }, rawWith({ prompts: { work: 'Standard\neigene Zeile\nletzte' } }));
    await screen.findByLabelText('Vorlage Umsetzung');
    await userEvent.click(inEditor('Umsetzung').getByLabelText('Unterschiede zum Standard'));
    await userEvent.click(inEditor('Umsetzung').getByRole('button', { name: 'Vorschau' }));
    expect(run).toHaveBeenCalledTimes(2);
    expect(run).toHaveBeenCalledWith('work', 'Standard\neigene Zeile\nletzte', undefined);
    expect(run).toHaveBeenCalledWith('work', null, undefined);
    const diff = await inEditor('Umsetzung').findByLabelText('Unterschiede zum Standard-Prompt');
    expect(within(diff).getByText('zweite Zeile').closest('.diff-line')!.className).toContain('diff-remove');
    expect(within(diff).getByText('eigene Zeile').closest('.diff-line')!.className).toContain('diff-add');
    expect(within(diff).getByText('Standard').closest('.diff-line')!.className).toContain('diff-same');
  });

  it('meldet bei unveränderter Vorlage „Entspricht dem Standard“ und ruft nur einmal', async () => {
    const run = vi.fn(async (kind: DispatchPromptKindDto, template?: string | null, id?: string) => preview(kind, template, id));
    open({ dispatchPromptPreview: run });
    await screen.findByLabelText('Vorlage Umsetzung');
    await userEvent.click(inEditor('Umsetzung').getByLabelText('Unterschiede zum Standard'));
    await userEvent.click(inEditor('Umsetzung').getByRole('button', { name: 'Vorschau' }));
    expect(await inEditor('Umsetzung').findByText('Entspricht dem Standard')).toBeTruthy();
    expect(run).toHaveBeenCalledTimes(1);
    expect(run).toHaveBeenCalledWith('work', null, undefined);
  });

  it('speichert durch die Vorschau nichts', async () => {
    const save = vi.fn(async (raw: RawDispatchConfigDto) => ({ ...view(raw), exists: true }));
    const run = vi.fn(async (kind: DispatchPromptKindDto, template?: string | null, id?: string) => preview(kind, template, id));
    open({ dispatchPromptPreview: run, saveDispatchConfig: save }, rawWith({ prompts: { work: 'Eigene Vorlage' } }));
    await screen.findByLabelText('Vorlage Umsetzung');
    await userEvent.type(inEditor('Umsetzung').getByLabelText(/Ticket-\/Frage-ID/), 'ARC-0042');
    await userEvent.click(inEditor('Umsetzung').getByRole('button', { name: 'Vorschau' }));
    await inEditor('Umsetzung').findByLabelText('Gerenderter Prompt');
    await new Promise((r) => setTimeout(r, 1500));
    expect(save).not.toHaveBeenCalled();
  });
});
