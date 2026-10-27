import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { AtomicWriter } from '../locking/AtomicWriter.js';

// Nimmt Dateien lokal aus Git heraus: Einträge in `<git-common-dir>/info/exclude` gelten für Hauptordner und alle Worktrees,
// ohne dass sich eine versionierte `.gitignore` ändert.
export class GitExclude {
  private readonly writer = new AtomicWriter();

  // Hängt die Zeile an, falls sie fehlt; liefert false, wenn sie schon eingetragen ist
  async add(commonDir: string, line: string): Promise<boolean> {
    const file = join(commonDir, 'info', 'exclude');
    const text = existsSync(file) ? await readFile(file, 'utf8') : '';
    if (text.split(/\r?\n/).some((l) => l.trim() === line)) return false;
    const separator = text === '' || text.endsWith('\n') ? '' : '\n';
    await mkdir(join(commonDir, 'info'), { recursive: true });
    await this.writer.write(file, `${text}${separator}${line}\n`);
    return true;
  }
}
