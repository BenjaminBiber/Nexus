import { InMemoryBoardDatabase } from '../../../src/board/InMemoryBoardDatabase.js';
import { boardDatabaseContract } from './helpers/BoardDatabaseContract.js';
import { dispatchRunContract } from './helpers/DispatchRunContract.js';

boardDatabaseContract('In-Memory', async () => new InMemoryBoardDatabase());
dispatchRunContract('In-Memory', async () => new InMemoryBoardDatabase());
