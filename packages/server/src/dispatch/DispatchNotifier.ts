import type { NotificationService } from '../notifications/NotificationService.js';
import type { ProjectService } from '../projects/ProjectService.js';
import type { DispatchNotice } from './DispatchContext.js';

// Leitet Meldungen des Dispatchers an die Windows-Meldungen weiter; die Adresse zeigt ins Harness-Projekt des Board-Repos.
export class DispatchNotifier {
  constructor(
    private readonly projects: ProjectService,
    private readonly notifications: NotificationService,
  ) {}

  // Meldung mit passendem Projekt (Repo-Pfad des Boards, sonst das erste erreichbare)
  async notify(notice: DispatchNotice): Promise<void> {
    const norm = (p: string) => p.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase();
    const reachable = (await this.projects.list()).filter((p) => p.reachable);
    const wanted = notice.repoPath ? norm(notice.repoPath) : null;
    const project = reachable.find((p) => wanted !== null && norm(p.path) === wanted) ?? reachable[0];
    await this.notifications.onDispatch({ projectId: project?.id ?? '', notice });
  }
}
