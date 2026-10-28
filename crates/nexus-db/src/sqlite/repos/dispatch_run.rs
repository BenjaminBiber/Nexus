//! Läufe des Dispatchers und die Fassungen seiner Einstellung; portiert aus
//! `packages/db/src/sqlite/SqliteDispatchRunRepository.ts`.

use std::sync::LazyLock;

use async_trait::async_trait;
use rusqlite::types::Type;
use rusqlite::Row;

use nexus_core::board::database::DispatchRunRepository;
use nexus_core::board::dispatch_run_types::{
    DispatchConfigVersion, DispatchProfileReason, DispatchRun, DispatchRunFilter, DispatchRunKind,
    DispatchRunOutcome, DispatchRunSource,
};
use nexus_core::text::utf16_prefix;
use nexus_core::{HarnessError, Result};

use crate::sqlite::runner::{bool_of, date, iso, iso_millis, ParamName, Params, SqlValue};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;
use crate::SqlChunks;

/// Die gelesenen und geschriebenen Spalten in fester Reihenfolge (wortgleich zu TypeScript)
const COLUMNS: &str = "Id, Kind, SubjectId, ProjectId, BoardId, Agent, Attempt, Profile, EscalatedFrom, RequestedModel, Model, Effort, \
StartedAt, EndedAt, DurationMs, NumTurns, ExitCode, TimedOut, Outcome, AbortCategory, AbortReason, CostUsd, InputTokens, OutputTokens, \
CacheReadTokens, CacheWriteTokens, ModelUsage, Source, MaxTurns, TimeoutMinutes, ProfileReason, RuleIndex, ConfigHash, DeniedCount, DeniedTools";

/// Höchstlänge von `AbortReason` und `DeniedTools` beim Schreiben (wie `slice(0, 500)` in
/// TypeScript, gezählt in UTF-16-Einheiten)
const MAX_TEXT: usize = 500;

/// Größte Zahl, die in die Spalte `DurationMs` gehört (int in SQL Server)
const MAX_INT: i32 = 2_147_483_647;

/// Parameternamen einer IN-Liste (`i0`…`i499`); der Runner verlangt Namen mit statischer
/// Lebensdauer, deshalb einmal je Prozess angelegt
static IN_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| namen("i"));

/// `prefix0`…`prefix<SQL_CHUNK_SIZE-1>` als Namen mit statischer Lebensdauer
pub(crate) fn namen(prefix: &str) -> Vec<&'static str> {
    (0..crate::SQL_CHUNK_SIZE)
        .map(|index| &*Box::leak(format!("{prefix}{index}").into_boxed_str()))
        .collect()
}

/// IN-Liste `@i0, @i1 …` samt Parametern für einen Block
fn in_list(values: &[String]) -> (String, Params) {
    liste(&IN_NAMES, values)
}

/// IN-Liste und Parameter zu den vorbereiteten Namen
pub(crate) fn liste(names: &[&'static str], values: &[String]) -> (String, Params) {
    let list = values
        .iter()
        .enumerate()
        .map(|(index, _)| format!("@{}", names[index]))
        .collect::<Vec<_>>()
        .join(", ");
    let params = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                ParamName::Borrowed(names[index]),
                SqlValue::from(value.clone()),
            )
        })
        .collect();
    (list, params)
}

#[async_trait]
impl DispatchRunRepository for SqliteUnitOfWork<'_> {
    async fn insert(&mut self, run: &DispatchRun) -> Result<bool> {
        let values = COLUMNS
            .split(", ")
            .map(|column| format!("@{column}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO DispatchRuns ({COLUMNS}) VALUES ({values}) ON CONFLICT(Id) DO NOTHING"
        );
        let affected = self.sql.execute(&sql, &params_of(run))?;
        Ok(affected > 0)
    }

    async fn list(&mut self, filter: &DispatchRunFilter) -> Result<Vec<DispatchRun>> {
        let board = match filter.board_id {
            None => "",
            Some(_) => " AND BoardId = @boardId",
        };
        let sql = format!(
            "SELECT {COLUMNS} FROM DispatchRuns WHERE ProjectId = @projectId AND StartedAt >= @from AND StartedAt < @to{board} \
             ORDER BY StartedAt, Id"
        );
        let params = sqlite_params! {
            "projectId": filter.project_id,
            "from": date(&filter.from),
            "to": date(&filter.to),
            "boardId": filter.board_id,
        };
        self.sql.query_rows(&sql, &params, map)
    }

    async fn list_all(&mut self, from: &str, to: &str) -> Result<Vec<DispatchRun>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM DispatchRuns WHERE StartedAt >= @from AND StartedAt < @to ORDER BY StartedAt, Id"
        );
        let params = sqlite_params! { "from": date(from), "to": date(to) };
        self.sql.query_rows(&sql, &params, map)
    }

    async fn first_started(
        &mut self,
        project_id: i32,
        source: DispatchRunSource,
    ) -> Result<Option<String>> {
        let rows = self.sql.query_rows(
            "SELECT MIN(StartedAt) AS At FROM DispatchRuns WHERE ProjectId = @projectId AND Source = @source",
            &sqlite_params! { "projectId": project_id, "source": source.as_str() },
            |row| row.get::<_, Option<String>>(0),
        )?;
        Ok(iso(rows.into_iter().next().flatten()))
    }

    async fn existing(&mut self, ids: &[String]) -> Result<Vec<String>> {
        let mut found = Vec::new();
        for chunk in SqlChunks::of(ids) {
            let (list, params) = in_list(chunk);
            let sql = format!("SELECT Id FROM DispatchRuns WHERE Id IN ({list})");
            found.extend(
                self.sql
                    .query_rows(&sql, &params, |row| row.get::<_, String>(0))?,
            );
        }
        Ok(found)
    }

    async fn record_config_version(&mut self, version: &DispatchConfigVersion) -> Result<bool> {
        let affected = self.sql.execute(
            "INSERT INTO DispatchConfigVersions (Hash, Dispatcher, FirstSeenAt, Content) VALUES (@Hash, @Dispatcher, @FirstSeenAt, @Content) \
             ON CONFLICT(Hash) DO NOTHING",
            &sqlite_params! {
                "Hash": version.hash.clone(),
                "Dispatcher": version.dispatcher.clone(),
                "FirstSeenAt": date(&version.first_seen_at),
                "Content": version.content.clone(),
            },
        )?;
        Ok(affected > 0)
    }

    async fn list_config_versions(
        &mut self,
        from: &str,
        to: &str,
    ) -> Result<Vec<DispatchConfigVersion>> {
        self.sql.query_rows(
            "SELECT Hash, Dispatcher, FirstSeenAt, Content FROM DispatchConfigVersions \
             WHERE FirstSeenAt >= @from AND FirstSeenAt < @to ORDER BY FirstSeenAt, Hash",
            &sqlite_params! { "from": date(from), "to": date(to) },
            |row| {
                Ok(DispatchConfigVersion {
                    hash: row.get(0)?,
                    dispatcher: row.get(1)?,
                    first_seen_at: iso_millis(&row.get::<_, String>(2)?),
                    content: row.get(3)?,
                })
            },
        )
    }
}

/// Die Parameter eines Laufs in der Reihenfolge von [`COLUMNS`]
fn params_of(run: &DispatchRun) -> Params {
    sqlite_params! {
        "Id": run.id.clone(),
        "Kind": run.kind.as_str(),
        "SubjectId": run.subject_id.clone(),
        "ProjectId": run.project_id,
        "BoardId": run.board_id,
        "Agent": run.agent.clone(),
        "Attempt": run.attempt,
        "Profile": run.profile.clone(),
        "EscalatedFrom": run.escalated_from.clone(),
        "RequestedModel": run.requested_model.clone(),
        "Model": run.model.clone(),
        "Effort": run.effort.clone(),
        "StartedAt": date(&run.started_at),
        "EndedAt": run.ended_at.as_deref().map_or(SqlValue::Null, date),
        "DurationMs": int(run.duration_ms),
        "NumTurns": run.num_turns,
        "ExitCode": run.exit_code,
        "TimedOut": run.timed_out,
        "Outcome": run.outcome.as_str(),
        "AbortCategory": run.abort_category.clone(),
        "AbortReason": gekurzt(run.abort_reason.as_deref()),
        "CostUsd": run.cost_usd,
        "InputTokens": run.input_tokens,
        "OutputTokens": run.output_tokens,
        "CacheReadTokens": run.cache_read_tokens,
        "CacheWriteTokens": run.cache_write_tokens,
        "ModelUsage": run.model_usage.clone(),
        "Source": run.source.as_str(),
        "MaxTurns": run.max_turns,
        "TimeoutMinutes": run.timeout_minutes,
        "ProfileReason": run.profile_reason.map(|reason| reason.as_str()),
        "RuleIndex": run.rule_index,
        "ConfigHash": run.config_hash.clone(),
        "DeniedCount": run.denied_count,
        "DeniedTools": gekurzt(run.denied_tools.as_deref()),
    }
}

/// Text auf [`MAX_TEXT`] UTF-16-Einheiten gekürzt; `None` bleibt `None`
fn gekurzt(value: Option<&str>) -> SqlValue {
    SqlValue::from(value.map(|text| utf16_prefix(text, MAX_TEXT).to_string()))
}

/// Dauer passt in int (bis ~24 Tage); größere oder negative Werte sind kaputt und werden
/// nicht gespeichert
fn int(value: Option<i32>) -> SqlValue {
    SqlValue::from(value.filter(|value| (0..=MAX_INT).contains(value)))
}

/// Eine Zeile als Lauf; die Spalten stehen in der Reihenfolge von [`COLUMNS`]
fn map(row: &Row<'_>) -> rusqlite::Result<DispatchRun> {
    Ok(DispatchRun {
        id: row.get(0)?,
        kind: enum_of(
            "Kind",
            &row.get::<_, String>(1)?,
            &DispatchRunKind::ALL,
            |k| k.as_str(),
        )?,
        subject_id: row.get(2)?,
        project_id: row.get(3)?,
        board_id: row.get(4)?,
        agent: row.get(5)?,
        attempt: row.get(6)?,
        profile: row.get(7)?,
        escalated_from: row.get(8)?,
        requested_model: row.get(9)?,
        model: row.get(10)?,
        effort: row.get(11)?,
        started_at: iso_millis(&row.get::<_, String>(12)?),
        ended_at: iso(row.get(13)?),
        duration_ms: row.get(14)?,
        num_turns: row.get(15)?,
        exit_code: row.get(16)?,
        timed_out: bool_of(row.get(17)?),
        outcome: enum_of(
            "Outcome",
            &row.get::<_, String>(18)?,
            &DispatchRunOutcome::ALL,
            |o| o.as_str(),
        )?,
        abort_category: row.get(19)?,
        abort_reason: row.get(20)?,
        cost_usd: row.get(21)?,
        input_tokens: row.get(22)?,
        output_tokens: row.get(23)?,
        cache_read_tokens: row.get(24)?,
        cache_write_tokens: row.get(25)?,
        model_usage: row.get(26)?,
        source: enum_of(
            "Source",
            &row.get::<_, String>(27)?,
            &DispatchRunSource::ALL,
            |s| s.as_str(),
        )?,
        max_turns: row.get(28)?,
        timeout_minutes: row.get(29)?,
        profile_reason: row
            .get::<_, Option<String>>(30)?
            .map(|text| {
                enum_of(
                    "ProfileReason",
                    &text,
                    &DispatchProfileReason::ALL,
                    |reason| reason.as_str(),
                )
            })
            .transpose()?,
        rule_index: row.get(31)?,
        config_hash: row.get(32)?,
        denied_count: row.get(33)?,
        denied_tools: row.get(34)?,
    })
}

/// Der Eintrag, dessen Schreibweise zum Spaltenwert passt; sonst ein Fehler mit Spalte und Wert
fn enum_of<T: Copy>(
    column: &str,
    value: &str,
    all: &[T],
    as_str: fn(&T) -> &'static str,
) -> rusqlite::Result<T> {
    all.iter()
        .find(|entry| as_str(entry) == value)
        .copied()
        .ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                Type::Text,
                Box::new(HarnessError::new(
                    "dispatch-run-invalid",
                    format!("DispatchRuns.{column} kennt den Wert „{value}“ nicht."),
                )),
            )
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
        assert_eq!(int(Some(300_000)), SqlValue::Int(300_000));
        assert_eq!(int(Some(MAX_INT)), SqlValue::Int(i64::from(MAX_INT)));
        assert_eq!(int(Some(-1)), SqlValue::Null);
        assert_eq!(int(None), SqlValue::Null);
    }

    #[test]
    fn lange_texte_werden_auf_500_einheiten_gekurzt() {
        assert_eq!(gekurzt(None), SqlValue::Null);
        let lang = "x".repeat(600);
        assert_eq!(gekurzt(Some(&lang)), SqlValue::Text("x".repeat(500)));
    }

    #[test]
    fn unbekannte_spaltenwerte_melden_spalte_und_wert() {
        let fehler = enum_of("Kind", "quatsch", &DispatchRunKind::ALL, |k| k.as_str())
            .expect_err("unbekannt");
        assert!(fehler.to_string().contains("DispatchRuns.Kind"));
        assert_eq!(
            enum_of("Kind", "merge", &DispatchRunKind::ALL, |k| k.as_str()).unwrap(),
            DispatchRunKind::Merge
        );
    }

    #[test]
    fn die_in_liste_zahlt_ab_null() {
        let (list, params) = in_list(&["a".to_string(), "b".to_string()]);
        assert_eq!(list, "@i0, @i1");
        assert_eq!(params[0].0, "i0");
        assert_eq!(params[1].1, SqlValue::Text("b".into()));
    }
}
