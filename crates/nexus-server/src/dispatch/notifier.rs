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
