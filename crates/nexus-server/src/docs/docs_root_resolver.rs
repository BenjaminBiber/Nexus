//! Findet zum Ordner eines Projekts das Board und damit den Repo-Pfad, aus dem die Docs
//! gelesen werden; portiert aus `packages/server/src/docs/DocsRootResolver.ts`.

use std::path::Path;

use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::board::repo_paths::BoardWithRepo;
use nexus_core::{HarnessHome, Result};
use serde::{Deserialize, Serialize};

use crate::board_runtime::BoardRuntimeHolder;
use crate::state::AppState;

/// Warum es keine Docs-Quelle gibt: kein Board zum Ordner, Board ohne Repo-Pfad, Pfad zeigt
/// ins Leere
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DocsSourceReason {
    NoBoard,
    NoRepoPath,
    MissingFolder,
}

/// Woher die Dokumente kommen; ohne Quelle sagt `reason`, was fehlt – die Docs-Seite zeigt
/// dann einen Hinweis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocsSource {
    /// Absoluter Repo-Pfad des Boards mit `/` als Trenner, sonst `None`
    pub root: Option<String>,
    pub board_id: Option<i32>,
    pub board_name: Option<String>,
    /// nur gesetzt, wenn `root` leer ist
    pub reason: Option<DocsSourceReason>,
}

impl DocsSource {
    /// Es gibt kein Board zu diesem Ordner
    pub fn no_board() -> Self {
        Self {
            root: None,
            board_id: None,
            board_name: None,
            reason: Some(DocsSourceReason::NoBoard),
        }
    }
}

/// Findet zum Ordner eines Projekts das Board und damit den Repo-Pfad der Dokumente.
pub struct DocsRootResolver<'a> {
    holder: Option<&'a BoardRuntimeHolder>,
    home: Option<&'a HarnessHome>,
}

impl<'a> DocsRootResolver<'a> {
    /// Auflöser über einer Board-Laufzeit und einem Benutzerordner; ohne beides gibt es keine
    /// Quelle
    pub fn new(holder: Option<&'a BoardRuntimeHolder>, home: Option<&'a HarnessHome>) -> Self {
        Self { holder, home }
    }

    /// Der Auflöser zum Zustand des Servers
    pub fn for_state(state: &'a AppState) -> Self {
        Self::new(Some(&state.board), Some(&state.home))
    }

    /// Nie „werfend“ wegen eines Fachfehlers: ohne Board-Laufzeit, ohne `board.yaml` oder bei
    /// Verbindungsfehler gibt es eben keine Quelle
    pub async fn resolve(&self, project_root: &str) -> Result<DocsSource> {
        let (Some(holder), Some(home)) = (self.holder, self.home) else {
            return Ok(DocsSource::no_board());
        };
        // Eine fehlende oder kaputte Konfig heißt „kein Board“, nicht „Fehler“
        let Ok(Some(config)) = BoardConfigStore::new(home.clone()).read() else {
            return Ok(DocsSource::no_board());
        };
        let owner = RepoBoardOwner {
            user_id: config.user_id.clone(),
            active_board: config.active_board,
        };
        let root = project_root.to_string();
        let found = holder
            .run(move |b| async move {
                let found = b.services().boards.for_repo(&root, &owner).await?;
                if found.is_some() || owner.active_board.is_none() {
                    return Ok(found);
                }
                // Ohne Treffer erklärt nur ein aktives Board ohne Repo-Pfad die fehlende
                // Quelle; zeigt es auf ein anderes Repo, gehört es nicht zu diesem Ordner und
                // die Docs bleiben leer.
                let active = b
                    .services()
                    .boards
                    .find(owner.active_board.unwrap_or_default())
                    .await?;
                Ok(active.filter(|board| blank(board.board.repo_path.as_deref())))
            })
            .await;
        match found {
            Ok(board) => Ok(source_of(board)),
            Err(error) if error.harness().is_some() => Ok(DocsSource::no_board()),
            Err(error) => Err(error),
        }
    }

    /// Quelle einer Anfrage: mit `board` aus der Query (Board-Umschalter der Docs-Seite)
    /// dieses Board, sonst das Board des Projektordners
    pub async fn resolve_for(&self, project_root: &str, board: Option<&str>) -> Result<DocsSource> {
        let id = board
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0);
        if id > 0 {
            self.for_board(id).await
        } else {
            self.resolve(project_root).await
        }
    }

    /// Quelle eines bestimmten Boards (Board-Umschalter der Docs-Seite); unbekanntes Board
    /// oder fehlende Laufzeit heißt „kein Board“
    pub async fn for_board(&self, board_id: i32) -> Result<DocsSource> {
        let Some(holder) = self.holder else {
            return Ok(DocsSource::no_board());
        };
        match holder
            .run(|b| async move { b.services().boards.find(board_id).await })
            .await
        {
            Ok(board) => Ok(source_of(board)),
            Err(error) if error.harness().is_some() => Ok(DocsSource::no_board()),
            Err(error) => Err(error),
        }
    }
}

/// Quelle eines Boards: sein Repo-Pfad, wenn er auf einen Ordner zeigt, sonst der Grund,
/// warum es keine gibt
fn source_of(board: Option<BoardWithRepo>) -> DocsSource {
    let Some(board) = board else {
        return DocsSource::no_board();
    };
    let path = board
        .board
        .repo_path
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let id = Some(board.board.id);
    let name = Some(board.board.name.clone());
    if path.is_empty() {
        return DocsSource {
            root: None,
            board_id: id,
            board_name: name,
            reason: Some(DocsSourceReason::NoRepoPath),
        };
    }
    if !Path::new(&path).is_dir() {
        return DocsSource {
            root: None,
            board_id: id,
            board_name: name,
            reason: Some(DocsSourceReason::MissingFolder),
        };
    }
    DocsSource {
        root: Some(path.replace('\\', "/")),
        board_id: id,
        board_name: name,
        reason: None,
    }
}

/// Fehlt der Repo-Pfad oder besteht er nur aus Leerzeichen?
fn blank(path: Option<&str>) -> bool {
    path.map(str::trim).unwrap_or_default().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schreibt_den_grund_wie_die_api_ihn_ausgibt() {
        let json = serde_json::to_value(DocsSource::no_board()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "root": null, "boardId": null, "boardName": null, "reason": "no-board",
            })
        );
        assert_eq!(
            serde_json::to_value(DocsSourceReason::NoRepoPath).unwrap(),
            "no-repo-path"
        );
        assert_eq!(
            serde_json::to_value(DocsSourceReason::MissingFolder).unwrap(),
            "missing-folder"
        );
    }
}
