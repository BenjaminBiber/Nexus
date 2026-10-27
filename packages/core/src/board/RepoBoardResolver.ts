import type { Board } from './BoardTypes.js';

// Wer fragt: eigener Benutzer und aktives Board aus board.yaml
export interface RepoBoardOwner {
  userId: string;
  activeBoard: number | null;
}

// Ordnet einem Git-Repo (Hauptordner) sein Board über Board.RepoPath zu.
export class RepoBoardResolver {
  constructor(private readonly platform: string = process.platform) {}

  // Schrägstriche statt Rückstriche, ohne Endstrich; unter Windows ohne Groß-/Kleinschreibung
  normalize(path: string): string {
    const cleaned = path.trim().replace(/\\/g, '/').replace(/\/+$/, '');
    return this.platform === 'win32' ? cleaned.toLowerCase() : cleaned;
  }

  // Mehrere Treffer: eigenes aktives Board, sonst eigenes (kleinste ID), sonst das erste (kleinste ID); kein Treffer: null
  pick(boards: Board[], repoRoot: string, me: RepoBoardOwner): Board | null {
    const wanted = this.normalize(repoRoot);
    const hits = boards.filter((b) => b.repoPath && this.normalize(b.repoPath) === wanted).sort((a, b) => a.id - b.id);
    const own = hits.filter((b) => b.ownerId === me.userId);
    return own.find((b) => b.id === me.activeBoard) ?? own[0] ?? hits[0] ?? null;
  }
}
