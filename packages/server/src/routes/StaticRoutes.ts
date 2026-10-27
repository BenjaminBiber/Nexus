import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { join, sep } from 'node:path';
import fastifyStatic from '@fastify/static';
import type { FastifyInstance } from 'fastify';

const HINT = '<!doctype html><meta charset="utf-8"><title>AI Harness</title>'
  + '<p>Die Oberfläche ist noch nicht gebaut. Bitte <code>npm run build</code> ausführen.</p>';

// Dateien unter assets/ tragen einen Inhalts-Hash im Namen und ändern sich nie; alles andere (index.html) wird jedes Mal geprüft
const IMMUTABLE = 'public, max-age=31536000, immutable';
const REVALIDATE = 'no-cache';

// Liefert die gebaute Oberfläche aus und beantwortet Seitenadressen mit index.html (Seitenrouting).
// Liegen .br/.gz neben einer Datei (vom Build erzeugt), gehen sie an Browser, die sie annehmen.
export class StaticRoutes {
  constructor(private readonly webDir: string | null) {}

  // Meldet Auslieferung und Rückfall an
  async register(app: FastifyInstance): Promise<void> {
    const index = this.webDir ? join(this.webDir, 'index.html') : null;
    const hasWeb = index !== null && existsSync(index);
    if (hasWeb) {
      const assets = join(this.webDir!, 'assets') + sep;
      await app.register(fastifyStatic, {
        root: this.webDir!,
        preCompressed: true,
        cacheControl: false,
        setHeaders: (res, path) => res.setHeader('cache-control', path.startsWith(assets) ? IMMUTABLE : REVALIDATE),
      });
    } else app.get('/', async (_request, reply) => reply.type('text/html; charset=utf-8').send(HINT));

    app.setNotFoundHandler(async (request, reply) => {
      const url = request.url.split('?')[0];
      // Docs-Adressen enden auf den Dateinamen (z. B. .md) und sind trotzdem Seiten, keine Dateien
      const isPage = !/\.[a-z0-9]+$/i.test(url) || /^\/p\/[^/]+\/docs\//.test(url);
      if (hasWeb && request.method === 'GET' && !url.startsWith('/api') && isPage) {
        return reply.type('text/html; charset=utf-8').header('cache-control', REVALIDATE).send(await readFile(index!, 'utf8'));
      }
      return reply.code(404).send({ code: 'not-found', message: 'Diese Adresse gibt es nicht.' });
    });
  }
}
