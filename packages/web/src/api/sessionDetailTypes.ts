// Antwortformate der Session-Details (Slide-Over auf der Seite Sessions); die Formen kommen aus ts-rs
// (src/api/generated/sessionDetail.ts). Die Aliase halten die Namen, die die Oberfläche verwendet.
export type {
  ChangeOrigin, ChangedFile, ContextUsage, DiffHunk, DiffLine, DiffLineKind, FileDiff, FileStatus, RunInfo,
  SessionChanges, SessionClient, SessionDetail, SystemLevel, TodoItem, TodoStatus, TokenTotals, ToolResult,
  TranscriptEntry, TranscriptInfo, TranscriptPage,
} from './generated/sessionDetail.js';

import type { ChangedFile, FileStatus, TodoItem, TokenTotals, TranscriptEntry } from './generated/sessionDetail.js';

export type SessionTodo = TodoItem;
export type SessionTokens = TokenTotals;
export type FileChange = ChangedFile;
export type FileChangeStatus = FileStatus;
export type TranscriptEntryKind = TranscriptEntry['kind'];
