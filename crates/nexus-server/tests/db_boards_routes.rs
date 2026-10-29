//! Die Routengruppe `db-boards`; portiert aus
//! `packages/server/tests/unit/DbBoardRoutes.test.ts`.
//!
//! Der Fall aus `packages/server/tests/live/dbboard.test.ts` (eine Änderung am beobachteten
//! Board kommt als Ereignis `dbboard` am Ereignisstrom an) steht am Ende als `#[ignore]`: er
//! prüft den Ereignisstrom `/api/live`, und der gehört zur Routengruppe `events`.

mod common;

use std::sync::Arc;

use common::{BoardBackendWithServices, TestServer, TestServerOptions};
use nexus_core::board::board_service::BoardListFilter;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_core::board::database::{BoardDatabase as _, NewBoard};
use nexus_core::board::repo_paths::BoardRepoStore as _;
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use nexus_core::board::types::BoardUser;
use nexus_core::tx;
use nexus_platform::{FakeFolderPicker, Platform};
use nexus_server::groups::{RouteGroup, RouteGroups};
use serde_json::{json, Value};

/// Legt (falls nötig) Projekt ARC und ein Board an und liefert dessen Nummer
async fn seed(server: &TestServer, name: &str) -> i64 {
    let projects = server.get("/api/db/projects").await;
    if projects
        .json()
        .and_then(|list| Some(list.as_array()?.is_empty()))
        == Some(true)
    {
        server
            .post("/api/db/projects", r#"{"key":"ARC","name":"Arches"}"#)
            .await;
    }
    let created = server
        .post(
            "/api/db/boards",
            &json!({ "project": "ARC", "name": name }).to_string(),
        )
        .await;
    created
        .json()
        .and_then(|board| board.get("id")?.as_i64())
        .expect("Nummer des Boards")
}

/// Der Körper einer Antwort als JSON
fn body(reply: &common::Reply) -> Value {
    reply
        .json()
        .unwrap_or_else(|| panic!("kein JSON: {}", reply.text))
}

#[tokio::test]
async fn legt_projekte_an_listet_und_benennt_sie_um() {
    let (server, _backend) = TestServer::start_with_board().await;
    let created = server
        .post("/api/db/projects", r#"{"key":"arc","name":"Arches"}"#)
        .await;
    assert_eq!(created.status, 201);
    assert_eq!(body(&created)["key"], "ARC");
    assert_eq!(body(&created)["name"], "Arches");

    let listed = server.get("/api/db/projects").await;
    assert_eq!(body(&listed)[0]["key"], "ARC");
    assert_eq!(body(&listed).as_array().expect("Liste").len(), 1);

    let renamed = server
        .patch("/api/db/projects/ARC", r#"{"name":"Arches 2"}"#)
        .await;
    assert_eq!(body(&renamed)["key"], "ARC");
    assert_eq!(body(&renamed)["name"], "Arches 2");

    let twice = server
        .post("/api/db/projects", r#"{"key":"ARC","name":"Doppelt"}"#)
        .await;
    assert_eq!(twice.code().as_deref(), Some("project-exists"));
    let unknown = server
        .patch("/api/db/projects/XYZ", r#"{"name":"x"}"#)
        .await;
    assert_eq!(unknown.status, 404);
    let missing = server.post("/api/db/projects", r#"{"key":"NEU"}"#).await;
    assert_eq!(missing.status, 422);
    assert_eq!(missing.code().as_deref(), Some("input-invalid"));
}

#[tokio::test]
async fn legt_boards_an_filtert_nach_projekt_und_andert_name_und_repo_pfad() {
    let (server, _backend) = TestServer::start_with_board().await;
    let id = seed(&server, "Sprint").await;
    server
        .post("/api/db/projects", r#"{"key":"OTH","name":"Other"}"#)
        .await;
    let foreign = server
        .post("/api/db/boards", r#"{"project":"OTH","name":"Fremd"}"#)
        .await;
    assert_eq!(foreign.status, 201);

    let only_arc = server.get("/api/db/boards?project=ARC").await;
    let list = body(&only_arc);
    let list = list.as_array().expect("Liste");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], id);
    assert_eq!(list[0]["name"], "Sprint");
    assert_eq!(list[0]["projectKey"], "ARC");
    assert_eq!(list[0]["ownerName"], "Florian");
    assert_eq!(list[0]["openTickets"], 0);
    assert_eq!(
        body(&server.get("/api/db/boards").await)
            .as_array()
            .expect("Liste")
            .len(),
        2
    );

    let changed = server
        .patch(
            &format!("/api/db/boards/{id}"),
            r#"{"name":"Sprint 2","repoPath":"C:/repo"}"#,
        )
        .await;
    assert_eq!(body(&changed)["name"], "Sprint 2");
    assert_eq!(body(&changed)["repoPath"], "C:/repo");
    assert_eq!(body(&changed)["projectKey"], "ARC");

    let cleared = server
        .patch(&format!("/api/db/boards/{id}"), r#"{"repoPath":null}"#)
        .await;
    assert_eq!(body(&cleared)["repoPath"], Value::Null);

    let nothing = server.patch(&format!("/api/db/boards/{id}"), "{}").await;
    assert_eq!(nothing.code().as_deref(), Some("nothing-changed"));
    assert_eq!(body(&nothing)["message"], "Keine Änderung angegeben.");
    assert_eq!(
        server
            .patch("/api/db/boards/999", r#"{"name":"x"}"#)
            .await
            .status,
        404
    );
    assert_eq!(
        server
            .get("/api/db/boards/abc/view")
            .await
            .code()
            .as_deref(),
        Some("id-invalid")
    );
}

#[tokio::test]
async fn speichert_mein_repo_pfad_lokal_und_zeigt_keine_fremden_db_pfade() {
    let (server, backend) = TestServer::start_with_board().await;
    let id = seed(&server, "Sprint").await;
    let saved = server
        .patch(&format!("/api/db/boards/{id}"), r#"{"repoPath":"C:/mein"}"#)
        .await;
    assert_eq!(body(&saved)["repoPath"], "C:/mein");
    assert_eq!(body(&saved)["repoSource"], "local");

    // Der Pfad steht beim Benutzer, nicht in der gemeinsamen Datenbank
    let id32 = id as i32;
    assert_eq!(
        backend
            .repos
            .read()
            .expect("Repo-Pfade lesen")
            .get(&i64::from(id32))
            .and_then(|path| path.as_deref()),
        Some("C:/mein")
    );
    let in_db = tx!(backend.services.db, |u| { u.boards().by_id(id32).await })
        .await
        .expect("Board lesen");
    assert_eq!(in_db.expect("Board").repo_path, None);

    // Ein Board eines Kollegen mit Pfad in der Datenbank
    let project_id = tx!(backend.services.db, |u| {
        Ok(u.projects().by_key("ARC").await?.expect("Projekt").id)
    })
    .await
    .expect("Projekt lesen");
    let colleague = BoardUser {
        id: "22222222-2222-2222-2222-222222222222".into(),
        name: "Kollege".into(),
    };
    let new_board = NewBoard {
        project_id,
        owner_id: colleague.id.clone(),
        name: "Fremd".into(),
        repo_path: Some("D:/kollege".into()),
        created_at: "2026-10-01T08:00:00.000Z".into(),
    };
    let foreign = tx!(backend.services.db, [colleague, new_board], |u| {
        u.users().upsert(&colleague).await?;
        u.boards().insert(&new_board).await
    })
    .await
    .expect("fremdes Board anlegen");

    let listed = server.get("/api/db/boards").await;
    let found = body(&listed)
        .as_array()
        .expect("Liste")
        .iter()
        .find(|board| board["id"] == foreign.id)
        .cloned()
        .expect("fremdes Board");
    assert_eq!(found["repoPath"], Value::Null);
    assert_eq!(found["repoSource"], Value::Null);
    assert!(!listed.text.contains("D:/kollege"), "{}", listed.text);
}

#[tokio::test]
async fn meldet_benutzer_und_aktives_board_und_aktiviert_ein_board_in_board_yaml() {
    let (server, _backend) = TestServer::start_with_board().await;
    let id = seed(&server, "Sprint").await;
    let empty = server.get("/api/db/active").await;
    assert_eq!(
        body(&empty),
        json!({
            "user": { "id": "11111111-1111-1111-1111-111111111111", "name": "Florian" },
            "boardId": Value::Null,
            "board": Value::Null,
        })
    );

    let activated = server
        .post(&format!("/api/db/boards/{id}/activate"), "{}")
        .await;
    assert_eq!(activated.status, 200);
    assert_eq!(body(&activated)["boardId"], id);
    assert_eq!(body(&activated)["board"]["name"], "Sprint");
    let config = std::fs::read_to_string(BoardConfigStore::new(server.home.clone()).file())
        .expect("board.yaml");
    assert!(config.contains(&format!("activeBoard: {id}")), "{config}");
    assert_eq!(body(&server.get("/api/db/active").await)["boardId"], id);
    assert_eq!(
        server
            .post("/api/db/boards/999/activate", "{}")
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn loscht_ein_board_mit_offenen_tickets_nur_mit_delete_open_und_vergisst_es_als_aktives_board(
) {
    let (server, backend) = TestServer::start_with_board().await;
    let id = seed(&server, "Sprint").await;
    server
        .post(&format!("/api/db/boards/{id}/activate"), "{}")
        .await;
    backend
        .services
        .tickets
        .create(
            id as i32,
            &NewTicketInput::titled("Offen"),
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");

    let refused = server.delete(&format!("/api/db/boards/{id}")).await;
    assert_eq!(refused.status, 422);
    assert_eq!(refused.code().as_deref(), Some("board-has-open-tickets"));

    let deleted = server
        .delete(&format!("/api/db/boards/{id}?deleteOpen=true"))
        .await;
    assert_eq!(deleted.status, 200);
    assert_eq!(body(&deleted), json!({ "deletedTickets": ["ARC-0001"] }));
    assert_eq!(
        body(&server.get("/api/db/active").await)["boardId"],
        Value::Null
    );
    let config = std::fs::read_to_string(BoardConfigStore::new(server.home.clone()).file())
        .expect("board.yaml");
    assert!(config.contains("activeBoard: null"), "{config}");
}

#[tokio::test]
async fn loscht_bei_kaputter_board_yaml_nichts_und_antwortet_mit_503() {
    let (server, backend) = TestServer::start_with_board().await;
    let id = seed(&server, "Sprint").await;
    std::fs::write(
        BoardConfigStore::new(server.home.clone()).file(),
        "server: [kaputt",
    )
    .expect("board.yaml beschädigen");

    let reply = server.delete(&format!("/api/db/boards/{id}")).await;
    assert_eq!(reply.status, 503);
    assert_eq!(reply.code().as_deref(), Some("board-config-broken"));
    let boards = backend
        .services
        .boards
        .list(&BoardListFilter::default())
        .await
        .expect("Boards lesen");
    assert!(boards.iter().any(|board| board.board.id == id as i32));
}

#[tokio::test]
async fn liefert_die_board_ansicht_mit_allen_spalten_und_kommentarzahl() {
    let (server, backend) = TestServer::start_with_board().await;
    let id = seed(&server, "Sprint").await;
    let actor = backend.actor();
    let first = backend
        .services
        .tickets
        .create(
            id as i32,
            &NewTicketInput::titled("A"),
            &actor,
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket A");
    backend
        .services
        .tickets
        .create(
            id as i32,
            &NewTicketInput {
                depends_on: Some(vec![first.ticket.id.clone()]),
                ..NewTicketInput::titled("B")
            },
            &actor,
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket B");
    backend
        .services
        .comments
        .add(&first.ticket.id, "Hallo", &actor, false)
        .await
        .expect("Kommentar");

    let view = server.get(&format!("/api/db/boards/{id}/view")).await;
    assert_eq!(view.status, 200);
    let view = body(&view);
    assert_eq!(view["boardId"], id);
    let columns: Vec<&str> = view["columns"]
        .as_array()
        .expect("Spalten")
        .iter()
        .map(|column| column["id"].as_str().expect("Spalten-Kennung"))
        .collect();
    assert_eq!(
        columns,
        [
            "backlog",
            "waiting",
            "in_planning",
            "planned",
            "in_progress",
            "blocked",
            "done",
            "not_needed"
        ]
    );
    assert_eq!(view["columns"][0]["tickets"][0]["id"], "ARC-0001");
    assert_eq!(view["columns"][0]["tickets"][0]["commentCount"], 1);
    assert_eq!(view["columns"][1]["title"], "Wartet");
    assert_eq!(view["columns"][1]["tickets"][0]["id"], "ARC-0002");
    assert_eq!(
        view["columns"][1]["tickets"][0]["dependsOpen"][0],
        "ARC-0001"
    );
    assert_eq!(server.get("/api/db/boards/999/view").await.status, 404);
}

#[tokio::test]
async fn liefert_den_gewahlten_ordner_gibt_den_startordner_weiter_und_meldet_abbruch_als_null() {
    let picker = Arc::new(FakeFolderPicker::new(Some("C:/Repos/Mein Repo".into())));
    let server = TestServer::with(TestServerOptions {
        platform: Some(Platform {
            folders: Box::new(ArcPicker(picker.clone())),
            ..Platform::fake()
        }),
        ..TestServerOptions::default()
    });
    let chosen = server
        .post("/api/db/pick-folder", r#"{"start":" C:/Repos "}"#)
        .await;
    assert_eq!(body(&chosen), json!({ "path": "C:/Repos/Mein Repo" }));
    // Ein leerer Startordner kommt als „kein Startordner“ an
    server.post("/api/db/pick-folder", "{}").await;
    assert_eq!(
        picker.calls.lock().expect("Aufrufe").as_slice(),
        [Some("C:/Repos".to_string()), None]
    );
    assert_eq!(
        server
            .post("/api/db/pick-folder", r#"{"start":5}"#)
            .await
            .status,
        422
    );

    // Abbruch der Person
    let server = TestServer::with(TestServerOptions::default());
    assert_eq!(
        body(&server.post("/api/db/pick-folder", "{}").await),
        json!({ "path": Value::Null })
    );
}

#[tokio::test]
async fn meldet_fehlende_ordnerauswahl_mit_verstandlicher_meldung_kein_windows() {
    let server = TestServer::with(TestServerOptions {
        platform: Some(Platform::unavailable()),
        ..TestServerOptions::default()
    });
    let reply = server.post("/api/db/pick-folder", "{}").await;
    assert_eq!(reply.code().as_deref(), Some("folder-picker-unavailable"));
    assert_eq!(
        body(&reply)["message"],
        "Die Ordnerauswahl gibt es nur unter Windows; den Pfad bitte eintippen."
    );
}

#[tokio::test]
async fn antwortet_ohne_board_yaml_mit_503_und_hinweis() {
    let server = TestServer::with(TestServerOptions {
        board_from_config: true,
        ..TestServerOptions::default()
    });
    let reply = server.get("/api/db/projects").await;
    assert_eq!(reply.status, 503);
    assert_eq!(reply.code().as_deref(), Some("board-config-missing"));
    assert!(
        body(&reply)["message"]
            .as_str()
            .expect("Meldung")
            .contains("Board-Datenbank"),
        "{}",
        reply.text
    );
}

#[tokio::test]
async fn antwortet_bei_kaputter_board_yaml_mit_503_board_config_broken() {
    let server = TestServer::with(TestServerOptions {
        board_from_config: true,
        ..TestServerOptions::default()
    });
    let file = BoardConfigStore::new(server.home.clone()).file();
    std::fs::create_dir_all(file.parent().expect("Ordner")).expect("Ordner anlegen");
    std::fs::write(&file, "server: [kaputt").expect("board.yaml beschädigen");
    for url in ["/api/db/active", "/api/db/projects"] {
        let reply = server.get(url).await;
        assert_eq!(reply.status, 503, "{url}");
        assert_eq!(
            reply.code().as_deref(),
            Some("board-config-broken"),
            "{url}"
        );
    }
}

#[tokio::test]
async fn liefert_die_branches_des_board_repos() {
    let (server, backend, repo) = with_repo().await;
    let id = seed(&server, "Sprint").await;
    backend
        .services
        .boards
        .set_repo_path(id as i32, Some(&repo.path().to_string_lossy()))
        .await
        .expect("Repo-Pfad setzen");
    assert_eq!(
        body(&server.get(&format!("/api/db/boards/{id}/branches")).await),
        json!({ "branches": ["main", "release/1"], "reason": Value::Null })
    );
}

#[tokio::test]
async fn meldet_ohne_repo_pfad_und_bei_fehlendem_ordner_no_repo_statt_eines_fehlers() {
    let (server, backend, repo) = with_repo().await;
    let id = seed(&server, "Ohne").await;
    assert_eq!(
        body(&server.get(&format!("/api/db/boards/{id}/branches")).await),
        json!({ "branches": [], "reason": "no-repo" })
    );
    let missing = repo.path().parent().expect("Elternordner").join("fehlt");
    backend
        .services
        .boards
        .set_repo_path(id as i32, Some(&missing.to_string_lossy()))
        .await
        .expect("Repo-Pfad setzen");
    assert_eq!(
        body(&server.get(&format!("/api/db/boards/{id}/branches")).await),
        json!({ "branches": [], "reason": "no-repo" })
    );
}

#[tokio::test]
async fn legt_einen_neuen_branch_im_board_repo_an() {
    let (server, backend, repo) = with_repo().await;
    let id = seed(&server, "Sprint").await;
    backend
        .services
        .boards
        .set_repo_path(id as i32, Some(&repo.path().to_string_lossy()))
        .await
        .expect("Repo-Pfad setzen");

    let created = server
        .post(
            &format!("/api/db/boards/{id}/branches"),
            r#"{"name":"release/2"}"#,
        )
        .await;
    assert_eq!(created.status, 201);
    assert_eq!(
        body(&created),
        json!({ "name": "release/2", "created": true, "from": "main" })
    );
    assert_eq!(
        body(&server.get(&format!("/api/db/boards/{id}/branches")).await)["branches"],
        json!(["main", "release/1", "release/2"])
    );
    let again = server
        .post(
            &format!("/api/db/boards/{id}/branches"),
            r#"{"name":"release/2"}"#,
        )
        .await;
    assert_eq!(again.status, 200);
    assert_eq!(
        body(&again),
        json!({ "name": "release/2", "created": false, "from": Value::Null })
    );
}

#[tokio::test]
async fn lehnt_ungultige_namen_und_boards_ohne_repo_ab() {
    let (server, backend, repo) = with_repo().await;
    let id = seed(&server, "Ohne").await;
    assert_eq!(
        server
            .post(
                &format!("/api/db/boards/{id}/branches"),
                r#"{"name":"release/3"}"#
            )
            .await
            .code()
            .as_deref(),
        Some("board-repo-missing")
    );
    backend
        .services
        .boards
        .set_repo_path(id as i32, Some(&repo.path().to_string_lossy()))
        .await
        .expect("Repo-Pfad setzen");
    for name in ["a b", "ticket/x"] {
        assert_eq!(
            server
                .post(
                    &format!("/api/db/boards/{id}/branches"),
                    &json!({ "name": name }).to_string()
                )
                .await
                .code()
                .as_deref(),
            Some("target-branch-invalid"),
            "{name}"
        );
    }
    // Leerer Name: „Kein Branch-Name angegeben.“
    let empty = server
        .post(&format!("/api/db/boards/{id}/branches"), r#"{"name":"  "}"#)
        .await;
    assert_eq!(empty.code().as_deref(), Some("target-branch-invalid"));
    assert_eq!(body(&empty)["message"], "Kein Branch-Name angegeben.");
}

#[tokio::test]
async fn meldet_ein_unbekanntes_board_als_nicht_gefunden() {
    let (server, _backend, _repo) = with_repo().await;
    assert_eq!(
        server
            .post("/api/db/boards/999/branches", r#"{"name":"x"}"#)
            .await
            .status,
        404
    );
    assert_eq!(server.get("/api/db/boards/999/branches").await.status, 404);
    assert_eq!(
        server
            .get("/api/db/boards/abc/branches")
            .await
            .code()
            .as_deref(),
        Some("id-invalid")
    );
}

#[tokio::test]
async fn gibt_die_gruppe_am_notschalter_an_den_node_server_ab() {
    let (server, _backend) = TestServer::start_with_board_and(TestServerOptions {
        proxy_groups: RouteGroups::from_env(Some(RouteGroup::DbBoards.as_str())),
        ..TestServerOptions::default()
    })
    .await;
    // Ohne Node-Server beantwortet der Rückfall die Anfrage – entscheidend ist, dass keine
    // Rust-Route mehr greift
    let reply = server.get("/api/db/projects").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.code().as_deref(), Some("not-found"));
}

/// Die Zeilen dieser Gruppe aus der Sicherheitsmatrix (`packages/server/tests/security/
/// Matrix.test.ts`): der Schutz greift vor jeder Route, auch ohne `board.yaml`
#[tokio::test]
async fn lehnt_die_schreibrouten_der_gruppe_ohne_pflichtkopf_ab() {
    let (server, backend) = TestServer::start_with_board().await;
    let id = seed(&server, "Sprint").await;
    let routes = [
        (
            "POST",
            "/api/db/projects".to_string(),
            r#"{"key":"EVIL","name":"Eingeschleust"}"#,
        ),
        ("DELETE", format!("/api/db/boards/{id}"), "{}"),
        ("POST", "/api/db/pick-folder".to_string(), "{}"),
    ];
    for (method, url, payload) in &routes {
        // Ohne den Pflichtkopf `x-harness`
        let reply = server
            .request(method, url, Some(payload), &[("x-harness", None)])
            .await;
        assert_eq!(reply.status, 403, "{method} {url}");
        assert_eq!(
            reply.code().as_deref(),
            Some("header-missing"),
            "{method} {url}"
        );
    }
    // Keine Anfrage hat gewirkt: Projekt und Board stehen unverändert da
    let projects = backend.services.projects.list().await.expect("Projekte");
    assert_eq!(
        projects
            .iter()
            .map(|project| project.key.as_str())
            .collect::<Vec<_>>(),
        ["ARC"]
    );
    let boards = backend
        .services
        .boards
        .list(&BoardListFilter::default())
        .await
        .expect("Boards");
    assert!(boards.iter().any(|board| board.board.id == id as i32));
}

#[tokio::test]
#[ignore = "braucht den Ereignisstrom /api/live (Routengruppe events)"]
async fn meldet_eine_anderung_am_beobachteten_board_als_dbboard() {
    // Portiert aus packages/server/tests/live/dbboard.test.ts; der Strom entsteht mit der
    // Routengruppe `events`.
}

/// Testserver mit Board-Laufzeit und einem Git-Projekt mit den Branches `main` und
/// `release/1`
async fn with_repo() -> (TestServer, Arc<BoardBackendWithServices>, common::TempRepo) {
    let (server, backend) = TestServer::start_with_board().await;
    let repo = common::TempRepo::new();
    repo.run_git(&["branch", "-M", "main"]);
    repo.run_git(&["branch", "release/1"]);
    server
        .post("/api/db/projects", r#"{"key":"ARC","name":"Arches"}"#)
        .await;
    (server, backend, repo)
}

/// Reicht einen geteilten Ordnerdialog an den Zustand des Servers weiter (die Attrappe prüft
/// hinterher ihre Aufrufe)
struct ArcPicker(Arc<FakeFolderPicker>);

#[async_trait::async_trait]
impl nexus_platform::FolderPicker for ArcPicker {
    async fn pick(&self, start: Option<&str>) -> nexus_platform::Result<Option<String>> {
        self.0.pick(start).await
    }
}
