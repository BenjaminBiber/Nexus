import { DEFAULT_SERVER_PORT, HarnessProject } from '@harness/core';
import { ServerLauncher } from '../board/ServerLauncher.js';
import { ServerClient } from './ServerClient.js';

// Meldet dem laufenden Server, dass eine Session jetzt an einem Ticket arbeitet (Anzeige der Sessions sofort aktuell).
export class ServerSessionNotice {
  // Best effort: ohne eingerichtetes Projekt oder ohne eigenen Server geschieht nichts
  async tell(projectDir: string, ticketId: string, sessionId: string): Promise<void> {
    try {
      const project = await HarnessProject.open(projectDir);
      const port = Number(process.env.HARNESS_PORT) || DEFAULT_SERVER_PORT;
      if ((await new ServerLauncher().probe(port)) !== 'ours') return;
      const client = new ServerClient(port);
      const projectId = await client.registerProject(project.root);
      if (projectId) await client.claim(projectId, sessionId, ticketId);
    } catch {
      // Meldung ist optional
    }
  }
}
