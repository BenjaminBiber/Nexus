//! Routengruppe `accounts`: Claude-Accounts des Dispatchers – Liste mit Anmeldung, Plugin
//! und Limits, anmelden (`claude auth login`), anlegen, umbenennen, abschalten, entfernen
//! (auch aus `dispatch.yaml`) und Pause eines Accounts aufheben. Portiert aus
//! `packages/server/src/routes/AccountRoutes.ts`.
//!
//! Jede schreibende Route antwortet mit der ganzen Liste.

use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use nexus_core::accounts::{AccountPatch, ClaudeAccount, NewAccount, STANDARD_ACCOUNT_ID};
use nexus_core::dispatch::{AccountReferences, DispatchConfigStore};
use serde::Serialize;
use serde_json::Value;

use crate::accounts::{AccountInfoDto, AccountLoginDto};
use crate::error::{json, ApiError, ApiResult};
use crate::extract::{JsonBody, QueryMap};
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Ein Account in der Liste: Angaben, Verweise aus `dispatch.yaml` und die laufende Anmeldung
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountEntryDto {
    #[serde(flatten)]
    pub info: AccountInfoDto,
    /// Stellen in `dispatch.yaml`, die den Account nennen („Profil schwer“, „Regel 2“)
    pub references: Vec<String>,
    pub login: Option<AccountLoginDto>,
}

/// Antwort von `GET /api/accounts`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountListDto {
    pub accounts: Vec<AccountEntryDto>,
    /// Mehr als ein aktiver Account: der Dispatcher verteilt und fragt dafür die Limits ab
    pub multi: bool,
    /// Ordner neuer Accounts (`~/.nexus/accounts`)
    pub accounts_dir: String,
    /// Vorhandene MCACC-Profile (Konfigurationsordner von Claude Code), die noch keinem
    /// Account gehören
    pub suggestions: Vec<String>,
    /// Befehle, die das Nexus-Plugin in einem Account-Ordner einrichten (leer: Marktplatz
    /// unbekannt)
    pub plugin_commands: Vec<String>,
}

/// Nur ID und Name eines Accounts (Ticket-Ansicht)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountNameDto {
    pub id: String,
    pub name: String,
}

/// Meldet die Routen der Gruppe an; andere Methoden auf denselben Pfaden fallen in den
/// Rückfall, damit axum kein 405 liefert
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/accounts",
            get(list).post(create).fallback(static_or_not_found),
        )
        .route(
            "/api/accounts/names",
            get(names).fallback(static_or_not_found),
        )
        .route(
            "/api/accounts/login",
            post(create_and_login).fallback(static_or_not_found),
        )
        .route(
            "/api/accounts/{id}",
            get(list)
                .patch(update)
                .delete(remove)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/accounts/{id}/login",
            post(login)
                .delete(cancel_login)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/accounts/{id}/login/code",
            post(login_code).fallback(static_or_not_found),
        )
        .route(
            "/api/accounts/{id}/plugin",
            post(install_plugin).fallback(static_or_not_found),
        )
        .route(
            "/api/accounts/{id}/resume",
            post(resume).fallback(static_or_not_found),
        )
}

/// `GET /api/accounts`
async fn list(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&listing(&state).await?))
}

/// `GET /api/accounts/names`: nur IDs und Namen, ohne Anmeldung und Limits
async fn names(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let accounts: Vec<AccountNameDto> = state
        .accounts
        .store
        .list()
        .into_iter()
        .map(|account| AccountNameDto {
            id: account.id,
            name: account.name,
        })
        .collect();
    Ok(json(&serde_json::json!({ "accounts": accounts })))
}

/// `POST /api/accounts/login`: neuer Account samt Anmeldung; ohne Namen heißt er nach der
/// Anmeldung wie die E-Mail
async fn create_and_login(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let body = object(&body)?;
    let name = text(body.get("name"), "name")?;
    let email = text(body.get("email"), "email")?;
    let account = state.accounts.store.create(&NewAccount {
        name: first_of(&[&name, &email, "Neuer Account"]),
        config_dir: None,
    })?;
    if name.is_empty() {
        state
            .accounts
            .rename_on_login
            .lock()
            .expect("Sperre der Umbenennungen")
            .insert(account.id.clone());
    }
    state
        .accounts
        .logins
        .start(&account, &command(&state), Some(&email));
    Ok(json(&listing(&state).await?))
}

/// `POST /api/accounts/{id}/login`
async fn login(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let account = require(&state, &id)?;
    let email = text(object(&body)?.get("email"), "email")?;
    state
        .accounts
        .logins
        .start(&account, &command(&state), Some(&email));
    Ok(json(&listing(&state).await?))
}

/// `POST /api/accounts/{id}/login/code`
async fn login_code(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let code = text(object(&body)?.get("code"), "code")?;
    state.accounts.logins.code(&id, &code)?;
    Ok(json(&listing(&state).await?))
}

/// `DELETE /api/accounts/{id}/login`
async fn cancel_login(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    state.accounts.logins.cancel(&id, true);
    Ok(json(&listing(&state).await?))
}

/// `POST /api/accounts`: neuer Account, wahlweise auf einem vorhandenen Ordner
async fn create(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let body = object(&body)?;
    let config_dir = match body.get("configDir") {
        None | Some(Value::Null) => None,
        Some(Value::String(dir)) => Some(dir.clone()),
        Some(_) => {
            return Err(ApiError::new(
                "input-invalid",
                "configDir muss ein Pfad sein.",
            ))
        }
    };
    let name = match body.get("name") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(name)) => name.clone(),
        Some(value) => value.to_string(),
    };
    state
        .accounts
        .store
        .create(&NewAccount { name, config_dir })?;
    Ok(json(&listing(&state).await?))
}

/// `PATCH /api/accounts/{id}`: Name, Schalter und Obergrenzen
async fn update(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let body = object(&body)?;
    let mut patch = AccountPatch::default();
    if let Some(value) = body.get("name") {
        patch.name = Some(
            value
                .as_str()
                .ok_or_else(|| ApiError::new("input-invalid", "name muss ein Text sein."))?
                .to_string(),
        );
    }
    patch.enabled = flag(&body, "enabled")?;
    patch.fallback = flag(&body, "fallback")?;
    patch.limit_five_hour = limit(&body, "limitFiveHour", "5 Stunden")?;
    patch.limit_week = limit(&body, "limitWeek", "Woche")?;
    state.accounts.store.update(&id, &patch)?;
    Ok(json(&listing(&state).await?))
}

/// `DELETE /api/accounts/{id}?deleteDir=1`
async fn remove(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    query: QueryMap,
) -> ApiResult<Response> {
    if id == STANDARD_ACCOUNT_ID {
        return Err(ApiError::new(
            "account-invalid",
            "Den Standard-Account kann man nur abschalten, nicht entfernen.",
        ));
    }
    require(&state, &id)?;
    // Verweise in dispatch.yaml zuerst entfernen: scheitert das Speichern, bleibt der Account
    let configs = DispatchConfigStore::new(state.home.clone());
    if let Ok(read) = configs.read() {
        let refs = AccountReferences::new();
        if read.exists && refs.find(&read.raw).contains_key(&id) {
            configs.write(&refs.without(&read.raw, &id))?;
        }
    }
    state.accounts.logins.cancel(&id, false);
    state
        .accounts
        .rename_on_login
        .lock()
        .expect("Sperre der Umbenennungen")
        .remove(&id);
    let delete_dir = query.first("deleteDir") == Some("1");
    state.accounts.store.remove(&id, delete_dir)?;
    state.dispatch.resume_account(&id).await?;
    Ok(json(&listing(&state).await?))
}

/// `POST /api/accounts/{id}/plugin`
async fn install_plugin(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    let account = require(&state, &id)?;
    state.accounts.plugins.install(&account, &command(&state))?;
    Ok(json(&listing(&state).await?))
}

/// `POST /api/accounts/{id}/resume`: Pause dieses Accounts aufheben
async fn resume(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Response> {
    state.dispatch.resume_account(&id).await?;
    Ok(json(&listing(&state).await?))
}

/// Die ganze Liste, wie jede Route sie beantwortet
async fn listing(state: &Arc<AppState>) -> ApiResult<AccountListDto> {
    let pauses = state.dispatch.account_pauses()?;
    let infos = state.accounts.usage.infos(&pauses, true).await;
    let accounts = state.accounts.store.list();
    let read = DispatchConfigStore::new(state.home.clone()).read().ok();
    let refs = read
        .map(|read| AccountReferences::new().find(&read.raw))
        .unwrap_or_default();
    let taken: Vec<String> = infos
        .iter()
        .map(|info| info.effective_dir.clone())
        .collect();
    Ok(AccountListDto {
        accounts: infos
            .into_iter()
            .map(|info| AccountEntryDto {
                references: refs.get(&info.id).cloned().unwrap_or_default(),
                login: state.accounts.logins.status(&info.id),
                info,
            })
            .collect(),
        multi: accounts.iter().filter(|account| account.enabled).count() > 1,
        accounts_dir: state.home.accounts_dir().to_string_lossy().into_owned(),
        suggestions: suggestions(state)
            .into_iter()
            .filter(|dir| !taken.contains(dir))
            .collect(),
        plugin_commands: state.accounts.plugins.commands(),
    })
}

/// Konfigurationsordner der MCACC-Profile (`~/.mcacc/profiles/<id>/claude`), soweit vorhanden
fn suggestions(state: &Arc<AppState>) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(&state.accounts.mcacc_dir) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.path().join("claude"))
        .filter(|dir| dir.is_dir())
        .map(|dir| dir.to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

/// Der Account oder `account-not-found`
fn require(state: &Arc<AppState>, id: &str) -> ApiResult<ClaudeAccount> {
    state.accounts.store.find(id).ok_or_else(|| {
        ApiError::new(
            "account-not-found",
            format!("Account „{id}“ gibt es nicht."),
        )
    })
}

/// Aufruf von Claude Code aus der Dispatcher-Einstellung
fn command(state: &Arc<AppState>) -> String {
    DispatchConfigStore::new(state.home.clone())
        .read()
        .map(|read| read.config.claude.command)
        .unwrap_or_else(|_| "claude".to_string())
}

/// Getrimmter Text oder leer; anderes als Text ist ein Fehler
fn text(value: Option<&Value>, name: &str) -> ApiResult<String> {
    match value {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(text)) => Ok(text.trim().to_string()),
        Some(_) => Err(ApiError::new(
            "input-invalid",
            format!("{name} muss ein Text sein."),
        )),
    }
}

/// Ein Schalter aus dem Körper: nicht genannt oder true/false
fn flag(body: &serde_json::Map<String, Value>, key: &str) -> ApiResult<Option<bool>> {
    match body.get(key) {
        None => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        Some(_) => Err(ApiError::new(
            "input-invalid",
            format!("{key} muss true oder false sein."),
        )),
    }
}

/// Eine Obergrenze aus dem Körper: nicht genannt, `null` (entfernen) oder eine ganze Zahl
/// von 1 bis 100
fn limit(
    body: &serde_json::Map<String, Value>,
    key: &str,
    label: &str,
) -> ApiResult<Option<Option<i64>>> {
    match body.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::Number(number)) => {
            match nexus_core::accounts::account_limit(&Value::Number(number.clone())) {
                Some(limit) => Ok(Some(Some(limit))),
                None => Err(ApiError::new(
                    "input-invalid",
                    format!("Die Obergrenze für {label} muss eine ganze Zahl von 1 bis 100 sein."),
                )),
            }
        }
        Some(_) => Err(ApiError::new(
            "input-invalid",
            format!("{key} muss eine Zahl oder null sein."),
        )),
    }
}

/// Der Körper als Objekt
fn object(raw: &Value) -> ApiResult<serde_json::Map<String, Value>> {
    raw.as_object()
        .cloned()
        .ok_or_else(|| ApiError::new("input-invalid", "Die Anfrage muss ein Objekt sein."))
}

/// Der erste nicht leere Text
fn first_of(texts: &[&str]) -> String {
    texts
        .iter()
        .find(|text| !text.is_empty())
        .unwrap_or(&"")
        .to_string()
}
