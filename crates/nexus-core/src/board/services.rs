//! Alle Dienste des Boards über einer Datenbank (für CLI und Server).
//! Portiert aus `packages/core/src/board/BoardServices.ts`.

use std::sync::Arc;

use crate::board::board_service::BoardService;
use crate::board::comment_service::CommentService;
use crate::board::database::BoardDatabase;
use crate::board::dependency_graph_service::DependencyGraphService;
use crate::board::note_service::NoteService;
use crate::board::project_service::ProjectService;
use crate::board::repo_paths::BoardRepoPaths;
use crate::board::ticket_service::TicketService;
use crate::board::view_service::BoardViewService;
use crate::clock::Clock;

/// Die Dienste des Boards über einer Datenbank. `repo_paths` liefert die eigenen Repo-Pfade des
/// Benutzers (Standard: im Speicher ohne Benutzer, für Tests; die Laufzeit gibt
/// `board-repos.yaml` und die eigene GUID mit).
///
/// Die Felder `planning`, `questions`, `search`, `dispatch_runs`, `inbox` und die Zeitleiste
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
