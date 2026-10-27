import { rename, rm, writeFile } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';

const RETRY_CODES = new Set(['EPERM', 'EBUSY', 'EACCES']);

// Schreibt Dateien atomar (temporäre Datei, dann Umbenennen), damit nie ein halber Inhalt sichtbar ist.
export class AtomicWriter {
  // Schreibt `content` nach `path`; unter Windows wird bei kurzer Sperre durch Dritte wiederholt
  async write(path: string, content: string): Promise<void> {
    const temp = `${path}.${process.pid}.${randomBytes(4).toString('hex')}.tmp`;
    await writeFile(temp, content, 'utf8');
    try {
      await this.renameWithRetry(temp, path);
    } catch (error) {
      await rm(temp, { force: true });
      throw error;
    }
  }

  // Benennt um und wiederholt kurz bei EPERM/EBUSY (Virenscanner, Indexdienst)
  private async renameWithRetry(from: string, to: string): Promise<void> {
    for (let attempt = 0; ; attempt++) {
      try {
        await rename(from, to);
        return;
      } catch (error) {
        const code = (error as NodeJS.ErrnoException).code ?? '';
        if (!RETRY_CODES.has(code) || attempt >= 40) throw error;
        await new Promise((resolve) => setTimeout(resolve, 10 + attempt * 5));
      }
    }
  }
}
