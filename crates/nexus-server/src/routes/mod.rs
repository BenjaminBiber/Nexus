//! Der Router des Servers: Schutz, Routengruppen und der Rückfall zum Node-Server.
//!
//! Reihenfolge: Kennzeichnung → Panik-Fang → Schutz → registrierte Rust-Routen → Rückfall.
//! Der Rückfall leitet `/api`-Pfade an den Node-Server weiter (falls einer eingerichtet ist)
//! und liefert sonst die Oberfläche bzw. 404 als JSON.

pub mod db_boards;
pub mod health;
pub mod settings;
pub mod static_files;
pub mod workflow;

use std::sync::Arc;

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use tower_http::catch_panic::CatchPanicLayer;

use crate::groups::RouteGroup;
use crate::guard::guard_middleware;
use crate::proxy::{proxy_or_static, BACKEND_HEADER, BACKEND_RUST};
use crate::state::AppState;

/// Baut den Router zum Zustand. Jede Routengruppe wird nur registriert, wenn sie aktiv ist;
/// steht ihr Name in `NEXUS_PROXY_GROUPS`, beantwortet sie weiter der Node-Server.
///
/// **Neue Routengruppen hier eintragen** – dazu in [`RouteGroup`].
pub fn build_router(state: Arc<AppState>) -> Router {
    let mut router = Router::new();
    if state.config.proxy_groups.enabled(RouteGroup::Basis) {
        router = router.merge(health::routes());
    }
    if state.config.proxy_groups.enabled(RouteGroup::Settings) {
        router = router.merge(settings::routes());
    }
    if state.config.proxy_groups.enabled(RouteGroup::DbBoards) {
        router = router.merge(db_boards::routes());
    }
    if state.config.proxy_groups.enabled(RouteGroup::Workflow) {
        router = router.merge(workflow::routes());
    }
    router
        .fallback(proxy_or_static)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            guard_middleware,
        ))
        .layer(CatchPanicLayer::custom(crate::error::panic_response))
        .layer(middleware::from_fn(mark_backend))
        .with_state(state)
}

/// Setzt `x-nexus-backend: rust` auf jede Antwort, die nicht vom Node-Server kommt (der
/// Proxy trägt dort `node` ein). Damit ist in Protokollen und beim Prüfen sichtbar, wer
/// geantwortet hat.
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
