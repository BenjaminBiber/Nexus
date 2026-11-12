//! Routengruppe `dispatch`: Einstellung, Zustand, Steuerung, „Erneut mergen“, „Abnehmen“,
//! Review, „Zurück an den Agenten“, Recherche, Planung, Läufe und Protokolle. Portiert aus
//! `packages/server/src/routes/DispatchRoutes.ts`.
//!
//! Schreibende Routen antworten mit dem Zustand; `merge`, `accept` und `return` hängen
//! `result` an, `research` und `plan` hängen `started` an. Die Board-ID aus Abfrage oder
//! Körper wird vor jeder Aktion geprüft (Form und Existenz), damit eine ungültige Eingabe
//! nichts verändert.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use nexus_core::dispatch::{DispatchConfigNormalizer, PromptKind};
use serde::Serialize;
use serde_json::Value;

use crate::dispatch::prompt_catalog::{DispatchPromptInfo, PromptCatalog};
use crate::dispatch::service::{DispatchService, PromptPreviewInput};
use crate::error::{json, ApiError, ApiResult};
use crate::extract::{JsonBody, QueryMap};
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Standard-Pause in Minuten, wenn der Körper keine nennt
const DEFAULT_PAUSE_MINUTES: i64 = 60;

/// Meldet die Routen der Gruppe an. Der Rückfall je Pfad gehört dazu: hat eine Rust-Route den
/// Pfad, aber nicht die Methode, darf axum kein 405 liefern.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/dispatch/config",
            get(config).put(save_config).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/prompts/preview",
            post(preview).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/status",
            get(status).fallback(static_or_not_found),
        )
        .route("/api/dispatch/run", post(run).fallback(static_or_not_found))
        .route(
            "/api/dispatch/pause",
            post(pause).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/resume",
            post(resume).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/enable",
            post(enable).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/disable",
            post(disable).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/merge/{id}",
            post(retry_merge).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/accept/{id}",
            post(accept).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/review/{id}",
            get(review).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/review/{id}/file",
            get(review_file).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/return/{id}",
            post(return_to_agent).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/research/{question_id}",
            get(research_status)
                .post(start_research)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/plan/{ticket_id}",
            get(planning_status)
                .post(start_planning)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/runs",
            get(runs).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/runs/{name}",
            get(run_log).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/logs",
            get(logs).fallback(static_or_not_found),
        )
        .route(
            "/api/dispatch/logs/{name}",
            get(log_text).fallback(static_or_not_found),
        )
}

/// Die Einstellung für die Oberfläche; eine kaputte Datei liefert das Beispiel mit
/// Fehlermeldung
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchConfigView {
    pub file: String,
    pub exists: bool,
    pub raw: Value,
    pub config: Option<Value>,
    pub error: Option<String>,
    /// Standardvorlage, Platzhalter und Pflichtteile je Laufart – unabhängig von der Datei
    pub prompt_info: Vec<DispatchPromptInfo>,
}

/// `GET /api/dispatch/config`
async fn config(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&config_view(&state)?))
}

/// `PUT /api/dispatch/config`: prüft, speichert und meldet einen Wechsel der Automatiken
async fn save_config(
    State(state): State<Arc<AppState>>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    if !raw.is_object() {
        return Err(ApiError::new(
            "input-invalid",
            "Die Einstellung muss ein Objekt sein.",
        ));
    }
    let service = state.dispatch.clone();
    let previous = config_view(&state)?;
    let before = auto_of(&previous, "research");
    let plan_before = auto_of(&previous, "planning");
    check_accounts(&state, &raw)?;
    let store = nexus_core::dispatch::DispatchConfigStore::new(state.home.clone());
    store.write(&raw)?;
    let view = config_view(&state)?;
    // Recherche-Automatik ein- oder ausgeschaltet: neuer Zeitpunkt, ab dem neue Fragen zählen
    let after = auto_of(&view, "research");
    if after != before {
        service.research_auto_switched(after).await?;
    }
    // Planungs-Automatik ein- oder ausgeschaltet: neuer Zeitpunkt, ab dem neue Tickets zählen
    let plan_after = auto_of(&view, "planning");
    if plan_after != plan_before {
        service.planning_auto_switched(plan_after).await?;
    }
    Ok(json(&view))
}

/// Profile und Regeln dürfen nur Accounts nennen, die es gibt (Einstellungen → Accounts)
fn check_accounts(state: &Arc<AppState>, raw: &Value) -> ApiResult<()> {
    let known: Vec<String> = state
        .accounts
        .store
        .list()
        .into_iter()
        .map(|account| account.id)
        .collect();
    let unknown: Vec<String> = nexus_core::dispatch::AccountReferences::new()
        .find(raw)
        .into_iter()
        .filter(|(id, _)| !known.contains(id))
        .map(|(id, places)| format!("„{id}“ ({})", places.join(", ")))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    Err(ApiError::new(
        "dispatch-config-invalid",
        format!(
            "Unbekannte Accounts: {}. Accounts legst du unter Einstellungen → Accounts an.",
            unknown.join(", ")
        ),
    ))
}

/// `POST /api/dispatch/prompts/preview`
async fn preview(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    if !body.is_object() {
        return Err(ApiError::new(
            "input-invalid",
            "Die Anfrage muss ein Objekt sein.",
        ));
    }
    let kinds = PromptKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let kind = body
        .get("kind")
        .and_then(Value::as_str)
        .and_then(PromptKind::parse)
        .ok_or_else(|| ApiError::new("input-invalid", format!("kind muss {kinds} sein.")))?;
    let template = optional_text(
        &body,
        "template",
        "template muss eine Vorlage als Text sein.",
    )?;
    let id = optional_text(&body, "id", "id muss eine Ticket- oder Fragen-ID sein.")?;
    if id.as_deref().is_some_and(|id| id.trim().is_empty()) {
        return Err(ApiError::new(
            "input-invalid",
            "id muss eine Ticket- oder Fragen-ID sein.",
        ));
    }
    let preview = state
        .dispatch
        .preview_prompt(&PromptPreviewInput { kind, template, id })
        .await?;
    Ok(json(&preview))
}

/// `GET /api/dispatch/status`
async fn status(State(state): State<Arc<AppState>>, query: QueryMap) -> ApiResult<Response> {
    let board_id = board_id(query.first("boardId"))?;
    state.dispatch.require_board(board_id).await?;
    Ok(json(&state.dispatch.status(board_id).await?))
}

/// `POST /api/dispatch/run`: ein Durchlauf von Hand
async fn run(State(state): State<Arc<AppState>>, JsonBody(body): JsonBody) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    state.dispatch.tick().await;
    Ok(json(&state.dispatch.status(board_id).await?))
}

/// `POST /api/dispatch/pause`
async fn pause(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    let minutes = match body.get("minutes") {
        None | Some(Value::Null) => DEFAULT_PAUSE_MINUTES,
        Some(value) => value
            .as_i64()
            .filter(|_| value.is_i64())
            .ok_or_else(bad_minutes)?,
    };
    if !(1..=1440).contains(&minutes) {
        return Err(bad_minutes());
    }
    state.dispatch.pause(minutes).await?;
    Ok(json(&state.dispatch.status(board_id).await?))
}

/// `POST /api/dispatch/resume`
async fn resume(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    state.dispatch.resume().await?;
    Ok(json(&state.dispatch.status(board_id).await?))
}

/// `POST /api/dispatch/enable`
async fn enable(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    state.dispatch.set_enabled(true).await?;
    Ok(json(&state.dispatch.status(board_id).await?))
}

/// `POST /api/dispatch/disable`
async fn disable(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    state.dispatch.set_enabled(false).await?;
    Ok(json(&state.dispatch.status(board_id).await?))
}

/// `POST /api/dispatch/merge/{id}`: „Erneut mergen“
async fn retry_merge(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    let report = state.dispatch.retry_merge(&id).await?;
    with_result(&state, board_id, &report.result).await
}

/// `POST /api/dispatch/accept/{id}`: „Abnehmen“ (Human Interaction)
async fn accept(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    let by = optional_text(&body, "by", "by muss ein Name sein.")?;
    let report = state.dispatch.accept(&id, by.as_deref()).await?;
    with_result(&state, board_id, &report.result).await
}

/// `GET /api/dispatch/review/{id}`
async fn review(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    Ok(json(&state.dispatch.review(&id).await?))
}

/// `GET /api/dispatch/review/{id}/file?path=…`
async fn review_file(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let path = query
        .first("path")
        .filter(|path| !path.is_empty())
        .ok_or_else(|| ApiError::new("input-invalid", "path (Datei aus der Liste) fehlt."))?
        .to_string();
    Ok(json(&state.dispatch.review_file(&id, &path).await?))
}

/// `POST /api/dispatch/return/{id}`: „Zurück an den Agenten“
async fn return_to_agent(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    let comment = body.get("comment").and_then(Value::as_str).ok_or_else(|| {
        ApiError::new(
            "input-invalid",
            "comment (Rückmeldung an den Agenten) fehlt.",
        )
    })?;
    let by = optional_text(&body, "by", "by muss ein Name sein.")?;
    let report = state
        .dispatch
        .return_to_agent(&id, comment, by.as_deref())
        .await?;
    with_result(&state, board_id, &report.result).await
}

/// `GET /api/dispatch/research/{questionId}`
async fn research_status(
    State(state): State<Arc<AppState>>,
    UrlPath(question_id): UrlPath<String>,
) -> ApiResult<Response> {
    Ok(json(&state.dispatch.research_status(&question_id).await?))
}

/// `POST /api/dispatch/research/{questionId}`: „Recherche starten“
async fn start_research(
    State(state): State<Arc<AppState>>,
    UrlPath(question_id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    let started = state.dispatch.start_research(&question_id).await?;
    with_started(&state, board_id, &started).await
}

/// `GET /api/dispatch/plan/{ticketId}`
async fn planning_status(
    State(state): State<Arc<AppState>>,
    UrlPath(ticket_id): UrlPath<String>,
) -> ApiResult<Response> {
    Ok(json(&state.dispatch.planning_status(&ticket_id).await?))
}

/// `POST /api/dispatch/plan/{ticketId}`: „Planen“
async fn start_planning(
    State(state): State<Arc<AppState>>,
    UrlPath(ticket_id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let board_id = body_board(&state.dispatch, &body).await?;
    let started = state.dispatch.start_planning(&ticket_id).await?;
    with_started(&state, board_id, &started).await
}

/// `GET /api/dispatch/runs?ticket=…`: die Läufe eines Tickets bzw. einer Frage
async fn runs(State(state): State<Arc<AppState>>, query: QueryMap) -> ApiResult<Response> {
    let ticket = query
        .first("ticket")
        .map(str::trim)
        .filter(|ticket| !ticket.is_empty())
        .ok_or_else(|| ApiError::new("input-invalid", "ticket (Ticket- oder Fragen-ID) fehlt."))?
        .to_string();
    let runs = state.dispatch.ticket_runs(&ticket).await?;
    Ok(json(
        &crate::dispatch::run_facts::with_facts(&state, runs).await,
    ))
}

/// `GET /api/dispatch/runs/{name}`: ein Lauf-Protokoll, zerlegt
async fn run_log(
    State(state): State<Arc<AppState>>,
    UrlPath(name): UrlPath<String>,
) -> ApiResult<Response> {
    Ok(json(&state.dispatch.run_log(&name).await?))
}

/// `GET /api/dispatch/logs`: die Namen der Protokolle
async fn logs(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&state.dispatch.stores.log.list()?))
}

/// `GET /api/dispatch/logs/{name}`: ein Protokoll als Text
async fn log_text(
    State(state): State<Arc<AppState>>,
    UrlPath(name): UrlPath<String>,
) -> ApiResult<Response> {
    let text = state.dispatch.stores.log.read(&name)?;
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response())
}

/// Die Einstellung für die Oberfläche; eine kaputte Datei liefert das Beispiel mit
/// Fehlermeldung. Die Vorlagen stehen auch im Fehlerfall dabei, damit die Oberfläche sie bei
/// kaputter `dispatch.yaml` noch anzeigen kann.
pub fn config_view(state: &Arc<AppState>) -> ApiResult<DispatchConfigView> {
    let store = nexus_core::dispatch::DispatchConfigStore::new(state.home.clone());
    let file = store.file().display().to_string();
    let prompt_info = PromptCatalog::new().list();
    match store.read() {
        Ok(read) => Ok(DispatchConfigView {
            file,
            exists: read.exists,
            raw: read.raw,
            config: Some(
                serde_json::to_value(&read.config).map_err(nexus_core::Error::unexpected)?,
            ),
            error: None,
            prompt_info,
        }),
        Err(error) => Ok(DispatchConfigView {
            file,
            exists: true,
            raw: DispatchConfigNormalizer::new().example(),
            config: None,
            error: Some(message_of(&error)),
            prompt_info,
        }),
    }
}

/// Die Meldung eines Fehlers, wie sie in TypeScript in `error.message` steht
fn message_of(error: &nexus_core::Error) -> String {
    match error.harness() {
        Some(harness) => harness.message.clone(),
        None => error.to_string(),
    }
}

/// Steht die Automatik (`research` bzw. `planning`) in dieser Sicht auf an?
fn auto_of(view: &DispatchConfigView, part: &str) -> bool {
    view.config
        .as_ref()
        .and_then(|config| config.get(part))
        .and_then(|part| part.get("auto"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Zustand plus `result` (für „Erneut mergen“, „Abnehmen“ und „Zurück an den Agenten“)
async fn with_result<T: Serialize>(
    state: &Arc<AppState>,
    board_id: Option<i32>,
    result: &T,
) -> ApiResult<Response> {
    merged(state, board_id, "result", result).await
}

/// Zustand plus `started` (für „Recherche starten“ und „Planen“)
async fn with_started<T: Serialize>(
    state: &Arc<AppState>,
    board_id: Option<i32>,
    started: &T,
) -> ApiResult<Response> {
    merged(state, board_id, "started", started).await
}

/// Der Zustand mit einem zusätzlichen Feld; der Zustand selbst bleibt unverändert
async fn merged<T: Serialize>(
    state: &Arc<AppState>,
    board_id: Option<i32>,
    name: &str,
    extra: &T,
) -> ApiResult<Response> {
    let status = state.dispatch.status(board_id).await?;
    let mut value = serde_json::to_value(&status).map_err(nexus_core::Error::unexpected)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            name.to_string(),
            serde_json::to_value(extra).map_err(nexus_core::Error::unexpected)?,
        );
    }
    Ok(json(&value))
}

/// Board-ID aus dem Körper; vor jeder Aktion geprüft (Form und Existenz), damit eine ungültige
/// Eingabe nichts verändert
async fn body_board(service: &DispatchService, body: &Value) -> ApiResult<Option<i32>> {
    let raw = body.get("boardId");
    let id = match raw {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if text.is_empty() => None,
        Some(Value::String(text)) => Some(parsed_board(text)?),
        Some(Value::Number(number)) => Some(checked_board(number.as_f64(), &number.to_string())?),
        Some(other) => return Err(invalid_board(&other.to_string())),
    };
    service.require_board(id).await?;
    Ok(id)
}

/// Board-ID aus dem Abfrageteil; fehlend oder leer → keine
fn board_id(value: Option<&str>) -> ApiResult<Option<i32>> {
    match value {
        None | Some("") => Ok(None),
        Some(text) => Ok(Some(parsed_board(text)?)),
    }
}

/// Eine Board-ID als Text; wie `Number(value)` in JavaScript, aber ohne dessen Eigenheiten für
/// leere Werte (die fängt der Aufrufer ab)
fn parsed_board(text: &str) -> ApiResult<i32> {
    checked_board(text.trim().parse::<f64>().ok(), text)
}

/// Eine Board-ID ist eine ganze Zahl ab 1
fn checked_board(value: Option<f64>, raw: &str) -> ApiResult<i32> {
    match value {
        Some(number) if number.fract() == 0.0 && number >= 1.0 && number <= i32::MAX as f64 => {
            Ok(number as i32)
        }
        _ => Err(invalid_board(raw)),
    }
}

/// „… ist keine Board-ID.“
fn invalid_board(raw: &str) -> ApiError {
    ApiError::new("id-invalid", format!("„{raw}“ ist keine Board-ID."))
}

/// Ein Textfeld, das fehlen oder `null` sein darf; alles andere ist ein Eingabefehler
fn optional_text(body: &Value, name: &str, message: &str) -> ApiResult<Option<String>> {
    match body.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(ApiError::new("input-invalid", message)),
    }
}

/// „minutes muss eine ganze Zahl von 1 bis 1440 sein.“
fn bad_minutes() -> ApiError {
    ApiError::new(
        "input-invalid",
        "minutes muss eine ganze Zahl von 1 bis 1440 sein.",
    )
}
