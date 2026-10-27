import { link, open, readFile, rename, rm, stat } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';
import { HarnessError } from '../errors/HarnessError.js';

export interface FileLockOptions {
  /** Wie lange auf eine fremde Sperre gewartet wird (Standard 10 s) */
  timeoutMs?: number;
  /** Ab welchem Alter eine Sperre als verwaist gilt (Standard 5 s) */
  staleMs?: number;
}

interface LockInfo {
  pid: number;
  at: number;
  token: string;
}

const BUSY_CODES = new Set(['EEXIST', 'EPERM', 'EBUSY', 'EACCES']);

// Sperre über eine Datei, die atomar angelegt wird (`wx`). Funktioniert unter Windows und Linux gleich.
export class FileLock {
  private readonly timeoutMs: number;
  private readonly staleMs: number;
  // Gleichzeitige Aktionen im selben Prozess werden hintereinander gestellt
  private queue: Promise<unknown> = Promise.resolve();

  constructor(private readonly lockPath: string, options: FileLockOptions = {}) {
    this.timeoutMs = options.timeoutMs ?? 10000;
    this.staleMs = options.staleMs ?? 5000;
  }

  // Führt `action` unter der Sperre aus und gibt sie danach immer wieder frei
  withLock<T>(action: () => T | Promise<T>): Promise<T> {
    const run = this.queue.then(() => this.runLocked(action));
    this.queue = run.catch(() => undefined);
    return run;
  }

  // Sperre holen, Aktion ausführen, Sperre freigeben
  private async runLocked<T>(action: () => T | Promise<T>): Promise<T> {
    const token = await this.acquire();
    try {
      return await action();
    } finally {
      await this.release(token);
    }
  }

  // Wartet mit wachsender Pause, bis die Sperre angelegt werden kann oder die Zeit abläuft
  private async acquire(): Promise<string> {
    const deadline = Date.now() + this.timeoutMs;
    let pause = 5;
    while (true) {
      const token = await this.tryCreate();
      if (token) return token;
      const stale = await this.staleLock();
      if (stale) {
        await this.removeStale(stale.token);
        continue;
      }
      if (Date.now() >= deadline) {
        throw new HarnessError(
          'lock-timeout',
          `Eine andere Aktion hält die Sperre (${this.lockPath}). Bitte später erneut versuchen.`,
        );
      }
      await this.sleep(pause + Math.random() * pause);
      pause = Math.min(pause * 2, 100);
    }
  }

  // Legt die Sperrdatei atomar an; liefert das Kennzeichen oder null, wenn sie schon existiert
  private async tryCreate(): Promise<string | null> {
    const token = randomBytes(8).toString('hex');
    try {
      const handle = await open(this.lockPath, 'wx');
      const info: LockInfo = { pid: process.pid, at: Date.now(), token };
      await handle.writeFile(JSON.stringify(info));
      await handle.close();
      return token;
    } catch (error) {
      if (BUSY_CODES.has((error as NodeJS.ErrnoException).code ?? '')) return null;
      throw error;
    }
  }

  // Verwaist: Prozess existiert nicht mehr oder die Sperre ist älter als `staleMs`.
  // Liefert das Kennzeichen der verwaisten Sperre (null, wenn unlesbar), sonst undefined.
  private async staleLock(): Promise<{ token: string | null } | undefined> {
    try {
      const info = await this.readInfo();
      if (info && !this.isAlive(info.pid)) return { token: info.token };
      const age = Date.now() - (await stat(this.lockPath)).mtimeMs;
      return age > this.staleMs ? { token: info?.token ?? null } : undefined;
    } catch {
      return undefined; // Datei ist inzwischen weg, der nächste Versuch klappt
    }
  }

  // Entfernt eine verwaiste Sperre. Sie wird zuerst beiseite umbenannt (das gelingt nur einem Prozess),
  // erst dann wird das Kennzeichen aus der beiseite gelegten Datei gelesen. Trägt sie inzwischen ein anderes
  // Kennzeichen (Halter hat freigegeben, ein anderer Prozess hat sie neu geholt) oder ist sie unlesbar und
  // noch jung, wird sie per link zurückgestellt; link überschreibt nie: Liegt dort schon eine neue Sperre
  // (EEXIST), gilt diese, und die beiseite gelegte Kopie entfällt.
  // Die Vorprüfung vor dem Umbenennen ist nötig: Ohne sie würde eine längst neu vergebene Sperre kurz
  // beiseite gelegt, und in dieser Lücke könnte ein dritter Prozess eine zweite Sperre anlegen.
  private async removeStale(expectedToken: string | null): Promise<void> {
    if (((await this.readInfo())?.token ?? null) !== expectedToken) return;
    const aside = `${this.lockPath}.stale.${process.pid}.${randomBytes(4).toString('hex')}`;
    try {
      await rename(this.lockPath, aside);
    } catch {
      return; // Ein anderer Prozess war schneller oder die Sperre ist schon weg
    }
    try {
      if (!(await this.isStaleAside(aside, expectedToken))) await this.restore(aside);
    } finally {
      await rm(aside, { force: true }).catch(() => undefined);
    }
  }

  // Ist die beiseite gelegte Sperre noch die erwartete verwaiste? Ohne lesbares Kennzeichen zählt nur das Alter.
  private async isStaleAside(aside: string, expectedToken: string | null): Promise<boolean> {
    const token = (await this.readInfo(aside))?.token ?? null;
    if (token !== expectedToken) return false;
    if (token !== null) return true;
    try {
      return Date.now() - (await stat(aside)).mtimeMs > this.staleMs;
    } catch {
      return false;
    }
  }

  // Stellt die beiseite gelegte Sperre zurück, ohne eine inzwischen neu angelegte zu überschreiben
  private async restore(aside: string): Promise<void> {
    try {
      await link(aside, this.lockPath);
    } catch {
      // EEXIST: die neue Sperre gilt. Andere Fehler: lieber die alte Sperre verlieren als eine fremde überschreiben
    }
  }

  // Gibt die Sperre frei, aber nur, wenn sie noch unser Kennzeichen trägt
  private async release(token: string): Promise<void> {
    const info = await this.readInfo().catch(() => null);
    if (info && info.token !== token) return;
    for (let attempt = 0; attempt < 20; attempt++) {
      try {
        await rm(this.lockPath, { force: true });
        return;
      } catch {
        await this.sleep(10); // Windows: Datei kurz von Virenscanner oder Indexdienst gehalten
      }
    }
  }

  // Liest die Angaben in der Sperrdatei (null, wenn unlesbar)
  private async readInfo(path: string = this.lockPath): Promise<LockInfo | null> {
    try {
      return JSON.parse(await readFile(path, 'utf8')) as LockInfo;
    } catch {
      return null;
    }
  }

  // Prüft, ob ein Prozess mit dieser Nummer noch läuft
  private isAlive(pid: number): boolean {
    try {
      process.kill(pid, 0);
      return true;
    } catch (error) {
      return (error as NodeJS.ErrnoException).code === 'EPERM';
    }
  }

  // Wartet die angegebene Zeit
  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}
