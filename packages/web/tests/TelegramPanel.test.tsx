// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { BotState, BotStatus, TelegramConfig, TelegramConfigInput, TelegramStatusDto } from '../src/api/telegramTypes.js';
import { TelegramPanel } from '../src/components/TelegramPanel.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

// Maske, wie der Server sie liefert: Bot-ID, Doppelpunkt, Auslassungszeichen, letzte vier Zeichen
const MASKED = '123456:…Dsaw';

const CONFIG: TelegramConfig = {
  enabled: true,
  botToken: MASKED,
  apiBase: 'https://api.telegram.org',
  chats: [4711, 815],
  notify: { review: true, blocked: true, question: true, merge: true, merged: false, paused: false },
  quietHours: null,
  profile: { brand: true, applied: { botId: 42, photo: 'p', texts: 't' } },
  ideas: { defaultTarget: 'note', maxRounds: 3 },
  speech: { backend: 'off', baseUrl: 'https://api.openai.com/v1', apiKey: '', model: 'whisper-1', language: null, whisperCli: { binary: 'whisper-cli', model: '', ffmpeg: 'ffmpeg' } },
};

const RUNNING: BotStatus = { state: 'running', botName: 'nexus_bot', botId: 42, lastUpdateAt: null, lastError: null, profileError: null };

function statusDto(bot: Partial<BotStatus> & { state: BotState }): TelegramStatusDto {
  return { bot: { ...RUNNING, ...bot }, pairedChats: CONFIG.chats.length, openCodes: 0 };
}

// Rendert das Panel mit austauschbaren Aufrufen
function open(overrides: Record<string, unknown> = {}, config: TelegramConfig = CONFIG, status: TelegramStatusDto = statusDto(RUNNING)) {
  render(
    <ApiProvider api={fakeApi({ getTelegramConfig: async () => config, getTelegramStatus: async () => status, ...overrides })}>
      <TelegramPanel />
    </ApiProvider>,
  );
}

describe('TelegramPanel', () => {
  it('lädt die Einstellung und zeigt das Token nur als Maske', async () => {
    open();
    expect((await screen.findByLabelText('Gespeichertes Bot-Token') as HTMLInputElement).value).toBe(MASKED);
    expect((screen.getByLabelText('Telegram-Bot') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText('Höchstens Runden') as HTMLInputElement).value).toBe('3');
    expect((screen.getByLabelText('Vorgabe') as HTMLSelectElement).value).toBe('note');
  });

  it('speichert Änderungen ohne das Token im Klartext', async () => {
    const save = vi.fn(async (input: TelegramConfigInput) => ({ ...CONFIG, ...input, chats: CONFIG.chats, profile: CONFIG.profile }) as TelegramConfig);
    open({ saveTelegramConfig: save });
    await userEvent.click(await screen.findByLabelText('Gemergt'));
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    const input = save.mock.calls[0]![0];
    expect(input).not.toHaveProperty('botToken');
    expect(input).not.toHaveProperty('chats');
    expect(input.notify).toEqual({ ...CONFIG.notify, merged: true });
    expect(input.profile).toEqual({ brand: true });
    expect((await screen.findByRole('status')).textContent).toContain('Gespeichert');
  });

  it('schickt ein neu eingetipptes Token mit', async () => {
    const save = vi.fn(async (input: TelegramConfigInput) => ({ ...CONFIG, ...input, chats: CONFIG.chats, profile: CONFIG.profile }) as TelegramConfig);
    open({ saveTelegramConfig: save });
    await userEvent.click(await screen.findByRole('button', { name: 'Ändern' }));
    await userEvent.type(screen.getByLabelText('Neues Bot-Token'), '999:neuesToken');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save.mock.calls[0]![0].botToken).toBe('999:neuesToken');
  });

  it('speichert die Nachtruhe mit Zeiten und schaltet sie wieder ab', async () => {
    const save = vi.fn(async (input: TelegramConfigInput) => ({ ...CONFIG, ...input, chats: CONFIG.chats, profile: CONFIG.profile }) as TelegramConfig);
    open({ saveTelegramConfig: save });
    await userEvent.click(await screen.findByLabelText('Nachtruhe'));
    expect((screen.getByLabelText('Von') as HTMLInputElement).value).toBe('22:00');
    await userEvent.clear(screen.getByLabelText('Bis'));
    await userEvent.type(screen.getByLabelText('Bis'), '06:30');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save.mock.calls[0]![0].quietHours).toEqual({ from: '22:00', to: '06:30' });
  });

  it('zeigt beim Koppeln Code, Ablauf und Link', async () => {
    const pair = vi.fn(async () => ({ code: 'ABC123', expiresAt: '2026-11-04T10:15:00.000Z', link: 'https://t.me/nexus_bot?start=ABC123' }));
    open({ pairTelegram: pair });
    await userEvent.click(await screen.findByRole('button', { name: 'Koppeln' }));
    const notice = await screen.findByRole('note', { name: 'Kopplungscode' });
    expect(notice.textContent).toContain('ABC123');
    expect(within(notice).getByRole('link', { name: 'Chat mit dem Bot öffnen' }).getAttribute('href')).toBe('https://t.me/nexus_bot?start=ABC123');
  });

  it('entkoppelt einen Chat und lädt die Liste neu', async () => {
    const unpairTelegramChat = vi.fn(async () => ({ ok: true }));
    let config = CONFIG;
    open({ unpairTelegramChat, getTelegramConfig: async () => config });
    await screen.findByText('4711');
    config = { ...CONFIG, chats: [815] };
    await userEvent.click(screen.getAllByRole('button', { name: 'Entkoppeln' })[0]!);
    expect(unpairTelegramChat).toHaveBeenCalledWith(4711);
    expect(await screen.findByText('815')).toBeTruthy();
    expect(screen.queryByText('4711')).toBeNull();
  });

  it('zeigt, dass der Bot läuft', async () => {
    open();
    expect((await screen.findByRole('note', { name: 'Stand des Bots' })).textContent).toContain('Läuft als @nexus_bot');
  });

  it('zeigt einen Bot auf einem anderen Rechner', async () => {
    open({}, CONFIG, statusDto({ state: 'conflict' }));
    expect((await screen.findByRole('note', { name: 'Stand des Bots' })).textContent).toContain('anderen Rechner');
  });

  it('zeigt ein ungültiges Token', async () => {
    open({}, CONFIG, statusDto({ state: 'tokenInvalid', botName: null, botId: null }));
    expect((await screen.findByRole('note', { name: 'Stand des Bots' })).textContent).toContain('Token ungültig');
  });

  it('zeigt einen Profilfehler als eigene Meldung', async () => {
    open({}, CONFIG, statusDto({ state: 'running', profileError: 'Das Profilbild wurde abgelehnt' }));
    const notes = await screen.findAllByRole('note');
    expect(notes.map((n) => n.textContent).join(' ')).toContain('Das Profilbild wurde abgelehnt');
  });

  it('setzt das Profil des Bots neu', async () => {
    const applyTelegramProfile = vi.fn(async () => ({ applied: true, error: null }));
    open({ applyTelegramProfile });
    await userEvent.click(await screen.findByRole('button', { name: 'Profil neu setzen' }));
    expect(applyTelegramProfile).toHaveBeenCalled();
    expect((await screen.findByRole('status')).textContent).toContain('Profil des Bots neu gesetzt');
  });

  it('meldet, wenn das Profil nicht gesetzt werden konnte', async () => {
    const applyTelegramProfile = vi.fn(async () => ({ applied: false, error: 'Der Name wurde abgelehnt' }));
    open({ applyTelegramProfile });
    await userEvent.click(await screen.findByRole('button', { name: 'Profil neu setzen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Der Name wurde abgelehnt');
  });

  it('zeigt die Vorschau des Bot-Profils', async () => {
    open();
    const image = await screen.findByAltText('Profilbild des Bots: Nexus-Logo');
    expect(image.getAttribute('src')).toBe('/favicon.svg');
  });

  it('schickt die Testnachricht und zeigt Erfolge und Fehler', async () => {
    const testTelegram = vi.fn(async () => ({ sent: 1, errors: [{ chatId: 815, error: 'chat not found' }] }));
    open({ testTelegram });
    await userEvent.click(await screen.findByRole('button', { name: 'Testnachricht' }));
    const notice = await screen.findByRole('note', { name: 'Ergebnis der Testnachricht' });
    expect(notice.textContent).toContain('An 1 Chat verschickt');
    expect(notice.textContent).toContain('815: chat not found');
  });
});
