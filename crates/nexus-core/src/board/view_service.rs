//! Ansicht eines Boards für die Oberfläche: alle Spalten (auch leere) in fester Reihenfolge,
//! Karten mit Kommentarzahl. Portiert aus `packages/core/src/board/BoardViewService.ts`.
//!
//! Eine Karte trägt die langen Texte (Beschreibung, Abnahme, Plan, Handbuch, Diagramm) nicht,
//! sondern nur die Angabe, ob es sie gibt – das Detail lädt die Oberfläche selbst. Deshalb
//! listet [`BoardCard`] die Felder einzeln auf, statt [`TicketView`] einzubetten.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::board::basics::{Priority, TicketStatus};
use crate::board::board_service::BoardService;
use crate::board::database::{BoardDatabase, TicketFilter};
use crate::board::question_types::BoardOpenQuestion;
use crate::board::ticket_ordering::{BoardColumn, COLUMN_ORDER};
use crate::board::ticket_service::{self, TicketView};
use crate::clock::Clock;
use crate::error::Result;
use crate::tx;

/// Eine Karte des Boards: ein Ticket ohne seine langen Texte, mit Kommentarzahl
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct BoardCard {
    pub id: String,
    pub project_id: i32,
    pub board_id: i32,
    pub number: i32,
    pub title: String,
    pub status: TicketStatus,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub depends_on: Vec<String>,
    pub sort_order: i32,
    pub assignee: Option<String>,
    pub needs_planning: bool,
    pub spec: Option<String>,
    pub spec_task: Option<String>,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub target_branch: Option<String>,
    pub touches: Vec<String>,
    pub consumes: Vec<String>,
    pub produces: Vec<String>,
    pub blocked_reason: Option<String>,
    pub planned_at: Option<String>,
    pub planned_by: Option<String>,
    pub planning_claim_by: Option<String>,
    pub planning_claim_at: Option<String>,
    pub created_at: String,
    pub created_by_id: String,
    pub created_by_name: String,
    pub updated_at: String,
    pub rev: i32,
    pub column: BoardColumn,
    pub waiting: bool,
    pub depends_open: Vec<String>,
    pub depends_unmerged: Vec<String>,
    pub dependents: Vec<String>,
    pub unlocks: Vec<String>,
    pub waiting_on_questions: Vec<BoardOpenQuestion>,
    pub steps_done: i32,
    pub steps_total: i32,
    pub sessions: Vec<String>,
    pub comment_count: i64,
    pub has_description: bool,
    pub has_acceptance: bool,
    pub has_plan: bool,
    pub has_handbook: bool,
    pub has_diagram: bool,
}

impl BoardCard {
    /// Karte aus einer Anzeigefassung; die langen Texte werden zu „vorhanden“
    fn of(view: TicketView, comment_count: i64) -> Self {
        let has = |text: &str| !text.trim().is_empty();
        let TicketView {
            ticket,
            column,
            waiting,
            depends_open,
            depends_unmerged,
            dependents,
            unlocks,
            waiting_on_questions,
            steps_done,
            steps_total,
            sessions,
        } = view;
        Self {
            has_description: has(&ticket.description),
            has_acceptance: has(&ticket.acceptance),
            has_plan: has(&ticket.plan),
            has_handbook: has(&ticket.handbook),
            has_diagram: has(&ticket.diagram),
            id: ticket.id,
            project_id: ticket.project_id,
            board_id: ticket.board_id,
            number: ticket.number,
            title: ticket.title,
            status: ticket.status,
            priority: ticket.priority,
            labels: ticket.labels,
            depends_on: ticket.depends_on,
            sort_order: ticket.sort_order,
            assignee: ticket.assignee,
            needs_planning: ticket.needs_planning,
            spec: ticket.spec,
            spec_task: ticket.spec_task,
            branch: ticket.branch,
            worktree: ticket.worktree,
            target_branch: ticket.target_branch,
            touches: ticket.touches,
            consumes: ticket.consumes,
            produces: ticket.produces,
            blocked_reason: ticket.blocked_reason,
            planned_at: ticket.planned_at,
            planned_by: ticket.planned_by,
            planning_claim_by: ticket.planning_claim_by,
            planning_claim_at: ticket.planning_claim_at,
            created_at: ticket.created_at,
            created_by_id: ticket.created_by_id,
            created_by_name: ticket.created_by_name,
            updated_at: ticket.updated_at,
            rev: ticket.rev,
            column,
            waiting,
            depends_open,
            depends_unmerged,
            dependents,
            unlocks,
            waiting_on_questions,
            steps_done,
            steps_total,
            sessions,
            comment_count,
        }
    }
}

/// Eine Spalte der Board-Ansicht
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct BoardViewColumn {
    pub id: BoardColumn,
    pub title: String,
    pub tickets: Vec<BoardCard>,
}

/// Die ganze Ansicht eines Boards
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct BoardView {
    pub board_id: i32,
    pub columns: Vec<BoardViewColumn>,
}

/// Baut die Ansicht eines Boards.
#[derive(Debug, Clone)]
pub struct BoardViewService<D: BoardDatabase> {
    db: Arc<D>,
}

impl<D: BoardDatabase> BoardViewService<D> {
    /// Dienst auf dieser Datenbank; die Uhr wird nur der Einheitlichkeit halber genommen
    pub fn new(db: Arc<D>, _clock: Clock) -> Self {
        Self { db }
    }

    /// Liest das Board in einer Transaktion; unbekannt ergibt `board-not-found`
    pub async fn get(&self, board_id: i32) -> Result<BoardView> {
        tx!(self.db, |u| {
            BoardService::<D>::require_in(u, board_id).await?;
            let tickets = u
                .tickets()
                .list(&TicketFilter {
                    board_ids: Some(vec![board_id]),
                    without_texts: true,
                    ..TicketFilter::default()
                })
                .await?;
            let mut views = ticket_service::views(u, tickets).await?;
            ticket_service::sort_views(&mut views);
            let counts = u.comments().count_by_board(board_id).await?;
            let cards: Vec<BoardCard> = views
                .into_iter()
                .map(|view| {
                    let count = counts.get(&view.ticket.id).copied().unwrap_or(0);
                    BoardCard::of(view, count)
                })
                .collect();
            Ok(BoardView {
                board_id,
                columns: COLUMN_ORDER
                    .iter()
                    .map(|column| BoardViewColumn {
                        id: *column,
                        title: column.label().to_string(),
                        tickets: cards
                            .iter()
                            .filter(|card| card.column == *column)
                            .cloned()
                            .collect(),
                    })
                    .collect(),
            })
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::board::basics::{Priority, TicketStatus};
    use crate::board::ticket_ordering::BoardColumn;
    use crate::board::types::Ticket;
    use crate::testing::BoardFixture;

    /// Die vier Tickets des TypeScript-Tests, direkt über die Repositories angelegt
    async fn seed(f: &BoardFixture) {
        let a = f.new_ticket(1).await;
        f.insert_ticket(&a).await;
        f.insert_ticket(&Ticket {
            title: "Dringend".into(),
            priority: Priority::Urgent,
            ..f.new_ticket(2).await
        })
        .await;
        f.insert_ticket(&Ticket {
            title: "Wartet".into(),
            depends_on: vec!["ARC-0001".into()],
            ..f.new_ticket(3).await
        })
        .await;
        f.insert_ticket(&Ticket {
            title: "Läuft".into(),
            status: TicketStatus::InProgress,
            ..f.new_ticket(4).await
        })
        .await;
    }

    #[tokio::test]
    async fn liefert_alle_spalten_in_fester_reihenfolge_mit_sortierten_tickets_und_kommentarzahl() {
        let f = BoardFixture::create().await;
        seed(&f).await;
        f.comments
            .add("ARC-0001", "Hallo", &f.actor, false)
            .await
            .unwrap();

        let view = f.views.get(f.board_id).await.unwrap();

        assert_eq!(view.board_id, f.board_id);
        assert_eq!(
            view.columns
                .iter()
                .map(|column| (column.id.as_str(), column.title.as_str()))
                .collect::<Vec<_>>(),
            [
                ("backlog", "Backlog"),
                ("waiting", "Wartet"),
                ("in_planning", "In Planung"),
                ("planned", "Geplant"),
                ("in_progress", "In Arbeit"),
                ("blocked", "Blockiert"),
                ("done", "Erledigt"),
                ("not_needed", "Nicht mehr nötig"),
            ]
        );
        assert_eq!(
            view.columns[0]
                .tickets
                .iter()
                .map(|card| (card.id.as_str(), card.comment_count))
                .collect::<Vec<_>>(),
            [("ARC-0002", 0), ("ARC-0001", 1)]
        );
        assert_eq!(view.columns[1].tickets.len(), 1);
        let waiting = &view.columns[1].tickets[0];
        assert_eq!(waiting.id, "ARC-0003");
        assert!(waiting.waiting);
        assert_eq!(waiting.column, BoardColumn::Waiting);
        assert_eq!(waiting.depends_open, ["ARC-0001"]);
        assert_eq!(waiting.comment_count, 0);
        assert_eq!(
            view.columns[4]
                .tickets
                .iter()
                .map(|card| card.id.as_str())
                .collect::<Vec<_>>(),
            ["ARC-0004"]
        );
        assert!(view.columns[2].tickets.is_empty());
    }

    #[tokio::test]
    async fn sendet_statt_der_langen_texte_nur_ob_sie_vorhanden_sind() {
        let f = BoardFixture::create().await;
        f.insert_ticket(&Ticket {
            title: "Mit Text".into(),
            description: "Lang".into(),
            acceptance: "Geht".into(),
            ..f.new_ticket(1).await
        })
        .await;

        let view = f.views.get(f.board_id).await.unwrap();
        let card = view
            .columns
            .iter()
            .flat_map(|column| column.tickets.iter())
            .next()
            .unwrap();

        assert!(card.has_description);
        assert!(card.has_acceptance);
        assert!(!card.has_plan);
        assert!(!card.has_handbook);
        assert!(!card.has_diagram);
        // Die langen Texte selbst stehen nicht in der Karte
        let json = serde_json::to_value(card).unwrap();
        for feld in ["description", "acceptance", "plan", "handbook", "diagram"] {
            assert!(
                json.get(feld).is_none(),
                "{feld} gehört nicht auf die Karte"
            );
        }
    }

    #[tokio::test]
    async fn zeigt_nur_tickets_des_boards_und_meldet_ein_unbekanntes_board() {
        let f = BoardFixture::create().await;
        let other = f.boards.create("ARC", "Anderes", &f.actor).await.unwrap();
        f.insert_ticket(&Ticket {
            title: "Fremd".into(),
            board_id: other.id,
            ..f.new_ticket(1).await
        })
        .await;

        let view = f.views.get(f.board_id).await.unwrap();

        assert!(view.columns.iter().all(|column| column.tickets.is_empty()));
        assert_eq!(
            f.views.get(999).await.unwrap_err().code(),
            Some("board-not-found")
        );
    }
}
