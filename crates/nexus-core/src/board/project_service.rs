//! Projekte anlegen, auflisten und umbenennen. Ein Projekt hat einen Key (Ticket-Präfix) und
//! einen Namen. Portiert aus `packages/core/src/board/ProjectService.ts`.

use std::sync::Arc;

use crate::board::basics::{BoardActor, MAX_PROJECT_NAME_LENGTH};
use crate::board::database::{BoardDatabase, BoardUnitOfWork, NewProject};
use crate::board::limits::BoardLimits;
use crate::board::ticket_ids::TicketIds;
use crate::board::types::Project;
use crate::clock::Clock;
use crate::error::{HarnessError, Result};
use crate::tx;

/// Projekte des Boards.
#[derive(Debug, Clone)]
pub struct ProjectService<D: BoardDatabase> {
    db: Arc<D>,
    clock: Clock,
}

impl<D: BoardDatabase> ProjectService<D> {
    /// Dienst auf dieser Datenbank mit dieser Uhr
    pub fn new(db: Arc<D>, clock: Clock) -> Self {
        Self { db, clock }
    }

    /// Alle Projekte nach Key
    pub async fn list(&self) -> Result<Vec<Project>> {
        tx!(self.db, |u| { u.projects().list().await }).await
    }

    /// Legt ein Projekt an; der Key wird normalisiert und muss frei sein
    pub async fn create(&self, key: &str, name: &str, actor: &BoardActor) -> Result<Project> {
        let normalized = TicketIds::normalize_key(key)?;
        let title = Self::require_name(name)?;
        let clock = self.clock.clone();
        let actor = actor.clone();
        tx!(self.db, [normalized, title, clock, actor], |u| {
            if u.projects().by_key(&normalized).await?.is_some() {
                return Err(HarnessError::new(
                    "project-exists",
                    format!("Projekt {normalized} gibt es schon."),
                )
                .into());
            }
            u.projects()
                .insert(&NewProject {
                    key: normalized.clone(),
                    name: title.clone(),
                    created_at: clock.iso(),
                    created_by: actor.user_id.clone(),
                })
                .await
        })
        .await
    }

    /// Benennt ein Projekt um
    pub async fn rename(&self, key: &str, name: &str) -> Result<Project> {
        let title = Self::require_name(name)?;
        let key = key.to_string();
        tx!(self.db, [key, title], |u| {
            let project = Self::require_in(u, &key).await?;
            u.projects().rename(project.id, &title).await?;
            Ok(Project {
                name: title.clone(),
                ..project
            })
        })
        .await
    }

    /// Projekt zum Key innerhalb einer Transaktion oder Fehler (`get` in TypeScript)
    pub async fn require_in(u: &mut dyn BoardUnitOfWork, key: &str) -> Result<Project> {
        let normalized = TicketIds::normalize_key(key)?;
        u.projects()
            .by_key(&normalized)
            .await?
            .ok_or_else(|| {
                HarnessError::new(
                    "project-not-found",
                    format!("Projekt {} gibt es nicht.", key.to_uppercase()),
                )
                .into()
            })
    }

    /// Name ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
    fn require_name(name: &str) -> Result<String> {
        let title = name.trim();
        if title.is_empty() {
            return Err(HarnessError::new(
                "project-name-empty",
                "Der Projektname darf nicht leer sein.",
            )
            .into());
        }
        BoardLimits::text(title, MAX_PROJECT_NAME_LENGTH, "Der Projektname")?;
        Ok(title.to_string())
    }
}
