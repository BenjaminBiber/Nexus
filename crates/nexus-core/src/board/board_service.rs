//! Boards eines Projekts: anlegen, auflisten, umbenennen, eigenen Repo-Pfad setzen, löschen.
//! Portiert aus `packages/core/src/board/BoardService.ts`.
//!
//! Alle gelieferten Boards tragen in `repo_path` den wirksamen Pfad des Benutzers
//! ([`BoardRepoPaths`]), nur [`BoardService::require_in`] den DB-Wert.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::board::basics::{BoardActor, TicketStatus, MAX_BOARD_NAME_LENGTH};
use crate::board::database::{BoardDatabase, BoardFilter, BoardUnitOfWork, NewBoard, TicketFilter};
use crate::board::dependency_cleanup::DependencyCleanup;
use crate::board::limits::BoardLimits;
use crate::board::project_service::ProjectService;
use crate::board::question_ticket_cleanup::{DeletedTicket, QuestionTicketCleanup};
use crate::board::repo_board_resolver::{RepoBoardOwner, RepoBoardResolver};
use crate::board::repo_paths::{BoardRepoPaths, BoardRepoSource, BoardWithRepo, RepoPathTarget};
use crate::board::types::{Board, Deletion};
use crate::clock::Clock;
use crate::error::{HarnessError, Result};
use crate::tx;

/// Board mit Projekt, Besitzer und der Zahl der offenen Tickets. `repo_path` ist der wirksame
/// Pfad des Benutzers (lokal oder Altbestand), `repo_source` seine Herkunft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct BoardSummary {
    #[serde(flatten)]
    #[ts(flatten)]
    pub board: Board,
    pub project_key: String,
    pub project_name: String,
    pub owner_name: String,
    pub open_tickets: i64,
    pub repo_source: Option<BoardRepoSource>,
}

impl RepoPathTarget for BoardSummary {
    fn board_id(&self) -> i32 {
        self.board.id
    }

    fn owner_id(&self) -> &str {
        &self.board.owner_id
    }

    fn repo_path(&self) -> Option<&str> {
        self.board.repo_path.as_deref()
    }

    fn set_repo_path(&mut self, path: Option<String>) {
        self.board.repo_path = path;
    }

    fn set_repo_source(&mut self, source: Option<BoardRepoSource>) {
        self.repo_source = source;
    }
}

/// Auswahl beim Auflisten der Boards
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardListFilter {
    pub project_key: Option<String>,
    pub owner_id: Option<String>,
}

/// Was beim Löschen eines Boards entfernt wurde
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "board.ts")]
pub struct BoardDeleteResult {
    pub deleted_tickets: Vec<String>,
}

/// Boards des Boards – anlegen, lesen, ändern, löschen.
#[derive(Debug, Clone)]
pub struct BoardService<D: BoardDatabase> {
    db: Arc<D>,
    clock: Clock,
    repo_paths: BoardRepoPaths,
    repos: RepoBoardResolver,
}

impl<D: BoardDatabase> BoardService<D> {
    /// Dienst mit eigener Zuordnung der Repo-Pfade
    pub fn new(db: Arc<D>, clock: Clock, repo_paths: BoardRepoPaths) -> Self {
        Self {
            db,
            clock,
            repo_paths,
            repos: RepoBoardResolver::new(),
        }
    }

    /// Dienst ohne lokale Zuordnung (wie der Standardwert in TypeScript)
    pub fn with_memory_paths(db: Arc<D>, clock: Clock) -> Self {
        Self::new(db, clock, BoardRepoPaths::memory())
    }

    /// Boards mit Projekt, Besitzer und Zahl der offenen Tickets, sortiert nach Projekt und Name
    pub async fn list(&self, filter: &BoardListFilter) -> Result<Vec<BoardSummary>> {
        let filter = filter.clone();
        let mut list: Vec<BoardSummary> = tx!(self.db, [filter], |u| {
            let project_id = match &filter.project_key {
                Some(key) => Some(ProjectService::<D>::require_in(u, key).await?.id),
                None => None,
            };
            let boards = u
                .boards()
                .list(&BoardFilter {
                    project_id,
                    owner_id: filter.owner_id.clone(),
                })
                .await?;
            let projects: HashMap<i32, (String, String)> = u
                .projects()
                .list()
                .await?
                .into_iter()
                .map(|project| (project.id, (project.key, project.name)))
                .collect();
            let users: HashMap<String, String> = u
                .users()
                .list()
                .await?
                .into_iter()
                .map(|user| (user.id, user.name))
                .collect();
            let open: BTreeMap<i32, i64> = if boards.is_empty() {
                BTreeMap::new()
            } else {
                let ids: Vec<i32> = boards.iter().map(|board| board.id).collect();
                u.tickets().open_count_by_board(&ids).await?
            };
            let mut summaries: Vec<BoardSummary> = boards
                .into_iter()
                .map(|board| {
                    let project = projects.get(&board.project_id);
                    BoardSummary {
                        project_key: project.map_or("?", |p| p.0.as_str()).to_string(),
                        project_name: project.map_or("?", |p| p.1.as_str()).to_string(),
                        owner_name: users
                            .get(&board.owner_id)
                            .cloned()
                            .unwrap_or_else(|| board.owner_id.clone()),
                        open_tickets: open.get(&board.id).copied().unwrap_or(0),
                        repo_source: None,
                        board,
                    }
                })
                .collect();
            // Wie `localeCompare` in TypeScript; Projektkeys sind ASCII, Namen vergleicht Rust
            // nach Zeichen
            summaries.sort_by(|a, b| {
                a.project_key
                    .cmp(&b.project_key)
                    .then_with(|| a.board.name.cmp(&b.board.name))
            });
            Ok(summaries)
        })
        .await?;
        self.repo_paths.apply_to(&mut list)?;
        Ok(list)
    }

    /// Eigene Boards eines Benutzers mit wirksamem Repo-Pfad
    pub async fn owned(&self, owner_id: &str) -> Result<Vec<BoardWithRepo>> {
        let owner_id = owner_id.to_string();
        let boards = tx!(self.db, [owner_id], |u| {
            u.boards()
                .list(&BoardFilter {
                    project_id: None,
                    owner_id: Some(owner_id.clone()),
                })
                .await
        })
        .await?;
        self.repo_paths.apply(boards)
    }

    /// Board zur ID mit wirksamem Repo-Pfad oder `None`
    pub async fn find(&self, id: i32) -> Result<Option<BoardWithRepo>> {
        let board = tx!(self.db, |u| { u.boards().by_id(id).await }).await?;
        match board {
            Some(board) => Ok(Some(self.repo_paths.one(board)?)),
            None => Ok(None),
        }
    }

    /// Board zur ID innerhalb einer Transaktion oder Fehler (`get` in TypeScript); `repo_path`
    /// ist hier der DB-Wert (Altbestand), nicht der wirksame Pfad
    pub async fn require_in(u: &mut dyn BoardUnitOfWork, id: i32) -> Result<Board> {
        u.boards().by_id(id).await?.ok_or_else(|| {
            HarnessError::new("board-not-found", format!("Board {id} gibt es nicht.")).into()
        })
    }

    /// Board zur ID mit wirksamem Repo-Pfad in eigener Transaktion oder Fehler `board-not-found`
    pub async fn require(&self, id: i32) -> Result<BoardWithRepo> {
        let board = tx!(self.db, |u| { Self::require_in(u, id).await }).await?;
        self.repo_paths.one(board)
    }

    /// Board, dessen wirksamer Repo-Pfad zum Hauptordner des Git-Repos passt, oder `None`
    pub async fn for_repo(
        &self,
        repo_root: &str,
        me: &RepoBoardOwner,
    ) -> Result<Option<BoardWithRepo>> {
        let boards = tx!(self.db, |u| { u.boards().list(&BoardFilter::default()).await }).await?;
        Ok(self
            .repos
            .pick(self.repo_paths.apply(boards)?, repo_root, me))
    }

    /// Wie [`BoardService::for_repo`], ohne Treffer mit Hinweis auf „harness board boards repo“
    pub async fn require_for_repo(
        &self,
        repo_root: &str,
        me: &RepoBoardOwner,
    ) -> Result<BoardWithRepo> {
        self.for_repo(repo_root, me).await?.ok_or_else(|| {
            HarnessError::new(
                "board-repo-unknown",
                format!("Kein Board ist dem Repo {repo_root} zugeordnet. Mit „harness board boards repo <id> {repo_root}“ zuordnen."),
            )
            .into()
        })
    }

    /// Legt ein Board im Projekt an; Besitzer ist der handelnde Benutzer
    pub async fn create(
        &self,
        project_key: &str,
        name: &str,
        actor: &BoardActor,
    ) -> Result<BoardWithRepo> {
        let title = Self::require_name(name)?;
        let project_key = project_key.to_string();
        let clock = self.clock.clone();
        let actor = actor.clone();
        let created = tx!(self.db, [project_key, title, clock, actor], |u| {
            let project = ProjectService::<D>::require_in(u, &project_key).await?;
            u.boards()
                .insert(&NewBoard {
                    project_id: project.id,
                    owner_id: actor.user_id.clone(),
                    name: title.clone(),
                    repo_path: None,
                    created_at: clock.iso(),
                })
                .await
        })
        .await?;
        self.repo_paths.one(created)
    }

    /// Benennt ein Board um
    pub async fn rename(&self, id: i32, name: &str) -> Result<BoardWithRepo> {
        let title = Self::require_name(name)?;
        // Liest, ändert und schreibt das Board in einer Transaktion; der DB-Wert des Repo-Pfads
        // bleibt unverändert
        let next = tx!(self.db, [title], |u| {
            let changed = Board {
                name: title.clone(),
                ..Self::require_in(u, id).await?
            };
            u.boards().update(&changed).await?;
            Ok(changed)
        })
        .await?;
        self.repo_paths.one(next)
    }

    /// Setzt den eigenen Repo-Pfad des Benutzers (lokal, nicht in der gemeinsamen DB);
    /// leer oder `None` hinterlegt „kein Pfad“
    pub async fn set_repo_path(&self, id: i32, path: Option<&str>) -> Result<BoardWithRepo> {
        let board = tx!(self.db, |u| { Self::require_in(u, id).await }).await?;
        self.repo_paths.set(id, path)?;
        self.repo_paths.one(board)
    }

    /// Löscht ein Board; offene Tickets nur mit `delete_open`. Jede Ticketlöschung wird
    /// protokolliert.
    pub async fn delete(
        &self,
        id: i32,
        delete_open: bool,
        actor: &BoardActor,
    ) -> Result<BoardDeleteResult> {
        let clock = self.clock.clone();
        let actor = actor.clone();
        tx!(self.db, [clock, actor], |u| {
            BoardLimits::actor(&actor)?;
            Self::require_in(u, id).await?;
            let mut tickets = u
                .tickets()
                .list(&TicketFilter {
                    board_ids: Some(vec![id]),
                    ..TicketFilter::default()
                })
                .await?;
            tickets.sort_by_key(|ticket| ticket.number);
            let open: Vec<&str> = tickets
                .iter()
                .filter(|ticket| {
                    !matches!(ticket.status, TicketStatus::Done | TicketStatus::NotNeeded)
                })
                .map(|ticket| ticket.id.as_str())
                .collect();
            if !open.is_empty() && !delete_open {
                return Err(HarnessError::new(
                    "board-has-open-tickets",
                    format!(
                        "Auf dem Board liegen noch offene Tickets: {}. Erst verschieben oder ausdrücklich alles löschen.",
                        open.join(", ")
                    ),
                )
                .into());
            }
            let at = clock.iso();
            let deleted: HashSet<String> =
                tickets.iter().map(|ticket| ticket.id.clone()).collect();
            for ticket in &tickets {
                DependencyCleanup::remove(u, &ticket.id, &actor, &at, &deleted).await?;
            }
            for ticket in &tickets {
                QuestionTicketCleanup::remove(
                    u,
                    &DeletedTicket {
                        id: ticket.id.clone(),
                        title: ticket.title.clone(),
                    },
                    &actor,
                    &at,
                )
                .await?;
            }
            for ticket in &tickets {
                u.tickets().delete(&ticket.id).await?;
                u.deletions()
                    .insert(&Deletion {
                        at: at.clone(),
                        by_id: actor.user_id.clone(),
                        by_name: actor.name.clone(),
                        ticket_id: ticket.id.clone(),
                        ticket_title: ticket.title.clone(),
                        from_status: Some(ticket.status),
                    })
                    .await?;
            }
            u.boards().delete(id).await?;
            Ok(BoardDeleteResult {
                deleted_tickets: tickets.iter().map(|ticket| ticket.id.clone()).collect(),
            })
        })
        .await
    }

    /// Name ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
    fn require_name(name: &str) -> Result<String> {
        let title = name.trim();
        if title.is_empty() {
            return Err(HarnessError::new(
                "board-name-empty",
                "Der Boardname darf nicht leer sein.",
            )
            .into());
        }
        BoardLimits::text(title, MAX_BOARD_NAME_LENGTH, "Der Boardname")?;
        Ok(title.to_string())
    }
}
