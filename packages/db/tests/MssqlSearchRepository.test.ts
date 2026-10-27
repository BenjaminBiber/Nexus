import { MssqlBoardDatabase } from '../src/MssqlBoardDatabase.js';
import { searchContract } from '../../core/tests/unit/board/helpers/SearchContract.js';
import { TestDb } from './helpers/TestDb.js';

const config = TestDb.config();

// Dieselben Fälle wie im Speicher, gegen AiHarness_Test (seriell über vitest.process.config.ts)
TestDb.describe('SQL Server', () => {
  searchContract('SQL Server', () => MssqlBoardDatabase.connect(config!), () => TestDb.reset());
});
