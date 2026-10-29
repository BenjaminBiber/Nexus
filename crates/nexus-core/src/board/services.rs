//! Alle Dienste des Boards über einer Datenbank (für CLI und Server).
//! Portiert aus `packages/core/src/board/BoardServices.ts`.

use std::sync::Arc;

use crate::board::board_service::BoardService;
use crate::board::comment_service::CommentService;
use crate::board::database::BoardDatabase;
use crate::board::dependency_graph_service::DependencyGraphService;
use crate::board::note_service::NoteService;
use crate::board::project_service::ProjectService;
use crate::board::question_service::BoardQuestionService;
use crate::board::repo_paths::BoardRepoPaths;
use crate::board::ticket_service::TicketService;
use crate::board::view_service::BoardViewService;
use crate::clock::Clock;

/// Die Dienste des Boards über einer Datenbank. `repo_paths` liefert die eigenen Repo-Pfade des
/// Benutzers (Standard: im Speicher ohne Benutzer, für Tests; die Laufzeit gibt
/// `board-repos.yaml` und die eigene GUID mit).
///
/// Die Felder `planning`, `search`, `dispatch_runs`, `inbox` und die Zeitleiste
/// (`timeline(names)`) ergänzen die Tickets P3-3 und P3-4, sobald es die Dienste gibt.
#[derive(Debug, Clone)]
pub struct BoardServices<D: BoardDatabase> {
    pub db: Arc<D>,
    pub repo_paths: BoardRepoPaths,
    pub projects: ProjectService<D>,
    pub boards: BoardService<D>,
    pub tickets: TicketService<D>,
    pub comments: CommentService<D>,
    pub notes: NoteService<D>,
    pub questions: BoardQuestionService<D>,
    pub views: BoardViewService<D>,
    pub graph: DependencyGraphService<D>,
}

impl<D: BoardDatabase> BoardServices<D> {
    /// Alle Dienste auf dieser Datenbank mit dieser Uhr und diesen Repo-Pfaden
    pub fn new(db: Arc<D>, clock: Clock, repo_paths: BoardRepoPaths) -> Self {
        Self {
            projects: ProjectService::new(db.clone(), clock.clone()),
            boards: BoardService::new(db.clone(), clock.clone(), repo_paths.clone()),
            tickets: TicketService::new(db.clone(), clock.clone()),
            comments: CommentService::new(db.clone(), clock.clone()),
            notes: NoteService::new(db.clone(), clock.clone()),
            questions: BoardQuestionService::new(db.clone(), clock.clone(), repo_paths.clone()),
            views: BoardViewService::new(db.clone(), clock.clone()),
            graph: DependencyGraphService::new(db.clone(), clock),
            repo_paths,
            db,
        }
    }

    /// Die Dienste ohne lokale Zuordnung der Repo-Pfade (der Standardwert aus TypeScript)
    pub fn with_memory_paths(db: Arc<D>, clock: Clock) -> Self {
        Self::new(db, clock, BoardRepoPaths::memory())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::BoardServices;
    use crate::board::database::BoardDatabase;
    use crate::board::in_memory::InMemoryBoardDatabase;
    use crate::board::note_service::{AddNoteOptions, NewNoteText};
    use crate::board::ticket_service::{NewTicketInput, TicketListFilter, WriteOptions};
    use crate::board::types::BoardUser;
    use crate::clock::Clock;
    use crate::testing::{USER_ID, USER_NAME};
    use crate::tx;

    /// Alle Dienste arbeiten auf derselben Datenbank: ein Projekt, ein Board, ein Ticket, ein
    /// Kommentar und eine Notiz in einem Durchlauf
    #[tokio::test]
    async fn stellt_alle_bis_hierher_portierten_dienste_uber_einer_datenbank_bereit() {
        let db = Arc::new(InMemoryBoardDatabase::new());
        let clock = Clock::fixed(
            "2026-10-13T10:00:00.000Z"
                .parse()
                .expect("der Zeitpunkt ist gültig"),
        );
        tx!(db, |u| {
            u.users()
                .upsert(&BoardUser {
                    id: USER_ID.into(),
                    name: USER_NAME.into(),
                })
                .await
        })
        .await
        .unwrap();
        let services = BoardServices::with_memory_paths(db.clone(), clock);
        let actor = crate::board::basics::BoardActor {
            user_id: USER_ID.into(),
            name: "claude".into(),
        };

        services
            .projects
            .create("ARC", "Arches", &actor)
            .await
            .unwrap();
        let board = services
            .boards
            .create("ARC", "Sprint", &actor)
            .await
            .unwrap();
        let ticket = services
            .tickets
            .create(
                board.board.id,
                &NewTicketInput::titled("Login bauen"),
                &actor,
                &WriteOptions::default(),
            )
            .await
            .unwrap();
        services
            .comments
            .add(&ticket.ticket.id, "Zwischenstand", &actor, false)
            .await
            .unwrap();
        let note = services
            .notes
            .add(
                board.board.id,
                &NewNoteText::titled("Cache läuft nie ab"),
                &actor,
                &AddNoteOptions::default(),
            )
            .await
            .unwrap();

        assert_eq!(ticket.ticket.id, "ARC-0001");
        assert_eq!(
            services
                .views
                .get(board.board.id)
                .await
                .unwrap()
                .columns
                .iter()
                .flat_map(|column| column.tickets.iter())
                .map(|card| (card.id.as_str(), card.comment_count))
                .collect::<Vec<_>>(),
            [("ARC-0001", 1)]
        );
        assert_eq!(
            services
                .tickets
                .list(&TicketListFilter {
                    board_ids: vec![board.board.id],
                    ..TicketListFilter::default()
                })
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            services.notes.get(note.id).await.unwrap().title,
            "Cache läuft nie ab"
        );
        assert!(services.repo_paths.one(board.board.clone()).is_ok());
        // Dieselbe Datenbank wie die hineingereichte
        assert_eq!(
            services.db.change_marker(board.board.id).await.unwrap(),
            db.change_marker(board.board.id).await.unwrap()
        );
    }
}
