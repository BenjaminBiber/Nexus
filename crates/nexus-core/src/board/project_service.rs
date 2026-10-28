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
        u.projects().by_key(&normalized).await?.ok_or_else(|| {
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

#[cfg(test)]
mod tests {
    use crate::testing::BoardFixture;

    #[tokio::test]
    async fn legt_ein_projekt_mit_normalisiertem_key_an_und_listet_es() {
        let f = BoardFixture::create().await;

        let project = f
            .projects
            .create("inc", "Incidents", &f.actor)
            .await
            .unwrap();

        assert_eq!(project.key, "INC");
        assert_eq!(project.name, "Incidents");
        assert_eq!(project.next_ticket_number, 1);
        assert_eq!(project.created_by, f.actor.user_id);
        assert_eq!(
            f.projects
                .list()
                .await
                .unwrap()
                .iter()
                .map(|project| project.key.as_str())
                .collect::<Vec<_>>(),
            ["ARC", "INC"]
        );
    }

    #[tokio::test]
    async fn lehnt_einen_doppelten_key_ab() {
        let f = BoardFixture::create().await;

        let error = f
            .projects
            .create("ARC", "Nochmal", &f.actor)
            .await
            .unwrap_err();

        assert_eq!(error.code(), Some("project-exists"));
    }

    #[tokio::test]
    async fn lehnt_einen_leeren_namen_ab() {
        let f = BoardFixture::create().await;

        let error = f.projects.create("TLS", "  ", &f.actor).await.unwrap_err();

        assert_eq!(error.code(), Some("project-name-empty"));
    }

    #[tokio::test]
    async fn benennt_um_und_meldet_unbekannte_projekte() {
        let f = BoardFixture::create().await;

        assert_eq!(
            f.projects.rename("arc", "Arches Shop").await.unwrap().name,
            "Arches Shop"
        );
        assert_eq!(
            f.projects.rename("XYZ", "n").await.unwrap_err().code(),
            Some("project-not-found")
        );
    }

    #[tokio::test]
    async fn lehnt_zu_lange_projektnamen_ab() {
        let f = BoardFixture::create().await;
        let zu_lang = "n".repeat(201);

        assert_eq!(
            f.projects
                .create("INC", &zu_lang, &f.actor)
                .await
                .unwrap_err()
                .code(),
            Some("value-too-long")
        );
        assert_eq!(
            f.projects.rename("ARC", &zu_lang).await.unwrap_err().code(),
            Some("value-too-long")
        );
    }
}
