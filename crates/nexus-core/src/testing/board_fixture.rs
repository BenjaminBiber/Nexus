//! Testumgebung der Board-Dienste: Datenbank im Speicher mit Benutzer, Projekt ARC und einem
//! Board; die Uhr lässt sich vorstellen.
//! Portiert aus `packages/core/tests/unit/board/helpers/BoardFixture.ts`.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};

use crate::actors::AuthorKind;
use crate::board::basics::BoardActor;
use crate::board::board_service::BoardService;
use crate::board::comment_service::CommentService;
use crate::board::database::BoardDatabase;
use crate::board::dependency_graph_service::DependencyGraphService;
use crate::board::in_memory::InMemoryBoardDatabase;
use crate::board::note_service::NoteService;
use crate::board::project_service::ProjectService;
use crate::board::question_service::BoardQuestionService;
use crate::board::question_types::{BoardQuestion, BoardQuestionActor};
use crate::board::repo_paths::{BoardRepoPaths, MemoryBoardRepoStore};
use crate::board::ticket_service::TicketService;
use crate::board::types::Ticket;
use crate::board::view_service::BoardViewService;
use crate::clock::Clock;
use crate::questions::QuestionState;
use crate::testing::fixtures::{question_fixture, ticket_fixture, user, NOW, USER_ID};

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
    pub tickets: TicketService<InMemoryBoardDatabase>,
    pub comments: CommentService<InMemoryBoardDatabase>,
    pub notes: NoteService<InMemoryBoardDatabase>,
    pub questions: BoardQuestionService<InMemoryBoardDatabase>,
    /// Session-Agent und Mensch als Handelnde bei Fragen
    pub agent: BoardQuestionActor,
    pub human: BoardQuestionActor,
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
            tickets: TicketService::new(db.clone(), clock.clone()),
            comments: CommentService::new(db.clone(), clock.clone()),
            notes: NoteService::new(db.clone(), clock.clone()),
            questions: BoardQuestionService::new(db.clone(), clock.clone(), repo_paths.clone()),
            agent: BoardQuestionActor {
                actor: BoardActor {
                    user_id: USER_ID.into(),
                    name: "claude".into(),
                },
                kind: AuthorKind::Session,
                session: Some("s-1".into()),
            },
            human: BoardQuestionActor {
                actor: BoardActor {
                    user_id: USER_ID.into(),
                    name: "Florian".into(),
                },
                kind: AuthorKind::Human,
                session: None,
            },
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

    /// Legt ein Ticket unmittelbar über die Repositories an – für Tests, die nur einen Bestand
    /// brauchen und nicht die Regeln von [`TicketService::create`]
    pub async fn insert_ticket(&self, ticket: &Ticket) {
        let ticket = ticket.clone();
        crate::tx!(self.db, [ticket], |u| { u.tickets().insert(&ticket).await })
            .await
            .expect("Ticket anlegen geht immer");
    }

    /// Legt eine offene Frage zu diesen Tickets unmittelbar über die Repositories an und liefert
    /// ihre ID. `BoardQuestionService` bringt erst P3-3; die abgeleitete Sperre hängt allein an
    /// der Zeile in `board.Questions`.
    pub async fn open_question(&self, number: i32, title: &str, ticket_ids: &[&str]) -> String {
        let question = BoardQuestion {
            title: title.to_string(),
            ticket_ids: ticket_ids.iter().map(|id| id.to_string()).collect(),
            ..question_fixture(
                &format!("ARC-F{number:04}"),
                self.project_id().await,
                number,
            )
        };
        crate::tx!(self.db, [question], |u| {
            u.questions().insert(&question).await
        })
        .await
        .expect("Frage anlegen geht immer");
        format!("ARC-F{number:04}")
    }

    /// Beantwortet eine Frage unmittelbar über die Repositories (das Gegenstück zu
    /// `BoardQuestionService::answer` aus P3-3): danach sperrt sie ihre Tickets nicht mehr
    pub async fn answer_question(&self, id: &str) {
        let id = id.to_string();
        crate::tx!(self.db, [id], |u| {
            let question = u
                .questions()
                .by_id(&id, true)
                .await?
                .expect("die Frage wurde angelegt");
            u.questions()
                .update(&BoardQuestion {
                    state: QuestionState::Answered,
                    rev: question.rev + 1,
                    ..question
                })
                .await
        })
        .await
        .expect("Frage beantworten geht immer");
    }

    /// Stellt die Uhr um Minuten vor
    pub fn advance(&self, minutes: i64) {
        let mut now = self.now.lock().expect("die Uhr ist nicht vergiftet");
        *now += Duration::minutes(minutes);
    }
}
