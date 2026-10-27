import type { BoardUnitOfWork } from '@harness/core';
import { SqliteBoardRepository } from './SqliteBoardRepository.js';
import { SqliteCommentRepository } from './SqliteCommentRepository.js';
import { SqliteDeletionRepository } from './SqliteDeletionRepository.js';
import { SqliteDispatchRunRepository } from './SqliteDispatchRunRepository.js';
import { SqliteDispatchStatsRepository } from './SqliteDispatchStatsRepository.js';
import { SqliteHistoryRepository } from './SqliteHistoryRepository.js';
import { SqliteNoteRepository } from './SqliteNoteRepository.js';
import { SqliteProjectRepository } from './SqliteProjectRepository.js';
import { SqliteQuestionCommentRepository } from './SqliteQuestionCommentRepository.js';
import { SqliteQuestionHistoryRepository } from './SqliteQuestionHistoryRepository.js';
import { SqliteQuestionRepository } from './SqliteQuestionRepository.js';
import { SqliteQuestionResearchRepository } from './SqliteQuestionResearchRepository.js';
import { SqliteSearchRepository } from './SqliteSearchRepository.js';
import { SqliteTicketRepository } from './SqliteTicketRepository.js';
import { SqliteTicketSessionRepository } from './SqliteTicketSessionRepository.js';
import { SqliteTicketStepRepository } from './SqliteTicketStepRepository.js';
import { SqliteUserRepository } from './SqliteUserRepository.js';
import type { SqliteRunner } from './SqliteRunner.js';

// Alle Repositories über einer laufenden SQLite-Transaktion.
export class SqliteUnitOfWork implements BoardUnitOfWork {
  readonly users: SqliteUserRepository;
  readonly projects: SqliteProjectRepository;
  readonly boards: SqliteBoardRepository;
  readonly tickets: SqliteTicketRepository;
  readonly comments: SqliteCommentRepository;
  readonly steps: SqliteTicketStepRepository;
  readonly ticketSessions: SqliteTicketSessionRepository;
  readonly history: SqliteHistoryRepository;
  readonly notes: SqliteNoteRepository;
  readonly deletions: SqliteDeletionRepository;
  readonly questions: SqliteQuestionRepository;
  readonly questionResearch: SqliteQuestionResearchRepository;
  readonly questionComments: SqliteQuestionCommentRepository;
  readonly questionHistory: SqliteQuestionHistoryRepository;
  readonly search: SqliteSearchRepository;
  readonly dispatchRuns: SqliteDispatchRunRepository;
  readonly dispatchStats: SqliteDispatchStatsRepository;

  constructor(sql: SqliteRunner) {
    this.users = new SqliteUserRepository(sql);
    this.projects = new SqliteProjectRepository(sql);
    this.boards = new SqliteBoardRepository(sql);
    this.tickets = new SqliteTicketRepository(sql);
    this.comments = new SqliteCommentRepository(sql);
    this.steps = new SqliteTicketStepRepository(sql);
    this.ticketSessions = new SqliteTicketSessionRepository(sql);
    this.history = new SqliteHistoryRepository(sql);
    this.notes = new SqliteNoteRepository(sql);
    this.deletions = new SqliteDeletionRepository(sql);
    this.questions = new SqliteQuestionRepository(sql);
    this.questionResearch = new SqliteQuestionResearchRepository(sql);
    this.questionComments = new SqliteQuestionCommentRepository(sql);
    this.questionHistory = new SqliteQuestionHistoryRepository(sql);
    this.search = new SqliteSearchRepository(sql);
    this.dispatchRuns = new SqliteDispatchRunRepository(sql);
    this.dispatchStats = new SqliteDispatchStatsRepository(sql);
  }
}
