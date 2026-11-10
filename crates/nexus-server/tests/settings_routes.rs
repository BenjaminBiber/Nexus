//! Routengruppe `settings`; portiert aus `packages/server/tests/unit/ProjectRoutes.test.ts`,
//! `SettingsRoutes.test.ts`, `UserSettingsRoutes.test.ts` und `BoardConfigRoutes.test.ts`.
//!
//! Die Fälle der beiden Testmeldungs-Routen (`POST /api/user-settings/test-notification` und
//! `…/test-ntfy`) stehen in `notification_routes.rs`: sie gehören zur Gruppe
//! `notifications`. Hier steht nur, wer sie beantwortet.

mod common;

use common::{TempRepo, TestServer};
use serde_json::{json, Value};

/// Eingabe einer SQL-Server-Konfig, wie die Oberfläche sie schickt
fn mssql_input() -> Value {
    json!({
        "server": "127.0.0.1", "port": 1, "database": "AiHarness", "user": "harness",
        "password": "geheim", "encrypt": false, "trustServerCertificate": true,
        "userId": "11111111-1111-1111-1111-111111111111", "userName": "Florian",
    })
}

/// Eingabe einer SQLite-Konfig
fn sqlite_input() -> Value {
    json!({
        "provider": "sqlite",
        "userId": "11111111-1111-1111-1111-111111111111",
        "userName": "Florian",
    })
}

/// Fügt der Eingabe weitere Felder hinzu (wie `{ ...INPUT, … }` in TypeScript)
fn with(mut base: Value, extra: Value) -> Value {
    let object = base.as_object_mut().expect("Objekt");
    for (key, value) in extra.as_object().expect("Objekt") {
        object.insert(key.clone(), value.clone());
    }
    base
}

// ---------------------------------------------------------------- Projekte

#[tokio::test]
async fn liefert_zunachst_eine_leere_liste() {
    let server = TestServer::start();
    let reply = server.get("/api/projects").await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.json(), Some(json!([])));
}

#[tokio::test]
async fn meldet_ein_projekt_an_201_neu_200_bekannt() {
    let server = TestServer::start();
    let (repo, _id) = server.add_project();
    let known = server
        .post("/api/projects", &json!({ "path": repo.path() }).to_string())
        .await;
    assert_eq!(known.status, 200);

    let other = TempRepo::new();
    nexus_core::HarnessProject::init(other.path()).expect("Projekt einrichten");
    let created = server
        .post(
            "/api/projects",
            &json!({ "path": other.path() }).to_string(),
        )
        .await;
    assert_eq!(created.status, 201);
    let body = created.json().expect("JSON");
    assert_eq!(body["name"], "Mein Projekt ä");
    assert_eq!(body["reachable"], true);

    let list = server.get("/api/projects").await.json().expect("JSON");
    assert_eq!(list.as_array().expect("Liste").len(), 2);
}

#[tokio::test]
async fn lehnt_fehlenden_relativen_und_nicht_eingerichteten_pfad_mit_422_ab() {
    let server = TestServer::start();
    assert_eq!(server.post("/api/projects", "{}").await.status, 422);
    assert_eq!(
        server
            .post("/api/projects", r#"{"path":"relativ/pfad"}"#)
            .await
            .status,
        422
    );
    let bare = TempRepo::new();
    let reply = server
        .post("/api/projects", &json!({ "path": bare.path() }).to_string())
        .await;
    assert_eq!(reply.status, 422);
    let message = reply.json().expect("JSON")["message"].to_string();
    assert!(message.contains("nexus init"), "{message}");
}

#[tokio::test]
async fn markiert_ein_projekt_mit_geloschtem_ordner_als_nicht_erreichbar() {
    let server = TestServer::start();
    let (repo, _id) = server.add_project();
    repo.remove();
    let list = server.get("/api/projects").await.json().expect("JSON");
    assert_eq!(list[0]["reachable"], false);
}

#[tokio::test]
async fn lehnt_eine_anmeldung_ohne_pflichtkopf_mit_403_ab() {
    let server = TestServer::start();
    let repo = TempRepo::new();
    nexus_core::HarnessProject::init(repo.path()).expect("Projekt einrichten");
    let reply = server
        .request(
            "POST",
            "/api/projects",
            Some(&json!({ "path": repo.path() }).to_string()),
            &[("x-harness", None)],
        )
        .await;
    assert_eq!(reply.status, 403);
    assert_eq!(reply.code().as_deref(), Some("header-missing"));
    // Nichts angemeldet
    assert_eq!(server.get("/api/projects").await.json(), Some(json!([])));
}

// ------------------------------------------------- Einstellungen des Projekts

#[tokio::test]
async fn liefert_projekt_und_token_aber_keine_spalten_und_keine_sicherung() {
    let server = TestServer::start();
    let (_repo, id) = server.add_project();
    let reply = server.get(&format!("/api/projects/{id}/settings")).await;
    assert_eq!(reply.status, 200);
    let body = reply.json().expect("JSON");
    assert_eq!(body["project"]["name"], "Mein Projekt ä");
    let token = body["token"].as_str().expect("Token");
    assert_eq!(token.len(), 64, "{token}");
    assert!(token.chars().all(|zeichen| zeichen.is_ascii_hexdigit()));
    let mut keys: Vec<&String> = body.as_object().expect("Objekt").keys().collect();
    keys.sort();
    assert_eq!(keys, ["project", "token"]);
}

#[tokio::test]
async fn kennt_weder_export_noch_sicherung() {
    let server = TestServer::start();
    let (_repo, id) = server.add_project();
    let base = format!("/api/projects/{id}");
    assert_eq!(
        server
            .post(&format!("{base}/export"), r#"{"target":"/x"}"#)
            .await
            .status,
        404
    );
    assert_eq!(
        server.post(&format!("{base}/backup"), "{}").await.status,
        404
    );
}

#[tokio::test]
async fn meldet_ein_unbekanntes_projekt_mit_404() {
    let server = TestServer::start();
    let reply = server.get("/api/projects/gibtsnicht/settings").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.code().as_deref(), Some("project-not-found"));
}

// ------------------------------------------------------ Benutzereinstellungen

#[tokio::test]
async fn liefert_system_als_standard_und_speichert_das_theme() {
    let server = TestServer::start();
    assert_eq!(
        server.get("/api/user-settings").await.json().expect("JSON")["theme"],
        "system"
    );
    let saved = server
        .put("/api/user-settings", r#"{"theme":"dark"}"#)
        .await;
    assert_eq!(saved.status, 200);
    assert_eq!(
        saved.json(),
        Some(json!({
            "notifications": "off", "heroImages": false, "theme": "dark", "claudeUsage": false,
            "compactCards": false, "hideNotNeeded": false, "hideInPlanning": false,
            "ntfy": false, "ntfyServer": "https://ntfy.sh", "ntfyTopic": "", "ntfyToken": "",
            "setupPrompts": { "pluginDismissed": false, "migrationDismissed": [] },
        }))
    );
    assert_eq!(
        server.get("/api/user-settings").await.json().expect("JSON")["theme"],
        "dark"
    );
}

#[tokio::test]
async fn lehnt_ein_unbekanntes_theme_ab() {
    let server = TestServer::start();
    let reply = server
        .put("/api/user-settings", r#"{"theme":"lila"}"#)
        .await;
    assert_eq!(reply.status, 422);
    assert_eq!(
        reply.json().expect("JSON")["message"],
        "theme muss ein eingebautes (system, light, dark) oder installiertes Theme sein."
    );
}

#[tokio::test]
async fn speichert_die_kompakten_ticket_kacheln() {
    let server = TestServer::start();
    assert_eq!(
        server.get("/api/user-settings").await.json().expect("JSON")["compactCards"],
        false
    );
    let saved = server
        .put("/api/user-settings", r#"{"compactCards":true}"#)
        .await;
    assert_eq!(saved.status, 200);
    assert_eq!(saved.json().expect("JSON")["compactCards"], true);
    assert_eq!(
        server.get("/api/user-settings").await.json().expect("JSON")["compactCards"],
        true
    );
}

#[tokio::test]
async fn lehnt_compact_cards_ohne_wahrheitswert_ab() {
    let server = TestServer::start();
    let reply = server
        .put("/api/user-settings", r#"{"compactCards":"ja"}"#)
        .await;
    assert_eq!(reply.status, 422);
    assert_eq!(
        reply.json().expect("JSON")["message"],
        "compactCards muss true oder false sein."
    );
}

#[tokio::test]
async fn speichert_die_ntfy_einstellungen() {
    let server = TestServer::start();
    let saved = server
        .put(
            "/api/user-settings",
            r#"{"ntfy":true,"ntfyServer":" https://push.example ","ntfyTopic":"harness-x","ntfyToken":"tk_1"}"#,
        )
        .await;
    assert_eq!(saved.status, 200);
    let body = saved.json().expect("JSON");
    assert_eq!(body["ntfy"], true);
    assert_eq!(body["ntfyServer"], "https://push.example");
    assert_eq!(body["ntfyTopic"], "harness-x");
    assert_eq!(body["ntfyToken"], "tk_1");
}

#[tokio::test]
async fn lehnt_ein_ntfy_thema_mit_ungultigen_zeichen_und_einen_server_ohne_http_ab() {
    let server = TestServer::start();
    assert_eq!(
        server
            .put("/api/user-settings", r#"{"ntfyTopic":"mein thema"}"#)
            .await
            .status,
        422
    );
    assert_eq!(
        server
            .put("/api/user-settings", r#"{"ntfyServer":"ntfy.sh"}"#)
            .await
            .status,
        422
    );
}

// ------------------------------------------------------------- Board-Konfig

#[tokio::test]
async fn liefert_ohne_datei_standardwerte_und_eine_vorschlags_guid() {
    let server = TestServer::start();
    let reply = server.get("/api/board-config").await;
    assert_eq!(reply.status, 200);
    let body = reply.json().expect("JSON");
    assert_eq!(body["configured"], false);
    assert_eq!(body["port"], 1433);
    assert_eq!(body["database"], "AiHarness");
    assert_eq!(body["hasPassword"], false);
    assert_eq!(body["provider"], "mssql");
    assert_eq!(
        body["dbFile"],
        server.home.board_db_file().display().to_string()
    );
    assert_eq!(
        body["file"],
        server.home.board_config_file().display().to_string()
    );
    let user_id = body["userId"].as_str().expect("userId");
    assert_eq!(user_id.len(), 36, "{user_id}");
    assert!(nexus_core::board::config::is_guid(user_id), "{user_id}");
}

#[tokio::test]
async fn speichert_zeigt_das_passwort_nie_und_behalt_es_bei_leerem_feld() {
    let server = TestServer::start();
    let saved = server
        .put("/api/board-config", &mssql_input().to_string())
        .await;
    assert_eq!(saved.status, 200);
    let body = saved.json().expect("JSON");
    assert_eq!(body["configured"], true);
    assert_eq!(body["server"], "127.0.0.1");
    assert_eq!(body["hasPassword"], true);
    assert!(body.get("password").is_none(), "{body}");
    assert_eq!(body["provider"], "mssql");
    assert_eq!(
        body["file"],
        server.home.board_config_file().display().to_string()
    );

    let again = with(mssql_input(), json!({ "password": "", "userName": "Flo" }));
    assert_eq!(
        server
            .put("/api/board-config", &again.to_string())
            .await
            .status,
        200
    );
    let yaml = std::fs::read_to_string(server.home.board_config_file()).expect("board.yaml");
    assert!(yaml.contains("password: geheim"), "{yaml}");
    assert!(yaml.contains("userName: Flo"), "{yaml}");
}

#[tokio::test]
async fn lehnt_eine_ungultige_konfig_mit_422_ab() {
    let server = TestServer::start();
    let reply = server
        .put(
            "/api/board-config",
            &with(mssql_input(), json!({ "userId": "kaputt" })).to_string(),
        )
        .await;
    assert_eq!(reply.status, 422);
    let message = reply.json().expect("JSON")["message"].to_string();
    assert!(message.contains("GUID"), "{message}");
}

#[tokio::test]
async fn meldet_beim_verbindungstest_einen_nicht_erreichbaren_server_ohne_zu_speichern() {
    let server = TestServer::start();
    let reply = server
        .post("/api/board-config/test", &mssql_input().to_string())
        .await;
    assert_eq!(reply.status, 200);
    let body = reply.json().expect("JSON");
    assert_eq!(body["ok"], false);
    let message = body["message"].as_str().expect("Meldung");
    assert!(message.contains("nicht erreichbar"), "{message}");
    assert_eq!(
        server.get("/api/board-config").await.json().expect("JSON")["configured"],
        false
    );
}

#[tokio::test]
async fn speichert_eine_sqlite_konfig_und_liest_sie_wieder() {
    let server = TestServer::start();
    let db_file = server.home.dir().join("daten").join("b.db");
    let db_file = db_file.display().to_string();
    let saved = server
        .put(
            "/api/board-config",
            &with(sqlite_input(), json!({ "dbFile": db_file })).to_string(),
        )
        .await;
    assert_eq!(saved.status, 200);
    let body = saved.json().expect("JSON");
    assert_eq!(body["configured"], true);
    assert_eq!(body["provider"], "sqlite");
    assert_eq!(body["dbFile"], db_file);
    assert_eq!(
        body["file"],
        server.home.board_config_file().display().to_string()
    );

    let read = server.get("/api/board-config").await.json().expect("JSON");
    assert_eq!(read["configured"], true);
    assert_eq!(read["provider"], "sqlite");
    assert_eq!(read["dbFile"], db_file);
    let yaml = std::fs::read_to_string(server.home.board_config_file()).expect("board.yaml");
    assert!(yaml.contains("provider: sqlite"), "{yaml}");
}

#[tokio::test]
async fn ubernimmt_ein_mitgeschicktes_file_nicht_als_datenbankpfad() {
    let server = TestServer::start();
    let saved = server
        .put(
            "/api/board-config",
            &with(sqlite_input(), json!({ "file": "X:/falsch.yaml" })).to_string(),
        )
        .await;
    assert_eq!(saved.status, 200);
    assert_eq!(
        saved.json().expect("JSON")["dbFile"],
        server.home.board_db_file().display().to_string()
    );
}

#[tokio::test]
async fn lehnt_ein_leeres_db_file_bei_sqlite_mit_422_ab() {
    let server = TestServer::start();
    let reply = server
        .put(
            "/api/board-config",
            &with(sqlite_input(), json!({ "dbFile": "" })).to_string(),
        )
        .await;
    assert_eq!(reply.status, 422);
    let message = reply.json().expect("JSON")["message"].to_string();
    assert!(message.contains("file"), "{message}");
}

#[tokio::test]
async fn behalt_sql_server_felder_beim_wechsel_auf_sqlite() {
    let server = TestServer::start();
    server
        .put("/api/board-config", &mssql_input().to_string())
        .await;
    let body = with(
        with(mssql_input(), sqlite_input()),
        json!({ "password": "" }),
    );
    let saved = server.put("/api/board-config", &body.to_string()).await;
    let body = saved.json().expect("JSON");
    assert_eq!(body["provider"], "sqlite");
    assert_eq!(body["server"], "127.0.0.1");
    assert_eq!(body["hasPassword"], true);
}

#[tokio::test]
async fn meldet_beim_verbindungstest_gegen_eine_neue_temp_datei_erfolg_ohne_zu_speichern() {
    let server = TestServer::start();
    let db_file = server.home.dir().join("probe").join("board.db");
    let reply = server
        .post(
            "/api/board-config/test",
            &with(
                sqlite_input(),
                json!({ "dbFile": db_file.display().to_string() }),
            )
            .to_string(),
        )
        .await;
    let body = reply.json().expect("JSON");
    assert_eq!(body["ok"], true, "{body}");
    let message = body["message"].as_str().expect("Meldung");
    assert!(message.contains("Datei geöffnet"), "{message}");
    assert!(db_file.exists(), "{}", db_file.display());
    assert_eq!(
        server.get("/api/board-config").await.json().expect("JSON")["configured"],
        false
    );
}

#[tokio::test]
async fn meldet_beim_verbindungstest_auf_einen_ordner_statt_einer_datei_einen_fehler() {
    let server = TestServer::start();
    std::fs::create_dir_all(server.home.dir()).expect("Benutzerordner");
    let reply = server
        .post(
            "/api/board-config/test",
            &with(
                sqlite_input(),
                json!({ "dbFile": server.home.dir().display().to_string() }),
            )
            .to_string(),
        )
        .await;
    let body = reply.json().expect("JSON");
    assert_eq!(body["ok"], false, "{body}");
    assert_ne!(body["message"], "");
}

#[tokio::test]
async fn repariert_eine_kaputte_board_yaml_per_put_get_meldet_den_fehler_vorher_weiter() {
    let server = TestServer::start();
    std::fs::create_dir_all(server.home.dir()).expect("Benutzerordner");
    std::fs::write(server.home.board_config_file(), "server: s\n").expect("board.yaml");
    assert_eq!(server.get("/api/board-config").await.status, 422);

    let saved = server
        .put("/api/board-config", &mssql_input().to_string())
        .await;
    assert_eq!(saved.status, 200);
    let body = saved.json().expect("JSON");
    assert_eq!(body["configured"], true);
    assert_eq!(body["server"], "127.0.0.1");
    assert_eq!(body["hasPassword"], true);

    let read = server.get("/api/board-config").await.json().expect("JSON");
    assert_eq!(read["configured"], true);
    assert_eq!(read["userName"], "Florian");
}
