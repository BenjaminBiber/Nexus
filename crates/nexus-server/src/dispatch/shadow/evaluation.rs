//! Rechnet die Auswertung eines Laufs nach, sobald seine Zeile in `board.DispatchRuns` steht,
//! und vergleicht sie mit dieser Zeile.
//!
//! Nachgerechnet wird alles, was allein aus Lauf, Profil und Lauf-Ausgabe folgt: Zahlen des
//! Verbrauchs, Dauer, Schritte, Rückgabecode, Zeitüberschreitung und die abgelehnten
//! Werkzeugaufrufe. Dasselbe tut [`RunRecorder::row`] im Betrieb, deshalb rechnet der
//! Vergleich mit genau dieser Funktion.
//!
//! Nicht nachgerechnet wird der Ausgang eines Umsetzungslaufs: er hängt daran, in welchem
//! Zustand das Ticket beim Auswerten war (erledigt, blockiert, Frage, von einem Menschen
//! übernommen) – und dieser Zustand entsteht erst durch die Schreibvorgänge des
//! auswertenden Dispatchers. Bei Merge-Sessions und Recherchen folgt der Ausgang allein aus
//! der Laufart und wird deshalb verglichen. Die Kategorie eines Abbruchs wird gegengeprüft,
//! indem Rust den Grund der Zeile erneut einordnet.

use chrono::Duration;
use nexus_core::board::dispatch_run_types::{DispatchRun, DispatchRunKind, DispatchRunOutcome};
use nexus_core::clock::iso;
use nexus_core::dispatch::shadow::{compare, ShadowDiff, ShadowEvaluation};
use nexus_core::dispatch::{AbortReasonClassifier, RunEnd, RunKind, RunRecord};
use nexus_core::Result;

use crate::dispatch::context::DispatchContext;
use crate::dispatch::run_recorder::RunRecorder;

/// So lange wird auf die Zeile in `board.DispatchRuns` gewartet, danach gilt sie als fehlend
pub const WAIT_MINUTES: i64 = 30;

/// Felder, die allein aus Lauf, Profil und Lauf-Ausgabe folgen
const PURE: &[&str] = &[
    "kind",
    "subjectId",
    "projectId",
    "boardId",
    "agent",
    "attempt",
    "profile",
    "escalatedFrom",
    "requestedModel",
    "model",
    "effort",
    "startedAt",
    "endedAt",
    "durationMs",
    "numTurns",
    "exitCode",
    "timedOut",
    "costUsd",
    "inputTokens",
    "outputTokens",
    "cacheReadTokens",
    "cacheWriteTokens",
    "modelUsage",
    "source",
    "maxTurns",
    "timeoutMinutes",
    "profileReason",
    "ruleIndex",
    "configHash",
    "deniedCount",
    "deniedTools",
];

/// Was der Vergleich einer Auswertung ergeben hat
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationStatus {
    /// Die Zeile ist da und verglichen
    Compared(ShadowEvaluation),
    /// Die Zeile fehlt noch; es lohnt, später erneut zu schauen
    Waiting,
    /// Die Zeile ist auch nach [`WAIT_MINUTES`] nicht erschienen
    Missing(ShadowEvaluation),
}

/// Rechnet Auswertungen nach und vergleicht sie.
#[derive(Debug)]
pub struct ShadowEvaluator<'a> {
    ctx: &'a DispatchContext,
}

impl<'a> ShadowEvaluator<'a> {
    /// Vergleicher auf diesem (nur lesenden) Kontext
    pub fn new(ctx: &'a DispatchContext) -> Self {
        Self { ctx }
    }

    /// Sucht die Zeile des Laufs und vergleicht sie mit der nachgerechneten
    pub async fn compare(&self, record: &RunRecord, end: &RunEnd) -> Result<EvaluationStatus> {
        let ctx = self.ctx;
        let mut line = ShadowEvaluation {
            at: ctx.now_iso(),
            run_id: record.id.clone(),
            row_id: None,
            ticket_id: record.ticket_id.clone(),
            kind: record.kind.as_str().to_string(),
            equal: true,
            missing_row: false,
            diffs: Vec::new(),
        };
        let Some(row) = self.row_of(record).await? else {
            return Ok(if self.overdue(end) {
                line.equal = false;
                line.missing_row = true;
                EvaluationStatus::Missing(line)
            } else {
                EvaluationStatus::Waiting
            });
        };
        line.row_id = Some(row.id.clone());
        let Some(mine) = self.recompute(record, end, &row) else {
            // Planungen stehen nicht in der Tabelle; eine Zeile dazu gibt es ohnehin nicht
            return Ok(EvaluationStatus::Waiting);
        };
        let node = serde_json::to_value(&row).unwrap_or_default();
        let rust = serde_json::to_value(&mine).unwrap_or_default();
        let mut fields: Vec<&'static str> = PURE.to_vec();
        if matches!(record.kind, RunKind::Merge | RunKind::Research) {
            fields.push("outcome");
        }
        line.diffs = compare::diffs(&node, &rust, &fields);
        if let Some(diff) = category_check(&row) {
            line.diffs.push(diff);
        }
        line.equal = line.diffs.is_empty();
        Ok(EvaluationStatus::Compared(line))
    }

    /// Die nachgerechnete Zeile; `None` für Laufarten, die die Tabelle nicht kennt
    fn recompute(
        &self,
        record: &RunRecord,
        end: &RunEnd,
        row: &DispatchRun,
    ) -> Option<DispatchRun> {
        let kind = match record.kind {
            RunKind::Work => DispatchRunKind::Work,
            RunKind::Merge => DispatchRunKind::Merge,
            RunKind::Research => DispatchRunKind::Research,
            RunKind::Plan => return None,
        };
        // Der Ausgang eines Umsetzungslaufs ist nur mit dem Zustand des Tickets beim
        // Auswerten zu bestimmen; dort übernimmt der Vergleich den der Zeile und lässt das
        // Feld außen vor
        let outcome = match record.kind {
            RunKind::Merge => DispatchRunOutcome::MergeSession,
            RunKind::Research => DispatchRunOutcome::Research,
            _ => row.outcome,
        };
        Some(RunRecorder::new(self.ctx).row(
            record,
            end,
            row.project_id,
            row.board_id.unwrap_or_default(),
            kind,
            (
                outcome,
                row.abort_category.clone(),
                row.abort_reason.clone(),
            ),
        ))
    }

    /// Die Zeile dieses Laufs in `board.DispatchRuns`; ihre ID ist die des Laufs mit einem
    /// zufälligen Anhang (`RunRecorder.id`)
    async fn row_of(&self, record: &RunRecord) -> Result<Option<DispatchRun>> {
        let ctx = self.ctx;
        let started = chrono::DateTime::parse_from_rfc3339(&record.started_at)
            .map(|at| at.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| ctx.now());
        let from = started - Duration::minutes(1);
        let to = ctx.now() + Duration::minutes(1);
        let wanted = format!("{}-", record.id);
        let rows = ctx.services().dispatch_runs.list_all(from, to).await?;
        Ok(rows.into_iter().find(|row| row.id.starts_with(&wanted)))
    }

    /// Ist die Wartefrist auf die Zeile abgelaufen?
    fn overdue(&self, end: &RunEnd) -> bool {
        let ended = match chrono::DateTime::parse_from_rfc3339(&end.ended_at) {
            Ok(at) => at.with_timezone(&chrono::Utc),
            // Ohne lesbare Endzeit lieber weiter warten, als eine Zeile zu Unrecht zu
            // vermissen
            Err(_) => return false,
        };
        self.ctx.now() > ended + Duration::minutes(WAIT_MINUTES)
    }
}

/// Ordnet den Abbruchgrund der Zeile erneut ein; weicht die Kategorie ab, ist das eine
/// Abweichung des Einordners selbst
fn category_check(row: &DispatchRun) -> Option<ShadowDiff> {
    let reason = row.abort_reason.as_deref()?;
    let mine = AbortReasonClassifier::category(reason).to_string();
    if row.abort_category.as_deref() == Some(mine.as_str()) {
        return None;
    }
    Some(ShadowDiff::new(
        "abortCategory",
        row.abort_category.clone(),
        Some(mine),
    ))
}

/// Die Endzeit eines Laufs als ISO-Text (für Protokollzeilen)
pub fn ended(end: &RunEnd) -> String {
    chrono::DateTime::parse_from_rfc3339(&end.ended_at)
        .map(|at| iso(at.with_timezone(&chrono::Utc)))
        .unwrap_or_else(|_| end.ended_at.clone())
}
