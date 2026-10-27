import type { Command } from 'commander';
import { DispatchConfigStore, GitRunner, HarnessError, HarnessHome, HarnessProject, type Actor, type AuthorKind, type JoinWorktrees, type QuestionActor } from '@harness/core';
import { BoardRuntime } from '@harness/db';
import { Printer } from './output/Printer.js';

const KINDS: AuthorKind[] = ['human', 'session', 'subagent'];

// Gemeinsamer Zugriff der Befehle auf globale Optionen, Ausgabe und das geöffnete Projekt.
export class CliContext {
  constructor(private readonly program: Command) {}

  // Ausgabe passend zu --json
  printer(): Printer {
    return new Printer(Boolean(this.program.opts().json));
  }

  // Projektordner aus --project oder dem aktuellen Verzeichnis
  projectDir(): string {
    return (this.program.opts().dir as string | undefined) ?? process.cwd();
  }

  // Öffnet das eingerichtete Projekt (Fehler mit Hinweis auf `harness init`, wenn nicht eingerichtet)
  openProject(): Promise<HarnessProject> {
    return HarnessProject.open(this.projectDir());
  }

  // Wer handelt: Art (Standard: „session“ innerhalb einer Session, sonst Mensch) und Name (Standard: Git-Benutzername)
  actor(kind?: string, author?: string): Actor {
    const resolvedKind = (kind ?? (process.env.HARNESS_SESSION ? 'session' : 'human')) as AuthorKind;
    if (!KINDS.includes(resolvedKind)) {
      throw new HarnessError('kind-invalid', `Art „${kind}“ ist ungültig. Erlaubt: ${KINDS.join(', ')}.`);
    }
    return { kind: resolvedKind, author: author ?? this.gitUserName() };
  }

  // Wer handelt bei Fragen: wie `actor`, bei Sessions und Subagenten zusätzlich mit der Kennung der Session.
  // Ohne --kind gilt „session“ innerhalb einer Session (HARNESS_SESSION oder --session), sonst „human“.
  questionActor(kind?: string, author?: string, session?: string): QuestionActor {
    const id = session ?? process.env.HARNESS_SESSION;
    const actor = this.actor(kind ?? (id ? 'session' : 'human'), author);
    return actor.kind !== 'human' && id ? { ...actor, session: id } : actor;
  }

  // Wer entscheidet: nur ein Mensch, also weder --kind session/subagent noch ein Aufruf aus einer Session
  humanActor(kind?: string, author?: string): Actor {
    if (process.env.HARNESS_SESSION) {
      throw new HarnessError(
        'human-only',
        'Diese Entscheidung trifft ein Mensch. Bitte in der Oberfläche oder in einem eigenen Terminal ausführen.',
      );
    }
    return this.actor(kind, author);
  }

  // Benutzerweiter Harness-Ordner (HARNESS_HOME oder ~/.harness)
  home(): HarnessHome {
    return new HarnessHome();
  }

  // Worktrees des Dispatchers (joinWorktrees aus ~/.harness/dispatch.yaml); ohne oder mit kaputter Datei: ein Worktree je Ticket
  async joinWorktrees(): Promise<JoinWorktrees> {
    try {
      return (await new DispatchConfigStore(this.home()).read()).config.joinWorktrees;
    } catch {
      return 'none';
    }
  }

  // Öffnet das Board aus ~/.harness/board.yaml, führt die Arbeit aus und schließt die Verbindung
  async withBoard<T>(work: (rt: BoardRuntime) => Promise<T>): Promise<T> {
    const runtime = await BoardRuntime.open(this.home());
    try {
      return await work(runtime);
    } finally {
      await runtime.close();
    }
  }

  // Git-Benutzername oder „unbekannt“
  private gitUserName(): string {
    try {
      return new GitRunner().run(this.projectDir(), ['config', 'user.name']) || 'unbekannt';
    } catch {
      return 'unbekannt';
    }
  }
}
