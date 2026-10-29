//! Routengruppe `db-misc`: Notizen, Suche, Dispatch-Statistik, Abhängigkeitsgraph,
//! Spec-Fortschritt und „Meine Aufgaben“. Portiert aus `packages/server/src/routes/`
//! (`DbNoteRoutes.ts`, `DbSearchRoutes.ts`, `DbStatsRoutes.ts`, `DbGraphRoutes.ts`,
//! `DbInboxRoutes.ts`).
//!
//! Die Fachlogik steckt in `nexus-core`; hier geht es um Eingabeprüfung, den Zeitraum der
//! Statistik in Ortszeit-Tagen und die benutzerlokale Datei mit der Reihenfolge der Inbox.

pub mod stats_range;

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::Router;
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use nexus_core::board::basics::NoteState;
use nexus_core::board::dependency_graph_service::GraphRequest;
use nexus_core::board::dispatch_run_types::DispatchStatsFilter;
use nexus_core::board::inbox::order::InboxOrder;
use nexus_core::board::inbox::order_file::InboxOrderFile;
use nexus_core::board::inbox::service::InboxFilter;
use nexus_core::board::inbox::types::{is_inbox_key, MAX_INBOX_KEYS, MAX_INBOX_KEY_LENGTH};
use nexus_core::board::note_service::{AddNoteOptions, NewNoteText, TakeNoteOptions};
use nexus_core::board::search_service::SearchInput;
use nexus_core::board::search_types::{SearchKind, SEARCH_KINDS};
use nexus_core::clock::iso;
use nexus_core::text::utf16_prefix;
use nexus_core::{HarnessError, Result};
use serde_json::{json, Value};

use crate::error::{json as json_response, ApiError, ApiResult};
use crate::extract::{JsonBody, QueryMap, QueryValue};
use crate::input::DbInput;
use crate::routes::static_or_not_found;
use crate::state::AppState;

use stats_range::{day_text, StatsQuery, StatsRange};

/// Filter der Notizliste und die Zustände dahinter
const FILTERS: [(&str, &[NoteState]); 4] = [
    ("open", &[NoteState::Open]),
    ("taken", &[NoteState::Taken]),
    ("dropped", &[NoteState::Dropped]),
    (
        "all",
        &[NoteState::Open, NoteState::Taken, NoteState::Dropped],
    ),
];

/// Meldet die 12 Routen der Gruppe an. Der Rückfall der Methoden gehört zu jeder Route: hat
/// eine Rust-Route den Pfad, aber nicht die Methode, darf axum kein 405 liefern.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/db/boards/{id}/notes",
            get(list_notes).post(add_note).fallback(static_or_not_found),
        )
        .route(
            "/api/db/notes/{id}/seen",
            post(seen_note).fallback(static_or_not_found),
        )
        .route(
            "/api/db/notes/{id}/take",
            post(take_note).fallback(static_or_not_found),
        )
        .route(
            "/api/db/notes/{id}/drop",
            post(drop_note).fallback(static_or_not_found),
        )
        .route("/api/db/search", get(search).fallback(static_or_not_found))
        .route(
            "/api/db/stats/dispatch",
            get(dispatch_stats).fallback(static_or_not_found),
        )
        .route("/api/db/graph", get(graph).fallback(static_or_not_found))
        .route(
            "/api/db/boards/{id}/spec-progress",
            get(spec_progress).fallback(static_or_not_found),
        )
        .route("/api/db/inbox", get(inbox).fallback(static_or_not_found))
        .route(
            "/api/db/inbox/order",
            put(write_order)
                .delete(clear_order)
                .fallback(static_or_not_found),
        )
}

// ---------------------------------------------------------------- Notizen

/// Die Notizen eines Boards, gefiltert nach Zustand (Standard: offen)
async fn list_notes(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    let board_id = DbInput::id(&raw_id, "Board")?;
    let wanted = query.first("state").unwrap_or("open");
    let states = FILTERS
        .into_iter()
        .find(|(name, _)| *name == wanted)
        .map(|(_, states)| states)
        .ok_or_else(|| {
            ApiError::new(
                "input-invalid",
                format!("Filter „{wanted}“ ist ungültig. Erlaubt: open, taken, dropped, all."),
            )
        })?;
    let notes = state
        .board
        .run(|b| async move { b.services().notes.list(board_id, Some(states), None).await })
        .await?;
    Ok(json_response(&notes))
}

/// Legt eine Notiz an (201)
async fn add_note(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let board_id = DbInput::id(&raw_id, "Board")?;
    let body = DbInput::body(&raw);
    let text = NewNoteText {
        title: DbInput::text(&body, "title")?,
        body: DbInput::optional_text(&body, "body")?,
    };
    let options = AddNoteOptions {
        source: DbInput::optional_text(&body, "source")?,
        labels: DbInput::optional_list(&body, "labels")?,
        force: DbInput::optional_bool(&body, "force")?.unwrap_or(false),
    };
    let note = state
        .board
        .run(|b| async move {
            b.services()
                .notes
                .add(board_id, &text, &b.actor(None), &options)
                .await
        })
        .await?;
    Ok((StatusCode::CREATED, json_response(&note)).into_response())
}

/// Zählt eine Notiz als erneut gesehen
async fn seen_note(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Notiz")?;
    let source = DbInput::optional_text(&DbInput::body(&raw), "source")?;
    let note = state
        .board
        .run(|b| async move { b.services().notes.seen(id, source.as_deref()).await })
        .await?;
    Ok(json_response(&note))
}

/// Macht aus einer offenen Notiz ein Ticket
async fn take_note(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Notiz")?;
    let body = DbInput::body(&raw);
    let options = TakeNoteOptions {
        title: DbInput::optional_text(&body, "title")?,
        status: DbInput::optional_status(&body, "status")?,
        priority: DbInput::optional_priority(&body, "priority")?,
        labels: DbInput::optional_list(&body, "labels")?,
    };
    let taken = state
        .board
        .run(|b| async move { b.services().notes.take(id, &b.actor(None), &options).await })
        .await?;
    Ok(json_response(
        &json!({ "note": taken.note, "ticket": taken.ticket }),
    ))
}

/// Verwirft eine Notiz mit Grund
async fn drop_note(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Notiz")?;
    let reason = DbInput::text(&DbInput::body(&raw), "reason")?;
    let note = state
        .board
        .run(|b| async move { b.services().notes.drop_note(id, &reason).await })
        .await?;
    Ok(json_response(&note))
}

// ------------------------------------------------------------------ Suche

/// Die Suche über alle Projekte und Boards
async fn search(State(state): State<Arc<AppState>>, query: QueryMap) -> ApiResult<Response> {
    // Wie in Fastify: ein wiederholter Parameter kommt als Liste an und ist damit kein Text
    let q = match query.get("q") {
        QueryValue::One(value) => value,
        _ => String::new(),
    };
    let input = SearchInput {
        q,
        kinds: kinds(&query)?,
        limit: limit(&query)?,
    };
    let result = state
        .board
        .run(|b| async move { b.services().search.search(&input).await })
        .await?;
    Ok(json_response(&result))
}

/// Die gesuchten Arten; ohne Angabe alle
fn kinds(query: &QueryMap) -> Result<Vec<SearchKind>> {
    let QueryValue::One(raw) = query.get("kinds") else {
        return Ok(Vec::new());
    };
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    raw.split(',')
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .map(|wanted| {
            SEARCH_KINDS
                .into_iter()
                .find(|kind| kind.as_str() == wanted)
                .ok_or_else(|| {
                    invalid(format!(
                        "„{wanted}“ ist keine Art (ticket, question, note)."
                    ))
                })
        })
        .collect()
}

/// Die Höchstzahl der Treffer; ohne Angabe der Standardwert des Dienstes
fn limit(query: &QueryMap) -> Result<Option<usize>> {
    let raw = match query.get("limit") {
        QueryValue::Missing => return Ok(None),
        QueryValue::One(raw) if raw.is_empty() => return Ok(None),
        QueryValue::One(raw) => raw,
        // Eine Liste ist keine Zahl (in JavaScript `NaN`)
        QueryValue::Many(_) => String::new(),
    };
    raw.trim()
        .parse::<usize>()
        .ok()
        .filter(|value| *value >= 1)
        .map(Some)
        .ok_or_else(|| invalid("„limit“ muss eine ganze Zahl ab 1 sein."))
}

// ------------------------------------------------------------- Statistik

/// Die Kennzahlen der Dispatcher-Läufe eines Projekts
async fn dispatch_stats(
    State(state): State<Arc<AppState>>,
    query: QueryMap,
) -> ApiResult<Response> {
    let project = query
        .first("project")
        .map(str::trim)
        .filter(|project| !project.is_empty())
        .ok_or_else(|| ApiError::new("input-invalid", "Parameter „project“ (Projekt-Key) fehlt."))?
        .to_string();
    let board_id = match query.first("board").filter(|board| !board.is_empty()) {
        Some(raw) => Some(DbInput::id(raw, "Board")?),
        None => None,
    };
    let wanted = StatsQuery {
        from: query.first("from").map(str::to_string),
        to: query.first("to").map(str::to_string),
        kind: query.first("kind").map(str::to_string),
    };
    let now = state.config.clock.now();
    let today = now.with_timezone(&Local).date_naive();
    let config_changed = match wanted.wants_config() {
        true => config_changed_at(&state, now).await?,
        false => None,
    };
    let range = StatsRange::resolve(&wanted, today, config_changed)?;
    let filter = DispatchStatsFilter {
        project_id: 0,
        board_id,
        from: midnight(range.from),
        to: midnight(range.end()),
        kind: Some(range.kind),
    };
    let found = state
        .board
        .run(|b| async move {
            let project = b
                .services()
                .projects
                .list()
                .await?
                .into_iter()
                .find(|found| found.key.to_uppercase() == project.to_uppercase())
                .ok_or_else(|| {
                    HarnessError::new(
                        "project-not-found",
                        format!("Projekt {project} gibt es nicht."),
                    )
                })?;
            if let Some(board_id) = board_id {
                let board = b.services().boards.require(board_id).await?;
                if board.project_id != project.id {
                    return Err(HarnessError::new(
                        "input-invalid",
                        format!("Board {board_id} gehört nicht zum Projekt {}.", project.key),
                    )
                    .into());
                }
            }
            let stats = b
                .services()
                .dispatch_runs
                .stats(&DispatchStatsFilter {
                    project_id: project.id,
                    ..filter
                })
                .await?;
            Ok((project.key, stats))
        })
        .await?;
    let (key, stats) = found;
    // Feldreihenfolge wie in TypeScript: `filter` zuerst, dann die Kennzahlen
    let mut body = json!({
        "filter": {
            "project": key,
            "boardId": board_id,
            "from": day_text(range.from),
            "to": day_text(range.to),
            "kind": range.kind.as_str(),
        }
    });
    merge(
        &mut body,
        serde_json::to_value(&stats).map_err(nexus_core::Error::unexpected)?,
    );
    Ok(json_response(&body))
}

/// Der Ortszeit-Tag, an dem die neueste Fassung der Dispatcher-Einstellung zuerst auftrat
async fn config_changed_at(state: &Arc<AppState>, now: DateTime<Utc>) -> Result<Option<NaiveDate>> {
    let epoch = iso(DateTime::from_timestamp_millis(0).expect("die Epoche ist gültig"));
    let until = iso(now);
    let versions = state
        .board
        .run(|b| async move {
            b.services()
                .dispatch_runs
                .config_versions(&epoch, &until)
                .await
        })
        .await?;
    Ok(versions
        .last()
        .and_then(|version| version.first_seen_at.parse::<DateTime<Utc>>().ok())
        .map(|at| at.with_timezone(&Local).date_naive()))
}

/// Mitternacht eines Tages in Ortszeit als Zeitpunkt; fällt sie wegen einer Zeitumstellung
/// aus, gilt der nächste gültige Augenblick
fn midnight(day: NaiveDate) -> DateTime<Utc> {
    let start = day.and_hms_opt(0, 0, 0).expect("Mitternacht gibt es");
    Local
        .from_local_datetime(&start)
        .earliest()
        .map(|at| at.with_timezone(&Utc))
        .unwrap_or_else(|| {
            // Die Stunde fehlt an diesem Tag (Umstellung): eine Stunde später
            Local
                .from_local_datetime(&(start + chrono::Duration::hours(1)))
                .earliest()
                .map(|at| at.with_timezone(&Utc))
                .unwrap_or_else(|| Utc.from_utc_datetime(&start))
        })
}

/// Hängt die Felder eines Objekts an ein anderes an
fn merge(target: &mut Value, extra: Value) {
    let (Some(target), Some(extra)) = (target.as_object_mut(), extra.as_object()) else {
        return;
    };
    for (key, value) in extra {
        target.insert(key.clone(), value.clone());
    }
}

// -------------------------------------------------------------------- Graph

/// Der Abhängigkeitsgraph eines Umfangs
async fn graph(State(state): State<Arc<AppState>>, query: QueryMap) -> ApiResult<Response> {
    let scope = scope(&query)?;
    let done = query
        .first("done")
        .map(|value| value == "1" || value == "true");
    let view = state
        .board
        .run(|b| async move { b.services().graph.build(&scope, done).await })
        .await?;
    Ok(json_response(&view))
}

/// Der Fortschritt je Spec eines Boards
async fn spec_progress(
    State(state): State<Arc<AppState>>,
    UrlPath(raw_id): UrlPath<String>,
) -> ApiResult<Response> {
    let id = DbInput::id(&raw_id, "Board")?;
    let progress = state
        .board
        .run(|b| async move { b.services().graph.for_board(id).await })
        .await?;
    Ok(json_response(&progress))
}

/// Genau ein Umfang: `ticket`, `board` oder `project` mit `spec`
fn scope(query: &QueryMap) -> Result<GraphRequest> {
    let value = |name: &str| {
        query
            .first(name)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let (ticket, board, project, spec) = (
        value("ticket"),
        value("board"),
        value("project"),
        value("spec"),
    );
    let given = [ticket, board, project.or(spec)]
        .iter()
        .filter(|value| value.is_some())
        .count();
    if given != 1 {
        return Err(invalid(
            "Genau einen Umfang angeben: „ticket“, „board“ oder „project“ mit „spec“.",
        ));
    }
    if let Some(ticket) = ticket {
        return Ok(GraphRequest::Ticket {
            id: ticket.to_string(),
        });
    }
    if let Some(board) = board {
        return Ok(GraphRequest::Board {
            board_id: DbInput::id(board, "Board")?,
        });
    }
    let (Some(project), Some(spec)) = (project, spec) else {
        return Err(invalid("Für eine Spec „project“ und „spec“ angeben."));
    };
    Ok(GraphRequest::Spec {
        project: project.to_string(),
        spec: spec.to_string(),
    })
}

// -------------------------------------------------------------------- Inbox

/// „Meine Aufgaben“ über alle eigenen Boards, in der gespeicherten Reihenfolge
async fn inbox(State(state): State<Arc<AppState>>, query: QueryMap) -> ApiResult<Response> {
    let board_id = match query.first("board").filter(|board| !board.is_empty()) {
        Some(raw) => Some(DbInput::id(raw, "Board")?),
        None => None,
    };
    let keys = order_file(&state).read();
    let list = state
        .board
        .run(|b| async move {
            let items = b
                .services()
                .inbox
                .list(&b.actor(None).user_id, &InboxFilter { board_id })
                .await?;
            Ok(nexus_core::board::inbox::service::InboxService::<
                crate::board_db::ServerBoardDatabase,
            >::summarize(InboxOrder::apply(items, &keys)))
        })
        .await?;
    Ok(json_response(&list))
}

/// Speichert die Reihenfolge der Inbox im Benutzerordner
async fn write_order(
    State(state): State<Arc<AppState>>,
    JsonBody(raw): JsonBody,
) -> ApiResult<Response> {
    let keys = inbox_keys(DbInput::body(&raw).get("keys"))?;
    order_file(&state).write(&keys)?;
    Ok(json_response(&json!({ "keys": keys })))
}

/// Setzt die Reihenfolge der Inbox zurück
async fn clear_order(
    State(state): State<Arc<AppState>>,
    // Der Körper wird nicht gelesen, aber wie in Fastify geprüft: leeres JSON ist 400
    JsonBody(_): JsonBody,
) -> ApiResult<Response> {
    order_file(&state).clear()?;
    Ok(json_response(&json!({ "keys": [] })))
}

/// Die Datei mit der Reihenfolge; sie ist benutzerlokal und gehört nicht in die Datenbank
fn order_file(state: &Arc<AppState>) -> InboxOrderFile {
    InboxOrderFile::new(state.home.clone())
}

/// Schlüssel der Reihenfolge: eine Liste mit höchstens [`MAX_INBOX_KEYS`] gültigen Schlüsseln
fn inbox_keys(raw: Option<&Value>) -> Result<Vec<String>> {
    let list = raw
        .and_then(Value::as_array)
        .filter(|list| list.len() <= MAX_INBOX_KEYS);
    let Some(list) = list else {
        return Err(invalid(format!(
            "Feld „keys“ muss eine Liste mit höchstens {MAX_INBOX_KEYS} Schlüsseln sein."
        )));
    };
    list.iter()
        .map(|value| {
            let key = value.as_str().unwrap_or("");
            if value.is_string() && is_inbox_key(key) {
                return Ok(key.to_string());
            }
            // Wie in TypeScript steht der gekürzte Wert in der Meldung
            let shown = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            Err(invalid(format!(
                "„{}“ ist kein gültiger Schlüssel (z. B. ticket:AIH-0001).",
                utf16_prefix(&shown, MAX_INBOX_KEY_LENGTH)
            )))
        })
        .collect()
}

/// Fehler einer ungültigen Eingabe
fn invalid(message: impl Into<String>) -> nexus_core::Error {
    HarnessError::new("input-invalid", message).into()
}
