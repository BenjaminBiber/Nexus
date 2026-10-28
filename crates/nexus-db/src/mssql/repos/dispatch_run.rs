//! Läufe des Dispatchers auf SQL Server; portiert aus
//! `packages/db/src/MssqlDispatchRunRepository.ts`.
//!
//! Jeder Zugriff verlangt `dispatch-runs` **und** `dispatch-run-details`: die Abfragen lesen die
//! Spalten aus 006 mit, ohne die Erweiterung scheiterten sie sonst als SQL-Fehler statt mit
//! einem Hinweis auf das Skript.

use async_trait::async_trait;
use nexus_core::board::database::DispatchRunRepository;
use nexus_core::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchProfileReason, DispatchRun, DispatchRunFilter, DispatchRunKind,
    DispatchRunOutcome, DispatchRunSource,
};
use nexus_core::text::utf16_prefix;
use nexus_core::{Error, HarnessError, Result};

use crate::mssql::runner::{MsRow, MsType, MsValue};
use crate::mssql::unit_of_work::MssqlUnitOfWork;
use crate::SqlChunks;

/// Die Erweiterungen, ohne die kein Zugriff auf die Läufe geht (Reihenfolge der Skripte)
const NEEDED: [&str; 2] = ["dispatch-runs", "dispatch-run-details"];

/// Die gelesenen und geschriebenen Spalten in fester Reihenfolge (wortgleich zu TypeScript)
const COLUMNS: &str = "Id, Kind, SubjectId, ProjectId, BoardId, Agent, Attempt, Profile, EscalatedFrom, RequestedModel, Model, Effort, \
StartedAt, EndedAt, DurationMs, NumTurns, ExitCode, TimedOut, Outcome, AbortCategory, AbortReason, CostUsd, InputTokens, OutputTokens, \
CacheReadTokens, CacheWriteTokens, ModelUsage, Source, MaxTurns, TimeoutMinutes, ProfileReason, RuleIndex, ConfigHash, DeniedCount, DeniedTools";

/// Höchstlänge von `AbortReason` und `DeniedTools` beim Schreiben (wie `slice(0, 500)` in
/// TypeScript, gezählt in UTF-16-Einheiten – so breit sind die Spalten)
const MAX_TEXT: usize = 500;

/// Größte Zahl, die in die Spalte `DurationMs` gehört (int)
const MAX_INT: i32 = 2_147_483_647;

/// Nachkommastellen der Spalte `CostUsd` (`decimal(12,6)`)
const COST_SCALE: u8 = 6;

/// Höchstlänge von `Dispatcher` in `board.DispatchConfigVersions` (`nvarchar(100)`)
const MAX_DISPATCHER: usize = 100;

#[async_trait]
impl DispatchRunRepository for MssqlUnitOfWork<'_> {
    async fn insert(&mut self, run: &DispatchRun) -> Result<bool> {
        self.gate.require(&NEEDED)?;
        let values = COLUMNS
            .split(", ")
            .map(|column| format!("@{column}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "IF NOT EXISTS (SELECT 1 FROM board.DispatchRuns WITH (UPDLOCK, HOLDLOCK) WHERE Id = @Id) \
             INSERT INTO board.DispatchRuns ({COLUMNS}) VALUES ({values})"
        );
        let affected = self.sql.execute(&sql, &params_of(run)?).await?;
        Ok(affected > 0)
    }

    async fn list(&mut self, filter: &DispatchRunFilter) -> Result<Vec<DispatchRun>> {
        self.gate.require(&NEEDED)?;
        let board = match filter.board_id {
            None => "",
            Some(_) => " AND BoardId = @boardId",
        };
        let sql = format!(
            "SELECT {COLUMNS} FROM board.DispatchRuns WHERE ProjectId = @projectId AND StartedAt >= @from AND StartedAt < @to{board} \
             ORDER BY StartedAt, Id"
        );
        let mut params = vec![
            ("projectId", MsValue::Int(filter.project_id)),
            ("from", MsValue::date(&filter.from)?),
            ("to", MsValue::date(&filter.to)?),
        ];
        if let Some(board_id) = filter.board_id {
            params.push(("boardId", MsValue::Int(board_id)));
        }
        let rows = self.sql.query_rows(&sql, &params).await?;
        rows.iter().map(map).collect()
    }

    async fn list_all(&mut self, from: &str, to: &str) -> Result<Vec<DispatchRun>> {
        self.gate.require(&NEEDED)?;
        let sql = format!(
            "SELECT {COLUMNS} FROM board.DispatchRuns WHERE StartedAt >= @from AND StartedAt < @to ORDER BY StartedAt, Id"
        );
        let rows = self
            .sql
            .query_rows(
                &sql,
                &[("from", MsValue::date(from)?), ("to", MsValue::date(to)?)],
            )
            .await?;
        rows.iter().map(map).collect()
    }

    async fn first_started(
        &mut self,
        project_id: i32,
        source: DispatchRunSource,
    ) -> Result<Option<String>> {
        self.gate.require(&NEEDED)?;
        let rows = self
            .sql
            .query_rows(
                "SELECT MIN(StartedAt) AS At FROM board.DispatchRuns WHERE ProjectId = @projectId AND Source = @source",
                &[
                    ("projectId", MsValue::Int(project_id)),
                    ("source", MsValue::text(source.as_str())),
                ],
            )
            .await?;
        match rows.first() {
            None => Ok(None),
            Some(row) => row.get_opt_iso("At"),
        }
    }

    async fn existing(&mut self, ids: &[String]) -> Result<Vec<String>> {
        self.gate.require(&NEEDED)?;
        let mut found = Vec::new();
        for chunk in SqlChunks::of(ids) {
            let werte: Vec<MsValue> = chunk.iter().map(|id| MsValue::text(id.clone())).collect();
            let (list, params) = SqlChunks::params("i", &werte);
            let sql = format!("SELECT Id FROM board.DispatchRuns WHERE Id IN ({list})");
            let rows = self.sql.query_rows(&sql, &geliehen(&params)).await?;
            for row in &rows {
                found.push(row.get_str("Id")?);
            }
        }
        Ok(found)
    }

    async fn record_config_version(&mut self, version: &DispatchConfigVersion) -> Result<bool> {
        self.gate.require(&NEEDED)?;
        let affected = self
            .sql
            .execute(
                "IF NOT EXISTS (SELECT 1 FROM board.DispatchConfigVersions WITH (UPDLOCK, HOLDLOCK) WHERE Hash = @Hash) \
                 INSERT INTO board.DispatchConfigVersions (Hash, Dispatcher, FirstSeenAt, Content) VALUES (@Hash, @Dispatcher, @FirstSeenAt, @Content)",
                &[
                    ("Hash", MsValue::VarChar(version.hash.clone())),
                    (
                        "Dispatcher",
                        MsValue::text(utf16_prefix(&version.dispatcher, MAX_DISPATCHER)),
                    ),
                    ("FirstSeenAt", MsValue::date(&version.first_seen_at)?),
                    ("Content", MsValue::text(version.content.clone())),
                ],
            )
            .await?;
        Ok(affected > 0)
    }

    async fn list_config_versions(
        &mut self,
        from: &str,
        to: &str,
    ) -> Result<Vec<DispatchConfigVersion>> {
        self.gate.require(&NEEDED)?;
        let rows = self
            .sql
            .query_rows(
                "SELECT Hash, Dispatcher, FirstSeenAt, Content FROM board.DispatchConfigVersions WHERE FirstSeenAt >= @from AND FirstSeenAt < @to \
                 ORDER BY FirstSeenAt, Hash",
                &[("from", MsValue::date(from)?), ("to", MsValue::date(to)?)],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(DispatchConfigVersion {
                    hash: row.get_str("Hash")?,
                    dispatcher: row.get_str("Dispatcher")?,
                    first_seen_at: row.get_iso("FirstSeenAt")?,
                    content: row.get_str("Content")?,
                })
            })
            .collect()
    }
}

/// Die Paare als geliehene Namen, wie der Runner sie verlangt
pub(crate) fn geliehen(params: &[(String, MsValue)]) -> Vec<(&str, MsValue)> {
    params
        .iter()
        .map(|(name, value)| (name.as_str(), value.clone()))
        .collect()
}

/// Die Parameter eines Laufs in der Reihenfolge von [`COLUMNS`], mit den festen Typen aus
/// TypeScript (int, bigint, decimal(12,6), datetime2(3), bit, varchar)
fn params_of(run: &DispatchRun) -> Result<Vec<(&'static str, MsValue)>> {
    Ok(vec![
        ("Id", MsValue::text(run.id.clone())),
        ("Kind", MsValue::text(run.kind.as_str())),
        ("SubjectId", MsValue::text(run.subject_id.clone())),
        ("ProjectId", MsValue::Int(run.project_id)),
        ("BoardId", zahl(run.board_id)),
        ("Agent", MsValue::text(run.agent.clone())),
        ("Attempt", zahl(run.attempt)),
        ("Profile", MsValue::text(run.profile.clone())),
        (
            "EscalatedFrom",
            MsValue::text_opt(run.escalated_from.as_deref()),
        ),
        (
            "RequestedModel",
            MsValue::text_opt(run.requested_model.as_deref()),
        ),
        ("Model", MsValue::text_opt(run.model.as_deref())),
        ("Effort", MsValue::text_opt(run.effort.as_deref())),
        ("StartedAt", MsValue::date(&run.started_at)?),
        ("EndedAt", MsValue::date_opt(run.ended_at.as_deref())?),
        ("DurationMs", zahl(int(run.duration_ms))),
        ("NumTurns", zahl(run.num_turns)),
        ("ExitCode", zahl(run.exit_code)),
        ("TimedOut", MsValue::Bit(run.timed_out)),
        ("Outcome", MsValue::text(run.outcome.as_str())),
        (
            "AbortCategory",
            MsValue::text_opt(run.abort_category.as_deref()),
        ),
        ("AbortReason", gekurzt(run.abort_reason.as_deref())),
        ("CostUsd", kosten(run.cost_usd)),
        ("InputTokens", grosse_zahl(run.input_tokens)),
        ("OutputTokens", grosse_zahl(run.output_tokens)),
        ("CacheReadTokens", grosse_zahl(run.cache_read_tokens)),
        ("CacheWriteTokens", grosse_zahl(run.cache_write_tokens)),
        ("ModelUsage", MsValue::text_opt(run.model_usage.as_deref())),
        ("Source", MsValue::text(run.source.as_str())),
        ("MaxTurns", zahl(run.max_turns)),
        ("TimeoutMinutes", zahl(run.timeout_minutes)),
        (
            "ProfileReason",
            match run.profile_reason {
                Some(reason) => MsValue::VarChar(reason.as_str().to_string()),
                None => MsValue::Null(MsType::VarChar),
            },
        ),
        ("RuleIndex", zahl(run.rule_index)),
        (
            "ConfigHash",
            match run.config_hash.as_deref() {
                Some(hash) => MsValue::VarChar(hash.to_string()),
                None => MsValue::Null(MsType::VarChar),
            },
        ),
        ("DeniedCount", zahl(run.denied_count)),
        ("DeniedTools", gekurzt(run.denied_tools.as_deref())),
    ])
}

/// `int` oder `NULL` mit ausdrücklichem Typ
fn zahl(value: Option<i32>) -> MsValue {
    value.map_or(MsValue::Null(MsType::Int), MsValue::Int)
}

/// `bigint` oder `NULL` mit ausdrücklichem Typ
fn grosse_zahl(value: Option<i64>) -> MsValue {
    value.map_or(MsValue::Null(MsType::BigInt), MsValue::BigInt)
}

/// `decimal(12,6)` oder `NULL` mit ausdrücklichem Typ; als Festkommazahl, nicht als `float`
fn kosten(value: Option<f64>) -> MsValue {
    value.map_or(MsValue::Null(MsType::Decimal(COST_SCALE)), |value| {
        MsValue::Decimal(value, COST_SCALE)
    })
}

/// Text auf [`MAX_TEXT`] UTF-16-Einheiten gekürzt; `None` bleibt `NULL`
fn gekurzt(value: Option<&str>) -> MsValue {
    match value {
        Some(text) => MsValue::NVarChar(utf16_prefix(text, MAX_TEXT).to_string()),
        None => MsValue::Null(MsType::NVarChar),
    }
}

/// Dauer passt in int (bis ~24 Tage); größere oder negative Werte sind kaputt und werden
/// nicht gespeichert
fn int(value: Option<i32>) -> Option<i32> {
    value.filter(|value| (0..=MAX_INT).contains(value))
}

/// Eine Zeile als Lauf; die Spalten heißen wie in [`COLUMNS`]
fn map(row: &MsRow) -> Result<DispatchRun> {
    Ok(DispatchRun {
        id: row.get_str("Id")?,
        kind: enum_of("Kind", &row.get_str("Kind")?, &DispatchRunKind::ALL, |k| {
            k.as_str()
        })?,
        subject_id: row.get_str("SubjectId")?,
        project_id: row.get_i32("ProjectId")?,
        board_id: row.get_opt_i32("BoardId")?,
        agent: row.get_str("Agent")?,
        attempt: row.get_opt_i32("Attempt")?,
        profile: row.get_str("Profile")?,
        escalated_from: row.get_opt_str("EscalatedFrom")?,
        requested_model: row.get_opt_str("RequestedModel")?,
        model: row.get_opt_str("Model")?,
        effort: row.get_opt_str("Effort")?,
        started_at: row.get_iso("StartedAt")?,
        ended_at: row.get_opt_iso("EndedAt")?,
        duration_ms: row.get_opt_i32("DurationMs")?,
        num_turns: row.get_opt_i32("NumTurns")?,
        exit_code: row.get_opt_i32("ExitCode")?,
        timed_out: row.get_opt_bool("TimedOut")?.unwrap_or(false),
        outcome: enum_of(
            "Outcome",
            &row.get_str("Outcome")?,
            &DispatchRunOutcome::ALL,
            |o| o.as_str(),
        )?,
        abort_category: row.get_opt_str("AbortCategory")?,
        abort_reason: row.get_opt_str("AbortReason")?,
        cost_usd: row.get_opt_f64("CostUsd")?,
        input_tokens: row.get_opt_i64("InputTokens")?,
        output_tokens: row.get_opt_i64("OutputTokens")?,
        cache_read_tokens: row.get_opt_i64("CacheReadTokens")?,
        cache_write_tokens: row.get_opt_i64("CacheWriteTokens")?,
        model_usage: row.get_opt_str("ModelUsage")?,
        source: enum_of(
            "Source",
            &row.get_str("Source")?,
            &DispatchRunSource::ALL,
            |s| s.as_str(),
        )?,
        max_turns: row.get_opt_i32("MaxTurns")?,
        timeout_minutes: row.get_opt_i32("TimeoutMinutes")?,
        profile_reason: row
            .get_opt_str("ProfileReason")?
            .map(|text| {
                enum_of(
                    "ProfileReason",
                    &text,
                    &DispatchProfileReason::ALL,
                    |reason| reason.as_str(),
                )
            })
            .transpose()?,
        rule_index: row.get_opt_i32("RuleIndex")?,
        config_hash: row.get_opt_str("ConfigHash")?,
        denied_count: row.get_opt_i32("DeniedCount")?,
        denied_tools: row.get_opt_str("DeniedTools")?,
    })
}

/// Der Eintrag, dessen Schreibweise zum Spaltenwert passt; sonst ein Fehler mit Spalte und Wert
fn enum_of<T: Copy>(
    column: &str,
    value: &str,
    all: &[T],
    as_str: fn(&T) -> &'static str,
) -> Result<T> {
    all.iter()
        .find(|entry| as_str(entry) == value)
        .copied()
        .ok_or_else(|| {
            Error::from(HarnessError::new(
                "dispatch-run-invalid",
                format!("DispatchRuns.{column} kennt den Wert „{value}“ nicht."),
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_spaltenliste_bleibt_wortgleich_zu_typescript() {
        assert_eq!(COLUMNS.split(", ").count(), 35);
        assert!(COLUMNS.starts_with("Id, Kind, SubjectId"));
        assert!(COLUMNS.ends_with("DeniedCount, DeniedTools"));
    }

    #[test]
    fn eine_kaputte_dauer_wird_nicht_gespeichert() {
        assert_eq!(int(Some(300_000)), Some(300_000));
        assert_eq!(int(Some(MAX_INT)), Some(MAX_INT));
        assert_eq!(int(Some(-1)), None);
        assert_eq!(int(None), None);
    }

    #[test]
    fn lange_texte_werden_auf_500_einheiten_gekurzt() {
        assert_eq!(gekurzt(None), MsValue::Null(MsType::NVarChar));
        let lang = "x".repeat(600);
        assert_eq!(
            gekurzt(Some(&lang)),
            MsValue::NVarChar("x".repeat(MAX_TEXT))
        );
    }

    #[test]
    fn die_kosten_gehen_als_festkommazahl_mit_sechs_stellen() {
        assert_eq!(kosten(Some(1.25)), MsValue::Decimal(1.25, 6));
        assert_eq!(kosten(None), MsValue::Null(MsType::Decimal(6)));
    }

    #[test]
    fn unbekannte_spaltenwerte_melden_spalte_und_wert() {
        let fehler = enum_of("Kind", "quatsch", &DispatchRunKind::ALL, |k| k.as_str())
            .expect_err("unbekannt");
        assert_eq!(fehler.code(), Some("dispatch-run-invalid"));
        assert!(fehler.to_string().contains("DispatchRuns.Kind"));
        assert_eq!(
            enum_of("Kind", "merge", &DispatchRunKind::ALL, |k| k.as_str()).unwrap(),
            DispatchRunKind::Merge
        );
    }
}
