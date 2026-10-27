import { join } from 'node:path';
import type { DisplayState, SessionView, UserSettings, UserSettingsStore } from '@harness/core';
import type { DispatchNotice } from '../dispatch/DispatchContext.js';
import type { NotifiableQuestion } from './QuestionChangeDetector.js';
import { AttentionTracker } from './AttentionTracker.js';
import type { BrowserNotice, BrowserNoticeHub } from './BrowserNoticeHub.js';
import { NtfySender } from './NtfySender.js';
import type { ToastModel } from './ToastModel.js';
import { ToastTexts } from './ToastTexts.js';
import { ToastXmlBuilder } from './ToastXmlBuilder.js';
import type { WindowsToastSender } from './WindowsToastSender.js';

export interface SessionChange {
  projectId: string;
  projectName: string;
  before: DisplayState | undefined;
  view: SessionView;
  ticket?: { id: string; title: string };
}

export interface QuestionChange {
  // Harness-Projekt der Adresse; leer: Startseite der Oberfläche
  projectId: string;
  projectName: string;
  /** neue Frage oder neue Recherche (die letzte der Frage wird gezeigt) */
  kind: 'frage-neu' | 'recherche';
  question: NotifiableQuestion;
  // Reiter der Fragen der Board-Datenbank
  route?: 'questions';
}

// Entscheidet bei Zustandswechseln von Sessions, ob eine Windows-Meldung erscheint, und zeigt sie;
// ist ntfy eingeschaltet, geht dieselbe Meldung zusätzlich als Push raus (unabhängig von der Art der Meldung).
export class NotificationService {
  private readonly tracker = new AttentionTracker();
  private readonly texts = new ToastTexts();
  private readonly xml = new ToastXmlBuilder();

  constructor(
    private readonly settings: UserSettingsStore,
    private readonly sender: WindowsToastSender,
    private readonly assetsDir: string,
    private readonly baseUrl: string,
    private readonly log: (line: string) => void = () => undefined,
    private readonly tabs: BrowserNoticeHub | null = null,
    private readonly ntfy: NtfySender = new NtfySender(),
  ) {}

  // Verarbeitet eine Zustandsänderung; gezeigt wird nur im Modus „windows“ oder mit ntfy
  async onSessionChange(change: SessionChange): Promise<void> {
    const label = change.ticket?.id ?? change.view.title ?? change.projectName;
    const kind = this.tracker.observe(change.before, change.view, label);
    if (!kind) return;
    const settings = await this.settings.read();
    if (!this.wanted(settings)) return;
    const pending = this.tracker.list();
    const model =
      pending.length >= 2
        ? this.texts.collected(pending, `${this.baseUrl}/p/${change.projectId}/sessions`)
        : this.texts.single({
            kind,
            projectName: change.projectName,
            ticket: change.ticket,
            session: change.view,
            openUrl: this.ticketUrl(change.projectId, change.ticket?.id),
          });
    await this.deliver(model, settings);
  }

  // Meldet eine neue Frage oder Recherche eines Agenten; gezeigt wird nur im Modus „windows“ oder mit ntfy
  async onQuestionChange(change: QuestionChange): Promise<void> {
    const settings = await this.settings.read();
    if (!this.wanted(settings)) return;
    const { question } = change;
    const input = {
      projectName: change.projectName,
      question,
      openUrl: change.projectId ? `${this.baseUrl}/p/${change.projectId}/${change.route ?? 'questions'}/${question.id}` : this.baseUrl,
    };
    const latest = question.research[question.research.length - 1];
    const model =
      change.kind === 'recherche' && latest
        ? this.texts.research({ ...input, research: latest })
        : this.texts.questionNew(input);
    await this.deliver(model, settings, `question-${change.projectId || 'db'}-${question.id}`);
  }

  // Meldet ein Ereignis des Dispatchers; gezeigt wird nur im Modus „windows“ oder mit ntfy
  async onDispatch(change: { projectId: string; notice: DispatchNotice }): Promise<void> {
    const settings = await this.settings.read();
    if (!this.wanted(settings)) return;
    const { projectId, notice } = change;
    const openUrl = projectId ? this.ticketUrl(projectId, notice.ticketId ?? undefined) : this.baseUrl;
    await this.deliver(this.texts.dispatch({ notice, openUrl }), settings, `dispatch-${notice.ticketId ?? notice.kind}`);
  }

  // Zeigt eine Beispielmeldung (Schaltfläche „Testmeldung“ in den Einstellungen); liefert den Fehlertext oder null
  async sendTest(): Promise<string | null> {
    const settings = await this.settings.read();
    return this.show(this.example(), settings.heroImages);
  }

  // Schickt die Beispielmeldung an ntfy (Testschaltfläche im Abschnitt ntfy); liefert den Fehlertext oder null
  async sendNtfyTest(): Promise<string | null> {
    const settings = await this.settings.read();
    if (!settings.ntfyTopic) return 'Für ntfy ist kein Thema eingetragen.';
    return this.push(this.example(), settings);
  }

  // Meldung der Testschaltflächen
  private example(): ToastModel {
    return this.texts.single({
      kind: 'wartet',
      projectName: 'Beispielprojekt',
      ticket: { id: 'ARC-0042', title: 'Favoriten' },
      session: { title: 'Testmeldung des Harness', model: undefined },
      openUrl: this.baseUrl,
    });
  }

  // Ob überhaupt gemeldet wird: Modus „windows“ oder eingeschaltetes ntfy mit Thema
  private wanted(settings: UserSettings): boolean {
    return settings.notifications === 'windows' || this.ntfyOn(settings);
  }

  private ntfyOn(settings: UserSettings): boolean {
    return settings.ntfy === true && !!settings.ntfyTopic;
  }

  // Verteilt die Meldung auf die eingeschalteten Wege (Windows bzw. Tab, ntfy)
  private async deliver(model: ToastModel, settings: UserSettings, tag?: string): Promise<void> {
    await Promise.all([
      settings.notifications === 'windows' ? this.show(model, settings.heroImages, tag) : null,
      this.ntfyOn(settings) ? this.push(model, settings) : null,
    ]);
  }

  // Zeigt die Meldung in einem offenen Tab, sonst baut es das XML und übergibt es an Windows
  private async show(model: ToastModel, heroImages: boolean, tag = 'attention'): Promise<string | null> {
    if (this.tabs?.deliver(this.notice(model, tag))) return null;
    const xml = this.xml.build(model, { assetsDir: this.assetsDir, heroImages });
    const error = await this.sender.show(xml, join(this.assetsDir, 'logo-app.png'), tag);
    if (error) this.log(`Windows-Benachrichtigung fehlgeschlagen: ${error}`);
    return error;
  }

  // Schickt die Meldung an ntfy; Fehler landen im Log
  private async push(model: ToastModel, settings: UserSettings): Promise<string | null> {
    const error = await this.ntfy.send({ server: settings.ntfyServer, topic: settings.ntfyTopic, token: settings.ntfyToken }, model);
    if (error) this.log(`ntfy-Benachrichtigung fehlgeschlagen: ${error}`);
    return error;
  }

  // Meldung für einen Tab: Text in zwei Zeilen, Adresse als Pfad der Oberfläche
  private notice(model: ToastModel, tag: string): BrowserNotice {
    const url = model.openUrl.startsWith(this.baseUrl) ? model.openUrl.slice(this.baseUrl.length) || '/' : '/';
    const body = [model.line1, model.line2].filter(Boolean).join('\n');
    return { type: 'notice', kind: model.kind, tag, title: model.title, body, url };
  }

  // Adresse des Tickets im Panel des DB-Boards, ohne Ticket die Sessions des Projekts
  private ticketUrl(projectId: string, ticketId?: string): string {
    const base = `${this.baseUrl}/p/${projectId}`;
    return ticketId ? `${base}/board/t/${encodeURIComponent(ticketId)}` : `${base}/sessions`;
  }
}
