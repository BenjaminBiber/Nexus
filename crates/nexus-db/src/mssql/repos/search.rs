//! Suche über alle Projekte und Boards auf SQL Server; portiert aus
//! `packages/db/src/MssqlSearchRepository.ts`.
//!
//! Die Suche verlangt keine Erweiterung: fehlt `note-structure`, lässt sie die Notizen weg
//! (`gate.has("note-structure")`), statt die ganze Suche scheitern zu lassen.
//!
//! Gesucht wird in **einer** Abfrage: je Quelle eine Teilabfrage mit `TOP (@take)`, alle über
//! `UNION ALL` verbunden. Übertragen wird nur ein Fenster um die erste Fundstelle des ersten
//! Begriffs – die Felder sind `nvarchar(max)`, das ganze Feld zu holen wäre teuer.
//! `CHARINDEX` mit der Collation `Latin1_General_100_CI_AS` macht Groß/Klein egal und braucht
//! kein Maskieren von `%`, `_` und `[`.

use async_trait::async_trait;
use nexus_core::board::database::BoardSearchRepository;
use nexus_core::board::search_types::{
    SearchField, SearchKind, SearchRequest, SearchRow, SearchRows, SEARCH_WINDOW,
};
use nexus_core::Result;

use crate::mssql::runner::MsValue;
use crate::mssql::unit_of_work::MssqlUnitOfWork;

/// Groß/Klein egal, Akzente zählen (wie die Suche im Speicher)
const COLLATE: &str = "COLLATE Latin1_General_100_CI_AS";

/// Die Erweiterung, ohne die `board.Notes` die Spalten der Notiz-Quelle fehlen
const NOTE_FEATURE: &str = "note-structure";

/// Eine Quelle: die Spalten des Treffers, der durchsuchte Wert, die Tabellen und die
/// Sortierung „neueste zuerst“
struct Source {
    kind: SearchKind,
    field: SearchField,
    /// FROM-Teil mit Alias
    from: &'static str,
    value: &'static str,
    id: &'static str,
    ticket_id: &'static str,
    title: &'static str,
    project_id: &'static str,
    board_id: &'static str,
    status: &'static str,
    at: &'static str,
    order: &'static str,
}

/// Ein durchsuchtes Feld eines Tickets
const fn ticket_field(field: SearchField, value: &'static str) -> Source {
    Source {
        kind: SearchKind::Ticket,
        field,
        from: "board.Tickets t",
        value,
        id: "t.Id",
        ticket_id: "t.Id",
        title: "t.Title",
        project_id: "t.ProjectId",
        board_id: "t.BoardId",
        status: "t.Status",
        at: "CAST(NULL AS datetime2(3))",
        order: "t.CreatedAt DESC, t.Id DESC",
    }
}

/// Ein durchsuchtes Feld einer Frage
const fn question_field(field: SearchField, value: &'static str) -> Source {
    Source {
        kind: SearchKind::Question,
        field,
        from: "board.Questions q",
        value,
        id: "q.Id",
        ticket_id: "CAST(NULL AS nvarchar(20))",
        title: "q.Title",
        project_id: "q.ProjectId",
        board_id: "CAST(NULL AS int)",
        status: "q.State",
        at: "CAST(NULL AS datetime2(3))",
        order: "q.CreatedAt DESC, q.Id DESC",
    }
}

/// Quellen in fester Reihenfolge; der Index ist die Spalte `Src` der Abfrage
const SOURCES: [Source; 11] = [
    ticket_field(SearchField::Title, "t.Title"),
    ticket_field(SearchField::Description, "t.Description"),
    ticket_field(SearchField::Acceptance, "t.Acceptance"),
    ticket_field(SearchField::Plan, "t.[Plan]"),
    ticket_field(SearchField::Handbook, "t.Handbook"),
    Source {
        from: "board.Comments c JOIN board.Tickets t ON t.Id = c.TicketId",
        value: "c.Body",
        at: "c.CreatedAt",
        order: "c.Id DESC",
        ..ticket_field(SearchField::Comment, "c.Body")
    },
    question_field(SearchField::Title, "q.Title"),
    question_field(SearchField::Description, "q.Description"),
    Source {
        at: "q.DecidedAt",
        ..question_field(
            SearchField::Decision,
            "CASE WHEN q.DecisionOutcome IS NULL THEN NULL ELSE CONCAT(q.DecisionText, NCHAR(10), q.DecisionReason) END",
        )
    },
    Source {
        from: "board.QuestionComments qc JOIN board.Questions q ON q.Id = qc.QuestionId",
        at: "qc.CreatedAt",
        order: "qc.Id DESC",
        ..question_field(SearchField::QuestionComment, "qc.Body")
    },
    Source {
        kind: SearchKind::Note,
        field: SearchField::Note,
        from: "board.Notes n JOIN board.Boards b ON b.Id = n.BoardId",
        value: "CONCAT(n.Title, NCHAR(10), n.Body)",
        id: "CAST(n.Id AS nvarchar(20))",
        ticket_id: "n.TicketId",
        title: "CASE WHEN n.Title = N'' THEN LEFT(n.Body, 200) ELSE n.Title END",
        project_id: "b.ProjectId",
        board_id: "n.BoardId",
        status: "n.State",
        at: "n.CreatedAt",
        order: "n.Id DESC",
    },
];

#[async_trait]
impl BoardSearchRepository for MssqlUnitOfWork<'_> {
    async fn find(&mut self, request: &SearchRequest) -> Result<SearchRows> {
        // Ohne note-structure fehlen board.Notes die Spalten der Notiz-Quelle: lieber ohne
        // Notizen suchen als gar nicht
        let note_ok = self.gate.has(NOTE_FEATURE);
        let sources: Vec<(usize, &Source)> = SOURCES
            .iter()
            .enumerate()
            .filter(|(_, source)| {
                request.kinds.contains(&source.kind) && (source.kind != SearchKind::Note || note_ok)
            })
            .collect();
        if sources.is_empty() || request.terms.is_empty() {
            return Ok(SearchRows {
                rows: Vec::new(),
                truncated: false,
            });
        }
        // Ein Treffer mehr als nötig je Quelle, um die Kürzung zu erkennen
        let mut params = vec![
            ("take", MsValue::Int(request.per_source.saturating_add(1))),
            ("win", MsValue::Int(SEARCH_WINDOW as i32)),
        ];
        let namen: Vec<String> = (0..request.terms.len())
            .map(|index| format!("t{index}"))
            .collect();
        for (name, term) in namen.iter().zip(&request.terms) {
            // Die Begriffe gehen nicht kleingeschrieben ins SQL; die Collation macht
            // Groß/Klein egal
            params.push((name.as_str(), MsValue::text(term.clone())));
        }
        let parts: Vec<String> = sources
            .iter()
            .map(|(index, source)| teil(*index, source, &namen))
            .collect();
        let sql = format!(
            "WITH hits AS ({})
       SELECT Src, Id, TicketId, Title, ProjectId, BoardId, Status, At,
         SUBSTRING(Val, w.Start, @win * 2 + LEN(@t0)) AS Txt, w.Start - 1 AS TextStart, DATALENGTH(Val) / 2 AS TextLength
       FROM hits CROSS APPLY (SELECT CASE WHEN CHARINDEX(@t0, Val {COLLATE}) > @win THEN CHARINDEX(@t0, Val {COLLATE}) - @win ELSE 1 END AS Start) w
       ORDER BY Src, Rn",
            parts.join("\nUNION ALL\n")
        );
        let rows = self.sql.query_rows(&sql, &params).await?;

        let per_source = request.per_source.max(0) as usize;
        let mut counts = vec![0usize; SOURCES.len()];
        let mut result: Vec<SearchRow> = Vec::new();
        let mut truncated = false;
        for row in &rows {
            let src = usize::try_from(row.get_i32("Src")?).unwrap_or(0);
            let source = SOURCES.get(src).ok_or_else(|| {
                nexus_core::Error::unexpected_message(format!(
                    "Die Suche liefert die unbekannte Quelle {src}."
                ))
            })?;
            counts[src] += 1;
            if counts[src] > per_source {
                truncated = true;
                continue;
            }
            result.push(SearchRow {
                kind: source.kind,
                id: row.get_str("Id")?,
                ticket_id: row.get_opt_str("TicketId")?,
                field: source.field,
                title: row.get_str("Title")?,
                project_id: row.get_i32("ProjectId")?,
                board_id: row.get_opt_i32("BoardId")?,
                status: row.get_str("Status")?,
                text: row.get_str("Txt")?,
                text_start: row.get_i32("TextStart")?,
                // DATALENGTH liefert bigint oder int, je nach Feld
                text_length: i32::try_from(row.get_i64("TextLength")?).unwrap_or(i32::MAX),
                at: row.get_opt_iso("At")?,
            });
        }
        Ok(SearchRows {
            rows: result,
            truncated,
        })
    }
}

/// Die Teilabfrage einer Quelle: `TOP (@take)` samt Zeilennummer, in eine Unterabfrage gepackt
/// (nur dort ist `ORDER BY` mit `TOP` erlaubt)
fn teil(index: usize, source: &Source, namen: &[String]) -> String {
    let where_ = namen
        .iter()
        .map(|name| format!("CHARINDEX(@{name}, {} {COLLATE}) > 0", source.value))
        .collect::<Vec<_>>()
        .join(" AND ");
    format!(
        "SELECT * FROM (SELECT TOP (@take) {index} AS Src, {} AS Id, {} AS TicketId, {} AS Title,
        {} AS ProjectId, {} AS BoardId, {} AS Status, {} AS At, {} AS Val,
        ROW_NUMBER() OVER (ORDER BY {}) AS Rn
        FROM {} WHERE {} IS NOT NULL AND {where_} ORDER BY {}) AS p{index}",
        source.id,
        source.ticket_id,
        source.title,
        source.project_id,
        source.board_id,
        source.status,
        source.at,
        source.value,
        source.order,
        source.from,
        source.value,
        source.order,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_quellen_stehen_in_der_reihenfolge_von_typescript() {
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
    fn der_plan_bleibt_in_klammern_und_der_kommentar_sortiert_nach_id() {
        assert_eq!(SOURCES[3].value, "t.[Plan]");
        assert_eq!(SOURCES[5].order, "c.Id DESC");
        assert_eq!(SOURCES[5].at, "c.CreatedAt");
    }

    #[test]
    fn jede_teilabfrage_filtert_alle_begriffe_im_selben_feld() {
        let sql = teil(0, &SOURCES[0], &["t0".to_string(), "t1".to_string()]);
        assert!(sql.contains("TOP (@take) 0 AS Src"));
        assert!(sql.contains(
            "CHARINDEX(@t0, t.Title COLLATE Latin1_General_100_CI_AS) > 0 AND CHARINDEX(@t1, t.Title COLLATE Latin1_General_100_CI_AS) > 0"
        ));
        assert!(sql.ends_with("ORDER BY t.CreatedAt DESC, t.Id DESC) AS p0"));
    }
}
