//! Antwortformate der Detailansicht einer Session: Kopf- und Laufdaten, Verlauf (Transcript)
//! und Änderungen. Vertrag zum Frontend, erzeugt nach `generated/sessionDetail.ts`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::sessions::dto::SessionDto;
use crate::sessions::session::SessionClient;

/// Kopf- und Laufdaten einer Session
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct SessionDetail {
    /// Die Session wie in der Liste
    pub session: SessionDto,
    /// Quelle: Lauf des Dispatchers, sonst Hook, sonst Transcript
    pub client: SessionClient,
    pub transcript: TranscriptInfo,
    /// Zuletzt gemeldeter Branch (Transcript)
    pub branch: Option<String>,
    /// Version von Claude Code (Transcript)
    pub claude_version: Option<String>,
    pub tokens: TokenTotals,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<i64>,
    /// Kontextstand der letzten Antwort (Input samt Cache)
    pub context: Option<ContextUsage>,
    pub todos: Vec<TodoItem>,
    /// Laufdaten des Dispatchers, nur bei Sessions von Nexus
    pub run: Option<RunInfo>,
}

/// Wo das Transcript liegt und ob es lesbar ist
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct TranscriptInfo {
    pub available: bool,
    pub path: Option<String>,
    pub size_bytes: Option<i64>,
}

/// Verbrauchte Tokens der ganzen Session (samt Subagenten)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct TokenTotals {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
}

/// Auslastung des Kontexts nach der letzten Antwort
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct ContextUsage {
    pub used_tokens: i64,
    pub limit_tokens: Option<i64>,
}

/// Stand einer Aufgabe der Todo-Liste
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "sessionDetail.ts")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl TodoStatus {
    /// Der Stand zur Schreibweise von Claude Code; Unbekanntes ergibt nichts
    pub fn from_str_opt(name: &str) -> Option<TodoStatus> {
        match name {
            "pending" => Some(TodoStatus::Pending),
            "in_progress" => Some(TodoStatus::InProgress),
            "completed" => Some(TodoStatus::Completed),
            _ => None,
        }
    }
}

/// Eine Aufgabe der Todo-Liste (TodoWrite bzw. TaskCreate/TaskUpdate)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
}

/// Laufdaten des Dispatchers zu einer Session von Nexus
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct RunInfo {
    /// Name des Lauf-Protokolls (`/api/dispatch/runs/{name}`)
    pub name: String,
    pub kind: Option<String>,
    pub profile: Option<String>,
    pub account: Option<String>,
    pub outcome: Option<String>,
    pub abort_reason: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

/// Ein Abschnitt des Verlaufs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct TranscriptPage {
    pub entries: Vec<TranscriptEntry>,
    /// Byte-Offset als Text, für `?after=`
    pub cursor: String,
    /// Kaputte Zeilen in diesem Abschnitt
    pub skipped: i64,
    pub available: bool,
}

/// Ergebnis eines Tool-Aufrufs, gekürzt auf 20 000 Zeichen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct ToolResult {
    pub text: String,
    pub is_error: bool,
    pub truncated: bool,
}

/// Gewicht eines Systemeintrags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub enum SystemLevel {
    Info,
    Warning,
    Error,
}

/// Ein Eintrag des Verlaufs. Die `id` ist stabil: liefert ein späterer Abschnitt einen
/// Eintrag mit bekannter `id` erneut (etwa einen Tool-Aufruf, dessen Ergebnis inzwischen da
/// ist), ersetzt er den bisherigen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub enum TranscriptEntry {
    /// Prompt des Nutzers
    Prompt {
        id: String,
        at: String,
        text: String,
    },
    /// Antworttext (Markdown)
    Text {
        id: String,
        at: String,
        text: String,
    },
    /// Nachdenken; oft ohne Text, dann nur mit Dauer
    Thinking {
        id: String,
        at: String,
        text: String,
        #[serde(rename = "durationMs")]
        #[ts(rename = "durationMs")]
        duration_ms: Option<i64>,
    },
    /// Tool-Aufruf mit Kurzinfo, Eingabe und (sobald da) Ergebnis
    Tool {
        id: String,
        at: String,
        name: String,
        summary: String,
        #[ts(type = "unknown")]
        input: serde_json::Value,
        result: Option<ToolResult>,
    },
    /// Hinweis des Systems (Hook-Fehler, Kompaktierung, API-Fehler …)
    System {
        id: String,
        at: String,
        text: String,
        level: SystemLevel,
    },
}

impl TranscriptEntry {
    /// Die stabile Kennung des Eintrags
    pub fn id(&self) -> &str {
        match self {
            TranscriptEntry::Prompt { id, .. }
            | TranscriptEntry::Text { id, .. }
            | TranscriptEntry::Thinking { id, .. }
            | TranscriptEntry::Tool { id, .. }
            | TranscriptEntry::System { id, .. } => id,
        }
    }
}

/// Woher die Änderungen stammen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub enum ChangeOrigin {
    Git,
    Transcript,
    None,
}

/// Art der Änderung einer Datei
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// Eine geänderte Datei mit Zeilenzahlen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct ChangedFile {
    /// Pfad relativ zum Repo bzw. zum Arbeitsordner der Session, mit `/`
    pub path: String,
    pub status: FileStatus,
    pub added: i64,
    pub removed: i64,
}

/// Die Änderungen einer Session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct SessionChanges {
    pub origin: ChangeOrigin,
    /// Basis-Branch bei `git`
    pub base: Option<String>,
    /// Hinweis zur Herkunft, z. B. dass Änderungen per Bash fehlen
    pub note: Option<String>,
    pub files: Vec<ChangedFile>,
}

impl SessionChanges {
    /// Keine Quelle für Änderungen (weder Git noch Transcript)
    pub fn none() -> Self {
        Self {
            origin: ChangeOrigin::None,
            base: None,
            note: None,
            files: Vec::new(),
        }
    }
}

/// Art einer Diff-Zeile
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub enum DiffLineKind {
    Add,
    Remove,
    Context,
}

/// Eine Zeile eines Diffs (ohne Präfix)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

/// Ein Abschnitt eines Diffs mit Kopfzeile (`@@ -a,b +c,d @@`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// Diff einer Datei, gekürzt auf 5 000 Zeilen
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "sessionDetail.ts")]
pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<DiffHunk>,
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schreibt_die_eintraege_als_nach_kind_unterschiedene_union() {
        let thinking = TranscriptEntry::Thinking {
            id: "u1:0".into(),
            at: "2026-11-04T08:00:00.000Z".into(),
            text: String::new(),
            duration_ms: Some(2000),
        };
        assert_eq!(
            serde_json::to_string(&thinking).unwrap(),
            r#"{"kind":"thinking","id":"u1:0","at":"2026-11-04T08:00:00.000Z","text":"","durationMs":2000}"#
        );
        let tool = TranscriptEntry::Tool {
            id: "toolu_1".into(),
            at: "t".into(),
            name: "Bash".into(),
            summary: "Bash: ls".into(),
            input: serde_json::json!({ "command": "ls" }),
            result: Some(ToolResult {
                text: "a".into(),
                is_error: false,
                truncated: false,
            }),
        };
        assert_eq!(
            serde_json::to_string(&tool).unwrap(),
            r#"{"kind":"tool","id":"toolu_1","at":"t","name":"Bash","summary":"Bash: ls","input":{"command":"ls"},"result":{"text":"a","isError":false,"truncated":false}}"#
        );
    }

    #[test]
    fn schreibt_todo_stand_und_aenderungsart_wie_im_vertrag() {
        assert_eq!(
            serde_json::to_string(&TodoStatus::InProgress).unwrap(),
            r#""in_progress""#
        );
        assert_eq!(
            serde_json::to_string(&ChangeOrigin::Transcript).unwrap(),
            r#""transcript""#
        );
        assert_eq!(
            serde_json::to_string(&SystemLevel::Warning).unwrap(),
            r#""warning""#
        );
    }
}
