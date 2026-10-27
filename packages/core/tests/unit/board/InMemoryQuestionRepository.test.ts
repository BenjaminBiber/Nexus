import { InMemoryBoardDatabase } from '../../../src/board/InMemoryBoardDatabase.js';
import { questionRepositoryContract } from './helpers/QuestionRepositoryContract.js';

questionRepositoryContract('In-Memory', async () => new InMemoryBoardDatabase());
