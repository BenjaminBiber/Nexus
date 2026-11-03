//! Erfasst ausgewertete Läufe für die Auswertung; portiert aus
//! `packages/server/src/dispatch/RunRecorder.ts`.
//!
//! Erst lokal vorgemerkt (`RunStore`, Ordner `stats`), dann in `board.DispatchRuns`. So geht
//! bei einem Verbindungsfehler kein Lauf verloren und keiner wird doppelt ausgewertet.

use std::sync::atomic::{AtomicBool, Ordering};

use nexus_core::board::basics::is_human_interaction;
use nexus_core::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchRun, DispatchRunKind, DispatchRunOutcome, DispatchRunSource,
};
use nexus_core::clock::iso;
use nexus_core::dispatch::{
    AbortReasonClassifier, DispatchConfigFingerprint, ModelUsage, OutputSummary, RunEnd, RunKind,
    RunRecord, RunSettings, RunUsage, UsageHistory, UsageSample,
};
use nexus_core::{Error, Result};
use serde_json::{Map, Number, Value};

use crate::board_runtime::BoardRuntimeHolder;
use crate::dispatch::context::DispatchContext;

/// Höchstlänge der Texte, die die Lauf-Zeile mitschreibt
const MAX_TEXT: usize = 500;

/// Ausgang der Auswertung eines Laufs, wie Bericht und Protokoll ihn nennen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleOutcome {
    Done,
    Review,
    Blocked,
    Question,
    Limit,
    ClaudeMissing,
    Released,
    Taken,
    Other,
    Gone,
    MergeSession,
    SetupFailed,
}

impl HandleOutcome {
    /// Schreibweise wie in TypeScript (Bericht des Durchlaufs)
    pub fn as_str(&self) -> &'static str {
        match self {
            HandleOutcome::Done => "done",
            HandleOutcome::Review => "review",
            HandleOutcome::Blocked => "blocked",
            HandleOutcome::Question => "question",
            HandleOutcome::Limit => "limit",
            HandleOutcome::ClaudeMissing => "claude-missing",
            HandleOutcome::Released => "released",
            HandleOutcome::Taken => "taken",
            HandleOutcome::Other => "other",
            HandleOutcome::Gone => "gone",
            HandleOutcome::MergeSession => "merge-session",
            HandleOutcome::SetupFailed => "setup-failed",
        }
    }

    /// Feste Kategorie für `board.DispatchRuns` (`OUTCOMES` in TypeScript); `Blocked` steht
    /// nicht in der Tabelle, weil erst der Grund „Human Interaction“ darüber entscheidet
    fn recorded(&self) -> DispatchRunOutcome {
        match self {
            HandleOutcome::Done => DispatchRunOutcome::Done,
            HandleOutcome::Review => DispatchRunOutcome::Review,
            HandleOutcome::Question => DispatchRunOutcome::Question,
            HandleOutcome::Limit => DispatchRunOutcome::PauseLimit,
            HandleOutcome::ClaudeMissing => DispatchRunOutcome::PauseClaudeMissing,
            HandleOutcome::Released => DispatchRunOutcome::Abort,
            HandleOutcome::Taken => DispatchRunOutcome::Taken,
            HandleOutcome::Gone => DispatchRunOutcome::Gone,
            HandleOutcome::MergeSession => DispatchRunOutcome::MergeSession,
            HandleOutcome::SetupFailed => DispatchRunOutcome::SetupFailed,
            // `Other` steht in der Abbildung, `Blocked` fehlt dort und fällt wie alles
            // Unbekannte auf „other“
            HandleOutcome::Other | HandleOutcome::Blocked => DispatchRunOutcome::Other,
        }
    }
}

/// Ein Abbruch samt Grund und ob es der letzte Versuch war
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbortInfo {
    pub reason: String,
    pub final_attempt: bool,
}

/// Ergebnis der Auswertung eines Laufs: Ausgang für Bericht und Protokoll, dazu der Abbruch-
/// bzw. Blockiert-Grund
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleResult {
    pub outcome: HandleOutcome,
    pub abort: Option<AbortInfo>,
    pub blocked_reason: Option<String>,
}

impl HandleResult {
    /// Ergebnis ohne Abbruch und ohne Grund
    pub fn new(outcome: HandleOutcome) -> Self {
        Self {
            outcome,
            abort: None,
            blocked_reason: None,
        }
    }
}

/// Was ein gescheiterter Start für die Auswertung braucht
#[derive(Debug, Clone)]
pub struct StartFailed {
    pub ticket_id: String,
    pub project_id: i32,
    pub board_id: i32,
    pub agent: String,
    pub run: RunSettings,
    pub attempt: i64,
    pub reason: String,
    pub final_attempt: bool,
}

/// Merkt Lauf-Zeilen vor und überträgt sie in `board.DispatchRuns`.
#[derive(Debug)]
pub struct RunRecorder<'a> {
    ctx: &'a DispatchContext,
    fingerprint: DispatchConfigFingerprint,
    /// Fehlende Erweiterung schon protokolliert (sonst in jedem Durchlauf derselbe Eintrag).
    /// In TypeScript ein Feld des Recorders, der dort den Dispatcher überlebt – hier gehört
    /// er zum Durchlauf, deshalb steht die Markierung im Kontext
    outdated_noted: AtomicBool,
}

impl<'a> RunRecorder<'a> {
    /// Recorder auf diesem Kontext
    pub fn new(ctx: &'a DispatchContext) -> Self {
        Self {
            ctx,
            fingerprint: DispatchConfigFingerprint::new(),
            outdated_noted: AtomicBool::new(false),
        }
    }

    /// Zeile zu einem beendeten Lauf vormerken; Fehler nur ins Protokoll (nie werfen: der Lauf
    /// ist schon ausgewertet und darf nicht ein zweites Mal ausgewertet werden)
    pub async fn keep(&self, record: &RunRecord, end: &RunEnd, result: &HandleResult) {
        self.keep_usage_sample(record, end).await;
        // Planungen erfasst board.DispatchRuns (noch) nicht: die Spalte Kind kennt nur work,
        // merge, research
        if record.kind == RunKind::Plan {
            return;
        }
        if let Err(error) = self.try_keep(record, end, result).await {
            self.ctx.log(&format!(
                "FEHLER {}: Lauf nicht für die Auswertung vorgemerkt: {error}",
                record.ticket_id
            ));
        }
    }

    /// Verbrauchsprobe eines beendeten Umsetzungslaufs: Auslastung beim Start aus dem Lauf, am
    /// Ende frisch gefragt. Ohne Account-Verwaltung, ohne Start-Auslastung oder ohne Antwort
    /// entsteht keine; Fehler nur ins Protokoll
    async fn keep_usage_sample(&self, record: &RunRecord, end: &RunEnd) {
        if record.kind != RunKind::Work {
            return;
        }
        let (Some(account), Some(before), Some(accounts)) = (
            record.run.account.as_deref(),
            record.run.usage_at_start.as_ref(),
            self.ctx.accounts(),
        ) else {
            return;
        };
        let Some(after) = accounts.usage_snapshot(account).await else {
            return;
        };
        let sample = UsageSample {
            account: account.to_string(),
            profile: record.run.profile.clone(),
            model: record.run.model.clone(),
            effort: record.run.effort.clone(),
            started_at: record.started_at.clone(),
            ended_at: end.ended_at.clone(),
            before: before.clone(),
            after,
        };
        if let Err(error) = UsageHistory::new(self.ctx.home.clone()).append(&sample) {
            self.ctx.log(&format!(
                "FEHLER {}: Verbrauchsprobe nicht gespeichert: {error}",
                record.ticket_id
            ));
        }
    }

    /// Der Versuch, der in [`RunRecorder::keep`] nur protokolliert wird
    async fn try_keep(
        &self,
        record: &RunRecord,
        end: &RunEnd,
        result: &HandleResult,
    ) -> Result<()> {
        let Some(board) = self.ctx.board(record.board_id as i32).await? else {
            return Ok(());
        };
        let Some(kind) = recorded_kind(record.kind) else {
            return Ok(());
        };
        let outcome = self.outcome(record, result);
        let row = self.row(record, end, board.project_id, board.id, kind, outcome);
        self.ctx.stores.runs.keep_stats(&row.id, &row)
    }

    /// Die Zeile eines beendeten Laufs: alles, was aus Lauf, Profil und Lauf-Ausgabe folgt,
    /// dazu der schon bestimmte Ausgang. Der Schattenbetrieb rechnet damit dieselben Zahlen
    /// nach, ohne den Lauf auszuwerten
    pub fn row(
        &self,
        record: &RunRecord,
        end: &RunEnd,
        project_id: i32,
        board_id: i32,
        kind: DispatchRunKind,
        outcome: (DispatchRunOutcome, Option<String>, Option<String>),
    ) -> DispatchRun {
        let usage = self.ctx.output.usage(&end.result.output);
        let summary = self.ctx.output.summarize(&end.result.output);
        let (outcome, abort_category, abort_reason) = outcome;
        let denied = denied(usage.as_ref(), &summary);
        DispatchRun {
            model: usage.as_ref().and_then(|usage| usage.model.clone()),
            ended_at: Some(end.ended_at.clone()),
            duration_ms: duration_ms(&record.started_at, &end.ended_at),
            num_turns: usage.as_ref().and_then(|usage| count(usage.num_turns)),
            exit_code: record_i32(end.result.code),
            timed_out: end.result.timed_out,
            outcome,
            abort_category,
            abort_reason,
            cost_usd: usage.as_ref().and_then(|usage| usage.cost_usd),
            input_tokens: usage.as_ref().and_then(|usage| tokens(usage.input_tokens)),
            output_tokens: usage.as_ref().and_then(|usage| tokens(usage.output_tokens)),
            cache_read_tokens: usage
                .as_ref()
                .and_then(|usage| tokens(usage.cache_read_tokens)),
            cache_write_tokens: usage
                .as_ref()
                .and_then(|usage| tokens(usage.cache_write_tokens)),
            model_usage: usage.as_ref().and_then(model_usage_json),
            denied_count: denied.0,
            denied_tools: denied.1,
            ..base(
                &self.id(&record.id),
                kind,
                &record.ticket_id,
                project_id,
                board_id,
                &record.agent,
                &record.run,
                record.attempt,
                &record.started_at,
            )
        }
    }

    /// Gescheiterter Start (kein Lauf-Datensatz): Abbruch mit Kategorie `start-failed` vormerken
    pub async fn keep_start_failed(&self, input: &StartFailed) {
        let now = self.ctx.now();
        let at = iso(now);
        let id = self.id(&format!(
            "{}-start",
            self.ctx
                .stores
                .log
                .run_name(&input.ticket_id, RunKind::Work, now)
        ));
        let row = DispatchRun {
            model: None,
            ended_at: Some(at.clone()),
            duration_ms: Some(0),
            num_turns: None,
            exit_code: None,
            timed_out: false,
            outcome: if input.final_attempt {
                DispatchRunOutcome::AbortFinal
            } else {
                DispatchRunOutcome::Abort
            },
            abort_category: Some("start-failed".to_string()),
            abort_reason: Some(input.reason.clone()),
            cost_usd: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            model_usage: None,
            ..base(
                &id,
                DispatchRunKind::Work,
                &input.ticket_id,
                input.project_id,
                input.board_id,
                &input.agent,
                &input.run,
                Some(input.attempt),
                &at,
            )
        };
        if let Err(error) = self.ctx.stores.runs.keep_stats(&id, &row) {
            self.ctx.log(&format!(
                "FEHLER {}: gescheiterter Start nicht für die Auswertung vorgemerkt: {error}",
                input.ticket_id
            ));
        }
    }

    /// Vorgemerkte Zeilen in die Datenbank schreiben; ein Verbindungsfehler lässt den Rest für
    /// den nächsten Durchlauf liegen, ebenso eine noch nicht eingespielte Erweiterung (die
    /// Zeilen gehen nach dem Einspielen hinein); eine Zeile, die die Datenbank ablehnt, wird
    /// nach dem Protokolleintrag verworfen
    pub async fn flush(&self) -> Result<usize> {
        let mut written = 0;
        let kept = self.ctx.stores.runs.kept_stats::<DispatchRun>()?;
        if !kept.is_empty() {
            self.keep_config_version().await?;
        }
        for (id, row) in kept {
            match self.ctx.services().dispatch_runs.record(&row).await {
                Ok(_) => written += 1,
                Err(error) => {
                    if BoardRuntimeHolder::is_connection_error(&error) {
                        return Err(error);
                    }
                    if self.outdated(&error) {
                        return Ok(written);
                    }
                    self.ctx.log(&format!(
                        "FEHLER Lauf {id} nicht in die Auswertung geschrieben: {error}"
                    ));
                }
            }
            self.ctx.stores.runs.forget_stats(&id)?;
        }
        Ok(written)
    }

    /// Die aktuelle Fassung der Einstellung einmal ablegen (idempotent über den Hash), damit
    /// das Dashboard Konfig-Änderungen benennen kann
    async fn keep_config_version(&self) -> Result<()> {
        let cfg = &self.ctx.cfg;
        let version = DispatchConfigVersion {
            hash: self.fingerprint.hash(cfg),
            dispatcher: cfg.dispatcher_name.clone(),
            first_seen_at: self.ctx.now_iso(),
            content: self.fingerprint.content(cfg),
        };
        match self
            .ctx
            .services()
            .dispatch_runs
            .record_config_version(&version)
            .await
        {
            Ok(_) => Ok(()),
            Err(error) => {
                if BoardRuntimeHolder::is_connection_error(&error) {
                    return Err(error);
                }
                if self.outdated(&error) {
                    return Ok(());
                }
                self.ctx
                    .log(&format!("FEHLER Konfig-Version nicht gespeichert: {error}"));
                Ok(())
            }
        }
    }

    /// Fehlt der Datenbank die Erweiterung für die Läufe? Protokolliert das einmal je Durchlauf
    fn outdated(&self, error: &Error) -> bool {
        if error.code() != Some("board-schema-outdated") {
            return false;
        }
        if !self.outdated_noted.swap(true, Ordering::SeqCst) {
            self.ctx.log(&format!(
                "Läufe bleiben vorgemerkt, bis die Erweiterung eingespielt ist: {error}"
            ));
        }
        true
    }

    /// Lauf-IDs haben Sekunden-Auflösung (Abbruch und Neustart im selben Durchlauf ergäben
    /// dieselbe): ein zufälliger Anhang macht die Zeile eindeutig; beim Nachschreiben bleibt
    /// sie gleich, weil sie in der vorgemerkten Datei steht
    fn id(&self, run_id: &str) -> String {
        format!("{run_id}-{}", nexus_core::random::hex(4))
    }

    /// Feste Kategorie des Ausgangs, bei Abbruch mit Grund und Kategorie
    fn outcome(
        &self,
        record: &RunRecord,
        result: &HandleResult,
    ) -> (DispatchRunOutcome, Option<String>, Option<String>) {
        if record.kind == RunKind::Research {
            return (DispatchRunOutcome::Research, None, None);
        }
        if let Some(abort) = &result.abort {
            return (
                if abort.final_attempt {
                    DispatchRunOutcome::AbortFinal
                } else {
                    DispatchRunOutcome::Abort
                },
                Some(AbortReasonClassifier::category(&abort.reason).to_string()),
                Some(cut(&abort.reason)),
            );
        }
        if result.outcome == HandleOutcome::Blocked {
            let human = is_human_interaction(result.blocked_reason.as_deref());
            return (
                if human {
                    DispatchRunOutcome::Human
                } else {
                    DispatchRunOutcome::Blocked
                },
                None,
                result.blocked_reason.as_deref().map(cut),
            );
        }
        (result.outcome.recorded(), None, None)
    }
}

/// Felder, die aus Lauf und Profil folgen
#[allow(clippy::too_many_arguments)]
fn base(
    id: &str,
    kind: DispatchRunKind,
    subject_id: &str,
    project_id: i32,
    board_id: i32,
    agent: &str,
    run: &RunSettings,
    attempt: Option<i64>,
    started_at: &str,
) -> DispatchRun {
    DispatchRun {
        id: id.to_string(),
        kind,
        subject_id: subject_id.to_string(),
        project_id,
        board_id: Some(board_id),
        agent: agent.to_string(),
        attempt: attempt.map(|attempt| attempt as i32),
        profile: run.profile.clone(),
        escalated_from: run.escalated_from.clone(),
        requested_model: run.model.clone(),
        model: None,
        effort: run.effort.clone(),
        started_at: started_at.to_string(),
        ended_at: None,
        duration_ms: None,
        num_turns: None,
        exit_code: None,
        timed_out: false,
        outcome: DispatchRunOutcome::Other,
        abort_category: None,
        abort_reason: None,
        cost_usd: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        model_usage: None,
        source: DispatchRunSource::Dispatcher,
        // Limits und Grund der Profilwahl, wie sie für diesen Lauf galten; Läufe älterer
        // Stände haben keinen Grund
        max_turns: Some(run.max_turns as i32),
        timeout_minutes: Some(run.timeout_minutes as i32),
        profile_reason: run.reason,
        rule_index: run.rule_index.map(|index| index as i32),
        // Fassung der Einstellung, die beim Start galt; Läufe älterer Stände haben keine
        config_hash: run.config_hash.clone(),
        // Ohne Lauf-Ausgabe (gescheiterter Start) gibt es keine Ablehnungen zu zählen
        denied_count: None,
        denied_tools: None,
    }
}

/// Art des Laufs in `board.DispatchRuns`; Planungen stehen dort nicht
fn recorded_kind(kind: RunKind) -> Option<DispatchRunKind> {
    match kind {
        RunKind::Work => Some(DispatchRunKind::Work),
        RunKind::Merge => Some(DispatchRunKind::Merge),
        RunKind::Research => Some(DispatchRunKind::Research),
        RunKind::Plan => None,
    }
}

/// Abgelehnte Werkzeugaufrufe: `None` ohne Ergebnis-Objekt (z. B. hart beendeter Lauf), sonst
/// Anzahl und eindeutige Werkzeugnamen (Reihenfolge des ersten Auftretens); `usage` ist bereits
/// `None`, wenn kein Ergebnis gelesen werden konnte
fn denied(usage: Option<&RunUsage>, summary: &OutputSummary) -> (Option<i32>, Option<String>) {
    if usage.is_none() {
        return (None, None);
    }
    if summary.denied.is_empty() {
        return (Some(0), None);
    }
    let mut tools: Vec<&str> = Vec::new();
    for entry in &summary.denied {
        let tool = entry.split(": ").next().unwrap_or(entry);
        if !tools.contains(&tool) {
            tools.push(tool);
        }
    }
    let joined: String = tools.join(", ").chars().take(MAX_TEXT).collect();
    (Some(summary.denied.len() as i32), Some(joined))
}

/// Dauer des Laufs in Millisekunden; 0 zählt wie in TypeScript (`|| null`) als „unbekannt“
fn duration_ms(started_at: &str, ended_at: &str) -> Option<i32> {
    let start = chrono::DateTime::parse_from_rfc3339(started_at).ok()?;
    let end = chrono::DateTime::parse_from_rfc3339(ended_at).ok()?;
    let millis = (end - start).num_milliseconds().max(0);
    if millis == 0 {
        None
    } else {
        Some(millis as i32)
    }
}

/// Verbrauch je Modell als JSON-Text (`JSON.stringify(usage.modelUsage)`); ohne Modelle `None`
fn model_usage_json(usage: &RunUsage) -> Option<String> {
    if usage.model_usage.is_empty() {
        return None;
    }
    let mut models = Map::new();
    for (name, model) in &usage.model_usage {
        models.insert(name.clone(), model_usage_value(model));
    }
    Some(Value::Object(models).to_string())
}

/// Ein Modell als JSON-Objekt; die Schlüssel heißen wie in TypeScript
fn model_usage_value(model: &ModelUsage) -> Value {
    let mut fields = Map::new();
    for (key, value) in [
        ("inputTokens", model.input_tokens),
        ("outputTokens", model.output_tokens),
        ("cacheReadTokens", model.cache_read_tokens),
        ("cacheWriteTokens", model.cache_write_tokens),
        ("costUsd", model.cost_usd),
    ] {
        fields.insert(key.to_string(), number(value));
    }
    Value::Object(fields)
}

/// Zahl wie JavaScript sie schreibt: ganze Werte ohne Nachkommastelle, `None` als `null`
fn number(value: Option<f64>) -> Value {
    match value {
        None => Value::Null,
        Some(value) if value.fract() == 0.0 && value.abs() < 9.007_199_254_740_992e15 => {
            Value::Number(Number::from(value as i64))
        }
        Some(value) => Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
    }
}

/// Tokenzahl als ganze Zahl
fn tokens(value: Option<f64>) -> Option<i64> {
    value.map(|value| value as i64)
}

/// Schrittzahl als ganze Zahl
fn count(value: Option<f64>) -> Option<i32> {
    value.map(|value| value as i32)
}

/// Exit-Code des Laufs in der Breite der Spalte
fn record_i32(code: Option<i64>) -> Option<i32> {
    code.map(|code| code as i32)
}

/// Text auf die Länge der Spalte kürzen
fn cut(text: &str) -> String {
    text.chars().take(MAX_TEXT).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ausgaenge_heissen_wie_in_typescript() {
        assert_eq!(HandleOutcome::ClaudeMissing.as_str(), "claude-missing");
        assert_eq!(HandleOutcome::Released.as_str(), "released");
        assert_eq!(
            HandleOutcome::Limit.recorded(),
            DispatchRunOutcome::PauseLimit
        );
        assert_eq!(
            HandleOutcome::Released.recorded(),
            DispatchRunOutcome::Abort
        );
        assert_eq!(HandleOutcome::Blocked.recorded(), DispatchRunOutcome::Other);
    }

    #[test]
    fn abgelehnte_aufrufe_werden_eindeutig_gezaehlt() {
        let summary = OutputSummary {
            denied: vec![
                "Bash: git push".into(),
                "Bash: rm -rf /".into(),
                "Edit".into(),
            ],
            ..OutputSummary::default()
        };
        let usage = RunUsage::default();
        assert_eq!(
            denied(Some(&usage), &summary),
            (Some(3), Some("Bash, Edit".to_string()))
        );
    }

    #[test]
    fn ohne_ergebnis_objekt_bleiben_ablehnungen_leer() {
        assert_eq!(denied(None, &OutputSummary::default()), (None, None));
    }

    #[test]
    fn ohne_ablehnungen_zaehlt_null_ohne_werkzeuge() {
        let usage = RunUsage::default();
        assert_eq!(
            denied(Some(&usage), &OutputSummary::default()),
            (Some(0), None)
        );
    }

    #[test]
    fn gleiche_zeiten_ergeben_keine_dauer() {
        let at = "2026-01-01T10:00:00.000Z";
        assert_eq!(duration_ms(at, at), None);
        assert_eq!(duration_ms(at, "2026-01-01T10:04:00.000Z"), Some(240_000));
        // Ein Ende vor dem Start wird wie in TypeScript auf 0 (also „unbekannt“) begrenzt
        assert_eq!(duration_ms("2026-01-01T10:04:00.000Z", at), None);
    }

    #[test]
    fn verbrauch_je_modell_wird_wie_in_javascript_geschrieben() {
        let mut usage = RunUsage::default();
        usage.model_usage.insert(
            "claude-opus-5-5".into(),
            ModelUsage {
                input_tokens: Some(12.0),
                output_tokens: Some(3400.0),
                cache_read_tokens: None,
                cache_write_tokens: None,
                cost_usd: Some(0.4),
            },
        );
        assert_eq!(
            model_usage_json(&usage).as_deref(),
            Some(
                r#"{"claude-opus-5-5":{"inputTokens":12,"outputTokens":3400,"cacheReadTokens":null,"cacheWriteTokens":null,"costUsd":0.4}}"#
            )
        );
    }

    #[test]
    fn ohne_modelle_bleibt_der_verbrauch_leer() {
        assert_eq!(model_usage_json(&RunUsage::default()), None);
    }
}
