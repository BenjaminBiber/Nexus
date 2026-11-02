//! Zustand des Dispatchers für Oberfläche und Kommandozeile; portiert aus
//! `packages/server/src/dispatch/DispatchStatus.ts`.
//!
//! Die JSON-Form ist Vertrag: `GET /api/dispatch/status`, `GET /api/dispatch/research/:id`
//! und `GET /api/dispatch/plan/:id` liefern genau diese Schlüssel.

use nexus_core::dispatch::{MergePhase, RunKind, RunLogEntry, RunLogParts};
use serde::{Deserialize, Serialize};

/// Zustand des Dispatchers in einem Wort
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DispatchStateKind {
    /// Ausgeschaltet
    Off,
    /// Eingeschaltet, nichts läuft
    Idle,
    /// Mindestens ein Ticket-Lauf läuft
    Running,
    /// Pause (von Hand oder nach einem Limit)
    Paused,
    /// Außerhalb des Zeitfensters
    Outside,
    /// Einstellung fehlerhaft
    Error,
}

/// Ein laufender Lauf für Oberfläche und Kommandozeile
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchRunInfo {
    pub id: String,
    pub kind: RunKind,
    pub ticket_id: String,
    pub title: String,
    pub profile: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub started_at: String,
    /// Dateiname des Protokolls
    pub log: String,
    /// Recherche: ID der Frage (fehlt bei allen anderen Laufarten)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_id: Option<String>,
    /// Claude-Account des Laufs; fehlt bei Läufen ohne Account-Wahl (= Standard)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

/// Lauf eines Tickets oder einer Frage aus den Protokollen (Reiter „Agent“)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchRunLog {
    /// Dateiname, z. B. `ARC-0001-20261016-091500-merge.log`
    pub name: String,
    pub kind: RunKind,
    /// Start laut Namen als ISO-Zeit
    pub started: Option<String>,
    /// Der Worker läuft noch: der Lauf ist eingetragen und hat noch keine Ergebnisdatei
    pub running: bool,
}

impl DispatchRunLog {
    /// Protokolleintrag mit dem Vermerk, ob sein Worker noch läuft
    pub fn new(entry: RunLogEntry, running: bool) -> Self {
        Self {
            name: entry.name,
            kind: entry.kind,
            started: entry.started,
            running,
        }
    }
}

/// Recherchen im Zustand: Automatik, Grenze und laufende Recherche-Läufe
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchResearchInfo {
    pub auto: bool,
    pub wip: i64,
    pub active: i64,
    /// Höchstens so viele automatische Recherchen am Tag …
    pub max_per_day: i64,
    /// … davon heute gestartet
    pub today: i64,
    /// Automatik gilt für Fragen ab diesem Zeitpunkt (null = aus)
    pub since: Option<String>,
    /// Nach einem Umgebungs- oder Repo-Fehler ausgesetzt bis (null = nicht ausgesetzt)
    pub retry_at: Option<String>,
    /// Letzter Umgebungs- oder Repo-Fehler der Automatik
    pub error: Option<String>,
}

/// Planungen im Zustand: Automatik, Grenze und laufende Planungsläufe
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchPlanningInfo {
    pub auto: bool,
    pub wip: i64,
    pub active: i64,
    /// Automatik gilt für Tickets ab diesem Zeitpunkt (null = aus)
    pub since: Option<String>,
    /// Nach einem Umgebungs- oder Repo-Fehler ausgesetzt bis (null = nicht ausgesetzt)
    pub retry_at: Option<String>,
    /// Letzter Umgebungs- oder Repo-Fehler der Automatik
    pub error: Option<String>,
}

/// Antwort von `GET /api/dispatch/research/:questionId`: läuft für die Frage gerade eine
/// Recherche?
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionResearchStatus {
    pub question_id: String,
    pub run: Option<DispatchRunInfo>,
    /// Pause des Dispatchers (z. B. Nutzungslimit); ein Start von Hand wird solange abgelehnt
    pub paused_until: Option<String>,
    pub last_started_at: Option<String>,
    pub count: i64,
}

/// Antwort von `GET /api/dispatch/plan/:ticketId`: läuft für das Ticket gerade eine Planung?
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketPlanningStatus {
    pub ticket_id: String,
    pub run: Option<DispatchRunInfo>,
    /// Pause des Dispatchers (z. B. Nutzungslimit); ein Start von Hand wird solange abgelehnt
    pub paused_until: Option<String>,
}

/// Ein Merge-Auftrag im Zustand
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchMergeInfo {
    pub ticket_id: String,
    pub phase: MergePhase,
    pub branch: String,
    /// Zielbranch des Tickets; null ohne Zielbranch (Merge in den ausgecheckten Branch) oder
    /// wenn das Ticket nicht lesbar ist
    pub target: Option<String>,
}

/// Bereitschaft eines Geplant-Tickets im Zustand
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchTicketRow {
    pub id: String,
    pub title: String,
    /// Warum (noch) nicht – oder „bereit“ bzw. „bereit – wartet auf freien Platz“
    pub reason: String,
}

/// Antwort von `GET /api/dispatch/status`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStatus {
    pub enabled: bool,
    pub state: DispatchStateKind,
    pub message: String,
    pub paused_until: Option<String>,
    /// Laufende Pausen einzelner Claude-Accounts (Nutzungslimit): Account-ID → ISO-Zeitpunkt
    pub account_pauses: std::collections::BTreeMap<String, String>,
    pub wip: i64,
    /// Laufende Ticket-Läufe (Umsetzung und Merge-Session); Recherchen zählen unter
    /// `research.active`, Planungen unter `planning`
    pub active: i64,
    /// Laufende Planungsläufe („Planen“ im Ticket; zählen nicht zum WIP)
    pub planning: i64,
    pub runs: Vec<DispatchRunInfo>,
    pub research: DispatchResearchInfo,
    /// Zustand der Planungs-Automatik (`planning` bleibt die Zahl der laufenden Planungen)
    pub planning_auto: DispatchPlanningInfo,
    pub merges: Vec<DispatchMergeInfo>,
    pub board_id: Option<i32>,
    pub board_in_scope: bool,
    pub tickets: Vec<DispatchTicketRow>,
    pub tickets_error: Option<String>,
    pub last_tick: Option<String>,
    pub last_errors: Vec<String>,
    pub config_file: String,
    pub config_error: Option<String>,
    /// Namensvorsatz der Agenten (Karten zeigen dafür „Agent“)
    pub agent_prefix: String,
}

/// Antwort von `GET /api/dispatch/runs/:name`: ein Lauf-Protokoll in seinen Teilen. Die
/// JSON-Form ist flach (`name`, `header`, `prompt`, `body`) wie in `DispatchService.runLog`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchRunLogText {
    /// Dateiname des Protokolls
    pub name: String,
    #[serde(flatten)]
    pub parts: RunLogParts,
}
