//! Standardwerte für die Vertragstests; portiert aus `ticketFixture`, `questionFixture` und
//! `runFixture` in `packages/core/tests/unit/board/helpers/`.
//!
//! Abweichungen werden in Rust mit der Struktur-Aktualisierung geschrieben – das entspricht
//! dem `...overrides` aus TypeScript:
//!
//! ```
//! # use nexus_core::board::types::Ticket;
//! # use nexus_core::testing::ticket_fixture;
//! let ticket = Ticket { title: "Eigener Titel".into(), ..ticket_fixture("ARC-0001", 1, 2, 1) };
//! assert_eq!(ticket.number, 1);
//! ```

use crate::actors::AuthorKind;
use crate::board::basics::{NoteState, Priority, TicketStatus};
use crate::board::database::{BoardDatabase, NewBoard, NewComment, NewNote, NewProject};
use crate::board::dispatch_run_types::{
    DispatchRun, DispatchRunKind, DispatchRunOutcome, DispatchRunSource,
};
use crate::board::question_types::BoardQuestion;
use crate::board::types::{BoardUser, HistoryEntry, Ticket};
use crate::questions::QuestionState;

/// Der Benutzer, mit dem jeder Vertrag arbeitet (`USER` in TypeScript)
pub const USER_ID: &str = "11111111-1111-1111-1111-111111111111";
/// Sein Name
pub const USER_NAME: &str = "Florian";
/// Zeitpunkt der meisten Fixtures
pub const NOW: &str = "2026-10-13T10:00:00.000Z";
/// Zeitpunkt der Fragen-Fixtures
pub const QUESTION_NOW: &str = "2026-10-14T10:00:00.000Z";
/// Eine Stunde später, für Änderungen an Fragen
pub const QUESTION_LATER: &str = "2026-10-14T11:00:00.000Z";

/// Der Benutzer der Verträge
pub fn user() -> BoardUser {
    user_named(USER_NAME)
}

/// Derselbe Benutzer unter einem anderen Namen
pub fn user_named(name: &str) -> BoardUser {
    BoardUser {
        id: USER_ID.to_string(),
        name: name.to_string(),
    }
}

/// Vollständiges Ticket mit Standardwerten für Tests
pub fn ticket_fixture(id: &str, project_id: i32, board_id: i32, number: i32) -> Ticket {
    Ticket {
        id: id.to_string(),
        project_id,
        board_id,
        number,
        title: "Titel".into(),
        description: String::new(),
        status: TicketStatus::Backlog,
        priority: Priority::Normal,
        labels: Vec::new(),
        depends_on: Vec::new(),
        sort_order: 0,
        assignee: None,
        needs_planning: true,
        acceptance: String::new(),
        plan: String::new(),
        handbook: String::new(),
        diagram: String::new(),
        spec: None,
        spec_task: None,
        branch: None,
        worktree: None,
        target_branch: None,
        touches: Vec::new(),
        consumes: Vec::new(),
        produces: Vec::new(),
        blocked_reason: None,
        planned_at: None,
        planned_by: None,
        planning_claim_by: None,
        planning_claim_at: None,
        created_at: NOW.into(),
        created_by_id: USER_ID.into(),
        created_by_name: USER_NAME.into(),
        updated_at: NOW.into(),
        rev: 1,
    }
}

/// Vollständige Frage mit Standardwerten für Tests
pub fn question_fixture(id: &str, project_id: i32, number: i32) -> BoardQuestion {
    BoardQuestion {
        id: id.to_string(),
        project_id,
        number,
        title: "Welcher Speicher?".into(),
        description: String::new(),
        state: QuestionState::Open,
        occasion: None,
        origin_kind: AuthorKind::Session,
        origin_name: "claude".into(),
        origin_session: None,
        tags: Vec::new(),
        ticket_ids: Vec::new(),
        depends_on: Vec::new(),
        decision: None,
        created_at: QUESTION_NOW.into(),
        created_by_id: USER_ID.into(),
        updated_at: QUESTION_NOW.into(),
        rev: 1,
    }
}

/// Vollständiger Lauf des Dispatchers mit Standardwerten
pub fn run_fixture(id: &str, subject_id: &str, project_id: i32) -> DispatchRun {
    DispatchRun {
        id: id.to_string(),
        kind: DispatchRunKind::Work,
        subject_id: subject_id.to_string(),
        project_id,
        board_id: None,
        agent: "junior-X".into(),
        attempt: Some(1),
        profile: "standard".into(),
        escalated_from: None,
        requested_model: Some("opus".into()),
        model: Some("claude-opus-5-5".into()),
        effort: Some("high".into()),
        started_at: NOW.into(),
        ended_at: Some("2026-10-13T10:05:00.000Z".into()),
        duration_ms: Some(300_000),
        num_turns: Some(12),
        exit_code: Some(0),
        timed_out: false,
        outcome: DispatchRunOutcome::Done,
        abort_category: None,
        abort_reason: None,
        cost_usd: Some(0.123_456),
        input_tokens: Some(10),
        output_tokens: Some(2_000_000_000_000),
        cache_read_tokens: Some(30),
        cache_write_tokens: Some(40),
        model_usage: Some(r#"{"claude-opus-5-5":{"costUsd":0.123456}}"#.into()),
        source: DispatchRunSource::Dispatcher,
        max_turns: None,
        timeout_minutes: None,
        profile_reason: None,
        rule_index: None,
        config_hash: None,
        denied_count: None,
        denied_tools: None,
    }
}

/// Ein neuer Kommentar ohne Lesemarke
pub fn new_comment(ticket_id: &str, author_name: &str, body: &str) -> NewComment {
    NewComment {
        ticket_id: ticket_id.to_string(),
        author_id: USER_ID.into(),
        author_name: author_name.to_string(),
        body: body.to_string(),
        created_at: NOW.into(),
        read_by_agent_at: None,
    }
}

/// Eine neue, offene Notiz ohne Herkunft und Labels
pub fn new_note(board_id: i32, title: &str, body: &str) -> NewNote {
    NewNote {
        board_id,
        author_id: USER_ID.into(),
        author_name: "claude".into(),
        title: title.to_string(),
        body: body.to_string(),
        source: None,
        labels: Vec::new(),
        seen_count: 1,
        seen_in: Vec::new(),
        state: NoteState::Open,
        ticket_id: None,
        reason: None,
        created_at: NOW.into(),
    }
}

/// Ein Verlaufseintrag zu einem Statuswechsel
pub fn status_entry(ticket_id: &str, at: &str, to: &str) -> HistoryEntry {
    HistoryEntry {
        ticket_id: ticket_id.to_string(),
        at: at.to_string(),
        by_id: USER_ID.into(),
        by_name: "x".into(),
        field: "status".into(),
        from: None,
        to: Some(to.to_string()),
        note: None,
    }
}

// ---------------------------------------------------------------------------------------------
// Grundbestand der Verträge
// ---------------------------------------------------------------------------------------------

/// Legt den Benutzer der Verträge an (das `beforeEach` aus TypeScript)
pub async fn upsert_user<D: BoardDatabase>(db: &D) {
    crate::tx!(db, |u| { u.users().upsert(&user()).await })
        .await
        .expect("Benutzer anlegen geht immer");
}

/// Projekt ARC mit einem Board
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardSeed {
    pub project_id: i32,
    pub board_id: i32,
}

/// Projekt ARC mit zwei Boards und je einem Ticket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestionSeed {
    pub project_id: i32,
    pub board_id: i32,
    pub other_id: i32,
}

/// Projekt ARC mit zwei Boards, Projekt OTH mit einem
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchSeed {
    pub arc: i32,
    pub oth: i32,
    pub b1: i32,
    pub b2: i32,
    pub b3: i32,
}

/// Ein neues Projekt mit diesem Schlüssel und Namen
fn new_project(key: &str, name: &str, created_at: &str) -> NewProject {
    NewProject {
        key: key.to_string(),
        name: name.to_string(),
        created_at: created_at.to_string(),
        created_by: USER_ID.into(),
    }
}

/// Ein neues Board dieses Projekts
fn new_board(project_id: i32, name: &str, created_at: &str) -> NewBoard {
    NewBoard {
        project_id,
        owner_id: USER_ID.into(),
        name: name.to_string(),
        repo_path: None,
        created_at: created_at.to_string(),
    }
}

/// Legt Projekt ARC und das Board „Sprint“ an
pub async fn seed_board<D: BoardDatabase>(db: &D) -> BoardSeed {
    crate::tx!(db, |u| {
        let project = u
            .projects()
            .insert(&new_project("ARC", "Arches", NOW))
            .await?;
        let board = u
            .boards()
            .insert(&new_board(project.id, "Sprint", NOW))
            .await?;
        Ok(BoardSeed {
            project_id: project.id,
            board_id: board.id,
        })
    })
    .await
    .expect("Grundbestand anlegen geht immer")
}

/// Legt Projekt ARC mit zwei Boards und je einem Ticket an
pub async fn seed_questions<D: BoardDatabase>(db: &D) -> QuestionSeed {
    crate::tx!(db, |u| {
        let project = u
            .projects()
            .insert(&new_project("ARC", "Arches", QUESTION_NOW))
            .await?;
        let board = u
            .boards()
            .insert(&new_board(project.id, "Sprint", QUESTION_NOW))
            .await?;
        let other = u
            .boards()
            .insert(&new_board(project.id, "Anderes", QUESTION_NOW))
            .await?;
        u.tickets()
            .insert(&ticket_fixture("ARC-0001", project.id, board.id, 1))
            .await?;
        u.tickets()
            .insert(&ticket_fixture("ARC-0002", project.id, other.id, 2))
            .await?;
        Ok(QuestionSeed {
            project_id: project.id,
            board_id: board.id,
            other_id: other.id,
        })
    })
    .await
    .expect("Grundbestand anlegen geht immer")
}

/// Legt Projekt ARC mit zwei Boards und Projekt OTH mit einem Board an
pub async fn seed_dispatch<D: BoardDatabase>(db: &D) -> DispatchSeed {
    crate::tx!(db, |u| {
        let arc = u
            .projects()
            .insert(&new_project("ARC", "Arches", NOW))
            .await?;
        let oth = u
            .projects()
            .insert(&new_project("OTH", "Andere", NOW))
            .await?;
        let b1 = u.boards().insert(&new_board(arc.id, "Eins", NOW)).await?;
        let b2 = u.boards().insert(&new_board(arc.id, "Zwei", NOW)).await?;
        let b3 = u.boards().insert(&new_board(oth.id, "Drei", NOW)).await?;
        Ok(DispatchSeed {
            arc: arc.id,
            oth: oth.id,
            b1: b1.id,
            b2: b2.id,
            b3: b3.id,
        })
    })
    .await
    .expect("Grundbestand anlegen geht immer")
}
