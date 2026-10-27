import type { BoardUnitOfWork } from '@harness/core';
import { MssqlBoardRepository } from './MssqlBoardRepository.js';
import { MssqlCommentRepository } from './MssqlCommentRepository.js';
import { MssqlDeletionRepository } from './MssqlDeletionRepository.js';
import { MssqlDispatchRunRepository } from './MssqlDispatchRunRepository.js';
import { MssqlDispatchStatsRepository } from './MssqlDispatchStatsRepository.js';
import { MssqlHistoryRepository } from './MssqlHistoryRepository.js';
import { MssqlNoteRepository } from './MssqlNoteRepository.js';
import { MssqlProjectRepository } from './MssqlProjectRepository.js';
import { MssqlQuestionCommentRepository } from './MssqlQuestionCommentRepository.js';
import { MssqlQuestionHistoryRepository } from './MssqlQuestionHistoryRepository.js';
import { MssqlQuestionRepository } from './MssqlQuestionRepository.js';
import { MssqlQuestionResearchRepository } from './MssqlQuestionResearchRepository.js';
import { MssqlSearchRepository } from './MssqlSearchRepository.js';
import { MssqlTicketRepository } from './MssqlTicketRepository.js';
import { MssqlTicketSessionRepository } from './MssqlTicketSessionRepository.js';
import { MssqlTicketStepRepository } from './MssqlTicketStepRepository.js';
import { MssqlUserRepository } from './MssqlUserRepository.js';
import type { SqlRunner } from './SqlRunner.js';

// Alle Repositories über einer laufenden SQL-Transaktion.
export class MssqlUnitOfWork implements BoardUnitOfWork {
  readonly users: MssqlUserRepository;
  readonly projects: MssqlProjectRepository;
  readonly boards: MssqlBoardRepository;
  readonly tickets: MssqlTicketRepository;
  readonly comments: MssqlCommentRepository;
  readonly steps: MssqlTicketStepRepository;
  readonly ticketSessions: MssqlTicketSessionRepository;
  readonly history: MssqlHistoryRepository;
  readonly notes: MssqlNoteRepository;
  readonly deletions: MssqlDeletionRepository;
  readonly questions: MssqlQuestionRepository;
  readonly questionResearch: MssqlQuestionResearchRepository;
  readonly questionComments: MssqlQuestionCommentRepository;
  readonly questionHistory: MssqlQuestionHistoryRepository;
  readonly search: MssqlSearchRepository;
  readonly dispatchRuns: MssqlDispatchRunRepository;
  readonly dispatchStats: MssqlDispatchStatsRepository;

  // missingFeatures/database nur für die Repositories, die eine additive Erweiterung brauchen (Notizen, Dispatcher-Läufe, Suche)
  constructor(sql: SqlRunner, missingFeatures: string[] = [], database = '') {
    this.users = new MssqlUserRepository(sql);
    this.projects = new MssqlProjectRepository(sql);
    this.boards = new MssqlBoardRepository(sql);
    this.tickets = new MssqlTicketRepository(sql, missingFeatures, database);
    this.comments = new MssqlCommentRepository(sql);
    this.steps = new MssqlTicketStepRepository(sql);
    this.ticketSessions = new MssqlTicketSessionRepository(sql);
    this.history = new MssqlHistoryRepository(sql);
    this.notes = new MssqlNoteRepository(sql, missingFeatures, database);
    this.deletions = new MssqlDeletionRepository(sql);
    this.questions = new MssqlQuestionRepository(sql);
    this.questionResearch = new MssqlQuestionResearchRepository(sql);
    this.questionComments = new MssqlQuestionCommentRepository(sql);
    this.questionHistory = new MssqlQuestionHistoryRepository(sql);
    this.search = new MssqlSearchRepository(sql, missingFeatures);
    this.dispatchRuns = new MssqlDispatchRunRepository(sql, missingFeatures, database);
    this.dispatchStats = new MssqlDispatchStatsRepository(sql);
  }
}
