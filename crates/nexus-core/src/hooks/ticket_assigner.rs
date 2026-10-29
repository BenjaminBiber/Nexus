//! Ordnet eine Session Tickets des DB-Boards zu: HARNESS_TICKET, verknüpfte Session, sonst
//! In-Arbeit-Tickets im Worktree. Portiert aus `packages/core/src/hooks/TicketAssigner.ts`.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use crate::board::basics::TicketStatus;
use crate::board::database::{BoardDatabase, TicketFilter};
use crate::board::ticket_service::TicketService;
use crate::error::Result;
use crate::tx;

/// Eingabe der Zuordnung
#[derive(Debug, Clone)]
pub struct AssignInput {
    pub cwd: String,
    pub session_id: String,
    pub env: HashMap<String, String>,
    /// Board des Repos (`RepoBoardResolver`); `None` = kein Board zugeordnet
    pub board_id: Option<i32>,
}

/// Ermittelt die Tickets einer Session
#[derive(Debug, Clone)]
pub struct TicketAssigner<D: BoardDatabase> {
    db: Arc<D>,
    tickets: TicketService<D>,
    /// Plattform für die Pfadnormalisierung (`process.platform` in TypeScript)
    platform: &'static str,
}

impl<D: BoardDatabase> TicketAssigner<D> {
    /// Zuordnung über dieser Datenbank auf der Plattform, auf der sie gerade läuft
    pub fn new(db: Arc<D>, tickets: TicketService<D>) -> Self {
        Self::with_platform(db, tickets, if cfg!(windows) { "win32" } else { "linux" })
    }

    /// Dieselbe Zuordnung mit vorgegebener Plattform (für Tests der Pfadnormalisierung)
    pub fn with_platform(db: Arc<D>, tickets: TicketService<D>, platform: &'static str) -> Self {
        Self {
            db,
            tickets,
            platform,
        }
    }

    /// Liefert die Ticket-IDs (sortiert, ohne Doppelungen); leer = „Nicht zugeordnet“
    pub async fn resolve(&self, input: &AssignInput) -> Result<Vec<String>> {
        let from_env: Vec<String> = input
            .env
            .get("HARNESS_TICKET")
            .map(String::as_str)
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .collect();
        if !from_env.is_empty() {
            let known: Vec<String> = self
                .tickets
                .summaries(&from_env)
                .await?
                .into_iter()
                .map(|ticket| ticket.id)
                .collect();
            if !known.is_empty() {
                return Ok(unique(known));
            }
        }
        let linked = self.tickets.tickets_of_session(&input.session_id).await?;
        if !linked.is_empty() {
            return Ok(unique(linked));
        }
        let Some(board_id) = input.board_id else {
            return Ok(Vec::new());
        };
        let cwd = self.normalize(&input.cwd);
        let filter = TicketFilter {
            board_ids: Some(vec![board_id]),
            ids: None,
            without_texts: false,
        };
        let tickets = tx!(self.db, [filter], |u| { u.tickets().list(&filter).await }).await?;
        Ok(unique(
            tickets
                .into_iter()
                .filter(|ticket| {
                    ticket.status == TicketStatus::InProgress
                        && ticket
                            .worktree
                            .as_deref()
                            .is_some_and(|worktree| contains(&self.normalize(worktree), &cwd))
                })
                .map(|ticket| ticket.id)
                .collect(),
        ))
    }

    /// Schrägstriche, ohne Endstrich, unter Windows ohne Beachtung der Schreibweise
    fn normalize(&self, path: &str) -> String {
        let cleaned = path.replace('\\', "/");
        let cleaned = cleaned.trim_end_matches('/');
        if self.platform == "win32" {
            cleaned.to_lowercase()
        } else {
            cleaned.to_string()
        }
    }
}

/// Ist `path` der Worktree selbst oder liegt er darin (ganze Pfadteile)?
fn contains(worktree: &str, path: &str) -> bool {
    path == worktree || path.starts_with(&format!("{worktree}/"))
}

/// Sortiert und entfernt Doppelungen
fn unique(ids: Vec<String>) -> Vec<String> {
    ids.into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::board::in_memory::InMemoryBoardDatabase;
    use crate::board::ticket_service::{NewTicketInput, TicketPatch, WriteOptions};
    use crate::board::MoveOptions;
    use crate::testing::BoardFixture;

    use super::*;

    /// Eingabe mit den Standardwerten des TypeScript-Tests
    fn input(
        cwd: &str,
        session_id: &str,
        env: &[(&str, &str)],
        board_id: Option<i32>,
    ) -> AssignInput {
        AssignInput {
            cwd: cwd.to_string(),
            session_id: session_id.to_string(),
            env: env
                .iter()
                .map(|(name, wert)| (name.to_string(), wert.to_string()))
                .collect(),
            board_id,
        }
    }

    /// Board mit drei Tickets, zwei davon in Arbeit im Worktree von ARC-0001
    async fn fixture() -> (BoardFixture, TicketAssigner<InMemoryBoardDatabase>) {
        let f = BoardFixture::create().await;
        for title in ["A", "B", "C"] {
            f.tickets
                .create(
                    f.board_id,
                    &NewTicketInput::titled(title),
                    &f.actor,
                    &WriteOptions::default(),
                )
                .await
                .unwrap();
        }
        for id in ["ARC-0001", "ARC-0003"] {
            f.tickets
                .update(
                    id,
                    &TicketPatch {
                        status: Some(TicketStatus::InProgress),
                        worktree: Some(Some("/w/repo-worktrees/ARC-0001".into())),
                        ..TicketPatch::default()
                    },
                    &f.actor,
                    &WriteOptions::default(),
                )
                .await
                .unwrap();
        }
        let assigner = TicketAssigner::with_platform(
            f.db.clone(),
            TicketService::new(f.db.clone(), f.clock.clone()),
            "linux",
        );
        (f, assigner)
    }

    #[tokio::test]
    async fn nimmt_harness_ticket_vor_allem_anderen_und_ubergeht_unbekannte_ids() {
        let (f, assigner) = fixture().await;
        assert_eq!(
            assigner
                .resolve(&input(
                    "/w/repo-worktrees/ARC-0001",
                    "s-1",
                    &[("HARNESS_TICKET", "arc-2, ARC-0099,kaputt")],
                    Some(f.board_id)
                ))
                .await
                .unwrap(),
            ["ARC-0002"]
        );
    }

    #[tokio::test]
    async fn nimmt_tickets_mit_denen_die_session_verknupft_ist() {
        let (f, assigner) = fixture().await;
        f.tickets.link_session("ARC-0002", "s-9").await.unwrap();
        assert_eq!(
            assigner
                .resolve(&input(
                    "/w/repo-worktrees/ARC-0001",
                    "s-9",
                    &[],
                    Some(f.board_id)
                ))
                .await
                .unwrap(),
            ["ARC-0002"]
        );
    }

    #[tokio::test]
    async fn ordnet_uber_den_worktree_zu_alle_laufenden_keine_erledigten() {
        let (f, assigner) = fixture().await;
        assert_eq!(
            assigner
                .resolve(&input(
                    "/w/repo-worktrees/ARC-0001/src/x",
                    "s-1",
                    &[],
                    Some(f.board_id)
                ))
                .await
                .unwrap(),
            ["ARC-0001", "ARC-0003"]
        );
        f.tickets
            .move_to(
                "ARC-0003",
                TicketStatus::Done,
                &f.actor,
                &MoveOptions {
                    acceptance: Some("ok".into()),
                    ..MoveOptions::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(
            assigner
                .resolve(&input(
                    "\\w\\repo-worktrees\\ARC-0001\\",
                    "s-1",
                    &[],
                    Some(f.board_id)
                ))
                .await
                .unwrap(),
            ["ARC-0001"]
        );
    }

    #[tokio::test]
    async fn verwechselt_ahnlich_benannte_ordner_nicht_und_ordnet_ohne_board_nichts_zu() {
        let (f, assigner) = fixture().await;
        for (cwd, board_id) in [
            ("/w/repo-worktrees/ARC-00011", Some(f.board_id)),
            ("/w/repo", Some(f.board_id)),
            ("/w/repo-worktrees/ARC-0001", None),
        ] {
            assert!(assigner
                .resolve(&input(cwd, "s-1", &[], board_id))
                .await
                .unwrap()
                .is_empty());
        }
    }
}
