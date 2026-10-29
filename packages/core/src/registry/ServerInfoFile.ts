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

  // Löscht die Angaben (`harness stop`, nachdem es den eingetragenen Server beendet hat)
  async remove(): Promise<void> {
    await rm(this.home.serverFile, { force: true });
  }

  // Trägt den startenden Server ein – außer ein anderer, noch laufender Server steht schon darin
  // (etwa wenn ein Testserver versehentlich mit dem echten Benutzerordner startet). `false`: nicht eingetragen
  async claim(info: ServerInfo): Promise<boolean> {
    const current = await this.read();
    if (current && current.pid !== info.pid && ServerInfoFile.alive(current.pid)) return false;
    await this.write(info);
    return true;
  }

  // Löscht die Angaben beim Beenden, aber nur, wenn sie zu diesem Server (`pid`) gehören
  async release(pid: number): Promise<void> {
    if ((await this.read())?.pid === pid) await this.remove();
  }

  // Läuft der Prozess noch? Gehört er einem anderen Benutzer, zählt er als laufend
  private static alive(pid: number): boolean {
    try {
      process.kill(pid, 0);
      return true;
    } catch (error) {
      return (error as NodeJS.ErrnoException).code === 'EPERM';
    }
  }
}
