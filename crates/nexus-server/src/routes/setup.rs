//! Routengruppe `setup`: Zustand und Installation des Claude-Code-Plugins, Erkennung und
//! Migration von Repos, die noch für den alten AI Harness eingerichtet sind. Die Logik steht in
//! [`crate::setup`].

use std::path::Path;
use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use nexus_core::install::PluginBundle;
use nexus_core::{Error, HarnessError};

use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::input::DbInput;
use crate::routes::db_boards::board_repo;
use crate::routes::static_or_not_found;
use crate::setup::legacy_check::check_repo;
use crate::setup::migration::migrate_repo;
use crate::setup::plugin_install::install_plugin;
use crate::setup::plugin_status::read_status;
use crate::state::AppState;

/// Meldet die beiden Routen der Gruppe an
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/setup/plugin",
            get(plugin_status)
                .post(plugin_install)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/db/boards/{id}/migration",
            get(migration_check)
                .post(migration_run)
                .fallback(static_or_not_found),
        )
}

/// Das mitgelieferte Plugin, wenn die App eines mitbringt
fn bundle(state: &AppState) -> Option<PluginBundle> {
    state
        .config
        .plugin_bundle
        .as_deref()
        .and_then(PluginBundle::open)
}

/// Zustand des Plugins; `claude --version` läuft beim ersten Aufruf, deshalb blockierend
async fn plugin_status(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let setup = state.config.setup.clone();
    let bundle = bundle(&state);
    let report = tokio::task::spawn_blocking(move || {
        read_status(&setup.claude_dir(), bundle.as_ref(), setup.claude_found())
    })
    .await
    .map_err(Error::unexpected)?;
    Ok(json(&report))
}

/// Installiert das mitgelieferte Plugin; ohne Bundle oder bei laufendem Lauf 409
async fn plugin_install(
    State(state): State<Arc<AppState>>,
    JsonBody(_): JsonBody,
) -> ApiResult<Response> {
    let bundle = bundle(&state).ok_or_else(|| {
        HarnessError::new(
            "plugin-not-bundled",
            "Diese Nexus-Installation bringt kein Plugin mit.",
        )
    })?;
    let guard = state.config.setup.try_begin()?;
    let setup = state.config.setup.clone();
    let home = state.home.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _guard = guard;
        install_plugin(&setup, &bundle, &home)
    })
    .await
    .map_err(Error::unexpected)??;
    Ok(json(&result))
}

/// Braucht das Repo des Boards eine Migration?
async fn migration_check(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Board")?;
    let repo = board_repo(&state, id).await?;
    Ok(json(&check_repo(repo.as_deref())))
}

/// Stellt das Repo des Boards um (wie `nexus init`); Board ohne Repo 400
async fn migration_run(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    JsonBody(_): JsonBody,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Board")?;
    let repo = board_repo(&state, id).await?.ok_or_else(|| {
        HarnessError::new(
            "bad-request",
            format!("Board {id} hat auf diesem Rechner kein Repo."),
        )
    })?;
    let guard = state.config.setup.try_begin()?;
    let approval = state.config.setup.approval();
    let home = state.home.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _guard = guard;
        migrate_repo(&home, &approval, Path::new(&repo))
    })
    .await
    .map_err(Error::unexpected)??;
    Ok(json(&result))
}
