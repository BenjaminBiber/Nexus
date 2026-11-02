//! Die Routengruppe `/api/accounts`; portiert aus
//! `packages/server/tests/unit/AccountRoutes.test.ts` des Referenzbranches.

mod common;

use nexus_core::dispatch::DispatchConfigStore;
use serde_json::{json, Value};

use common::{TestServer, TestServerOptions};

/// Ein Server, dessen Account-Liste nie echte Profile des Rechners vorschlägt
fn server() -> TestServer {
    TestServer::with(TestServerOptions::default())
}

/// Die Antwort als JSON
fn body(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON der Antwort")
}

#[tokio::test]
async fn listet_den_standard_account_und_ein_zweiter_aktiver_macht_den_dispatcher_mehrfaehig() {
    let server = server();

    let first = server.get("/api/accounts").await;
    assert_eq!(first.status, 200);
    let list = body(&first.text);
    assert_eq!(list["multi"], false);
    assert_eq!(
        list["accountsDir"],
        json!(server.home.accounts_dir().to_string_lossy())
    );
    assert_eq!(list["accounts"][0]["id"], "standard");
    assert_eq!(list["accounts"][0]["configDir"], Value::Null);
    assert_eq!(list["accounts"][0]["enabled"], true);
    assert_eq!(list["accounts"][0]["references"], json!([]));

    let created = server
        .post("/api/accounts", r#"{"name":"Max privat"}"#)
        .await;
    assert_eq!(created.status, 200);
    let list = body(&created.text);
    assert_eq!(list["multi"], true);
    let account = &list["accounts"][1];
    assert_eq!(account["id"], "max-privat");
    assert_eq!(
        account["configDir"],
        json!(server
            .home
            .accounts_dir()
            .join("max-privat")
            .to_string_lossy())
    );
    assert_eq!(account["loggedIn"], false);
    assert_eq!(account["plugin"], false);
    assert!(server.home.accounts_dir().join("max-privat").is_dir());

    let names = server.get("/api/accounts/names").await;
    assert_eq!(
        body(&names.text),
        json!({ "accounts": [
            { "id": "standard", "name": "Standard" },
            { "id": "max-privat", "name": "Max privat" },
        ]})
    );
}

#[tokio::test]
async fn benennt_um_schaltet_ab_und_lehnt_ungueltige_eingaben_ab() {
    let server = server();
    server.post("/api/accounts", r#"{"name":"Pro"}"#).await;

    let patched = server
        .patch(
            "/api/accounts/pro",
            r#"{"name":"Pro Firma","enabled":false}"#,
        )
        .await;
    let list = body(&patched.text);
    assert_eq!(list["accounts"][1]["name"], "Pro Firma");
    assert_eq!(list["accounts"][1]["enabled"], false);
    assert_eq!(list["multi"], false);

    let fallback = server
        .patch("/api/accounts/pro", r#"{"fallback":true}"#)
        .await;
    assert_eq!(body(&fallback.text)["accounts"][1]["fallback"], true);

    let limits = server
        .patch(
            "/api/accounts/pro",
            r#"{"limitFiveHour":50,"limitWeek":null}"#,
        )
        .await;
    let list = body(&limits.text);
    assert_eq!(list["accounts"][1]["limitFiveHour"], 50);
    assert_eq!(list["accounts"][1]["limitWeek"], Value::Null);

    for (url, payload) in [
        ("/api/accounts/pro", r#"{"fallback":"ja"}"#),
        ("/api/accounts/pro", r#"{"enabled":"ja"}"#),
        ("/api/accounts/pro", r#"{"limitWeek":150}"#),
        ("/api/accounts/pro", r#"{"limitWeek":"50"}"#),
    ] {
        assert_eq!(server.patch(url, payload).await.status, 422, "{payload}");
    }
    assert_eq!(
        server.post("/api/accounts", r#"{"name":""}"#).await.status,
        422
    );
    assert_eq!(
        server
            .patch("/api/accounts/gibt-es-nicht", r#"{"enabled":true}"#)
            .await
            .status,
        404
    );
    assert_eq!(server.delete("/api/accounts/standard").await.status, 422);
}

#[tokio::test]
async fn die_dispatcher_einstellung_lehnt_unbekannte_accounts_ab_und_verweise_verschwinden_mit() {
    let server = server();
    let raw = json!({
        "enabled": false,
        "profiles": { "standard": { "model": "opus" }, "schwer": { "model": "opus", "accounts": ["max"] } },
        "defaultProfile": "standard",
    })
    .to_string();

    let rejected = server.put("/api/dispatch/config", &raw).await;
    assert_eq!(rejected.status, 422);
    assert!(
        body(&rejected.text)["message"]
            .as_str()
            .expect("Meldung")
            .contains("Unbekannte Accounts: „max“ (Profil schwer)"),
        "{}",
        rejected.text
    );

    server.post("/api/accounts", r#"{"name":"Max"}"#).await;
    assert_eq!(server.put("/api/dispatch/config", &raw).await.status, 200);
    let list = body(&server.get("/api/accounts").await.text);
    assert_eq!(list["accounts"][1]["references"], json!(["Profil schwer"]));

    let removed = server.delete("/api/accounts/max").await;
    assert_eq!(removed.status, 200);
    let ids: Vec<String> = body(&removed.text)["accounts"]
        .as_array()
        .expect("Liste")
        .iter()
        .map(|account| account["id"].as_str().expect("ID").to_string())
        .collect();
    assert_eq!(ids, vec!["standard".to_string()]);
    let read = DispatchConfigStore::new(server.home.clone())
        .read()
        .expect("Einstellung");
    assert_eq!(read.raw["profiles"]["schwer"], json!({ "model": "opus" }));
}

#[tokio::test]
async fn uebernimmt_einen_vorhandenen_ordner_und_loescht_beim_entfernen_nur_eigene() {
    let server = server();
    let outside = server.home.dir().join("fremd");
    std::fs::create_dir_all(&outside).expect("fremder Ordner");
    let payload = json!({ "name": "Fremd", "configDir": outside.to_string_lossy() }).to_string();
    assert_eq!(server.post("/api/accounts", &payload).await.status, 200);

    assert_eq!(
        server
            .delete("/api/accounts/fremd?deleteDir=1")
            .await
            .status,
        422
    );
    assert_eq!(server.delete("/api/accounts/fremd").await.status, 200);
    assert!(outside.is_dir());
}

#[tokio::test]
async fn hebt_die_pause_eines_accounts_auf() {
    let server = server();
    server.post("/api/accounts", r#"{"name":"Pro"}"#).await;
    let until = (chrono::Utc::now() + chrono::Duration::minutes(60)).to_rfc3339();
    server
        .state
        .dispatch
        .stores
        .state
        .update(|state| {
            state.account_pauses.insert("pro".into(), until);
        })
        .await
        .expect("Pause setzen");

    let resumed = server.post("/api/accounts/pro/resume", "{}").await;

    assert_eq!(resumed.status, 200);
    assert_eq!(
        body(&resumed.text)["accounts"][1]["pausedUntil"],
        Value::Null
    );
    assert!(server
        .state
        .dispatch
        .account_pauses()
        .expect("Pausen")
        .is_empty());
}

#[tokio::test]
async fn schlaegt_mcacc_profile_vor_die_noch_keinem_account_gehoeren() {
    let dir = tempfile::Builder::new()
        .prefix("nexus-mcacc-")
        .tempdir()
        .expect("Ordner");
    std::fs::create_dir_all(dir.path().join("arbeit").join("claude")).expect("Profil");
    // Ein Ordner ohne `claude` darin zählt nicht
    std::fs::create_dir_all(dir.path().join("leer")).expect("leeres Profil");
    let server = TestServer::with(TestServerOptions {
        mcacc_dir: Some(dir.path().to_path_buf()),
        ..TestServerOptions::default()
    });

    let list = body(&server.get("/api/accounts").await.text);

    assert_eq!(
        list["suggestions"],
        json!([dir.path().join("arbeit").join("claude").to_string_lossy()])
    );

    // Gehört der Ordner einem Account, verschwindet der Vorschlag
    let payload = json!({
        "name": "Arbeit",
        "configDir": dir.path().join("arbeit").join("claude").to_string_lossy(),
    })
    .to_string();
    let created = server.post("/api/accounts", &payload).await;
    assert_eq!(created.status, 200);
    assert_eq!(body(&created.text)["suggestions"], json!([]));
}
