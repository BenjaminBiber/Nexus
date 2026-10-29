//! Routengruppe `db-boards`: Projekte, Boards, aktives Board, Board-Ansicht, Zielbranches und
//! die Ordnerwahl; portiert aus `packages/server/src/routes/DbBoardRoutes.ts`.
//!
//! Das aktive Board steht in `board.yaml` und wird mit CLI und Agenten geteilt; alle übrigen
//! Daten kommen aus der Board-Datenbank über [`crate::board_runtime::BoardRuntimeHolder`].

use std::path::Path;
use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use nexus_core::board::board_service::{BoardListFilter, BoardSummary};
use nexus_core::board::config::BoardConfig;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::dispatch::git::target_branches::TargetBranches;
use nexus_core::git::{BranchNames, TargetBranchRule};
use nexus_core::{HarnessError, Result};
use serde::Serialize;

use crate::board_runtime::BoardRuntimeHolder;
use crate::error::{json, ApiResult};
use crate::extract::{JsonBody, QueryMap};
use crate::input::{DbInput, Nullable};
use crate::proxy::proxy_or_static;
use crate::state::AppState;

/// Benutzer aus `board.yaml` und sein aktives Board
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbActiveDto {
    pub user: DbActiveUser,
    pub board_id: Option<i32>,
    pub board: Option<BoardSummary>,
}

/// Der eigene Benutzer, wie `board.yaml` ihn nennt
#[derive(Debug, Clone, Serialize)]
pub struct DbActiveUser {
    pub id: String,
    pub name: String,
}

/// Meldet die 13 Routen der Gruppe an. Der Rückfall der Methoden gehört zu jeder Route: hat
/// eine Rust-Route den Pfad, aber nicht die Methode, darf axum kein 405 liefern.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/db/projects",
            get(list_projects)
                .post(create_project)
                .fallback(proxy_or_static),
        )
        // Nur der genaue Pfad: `/api/db/projects/{key}/questions` gehört zu einer anderen
        // Gruppe und muss weiter in den Rückfall laufen
        .route(
            "/api/db/projects/{key}",
            axum::routing::patch(rename_project).fallback(proxy_or_static),
        )
        .route(
            "/api/db/boards",
            get(list_boards)
                .post(create_board)
                .fallback(proxy_or_static),
        )
        .route(
            "/api/db/boards/{id}",
            axum::routing::patch(update_board)
                .delete(delete_board)
                .fallback(proxy_or_static),
        )
        .route(
            "/api/db/pick-folder",
            post(pick_folder).fallback(proxy_or_static),
        )
        .route(
            "/api/db/boards/{id}/activate",
            post(activate_board).fallback(proxy_or_static),
        )
        .route("/api/db/active", get(active).fallback(proxy_or_static))
        .route(
            "/api/db/boards/{id}/view",
            get(board_view).fallback(proxy_or_static),
        )
        .route(
            "/api/db/boards/{id}/branches",
            get(list_branches)
                .post(create_branch)
                .fallback(proxy_or_static),
        )
}

/// Alle Projekte des Boards
async fn list_projects(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let projects = state
        .board
        .run(|b| async move { b.services().projects.list().await })
        .await?;
    Ok(json(&projects))
}

/// Legt ein Projekt an (201)
async fn create_project(
    State(state): State<Arc<AppState>>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let key = DbInput::text(&body, "key")?;
    let name = DbInput::text(&body, "name")?;
    let project = state
        .board
        .run(|b| async move {
            b.services()
                .projects
                .create(&key, &name, &b.actor(None))
                .await
        })
        .await?;
    Ok((StatusCode::CREATED, json(&project)).into_response())
}

/// Benennt ein Projekt um
async fn rename_project(
    State(state): State<Arc<AppState>>,
    UrlPath(key): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let name = DbInput::text(&DbInput::body(&raw), "name")?;
    let project = state
        .board
        .run(|b| async move { b.services().projects.rename(&key, &name).await })
        .await?;
    Ok(json(&project))
}

/// Die Boards, auf Wunsch nur die eines Projekts
async fn list_boards(State(state): State<Arc<AppState>>, query: QueryMap) -> ApiResult<Response> {
    let filter = BoardListFilter {
        project_key: query.first("project").map(str::to_string),
        ..BoardListFilter::default()
    };
    Ok(json(&boards(&state, &filter).await?))
}

/// Legt ein Board an und liefert es als Zusammenfassung (201)
async fn create_board(
    State(state): State<Arc<AppState>>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let body = DbInput::body(&raw);
    let project = DbInput::text(&body, "project")?;
    let name = DbInput::text(&body, "name")?;
    let summary = state
        .board
        .run(|b| async move {
            let board = b
                .services()
                .boards
                .create(&project, &name, &b.actor(None))
                .await?;
            let list = b
                .services()
                .boards
                .list(&BoardListFilter::default())
                .await?;
            summary(list, board.board.id)
        })
        .await?;
    Ok((StatusCode::CREATED, json(&summary)).into_response())
}

/// Ändert Name und eigenen Repo-Pfad eines Boards; ohne Angabe `nothing-changed`
async fn update_board(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Board")?;
    let body = DbInput::body(&raw);
    let name = DbInput::optional_text(&body, "name")?;
    let repo_path = DbInput::nullable_text(&body, "repoPath")?;
    if name.is_none() && repo_path == Nullable::Missing {
        return Err(HarnessError::new("nothing-changed", "Keine Änderung angegeben.").into());
    }
    let summary = state
        .board
        .run(|b| async move {
            if let Some(name) = &name {
                b.services().boards.rename(id, name).await?;
            }
            match &repo_path {
                Nullable::Missing => {}
                Nullable::Null => {
                    b.services().boards.set_repo_path(id, None).await?;
                }
                Nullable::Value(path) => {
                    b.services().boards.set_repo_path(id, Some(path)).await?;
                }
            }
            let list = b
                .services()
                .boards
                .list(&BoardListFilter::default())
                .await?;
            summary(list, id)
        })
        .await?;
    Ok(json(&summary))
}

/// Löscht ein Board; offene Tickets nur mit `deleteOpen=true`
async fn delete_board(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Board")?;
    let delete_open = query.first("deleteOpen") == Some("true");
    // board.yaml vorher prüfen: ein Konfigurationsfehler darf nicht erst nach dem Löschen als
    // 503 kommen
    let config = config(&state)?;
    let result = state
        .board
        .run(|b| async move {
            b.services()
                .boards
                .delete(id, delete_open, &b.actor(None))
                .await
        })
        .await?;
    if config.active_board == Some(id) {
        let cleared = BoardConfig {
            active_board: None,
            ..config
        };
        store(&state).write(&cleared)?;
    }
    Ok(json(&result))
}

/// Ordnerdialog auf dem Rechner des Servers für „Mein Repo-Pfad“; `path` ist `null`, wenn die
/// Person abbricht
async fn pick_folder(
    State(state): State<Arc<AppState>>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let start = DbInput::optional_text(&DbInput::body(&raw), "start")?;
    let start = start
        .as_deref()
        .map(str::trim)
        .filter(|start| !start.is_empty());
    let path = state.platform.folders.pick(start).await?;
    Ok(json(&serde_json::json!({ "path": path })))
}

/// Macht ein Board zum aktiven Board (in `board.yaml`)
async fn activate_board(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Board")?;
    let config = config(&state)?;
    // Nur prüfen, ob es das Board gibt (sonst board-not-found)
    boards(&state, &BoardListFilter::default())
        .await
        .and_then(|list| summary(list, id))?;
    store(&state).write(&BoardConfig {
        active_board: Some(id),
        ..config
    })?;
    Ok(json(&active_dto(&state).await?))
}

/// Benutzer und aktives Board
async fn active(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&active_dto(&state).await?))
}

/// Die Board-Ansicht mit allen Spalten
async fn board_view(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Board")?;
    let view = state
        .board
        .run(|b| async move { b.services().views.get(id).await })
        .await?;
    Ok(json(&view))
}

/// Vorschläge für den Zielbranch; ohne Repo auf diesem Rechner kein Fehler, sondern
/// „no-repo“ (das Feld bleibt dann reine Texteingabe)
async fn list_branches(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Board")?;
    let Some(root) = board_repo(&state, id).await? else {
        return Ok(json(
            &serde_json::json!({ "branches": [], "reason": "no-repo" }),
        ));
    };
    // Git ist ein blockierender Aufruf und gehört deshalb nicht in die Laufzeit der Anfrage
    let branches = tokio::task::spawn_blocking(move || BranchNames::new().list(Path::new(&root)))
        .await
        .map_err(nexus_core::Error::unexpected)?;
    Ok(json(
        &serde_json::json!({ "branches": branches, "reason": serde_json::Value::Null }),
    ))
}

/// Legt einen neuen Zielbranch im Board-Repo lokal an (von origin/<name>, sonst vom HEAD des
/// Hauptordners); kein Push
async fn create_branch(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Board")?;
    let wanted = DbInput::text(&DbInput::body(&raw), "name")?;
    let name = TargetBranchRule::new()
        .normalize(Some(&wanted))?
        .ok_or_else(|| HarnessError::new("target-branch-invalid", "Kein Branch-Name angegeben."))?;
    let root = board_repo(&state, id).await?.ok_or_else(|| {
        HarnessError::new(
            "board-repo-missing",
            format!("Board {id} hat auf diesem Rechner kein Repo."),
        )
    })?;
    let wanted = name.clone();
    let ensured = tokio::task::spawn_blocking(move || {
        TargetBranches::new().ensure(Path::new(&root), &wanted)
    })
    .await
    .map_err(nexus_core::Error::unexpected)??;
    let status = if ensured.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    let body = serde_json::json!({
        "name": name,
        "created": ensured.created,
        "from": ensured.from,
    });
    Ok((status, json(&body)).into_response())
}

/// Der Repo-Pfad eines Boards, wenn der Ordner auf diesem Rechner liegt; ein unbekanntes
/// Board ist `board-not-found`
async fn board_repo(state: &Arc<AppState>, id: i32) -> Result<Option<String>> {
    let board = state
        .board
        .run(|b| async move { b.services().boards.find(id).await })
        .await?
        .ok_or_else(|| {
            HarnessError::new("board-not-found", format!("Board {id} gibt es nicht."))
        })?;
    Ok(board
        .board
        .repo_path
        .filter(|path| Path::new(path).exists()))
}

/// Benutzer aus `board.yaml` und das aktive Board (ein gelöschtes aktives Board gilt als
/// keins)
async fn active_dto(state: &Arc<AppState>) -> Result<DbActiveDto> {
    let config = config(state)?;
    let list = boards(state, &BoardListFilter::default()).await?;
    let board = list
        .into_iter()
        .find(|summary| Some(summary.board.id) == config.active_board);
    Ok(DbActiveDto {
        user: DbActiveUser {
            id: config.user_id,
            name: config.user_name,
        },
        board_id: board.as_ref().map(|summary| summary.board.id),
        board,
    })
}

/// Die Boards der Datenbank zu diesem Filter
async fn boards(state: &Arc<AppState>, filter: &BoardListFilter) -> Result<Vec<BoardSummary>> {
    let filter = filter.clone();
    state
        .board
        .run(|b| async move { b.services().boards.list(&filter).await })
        .await
}

/// Board mit Projekt, Besitzer und Zahl offener Tickets oder `board-not-found`
fn summary(list: Vec<BoardSummary>, id: i32) -> Result<BoardSummary> {
    list.into_iter()
        .find(|summary| summary.board.id == id)
        .ok_or_else(|| {
            HarnessError::new("board-not-found", format!("Board {id} gibt es nicht.")).into()
        })
}

/// `board.yaml`; fehlend → `board-config-missing`, kaputt → `board-config-broken` (beides 503)
fn config(state: &Arc<AppState>) -> Result<BoardConfig> {
    store(state)
        .require()
        .map_err(BoardRuntimeHolder::translate)
}

/// Die Konfigdatei des Boards im Benutzerordner
fn store(state: &Arc<AppState>) -> BoardConfigStore {
    BoardConfigStore::new(state.home.clone())
}
