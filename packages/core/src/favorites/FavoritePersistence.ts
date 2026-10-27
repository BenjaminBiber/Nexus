import { existsSync } from 'node:fs';
import { copyFile, mkdir, readFile, rm } from 'node:fs/promises';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import type { HarnessHome } from '../registry/HarnessHome.js';
import type { Favorite } from './Favorite.js';

// Speichert die Favoriten aller Projekte in `~/.harness/favorites.json` (atomar).
export class FavoritePersistence {
  private readonly writer = new AtomicWriter();

  constructor(private readonly home: HarnessHome) {}

  // Liest die Favoriten; eine beschädigte Datei wird nach `.bak` gesichert, `recovered` meldet das
  async read(): Promise<{ favorites: Favorite[]; recovered: boolean }> {
    const file = this.home.favoritesFile;
    if (!existsSync(file)) return { favorites: [], recovered: false };
    try {
      const data = JSON.parse(await readFile(file, 'utf8')) as { favorites?: Favorite[] };
      return { favorites: Array.isArray(data.favorites) ? data.favorites : [], recovered: false };
    } catch {
      await copyFile(file, `${file}.bak`);
      await rm(file, { force: true });
      return { favorites: [], recovered: true };
    }
  }

  // Schreibt die Favoriten; der Ordner wird bei Bedarf angelegt
  async write(favorites: Favorite[]): Promise<void> {
    await mkdir(this.home.dir, { recursive: true });
    await this.writer.write(this.home.favoritesFile, JSON.stringify({ favorites }, null, 2));
  }
}
