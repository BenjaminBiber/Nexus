import Fastify, { type FastifyInstance } from 'fastify';
import type { ServerConfig } from './config/ServerConfig.js';
import { ErrorMapper } from './errors/ErrorMapper.js';
import { ProjectService } from './projects/ProjectService.js';
import { DocRevisionService } from './docs/DocRevisionService.js';
import { DocQuestionService } from './docs/DocQuestionService.js';
import { DocTicketService } from './docs/DocTicketService.js';
import { DocsRootResolver } from './docs/DocsRootResolver.js';
import { DocsService } from './docs/DocsService.js';
import { TicketDraftService } from './docs/TicketDraftService.js';
import { DocCommentsRoutes } from './routes/DocCommentsRoutes.js';
import { DocsRoutes } from './routes/DocsRoutes.js';
import { SettingsRoutes } from './routes/SettingsRoutes.js';
import { EventHub } from './events/EventHub.js';
import { SessionService } from './sessions/SessionService.js';
import { HookRoutes } from './routes/HookRoutes.js';
import { SessionRoutes } from './routes/SessionRoutes.js';
import { TicketInfoLookup } from './sessions/TicketInfoLookup.js';
import { BoardChangeWatcher, type DbQuestionsEvent } from './events/BoardChangeWatcher.js';
import { SpecDocWatcher } from './events/SpecDocWatcher.js';
import { EventRoutes } from './routes/EventRoutes.js';
import { SpecRoutes } from './routes/SpecRoutes.js';
import { HealthRoutes } from './routes/HealthRoutes.js';
import { ProjectRoutes } from './routes/ProjectRoutes.js';
import { StaticRoutes } from './routes/StaticRoutes.js';
import { FavoriteService } from './favorites/FavoriteService.js';
import { FavoriteRoutes } from './routes/FavoriteRoutes.js';
import { BoardRuntime } from '@harness/db';
import { BoardRuntimeHolder } from './dbboard/BoardRuntimeHolder.js';
import { TicketTimelineAssembler } from './dbboard/TicketTimelineAssembler.js';
import { SuperpowersDocScanner, UserSettingsStore } from '@harness/core';
import { UserSettingsRoutes } from './routes/UserSettingsRoutes.js';
import { BoardConfigRoutes } from './routes/BoardConfigRoutes.js';
import { DbBoardRoutes } from './routes/DbBoardRoutes.js';
import { WindowsFolderPicker } from './dbboard/FolderPicker.js';
import { WindowsFileOpener } from './docs/FileOpener.js';
import { DbInboxRoutes } from './routes/DbInboxRoutes.js';
import { DbNoteRoutes } from './routes/DbNoteRoutes.js';
import { DbSearchRoutes } from './routes/DbSearchRoutes.js';
import { DbStatsRoutes } from './routes/DbStatsRoutes.js';
import { DbGraphRoutes } from './routes/DbGraphRoutes.js';
import { DbTicketRoutes } from './routes/DbTicketRoutes.js';
import { DispatchService } from './dispatch/DispatchService.js';
import { DispatchScheduler } from './dispatch/DispatchScheduler.js';
import { DispatchNotifier } from './dispatch/DispatchNotifier.js';
import { ProcessWorkerLauncher } from './dispatch/WorkerLauncher.js';
import { DispatchRoutes } from './routes/DispatchRoutes.js';
import { DispatchSuggestionService } from './dispatch/DispatchSuggestionService.js';
import { DispatchSuggestionRoutes } from './routes/DispatchSuggestionRoutes.js';
import { SuggestionAnalyzer } from './dispatch/SuggestionAnalyzer.js';
import { ClaudeProcess } from './dispatch/ClaudeProcess.js';
import { DbQuestionRoutes } from './routes/DbQuestionRoutes.js';
import { WorkflowRoutes } from './routes/WorkflowRoutes.js';
import { CustomizationRoutes } from './routes/CustomizationRoutes.js';
import { NotificationService } from './notifications/NotificationService.js';
import { DbQuestionWatch } from './notifications/DbQuestionWatch.js';
import { WindowsToastSender } from './notifications/WindowsToastSender.js';
import { BrowserNoticeHub } from './notifications/BrowserNoticeHub.js';
import { RequestGuard } from './security/RequestGuard.js';
import { UsageRoutes } from './routes/UsageRoutes.js';
import { ClaudeUsageService } from './usage/ClaudeUsageService.js';

// Baut die Fastify-Instanz: Schutz vor jeder Anfrage, Fehlerabbildung, Routen, Auslieferung.
export class ServerApp {
  // Erzeugt die Anwendung (noch ohne Port; `listen` übernimmt der Startpunkt)
  static async create(config: ServerConfig): Promise<FastifyInstance> {
    const app = Fastify({ logger: false });
    const guard = new RequestGuard(config.port);
    const errors = new ErrorMapper();
    const projects = new ProjectService(config.home);

    app.decorate('guard', guard);
    app.decorate('projects', projects);
    const events = new EventHub();
    app.decorate('events', events);
    const sessions = await SessionService.create(config.home, events);
    app.decorate('sessions', sessions);
    app.addHook('onClose', async () => sessions.close());
    const favorites = await FavoriteService.create(config.home, config.log);

    // Eine Board-Laufzeit für alle /api/db-Routen; Aufbau beim ersten Bedarf
    const boardRuntime = new BoardRuntimeHolder(config.boardBackend ?? (() => BoardRuntime.open(config.home)));
    app.decorate('boardRuntime', boardRuntime);
    app.addHook('onClose', async () => boardRuntime.reset());
    const ticketInfo = new TicketInfoLookup(boardRuntime);

    // Offene Tabs zeigen Meldungen selbst (Klick bleibt im Tab); nur ohne Tab kommt der Windows-Toast
    const noticeTabs = new BrowserNoticeHub();
    app.decorate('noticeTabs', noticeTabs);
    const notifications = new NotificationService(
      new UserSettingsStore(config.home),
      new WindowsToastSender(),
      config.toastDir ?? '',
      `http://${config.host}:${config.port}`,
      config.log,
      noticeTabs,
    );

    // Meldungen für neue Fragen und Recherchen auf der Board-Datenbank (nur Fragen zu Tickets auf eigenen Boards)
    const dbQuestionWatch = new DbQuestionWatch(boardRuntime, projects, notifications, config.dbQuestionPollMs ?? 10000);
    dbQuestionWatch.start();
    // vor dem Schließen der Board-Laufzeit (onClose) anhalten, damit kein Durchlauf sie neu aufbaut
    app.addHook('preClose', async () => dbQuestionWatch.stop());

    // Dispatcher: übernimmt bereite Tickets der eigenen Boards und lässt sie von claude -p umsetzen (Takt jede Minute)
    const dispatchNotifier = new DispatchNotifier(projects, notifications);
    const dispatch = new DispatchService({
      home: config.home, boards: boardRuntime, launcher: config.dispatchLauncher ?? new ProcessWorkerLauncher(),
      notify: (notice) => dispatchNotifier.notify(notice),
    });
    const dispatchScheduler = new DispatchScheduler(dispatch, config.dispatchTickMs ?? 60000, config.log);
    dispatchScheduler.start();
    // Takt anhalten und auf einen laufenden Tick warten (höchstens 30 s), bevor onClose die Board-Laufzeit abbaut
    app.addHook('preClose', async () => dispatchScheduler.stop());

    app.addHook('onRequest', async (request, reply) => {
      const denial = guard.check(request.method, request.headers);
      if (denial) return reply.code(403).send(denial);
    });
    app.setErrorHandler((error, _request, reply) => {
      const mapped = errors.map(error);
      return reply.code(mapped.status).send(mapped.body);
    });

    new HealthRoutes(config.version).register(app);
    new UsageRoutes(config.claudeUsage ?? new ClaudeUsageService(), new UserSettingsStore(config.home)).register(app);
    new ProjectRoutes(projects).register(app);
    const docsService = new DocsService();
    const docsRoot = new DocsRootResolver(boardRuntime, config.home);
    const docRevisions = new DocRevisionService(docsService, config.reviseClaude ?? new ClaudeProcess(), config.home);
    const ticketDrafts = new TicketDraftService(docsService, config.ticketDraftClaude ?? new ClaudeProcess(), config.home);
    const docTickets = new DocTicketService(docsService, boardRuntime);
    // Fragen-Entwürfe laufen über denselben Claude-Aufruf wie die Ticket-Entwürfe
    const docQuestions = new DocQuestionService(docsService, config.ticketDraftClaude ?? new ClaudeProcess(), config.home, boardRuntime, docTickets);
    new DocsRoutes(projects, docsService, docsRoot, docRevisions, ticketDrafts, docTickets, config.fileOpener ?? new WindowsFileOpener(), docQuestions).register(app);
    new DocCommentsRoutes(projects, docsService, docsRoot).register(app);
    new SpecRoutes(projects, boardRuntime, config.home).register(app);
    new SettingsRoutes(projects).register(app);
    new UserSettingsRoutes(new UserSettingsStore(config.home), notifications).register(app);
    new WorkflowRoutes(config.home).register(app);
    new CustomizationRoutes(config.home).register(app);
    new BoardConfigRoutes(config.home, () => boardRuntime.reset()).register(app);
    new DbBoardRoutes(boardRuntime, config.home, config.folderPicker ?? new WindowsFolderPicker()).register(app);
    new DbTicketRoutes(boardRuntime, new TicketTimelineAssembler(boardRuntime, config.home)).register(app);
    new DbNoteRoutes(boardRuntime).register(app);
    new DbSearchRoutes(boardRuntime).register(app);
    new DbStatsRoutes(boardRuntime).register(app);
    new DbGraphRoutes(boardRuntime).register(app);
    new DbInboxRoutes(boardRuntime, config.home).register(app);
    new DbQuestionRoutes(boardRuntime).register(app);
    new DispatchRoutes(dispatch, config.home).register(app);
    const suggestions = new DispatchSuggestionService(boardRuntime, config.home);
    new DispatchSuggestionRoutes(suggestions, new SuggestionAnalyzer(suggestions, config.suggestionClaude ?? new ClaudeProcess(), config.home)).register(app);
    new HookRoutes(projects, sessions, favorites, notifications, ticketInfo).register(app);
    new SessionRoutes(projects, sessions, ticketInfo, boardRuntime).register(app);
    new FavoriteRoutes(projects, sessions, favorites, ticketInfo).register(app);
    // Änderungen an beobachteten DB-Boards erkennen (nur solange ein Ereignisstrom sie beobachtet)
    const boardWatcher = new BoardChangeWatcher((id) => boardRuntime.run((b) => b.services.db.changeMarker(id)), config.dbPollMs ?? 2500);
    // Änderungen an den Fragen beobachteter Board-Projekte (für den Reiter „Fragen“)
    const questionWatcher = new BoardChangeWatcher<DbQuestionsEvent>(
      (id) => boardRuntime.run((b) => b.services.db.questionMarker(id)),
      config.dbPollMs ?? 2500,
      (projectId) => ({ type: 'dbquestions', projectId }),
    );
    // Änderungen an den Superpowers-Dokumenten beobachteter Projekte (Ereignis docs-changed)
    const docScanner = new SuperpowersDocScanner();
    const docWatcher = new SpecDocWatcher(async (pid) => {
      const { project } = await projects.open(pid);
      const source = await docsRoot.resolve(project.root);
      // Ohne Quelle ein fester Wert – taucht später eine auf, meldet das genau eine Änderung
      return source.root ? docScanner.signature(source.root) : 'none';
    }, config.docsPollMs ?? 10000);
    new EventRoutes(projects, events, config.heartbeatMs ?? 15000, boardWatcher, questionWatcher, docWatcher, noticeTabs).register(app);
    await new StaticRoutes(config.webDir).register(app);
    return app;
  }
}

declare module 'fastify' {
  interface FastifyInstance {
    guard: RequestGuard;
    projects: ProjectService;
    events: EventHub;
    noticeTabs: BrowserNoticeHub;
    sessions: SessionService;
    boardRuntime: BoardRuntimeHolder;
  }
}
