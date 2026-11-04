// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { SpeechConfig, TelegramConfig, TelegramConfigInput } from '../src/api/telegramTypes.js';
import { SpeechPanel } from '../src/components/settings/SpeechPanel.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

// Maske, wie der Server sie liefert: drei Zeichen, Auslassungszeichen, letzte vier
const MASKED = 'sk-…9xZq';

const SPEECH: SpeechConfig = {
  backend: 'openai-compatible',
  baseUrl: 'http://localhost:8000',
  apiKey: MASKED,
  model: 'Systran/faster-whisper-small',
  language: 'de',
  whisperCli: { binary: 'whisper-cli', model: '', ffmpeg: 'ffmpeg' },
};

const CONFIG: TelegramConfig = {
  enabled: true,
  botToken: '123456:…Dsaw',
  apiBase: 'https://api.telegram.org',
  chats: [4711],
  notify: { review: true, blocked: true, question: true, merge: true, merged: false, paused: false },
  quietHours: null,
  profile: { brand: true, applied: null },
  ideas: { defaultTarget: 'note', maxRounds: 3 },
  speech: SPEECH,
};

// Rendert das Panel mit austauschbaren Aufrufen
function open(overrides: Record<string, unknown> = {}, speech: SpeechConfig = SPEECH) {
  render(
    <ApiProvider api={fakeApi({ getTelegramConfig: async () => ({ ...CONFIG, speech }), ...overrides })}>
      <SpeechPanel />
    </ApiProvider>,
  );
}

// Nimmt die Eingabe entgegen und antwortet, als hätte der Server sie übernommen
function saveSpy() {
  return vi.fn(async (input: TelegramConfigInput) => ({ ...CONFIG, ...input, speech: { ...SPEECH, ...input.speech } }) as TelegramConfig);
}

describe('SpeechPanel', () => {
  it('erkennt die Vorlage und füllt mit ihr Adresse und Modell', async () => {
    open();
    expect((await screen.findByLabelText('Vorlage') as HTMLSelectElement).value).toBe('speaches');
    await userEvent.selectOptions(screen.getByLabelText('Vorlage'), 'localai');
    expect((screen.getByLabelText('Adresse') as HTMLInputElement).value).toBe('http://localhost:8080');
    expect((screen.getByLabelText('Modell') as HTMLInputElement).value).toBe('whisper-1');
    expect((screen.getByLabelText('Vorlage') as HTMLSelectElement).value).toBe('localai');
  });

  it('zeigt bei OpenAI den Datenschutz-Hinweis, bei einem lokalen Dienst nicht', async () => {
    open();
    expect((await screen.findByLabelText('Datenschutz')).textContent).toContain('bleiben im eigenen Netz');
    await userEvent.selectOptions(screen.getByLabelText('Vorlage'), 'openai');
    expect(screen.getByLabelText('Datenschutz').textContent).toContain('verlassen den Rechner');
  });

  it('zeigt den Schlüssel nur als Maske und schickt ihn nicht mit', async () => {
    const save = saveSpy();
    open({ saveTelegramConfig: save });
    expect((await screen.findByLabelText('Gespeicherter Schlüssel') as HTMLInputElement).value).toBe(MASKED);
    await userEvent.selectOptions(screen.getByLabelText('Vorlage'), 'openai');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    const input = save.mock.calls[0]![0];
    expect(input.speech).not.toHaveProperty('apiKey');
    expect(input.speech?.baseUrl).toBe('https://api.openai.com');
    expect(input).not.toHaveProperty('botToken');
    expect((await screen.findByRole('status')).textContent).toContain('Gespeichert');
  });

  it('schickt einen neu eingetippten Schlüssel mit', async () => {
    const save = saveSpy();
    open({ saveTelegramConfig: save });
    await userEvent.click(await screen.findByRole('button', { name: 'Ändern' }));
    await userEvent.type(screen.getByLabelText('Neuer Schlüssel'), 'sk-neu');
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save.mock.calls[0]![0].speech?.apiKey).toBe('sk-neu');
  });

  it('zeigt nach dem Testen den erkannten Text und die Dauer', async () => {
    open({ testSpeech: async () => ({ text: 'Hallo Nexus', language: 'de', backend: 'openai-compatible', millis: 742 }) });
    await userEvent.click(await screen.findByRole('button', { name: 'Testen' }));
    const result = await screen.findByLabelText('Ergebnis des Tests');
    expect(result.textContent).toContain('Hallo Nexus');
    expect(result.textContent).toContain('742 ms');
  });

  it('zeigt den Fehler, wenn der Dienst nicht antwortet', async () => {
    open({ testSpeech: async () => { throw new Error('Der Dienst antwortet nicht.'); } });
    await userEvent.click(await screen.findByRole('button', { name: 'Testen' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Der Dienst antwortet nicht.');
  });

  it('lässt den Test aus, solange die Erkennung aus ist', async () => {
    open({}, { ...SPEECH, backend: 'off' });
    expect((await screen.findByRole('button', { name: 'Testen' }) as HTMLButtonElement).disabled).toBe(true);
  });
});
