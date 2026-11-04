//! Detailansicht einer Session: Transcript von Claude Code finden und lesen (Verlauf und
//! Kennzahlen), Kurzinfos der Tools, Änderungen aus Git oder aus dem Transcript.
//! Reine Logik ohne HTTP; der Server verbindet sie mit Sessions, Läufen und Live-Ereignissen.

pub mod git_changes;
pub mod jsonl;
pub mod line_diff;
pub mod session_detail;
pub mod tool_summary;
pub mod transcript_changes;
pub mod transcript_facts;
pub mod transcript_locator;
pub mod transcript_reader;
pub mod types;
pub mod unified_diff;

#[cfg(test)]
mod fixtures;

/// Höchstlänge eines Tool-Ergebnisses in Zeichen
pub const MAX_RESULT_CHARS: usize = 20_000;
/// Höchstzahl der Diff-Zeilen je Datei
pub const MAX_DIFF_LINES: usize = 5_000;
/// Einträge je Abschnitt des Verlaufs ohne `limit`
pub const DEFAULT_PAGE_LIMIT: usize = 500;
/// Hinweis bei Änderungen aus dem Transcript
pub const TRANSCRIPT_NOTE: &str = "Änderungen per Bash sind nicht enthalten.";

pub use git_changes::{GitChanges, GitScope};
pub use session_detail::{resolve_client, run_kind_of, SessionDetailParts, TranscriptScan};
pub use transcript_changes::TranscriptChanges;
pub use transcript_facts::{FactsCollector, TranscriptFacts};
pub use transcript_locator::{encode_cwd, TranscriptLocator};
pub use transcript_reader::{read_page, TranscriptChunk};
pub use types::{
    ChangeOrigin, ChangedFile, ContextUsage, DiffHunk, DiffLine, DiffLineKind, FileDiff,
    FileStatus, RunInfo, SessionChanges, SessionDetail, SystemLevel, TodoItem, TodoStatus,
    TokenTotals, ToolResult, TranscriptEntry, TranscriptInfo, TranscriptPage,
};
