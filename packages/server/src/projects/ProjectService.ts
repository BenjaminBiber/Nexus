import { isAbsolute } from 'node:path';
import { HarnessError, HarnessProject, ProjectRegistry, type HarnessHome, type ProjectState, type RegisteredProject } from '@harness/core';

// Verwaltet die Projekte des Servers: Liste, Anmeldung und geöffnete Projekte (zwischengespeichert).
export class ProjectService {
  readonly registry: ProjectRegistry;
  private readonly open_ = new Map<string, HarnessProject>();

  constructor(home: HarnessHome) {
    this.registry = new ProjectRegistry(home);
  }

  // Alle Projekte mit Erreichbarkeit
  list(): Promise<ProjectState[]> {
    return this.registry.listWithState();
  }

  // Meldet ein Projekt an; nur absolute Pfade
  async register(path: unknown): Promise<{ project: ProjectState; created: boolean }> {
    if (typeof path !== 'string' || !path.trim()) {
      throw new HarnessError('path-missing', 'Der Pfad des Projekts fehlt.');
    }
    if (!isAbsolute(path)) throw new HarnessError('path-relative', 'Der Pfad muss absolut sein.');
    const { project, created } = await this.registry.add(path);
    return { project: { ...project, reachable: true }, created };
  }

  // Öffnet ein Projekt über einen (auch im Worktree liegenden) Pfad; es muss angemeldet sein
  async openByPath(path: string): Promise<{ project: HarnessProject; entry: RegisteredProject }> {
    const opened = await HarnessProject.open(path);
    return this.open(this.registry.idOf(opened.root));
  }

  // Liefert das geöffnete Projekt zur ID; unbekannt → 404, nicht erreichbar → Fehler aus `core`
  async open(id: string): Promise<{ project: HarnessProject; entry: RegisteredProject }> {
    const entry = await this.registry.find(id);
    if (!entry) throw new HarnessError('project-not-found', `Projekt ${id} gibt es nicht.`);
    let project = this.open_.get(id);
    if (!project) {
      project = await HarnessProject.open(entry.path);
      this.open_.set(id, project);
    }
    return { project, entry };
  }
}
