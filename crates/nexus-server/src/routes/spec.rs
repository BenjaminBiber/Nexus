//! Routengruppe `docs`, Teil Spec-Fortschritt: `GET …/spec-progress`; portiert aus
//! `packages/server/src/routes/SpecRoutes.ts`.
//!
//! Ohne `board.yaml`, ohne Board zum Repo oder bei einem Fachfehler bleibt die Liste leer –
//! die Docs-Seite zeigt dann keinen Fortschritt.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::repo_board_resolver::RepoBoardOwner;
use nexus_core::board::spec_progress::SpecProgressCount;
use nexus_core::Result;

use crate::error::{json, ApiResult};
use crate::proxy::proxy_or_static;
use crate::state::AppState;

/// Meldet die Route des Spec-Fortschritts an
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/projects/{pid}/spec-progress",
        get(spec_progress).fallback(proxy_or_static),
    )
}

/// Stand der Specs aus dem Board
async fn spec_progress(
    State(state): State<Arc<AppState>>,
    UrlPath(pid): UrlPath<String>,
) -> ApiResult<Response> {
    let project = state.projects.open(&pid)?;
    Ok(json(&board_progress(&state, &project.project.root).await?))
}

/// Board des Repos (eigener Repo-Pfad) → dessen DB-Projekt → Zählung; ohne Board-Konfig,
/// Board oder Verbindung leer
async fn board_progress(state: &Arc<AppState>, root: &str) -> Result<Vec<SpecProgressCount>> {
    let Ok(Some(config)) = BoardConfigStore::new(state.home.clone()).read() else {
        return Ok(Vec::new());
    };
    let owner = RepoBoardOwner {
        user_id: config.user_id,
        active_board: config.active_board,
    };
    let root = root.to_string();
    let counted = state
        .board
        .run(move |b| async move {
            let board = b.services().boards.for_repo(&root, &owner).await?;
            match board {
                Some(board) => b.services().graph.for_project(board.board.project_id).await,
                None => Ok(Vec::new()),
            }
        })
        .await;
    match counted {
        Ok(counted) => Ok(counted),
        // Die Docs zeigen dann keinen Fortschritt
        Err(error) if error.harness().is_some() => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}
