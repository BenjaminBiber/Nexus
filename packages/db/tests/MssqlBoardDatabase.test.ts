import { MssqlBoardDatabase } from '../src/MssqlBoardDatabase.js';
import { boardDatabaseContract } from '../../core/tests/unit/board/helpers/BoardDatabaseContract.js';
import { dispatchRunContract } from '../../core/tests/unit/board/helpers/DispatchRunContract.js';
import { TestDb } from './helpers/TestDb.js';

const config = TestDb.config();

TestDb.describe('SQL Server', () => {
  boardDatabaseContract('SQL Server', () => MssqlBoardDatabase.connect(config!), () => TestDb.reset());
  dispatchRunContract('SQL Server', () => MssqlBoardDatabase.connect(config!), () => TestDb.reset());
});
