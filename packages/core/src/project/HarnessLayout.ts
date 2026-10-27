import { join } from 'node:path';

// Alle Pfade im Harness-Ordner (`<git-common-dir>/harness`).
export class HarnessLayout {
  readonly dir: string;

  constructor(commonDir: string) {
    this.dir = join(commonDir, 'harness');
  }

  get tokenFile(): string { return join(this.dir, 'token'); }
  get countersFile(): string { return join(this.dir, 'counters.json'); }
  get countersLock(): string { return join(this.dir, 'counters.lock'); }
  get sessionNumbersFile(): string { return join(this.dir, 'session-numbers.json'); }
  get sessionNumbersLock(): string { return join(this.dir, 'session-numbers.lock'); }
  get worktreesFile(): string { return join(this.dir, 'worktrees.json'); }
  get worktreesLock(): string { return join(this.dir, 'worktrees.lock'); }
  get questionContextFile(): string { return join(this.dir, 'question-context.json'); }
  get questionContextLock(): string { return join(this.dir, 'question-context.lock'); }
}
