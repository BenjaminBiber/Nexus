//! Suche über alle Projekte und Boards; portiert aus
//! `packages/db/src/sqlite/SqliteSearchRepository.ts`. Die Funktion `harness_lower`, die jede
//! Abfrage der Suche braucht, meldet [`crate::sqlite::SqliteBoardDatabase`] schon beim Öffnen an.
//!
//! SQLite faltet Groß/Klein nur für ASCII (`lower`, `LIKE`, `NOCASE`) – „ärger“ fände „Ärger“
//! also nicht. Deshalb filtert SQL über `harness_lower`, das `to_lowercase` aufruft, und liefert
//! je Quelle nur Treffer (neueste zuerst, eine Abfrage mit LIMIT); Ausschnitt und Positionen
//! rechnet [`BoardSearchMatch`]. Die Suchbegriffe gehen nur als Parameter ins SQL.

use std::sync::LazyLock;

use async_trait::async_trait;

use nexus_core::board::database::BoardSearchRepository;
use nexus_core::board::search_match::BoardSearchMatch;
use nexus_core::board::search_types::{
    SearchField, SearchKind, SearchRequest, SearchRow, SearchRows,
};
use nexus_core::text::utf16_prefix;
use nexus_core::Result;

use super::dispatch_run::namen;
use crate::sqlite::runner::{iso, Params, SqlValue};
use crate::sqlite::unit_of_work::SqliteUnitOfWork;
use crate::sqlite_params;

/// Name der SQL-Funktion mit der Groß/Klein-Faltung von JavaScript (`toLowerCase`, Unicode)
pub const SEARCH_LOWER: &str = "harness_lower";

/// Höchstlänge des Titels einer Notiz (UTF-16-Einheiten), wie `slice(0, 200)` in TypeScript
const NOTE_TITLE_MAX: usize = 200;

/// Parameternamen der Suchbegriffe (`t0`, `t1`, …); der Runner verlangt Namen mit statischer
/// Lebensdauer, deshalb einmal je Prozess angelegt
static TERM_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| namen("t"));

/// Die Spalten einer Ticket-Quelle
macro_rules! ticket_cols {
    () => {
        "t.Id AS Id, t.Id AS TicketId, t.Title AS Title, t.ProjectId AS ProjectId, t.BoardId AS BoardId, t.Status AS Status"
    };
}

/// Die Spalten einer Fragen-Quelle
macro_rules! question_cols {
    () => {
        "q.Id AS Id, NULL AS TicketId, q.Title AS Title, q.ProjectId AS ProjectId, NULL AS BoardId, q.State AS Status"
    };
}

/// Eine Quelle: Spalten, durchsuchter Text, Tabellen samt fester Bedingung und die Sortierung
/// „neueste zuerst“
struct Source {
    kind: SearchKind,
    field: SearchField,
    cols: &'static str,
    /// Ausdruck des durchsuchten Textes; wird als Val geliefert und im WHERE gefiltert
    val: &'static str,
    from: &'static str,
    where_: Option<&'static str>,
    order: &'static str,
}

/// Ein durchsuchtes Feld eines Tickets
const fn ticket_field(field: SearchField, column: &'static str) -> Source {
    Source {
        kind: SearchKind::Ticket,
        field,
        cols: concat!(ticket_cols!(), ", NULL AS At"),
        val: column,
        from: "Tickets t",
        where_: None,
        order: "t.CreatedAt DESC, t.Id DESC",
    }
}

/// Quellen in derselben festen Reihenfolge wie `MssqlSearchRepository`
const SOURCES: [Source; 11] = [
    ticket_field(SearchField::Title, "t.Title"),
    ticket_field(SearchField::Description, "t.Description"),
    ticket_field(SearchField::Acceptance, "t.Acceptance"),
    ticket_field(SearchField::Plan, "t.\"Plan\""),
    ticket_field(SearchField::Handbook, "t.Handbook"),
    Source {
        kind: SearchKind::Ticket,
        field: SearchField::Comment,
        cols: concat!(ticket_cols!(), ", c.CreatedAt AS At"),
        val: "c.Body",
        from: "Comments c JOIN Tickets t ON t.Id = c.TicketId",
        where_: None,
        order: "c.Id DESC",
    },
    Source {
        kind: SearchKind::Question,
        field: SearchField::Title,
        cols: concat!(question_cols!(), ", NULL AS At"),
        val: "q.Title",
        from: "Questions q",
        where_: None,
        order: "q.CreatedAt DESC, q.Id DESC",
    },
    Source {
        kind: SearchKind::Question,
        field: SearchField::Description,
        cols: concat!(question_cols!(), ", NULL AS At"),
        val: "q.Description",
        from: "Questions q",
        where_: None,
        order: "q.CreatedAt DESC, q.Id DESC",
    },
    Source {
        // CONCAT behandelt NULL in SQL Server als leeren Text, || in SQLite nicht – deshalb coalesce
        kind: SearchKind::Question,
        field: SearchField::Decision,
        cols: concat!(question_cols!(), ", q.DecidedAt AS At"),
        val: "coalesce(q.DecisionText, '') || char(10) || coalesce(q.DecisionReason, '')",
        from: "Questions q",
        where_: Some("q.DecisionOutcome IS NOT NULL"),
        order: "q.CreatedAt DESC, q.Id DESC",
    },
    Source {
        kind: SearchKind::Question,
        field: SearchField::QuestionComment,
        cols: concat!(question_cols!(), ", qc.CreatedAt AS At"),
        val: "qc.Body",
        from: "QuestionComments qc JOIN Questions q ON q.Id = qc.QuestionId",
        where_: None,
        order: "qc.Id DESC",
    },
    Source {
        kind: SearchKind::Note,
        field: SearchField::Note,
        cols: "CAST(n.Id AS TEXT) AS Id, n.TicketId AS TicketId, CASE WHEN n.Title = '' THEN n.Body ELSE n.Title END AS Title, b.ProjectId AS ProjectId,
      n.BoardId AS BoardId, n.State AS Status, n.CreatedAt AS At",
        val: "n.Title || char(10) || n.Body",
        from: "Notes n JOIN Boards b ON b.Id = n.BoardId",
        where_: None,
        order: "n.Id DESC",
    },
];

/// Eine gelesene Zeile einer Quelle samt durchsuchtem Text
struct Row {
    id: String,
    ticket_id: Option<String>,
    title: String,
    project_id: i32,
    board_id: Option<i32>,
    status: String,
    at: Option<String>,
    val: Option<String>,
}

#[async_trait]
impl BoardSearchRepository for SqliteUnitOfWork<'_> {
    async fn find(&mut self, request: &SearchRequest) -> Result<SearchRows> {
        let sources: Vec<&Source> = SOURCES
            .iter()
            .filter(|source| request.kinds.contains(&source.kind))
            .collect();
        if sources.is_empty() || request.terms.is_empty() {
            return Ok(SearchRows {
                rows: Vec::new(),
                truncated: false,
            });
        }
        let per_source = usize::try_from(request.per_source).unwrap_or(0);
        let match_ = BoardSearchMatch::new(&request.terms);
        let mut rows = Vec::new();
        let mut truncated = false;
        for source in sources {
            // Ein Treffer mehr als nötig, um die Kürzung zu erkennen
            let mut found = self.collect(source, &request.terms, &match_, per_source + 1)?;
            if found.len() > per_source {
                truncated = true;
                found.truncate(per_source);
            }
            rows.extend(found);
        }
        Ok(SearchRows { rows, truncated })
    }
}

impl SqliteUnitOfWork<'_> {
    /// Treffer einer Quelle: alle Begriffe im selben Feld; `instr` statt `LIKE`, damit `%` und
    /// `_` wörtlich zählen
    fn collect(
        &mut self,
        source: &Source,
        terms: &[String],
        match_: &BoardSearchMatch,
        limit: usize,
    ) -> Result<Vec<SearchRow>> {
        let mut params: Params = sqlite_params! { "take": limit as i64 };
        let mut conditions: Vec<String> =
            source.where_.into_iter().map(ToString::to_string).collect();
        for (index, term) in terms.iter().enumerate() {
            let name = TERM_NAMES[index];
            params.push((
                std::borrow::Cow::Borrowed(name),
                SqlValue::Text(term.to_lowercase()),
            ));
            conditions.push(format!(
                "instr({SEARCH_LOWER}({}), @{name}) > 0",
                source.val
            ));
        }
        let sql = format!(
            "SELECT {}, {} AS Val FROM {} WHERE {} ORDER BY {} LIMIT @take",
            source.cols,
            source.val,
            source.from,
            conditions.join(" AND "),
            source.order
        );
        let rows = self.sql.query_rows(&sql, &params, |row| {
            Ok(Row {
                id: row.get(0)?,
                ticket_id: row.get(1)?,
                title: row.get(2)?,
                project_id: row.get(3)?,
                board_id: row.get(4)?,
                status: row.get(5)?,
                at: row.get(6)?,
                val: row.get(7)?,
            })
        })?;
        Ok(rows
            .into_iter()
            .filter(|row| match_.matches(row.val.as_deref()))
            .map(|row| {
                let excerpt = match_.excerpt(row.val.as_deref().unwrap_or_default());
                SearchRow {
                    kind: source.kind,
                    id: row.id,
                    ticket_id: row.ticket_id,
                    field: source.field,
                    title: if source.kind == SearchKind::Note {
                        utf16_prefix(&row.title, NOTE_TITLE_MAX).to_string()
                    } else {
                        row.title
                    },
                    project_id: row.project_id,
                    board_id: row.board_id,
                    status: row.status,
                    text: excerpt.text,
                    text_start: excerpt.text_start,
                    text_length: excerpt.text_length,
                    at: iso(row.at),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_quellen_stehen_in_der_reihenfolge_des_mssql_repositorys() {
        let reihenfolge: Vec<(SearchKind, SearchField)> =
            SOURCES.iter().map(|s| (s.kind, s.field)).collect();
        assert_eq!(
            reihenfolge,
            [
                (SearchKind::Ticket, SearchField::Title),
                (SearchKind::Ticket, SearchField::Description),
                (SearchKind::Ticket, SearchField::Acceptance),
                (SearchKind::Ticket, SearchField::Plan),
                (SearchKind::Ticket, SearchField::Handbook),
                (SearchKind::Ticket, SearchField::Comment),
                (SearchKind::Question, SearchField::Title),
                (SearchKind::Question, SearchField::Description),
                (SearchKind::Question, SearchField::Decision),
                (SearchKind::Question, SearchField::QuestionComment),
                (SearchKind::Note, SearchField::Note),
            ]
        );
    }

    #[test]
    fn der_plan_bleibt_in_anfuhrungszeichen() {
        assert_eq!(SOURCES[3].val, "t.\"Plan\"");
        assert!(SOURCES[8].where_ == Some("q.DecisionOutcome IS NOT NULL"));
    }
}
