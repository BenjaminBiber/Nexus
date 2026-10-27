import { InMemoryBoardDatabase } from '../../../src/board/InMemoryBoardDatabase.js';
import { searchContract } from './helpers/SearchContract.js';

searchContract('In-Memory', async () => new InMemoryBoardDatabase());
