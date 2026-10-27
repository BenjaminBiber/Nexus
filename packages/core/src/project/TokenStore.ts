import { randomBytes } from 'node:crypto';
import { existsSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import type { HarnessLayout } from './HarnessLayout.js';

// Zugriffs-Token für spätere Hook-Aufrufe; wird einmal bei `harness init` erzeugt.
export class TokenStore {
  constructor(private readonly layout: HarnessLayout) {}

  // Legt das Token an, falls es noch fehlt (bestehendes Token bleibt unverändert)
  async ensure(): Promise<void> {
    if (existsSync(this.layout.tokenFile)) return;
    await writeFile(this.layout.tokenFile, randomBytes(32).toString('hex'), { encoding: 'utf8', flag: 'wx' }).catch(
      (error) => {
        if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
      },
    );
  }

  // Liest das Token
  async read(): Promise<string> {
    return (await readFile(this.layout.tokenFile, 'utf8')).trim();
  }
}
