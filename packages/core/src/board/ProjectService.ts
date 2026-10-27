import { HarnessError } from '../errors/HarnessError.js';
import type { BoardDatabase, BoardUnitOfWork } from './BoardDatabase.js';
import { MAX_PROJECT_NAME_LENGTH, type BoardActor, type Project } from './BoardTypes.js';
import { BoardLimits } from './BoardLimits.js';
import { TicketIds } from './TicketIds.js';

// Projekte anlegen, auflisten und umbenennen. Ein Projekt hat einen Key (Ticket-Präfix) und einen Namen.
export class ProjectService {
  private readonly ids = new TicketIds();

  constructor(private readonly db: BoardDatabase, private readonly clock: () => Date) {}

  // Alle Projekte nach Key
  list(): Promise<Project[]> {
    return this.db.transaction((u) => u.projects.list());
  }

  // Legt ein Projekt an; der Key wird normalisiert und muss frei sein
  async create(key: string, name: string, actor: BoardActor): Promise<Project> {
    const normalized = this.ids.normalizeKey(key);
    const title = this.requireName(name);
    return this.db.transaction(async (u) => {
      if (await u.projects.byKey(normalized)) throw new HarnessError('project-exists', `Projekt ${normalized} gibt es schon.`);
      return u.projects.insert({ key: normalized, name: title, createdAt: this.clock().toISOString(), createdBy: actor.userId });
    });
  }

  // Benennt ein Projekt um
  async rename(key: string, name: string): Promise<Project> {
    const title = this.requireName(name);
    return this.db.transaction(async (u) => {
      const project = await this.get(u, key);
      await u.projects.rename(project.id, title);
      return { ...project, name: title };
    });
  }

  // Projekt zum Key innerhalb einer Transaktion oder Fehler
  async get(u: BoardUnitOfWork, key: string): Promise<Project> {
    const project = await u.projects.byKey(this.ids.normalizeKey(key));
    if (!project) throw new HarnessError('project-not-found', `Projekt ${key.toUpperCase()} gibt es nicht.`);
    return project;
  }

  // Name ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
  private requireName(name: string): string {
    const title = name.trim();
    if (!title) throw new HarnessError('project-name-empty', 'Der Projektname darf nicht leer sein.');
    return BoardLimits.text(title, MAX_PROJECT_NAME_LENGTH, 'Der Projektname');
  }
}
