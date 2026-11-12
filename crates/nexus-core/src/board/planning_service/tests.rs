//! Portiert aus `packages/core/tests/unit/board/PlanningService.test.ts`.

use std::sync::Arc;

use async_trait::async_trait;

use crate::board::basics::{Priority, TicketStatus};
use crate::board::database::{
    BoardDatabase, BoardQuestionCommentRepository, BoardQuestionFilter,
    BoardQuestionHistoryRepository, BoardQuestionRepository, BoardQuestionResearchRepository,
    BoardRepository, BoardSearchRepository, BoardUnitOfWork, CommentRepository, DeletionRepository,
    DispatchRunRepository, DispatchStatsRepository, HistoryRepository, NoteRepository,
    ProjectRepository, TicketFilter, TicketRepository, TicketSessionRepository,
    TicketStepRepository, UserRepository,
};
use crate::board::planning_service::{PlanInput, PlanningService};
use crate::board::question_types::{BoardQuestion, BoardQuestionHistoryEntry};
use crate::board::ticket_service::NewTicketInput;
use crate::board::types::{HistoryEntry, StepProgress, Ticket};
use crate::error::Result;
use crate::testing::BoardFixture;

/// Legt ein Ticket auf dem Board „Sprint“ der Umgebung an
async fn add(f: &BoardFixture, input: NewTicketInput) {
    f.tickets
        .create(f.board_id, &input, &f.actor, &Default::default())
        .await
        .expect("Ticket anlegen geht in dieser Umgebung immer");
}

/// Ein Plan, der nur den Plan-Text setzt
fn plan(text: &str) -> PlanInput {
    PlanInput {
        plan: Some(text.to_string()),
        ..PlanInput::default()
    }
}

#[tokio::test]
async fn schreibt_den_plan_setzt_geplant_loscht_planungsbedarf_und_claim() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("A"),
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();
    f.planning
        .next(&[f.board_id], Some("claude"), &f.actor)
        .await
        .unwrap();

    let view = f
        .planning
        .plan(
            "ARC-0001",
            &PlanInput {
                plan: Some("## Ziel".into()),
                diagram: Some("```mermaid\nflowchart TD\n  A-->B\n```".into()),
                ..PlanInput::default()
            },
            &f.actor,
        )
        .await
        .unwrap();

    assert_eq!(view.status, TicketStatus::Planned);
    assert!(!view.needs_planning);
    assert_eq!(view.planning_claim_by, None);
    assert_eq!(view.planning_claim_at, None);
    assert_eq!(view.planned_by.as_deref(), Some("claude"));
    assert_eq!(view.planned_at.as_deref(), Some("2026-10-13T10:00:00.000Z"));
    assert_eq!(view.plan, "## Ziel");
    assert_eq!(view.diagram, "flowchart TD\n  A-->B");
    let detail = f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(
        detail.history.last().and_then(|h| h.note.as_deref()),
        Some("geplant")
    );
}

#[tokio::test]
async fn protokolliert_planungs_claim_und_dessen_freigabe_im_verlauf() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("A"),
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();
    f.planning
        .next(&[f.board_id], Some("junior-x"), &f.actor)
        .await
        .unwrap();
    f.planning
        .plan("ARC-0001", &plan("P"), &f.actor)
        .await
        .unwrap();

    let detail = f.tickets.get("ARC-0001").await.unwrap();
    let claims: Vec<(Option<&str>, Option<&str>)> = detail
        .history
        .iter()
        .filter(|h| h.field == "planningClaimBy")
        .map(|h| (h.from.as_deref(), h.to.as_deref()))
        .collect();
    assert_eq!(claims, [(None, Some("junior-x")), (Some("junior-x"), None)]);
}

#[tokio::test]
async fn vermerkt_plan_aktualisiert_wenn_der_status_gleich_bleibt() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                status: Some(TicketStatus::Planned),
                ..NewTicketInput::titled("A")
            },
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();
    f.planning
        .plan(
            "ARC-0001",
            &PlanInput {
                handbook: Some("HB".into()),
                ..PlanInput::default()
            },
            &f.actor,
        )
        .await
        .unwrap();

    let detail = f.tickets.get("ARC-0001").await.unwrap();
    assert_eq!(
        detail.history.last().and_then(|h| h.note.as_deref()),
        Some("Plan aktualisiert")
    );
}

#[tokio::test]
async fn schreibt_die_abnahme_mit_ohne_oder_leere_bleibt_die_vorhandene() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                acceptance: Some("alt".into()),
                ..NewTicketInput::titled("A")
            },
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();

    assert_eq!(
        f.planning
            .plan("ARC-0001", &plan("P"), &f.actor)
            .await
            .unwrap()
            .acceptance,
        "alt"
    );
    assert_eq!(
        f.planning
            .plan(
                "ARC-0001",
                &PlanInput {
                    acceptance: Some("  ".into()),
                    ..plan("P")
                },
                &f.actor
            )
            .await
            .unwrap()
            .acceptance,
        "alt"
    );
    assert_eq!(
        f.planning
            .plan(
                "ARC-0001",
                &PlanInput {
                    acceptance: Some("- Test deckt den Fall ab".into()),
                    ..plan("P")
                },
                &f.actor
            )
            .await
            .unwrap()
            .acceptance,
        "- Test deckt den Fall ab"
    );
}

#[tokio::test]
async fn verlangt_mindestens_einen_text() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("A"),
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();
    let fehler = f
        .planning
        .plan("ARC-0001", &PlanInput::default(), &f.actor)
        .await
        .unwrap_err();
    assert_eq!(fehler.code(), Some("plan-empty"));
}

#[tokio::test]
async fn liefert_das_nachste_ticket_mit_planungsbedarf_und_uberspringt_gesperrte() {
    let f = BoardFixture::create().await;
    add(
        &f,
        NewTicketInput {
            status: Some(TicketStatus::InPlanning),
            ..NewTicketInput::titled("Nutzer plant")
        },
    )
    .await;
    add(
        &f,
        NewTicketInput {
            priority: Some(Priority::Urgent),
            ..NewTicketInput::titled("Mit Frage")
        },
    )
    .await;
    f.open_question(1, "Offen", &["ARC-0002"]).await;
    add(
        &f,
        NewTicketInput {
            needs_planning: Some(false),
            ..NewTicketInput::titled("Ohne Planung")
        },
    )
    .await;
    add(&f, NewTicketInput::titled("Normal")).await;
    add(
        &f,
        NewTicketInput {
            priority: Some(Priority::Urgent),
            ..NewTicketInput::titled("Dringend")
        },
    )
    .await;

    let next = f
        .planning
        .next(&[f.board_id], None, &f.actor)
        .await
        .unwrap();
    assert_eq!(
        next.map(|view| view.ticket.id),
        Some("ARC-0005".to_string())
    );
}

#[tokio::test]
async fn beansprucht_mit_claim_fur_15_minuten() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("A"),
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();

    let claimed = f
        .planning
        .next(&[f.board_id], Some("claude-1"), &f.actor)
        .await
        .unwrap()
        .expect("das Ticket braucht Planung");
    assert_eq!(claimed.ticket.id, "ARC-0001");
    assert_eq!(claimed.planning_claim_by.as_deref(), Some("claude-1"));
    assert_eq!(claimed.rev, 2);
    assert!(f
        .planning
        .next(&[f.board_id], Some("claude-2"), &f.actor)
        .await
        .unwrap()
        .is_none());

    f.advance(16);
    assert_eq!(
        f.planning
            .next(&[f.board_id], Some("claude-2"), &f.actor)
            .await
            .unwrap()
            .and_then(|view| view.ticket.planning_claim_by.clone()),
        Some("claude-2".to_string())
    );
}

#[tokio::test]
async fn claim_beansprucht_genau_ein_ticket_und_release_gibt_nur_den_eigenen_frei() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("A"),
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();

    assert_eq!(
        f.planning
            .claim("ARC-0001", "planner-ARC-0001", &f.actor)
            .await
            .unwrap()
            .planning_claim_by
            .as_deref(),
        Some("planner-ARC-0001")
    );
    assert!(f
        .planning
        .next(&[f.board_id], Some("claude"), &f.actor)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        f.planning
            .claim("ARC-0001", "claude", &f.actor)
            .await
            .unwrap_err()
            .code(),
        Some("plan-claimed")
    );

    f.planning
        .release("ARC-0001", "claude", &f.actor)
        .await
        .unwrap();
    assert_eq!(
        f.tickets
            .get("ARC-0001")
            .await
            .unwrap()
            .planning_claim_by
            .as_deref(),
        Some("planner-ARC-0001")
    );
    f.planning
        .release("ARC-0001", "planner-ARC-0001", &f.actor)
        .await
        .unwrap();
    assert_eq!(
        f.tickets.get("ARC-0001").await.unwrap().planning_claim_by,
        None
    );

    f.planning
        .claim("ARC-0001", "planner-ARC-0001", &f.actor)
        .await
        .unwrap();
    f.advance(16);
    assert_eq!(
        f.planning
            .claim("ARC-0001", "claude", &f.actor)
            .await
            .unwrap()
            .planning_claim_by
            .as_deref(),
        Some("claude")
    );
    assert_eq!(
        f.planning
            .claim("ARC-0001", " ", &f.actor)
            .await
            .unwrap_err()
            .code(),
        Some("agent-missing")
    );
}

#[tokio::test]
async fn lehnt_einen_leeren_agentennamen_ab_und_kurzt_leerzeichen() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("A"),
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();

    assert_eq!(
        f.planning
            .next(&[f.board_id], Some("   "), &f.actor)
            .await
            .unwrap_err()
            .code(),
        Some("agent-missing")
    );
    assert_eq!(
        f.planning
            .next(&[f.board_id], Some(&"a".repeat(101)), &f.actor)
            .await
            .unwrap_err()
            .code(),
        Some("value-too-long")
    );
    assert_eq!(
        f.planning
            .next(&[f.board_id], Some(" claude-1 "), &f.actor)
            .await
            .unwrap()
            .and_then(|view| view.ticket.planning_claim_by.clone()),
        Some("claude-1".to_string())
    );
}

#[tokio::test]
async fn uberspringt_ein_ticket_das_zwischen_auswahl_und_sperre_beansprucht_wurde() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput {
                priority: Some(Priority::Urgent),
                ..NewTicketInput::titled("Dringend")
            },
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("Normal"),
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();

    // Gesperrtes Lesen sieht den Claim eines anderen Agenten, der nach der Auswahl bestätigt
    // wurde
    let racing = Arc::new(RacingDatabase {
        inner: f.db.clone(),
        race: Race::ClaimedBy("other"),
        at: f.clock.iso(),
    });
    let planning = PlanningService::new(racing, f.clock.clone());
    let claimed = planning
        .next(&[f.board_id], Some("claude-1"), &f.actor)
        .await
        .unwrap()
        .expect("das zweite Ticket bleibt übrig");
    assert_eq!(claimed.ticket.id, "ARC-0002");
    assert_eq!(claimed.planning_claim_by.as_deref(), Some("claude-1"));
}

#[tokio::test]
async fn liefert_null_wenn_beim_sperren_kein_kandidat_mehr_ubrig_ist() {
    let f = BoardFixture::create().await;
    f.tickets
        .create(
            f.board_id,
            &NewTicketInput::titled("A"),
            &f.actor,
            &Default::default(),
        )
        .await
        .unwrap();

    let racing = Arc::new(RacingDatabase {
        inner: f.db.clone(),
        race: Race::Vanished,
        at: f.clock.iso(),
    });
    assert!(PlanningService::new(racing, f.clock.clone())
        .next(&[f.board_id], Some("claude-1"), &f.actor)
        .await
        .unwrap()
        .is_none());
}

// ---------------------------------------------------------------------------------------------
// Eine Datenbank, bei der das gesperrte Lesen etwas anderes liefert als das freie
// ---------------------------------------------------------------------------------------------

/// Was das gesperrte Lesen aus `ARC-0001` macht
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Race {
    /// Ein anderer Agent hat es inzwischen beansprucht
    ClaimedBy(&'static str),
    /// Es ist verschwunden (gelöscht)
    Vanished,
}

/// Reicht alles an die Datenbank im Speicher weiter und verstellt nur das gesperrte Lesen;
/// entspricht dem Objekt `racing` aus TypeScript
struct RacingDatabase<D: BoardDatabase> {
    inner: Arc<D>,
    race: Race,
    /// Zeitpunkt des vorgetäuschten fremden Claims
    at: String,
}

impl<D: BoardDatabase> BoardDatabase for RacingDatabase<D> {
    fn transaction<'a, T, W>(
        &'a self,
        work: W,
    ) -> impl std::future::Future<Output = Result<T>> + Send + 'a
    where
        T: Send + 'static,
        W: crate::board::database::TransactionWork<T> + 'static,
    {
        let race = self.race;
        let at = self.at.clone();
        // Die Arbeit wird geteilt, damit das umhüllende Closure sie bei jedem Aufruf in den
        // eigenen Rumpf mitnehmen kann (die Arbeit darf mehrfach laufen)
        let work = Arc::new(work);
        self.inner.transaction(move |u| {
            let work = Arc::clone(&work);
            let at = at.clone();
            Box::pin(async move {
                let mut unit = RacingUnit { inner: u, race, at };
                work(&mut unit).await
            })
        })
    }

    fn change_marker(
        &self,
        board_id: i32,
    ) -> impl std::future::Future<Output = Result<String>> + Send + '_ {
        self.inner.change_marker(board_id)
    }

    fn question_marker(
        &self,
        project_id: i32,
    ) -> impl std::future::Future<Output = Result<String>> + Send + '_ {
        self.inner.question_marker(project_id)
    }

    fn close(&self) -> impl std::future::Future<Output = Result<()>> + Send + '_ {
        self.inner.close()
    }
}

/// Die Unit of Work dazu: nur `tickets().by_id(…, true)` verhält sich anders
struct RacingUnit<'u> {
    inner: &'u mut dyn BoardUnitOfWork,
    race: Race,
    at: String,
}

#[async_trait]
impl TicketRepository for RacingUnit<'_> {
    async fn by_id(&mut self, id: &str, for_update: bool) -> Result<Option<Ticket>> {
        let ticket = self.inner.tickets().by_id(id, for_update).await?;
        if !for_update || id != "ARC-0001" {
            return Ok(ticket);
        }
        match self.race {
            Race::Vanished => Ok(None),
            Race::ClaimedBy(agent) => Ok(ticket.map(|ticket| Ticket {
                planning_claim_by: Some(agent.to_string()),
                planning_claim_at: Some(self.at.clone()),
                ..ticket
            })),
        }
    }

    async fn list(&mut self, filter: &TicketFilter) -> Result<Vec<Ticket>> {
        self.inner.tickets().list(filter).await
    }

    async fn insert(&mut self, ticket: &Ticket) -> Result<()> {
        self.inner.tickets().insert(ticket).await
    }

    async fn update(&mut self, ticket: &Ticket) -> Result<()> {
        self.inner.tickets().update(ticket).await
    }

    async fn delete(&mut self, id: &str) -> Result<()> {
        self.inner.tickets().delete(id).await
    }

    async fn dependents(&mut self, id: &str) -> Result<Vec<String>> {
        self.inner.tickets().dependents(id).await
    }

    async fn remove_dependency_on(&mut self, id: &str) -> Result<Vec<String>> {
        self.inner.tickets().remove_dependency_on(id).await
    }

    async fn max_sort_order(&mut self, board_id: i32, status: TicketStatus) -> Result<Option<i32>> {
        self.inner.tickets().max_sort_order(board_id, status).await
    }

    async fn open_count_by_board(
        &mut self,
        board_ids: &[i32],
    ) -> Result<std::collections::BTreeMap<i32, i64>> {
        self.inner.tickets().open_count_by_board(board_ids).await
    }
}

#[async_trait]
impl HistoryRepository for RacingUnit<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<HistoryEntry>> {
        self.inner.history().list(ticket_id).await
    }

    async fn insert(&mut self, entries: &[HistoryEntry]) -> Result<()> {
        self.inner.history().insert(entries).await
    }

    async fn done_at_by_board(
        &mut self,
        board_id: i32,
    ) -> Result<std::collections::BTreeMap<String, String>> {
        self.inner.history().done_at_by_board(board_id).await
    }
}

#[async_trait]
impl TicketStepRepository for RacingUnit<'_> {
    async fn list(&mut self, ticket_id: &str) -> Result<Vec<crate::board::types::TicketStep>> {
        self.inner.steps().list(ticket_id).await
    }

    async fn replace(
        &mut self,
        ticket_id: &str,
        steps: &[crate::board::types::TicketStep],
    ) -> Result<()> {
        self.inner.steps().replace(ticket_id, steps).await
    }

    async fn set_done(&mut self, ticket_id: &str, position: i32, done: bool) -> Result<bool> {
        self.inner.steps().set_done(ticket_id, position, done).await
    }

    async fn progress(
        &mut self,
        ticket_ids: &[String],
    ) -> Result<std::collections::BTreeMap<String, StepProgress>> {
        self.inner.steps().progress(ticket_ids).await
    }
}

#[async_trait]
impl TicketSessionRepository for RacingUnit<'_> {
    async fn add(&mut self, ticket_id: &str, session_id: &str, at: &str) -> Result<bool> {
        self.inner
            .ticket_sessions()
            .add(ticket_id, session_id, at)
            .await
    }

    async fn by_tickets(
        &mut self,
        ticket_ids: &[String],
    ) -> Result<std::collections::BTreeMap<String, Vec<String>>> {
        self.inner.ticket_sessions().by_tickets(ticket_ids).await
    }

    async fn tickets_of(&mut self, session_id: &str) -> Result<Vec<String>> {
        self.inner.ticket_sessions().tickets_of(session_id).await
    }
}

#[async_trait]
impl BoardQuestionRepository for RacingUnit<'_> {
    async fn by_id(&mut self, id: &str, for_update: bool) -> Result<Option<BoardQuestion>> {
        self.inner.questions().by_id(id, for_update).await
    }

    async fn list(&mut self, filter: &BoardQuestionFilter) -> Result<Vec<BoardQuestion>> {
        self.inner.questions().list(filter).await
    }

    async fn insert(&mut self, question: &BoardQuestion) -> Result<()> {
        self.inner.questions().insert(question).await
    }

    async fn update(&mut self, question: &BoardQuestion) -> Result<()> {
        self.inner.questions().update(question).await
    }

    async fn ids_referencing_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        self.inner
            .questions()
            .ids_referencing_ticket(ticket_id)
            .await
    }

    async fn remove_ticket(&mut self, ticket_id: &str) -> Result<Vec<String>> {
        self.inner.questions().remove_ticket(ticket_id).await
    }
}

/// Nur die Repositories, die `next` und `save` anfassen, reicht die Umgebung weiter
impl BoardUnitOfWork for RacingUnit<'_> {
    fn tickets(&mut self) -> &mut dyn TicketRepository {
        self
    }

    fn history(&mut self) -> &mut dyn HistoryRepository {
        self
    }

    fn steps(&mut self) -> &mut dyn TicketStepRepository {
        self
    }

    fn ticket_sessions(&mut self) -> &mut dyn TicketSessionRepository {
        self
    }

    fn questions(&mut self) -> &mut dyn BoardQuestionRepository {
        self
    }

    fn users(&mut self) -> &mut dyn UserRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn projects(&mut self) -> &mut dyn ProjectRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn boards(&mut self) -> &mut dyn BoardRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn comments(&mut self) -> &mut dyn CommentRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn notes(&mut self) -> &mut dyn NoteRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn deletions(&mut self) -> &mut dyn DeletionRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn question_research(&mut self) -> &mut dyn BoardQuestionResearchRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn question_comments(&mut self) -> &mut dyn BoardQuestionCommentRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn question_history(&mut self) -> &mut dyn BoardQuestionHistoryRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn search(&mut self) -> &mut dyn BoardSearchRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn dispatch_runs(&mut self) -> &mut dyn DispatchRunRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }

    fn dispatch_stats(&mut self) -> &mut dyn DispatchStatsRepository {
        unimplemented!("die Umgebung des Wettlaufs kennt nur Tickets und Fragen")
    }
}

#[async_trait]
impl BoardQuestionHistoryRepository for RacingUnit<'_> {
    async fn list(&mut self, question_id: &str) -> Result<Vec<BoardQuestionHistoryEntry>> {
        self.inner.question_history().list(question_id).await
    }

    async fn insert(&mut self, entries: &[BoardQuestionHistoryEntry]) -> Result<()> {
        self.inner.question_history().insert(entries).await
    }
}
