import type { BoardBackendFactory } from '../../src/dbboard/BoardBackend.js';
import { TestServer } from './TestServer.js';

export interface Collected {
  events: any[];
  raw: string;
  stop: () => Promise<void>;
}

// Server mit echtem Port für Tests der Live-Meldungen (Server-Sent Events).
export class LiveServer {
  private constructor(
    readonly server: TestServer,
    readonly url: string,
  ) {}

  // Startet den Server auf einem freien Port und erlaubt ihn im Schutz; optional mit Board-Laufzeit im Speicher
  static async start(heartbeatMs = 15000, options: { boardBackend?: BoardBackendFactory; dbPollMs?: number } = {}): Promise<LiveServer> {
    const server = await TestServer.start({ heartbeatMs, ...options });
    await server.app.listen({ port: 0, host: '127.0.0.1' });
    const port = (server.app.server.address() as { port: number }).port;
    server.app.guard.allowPort(port);
    return new LiveServer(server, `http://127.0.0.1:${port}`);
  }

  // Öffnet den Ereignisstrom eines Projekts und sammelt alles, was ankommt
  async listen(projectId: string, query = ''): Promise<Collected> {
    const controller = new AbortController();
    const res = await fetch(`${this.url}/api/projects/${projectId}/events${query}`, { signal: controller.signal });
    if (!res.ok || !res.body) throw new Error(`Ereignisstrom nicht erreichbar: ${res.status}`);
    const collected: Collected = {
      events: [],
      raw: '',
      stop: async () => controller.abort(),
    };
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    (async () => {
      try {
        for (;;) {
          const { done, value } = await reader.read();
          if (done) return;
          collected.raw += decoder.decode(value, { stream: true });
          collected.events = collected.raw
            .split('\n\n')
            .map((block) => block.trim())
            .filter((block) => block.startsWith('data:'))
            .map((block) => JSON.parse(block.slice(5)));
        }
      } catch {
        /* abgebrochen */
      }
    })();
    return collected;
  }

  // Wartet, bis die Bedingung erfüllt ist (höchstens `ms`)
  static async until(condition: () => boolean, ms = 3000): Promise<boolean> {
    const deadline = Date.now() + ms;
    while (Date.now() < deadline) {
      if (condition()) return true;
      await new Promise((r) => setTimeout(r, 25));
    }
    return condition();
  }
}
