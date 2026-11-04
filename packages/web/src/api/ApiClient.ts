import { ApiError } from './ApiError.js';
import type {
  DispatchConfigViewDto, DispatchMergeResultDto, DispatchPromptKindDto, DispatchPromptPreviewDto, DispatchResearchStartDto, DispatchReviewDto, DispatchReviewPatchDto, DispatchRunLogDto, DispatchRunLogPartsDto, DispatchStatusDto, QuestionResearchStatusDto, RawDispatchConfigDto, TicketPlanningStatusDto,
  SuggestionApplyDto, SuggestionListDto,
} from './dispatchTypes.js';
import type {
  BoardConfigDto, BoardConfigInput, BoardConfigTestDto, FavoriteDto, DocFileContentDto, DocsListDto, SessionDto, SettingsDto, UserSettingsDto, TestNotificationDto,
  SearchHitDto, QuestionDraftInputDto, QuestionDraftResultDto, DocQuestionResultDto, TicketDraftDto, TicketDraftInput, TicketDraftResultDto, DocTicketInput, DocTicketRefDto, DocTicketResultDto, DocCommentDto, NewDocCommentInput, DocCommentPatch, DocWriteResultDto, WriteDocSectionInput,
  ReviseDocInput, ReviseDocResultDto, ProjectDto,
  WorkflowDto, CustomizationsDto, PluginSetupDto, PluginInstallDto, BoardMigrationDto, BoardMigrationResultDto,
} from './types.js';
import type {
  DbActiveDto, DbBoardDto, DbBoardPatch, DbBoardViewDto, DbBranchListDto, DbCreatedBranchDto, DbDecisionInput, DbGraphDto, DbGraphScope, DbSpecProgressDto, DbImportResult, DbMoveInput, DbNewNoteInput, DbNewQuestionInput,
  DbNewTicketInput, DbNoteDto, DbNoteFilter, DbNoteTakeInput, DbNoteTakeResult, DbProjectDto, DbQuestionDetailDto, DbQuestionListDto,
  DbQuestionPatch, DbSearchKind, DbSearchResultDto, DbStepChange, DbStepInput, DbTicketDetailDto, DbTicketDto, DbTicketPatch, DbTicketTimelineDto,
} from './dbTypes.js';
import type { DispatchStatsDto, DispatchStatsFilter } from './statsTypes.js';
import type { FileDiff, SessionChanges, SessionDetail, TranscriptPage } from './sessionDetailTypes.js';
import type { InboxListDto } from './inboxTypes.js';
import type { AccountListDto, ClaudeUsageDto, UsageOverviewDto } from './usageTypes.js';
import type {
  SpeechStatusDto, SpeechTestDto, TelegramConfig, TelegramConfigInput, TelegramPairingDto, TelegramProfileDto, TelegramStatusDto, TelegramTestDto,
} from './telegramTypes.js';

// Wie lange ein laufendes GET für dieselbe Adresse mitbenutzt wird: deckt gleichzeitiges Laden beim Seitenaufbau ab,
// aber nicht ein Nachladen nach einem Ereignis des Servers
const SHARE_WINDOW_MS = 50;

// Hängt das Board des Docs-Umschalters an eine Docs-Adresse; ohne Board bleibt sie, wie sie ist
const withBoard = (url: string, board?: number) => (board === undefined ? url : `${url}${url.includes('?') ? '&' : '?'}board=${board}`);

// Spricht mit dem Harness-Server; schreibende Anfragen tragen den Pflichtkopf.
// In der Desktop-App kommt die Oberfläche aus den Dateien der App und damit von einer fremden Herkunft:
// dort stehen die Serveradresse in `base` und das Start-Token in `token`, das jede Anfrage mitnimmt.
export class ApiClient {
  constructor(private readonly base = '', private readonly token?: string) {}

  // Auslastung der Claude-Abo-Limits (5-Stunden-Fenster, Woche)
  getClaudeUsage(): Promise<ClaudeUsageDto> {
    return this.request('GET', '/api/claude-usage');
  }

  // Limits aller aktiven Accounts samt Obergrenzen (Ringe in der Seitenleiste)
  getUsageOverview(): Promise<UsageOverviewDto> {
    return this.request('GET', '/api/claude-usage/accounts');
  }

  getAccountNames(): Promise<{ accounts: Array<{ id: string; name: string }> }> {
    return this.request('GET', '/api/accounts/names');
  }

  getAccounts(): Promise<AccountListDto> {
    return this.request('GET', '/api/accounts');
  }

  createAccount(name: string, configDir: string | null): Promise<AccountListDto> {
    return this.request('POST', '/api/accounts', { name, configDir });
  }

  updateAccount(id: string, patch: { name?: string; enabled?: boolean; fallback?: boolean; limitFiveHour?: number | null; limitWeek?: number | null }): Promise<AccountListDto> {
    return this.request('PATCH', `/api/accounts/${encodeURIComponent(id)}`, patch);
  }

  deleteAccount(id: string, deleteDir: boolean): Promise<AccountListDto> {
    return this.request('DELETE', `/api/accounts/${encodeURIComponent(id)}${deleteDir ? '?deleteDir=1' : ''}`, {});
  }

  loginNewAccount(name: string, email: string): Promise<AccountListDto> {
    return this.request('POST', '/api/accounts/login', { name, email });
  }

  loginAccount(id: string): Promise<AccountListDto> {
    return this.request('POST', `/api/accounts/${encodeURIComponent(id)}/login`, {});
  }

  sendLoginCode(id: string, code: string): Promise<AccountListDto> {
    return this.request('POST', `/api/accounts/${encodeURIComponent(id)}/login/code`, { code });
  }

  cancelLogin(id: string): Promise<AccountListDto> {
    return this.request('DELETE', `/api/accounts/${encodeURIComponent(id)}/login`, {});
  }

  installAccountPlugin(id: string): Promise<AccountListDto> {
    return this.request('POST', `/api/accounts/${encodeURIComponent(id)}/plugin`, {});
  }

  resumeAccount(id: string): Promise<AccountListDto> {
    return this.request('POST', `/api/accounts/${encodeURIComponent(id)}/resume`, {});
  }

  setAccountRunToken(id: string, token: string): Promise<AccountListDto> {
    return this.request('PUT', `/api/accounts/${encodeURIComponent(id)}/run-token`, { token });
  }

  removeAccountRunToken(id: string): Promise<AccountListDto> {
    return this.request('DELETE', `/api/accounts/${encodeURIComponent(id)}/run-token`, {});
  }

  // Alle angemeldeten Projekte
  getProjects(): Promise<ProjectDto[]> {
    return this.request('GET', '/api/projects');
  }

  // Lesbare Dokumente eines Projekts samt Quelle (Superpowers-Specs und -Pläne, Glossar, ADRs).
  // Alle Docs-Methoden nehmen optional `board`: dann kommen die Dokumente aus dem Repo dieses Boards statt aus dem Projektordner.
  getDocs(pid: string, board?: number): Promise<DocsListDto> {
    return this.request('GET', withBoard(`/api/projects/${pid}/docs`, board));
  }

  // Inhalt eines Dokuments mit Hash und Abschnitten
  getDoc(pid: string, path: string, board?: number): Promise<DocFileContentDto> {
    return this.request('GET', withBoard(`/api/projects/${pid}/docs/file?path=${encodeURIComponent(path)}`, board));
  }

  // Ersetzt einen Abschnitt; bei fremder Änderung antwortet der Server mit 409 (doc-changed)
  writeDocSection(pid: string, input: WriteDocSectionInput, board?: number): Promise<DocWriteResultDto> {
    return this.request('PUT', withBoard(`/api/projects/${pid}/docs/section`, board), input);
  }

  // Volltextsuche in den Dokumenten
  searchDocs(pid: string, query: string, board?: number): Promise<SearchHitDto[]> {
    return this.request('GET', withBoard(`/api/projects/${pid}/docs/search?q=${encodeURIComponent(query)}`, board));
  }

  // Ticket-Entwürfe aus markierten Stellen eines Dokuments (legt noch nichts an)
  createTicketDrafts(pid: string, input: TicketDraftInput, board?: number): Promise<TicketDraftResultDto> {
    return this.request('POST', withBoard(`/api/projects/${pid}/docs/ticket-drafts`, board), input);
  }

  // Bricht die laufende Erzeugung von Ticket-Entwürfen des Projekts ab (beendet Claude)
  cancelTicketDrafts(pid: string): Promise<{ cancelled: boolean }> {
    return this.request('DELETE', `/api/projects/${pid}/docs/ticket-drafts`, {});
  }

  // Gespeicherte Entwürfe eines Dokuments (die zuletzt erzeugten, samt Änderungen aus dem Dialog); null ohne
  getSavedTicketDrafts(pid: string, path: string, board?: number): Promise<{ result: TicketDraftResultDto | null }> {
    return this.request('GET', withBoard(`/api/projects/${pid}/docs/ticket-drafts/saved?path=${encodeURIComponent(path)}`, board));
  }

  // Speichert die im Dialog bearbeiteten Entwürfe eines Dokuments
  saveTicketDrafts(pid: string, path: string, drafts: TicketDraftDto[], board?: number): Promise<{ result: TicketDraftResultDto }> {
    return this.request('PUT', withBoard(`/api/projects/${pid}/docs/ticket-drafts/saved`, board), { path, drafts });
  }

  // Verwirft die gespeicherten Entwürfe eines Dokuments
  discardTicketDrafts(pid: string, path: string, board?: number): Promise<{ ok: boolean }> {
    return this.request('DELETE', withBoard(`/api/projects/${pid}/docs/ticket-drafts/saved?path=${encodeURIComponent(path)}`, board), {});
  }

  // Legt geprüfte Entwürfe als Tickets auf dem Board des Projekts (oder dem genannten Board) an
  createDocTickets(pid: string, input: DocTicketInput, board?: number): Promise<DocTicketResultDto> {
    return this.request('POST', withBoard(`/api/projects/${pid}/docs/tickets`, board), input);
  }

  // Lässt Claude die offenen Fragen eines Dokuments herauslesen (legt noch nichts an)
  createQuestionDrafts(pid: string, path: string, board?: number): Promise<QuestionDraftResultDto> {
    return this.request('POST', withBoard(`/api/projects/${pid}/docs/question-drafts`, board), { path });
  }

  // Bricht die laufende Erzeugung von Fragen-Entwürfen des Projekts ab (beendet Claude)
  cancelQuestionDrafts(pid: string): Promise<{ cancelled: boolean }> {
    return this.request('DELETE', `/api/projects/${pid}/docs/question-drafts`, {});
  }

  // Legt die gewählten Entwürfe als Fragen im Projekt des Boards an
  createDocQuestions(pid: string, path: string, drafts: QuestionDraftInputDto[], board?: number): Promise<DocQuestionResultDto> {
    return this.request('POST', withBoard(`/api/projects/${pid}/docs/questions`, board), { path, drafts });
  }

  // Öffnet das Dokument auf diesem Rechner im Explorer (Datei markiert) oder in VS Code
  openDoc(pid: string, path: string, target: 'explorer' | 'vscode', board?: number): Promise<{ ok: true }> {
    return this.request('POST', withBoard(`/api/projects/${pid}/docs/open`, board), { path, target });
  }

  // Tickets, die aus diesem Dokument entstanden sind, samt Überschrift und Zeilen ihrer Quelle
  getDocTickets(pid: string, path: string, board?: number): Promise<DocTicketRefDto[]> {
    return this.request('GET', withBoard(`/api/projects/${pid}/docs/tickets?path=${encodeURIComponent(path)}`, board));
  }

  // Lässt Claude einen markierten Bereich umschreiben; geschrieben wird erst über writeDocSection
  reviseDoc(pid: string, input: ReviseDocInput, board?: number): Promise<ReviseDocResultDto> {
    return this.request('POST', withBoard(`/api/projects/${pid}/docs/revise`, board), input);
  }

  // Kommentare zu einem Dokument
  getDocComments(pid: string, path: string, board?: number): Promise<DocCommentDto[]> {
    return this.request('GET', withBoard(`/api/projects/${pid}/docs/comments?path=${encodeURIComponent(path)}`, board));
  }

  // Legt einen Kommentar zu einer markierten Stelle an
  addDocComment(pid: string, path: string, input: NewDocCommentInput, board?: number): Promise<DocCommentDto> {
    return this.request('POST', withBoard(`/api/projects/${pid}/docs/comments?path=${encodeURIComponent(path)}`, board), input);
  }

  // Ändert Text oder Status eines Kommentars
  patchDocComment(pid: string, path: string, id: string, patch: DocCommentPatch, board?: number): Promise<DocCommentDto> {
    return this.request('PATCH', withBoard(`/api/projects/${pid}/docs/comments/${encodeURIComponent(id)}?path=${encodeURIComponent(path)}`, board), patch);
  }

  // Löscht einen Kommentar (leerer Body, sonst lehnt der Server ab)
  async deleteDocComment(pid: string, path: string, id: string, board?: number): Promise<void> {
    await this.request('DELETE', withBoard(`/api/projects/${pid}/docs/comments/${encodeURIComponent(id)}?path=${encodeURIComponent(path)}`, board), {});
  }

  // Projektangaben und Token
  getSettings(pid: string): Promise<SettingsDto> {
    return this.request('GET', `/api/projects/${pid}/settings`);
  }

  // Sessions eines Projekts (letzte 7 Tage)
  getSessions(pid: string): Promise<SessionDto[]> {
    return this.request('GET', `/api/projects/${pid}/sessions`);
  }

  // Details einer Session: Kopf, Tokens, Kosten, Kontext, Todos und bei Nexus-Läufen die Laufdaten
  getSessionDetail(pid: string, sid: string): Promise<SessionDetail> {
    return this.request('GET', `/api/projects/${pid}/sessions/${encodeURIComponent(sid)}/detail`);
  }

  // Verlauf einer Session in Seiten; ohne `after` von Anfang an, sonst ab dem Cursor der letzten Seite
  getSessionTranscript(pid: string, sid: string, after?: string | null, limit?: number): Promise<TranscriptPage> {
    const query = new URLSearchParams();
    if (after) query.set('after', after);
    if (limit !== undefined) query.set('limit', String(limit));
    const qs = query.toString();
    return this.request('GET', `/api/projects/${pid}/sessions/${encodeURIComponent(sid)}/transcript${qs ? `?${qs}` : ''}`);
  }

  // Geänderte Dateien einer Session (aus Git oder dem Transcript)
  getSessionChanges(pid: string, sid: string): Promise<SessionChanges> {
    return this.request('GET', `/api/projects/${pid}/sessions/${encodeURIComponent(sid)}/changes`);
  }

  // Diff einer geänderten Datei der Session
  getSessionFileDiff(pid: string, sid: string, path: string): Promise<FileDiff> {
    return this.request('GET', `/api/projects/${pid}/sessions/${encodeURIComponent(sid)}/changes/file?path=${encodeURIComponent(path)}`);
  }

  // Favoriten eines Projekts
  getFavorites(pid: string): Promise<FavoriteDto[]> {
    return this.request('GET', `/api/projects/${pid}/favorites`);
  }

  // Markiert eine Session als Favorit oder ändert die Notiz
  markFavorite(pid: string, sessionId: string, note?: string): Promise<FavoriteDto> {
    return this.request('PUT', `/api/projects/${pid}/favorites/${encodeURIComponent(sessionId)}`, note === undefined ? {} : { note });
  }

  // Entfernt die Markierung
  async unmarkFavorite(pid: string, sessionId: string): Promise<void> {
    await this.request('DELETE', `/api/projects/${pid}/favorites/${encodeURIComponent(sessionId)}`, {});
  }

  // Angewendeter Workflow (eigener oder Standard) für die Anleitung
  getWorkflow(): Promise<WorkflowDto> {
    return this.request('GET', '/api/workflow');
  }

  // Übersicht der Anpassungen des Devs (gilt für alle Projekte)
  getCustomizations(): Promise<CustomizationsDto> {
    return this.request('GET', '/api/customizations');
  }

  // Benutzerweite Einstellungen
  getUserSettings(): Promise<UserSettingsDto> {
    return this.request('GET', '/api/user-settings');
  }

  // Speichert die benutzerweiten Einstellungen
  saveUserSettings(settings: Partial<UserSettingsDto>): Promise<UserSettingsDto> {
    return this.request('PUT', '/api/user-settings', settings);
  }

  // Einstellung des Telegram-Bots; Geheimnisse kommen nur als Maske zurück
  getTelegramConfig(): Promise<TelegramConfig> {
    return this.request('GET', '/api/telegram/config');
  }

  // Speichert die Einstellung; ein Geheimnis in Maskenform bleibt unverändert, "" löscht es
  saveTelegramConfig(config: TelegramConfigInput): Promise<TelegramConfig> {
    return this.request('PUT', '/api/telegram/config', config);
  }

  // Stand des Pollers samt Zahl der gekoppelten Chats und offenen Codes
  getTelegramStatus(): Promise<TelegramStatusDto> {
    return this.request('GET', '/api/telegram/status');
  }

  // Gibt einen Einmal-Code samt Link in den Chat aus
  pairTelegram(): Promise<TelegramPairingDto> {
    return this.request('POST', '/api/telegram/pair');
  }

  // Nimmt einen Chat aus der Kopplung
  unpairTelegramChat(chatId: number): Promise<{ ok: boolean }> {
    return this.request('DELETE', `/api/telegram/chats/${chatId}`);
  }

  // Schickt die Testnachricht an alle gekoppelten Chats
  testTelegram(): Promise<TelegramTestDto> {
    return this.request('POST', '/api/telegram/test');
  }

  // Richtet Profilbild, Name, Beschreibungen und Befehlsliste des Bots neu ein
  applyTelegramProfile(): Promise<TelegramProfileDto> {
    return this.request('POST', '/api/telegram/profile');
  }

  // Probiert die eingerichtete Spracherkennung mit dem mitgelieferten Beispiel aus
  testSpeech(): Promise<SpeechTestDto> {
    return this.request('POST', '/api/speech/test');
  }

  // Wann die Spracherkennung zuletzt geklappt hat und woran sie zuletzt gescheitert ist
  getSpeechStatus(): Promise<SpeechStatusDto> {
    return this.request('GET', '/api/speech/status');
  }

  getPluginSetup(): Promise<PluginSetupDto> {
    return this.request('GET', '/api/setup/plugin');
  }

  installPlugin(): Promise<PluginInstallDto> {
    return this.request('POST', '/api/setup/plugin', {});
  }

  getBoardMigration(id: number): Promise<BoardMigrationDto> {
    return this.request('GET', `/api/db/boards/${id}/migration`);
  }

  migrateBoard(id: number): Promise<BoardMigrationResultDto> {
    return this.request('POST', `/api/db/boards/${id}/migration`, {});
  }

  // Zeigt eine Beispielmeldung als Windows-Benachrichtigung
  sendTestNotification(): Promise<TestNotificationDto> {
    return this.request('POST', '/api/user-settings/test-notification', {});
  }

  // Schickt eine Beispielmeldung an das eingetragene ntfy-Thema
  sendTestNtfy(): Promise<TestNotificationDto> {
    return this.request('POST', '/api/user-settings/test-ntfy', {});
  }

  // Verbindung zur Board-Datenbank (ohne Passwort)
  getBoardConfig(): Promise<BoardConfigDto> {
    return this.request('GET', '/api/board-config');
  }

  // Speichert die Verbindung; ohne Passwort bleibt das gespeicherte
  saveBoardConfig(input: BoardConfigInput): Promise<BoardConfigDto> {
    return this.request('PUT', '/api/board-config', input);
  }

  // Prüft die Verbindung, ohne zu speichern
  testBoardConfig(input: BoardConfigInput): Promise<BoardConfigTestDto> {
    return this.request('POST', '/api/board-config/test', input);
  }

  // Benutzer und aktives Board der Board-Datenbank
  getDbActive(): Promise<DbActiveDto> {
    return this.request('GET', '/api/db/active');
  }

  // Alle Projekte der Board-Datenbank
  getDbProjects(): Promise<DbProjectDto[]> {
    return this.request('GET', '/api/db/projects');
  }

  // Legt ein Projekt an (Key wird vom Server in Großbuchstaben geführt)
  createDbProject(key: string, name: string): Promise<DbProjectDto> {
    return this.request('POST', '/api/db/projects', { key, name });
  }

  // Benennt ein Projekt um
  renameDbProject(key: string, name: string): Promise<DbProjectDto> {
    return this.request('PATCH', `/api/db/projects/${encodeURIComponent(key)}`, { name });
  }

  // Auswertung der Dispatcher-Läufe eines Projekts (Board, Zeitraum, Lauf-Art)
  getDispatchStats(filter: DispatchStatsFilter): Promise<DispatchStatsDto> {
    const query = new URLSearchParams({ project: filter.project });
    if (filter.board) query.set('board', String(filter.board));
    if (filter.from) query.set('from', filter.from);
    if (filter.to) query.set('to', filter.to);
    if (filter.kind) query.set('kind', filter.kind);
    return this.request('GET', `/api/db/stats/dispatch?${query}`);
  }

  // Boards, optional nur eines Projekts
  getDbBoards(project?: string): Promise<DbBoardDto[]> {
    return this.request('GET', project ? `/api/db/boards?project=${encodeURIComponent(project)}` : '/api/db/boards');
  }

  // Legt ein Board im Projekt an
  createDbBoard(project: string, name: string): Promise<DbBoardDto> {
    return this.request('POST', '/api/db/boards', { project, name });
  }

  // Ändert Name und/oder Repo-Pfad eines Boards
  patchDbBoard(id: number, patch: DbBoardPatch): Promise<DbBoardDto> {
    return this.request('PATCH', `/api/db/boards/${id}`, patch);
  }

  // Öffnet den Ordnerdialog auf dem Rechner des Servers (= dieser Rechner); path ist null, wenn abgebrochen wird
  pickDbFolder(start?: string): Promise<{ path: string | null }> {
    return this.request('POST', '/api/db/pick-folder', start ? { start } : {});
  }

  // Löscht ein Board; offene Tickets nur mit deleteOpen
  deleteDbBoard(id: number, deleteOpen: boolean): Promise<{ deletedTickets: string[] }> {
    return this.request('DELETE', `/api/db/boards/${id}?deleteOpen=${deleteOpen}`, {});
  }

  // Macht ein Board zum aktiven (board.yaml, gemeinsam mit CLI und Agenten)
  activateDbBoard(id: number): Promise<DbActiveDto> {
    return this.request('POST', `/api/db/boards/${id}/activate`, {});
  }

  // Spalten eines Boards mit Karten
  getDbBoardView(id: number): Promise<DbBoardViewDto> {
    return this.request('GET', `/api/db/boards/${id}/view`);
  }

  // Branches des Board-Repos als Vorschläge für den Zielbranch
  getDbBranches(boardId: number): Promise<DbBranchListDto> {
    return this.request('GET', `/api/db/boards/${boardId}/branches`);
  }

  // Legt einen neuen Zielbranch im Board-Repo lokal an
  createDbBranch(boardId: number, name: string): Promise<DbCreatedBranchDto> {
    return this.request('POST', `/api/db/boards/${boardId}/branches`, { name });
  }

  // Abhängigkeitsgraph eines Tickets, einer Spec oder eines Boards; done: Erledigte zeigen (ohne Angabe: Standard des Umfangs)
  getDbGraph(scope: DbGraphScope, done?: boolean): Promise<DbGraphDto> {
    const query = new URLSearchParams();
    if (scope.kind === 'ticket') query.set('ticket', scope.id);
    else if (scope.kind === 'board') query.set('board', String(scope.boardId));
    else {
      query.set('project', scope.project);
      query.set('spec', scope.spec);
    }
    if (done !== undefined) query.set('done', done ? '1' : '0');
    return this.request('GET', `/api/db/graph?${query}`);
  }

  // Fortschritt der Specs, die auf dem Board vorkommen (projektweit gezählt)
  getDbSpecProgress(boardId: number): Promise<DbSpecProgressDto[]> {
    return this.request('GET', `/api/db/boards/${boardId}/spec-progress`);
  }

  // Fortschritt der Specs aus dem Board, das zum Repo des Projekts gehört (leer ohne Board)
  getSpecProgress(pid: string): Promise<DbSpecProgressDto[]> {
    return this.request('GET', `/api/projects/${pid}/spec-progress`);
  }

  // Legt ein Ticket auf dem Board an
  createDbTicket(boardId: number, input: DbNewTicketInput): Promise<DbTicketDto> {
    return this.request('POST', `/api/db/boards/${boardId}/tickets`, input);
  }

  // Ticket mit Kommentaren und Verlauf
  getDbTicket(id: string): Promise<DbTicketDetailDto> {
    return this.request('GET', `/api/db/tickets/${encodeURIComponent(id)}`);
  }

  // Zeitleiste eines Tickets: Verlauf, Kommentare, Fragen, Läufe und Commits in Zeitfolge
  getDbTicketTimeline(id: string): Promise<DbTicketTimelineDto> {
    return this.request('GET', `/api/db/tickets/${encodeURIComponent(id)}/timeline`);
  }

  // Suche über alle Boards; kinds leer = alle Arten
  searchDb(q: string, options: { kinds?: DbSearchKind[]; limit?: number } = {}): Promise<DbSearchResultDto> {
    const params = new URLSearchParams({ q });
    if (options.kinds?.length) params.set('kinds', options.kinds.join(','));
    if (options.limit !== undefined) params.set('limit', String(options.limit));
    return this.request('GET', `/api/db/search?${params.toString()}`);
  }

  // „Meine Aufgaben“ über alle eigenen Boards (oder ein eigenes Board), in der gespeicherten Reihenfolge
  getInbox(boardId?: number | null): Promise<InboxListDto> {
    return this.request('GET', boardId ? `/api/db/inbox?board=${boardId}` : '/api/db/inbox');
  }

  // Speichert die eigene Reihenfolge von „Meine Aufgaben“ (Schlüssel der Einträge, oben zuerst)
  saveInboxOrder(keys: string[]): Promise<{ keys: string[] }> {
    return this.request('PUT', '/api/db/inbox/order', { keys });
  }

  // Setzt die eigene Reihenfolge zurück (Standard: nach Dringlichkeit)
  resetInboxOrder(): Promise<{ keys: string[] }> {
    return this.request('DELETE', '/api/db/inbox/order', {});
  }

  // Ändert ein Ticket; veraltete Version → ApiError „conflict“ mit `current`
  patchDbTicket(id: string, patch: DbTicketPatch): Promise<DbTicketDetailDto> {
    return this.request('PATCH', `/api/db/tickets/${encodeURIComponent(id)}`, patch);
  }

  // Löscht ein Ticket endgültig (leerer Body, sonst lehnt der Server ab)
  async deleteDbTicket(id: string): Promise<void> {
    await this.request('DELETE', `/api/db/tickets/${encodeURIComponent(id)}`, {});
  }

  // Verschiebt ein Ticket in eine Spalte
  moveDbTicket(id: string, input: DbMoveInput): Promise<DbTicketDto> {
    return this.request('POST', `/api/db/tickets/${encodeURIComponent(id)}/move`, input);
  }

  // Holt Tickets von anderen Boards desselben Projekts auf dieses Board (alles oder nichts)
  importDbTickets(boardId: number, ticketIds: string[]): Promise<DbImportResult> {
    return this.request('POST', `/api/db/boards/${boardId}/import`, { ticketIds });
  }

  // Hängt einen Kommentar an
  addDbComment(id: string, body: string): Promise<DbTicketDetailDto> {
    return this.request('POST', `/api/db/tickets/${encodeURIComponent(id)}/comments`, { body });
  }

  // Hakt einen Schritt ab (Nummer ab 1) oder nimmt den Haken zurück; liefert den neuen Stand des Tickets
  // expectText (alle Schritt-Aufrufe): bisheriger Text des Schritts; steht dort inzwischen etwas anderes, antwortet der Server mit 409
  setDbStep(id: string, number: number, done: boolean, expectText?: string): Promise<DbTicketDetailDto> {
    return this.request('POST', `/api/db/tickets/${encodeURIComponent(id)}/steps/${number}`, { done, expectText });
  }

  // Legt einen Schritt an (ans Ende oder an Nummer at); liefert den neuen Stand des Tickets
  addDbStep(id: string, input: DbStepInput): Promise<DbTicketDetailDto> {
    return this.request('POST', `/api/db/tickets/${encodeURIComponent(id)}/steps`, input);
  }

  // Ändert Text und/oder Anleitung eines Schritts (leere Anleitung entfernt sie)
  editDbStep(id: string, number: number, change: DbStepChange, expectText?: string): Promise<DbTicketDetailDto> {
    return this.request('PATCH', `/api/db/tickets/${encodeURIComponent(id)}/steps/${number}`, { ...change, expectText });
  }

  // Löscht einen Schritt; die folgenden rücken nach
  removeDbStep(id: string, number: number, expectText?: string): Promise<DbTicketDetailDto> {
    return this.request('DELETE', `/api/db/tickets/${encodeURIComponent(id)}/steps/${number}`, { expectText });
  }

  // Verschiebt einen Schritt auf Nummer to
  moveDbStep(id: string, number: number, to: number, expectText?: string): Promise<DbTicketDetailDto> {
    return this.request('POST', `/api/db/tickets/${encodeURIComponent(id)}/steps/${number}/move`, { to, expectText });
  }

  // Notizen eines Boards nach Zustand
  getDbNotes(boardId: number, state: DbNoteFilter = 'open'): Promise<DbNoteDto[]> {
    return this.request('GET', `/api/db/boards/${boardId}/notes?state=${state}`);
  }

  // Legt eine Notiz an
  addDbNote(boardId: number, input: DbNewNoteInput): Promise<DbNoteDto> {
    return this.request('POST', `/api/db/boards/${boardId}/notes`, input);
  }

  // Derselbe Befund ist erneut aufgefallen: zählt die Notiz hoch
  seenDbNote(id: number, source?: string): Promise<DbNoteDto> {
    return this.request('POST', `/api/db/notes/${id}/seen`, source ? { source } : {});
  }

  // Macht aus einer Notiz ein Ticket
  takeDbNote(id: number, input: DbNoteTakeInput = {}): Promise<DbNoteTakeResult> {
    return this.request('POST', `/api/db/notes/${id}/take`, input);
  }

  // Verwirft eine Notiz mit Grund
  dropDbNote(id: number, reason: string): Promise<DbNoteDto> {
    return this.request('POST', `/api/db/notes/${id}/drop`, { reason });
  }

  // Fragen eines Board-Projekts mit Zählern je Zustand
  getDbQuestions(projectKey: string): Promise<DbQuestionListDto> {
    return this.request('GET', `/api/db/projects/${encodeURIComponent(projectKey)}/questions`);
  }

  // Detail einer Frage mit Recherchen, Kommentaren und Verlauf
  getDbQuestion(id: string): Promise<DbQuestionDetailDto> {
    return this.request('GET', `/api/db/questions/${encodeURIComponent(id)}`);
  }

  // Stellt eine Frage im Board-Projekt (Handelnder: der Mensch aus board.yaml)
  createDbQuestion(projectKey: string, input: DbNewQuestionInput): Promise<DbQuestionDetailDto> {
    return this.request('POST', `/api/db/projects/${encodeURIComponent(projectKey)}/questions`, input);
  }

  // Ändert Titel, Beschreibung, Tags oder Anlass; veraltete Version: ApiError „conflict“ mit aktuellem Stand
  patchDbQuestion(id: string, patch: DbQuestionPatch): Promise<DbQuestionDetailDto> {
    return this.request('PATCH', `/api/db/questions/${encodeURIComponent(id)}`, patch);
  }

  // Ersetzt die betroffenen Tickets
  putDbQuestionTickets(id: string, rev: number, ticketIds: string[]): Promise<DbQuestionDetailDto> {
    return this.request('PUT', `/api/db/questions/${encodeURIComponent(id)}/tickets`, { rev, ticketIds });
  }

  // Ersetzt die Abhängigkeiten (Fragen oder Tickets; Kreis: 422 „dependency-cycle“)
  putDbQuestionDepends(id: string, rev: number, dependsOn: string[]): Promise<DbQuestionDetailDto> {
    return this.request('PUT', `/api/db/questions/${encodeURIComponent(id)}/depends`, { rev, dependsOn });
  }

  // Hängt einen Kommentar an eine Frage
  addDbQuestionComment(id: string, body: string): Promise<DbQuestionDetailDto> {
    return this.request('POST', `/api/db/questions/${encodeURIComponent(id)}/comments`, { body });
  }

  // Beantwortet mit Option oder Freitext und Begründung
  decideDbQuestion(id: string, input: DbDecisionInput): Promise<DbQuestionDetailDto> {
    return this.request('POST', `/api/db/questions/${encodeURIComponent(id)}/decision`, input);
  }

  // Verwirft mit Begründung
  discardDbQuestion(id: string, rev: number, reason: string): Promise<DbQuestionDetailDto> {
    return this.request('POST', `/api/db/questions/${encodeURIComponent(id)}/discard`, { rev, reason });
  }

  // Öffnet wieder, auf Wunsch mit Grund
  reopenDbQuestion(id: string, rev: number, reason?: string): Promise<DbQuestionDetailDto> {
    return this.request('POST', `/api/db/questions/${encodeURIComponent(id)}/reopen`, reason ? { rev, reason } : { rev });
  }

  // Öffnet den Ereignisstrom eines Projekts; dbBoard meldet zusätzlich Änderungen dieses DB-Boards, dbProject Änderungen an dessen Fragen;
  // notify: der Tab zeigt Benachrichtigungen selbst (statt des Windows-Toasts)
  openEvents(pid: string, dbBoard: number | null = null, dbProject: number | null = null, notify = false): EventSource {
    // EventSource kann keine Köpfe setzen; in der App geht das Token deshalb in die Adresse
    const query = [dbBoard ? `dbBoard=${dbBoard}` : '', dbProject ? `dbProject=${dbProject}` : '', notify ? 'notify=1' : '', this.token ? `token=${encodeURIComponent(this.token)}` : '']
      .filter(Boolean).join('&');
    return new EventSource(`${this.base}/api/projects/${pid}/events${query ? `?${query}` : ''}`);
  }

  // Einstellung des Dispatchers (ohne Datei: Beispiel; kaputte Datei: Beispiel mit Fehler)
  getDispatchConfig(): Promise<DispatchConfigViewDto> {
    return this.request('GET', '/api/dispatch/config');
  }

  // Speichert die Einstellung; ungültig → ApiError „dispatch-config-invalid“
  saveDispatchConfig(raw: RawDispatchConfigDto): Promise<DispatchConfigViewDto> {
    return this.request('PUT', '/api/dispatch/config', raw);
  }

  // Vorschau einer (noch ungespeicherten) Vorlage; mit id die Werte dieses Tickets bzw. dieser Frage, sonst Beispielwerte.
  // Ungültige Vorlage → ApiError „dispatch-prompt-invalid“ mit details.missing/unknown
  dispatchPromptPreview(kind: DispatchPromptKindDto, template?: string | null, id?: string): Promise<DispatchPromptPreviewDto> {
    return this.request('POST', '/api/dispatch/prompts/preview', { kind, template: template ?? null, ...(id ? { id } : {}) });
  }

  // Optimierungs-Vorschläge zur Dispatcher-Einstellung (Läufe aller Projekte)
  getDispatchSuggestions(): Promise<SuggestionListDto> {
    return this.request('GET', '/api/dispatch/suggestions');
  }

  // Übernimmt einen Vorschlag; Einstellung inzwischen geändert → ApiError „conflict“, Vorschlag weg → „suggestion-not-found“
  applyDispatchSuggestion(id: string, configHash: string): Promise<SuggestionApplyDto> {
    return this.request('POST', `/api/dispatch/suggestions/${encodeURIComponent(id)}/apply`, { configHash });
  }

  dismissDispatchSuggestion(id: string): Promise<SuggestionListDto> {
    return this.request('POST', `/api/dispatch/suggestions/${encodeURIComponent(id)}/dismiss`, {});
  }

  restoreDispatchSuggestion(id: string): Promise<SuggestionListDto> {
    return this.request('DELETE', `/api/dispatch/suggestions/${encodeURIComponent(id)}/dismiss`, {});
  }

  // Claude-Analyse (bis zu 5 Minuten); läuft schon eine → ApiError „conflict“
  analyzeDispatchSuggestions(): Promise<SuggestionListDto> {
    return this.request('POST', '/api/dispatch/suggestions/analyze', {});
  }

  // Zustand des Dispatchers; mit Board die Bereitschaft seiner Geplant-Tickets
  getDispatchStatus(boardId?: number): Promise<DispatchStatusDto> {
    return this.request('GET', `/api/dispatch/status${boardId ? `?boardId=${boardId}` : ''}`);
  }

  // Sofort einen Durchlauf ausführen
  runDispatch(boardId?: number): Promise<DispatchStatusDto> {
    return this.request('POST', '/api/dispatch/run', this.withBoard({}, boardId));
  }

  // Neue Starts pausieren
  pauseDispatch(minutes: number, boardId?: number): Promise<DispatchStatusDto> {
    return this.request('POST', '/api/dispatch/pause', this.withBoard({ minutes }, boardId));
  }

  // Pause aufheben
  resumeDispatch(boardId?: number): Promise<DispatchStatusDto> {
    return this.request('POST', '/api/dispatch/resume', this.withBoard({}, boardId));
  }

  // Ein- oder ausschalten
  setDispatchEnabled(on: boolean, boardId?: number): Promise<DispatchStatusDto> {
    return this.request('POST', `/api/dispatch/${on ? 'enable' : 'disable'}`, this.withBoard({}, boardId));
  }

  // „Erneut mergen“ für ein Ticket mit Grund „Merge: …“ (oder Erledigt mit Branch); liefert auch das Ergebnis für das Ticket
  retryDispatchMerge(id: string, boardId?: number): Promise<DispatchMergeResultDto> {
    return this.request('POST', `/api/dispatch/merge/${encodeURIComponent(id)}`, this.withBoard({}, boardId));
  }

  // „Abnehmen“ eines Tickets mit Grund „Human Interaction: …“: Erledigt und auf eigenem Board Merge; liefert auch das Ergebnis
  acceptDispatchTicket(id: string, boardId?: number): Promise<DispatchMergeResultDto> {
    return this.request('POST', `/api/dispatch/accept/${encodeURIComponent(id)}`, this.withBoard({}, boardId));
  }

  // Review vor dem Merge: Dateien des Ticket-Branches gegen das Ziel und Testläufe des Agenten
  getDispatchReview(id: string): Promise<DispatchReviewDto> {
    return this.request('GET', `/api/dispatch/review/${encodeURIComponent(id)}`);
  }

  // Patch einer Datei aus dem Review
  getDispatchReviewFile(id: string, path: string): Promise<DispatchReviewPatchDto> {
    return this.request('GET', `/api/dispatch/review/${encodeURIComponent(id)}/file?path=${encodeURIComponent(path)}`);
  }

  // „Zurück an den Agenten“ mit Rückmeldung: Ticket nach Geplant, der nächste Lauf setzt im selben Worktree fort
  returnDispatchTicket(id: string, comment: string, boardId?: number): Promise<DispatchMergeResultDto> {
    return this.request('POST', `/api/dispatch/return/${encodeURIComponent(id)}`, this.withBoard({ comment }, boardId));
  }

  // Läuft für die Frage gerade eine Recherche des Dispatchers?
  getQuestionResearch(id: string): Promise<QuestionResearchStatusDto> {
    return this.request('GET', `/api/dispatch/research/${encodeURIComponent(id)}`);
  }

  // „Recherche starten“: lässt die offene Frage sofort von einer Claude-Session recherchieren
  startQuestionResearch(id: string): Promise<DispatchResearchStartDto> {
    return this.request('POST', `/api/dispatch/research/${encodeURIComponent(id)}`, {});
  }

  // Läuft für das Ticket gerade eine Planung des Dispatchers?
  getTicketPlanning(id: string): Promise<TicketPlanningStatusDto> {
    return this.request('GET', `/api/dispatch/plan/${encodeURIComponent(id)}`);
  }

  // „Planen“: lässt das Ticket sofort von einer Claude-Session planen
  startTicketPlanning(id: string, boardId?: number): Promise<DispatchResearchStartDto> {
    return this.request('POST', `/api/dispatch/plan/${encodeURIComponent(id)}`, this.withBoard({}, boardId));
  }

  // Läufe eines Tickets oder einer Frage (neueste zuerst, mit „läuft“)
  getDispatchRuns(ticketId: string): Promise<DispatchRunLogDto[]> {
    return this.request('GET', `/api/dispatch/runs?ticket=${encodeURIComponent(ticketId)}`);
  }

  // Lauf-Protokoll zerlegt in Kopf, Arbeitsanweisung und Ausgabe
  getDispatchRunLog(name: string): Promise<DispatchRunLogPartsDto> {
    return this.request('GET', `/api/dispatch/runs/${encodeURIComponent(name)}`);
  }

  // Protokolldateien (dispatch.log zuerst, dann neueste Läufe)
  getDispatchLogs(): Promise<string[]> {
    return this.request('GET', '/api/dispatch/logs');
  }

  // Inhalt eines Protokolls als Text
  async getDispatchLog(name: string): Promise<string> {
    let res: Response;
    try {
      res = await fetch(`${this.base}/api/dispatch/logs/${encodeURIComponent(name)}`, { method: 'GET', headers: this.headers() });
    } catch {
      throw new ApiError('unreachable', 'Server nicht erreichbar', 0);
    }
    if (!res.ok) {
      const data = await res.json().catch(() => null);
      throw new ApiError(data?.code ?? 'error', data?.message ?? 'Unbekannter Fehler', res.status);
    }
    return res.text();
  }

  // Body mit Board, falls eins angegeben ist
  private withBoard(body: Record<string, unknown>, boardId?: number): Record<string, unknown> {
    return boardId ? { ...body, boardId } : body;
  }

  // Führt eine Anfrage aus. Gleiche GETs kurz hintereinander (mehrere Komponenten laden beim Aufbau dasselbe)
  // teilen sich eine Anfrage; jeder Aufrufer bekommt eine eigene Kopie. Schreibende Anfragen beenden das Teilen,
  // damit kein späteres GET einen Stand von vor der Änderung erhält.
  protected async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    if (method !== 'GET') {
      this.shared.clear();
      try {
        return (await this.send(method, path, body)) as T;
      } finally {
        this.shared.clear();
      }
    }
    const now = Date.now();
    let entry = this.shared.get(path);
    if (!entry || now - entry.startedAt > SHARE_WINDOW_MS) {
      // Eintrag schon beim Abschluss entfernen, bevor die Aufrufer weiterlaufen: ein GET danach fragt neu
      const own: { startedAt: number; promise: Promise<unknown> } = { startedAt: now, promise: Promise.resolve() };
      own.promise = this.send('GET', path).finally(() => {
        if (this.shared.get(path) === own) this.shared.delete(path);
      });
      this.shared.set(path, own);
      entry = own;
    }
    return structuredClone(await entry.promise) as T;
  }

  private readonly shared = new Map<string, { startedAt: number; promise: Promise<unknown> }>();

  // Köpfe, die jede Anfrage trägt: in der App das Start-Token, im Browser nichts
  private headers(): Record<string, string> {
    return this.token ? { 'x-nexus-token': this.token } : {};
  }

  // Schickt die Anfrage und übersetzt Fehlerantworten in ApiError
  private async send(method: string, path: string, body?: unknown): Promise<unknown> {
    const headers: Record<string, string> = this.headers();
    if (method !== 'GET') {
      headers['content-type'] = 'application/json';
      headers['x-harness'] = '1';
    }
    let res: Response;
    try {
      res = await fetch(this.base + path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
    } catch {
      throw new ApiError('unreachable', 'Server nicht erreichbar', 0);
    }
    const data = await res.json().catch(() => null);
    if (!res.ok) {
      throw new ApiError(data?.code ?? 'error', data?.message ?? 'Unbekannter Fehler', res.status, data?.current, data?.details);
    }
    return data;
  }
}
