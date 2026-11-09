//! Der Router des Servers: Schutz, Routen und der Rückfall auf die Oberfläche.
//!
//! Reihenfolge: Kennzeichnung → Panik-Fang → Schutz → Routen → Rückfall. Der Rückfall
//! beantwortet unbekannte `/api`-Pfade mit 404 als JSON und liefert sonst die Oberfläche.

pub mod accounts;
pub mod db_boards;
pub mod db_misc;
pub mod db_tickets;
pub mod dispatch;
pub mod dispatch_suggestions;
pub mod doc_comments;
pub mod docs;
pub mod docs_claude;
pub mod events;
pub mod favorites;
pub mod health;
pub mod hook;
pub mod notifications;
pub mod questions;
pub mod session_detail;
pub mod sessions;
pub mod settings;
pub mod setup;
pub mod snippets;
pub mod spec;
pub mod speech;
pub mod static_files;
pub mod store_index;
pub mod store_install;
pub mod store_repos;
pub mod telegram;
pub mod theme_fetch;
pub mod theme_packages;
pub mod themes;
pub mod usage;
pub mod workflow;

use std::sync::Arc;

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use tower_http::catch_panic::CatchPanicLayer;

use crate::guard::guard_middleware;
use crate::state::AppState;

/// Kopf, der sagt, wer geantwortet hat; seit dem Wegfall des Node-Servers immer `rust`
pub const BACKEND_HEADER: &str = "x-nexus-backend";
/// Wert des Kopfes
pub const BACKEND_RUST: &str = "rust";

/// Baut den Router zum Zustand.
///
/// **Neue Routengruppen hier eintragen.**
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(health::routes())
        .merge(settings::routes())
        .merge(setup::routes())
        .merge(db_boards::routes())
        .merge(db_tickets::routes())
        .merge(db_misc::routes())
        .merge(questions::routes())
        .merge(events::routes())
        .merge(docs::routes())
        .merge(doc_comments::routes())
        .merge(spec::routes())
        .merge(docs_claude::routes())
        .merge(hook::routes())
        .merge(sessions::routes())
        .merge(session_detail::routes())
        .merge(favorites::routes())
        .merge(snippets::routes())
        .merge(store_index::routes())
        .merge(store_install::routes())
        .merge(store_repos::routes())
        .merge(usage::routes())
        .merge(accounts::routes())
        .merge(notifications::routes())
        .merge(workflow::routes())
        .merge(dispatch::routes())
        .merge(dispatch_suggestions::routes())
        .merge(speech::routes())
        .merge(telegram::routes())
        .merge(themes::routes())
        .merge(theme_packages::routes())
        .merge(theme_fetch::routes())
        .fallback(static_or_not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            guard_middleware,
        ))
        .layer(CatchPanicLayer::custom(crate::error::panic_response))
        .layer(middleware::from_fn(mark_backend))
        .with_state(state)
}

/// Rückfall des Routers: unbekannte `/api`-Pfade sind 404 als JSON, alles andere geht an die
/// statische Auslieferung der Oberfläche
pub async fn static_or_not_found(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    request: Request,
) -> Response {
    if request.uri().path().starts_with("/api") {
        return crate::error::not_found_response();
    }
    static_files::serve(state, request).await
}

/// Setzt `x-nexus-backend: rust` auf jede Antwort. Damit ist in Protokollen und beim Prüfen
/// sichtbar, wer geantwortet hat.
async fn mark_backend(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    if !response.headers().contains_key(BACKEND_HEADER) {
        response.headers_mut().insert(
            HeaderName::from_static(BACKEND_HEADER),
            HeaderValue::from_static(BACKEND_RUST),
        );
    }
    response
}
