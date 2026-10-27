import { existsSync } from 'node:fs';
import { appendFile, readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import type { UserPath } from './UserPath.js';

const PROFILES = ['.zshrc', '.bashrc', '.profile'];
const MARKER = '# ai-harness';

// PATH-Eintrag unter macOS und Linux: eine Zeile in den vorhandenen Shell-Profilen (sonst `.profile`).
export class ShellProfilePath implements UserPath {
  constructor(private readonly homeDir: string = homedir()) {}

  // Hängt die Zeile an jedes passende Profil an, das den Ordner noch nicht kennt
  async add(dir: string): Promise<boolean> {
    let changed = false;
    for (const file of this.targets()) {
      const text = existsSync(file) ? await readFile(file, 'utf8') : '';
      if (text.includes(dir)) continue;
      const prefix = text === '' || text.endsWith('\n') ? '' : '\n';
      await appendFile(file, `${prefix}\n${MARKER}\nexport PATH="${dir}:$PATH"\n`, 'utf8');
      changed = true;
    }
    return changed;
  }

  // Vorhandene Profile; gibt es keines, wird `.profile` neu angelegt
  private targets(): string[] {
    const files = PROFILES.map((name) => join(this.homeDir, name));
    const existing = files.filter((file) => existsSync(file));
    return existing.length > 0 ? existing : [join(this.homeDir, '.profile')];
  }
}
