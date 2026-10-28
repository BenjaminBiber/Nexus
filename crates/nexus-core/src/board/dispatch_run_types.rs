//! Läufe des Dispatchers und ihre Auswertung; portiert aus
//! `packages/core/src/board/DispatchRunTypes.ts`.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Art eines Laufs: Ticket umsetzen, mergen oder zu einer Frage recherchieren
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "dispatch.ts")]
pub enum DispatchRunKind {
    Work,
    Merge,
    Research,
}

impl DispatchRunKind {
    /// Alle Arten in der Reihenfolge von TypeScript
    pub const ALL: [DispatchRunKind; 3] = [
        DispatchRunKind::Work,
        DispatchRunKind::Merge,
        DispatchRunKind::Research,
    ];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            DispatchRunKind::Work => "work",
            DispatchRunKind::Merge => "merge",
            DispatchRunKind::Research => "research",
        }
    }
}

impl fmt::Display for DispatchRunKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ergebnis eines Laufs als feste Kategorie
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "dispatch.ts")]
pub enum DispatchRunOutcome {
    Done,
    Review,
    Human,
    Abort,
    AbortFinal,
    Blocked,
    Question,
    PauseLimit,
    PauseClaudeMissing,
    SetupFailed,
    Taken,
    Gone,
    Other,
    MergeSession,
    Research,
}

impl DispatchRunOutcome {
    /// Alle Ergebnisse in der Reihenfolge von TypeScript
    pub const ALL: [DispatchRunOutcome; 15] = [
        DispatchRunOutcome::Done,
        DispatchRunOutcome::Review,
        DispatchRunOutcome::Human,
        DispatchRunOutcome::Abort,
        DispatchRunOutcome::AbortFinal,
        DispatchRunOutcome::Blocked,
        DispatchRunOutcome::Question,
        DispatchRunOutcome::PauseLimit,
        DispatchRunOutcome::PauseClaudeMissing,
        DispatchRunOutcome::SetupFailed,
        DispatchRunOutcome::Taken,
        DispatchRunOutcome::Gone,
        DispatchRunOutcome::Other,
        DispatchRunOutcome::MergeSession,
        DispatchRunOutcome::Research,
    ];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            DispatchRunOutcome::Done => "done",
            DispatchRunOutcome::Review => "review",
            DispatchRunOutcome::Human => "human",
            DispatchRunOutcome::Abort => "abort",
            DispatchRunOutcome::AbortFinal => "abort-final",
            DispatchRunOutcome::Blocked => "blocked",
            DispatchRunOutcome::Question => "question",
            DispatchRunOutcome::PauseLimit => "pause-limit",
            DispatchRunOutcome::PauseClaudeMissing => "pause-claude-missing",
            DispatchRunOutcome::SetupFailed => "setup-failed",
            DispatchRunOutcome::Taken => "taken",
            DispatchRunOutcome::Gone => "gone",
            DispatchRunOutcome::Other => "other",
            DispatchRunOutcome::MergeSession => "merge-session",
            DispatchRunOutcome::Research => "research",
        }
    }
}

impl fmt::Display for DispatchRunOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Warum das Basisprofil eines Laufs gewählt wurde: Standardprofil, Label profil:,
/// Aufwand laut Plan, Regel, Einstellung research/planning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "dispatch.ts")]
pub enum DispatchProfileReason {
    Default,
    Label,
    Plan,
    Rule,
    Setting,
}

impl DispatchProfileReason {
    /// Alle Gründe in der Reihenfolge von TypeScript
    pub const ALL: [DispatchProfileReason; 5] = [
        DispatchProfileReason::Default,
        DispatchProfileReason::Label,
        DispatchProfileReason::Plan,
        DispatchProfileReason::Rule,
        DispatchProfileReason::Setting,
    ];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            DispatchProfileReason::Default => "default",
            DispatchProfileReason::Label => "label",
            DispatchProfileReason::Plan => "plan",
            DispatchProfileReason::Rule => "rule",
            DispatchProfileReason::Setting => "setting",
        }
    }
}

impl fmt::Display for DispatchProfileReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Herkunft einer Lauf-Zeile: vom Dispatcher erfasst oder aus
/// `[dispatch]`-Kommentaren rekonstruiert
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "dispatch.ts")]
pub enum DispatchRunSource {
    Dispatcher,
    Comments,
}

impl DispatchRunSource {
    /// Beide Herkünfte in der Reihenfolge von TypeScript
    pub const ALL: [DispatchRunSource; 2] =
        [DispatchRunSource::Dispatcher, DispatchRunSource::Comments];

    /// Schreibweise in JSON und in den Datenbank-Spalten
    pub fn as_str(&self) -> &'static str {
        match self {
            DispatchRunSource::Dispatcher => "dispatcher",
            DispatchRunSource::Comments => "comments",
        }
    }
}

impl fmt::Display for DispatchRunSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ein ausgewerteter Lauf des Dispatchers (Tabelle board.DispatchRuns)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchRun {
    /// Lauf-ID des RunStore bzw. c<CommentId> beim Backfill
    pub id: String,
    pub kind: DispatchRunKind,
    /// Ticket- oder Fragen-ID (kein Fremdschlüssel)
    pub subject_id: String,
    pub project_id: i32,
    pub board_id: Option<i32>,
    pub agent: String,
    pub attempt: Option<i32>,
    pub profile: String,
    pub escalated_from: Option<String>,
    /// Modell-Alias aus Profil oder Label
    pub requested_model: Option<String>,
    /// Hauptmodell laut `model_usage`
    pub model: Option<String>,
    pub effort: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_ms: Option<i32>,
    pub num_turns: Option<i32>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub outcome: DispatchRunOutcome,
    pub abort_category: Option<String>,
    pub abort_reason: Option<String>,
    pub cost_usd: Option<f64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    /// Verbrauch je Modell als JSON-Text
    pub model_usage: Option<String>,
    pub source: DispatchRunSource,
    /// Schrittlimit, das für diesen Lauf galt
    pub max_turns: Option<i32>,
    /// Zeitlimit in Minuten, das für diesen Lauf galt
    pub timeout_minutes: Option<i32>,
    /// Warum das (Basis-)Profil gewählt wurde
    pub profile_reason: Option<DispatchProfileReason>,
    /// 1-basierte Nummer der Regel bei `profile_reason` = `rule`
    pub rule_index: Option<i32>,
    /// Version der Dispatcher-Einstellung (Hash)
    pub config_hash: Option<String>,
    /// Anzahl abgelehnter Werkzeugaufrufe
    pub denied_count: Option<i32>,
    /// Abgelehnte Werkzeuge, kommagetrennt und eindeutig
    pub denied_tools: Option<String>,
}

/// Eine Fassung der Dispatcher-Einstellung (Tabelle board.DispatchConfigVersions)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchConfigVersion {
    /// Hash der wirksamen Einstellung (DispatchConfigFingerprint)
    pub hash: String,
    /// Name des Dispatchers, der diese Fassung zuerst benutzt hat
    pub dispatcher: String,
    /// Zeitpunkt des ersten Auftretens (ISO, UTC)
    pub first_seen_at: String,
    /// Die gehashten Teile der Einstellung als JSON-Text
    pub content: String,
}

/// Läufe eines Projekts, deren Start in [from, to) liegt (ISO, UTC)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchRunFilter {
    pub project_id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub board_id: Option<i32>,
    pub from: String,
    pub to: String,
}

/// Zahl der `[dispatch]`-Kommentare „Start“ und „Abbruch“ eines Tickets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchCommentCounts {
    pub starts: i32,
    pub aborts: i32,
}

/// Filter der Auswertung; `from`/`to` als Zeitpunkte (`to` ausschließlich).
/// Nur intern: in TypeScript stehen hier `Date`-Werte, deshalb kein Export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchStatsFilter {
    pub project_id: i32,
    pub board_id: Option<i32>,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    /// Art der ausgewerteten Läufe (Standard `work`)
    pub kind: Option<DispatchRunKind>,
}

/// Ausgänge der Läufe eines Profils: gelöst, abgebrochen mit weiterem Versuch
/// (eskaliert), endgültig gescheitert, Rest
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchProfileRow {
    pub profile: String,
    pub runs: i32,
    pub done: i32,
    pub escalated: i32,
    /// Heißt in JSON `final`; `final` ist in Rust für künftige Zwecke reserviert
    #[serde(rename = "final")]
    #[ts(rename = "final")]
    pub final_failures: i32,
    pub other: i32,
}

/// Kennzahlen der Auswertung
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchStatsKpis {
    pub runs: i32,
    /// Verschiedene Tickets bzw. Fragen
    pub subjects: i32,
    pub done: i32,
    pub aborted: i32,
    /// Gegenstände mit einem erfolgreichen Lauf
    pub solved: i32,
    /// Gelöste / abgeschlossene Gegenstände (gelöst oder endgültig gescheitert);
    /// wie die Erstversuch-Quote je Gegenstand
    pub success_rate: Option<f64>,
    /// Abgeschlossene Gegenstände, die im ersten Lauf ohne Eskalation gelöst wurden
    pub first_try_rate: Option<f64>,
    /// Gegenstände mit mindestens einem Lauf nach einem Abbruch
    /// (Wiederholung oder Eskalation)
    pub escalation_rate: Option<f64>,
    pub final_failures: i32,
}

/// Raster der Zeitreihe: je Tag oder je Woche
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "dispatch.ts")]
pub enum DispatchSeriesUnit {
    Day,
    Week,
}

impl DispatchSeriesUnit {
    /// Beide Raster in der Reihenfolge von TypeScript
    pub const ALL: [DispatchSeriesUnit; 2] = [DispatchSeriesUnit::Day, DispatchSeriesUnit::Week];

    /// Schreibweise in JSON
    pub fn as_str(&self) -> &'static str {
        match self {
            DispatchSeriesUnit::Day => "day",
            DispatchSeriesUnit::Week => "week",
        }
    }
}

impl fmt::Display for DispatchSeriesUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ein Punkt der Zeitreihe; `start` = JJJJ-MM-TT in Ortszeit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchSeriesPoint {
    pub start: String,
    pub done: i32,
    pub aborted: i32,
    pub other: i32,
}

/// Läufe im Zeitverlauf: bis 28 Tage je Tag, darüber je Woche (Montag)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchSeries {
    pub unit: DispatchSeriesUnit,
    pub points: Vec<DispatchSeriesPoint>,
}

/// Abgeschlossene Gegenstände: im ersten Versuch gelöst, nach Eskalation gelöst,
/// endgültig gescheitert (Anteile)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchLadder {
    pub finished: i32,
    pub first_try: Option<f64>,
    pub after_escalation: Option<f64>,
    /// Heißt in JSON `final`; `final` ist in Rust für künftige Zwecke reserviert
    #[serde(rename = "final")]
    #[ts(rename = "final")]
    pub final_failures: Option<f64>,
}

/// Abbrüche einer Kategorie mit Anzeigetext und Anzahl
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchAbortReason {
    pub category: String,
    pub label: String,
    pub count: i32,
}

/// Ergebnis der Auswertung (Läufe einer Art, Start im Zeitraum)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "dispatch.ts")]
pub struct DispatchStats {
    pub kind: DispatchRunKind,
    pub runs_since: Option<String>,
    pub backfilled_runs: i32,
    /// Erstes Auftreten der neuesten Fassung der Dispatcher-Einstellung (ISO)
    pub config_changed_at: Option<String>,
    pub kpis: DispatchStatsKpis,
    pub series: DispatchSeries,
    pub by_profile: Vec<DispatchProfileRow>,
    pub ladder: DispatchLadder,
    pub abort_reasons: Vec<DispatchAbortReason>,
    /// Anteil der Abbrüche an Schritt-, Zeitlimit oder vollem Kontext
    /// (per Einstellung beeinflussbar)
    pub configurable_abort_share: Option<f64>,
    /// Pausen wegen Nutzungslimit im Zeitraum (alle Arten)
    pub usage_pauses: i32,
    /// Läufe ohne Lebenszeichen (staleMinutes überschritten) im Zeitraum (alle Arten)
    pub stale_runs: i32,
}
