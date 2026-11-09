//! Routengruppe `settings`: Projekte, Projekteinstellungen, Benutzereinstellungen und die
//! Board-Konfig; portiert aus `packages/server/src/routes/{Project,Settings,UserSettings,
//! BoardConfig}Routes.ts`.
//!
//! Die beiden Testmeldungen der Benutzereinstellungen (`POST /api/user-settings/
//! test-notification` und `…/test-ntfy`) gehören zur Gruppe `notifications`
//! ([`crate::routes::notifications`]); hier wird nur `/api/user-settings` selbst angemeldet.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use nexus_core::board::config::BoardConfig;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::project::TokenStore;
use nexus_core::registry::NexusHome;
use nexus_core::settings::{
    known_theme, SetupPrompts, ThemeMode, UserSettings, UserSettingsStore, NOTIFICATION_MODES,
    NTFY_TOPIC_PATTERN,
};
use nexus_core::{Error, HarnessError, Result};
use nexus_db::ConnectOptions;
use serde_json::Value;
use serde_yaml_ng::{Mapping, Value as Yaml};

use crate::error::{json, ApiResult};
use crate::extract::JsonBody;
use crate::routes::static_or_not_found;
use crate::state::AppState;

/// Adresse des ntfy-Servers: http oder https, danach mindestens ein Zeichen ohne Leerraum
/// (wortgleich zu `/^https?:\/\/\S+$/` in TypeScript)
fn is_ntfy_server(value: &str) -> bool {
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty() && !rest.chars().any(char::is_whitespace))
}

/// Meldet die acht Routen der Gruppe an. Der Rückfall der Methoden gehört zu jeder Route:
/// hat eine Rust-Route den Pfad, aber nicht die Methode, darf axum kein 405 liefern.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/projects",
            get(list_projects)
                .post(register_project)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/projects/{pid}/settings",
            get(project_settings).fallback(static_or_not_found),
        )
        .route(
            "/api/user-settings",
            get(read_user_settings)
                .put(write_user_settings)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/board-config",
            get(read_board_config)
                .put(write_board_config)
                .fallback(static_or_not_found),
        )
        .route(
            "/api/board-config/test",
            post(test_board_config).fallback(static_or_not_found),
        )
}

/// Alle angemeldeten Projekte mit Erreichbarkeit
async fn list_projects(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&state.projects.list()?))
}

/// Meldet ein Projekt an; 201 neu, 200 schon bekannt
async fn register_project(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let path = body.get("path").cloned().unwrap_or(Value::Null);
    let added = state.projects.register(&path)?;
    let status = if added.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, json(&added.project)).into_response())
}

/// Projektangaben und das Token für die Hooks
async fn project_settings(
    State(state): State<Arc<AppState>>,
    Path(pid): Path<String>,
) -> ApiResult<Response> {
    let opened = state.projects.open(&pid)?;
    let token = TokenStore::new(opened.project.layout.clone()).read()?;
    Ok(json(&serde_json::json!({
        "project": opened.entry,
        "token": token,
    })))
}

/// Die benutzerweiten Einstellungen
async fn read_user_settings(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    Ok(json(&UserSettingsStore::new(state.home.clone()).read()))
}

/// Übernimmt die angegebenen Felder und liefert den gespeicherten Stand
async fn write_user_settings(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let store = UserSettingsStore::new(state.home.clone());
    let merged = merge_user_settings(store.read(), &body, &state.home)?;
    store.write(&merged)?;
    Ok(json(&merged))
}

/// Ungültige Einstellung; Code und Meldung wie in TypeScript
fn settings_invalid(message: &str) -> Error {
    HarnessError::new("settings-invalid", message).into()
}

/// Übernimmt die angegebenen Felder; ungültige Werte werden abgelehnt
fn merge_user_settings(
    current: UserSettings,
    body: &Value,
    home: &NexusHome,
) -> Result<UserSettings> {
    let mut next = current;
    // In JavaScript zählt nur `undefined` als „nicht angegeben“; ein ausdrückliches `null`
    // läuft in die Prüfung und wird dort abgelehnt
    let field = |name: &str| body.get(name);
    // Ein Wahrheitsfeld: alles außer true/false wird abgelehnt
    let flag = |name: &str, value: &Value| -> Result<bool> {
        value
            .as_bool()
            .ok_or_else(|| settings_invalid(&format!("{name} muss true oder false sein.")))
    };
    if let Some(value) = field("notifications") {
        next.notifications = NOTIFICATION_MODES
            .into_iter()
            .find(|mode| value.as_str() == Some(mode.as_str()))
            .ok_or_else(|| {
                settings_invalid("notifications muss off, browser oder windows sein.")
            })?;
    }
    if let Some(value) = field("heroImages") {
        next.hero_images = flag("heroImages", value)?;
    }
    if let Some(value) = field("theme") {
        next.theme = value
            .as_str()
            .filter(|id| known_theme(home, id))
            .map(ThemeMode::from)
            .ok_or_else(|| {
                settings_invalid(
                    "theme muss ein eingebautes (system, light, dark, pastel, win95) oder installiertes Theme sein.",
                )
            })?;
    }
    if let Some(value) = field("claudeUsage") {
        next.claude_usage = flag("claudeUsage", value)?;
    }
    if let Some(value) = field("compactCards") {
        next.compact_cards = flag("compactCards", value)?;
    }
    if let Some(value) = field("hideNotNeeded") {
        next.hide_not_needed = flag("hideNotNeeded", value)?;
    }
    if let Some(value) = field("hideInPlanning") {
        next.hide_in_planning = flag("hideInPlanning", value)?;
    }
    if let Some(value) = field("ntfy") {
        next.ntfy = flag("ntfy", value)?;
    }
    if let Some(value) = field("ntfyServer") {
        let server = value.as_str().unwrap_or_default().trim().to_string();
        if !is_ntfy_server(&server) {
            return Err(settings_invalid(
                "ntfyServer muss eine http- oder https-Adresse sein.",
            ));
        }
        next.ntfy_server = server;
    }
    if let Some(value) = field("ntfyTopic") {
        let topic = value
            .as_str()
            .map(|topic| topic.trim().to_string())
            .ok_or_else(|| settings_invalid(TOPIC_MESSAGE))?;
        if !topic.is_empty() && !NTFY_TOPIC_PATTERN.is_match(&topic) {
            return Err(settings_invalid(TOPIC_MESSAGE));
        }
        next.ntfy_topic = topic;
    }
    if let Some(value) = field("ntfyToken") {
        let token = value
            .as_str()
            .ok_or_else(|| settings_invalid("ntfyToken muss ein Text sein."))?;
        next.ntfy_token = token.trim().to_string();
    }
    if let Some(value) = field("setupPrompts") {
        next.setup_prompts = merge_setup_prompts(next.setup_prompts, value)?;
    }
    Ok(next)
}

/// Übernimmt die angegebenen Felder von `setupPrompts`
fn merge_setup_prompts(current: SetupPrompts, value: &Value) -> Result<SetupPrompts> {
    let object = value
        .as_object()
        .ok_or_else(|| settings_invalid("setupPrompts muss ein Objekt sein."))?;
    let mut next = current;
    if let Some(value) = object.get("pluginDismissed") {
        next.plugin_dismissed = value.as_bool().ok_or_else(|| {
            settings_invalid("setupPrompts.pluginDismissed muss true oder false sein.")
        })?;
    }
    if let Some(value) = object.get("migrationDismissed") {
        const MESSAGE: &str = "setupPrompts.migrationDismissed muss eine Liste von Board-IDs sein.";
        let list = value.as_array().ok_or_else(|| settings_invalid(MESSAGE))?;
        let mut ids = Vec::with_capacity(list.len());
        for id in list {
            let id = id
                .as_i64()
                .filter(|id| *id > 0)
                .and_then(|id| i32::try_from(id).ok())
                .ok_or_else(|| settings_invalid(MESSAGE))?;
            ids.push(id);
        }
        ids.sort_unstable();
        ids.dedup();
        next.migration_dismissed = ids;
    }
    Ok(next)
}

/// Meldung zu einem ungültigen ntfy-Thema (steht zweimal in der Prüfung)
const TOPIC_MESSAGE: &str =
    "ntfyTopic darf nur Buchstaben, Ziffern, - und _ enthalten (höchstens 64 Zeichen).";

/// Die gespeicherte Verbindung zur Board-Datenbank; eine kaputte Datei meldet hier den Fehler
async fn read_board_config(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
    let store = BoardConfigStore::new(state.home.clone());
    let config = store.read()?;
    Ok(json(&board_config_dto(&state, &store, config.as_ref())))
}

/// Speichert die Verbindung und verwirft die Board-Laufzeit, damit sie neu entsteht
async fn write_board_config(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let store = BoardConfigStore::new(state.home.clone());
    let current = read_for_repair(&store)?;
    let config = merge_board_config(&store, current.as_ref(), &body)?;
    store.write(&config)?;
    state.board.reset().await;
    Ok(json(&board_config_dto(&state, &store, Some(&config))))
}

/// Verbindet mit den übergebenen Angaben und schließt sofort wieder; gespeichert wird nichts
async fn test_board_config(
    State(state): State<Arc<AppState>>,
    JsonBody(body): JsonBody,
) -> ApiResult<Response> {
    let store = BoardConfigStore::new(state.home.clone());
    let current = read_for_repair(&store)?;
    let result = match merge_board_config(&store, current.as_ref(), &body) {
        Ok(config) => connect_and_close(config).await,
        Err(error) => Err(error),
    };
    Ok(json(&match result {
        Ok(message) => serde_json::json!({ "ok": true, "message": message }),
        Err(error) => serde_json::json!({ "ok": false, "message": failure(&error) }),
    }))
}

/// Öffnet die Datenbank der Konfig, prüft das Schema und schließt wieder
async fn connect_and_close(config: BoardConfig) -> Result<String> {
    let message = if config.provider == nexus_core::board::config::BoardProvider::Sqlite {
        format!("Datei geöffnet, Schema passt ({}).", config.file)
    } else {
        "Verbindung erfolgreich, Schema-Version passt.".to_string()
    };
    let opened = nexus_db::open_checked_owned(config, ConnectOptions::default()).await?;
    use nexus_core::board::database::BoardDatabase as _;
    opened.db.close().await?;
    Ok(message)
}

/// Meldung eines gescheiterten Verbindungstests; nur fachliche Fehler stehen im Klartext
fn failure(error: &Error) -> String {
    match error.harness() {
        Some(_) => error.to_string(),
        None => format!("Unerwarteter Fehler: {error}"),
    }
}

/// Gespeicherte Konfig als Grundlage für Speichern und Test; eine kaputte Datei gilt als
/// nicht vorhanden, damit die Oberfläche sie reparieren kann
fn read_for_repair(store: &BoardConfigStore) -> Result<Option<BoardConfig>> {
    match store.read() {
        Err(error) if error.code() == Some("board-config-invalid") => Ok(None),
        other => other,
    }
}

/// Antwort ohne Passwort; ohne Datei Standardwerte mit neuer Vorschlags-GUID.
///
/// `file` ist immer der Pfad der `board.yaml`, der Pfad der SQLite-Datei heißt `dbFile`.
fn board_config_dto(
    state: &AppState,
    store: &BoardConfigStore,
    config: Option<&BoardConfig>,
) -> Value {
    let file = store.file().display().to_string();
    let Some(config) = config else {
        return serde_json::json!({
            "configured": false,
            "provider": "mssql",
            "dbFile": state.home.board_db_file().display().to_string(),
            "file": file,
            "server": "",
            "port": 1433,
            "database": "AiHarness",
            "user": "",
            "hasPassword": false,
            "encrypt": true,
            "trustServerCertificate": true,
            "userId": uuid::Uuid::new_v4().to_string(),
            "userName": "",
            "activeBoard": Value::Null,
        });
    };
    serde_json::json!({
        "configured": true,
        "file": file,
        "provider": config.provider.as_str(),
        "server": config.server,
        "port": config.port,
        "database": config.database,
        "user": config.user,
        "encrypt": config.encrypt,
        "trustServerCertificate": config.trust_server_certificate,
        "userId": config.user_id,
        "userName": config.user_name,
        "activeBoard": config.active_board,
        "dbFile": config.file,
        "hasPassword": !config.password.is_empty(),
    })
}

/// Übernimmt die Eingabe über die gespeicherte Konfig: ein leeres Passwort behält das
/// gespeicherte, `dbFile` wird zu `file`, ein mitgeschicktes `file` zählt nicht und das
/// aktive Board bleibt.
fn merge_board_config(
    store: &BoardConfigStore,
    current: Option<&BoardConfig>,
    body: &Value,
) -> Result<BoardConfig> {
    let mut data = match current {
        Some(config) => match serde_yaml_ng::to_value(config).map_err(Error::unexpected)? {
            Yaml::Mapping(mapping) => mapping,
            _ => Mapping::new(),
        },
        None => Mapping::new(),
    };
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .filter(|password| !password.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            current
                .map(|config| config.password.clone())
                .unwrap_or_default()
        });
    let db_file = body
        .get("dbFile")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| current.map(|config| config.file.clone()));
    if let Some(object) = body.as_object() {
        for (key, value) in object {
            if key == "dbFile" || key == "file" {
                continue;
            }
            data.insert(Yaml::from(key.as_str()), to_yaml(value));
        }
    }
    match db_file {
        Some(file) => data.insert(Yaml::from("file"), Yaml::from(file)),
        None => data.remove(Yaml::from("file")),
    };
    data.insert(Yaml::from("password"), Yaml::from(password));
    data.insert(
        Yaml::from("activeBoard"),
        match current.and_then(|config| config.active_board) {
            Some(board) => Yaml::from(board),
            None => Yaml::Null,
        },
    );
    store.validate(&Yaml::Mapping(data))
}

/// JSON-Wert als YAML-Wert; die Konfig wird als YAML geprüft und geschrieben
fn to_yaml(value: &Value) -> Yaml {
    match value {
        Value::Null => Yaml::Null,
        Value::Bool(flag) => Yaml::from(*flag),
        Value::Number(number) => match number.as_i64() {
            Some(number) => Yaml::from(number),
            None => Yaml::from(number.as_f64().unwrap_or(f64::NAN)),
        },
        Value::String(text) => Yaml::from(text.as_str()),
        Value::Array(items) => Yaml::Sequence(items.iter().map(to_yaml).collect()),
        Value::Object(object) => Yaml::Mapping(
            object
                .iter()
                .map(|(key, value)| (Yaml::from(key.as_str()), to_yaml(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::settings::NotificationMode;

    #[test]
    fn erkennt_eine_ntfy_adresse_wie_der_regulare_ausdruck() {
        assert!(is_ntfy_server("https://ntfy.sh"));
        assert!(is_ntfy_server("http://x"));
        assert!(!is_ntfy_server("ntfy.sh"));
        assert!(!is_ntfy_server("https://"));
        assert!(!is_ntfy_server("https://mit leerzeichen"));
    }

    #[test]
    fn ubersetzt_json_werte_nach_yaml() {
        let value = serde_json::json!({ "a": 1, "b": true, "c": null, "d": [1.5, "x"] });
        let yaml = to_yaml(&value);
        let back: Value = serde_yaml_ng::from_str(&serde_yaml_ng::to_string(&yaml).unwrap())
            .map(|value: Yaml| serde_json::to_value(value).unwrap())
            .unwrap();
        assert_eq!(back, value);
    }

    /// Leerer Benutzerordner
    fn home() -> (tempfile::TempDir, NexusHome) {
        let dir = tempfile::tempdir().unwrap();
        let home = NexusHome::at(dir.path());
        (dir, home)
    }

    #[test]
    fn ubernimmt_die_setup_dialoge_teilweise() {
        let (_dir, home) = home();
        let merged = merge_user_settings(
            UserSettings::default(),
            &serde_json::json!({ "setupPrompts": { "migrationDismissed": [4, 2, 4] } }),
            &home,
        )
        .unwrap();
        assert_eq!(merged.setup_prompts.migration_dismissed, vec![2, 4]);
        assert!(!merged.setup_prompts.plugin_dismissed);
    }

    #[test]
    fn lehnt_ungueltige_setup_dialoge_ab() {
        let (_dir, home) = home();
        for body in [
            serde_json::json!({ "setupPrompts": true }),
            serde_json::json!({ "setupPrompts": { "pluginDismissed": "ja" } }),
            serde_json::json!({ "setupPrompts": { "migrationDismissed": [0] } }),
        ] {
            assert!(
                merge_user_settings(UserSettings::default(), &body, &home).is_err(),
                "{body}"
            );
        }
    }

    #[test]
    fn ubernimmt_ein_installiertes_theme_und_lehnt_ein_unbekanntes_ab() {
        let (dir, home) = home();
        let theme_dir = dir.path().join("themes").join("ocean");
        std::fs::create_dir_all(&theme_dir).unwrap();
        std::fs::write(theme_dir.join("manifest.json"), "{}").unwrap();

        let merged = merge_user_settings(
            UserSettings::default(),
            &serde_json::json!({ "theme": "ocean" }),
            &home,
        )
        .unwrap();
        assert_eq!(merged.theme, ThemeMode::from("ocean"));
        for theme in [serde_json::json!("lila"), serde_json::json!(5)] {
            assert!(merge_user_settings(
                UserSettings::default(),
                &serde_json::json!({ "theme": theme }),
                &home,
            )
            .is_err());
        }
    }

    #[test]
    fn ubernimmt_nur_die_angegebenen_felder() {
        let (_dir, home) = home();
        let merged = merge_user_settings(
            UserSettings::default(),
            &serde_json::json!({ "theme": "dark", "ntfyToken": "  tk  " }),
            &home,
        )
        .unwrap();
        assert_eq!(merged.theme, ThemeMode::from("dark"));
        assert_eq!(merged.ntfy_token, "tk");
        assert_eq!(merged.notifications, NotificationMode::Off);
    }
}
