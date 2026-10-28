//! Testumgebung der Board-Dienste: Datenbank im Speicher mit Benutzer, Projekt ARC und einem
//! Board; die Uhr lässt sich vorstellen.
//! Portiert aus `packages/core/tests/unit/board/helpers/BoardFixture.ts`.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};

use crate::board::basics::BoardActor;
use crate::board::board_service::BoardService;
use crate::board::comment_service::CommentService;
use crate::board::database::BoardDatabase;
use crate::board::dependency_graph_service::DependencyGraphService;
use crate::board::in_memory::InMemoryBoardDatabase;
use crate::board::project_service::ProjectService;
use crate::board::repo_paths::{BoardRepoPaths, MemoryBoardRepoStore};
use crate::board::types::Ticket;
use crate::board::view_service::BoardViewService;
use crate::clock::Clock;
use crate::testing::fixtures::{ticket_fixture, user, NOW, USER_ID};

/// Die Dienste des Boards auf einer Datenbank im Speicher.
pub struct BoardFixture {
    /// Der Zeitpunkt, den die Uhr liefert; [`BoardFixture::advance`] stellt ihn vor
    now: Arc<Mutex<DateTime<Utc>>>,
    pub clock: Clock,
    pub db: Arc<InMemoryBoardDatabase>,
    pub actor: BoardActor,
    /// Lokale Repo-Pfade des Testbenutzers, gemeinsam für alle Dienste der Umgebung
    pub repo_store: Arc<MemoryBoardRepoStore>,
    pub repo_paths: BoardRepoPaths,
    pub projects: ProjectService<InMemoryBoardDatabase>,
    pub boards: BoardService<InMemoryBoardDatabase>,
    pub comments: CommentService<InMemoryBoardDatabase>,
    pub views: BoardViewService<InMemoryBoardDatabase>,
    pub graph: DependencyGraphService<InMemoryBoardDatabase>,
    /// Das Board „Sprint“ des Projekts ARC
    pub board_id: i32,
}

impl BoardFixture {
    /// Die Dienste ohne Grundbestand (Benutzer, Projekt und Board fehlen noch)
    pub fn empty() -> Self {
        let now: Arc<Mutex<DateTime<Utc>>> = Arc::new(Mutex::new(
            NOW.parse().expect("der Zeitpunkt der Vorlage ist gültig"),
        ));
        let clock = Clock::from_fn({
            let now = Arc::clone(&now);
            move || *now.lock().expect("die Uhr ist nicht vergiftet")
        });
        let db = Arc::new(InMemoryBoardDatabase::new());
        let repo_store = Arc::new(MemoryBoardRepoStore::new());
        let repo_paths = BoardRepoPaths::new(repo_store.clone(), Some(USER_ID.to_string()));
        Self {
            now,
            actor: BoardActor {
                user_id: USER_ID.into(),
                name: "claude".into(),
            },
            projects: ProjectService::new(db.clone(), clock.clone()),
            boards: BoardService::new(db.clone(), clock.clone(), repo_paths.clone()),
            comments: CommentService::new(db.clone(), clock.clone()),
            views: BoardViewService::new(db.clone(), clock.clone()),
            graph: DependencyGraphService::new(db.clone(), clock.clone()),
            repo_store,
            repo_paths,
            clock,
            db,
            board_id: 0,
        }
    }

    /// Legt Benutzer, Projekt ARC und das Board „Sprint“ an
    pub async fn create() -> Self {
        let mut fixture = Self::empty();
        crate::tx!(fixture.db, |u| { u.users().upsert(&user()).await })
            .await
            .expect("Benutzer anlegen geht immer");
        fixture
            .projects
            .create("ARC", "Arches", &fixture.actor)
            .await
            .expect("Projekt anlegen geht immer");
        fixture.board_id = fixture
            .boards
            .create("ARC", "Sprint", &fixture.actor)
            .await
            .expect("Board anlegen geht immer")
            .id;
        fixture
    }

    /// Die ID des Projekts ARC
    pub async fn project_id(&self) -> i32 {
        crate::tx!(self.db, |u| { u.projects().by_key("ARC").await })
            .await
            .expect("Projekte lesen geht immer")
            .expect("Projekt ARC gehört zum Grundbestand")
            .id
    }

    /// Eine Vorlage für ein Ticket des Boards „Sprint“; Abweichungen setzt der Aufrufer mit
    /// der Struktur-Aktualisierung
    pub async fn new_ticket(&self, number: i32) -> Ticket {
        ticket_fixture(
            &format!("ARC-{number:04}"),
            self.project_id().await,
            self.board_id,
            number,
        )
    }

    /// Legt ein Ticket unmittelbar über die Repositories an. `TicketService::create` bringt
    /// erst Teil 2 des Tickets P3-2; hier zählt nur, dass das Ticket in der Datenbank steht.
    pub async fn insert_ticket(&self, ticket: &Ticket) {
        let ticket = ticket.clone();
        crate::tx!(self.db, [ticket], |u| { u.tickets().insert(&ticket).await })
            .await
            .expect("Ticket anlegen geht immer");
    }

    /// Stellt die Uhr um Minuten vor
    pub fn advance(&self, minutes: i64) {
        let mut now = self.now.lock().expect("die Uhr ist nicht vergiftet");
        *now += Duration::minutes(minutes);
    }
}
