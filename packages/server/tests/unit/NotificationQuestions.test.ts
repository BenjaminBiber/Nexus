import { describe, expect, it } from 'vitest';
import type { Question, UserSettings, UserSettingsStore } from '@harness/core';
import { NotificationService } from '../../src/notifications/NotificationService.js';
import { ToastTexts } from '../../src/notifications/ToastTexts.js';
import { ToastXmlBuilder } from '../../src/notifications/ToastXmlBuilder.js';
import { WindowsToastSender } from '../../src/notifications/WindowsToastSender.js';

type Kind = 'human' | 'session' | 'subagent';
const option = (id: string, recommended = false) => ({ id, title: id, text: 't', pros: [], cons: [], recommended });
const research = (kind: Kind, options: ReturnType<typeof option>[] = []) => ({ at: 'x', author: 'a', kind, context: 'k', options });
const question = (id: string, extra: Partial<Question> = {}, kind: Kind = 'session') =>
  ({ id, title: 'Welcher Speicher?', origin: { kind, author: 'a' }, occasion: 'T-0042', research: [], ...extra }) as unknown as Question;

describe('ToastTexts für Fragen', () => {
  const texts = new ToastTexts();

  it('formuliert die Meldung einer neuen Frage', () => {
    const model = texts.questionNew({ projectName: 'webshop', question: question('F-0007'), openUrl: 'u' });
    expect(model).toEqual({
      kind: 'frage-neu',
      title: 'Neue Frage von einer Session',
      line1: 'webshop · F-0007 Welcher Speicher?',
      line2: 'Anlass: T-0042 · noch keine Recherche',
      openUrl: 'u',
    });
  });

  it('nennt bei einer neuen Frage mit vorhandener Recherche, dass sie vorliegt', () => {
    const q = question('F-0007', { research: [research('session', [option('O1')])] });
    const model = texts.questionNew({ projectName: 'webshop', question: q, openUrl: 'u' });
    expect(model.line2).toBe('Anlass: T-0042 · Recherche liegt vor');
  });

  it('formuliert die Recherche mit Bereich und Empfehlung', () => {
    const r = research('session', [option('O3'), option('O4', true), option('O5')]);
    const model = texts.research({ projectName: 'webshop', question: question('F-0007'), research: r, openUrl: 'u' });
    expect(model).toMatchObject({ kind: 'recherche', title: 'Recherche ist da', line2: '3 Optionen (O3 bis O5), O4 empfohlen' });
  });

  it('nennt ohne Optionen nur den ergänzten Kontext', () => {
    const model = texts.research({ projectName: 'w', question: question('F-1'), research: research('session'), openUrl: 'u' });
    expect(model.line2).toBe('Kontext ergänzt');
  });
});

describe('ToastXmlBuilder für Fragen', () => {
  const xml = new ToastXmlBuilder();
  const model = { kind: 'frage-neu' as const, title: 't', line1: 'a', line2: 'b', openUrl: 'http://x/p/p1/questions/F-0007' };

  it('nutzt das Frage-Logo, Frage öffnen und kein Hero', () => {
    const out = xml.build(model, { assetsDir: 'C:\\a', heroImages: true });
    expect(out).toContain('logo-frage.png');
    expect(out).toContain('content="Frage öffnen"');
    expect(out).not.toContain('gnorieren');
    expect(out).not.toContain('placement="hero"');
  });
});

describe('NotificationService.onQuestionChange', () => {
  const store = (settings: Partial<UserSettings>) =>
    ({ read: async () => ({ notifications: 'windows', heroImages: false, ...settings }) }) as unknown as UserSettingsStore;
  const setup = (settings: Partial<UserSettings> = {}) => {
    const shown: string[] = [];
    const sender = new WindowsToastSender(async (_s, env) => (shown.push(env.HARNESS_TOAST_XML), 0), 'win32');
    return { shown, service: new NotificationService(store(settings), sender, 'C:\\a', 'http://127.0.0.1:7777') };
  };

  it('zeigt die Meldung mit der Adresse der Frage', async () => {
    const { shown, service } = setup();
    await service.onQuestionChange({ projectId: 'p1', projectName: 'webshop', kind: 'frage-neu', question: question('F-0007') });
    expect(shown).toHaveLength(1);
    expect(shown[0]).toContain('http://127.0.0.1:7777/p/p1/questions/F-0007');
    expect(shown[0]).toContain('Neue Frage von einer Session');
  });

  it('zeigt im Modus off nichts', async () => {
    const { shown, service } = setup({ notifications: 'off' } as Partial<UserSettings>);
    await service.onQuestionChange({ projectId: 'p1', projectName: 'webshop', kind: 'frage-neu', question: question('F-0007') });
    expect(shown).toHaveLength(0);
  });

  it('öffnet Fragen der Board-Datenbank unter /questions und ohne Projekt die Startseite', async () => {
    const { shown, service } = setup();
    await service.onQuestionChange({ projectId: 'p1', projectName: 'ARC · Arches', kind: 'frage-neu', question: question('ARC-F0001'), route: 'questions' });
    await service.onQuestionChange({ projectId: '', projectName: 'ARC · Arches', kind: 'frage-neu', question: question('ARC-F0002'), route: 'questions' });
    expect(shown[0]).toContain('http://127.0.0.1:7777/p/p1/questions/ARC-F0001');
    expect(shown[0]).toContain('ARC · Arches · ARC-F0001 Welcher Speicher?');
    expect(shown[1]).toContain('http://127.0.0.1:7777');
    expect(shown[1]).not.toContain('/p/');
  });
});
