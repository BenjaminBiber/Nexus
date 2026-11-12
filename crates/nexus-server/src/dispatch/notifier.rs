//! Meldungen des Dispatchers; portiert aus
//! `packages/server/src/dispatch/DispatchNotifier.ts`.
//!
//! Hier steht nur der Trait samt Leer-Umsetzung: die Anbindung an die Benachrichtigungen des
//! Servers (Windows-Meldungen) folgt in P4-16a. Fehler einer Meldung halten einen Durchlauf
//! nie auf – [`crate::dispatch::DispatchContext::notify`] protokolliert sie und macht weiter.

use async_trait::async_trait;
use nexus_core::Result;
use serde::{Deserialize, Serialize};

/// Anlass einer Meldung
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DispatchNoticeKind {
    Merged,
    Blocked,
    Question,
    Paused,
    Review,
    /// Ein Autopilot-Ticket steckt in einer Sackgasse fest (statt `Blocked`)
    Stuck,
}

impl DispatchNoticeKind {
    /// Der Anlass, wie er im JSON und in der Kennung einer Meldung steht
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merged => "merged",
            Self::Blocked => "blocked",
            Self::Question => "question",
            Self::Paused => "paused",
            Self::Review => "review",
            Self::Stuck => "stuck",
        }
    }
}

/// Eine Meldung des Dispatchers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchNotice {
    pub kind: DispatchNoticeKind,
    pub ticket_id: Option<String>,
    pub title: String,
    pub text: String,
    /// Repo-Pfad des Boards (für die Adresse in der Meldung)
    pub repo_path: Option<String>,
}

impl DispatchNotice {
    /// Meldung zu einem blockierten Ticket: im Autopilot „Autopilot steckt fest: <ID> – <Grund>“
    /// (`Stuck`), sonst der Grund unverändert (`Blocked`)
    pub fn blocked(
        autopilot: bool,
        ticket_id: &str,
        title: &str,
        reason: &str,
        repo_path: Option<String>,
    ) -> Self {
        let (kind, text) = match autopilot {
            true => (
                DispatchNoticeKind::Stuck,
                format!("Autopilot steckt fest: {ticket_id} – {reason}"),
            ),
            false => (DispatchNoticeKind::Blocked, reason.to_string()),
        };
        Self {
            kind,
            ticket_id: Some(ticket_id.to_string()),
            title: title.to_string(),
            text,
            repo_path,
        }
    }
}

/// Nimmt Meldungen des Dispatchers an; Tests merken sie sich, der Betrieb schickt sie weiter.
#[async_trait]
pub trait DispatchNotify: std::fmt::Debug + Send + Sync {
    /// Meldet; der Aufrufer wirft den Fehler nicht weiter
    async fn notify(&self, notice: &DispatchNotice) -> Result<()>;
}

/// Verwirft jede Meldung (Standard, bis P4-16a die Benachrichtigungen anbindet).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDispatchNotify;

#[async_trait]
impl DispatchNotify for NoDispatchNotify {
    async fn notify(&self, _notice: &DispatchNotice) -> Result<()> {
        Ok(())
    }
}

/// Leitet Meldungen des Dispatchers an die Benachrichtigungen des Servers weiter; die Adresse
/// zeigt ins Projekt des Board-Repos. Portiert aus
/// `packages/server/src/dispatch/DispatchNotifier.ts`.
pub struct ServerDispatchNotify {
    projects: crate::projects::ProjectService,
    notifications: std::sync::Arc<crate::notifications::NotificationService>,
}

impl std::fmt::Debug for ServerDispatchNotify {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerDispatchNotify")
            .finish_non_exhaustive()
    }
}

impl ServerDispatchNotify {
    /// Melder auf Projekten und Benachrichtigungen des Servers
    pub fn new(
        projects: crate::projects::ProjectService,
        notifications: std::sync::Arc<crate::notifications::NotificationService>,
    ) -> Self {
        Self {
            projects,
            notifications,
        }
    }
}

/// Pfadvergleich wie in TypeScript: ohne Schlusstrenner, mit `/` und in Kleinschreibung
fn normalized(path: &str) -> String {
    path.trim_end_matches(['\\', '/'])
        .replace('\\', "/")
        .to_lowercase()
}

#[async_trait]
impl DispatchNotify for ServerDispatchNotify {
    async fn notify(&self, notice: &DispatchNotice) -> Result<()> {
        let reachable: Vec<_> = self
            .projects
            .list()?
            .into_iter()
            .filter(|state| state.reachable)
            .collect();
        let wanted = notice.repo_path.as_deref().map(normalized);
        // Projekt des Board-Repos, sonst das erste erreichbare
        let project = reachable
            .iter()
            .find(|state| {
                wanted
                    .as_deref()
                    .is_some_and(|wanted| normalized(&state.project.path) == wanted)
            })
            .or(reachable.first());
        let project_id = project
            .map(|state| state.project.id.as_str())
            .unwrap_or_default();
        self.notifications.on_dispatch(project_id, notice).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stuck_heisst_im_json_und_in_der_kennung_stuck() {
        assert_eq!(DispatchNoticeKind::Stuck.as_str(), "stuck");
        assert_eq!(
            serde_json::to_string(&DispatchNoticeKind::Stuck).expect("serialisieren"),
            "\"stuck\""
        );
    }

    #[test]
    fn blockiert_meldet_im_autopilot_stuck_sonst_blocked() {
        let stuck = DispatchNotice::blocked(true, "AIH-1", "Titel", "Zugang fehlt", None);
        assert_eq!(stuck.kind, DispatchNoticeKind::Stuck);
        assert_eq!(stuck.text, "Autopilot steckt fest: AIH-1 – Zugang fehlt");
        assert_eq!(stuck.ticket_id.as_deref(), Some("AIH-1"));

        let blocked = DispatchNotice::blocked(false, "AIH-1", "Titel", "Zugang fehlt", None);
        assert_eq!(blocked.kind, DispatchNoticeKind::Blocked);
        assert_eq!(blocked.text, "Zugang fehlt");
    }
}
