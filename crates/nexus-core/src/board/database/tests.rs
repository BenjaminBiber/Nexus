//! Kompiliertest der Verträge: ein Mini-Dummy erfüllt alle 17 Repository-Traits auf *einem*
//! Unit-of-Work-Struct und zeigt, dass die Arbeit einer Transaktion mehrfach aufrufbar ist (`Fn`).

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::board::basics::Priority;

/// Unit of Work des Dummys: hält die Daten im Speicher und erfüllt alle Repository-Traits selbst
#[derive(Default)]
struct DummyUnitOfWork {
    users: Vec<BoardUser>,
    tickets: Vec<Ticket>,
}

#[async_trait]
impl UserRepository for DummyUnitOfWork {
    async fn upsert(&mut self, user: &BoardUser) -> Result<()> {
        self.users.retain(|u| u.id != user.id);
        self.users.push(user.clone());
        Ok(())
    }

    async fn by_id(&mut self, id: &str) -> Result<Option<BoardUser>> {
        Ok(self.users.iter().find(|u| u.id == id).cloned())
    }

    async fn list(&mut self) -> Result<Vec<BoardUser>> {
        Ok(self.users.clone())
    }
}

#[async_trait]
impl ProjectRepository for DummyUnitOfWork {
    async fn list(&mut self) -> Result<Vec<Project>> {
        Ok(Vec::new())
    }
    async fn by_id(&mut self, _id: i32) -> Result<Option<Project>> {
        Ok(None)
    }
    async fn by_key(&mut self, _key: &str) -> Result<Option<Project>> {
        Ok(None)
    }
    async fn insert(&mut self, project: &NewProject) -> Result<Project> {
        Ok(Project {
            id: 1,
            key: project.key.clone(),
            name: project.name.clone(),
            next_ticket_number: 1,
            next_question_number: 1,
            created_at: project.created_at.clone(),
            created_by: project.created_by.clone(),
        })
    }
    async fn rename(&mut self, _id: i32, _name: &str) -> Result<()> {
        Ok(())
    }
    async fn take_ticket_number(&mut self, _id: i32) -> Result<i32> {
        Ok(1)
    }
    async fn take_question_number(&mut self, _id: i32) -> Result<i32> {
        Ok(1)
    }
}

#[async_trait]
impl BoardRepository for DummyUnitOfWork {
    async fn list(&mut self, _filter: &BoardFilter) -> Result<Vec<Board>> {
        Ok(Vec::new())
    }
    async fn by_id(&mut self, _id: i32) -> Result<Option<Board>> {
        Ok(None)
    }
    async fn insert(&mut self, board: &NewBoard) -> Result<Board> {
        Ok(Board {
            id: 1,
            project_id: board.project_id,
            owner_id: board.owner_id.clone(),
            name: board.name.clone(),
            repo_path: board.repo_path.clone(),
            created_at: board.created_at.clone(),
        })
    }
    async fn update(&mut self, _board: &Board) -> Result<()> {
        Ok(())
    }
    async fn delete(&mut self, _id: i32) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl TicketRepository for DummyUnitOfWork {
    async fn by_id(&mut self, id: &str, _for_update: bool) -> Result<Option<Ticket>> {
        Ok(self.tickets.iter().find(|t| t.id == id).cloned())
    }
    async fn list(&mut self, _filter: &TicketFilter) -> Result<Vec<Ticket>> {
        Ok(self.tickets.clone())
    }
    async fn insert(&mut self, ticket: &Ticket) -> Result<()> {
        self.tickets.push(ticket.clone());
        Ok(())
    }
    async fn update(&mut self, _ticket: &Ticket) -> Result<()> {
        Ok(())
    }
    async fn delete(&mut self, _id: &str) -> Result<()> {
        Ok(())
    }
    async fn dependents(&mut self, _id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
    async fn remove_dependency_on(&mut self, _id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
    async fn max_sort_order(
        &mut self,
        _board_id: i32,
        _status: TicketStatus,
    ) -> Result<Option<i32>> {
        Ok(None)
    }
    async fn open_count_by_board(&mut self, _board_ids: &[i32]) -> Result<BTreeMap<i32, i64>> {
        Ok(BTreeMap::new())
    }
}

#[async_trait]
impl CommentRepository for DummyUnitOfWork {
    async fn list(&mut self, _ticket_id: &str) -> Result<Vec<Comment>> {
        Ok(Vec::new())
    }
    async fn insert(&mut self, comment: &NewComment) -> Result<Comment> {
        Ok(Comment {
            id: 1,
            ticket_id: comment.ticket_id.clone(),
            author_id: comment.author_id.clone(),
            author_name: comment.author_name.clone(),
            body: comment.body.clone(),
            created_at: comment.created_at.clone(),
            read_by_agent_at: comment.read_by_agent_at.clone(),
        })
    }
    async fn count_by_board(&mut self, _board_id: i32) -> Result<BTreeMap<String, i64>> {
        Ok(BTreeMap::new())
    }
    async fn unread_by_agent(&mut self, _ticket_ids: &[String]) -> Result<Vec<Comment>> {
        Ok(Vec::new())
    }
    async fn mark_read_by_agent(&mut self, _ids: &[i64], _at: &str) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl TicketStepRepository for DummyUnitOfWork {
    async fn list(&mut self, _ticket_id: &str) -> Result<Vec<TicketStep>> {
        Ok(Vec::new())
    }
    async fn replace(&mut self, _ticket_id: &str, _steps: &[TicketStep]) -> Result<()> {
        Ok(())
    }
    async fn set_done(&mut self, _ticket_id: &str, _position: i32, _done: bool) -> Result<bool> {
        Ok(false)
    }
    async fn progress(&mut self, _ticket_ids: &[String]) -> Result<BTreeMap<String, StepProgress>> {
        Ok(BTreeMap::new())
    }
}

#[async_trait]
impl TicketSessionRepository for DummyUnitOfWork {
    async fn add(&mut self, _ticket_id: &str, _session_id: &str, _at: &str) -> Result<bool> {
        Ok(true)
    }
    async fn by_tickets(
        &mut self,
        _ticket_ids: &[String],
    ) -> Result<BTreeMap<String, Vec<String>>> {
        Ok(BTreeMap::new())
    }
    async fn tickets_of(&mut self, _session_id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl HistoryRepository for DummyUnitOfWork {
    async fn list(&mut self, _ticket_id: &str) -> Result<Vec<HistoryEntry>> {
        Ok(Vec::new())
    }
    async fn insert(&mut self, _entries: &[HistoryEntry]) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl NoteRepository for DummyUnitOfWork {
    async fn list(&mut self, _filter: &NoteFilter) -> Result<Vec<Note>> {
        Ok(Vec::new())
    }
    async fn by_id(&mut self, _id: i32, _for_update: bool) -> Result<Option<Note>> {
        Ok(None)
    }
    async fn insert(&mut self, note: &NewNote) -> Result<Note> {
        Ok(Note {
            id: 1,
            board_id: note.board_id,
            author_id: note.author_id.clone(),
            author_name: note.author_name.clone(),
            title: note.title.clone(),
            body: note.body.clone(),
            source: note.source.clone(),
            labels: note.labels.clone(),
            seen_count: note.seen_count,
            seen_in: note.seen_in.clone(),
            state: note.state,
            ticket_id: note.ticket_id.clone(),
            reason: note.reason.clone(),
            created_at: note.created_at.clone(),
        })
    }
    async fn update(&mut self, _note: &Note) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl DeletionRepository for DummyUnitOfWork {
    async fn insert(&mut self, _deletion: &Deletion) -> Result<()> {
        Ok(())
    }
    async fn list(&mut self) -> Result<Vec<Deletion>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl BoardQuestionRepository for DummyUnitOfWork {
    async fn by_id(&mut self, _id: &str, _for_update: bool) -> Result<Option<BoardQuestion>> {
        Ok(None)
    }
    async fn list(&mut self, _filter: &BoardQuestionFilter) -> Result<Vec<BoardQuestion>> {
        Ok(Vec::new())
    }
    async fn insert(&mut self, _question: &BoardQuestion) -> Result<()> {
        Ok(())
    }
    async fn update(&mut self, _question: &BoardQuestion) -> Result<()> {
        Ok(())
    }
    async fn ids_referencing_ticket(&mut self, _ticket_id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
    async fn remove_ticket(&mut self, _ticket_id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl BoardQuestionResearchRepository for DummyUnitOfWork {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionResearch>> {
        Ok(Vec::new())
    }
    async fn insert(&mut self, research: &NewQuestionResearch) -> Result<BoardQuestionResearch> {
        Ok(BoardQuestionResearch {
            id: 1,
            question_id: research.question_id.clone(),
            at: research.at.clone(),
            by_id: research.by_id.clone(),
            by_name: research.by_name.clone(),
            kind: research.kind,
            context: research.context.clone(),
            options: research.options.clone(),
        })
    }
    async fn summaries(
        &mut self,
        _question_ids: &[String],
    ) -> Result<BTreeMap<String, BoardQuestionResearchSummary>> {
        Ok(BTreeMap::new())
    }
}

#[async_trait]
impl BoardQuestionCommentRepository for DummyUnitOfWork {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionComment>> {
        Ok(Vec::new())
    }
    async fn insert(&mut self, comment: &NewQuestionComment) -> Result<BoardQuestionComment> {
        Ok(BoardQuestionComment {
            id: 1,
            question_id: comment.question_id.clone(),
            author_id: comment.author_id.clone(),
            author_name: comment.author_name.clone(),
            author_kind: comment.author_kind,
            body: comment.body.clone(),
            created_at: comment.created_at.clone(),
        })
    }
}

#[async_trait]
impl BoardQuestionHistoryRepository for DummyUnitOfWork {
    async fn list(&mut self, _question_id: &str) -> Result<Vec<BoardQuestionHistoryEntry>> {
        Ok(Vec::new())
    }
    async fn insert(&mut self, _entries: &[BoardQuestionHistoryEntry]) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl BoardSearchRepository for DummyUnitOfWork {
    async fn find(&mut self, _request: &SearchRequest) -> Result<SearchRows> {
        Ok(SearchRows {
            rows: Vec::new(),
            truncated: false,
        })
    }
}

#[async_trait]
impl DispatchRunRepository for DummyUnitOfWork {
    async fn insert(&mut self, _run: &DispatchRun) -> Result<bool> {
        Ok(true)
    }
    async fn list(&mut self, _filter: &DispatchRunFilter) -> Result<Vec<DispatchRun>> {
        Ok(Vec::new())
    }
    async fn list_all(&mut self, _from: &str, _to: &str) -> Result<Vec<DispatchRun>> {
        Ok(Vec::new())
    }
    async fn first_started(
        &mut self,
        _project_id: i32,
        _source: DispatchRunSource,
    ) -> Result<Option<String>> {
        Ok(None)
    }
    async fn existing(&mut self, _ids: &[String]) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
    async fn record_config_version(&mut self, _version: &DispatchConfigVersion) -> Result<bool> {
        Ok(true)
    }
    async fn list_config_versions(
        &mut self,
        _from: &str,
        _to: &str,
    ) -> Result<Vec<DispatchConfigVersion>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl DispatchStatsRepository for DummyUnitOfWork {
    async fn done_ticket_ids(&mut self, _filter: &DispatchRunFilter) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
    async fn status_history(&mut self, _ticket_ids: &[String]) -> Result<Vec<HistoryEntry>> {
        Ok(Vec::new())
    }
    async fn dispatch_comment_counts(
        &mut self,
        _ticket_ids: &[String],
    ) -> Result<BTreeMap<String, DispatchCommentCounts>> {
        Ok(BTreeMap::new())
    }
    async fn dispatch_comments(&mut self, _project_id: i32) -> Result<Vec<Comment>> {
        Ok(Vec::new())
    }
}

// Alle 17 Zugriffe zeigen auf dasselbe Struct – genau so arbeiten SQLite und SQL Server
// mit ihrer einen Verbindung.
impl BoardUnitOfWork for DummyUnitOfWork {
    fn users(&mut self) -> &mut dyn UserRepository {
        self
    }
    fn projects(&mut self) -> &mut dyn ProjectRepository {
        self
    }
    fn boards(&mut self) -> &mut dyn BoardRepository {
        self
    }
    fn tickets(&mut self) -> &mut dyn TicketRepository {
        self
    }
    fn comments(&mut self) -> &mut dyn CommentRepository {
        self
    }
    fn steps(&mut self) -> &mut dyn TicketStepRepository {
        self
    }
    fn ticket_sessions(&mut self) -> &mut dyn TicketSessionRepository {
        self
    }
    fn history(&mut self) -> &mut dyn HistoryRepository {
        self
    }
    fn notes(&mut self) -> &mut dyn NoteRepository {
        self
    }
    fn deletions(&mut self) -> &mut dyn DeletionRepository {
        self
    }
    fn questions(&mut self) -> &mut dyn BoardQuestionRepository {
        self
    }
    fn question_research(&mut self) -> &mut dyn BoardQuestionResearchRepository {
        self
    }
    fn question_comments(&mut self) -> &mut dyn BoardQuestionCommentRepository {
        self
    }
    fn question_history(&mut self) -> &mut dyn BoardQuestionHistoryRepository {
        self
    }
    fn search(&mut self) -> &mut dyn BoardSearchRepository {
        self
    }
    fn dispatch_runs(&mut self) -> &mut dyn DispatchRunRepository {
        self
    }
    fn dispatch_stats(&mut self) -> &mut dyn DispatchStatsRepository {
        self
    }
}

/// Dummy-Datenbank: führt die Arbeit zweimal aus – wie SQLite, das bei einem Schreibkonflikt
/// wiederholt. Dass das überhaupt geht, ist der Beweis, dass die Arbeit `Fn` ist.
#[derive(Default)]
struct Dummy {
    runs: AtomicUsize,
}

impl BoardDatabase for Dummy {
    async fn transaction<T, W>(&self, work: W) -> Result<T>
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'static,
    {
        let mut uow = DummyUnitOfWork::default();
        self.runs.fetch_add(1, Ordering::SeqCst);
        // Erster Anlauf; sein Ergebnis wird verworfen, als wäre er am Schreibkonflikt gescheitert
        let _verworfen = work(&mut uow).await?;
        self.runs.fetch_add(1, Ordering::SeqCst);
        work(&mut uow).await
    }

    async fn change_marker(&self, board_id: i32) -> Result<String> {
        Ok(format!("board-{board_id}"))
    }

    async fn question_marker(&self, project_id: i32) -> Result<String> {
        Ok(format!("projekt-{project_id}"))
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }
}

/// Ein Ticket mit Pflichtfeldern, sonst leer
fn ticket(id: &str) -> Ticket {
    Ticket {
        id: id.into(),
        project_id: 1,
        board_id: 1,
        number: 1,
        title: "Titel".into(),
        description: String::new(),
        status: TicketStatus::Backlog,
        priority: Priority::Normal,
        labels: Vec::new(),
        depends_on: Vec::new(),
        sort_order: 0,
        assignee: None,
        needs_planning: false,
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
        created_at: "2026-10-28T00:00:00.000Z".into(),
        created_by_id: "11111111-1111-1111-1111-111111111111".into(),
        created_by_name: "claude".into(),
        updated_at: "2026-10-28T00:00:00.000Z".into(),
        rev: 1,
    }
}

#[tokio::test]
async fn transaktion_nimmt_die_aufrufform_aus_dem_plan() {
    let db = Dummy::default();
    let namen = db
        .transaction(|u| {
            Box::pin(async move {
                u.users()
                    .upsert(&BoardUser {
                        id: "11111111-1111-1111-1111-111111111111".into(),
                        name: "claude".into(),
                    })
                    .await?;
                let users = u.users().list().await?;
                Ok(users.into_iter().map(|user| user.name).collect::<Vec<_>>())
            })
        })
        .await
        .unwrap();

    assert_eq!(namen, ["claude"]);
}

#[tokio::test]
async fn arbeit_ist_mehrfach_aufrufbar() {
    let db = Dummy::default();
    // Die Werte werden im Closure geklont, damit die Arbeit `Fn` bleibt
    let id = String::from("NXS-0001");
    let anzahl = db
        .transaction(move |u| {
            let id = id.clone();
            Box::pin(async move {
                u.tickets().insert(&ticket(&id)).await?;
                Ok(u.tickets().list(&TicketFilter::default()).await?.len())
            })
        })
        .await
        .unwrap();

    // Die Dummy-Datenbank hat die Arbeit zweimal ausgeführt, auf derselben Unit of Work
    assert_eq!(db.runs.load(Ordering::SeqCst), 2);
    assert_eq!(anzahl, 2);
}

#[tokio::test]
async fn das_makro_erspart_box_pin() {
    let db = Dummy::default();
    let leer = tx!(db, |u| { u.projects().list().await }).await.unwrap();
    assert!(leer.is_empty());
}

#[tokio::test]
async fn kennwerte_und_schliessen_sind_teil_des_vertrags() {
    let db = Dummy::default();
    assert_eq!(db.change_marker(7).await.unwrap(), "board-7");
    assert_eq!(db.question_marker(3).await.unwrap(), "projekt-3");
    db.close().await.unwrap();
}

#[test]
fn filter_sind_ohne_angabe_leer() {
    assert_eq!(
        BoardFilter::default(),
        BoardFilter {
            project_id: None,
            owner_id: None
        }
    );
    assert!(!TicketFilter::default().without_texts);
    assert_eq!(BoardQuestionFilter::default().states, None);
    assert_eq!(NoteFilter::of_board(4).board_id, 4);
}
