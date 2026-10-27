import { MssqlBoardDatabase } from '../src/MssqlBoardDatabase.js';
import { questionRepositoryContract } from '../../core/tests/unit/board/helpers/QuestionRepositoryContract.js';
import { TestDb } from './helpers/TestDb.js';

const config = TestDb.config();

// Läuft nur mit HARNESS_TEST_DB gegen AiHarness_Test (Schema-Version 2 nötig, siehe Task 16)
TestDb.describe('SQL Server: Fragen', () => {
  questionRepositoryContract('SQL Server', () => MssqlBoardDatabase.connect(config!), () => TestDb.reset());
});
