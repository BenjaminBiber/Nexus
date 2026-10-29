//! Die Routengruppe `questions`; portiert aus
//! `packages/server/tests/unit/DbQuestionRoutes.test.ts`, dazu die Zeilen dieser Gruppe aus
//! `packages/server/tests/security/Matrix.test.ts`.

mod common;

use std::sync::Arc;

use common::{BoardBackendWithServices, Reply, TestServer, TestServerOptions};
use nexus_core::actors::AuthorKind;
use nexus_core::board::basics::BoardActor;
use nexus_core::board::database::BoardDatabase as _;
use nexus_core::board::question_service::NewBoardQuestionInput;
use nexus_core::board::question_types::BoardQuestionActor;
use nexus_core::board::ticket_service::{NewTicketInput, WriteOptions};
use nexus_core::board::types::BoardUser;
use nexus_core::questions::{ResearchInput, ResearchOptionInput};
use nexus_core::tx;
use nexus_server::groups::{RouteGroup, RouteGroups};
use serde_json::{json, Value};

/// Server mit Projekt ARC, Board „Sprint“ und dem Ticket ARC-0001
async fn start() -> (TestServer, Arc<BoardBackendWithServices>) {
    let (server, backend) = TestServer::start_with_board().await;
    backend
        .services
        .projects
        .create("ARC", "Arches", &backend.actor())
        .await
        .expect("Projekt anlegen");
    let board = backend
        .services
        .boards
        .create("ARC", "Sprint", &backend.actor())
        .await
        .expect("Board anlegen");
    backend
        .services
        .tickets
        .create(
            board.board.id,
            &NewTicketInput::titled("Login"),
            &backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("Ticket anlegen");
    (server, backend)
}

/// Der Körper einer Antwort als JSON
fn body(reply: &Reply) -> Value {
    reply
        .json()
        .unwrap_or_else(|| panic!("kein JSON: {}", reply.text))
}

/// Der Fehlercode einer Antwort
fn code(reply: &Reply) -> String {
    reply
        .code()
        .unwrap_or_else(|| panic!("kein Fehlercode: {}", reply.text))
}

/// Eine Frage über die Route stellen
async fn ask(server: &TestServer, payload: Value) -> Reply {
    server
        .post("/api/db/projects/ARC/questions", &payload.to_string())
        .await
}

/// Ein Agent derselben Sitzung (das `AGENT` der TypeScript-Tests)
fn agent() -> BoardQuestionActor {
    BoardQuestionActor {
        actor: BoardActor {
            user_id: common::MEMORY_USER_ID.into(),
            name: "claude".into(),
        },
        kind: AuthorKind::Session,
        session: Some("s-1".into()),
    }
}

#[tokio::test]
async fn stellt_eine_frage_als_mensch_und_listet_die_fragen_des_projekts_mit_zahlern() {
    let (server, _backend) = start().await;
    let created = ask(
        &server,
        json!({ "title": "Welcher Speicher?", "ticketIds": ["ARC-0001"], "tags": ["speicher"], "occasion": "Board" }),
    )
    .await;
    assert_eq!(created.status, 201);
    let question = body(&created);
    assert_eq!(question["id"], "ARC-F0001");
    assert_eq!(question["originKind"], "human");
    assert_eq!(question["originName"], common::MEMORY_USER_NAME);
    assert_eq!(question["originSession"], Value::Null);
    assert_eq!(question["ticketIds"], json!(["ARC-0001"]));
    assert_eq!(question["tickets"][0]["id"], "ARC-0001");
    assert_eq!(question["tickets"][0]["title"], "Login");
    assert_eq!(question["tags"], json!(["speicher"]));
    assert_eq!(question["occasion"], "Board");
    assert_eq!(question["rev"], 1);

    // Der Projektschlüssel darf klein geschrieben sein
    let list = body(&server.get("/api/db/projects/arc/questions").await);
    assert_eq!(
        list["counts"],
        json!({ "open": 1, "answered": 0, "discarded": 0, "mine": 1 })
    );
    assert_eq!(list["questions"][0]["id"], "ARC-F0001");
    assert_eq!(list["questions"][0]["ready"], true);
    assert_eq!(list["questions"][0]["mine"], true);

    assert_eq!(
        body(
            &server
                .get("/api/db/projects/ARC/questions?state=answered")
                .await
        )["questions"],
        json!([])
    );
    assert_eq!(
        body(
            &server
                .get("/api/db/projects/ARC/questions?ticket=ARC-0001&q=speicher")
                .await
        )["questions"]
            .as_array()
            .expect("Fragen")
            .len(),
        1
    );
    assert_eq!(
        code(
            &server
                .get("/api/db/projects/ARC/questions?state=quatsch")
                .await
        ),
        "input-invalid"
    );

    assert_eq!(
        server.get("/api/db/projects/XXX/questions").await.status,
        404
    );
    assert_eq!(
        code(&ask(&server, json!({ "title": " " })).await),
        "title-empty"
    );
    assert_eq!(code(&ask(&server, json!({})).await), "input-invalid");

    let detail = body(&server.get("/api/db/questions/ARC-F0001").await);
    assert_eq!(detail["id"], "ARC-F0001");
    assert_eq!(detail["research"], json!([]));
    assert_eq!(detail["comments"], json!([]));
    assert_eq!(detail["history"][0]["type"], "created");
    assert_eq!(detail["history"][0]["byKind"], "human");
    assert_eq!(server.get("/api/db/questions/ARC-F0009").await.status, 404);

    assert_eq!(
        body(&server.get("/api/db/tickets/ARC-0001").await)["waitingOnQuestions"],
        json!([{ "id": "ARC-F0001", "title": "Welcher Speicher?" }])
    );
}

#[tokio::test]
async fn lehnt_einen_mehrfach_angegebenen_parameter_der_liste_mit_422_ab() {
    let (server, _backend) = start().await;
    for bad in [
        "ticket=A&ticket=B",
        "tag=a&tag=b",
        "q=a&q=b",
        "state=open&state=all",
    ] {
        let reply = server
            .get(&format!("/api/db/projects/ARC/questions?{bad}"))
            .await;
        assert_eq!(reply.status, 422, "{bad}");
        assert_eq!(code(&reply), "input-invalid", "{bad}");
    }
    assert_eq!(
        body(
            &server
                .get("/api/db/projects/ARC/questions?state=open&state=all")
                .await
        )["message"],
        "Parameter „state“ muss genau einmal als Text angegeben werden."
    );
    // Ein leerer Text gilt als nicht gesetzt: `state=` heißt „alle“
    assert_eq!(
        server
            .get("/api/db/projects/ARC/questions?state=&ticket=&tag=&q=")
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn kennzeichnet_eigene_fragen_und_zahlt_offene_eigene_in_counts_mine() {
    let (server, backend) = start().await;
    let other = BoardQuestionActor {
        actor: BoardActor {
            user_id: "22222222-2222-2222-2222-222222222222".into(),
            name: "Kollege".into(),
        },
        kind: AuthorKind::Human,
        session: None,
    };
    let user = BoardUser {
        id: other.actor.user_id.clone(),
        name: other.actor.name.clone(),
    };
    let db = backend.services.db.clone();
    tx!(db, [user], |u| { u.users().upsert(&user).await })
        .await
        .expect("zweiten Benutzer anlegen");

    let questions = &backend.services.questions;
    questions
        .create(
            "ARC",
            &NewBoardQuestionInput {
                title: "Zu meinem Ticket".into(),
                ticket_ids: Some(vec!["ARC-0001".into()]),
                ..NewBoardQuestionInput::default()
            },
            &other,
        )
        .await
        .expect("fremde Frage zu eigenem Ticket");
    questions
        .create(
            "ARC",
            &NewBoardQuestionInput::titled("Selbst gestellt"),
            &agent(),
        )
        .await
        .expect("eigene Frage");
    questions
        .create("ARC", &NewBoardQuestionInput::titled("Fremd"), &other)
        .await
        .expect("fremde Frage");

    let list = body(&server.get("/api/db/projects/ARC/questions").await);
    assert_eq!(
        list["counts"],
        json!({ "open": 3, "answered": 0, "discarded": 0, "mine": 2 })
    );
    let mine: Vec<(String, bool)> = list["questions"]
        .as_array()
        .expect("Fragen")
        .iter()
        .map(|question| {
            (
                question["id"].as_str().expect("ID").to_string(),
                question["mine"].as_bool().expect("mine"),
            )
        })
        .collect();
    assert_eq!(
        mine,
        [
            ("ARC-F0001".to_string(), true),
            ("ARC-F0002".to_string(), true),
            ("ARC-F0003".to_string(), false),
        ]
    );
}

#[tokio::test]
async fn andert_verknupft_und_kommentiert_mit_version_und_meldet_konflikte() {
    let (server, _backend) = start().await;
    ask(&server, json!({ "title": "A" })).await;

    let changed = server
        .patch(
            "/api/db/questions/ARC-F0001",
            &json!({ "rev": 1, "title": "B", "occasion": Value::Null }).to_string(),
        )
        .await;
    assert_eq!(changed.status, 200);
    assert_eq!(body(&changed)["title"], "B");
    assert_eq!(body(&changed)["rev"], 2);

    let stale = server
        .patch(
            "/api/db/questions/ARC-F0001",
            &json!({ "rev": 1, "title": "Alt" }).to_string(),
        )
        .await;
    assert_eq!(stale.status, 409);
    let json = body(&stale);
    assert_eq!(json["code"], "conflict");
    assert_eq!(json["current"]["id"], "ARC-F0001");
    assert_eq!(json["current"]["title"], "B");
    assert_eq!(json["current"]["rev"], 2);
    assert_eq!(
        code(
            &server
                .patch(
                    "/api/db/questions/ARC-F0001",
                    &json!({ "title": "ohne" }).to_string()
                )
                .await
        ),
        "rev-missing"
    );

    let tickets = server
        .put(
            "/api/db/questions/ARC-F0001/tickets",
            &json!({ "rev": 2, "ticketIds": ["ARC-0001"] }).to_string(),
        )
        .await;
    assert_eq!(body(&tickets)["ticketIds"], json!(["ARC-0001"]));
    assert_eq!(body(&tickets)["rev"], 3);
    let missing = server
        .put(
            "/api/db/questions/ARC-F0001/tickets",
            &json!({ "rev": 3 }).to_string(),
        )
        .await;
    assert_eq!(code(&missing), "input-invalid");
    assert_eq!(
        body(&missing)["message"],
        "Feld „ticketIds“ fehlt oder ist keine Liste von Texten."
    );
    assert_eq!(
        server
            .put(
                "/api/db/questions/ARC-F0001/tickets",
                &json!({ "rev": 1, "ticketIds": [] }).to_string()
            )
            .await
            .status,
        409
    );

    let depends = server
        .put(
            "/api/db/questions/ARC-F0001/depends",
            &json!({ "rev": 3, "dependsOn": ["ARC-0001"] }).to_string(),
        )
        .await;
    assert_eq!(body(&depends)["dependsOn"], json!(["ARC-0001"]));
    assert_eq!(body(&depends)["ready"], false);
    assert_eq!(body(&depends)["rev"], 4);
    assert_eq!(
        code(
            &server
                .put(
                    "/api/db/questions/ARC-F0001/depends",
                    &json!({ "rev": 4, "dependsOn": ["ARC-F0001"] }).to_string()
                )
                .await
        ),
        "dependency-self"
    );

    let commented = server
        .post(
            "/api/db/questions/ARC-F0001/comments",
            &json!({ "body": "Hallo" }).to_string(),
        )
        .await;
    assert_eq!(commented.status, 201);
    let comments = body(&commented)["comments"]
        .as_array()
        .cloned()
        .expect("Kommentare");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "Hallo");
    assert_eq!(comments[0]["authorKind"], "human");
    assert_eq!(comments[0]["authorName"], common::MEMORY_USER_NAME);
    assert_eq!(
        code(
            &server
                .post(
                    "/api/db/questions/ARC-F0001/comments",
                    &json!({ "body": " " }).to_string()
                )
                .await
        ),
        "comment-empty"
    );
}

#[tokio::test]
async fn entscheidet_verwirft_und_offnet_wieder_als_mensch() {
    let (server, backend) = start().await;
    ask(&server, json!({ "title": "A", "ticketIds": ["ARC-0001"] })).await;
    backend
        .services
        .questions
        .add_research(
            "ARC-F0001",
            &ResearchInput::options(vec![ResearchOptionInput {
                title: Some("Datei".into()),
                text: Some("JSON".into()),
                ..ResearchOptionInput::default()
            }]),
            &agent(),
        )
        .await
        .expect("Recherche anlegen");

    let stale = server
        .post(
            "/api/db/questions/ARC-F0001/decision",
            &json!({ "rev": 1, "option": "O1", "reason": "x" }).to_string(),
        )
        .await;
    assert_eq!(stale.status, 409);
    assert_eq!(body(&stale)["current"]["rev"], 2);
    assert_eq!(
        code(
            &server
                .post(
                    "/api/db/questions/ARC-F0001/decision",
                    &json!({ "rev": 2, "option": "O9" }).to_string()
                )
                .await
        ),
        "decision-option-unknown"
    );

    let answered = server
        .post(
            "/api/db/questions/ARC-F0001/decision",
            &json!({ "rev": 2, "option": "O1", "reason": "reicht" }).to_string(),
        )
        .await;
    let json = body(&answered);
    assert_eq!(json["state"], "answered");
    assert_eq!(json["rev"], 3);
    assert_eq!(json["decision"]["option"], "O1");
    assert_eq!(json["decision"]["byName"], common::MEMORY_USER_NAME);
    assert_eq!(
        body(&server.get("/api/db/tickets/ARC-0001").await)["waitingOnQuestions"],
        json!([])
    );

    let reopened = server
        .post(
            "/api/db/questions/ARC-F0001/reopen",
            &json!({ "rev": 3, "reason": "neu" }).to_string(),
        )
        .await;
    assert_eq!(body(&reopened)["state"], "open");
    assert_eq!(body(&reopened)["rev"], 4);

    assert_eq!(
        code(
            &server
                .post(
                    "/api/db/questions/ARC-F0001/discard",
                    &json!({ "rev": 4 }).to_string()
                )
                .await
        ),
        "decision-reason-missing"
    );
    let discarded = server
        .post(
            "/api/db/questions/ARC-F0001/discard",
            &json!({ "rev": 4, "reason": "doppelt" }).to_string(),
        )
        .await;
    assert_eq!(body(&discarded)["state"], "discarded");
    assert_eq!(body(&discarded)["rev"], 5);
}

/// Die Zeilen dieser Gruppe aus der Sicherheitsmatrix (`packages/server/tests/security/
/// Matrix.test.ts`): der Schutz greift vor jeder Route
#[tokio::test]
async fn lehnt_die_schreibrouten_der_gruppe_ohne_pflichtkopf_ab() {
    let (server, backend) = start().await;
    ask(&server, json!({ "title": "A" })).await;
    let routes = [
        (
            "POST",
            "/api/db/projects/ARC/questions",
            json!({ "title": "Eingeschleust" }),
        ),
        (
            "PATCH",
            "/api/db/questions/ARC-F0001",
            json!({ "rev": 1, "title": "Überschrieben" }),
        ),
        (
            "PUT",
            "/api/db/questions/ARC-F0001/tickets",
            json!({ "rev": 1, "ticketIds": ["ARC-0001"] }),
        ),
        (
            "PUT",
            "/api/db/questions/ARC-F0001/depends",
            json!({ "rev": 1, "dependsOn": ["ARC-0001"] }),
        ),
        (
            "POST",
            "/api/db/questions/ARC-F0001/comments",
            json!({ "body": "Eingeschleust" }),
        ),
        (
            "POST",
            "/api/db/questions/ARC-F0001/decision",
            json!({ "rev": 1, "text": "Eingeschleust" }),
        ),
        (
            "POST",
            "/api/db/questions/ARC-F0001/discard",
            json!({ "rev": 1, "reason": "Eingeschleust" }),
        ),
        (
            "POST",
            "/api/db/questions/ARC-F0001/reopen",
            json!({ "rev": 1 }),
        ),
    ];
    for (method, url, payload) in &routes {
        // Ohne den Pflichtkopf `x-harness`
        let reply = server
            .request(
                method,
                url,
                Some(&payload.to_string()),
                &[("x-harness", None)],
            )
            .await;
        assert_eq!(reply.status, 403, "{method} {url}");
        assert_eq!(
            reply.code().as_deref(),
            Some("header-missing"),
            "{method} {url}"
        );
    }
    // Keine Anfrage hat gewirkt
    let question = backend
        .services
        .questions
        .get("ARC-F0001")
        .await
        .expect("Frage");
    assert_eq!(question.view.question.title, "A");
    assert_eq!(question.view.question.rev, 1);
    assert!(question.comments.is_empty());
}

/// Der Notschalter: steht `questions` in `NEXUS_PROXY_GROUPS`, meldet Rust die Routen nicht an
/// und der Rückfall übernimmt
#[tokio::test]
async fn lasst_die_gruppe_mit_dem_notschalter_zum_node_server_gehen() {
    let (server, _backend) = TestServer::start_with_board_and(TestServerOptions {
        proxy_groups: RouteGroups::from_env(Some(RouteGroup::Questions.as_str())),
        ..TestServerOptions::default()
    })
    .await;
    // Ohne Node-Server beantwortet der Rückfall die Anfrage – entscheidend ist, dass keine
    // Rust-Route mehr greift
    let reply = server.get("/api/db/questions/ARC-F0001").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.code().as_deref(), Some("not-found"));
}
