import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import { FileLock } from '../locking/FileLock.js';
import { HarnessError } from '../errors/HarnessError.js';
import { HarnessProject } from '../project/HarnessProject.js';
import type { HarnessHome } from './HarnessHome.js';

export interface RegisteredProject {
  id: string;
  name: string;
  path: string;
}

export interface ProjectState extends RegisteredProject {
  reachable: boolean;
}

// Benutzerweite Liste der angemeldeten Projekte (`projects.json`); Schreiben nur unter Sperre.
export class ProjectRegistry {
  private readonly lock: FileLock;
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {
    this.lock = new FileLock(home.projectsLock);
  }

  // Alle angemeldeten Projekte
  async list(): Promise<RegisteredProject[]> {
    if (!existsSync(this.home.projectsFile)) return [];
    try {
      return JSON.parse(await readFile(this.home.projectsFile, 'utf8')) as RegisteredProject[];
    } catch {
      throw new HarnessError('projects-corrupt', `Die Projektliste ist beschädigt: ${this.home.projectsFile}`);
    }
  }

  // Projekte mit Angabe, ob Ordner und Harness noch erreichbar sind (Einträge bleiben erhalten)
  async listWithState(): Promise<ProjectState[]> {
    const projects = await this.list();
    return Promise.all(projects.map(async (p) => ({ ...p, reachable: await this.isReachable(p.path) })));
  }

  // Sucht ein Projekt über seine ID
  async find(id: string): Promise<RegisteredProject | undefined> {
    return (await this.list()).find((p) => p.id === id);
  }

  // Meldet ein Projekt an (idempotent); auch ein Worktree-Pfad führt zum Hauptordner
  async add(path: string): Promise<{ project: RegisteredProject; created: boolean }> {
    const opened = await HarnessProject.open(path);
    const project: RegisteredProject = { id: this.idOf(opened.root), name: basename(opened.root), path: opened.root };
    await mkdir(this.home.dir, { recursive: true });
    const created = await this.lock.withLock(async () => {
      const all = await this.list();
      if (all.some((p) => p.id === project.id)) return false;
      await this.writer.write(this.home.projectsFile, JSON.stringify([...all, project], null, 2));
      return true;
    });
    return { project, created };
  }

  // Stabile Kurz-ID aus dem Pfad (unter Windows ohne Beachtung der Schreibweise)
  idOf(path: string): string {
    const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '');
    const key = process.platform === 'win32' ? normalized.toLowerCase() : normalized;
    return createHash('sha1').update(key).digest('hex').slice(0, 8);
  }

  // Ordner vorhanden und Harness eingerichtet?
  private async isReachable(path: string): Promise<boolean> {
    try {
      await HarnessProject.open(path);
      return true;
    } catch {
      return false;
    }
  }
}
