import type { NotifiableBoardQuestion, ProjectState } from '@harness/core';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { QuestionChange } from './NotificationService.js';
import { QuestionChangeDetector } from './QuestionChangeDetector.js';

// Empfänger der Meldungen (die NotificationService)
export interface DbQuestionSink {
  onQuestionChange(change: QuestionChange): Promise<void>;
}

// Fragt regelmäßig die offenen eigenen Fragen ab (Reiter „Eigene“: selbst gestellt oder zu Tickets auf eigenen Boards) und meldet
// neue Fragen und Recherchen von Agenten.
// Ohne eingerichtete oder erreichbare Board-Datenbank bleibt sie still.
export class DbQuestionWatch {
  private readonly detector = new QuestionChangeDetector<NotifiableBoardQuestion>();
  private timer: ReturnType<typeof setInterval> | null = null;
  private running = false;

  constructor(
    private readonly holder: Pick<BoardRuntimeHolder, 'run'>,
    private readonly projects: { list(): Promise<ProjectState[]> },
    private readonly sink: DbQuestionSink,
    private readonly intervalMs = 10000,
  ) {}

  // Liest sofort den Bestand ein (ohne Meldung) und vergleicht danach im Takt; Abstand 0 schaltet ab
  start(): void {
    if (this.timer || this.intervalMs <= 0) return;
    void this.tick();
    this.timer = setInterval(() => void this.tick(), this.intervalMs);
    this.timer.unref?.();
  }

  // Hält den Takt an
  stop(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }

  // Ein Durchlauf (nie zwei gleichzeitig); Fehler wie fehlende Konfig oder verlorene Verbindung werden übergangen
  async tick(): Promise<void> {
    if (this.running) return;
    this.running = true;
    try {
      const questions = await this.holder.run((b) => b.services.questions.notifiable(b.actor().userId));
      // Erst einlesen (ohne Meldung); danach das Ziel auflösen, bevor der gemerkte Stand fortgeschrieben wird:
      // schlägt die Projektliste fehl, bleiben die Änderungen für den nächsten Durchlauf erhalten
      const projects = this.detector.primed ? await this.projects.list() : [];
      const changes = this.detector.compare(questions);
      for (const { kind, question } of changes) {
        try {
          await this.sink.onQuestionChange({
            projectId: this.pidFor(question.repoPaths, projects),
            projectName: `${question.projectKey} · ${question.projectName}`,
            kind, question, route: 'questions',
          });
        } catch {
          // Eine fehlgeschlagene Meldung hält die übrigen nicht auf
        }
      }
    } catch {
      // Board-Datenbank nicht eingerichtet oder nicht erreichbar: Meldungen sind optional
    } finally {
      this.running = false;
    }
  }

  // Harness-Projekt für die Adresse: eines, dessen Pfad einem Repo-Pfad der Boards entspricht, sonst das erste erreichbare
  private pidFor(paths: string[], projects: ProjectState[]): string {
    const norm = (p: string) => p.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase();
    const wanted = new Set(paths.map(norm));
    const reachable = projects.filter((p) => p.reachable);
    return (reachable.find((p) => wanted.has(norm(p.path))) ?? reachable[0])?.id ?? '';
  }
}
