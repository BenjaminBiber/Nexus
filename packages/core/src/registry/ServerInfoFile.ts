import { existsSync } from 'node:fs';
import { mkdir, readFile, rm } from 'node:fs/promises';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import type { HarnessHome } from './HarnessHome.js';

export interface ServerInfo {
  pid: number;
  port: number;
  startedAt: string;
}

// Prozessangaben des laufenden Servers (`server.json`); nur ein Hinweis, maßgeblich ist der Health-Aufruf.
export class ServerInfoFile {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {}

  // Liest die Angaben (null, wenn es sie nicht gibt oder sie unlesbar sind)
  async read(): Promise<ServerInfo | null> {
    if (!existsSync(this.home.serverFile)) return null;
    try {
      return JSON.parse(await readFile(this.home.serverFile, 'utf8')) as ServerInfo;
    } catch {
      return null;
    }
  }

  // Schreibt die Angaben
  async write(info: ServerInfo): Promise<void> {
    await mkdir(this.home.dir, { recursive: true });
    await this.writer.write(this.home.serverFile, JSON.stringify(info));
  }

  // Löscht die Angaben beim Beenden
  async remove(): Promise<void> {
    await rm(this.home.serverFile, { force: true });
  }
}
