//! Sperrt der Dienst die Notiz beim Übernehmen und Verwerfen (`for_update`)?
//! Portiert aus dem Fall „sperrt die Notiz beim Übernehmen und Verwerfen (forUpdate)“ in
//! `packages/core/tests/unit/board/NoteService.test.ts`; dort horcht ein Ersatz-Objekt die
//! Zugriffe ab, hier eine Datenbank, die jeden Zugriff durchreicht und mitschreibt.

use std::future::Future;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::board::database::{
    BoardDatabase, BoardQuestionCommentRepository, BoardQuestionHistoryRepository,
    BoardQuestionRepository, BoardQuestionResearchRepository, BoardRepository,
    BoardSearchRepository, BoardUnitOfWork, CommentRepository, DeletionRepository,
    DispatchRunRepository, DispatchStatsRepository, HistoryRepository, NewNote, NoteFilter,
    NoteRepository, ProjectRepository, TicketRepository, TicketSessionRepository,
    TicketStepRepository, TransactionWork, UserRepository,
};
use crate::board::in_memory::InMemoryBoardDatabase;
use crate::board::note_service::tests::{note, ticket};
use crate::board::note_service::{AddNoteOptions, NewNoteText, NoteService, TakeNoteOptions};
use crate::board::types::Note;
use crate::error::Result;
use crate::testing::BoardFixture;

/// Die IDs, die mit `for_update` gelesen wurden
type Locked = Arc<Mutex<Vec<i32>>>;

/// Eine Unit of Work, die alles durchreicht und die gesperrten Notizen mitschreibt
struct SpyUnit<'u> {
    inner: &'u mut dyn BoardUnitOfWork,
    locked: Locked,
}

#[async_trait]
impl NoteRepository for SpyUnit<'_> {
    async fn list(&mut self, filter: &NoteFilter) -> Result<Vec<Note>> {
        self.inner.notes().list(filter).await
    }

    async fn by_id(&mut self, id: i32, for_update: bool) -> Result<Option<Note>> {
        if for_update {
            self.locked
                .lock()
                .expect("die Aufzeichnung ist nicht vergiftet")
                .push(id);
        }
        self.inner.notes().by_id(id, for_update).await
    }

    async fn insert(&mut self, note: &NewNote) -> Result<Note> {
        self.inner.notes().insert(note).await
    }

    async fn update(&mut self, note: &Note) -> Result<()> {
        self.inner.notes().update(note).await
    }
}

impl BoardUnitOfWork for SpyUnit<'_> {
    fn users(&mut self) -> &mut dyn UserRepository {
        self.inner.users()
    }

    fn projects(&mut self) -> &mut dyn ProjectRepository {
        self.inner.projects()
    }

    fn boards(&mut self) -> &mut dyn BoardRepository {
        self.inner.boards()
    }

    fn tickets(&mut self) -> &mut dyn TicketRepository {
        self.inner.tickets()
    }

    fn comments(&mut self) -> &mut dyn CommentRepository {
        self.inner.comments()
    }

    fn steps(&mut self) -> &mut dyn TicketStepRepository {
        self.inner.steps()
    }

    fn ticket_sessions(&mut self) -> &mut dyn TicketSessionRepository {
        self.inner.ticket_sessions()
    }

    fn history(&mut self) -> &mut dyn HistoryRepository {
        self.inner.history()
    }

    /// Nur die Notizen gehen über die Aufzeichnung
    fn notes(&mut self) -> &mut dyn NoteRepository {
        self
    }

    fn deletions(&mut self) -> &mut dyn DeletionRepository {
        self.inner.deletions()
    }

    fn questions(&mut self) -> &mut dyn BoardQuestionRepository {
        self.inner.questions()
    }

    fn question_research(&mut self) -> &mut dyn BoardQuestionResearchRepository {
        self.inner.question_research()
    }

    fn question_comments(&mut self) -> &mut dyn BoardQuestionCommentRepository {
        self.inner.question_comments()
    }

    fn question_history(&mut self) -> &mut dyn BoardQuestionHistoryRepository {
        self.inner.question_history()
    }

    fn search(&mut self) -> &mut dyn BoardSearchRepository {
        self.inner.search()
    }

    fn dispatch_runs(&mut self) -> &mut dyn DispatchRunRepository {
        self.inner.dispatch_runs()
    }

    fn dispatch_stats(&mut self) -> &mut dyn DispatchStatsRepository {
        self.inner.dispatch_stats()
    }
}

/// Eine Datenbank, die jede Transaktion der Speicher-Datenbank überlässt und dabei die
/// gesperrten Notizen mitschreibt
struct SpyDatabase {
    inner: Arc<InMemoryBoardDatabase>,
    locked: Locked,
}

impl BoardDatabase for SpyDatabase {
    fn transaction<'a, T, W>(&'a self, work: W) -> impl Future<Output = Result<T>> + Send + 'a
    where
        T: Send + 'static,
        W: TransactionWork<T> + 'static,
    {
        let work = Arc::new(work);
        let locked = self.locked.clone();
        self.inner.transaction(move |u: &mut dyn BoardUnitOfWork| {
            let work = work.clone();
            let locked = locked.clone();
            Box::pin(async move {
                let mut spy = SpyUnit { inner: u, locked };
                work(&mut spy).await
            })
        })
    }

    fn change_marker(&self, board_id: i32) -> impl Future<Output = Result<String>> + Send + '_ {
        self.inner.change_marker(board_id)
    }

    fn question_marker(&self, project_id: i32) -> impl Future<Output = Result<String>> + Send + '_ {
        self.inner.question_marker(project_id)
    }

    fn close(&self) -> impl Future<Output = Result<()>> + Send + '_ {
        self.inner.close()
    }
}

#[tokio::test]
async fn sperrt_die_notiz_beim_ubernehmen_und_verwerfen() {
    let f = BoardFixture::create().await;
    ticket(&f, "Grundbestand").await;
    let a = note(&f, "A").await.unwrap();
    let b = note(&f, "B").await.unwrap();
    let locked: Locked = Arc::new(Mutex::new(Vec::new()));
    let notes = NoteService::new(
        Arc::new(SpyDatabase {
            inner: f.db.clone(),
            locked: locked.clone(),
        }),
        f.clock.clone(),
    );

    notes.get(a.id).await.unwrap();
    assert!(locked.lock().unwrap().is_empty());

    notes
        .take(a.id, &f.actor, &TakeNoteOptions::default())
        .await
        .unwrap();
    notes.drop_note(b.id, "egal").await.unwrap();

    assert_eq!(*locked.lock().unwrap(), [a.id, b.id]);
}

/// Die Aufzeichnung reicht auch das Anlegen durch; sonst wäre der Test oben wertlos
#[tokio::test]
async fn reicht_die_ubrigen_zugriffe_durch() {
    let f = BoardFixture::create().await;
    let locked: Locked = Arc::new(Mutex::new(Vec::new()));
    let notes = NoteService::new(
        Arc::new(SpyDatabase {
            inner: f.db.clone(),
            locked: locked.clone(),
        }),
        f.clock.clone(),
    );

    let n = notes
        .add(
            f.board_id,
            &NewNoteText::titled("Durchgereicht"),
            &f.actor,
            &AddNoteOptions::default(),
        )
        .await
        .unwrap();

    assert_eq!(f.notes.get(n.id).await.unwrap().title, "Durchgereicht");
    assert!(locked.lock().unwrap().is_empty());
}
