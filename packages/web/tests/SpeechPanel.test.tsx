// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { SpeechConfig, SpeechStatusDto, TelegramConfig, TelegramConfigInput } from '../src/api/telegramTypes.js';
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
  profile: { applied: null },
  ideas: { defaultTarget: 'note', maxRounds: 3 },
  speech: SPEECH,
};

const NO_STATUS: SpeechStatusDto = { lastSuccessAt: null, lastSuccessVia: null, lastMillis: null, lastErrorAt: null, lastError: null };

// Zeitpunkt vor n Minuten im Format des Servers
const minutesAgo = (n: number) => new Date(Date.now() - n * 60_000).toISOString();

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
    expect((await screen.findByRole('radio', { name: 'Speaches' }) as HTMLInputElement).checked).toBe(true);
    await userEvent.click(screen.getByRole('radio', { name: 'LocalAI' }));
    expect((screen.getByLabelText('Adresse') as HTMLInputElement).value).toBe('http://localhost:8080');
    expect((screen.getByLabelText('Modell') as HTMLInputElement).value).toBe('whisper-1');
    expect((screen.getByRole('radio', { name: 'LocalAI' }) as HTMLInputElement).checked).toBe(true);
  });

  it('zeigt bei OpenAI den Datenschutz-Hinweis, bei einem lokalen Dienst nicht', async () => {
    open();
    await screen.findByRole('radio', { name: 'Speaches' });
    expect(screen.queryByLabelText('Datenschutz')).toBeNull();
    await userEvent.click(screen.getByRole('radio', { name: 'OpenAI' }));
    expect(screen.getByLabelText('Datenschutz').textContent).toContain('verlassen den Rechner');
  });

  it('zeigt den Schlüssel nur als Maske und schickt ihn nicht mit', async () => {
    const save = saveSpy();
    open({ saveTelegramConfig: save });
    expect((await screen.findByLabelText('Gespeicherter Schlüssel')).textContent).toBe(MASKED);
    await userEvent.click(screen.getByRole('radio', { name: 'OpenAI' }));
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

  it('zeigt im Kopf Dienst, Modell und die letzte Erkennung', async () => {
    const status: SpeechStatusDto = { ...NO_STATUS, lastSuccessAt: minutesAgo(20), lastSuccessVia: 'voice', lastMillis: 1240 };
    open({ getSpeechStatus: async () => status });
    const hero = await screen.findByRole('note', { name: 'Stand der Spracherkennung' });
    expect(hero.textContent).toContain('Speaches');
    expect(hero.textContent).toContain('Systran/faster-whisper-small');
    expect(await screen.findByText(/Zuletzt erkannt vor 20 min \(Sprachnachricht, 1,2 s\)/)).toBeTruthy();
    expect(hero.textContent).toContain('Funktioniert');
  });

  it('meldet einen Fehler, der neuer ist als die letzte Erkennung', async () => {
    const status: SpeechStatusDto = {
      ...NO_STATUS,
      lastSuccessAt: minutesAgo(60),
      lastSuccessVia: 'test',
      lastMillis: 500,
      lastErrorAt: minutesAgo(5),
      lastError: 'Die Spracherkennung antwortete mit 401.',
    };
    open({ getSpeechStatus: async () => status });
    expect((await screen.findByRole('note', { name: 'Letzter Fehler' })).textContent).toContain('401');
    expect(screen.getByRole('note', { name: 'Stand der Spracherkennung' }).textContent).toContain('Fehler');
  });

  it('zeigt „Noch nicht genutzt“, solange nichts erkannt wurde, und „Aus“ bei ausgeschalteter Erkennung', async () => {
    open();
    expect((await screen.findByRole('note', { name: 'Stand der Spracherkennung' })).textContent).toContain('Noch nicht genutzt');
    cleanup();
    open({}, { ...SPEECH, backend: 'off' });
    const hero = await screen.findByRole('note', { name: 'Stand der Spracherkennung' });
    expect(hero.textContent).toContain('Aus');
    expect(hero.textContent).toContain('nicht verarbeitet');
  });

  it('zeigt im Kopf bis zum Speichern den laufenden Dienst', async () => {
    open();
    await userEvent.click(await screen.findByRole('radio', { name: 'OpenAI' }));
    const hero = screen.getByRole('note', { name: 'Stand der Spracherkennung' });
    expect(hero.textContent).toContain('Speaches');
    expect(hero.textContent).not.toContain('OpenAI');
  });

  it('schaltet die Erkennung mit dem Ein/Aus-Knopf', async () => {
    const save = saveSpy();
    open({ saveTelegramConfig: save });
    const power = await screen.findByRole('button', { name: 'Erkennung aktiv' });
    expect(power.getAttribute('aria-pressed')).toBe('true');
    await userEvent.click(power);
    expect(power.getAttribute('aria-pressed')).toBe('false');
    expect((screen.getByRole('group', { name: 'Vorlage' }) as HTMLFieldSetElement).disabled).toBe(true);
    await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
    expect(save.mock.calls[0]![0].speech?.backend).toBe('off');
  });

  it('zählt ungespeicherte Änderungen und verwirft sie', async () => {
    open();
    await userEvent.click(await screen.findByRole('radio', { name: 'OpenAI' }));
    expect(screen.getByText('2 ungespeicherte Änderungen')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(screen.getByText('Alles gespeichert.')).toBeTruthy();
    expect((screen.getByLabelText('Adresse') as HTMLInputElement).value).toBe('http://localhost:8000');
  });

  it('lädt den Stand nach einem Test neu', async () => {
    const getSpeechStatus = vi.fn(async () => NO_STATUS);
    open({ getSpeechStatus, testSpeech: async () => ({ text: 'Hallo', language: 'de', backend: 'openai-compatible', millis: 10 }) });
    await userEvent.click(await screen.findByRole('button', { name: 'Testen' }));
    await screen.findByLabelText('Ergebnis des Tests');
    expect(getSpeechStatus).toHaveBeenCalledTimes(2);
  });

  it('zeigt statt eines fehlenden Logos eine Kachel mit Kürzel', async () => {
    open();
    const hero = await screen.findByRole('note', { name: 'Stand der Spracherkennung' });
    const img = hero.querySelector('img.speech-logo') as HTMLImageElement;
    expect(img.getAttribute('src')).toBe('/speech-logos/speaches.svg');
    fireEvent.error(img);
    expect(hero.querySelector('img.speech-logo')).toBeNull();
    expect(hero.querySelector('.speech-logo')?.textContent).toBe('Sp');
  });
});
