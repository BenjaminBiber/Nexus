//! Tickets aus Dokumenten anlegen und lesen; portiert aus
//! `packages/server/tests/unit/DocTickets.test.ts`.
//!
//! Die Einzelheiten eines Tickets holt der Test direkt von den Board-Diensten: die Routen
//! `/api/db/tickets/{id}` und `/api/db/boards/{id}/tickets` gehören zur Gruppe `db-tickets`
//! und laufen noch über den Node-Server. Der Spec-Fortschritt kommt über die Projektroute
//! `…/spec-progress`, die diese Gruppe mitbringt.

mod common;

use common::{encode, DocsFixture};
use nexus_core::board::ticket_service::{NewTicketInput, TicketDetail, WriteOptions};
use nexus_server::board_runtime::{factory, BoardBackendFuture, BoardRuntimeHolder};
use nexus_server::docs::{DocTicketRequest, DocTicketService, DocsRootResolver, DocsService};
use serde_json::{json, Value};

const SPEC: &str = "docs/superpowers/specs/2026-10-22-docs-rework-design.md";

/// Ein vollständiger Entwurf, wie ihn `docs/ticket-drafts` liefert
fn draft(title: &str) -> Value {
    json!({
        "title": title,
        "description": format!("Beschreibung {title}"),
        "acceptance": format!("Abnahme {title}"),
        "steps": ["Test schreiben", "Umsetzen"],
        "dependsOn": [],
        "labels": ["server"],
        "source": { "heading": "Ziel", "startLine": 3, "endLine": 5 },
    })
}

/// Entwurf mit abweichenden Feldern
fn draft_with(title: &str, extra: Value) -> Value {
    let mut value = draft(title);
    for (key, item) in extra.as_object().expect("Felder") {
        value[key] = item.clone();
    }
    value
}

/// Server, Projekt, Board und die Spec der Ticket-Tests
async fn start(with_repo_path: bool) -> DocsFixture {
    let fixture = DocsFixture::start(with_repo_path).await;
    fixture.write(
        "docs/superpowers/specs",
        "2026-10-22-docs-rework-design.md",
        "# Docs-Rework\n\n## Ziel\n\nEine Quelle für Docs.\n\n## Plan\n\nSchritt eins.\n",
    );
    fixture
}

/// Legt die Entwürfe über die Route an
async fn create(fixture: &DocsFixture, drafts: Value, path: &str) -> common::Reply {
    fixture
        .server
        .post(
            &format!("{}/tickets", fixture.base()),
            &json!({ "path": path, "drafts": drafts }).to_string(),
        )
        .await
}

/// Die Einzelheiten eines Tickets aus den Board-Diensten
async fn ticket(fixture: &DocsFixture, id: &str) -> TicketDetail {
    fixture
        .backend
        .services
        .tickets
        .get(id)
        .await
        .expect("Ticket")
}

/// Die Nummern der angelegten Tickets
fn created_ids(res: &common::Reply) -> Vec<String> {
    res.json().expect("JSON")["created"]
        .as_array()
        .expect("angelegt")
        .iter()
        .map(|item| item["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// Die Eingabeplätze der angelegten Entwürfe
fn created_indexes(res: &common::Reply) -> Vec<i64> {
    res.json().expect("JSON")["created"]
        .as_array()
        .expect("angelegt")
        .iter()
        .map(|item| item["index"].as_i64().unwrap_or_default())
        .collect()
}

#[tokio::test]
async fn legt_die_entwurfe_in_eingabereihenfolge_an_und_ubersetzt_dependson_in_ids() {
    let fixture = start(true).await;

    let res = create(
        &fixture,
        json!([
            draft("A"),
            draft_with("B", json!({ "dependsOn": [0] })),
            draft_with("C", json!({ "dependsOn": [0, 1] })),
        ]),
        SPEC,
    )
    .await;

    assert_eq!(res.status, 200);
    assert_eq!(res.json().expect("JSON")["skipped"], json!([]));
    assert_eq!(created_indexes(&res), vec![0, 1, 2]);
    let ids = created_ids(&res);
    assert_eq!(ids, vec!["ARC-0001", "ARC-0002", "ARC-0003"]);
    assert_eq!(
        ticket(&fixture, &ids[1]).await.depends_on,
        vec![ids[0].clone()]
    );
    assert_eq!(
        ticket(&fixture, &ids[2]).await.depends_on,
        vec![ids[0].clone(), ids[1].clone()]
    );
}

#[tokio::test]
async fn setzt_status_planungsbedarf_labels_schritte_abnahme_und_spec() {
    let fixture = start(true).await;

    let ids = created_ids(&create(&fixture, json!([draft("A")]), SPEC).await);
    let detail = ticket(&fixture, &ids[0]).await;

    assert_eq!(
        serde_json::to_value(detail.ticket.status).expect("Status"),
        "backlog"
    );
    assert!(detail.ticket.needs_planning);
    assert_eq!(detail.ticket.labels, vec!["server".to_string()]);
    assert_eq!(detail.ticket.acceptance, "Abnahme A");
    assert_eq!(detail.ticket.spec.as_deref(), Some(SPEC));
    let steps: Vec<&str> = detail.steps.iter().map(|step| step.text.as_str()).collect();
    assert_eq!(steps, vec!["Test schreiben", "Umsetzen"]);
}

#[tokio::test]
async fn hangt_die_quelle_an_die_beschreibung() {
    let fixture = start(true).await;

    let ids = created_ids(&create(&fixture, json!([draft("A")]), SPEC).await);

    assert_eq!(
        ticket(&fixture, &ids[0]).await.ticket.description,
        format!("Beschreibung A\n\nQuelle: {SPEC} › Ziel (Z. 3–5)")
    );
}

#[tokio::test]
async fn uberspringt_eine_dublette_und_lost_abhangigkeiten_auf_das_vorhandene_ticket_auf() {
    let fixture = start(true).await;
    let existing = fixture
        .backend
        .services
        .tickets
        .create(
            fixture.board_id,
            &NewTicketInput::titled("B"),
            &fixture.backend.actor(),
            &WriteOptions::default(),
        )
        .await
        .expect("vorhandenes Ticket")
        .ticket
        .id;

    let res = create(
        &fixture,
        json!([
            draft("A"),
            draft("B"),
            draft_with("C", json!({ "dependsOn": [1] })),
        ]),
        SPEC,
    )
    .await;

    assert_eq!(res.status, 200);
    assert_eq!(created_indexes(&res), vec![0, 2]);
    assert_eq!(
        res.json().expect("JSON")["skipped"],
        json!([{ "index": 1, "reason": "ticket-duplicate", "existingId": existing }])
    );
    let ids = created_ids(&res);
    assert_eq!(ticket(&fixture, &ids[1]).await.depends_on, vec![existing]);
}

#[tokio::test]
async fn uberspringt_einen_kaputten_entwurf_ohne_den_rest_zu_blockieren() {
    let fixture = start(true).await;

    let res = create(
        &fixture,
        json!([
            draft("A"),
            draft_with("", json!({ "title": "" })),
            draft("C")
        ]),
        SPEC,
    )
    .await;

    assert_eq!(created_indexes(&res), vec![0, 2]);
    let skipped = res.json().expect("JSON")["skipped"].clone();
    assert_eq!(skipped.as_array().expect("übersprungen").len(), 1);
    assert_eq!(skipped[0]["index"], 1);
    assert_eq!(skipped[0].get("existingId"), None);
}

#[tokio::test]
async fn pruft_jeden_entwurf_auf_dem_server_und_uberspringt_unvollstandige() {
    let fixture = start(true).await;
    let mut no_source = draft("B");
    no_source.as_object_mut().expect("Felder").remove("source");

    let res = create(
        &fixture,
        json!([
            draft("A"),
            draft_with("X", json!({ "acceptance": "" })),
            no_source,
            "kein Objekt",
            draft_with(
                "E",
                json!({ "source": { "heading": "Ziel", "startLine": 50, "endLine": 60 } })
            ),
        ]),
        SPEC,
    )
    .await;

    assert_eq!(res.status, 200);
    assert_eq!(
        res.json().expect("JSON")["skipped"],
        json!([
            { "index": 1, "reason": "draft-invalid" },
            { "index": 3, "reason": "draft-invalid" },
        ])
    );
    assert_eq!(created_indexes(&res), vec![0, 2, 4]);
    // ohne oder mit unbrauchbarem Zeilenbereich zeigt die Quelle auf die erste Zeile
    let ids = created_ids(&res);
    for id in &ids[1..] {
        assert!(ticket(&fixture, id)
            .await
            .ticket
            .description
            .ends_with("(Z. 1–1)"));
    }
}

#[tokio::test]
async fn verwirft_abhangigkeiten_auf_unbekannte_entwurfe() {
    let fixture = start(true).await;

    let res = create(
        &fixture,
        json!([
            draft_with("A", json!({ "dependsOn": [2] })),
            draft_with("B", json!({ "dependsOn": [0] })),
        ]),
        SPEC,
    )
    .await;

    let ids = created_ids(&res);
    assert_eq!(ids.len(), 2);
    assert!(ticket(&fixture, &ids[0]).await.depends_on.is_empty());
    assert_eq!(
        ticket(&fixture, &ids[1]).await.depends_on,
        vec![ids[0].clone()]
    );
}

#[tokio::test]
async fn liefert_den_dokumentpfad_als_gruppe_in_spec_progress() {
    let fixture = start(true).await;
    create(&fixture, json!([draft("A"), draft("B")]), SPEC).await;

    let progress = fixture
        .server
        .get(&format!("/api/projects/{}/spec-progress", fixture.pid))
        .await;

    assert_eq!(progress.status, 200);
    let group = progress
        .json()
        .expect("JSON")
        .as_array()
        .expect("Gruppen")
        .iter()
        .find(|item| item["spec"] == SPEC)
        .cloned()
        .expect("Gruppe des Dokuments");
    assert_eq!(group["total"], 2);
}

#[tokio::test]
async fn pruft_pfad_und_entwurfe() {
    let fixture = start(true).await;

    let forbidden = create(&fixture, json!([draft("A")]), "../geheim.md").await;
    assert_eq!(forbidden.code().as_deref(), Some("doc-not-found"));

    let empty = create(&fixture, json!([]), SPEC).await;
    assert_eq!(empty.code().as_deref(), Some("invalid-input"));
}

#[tokio::test]
async fn meldet_ein_board_ohne_repo_pfad_mit_einem_hinweis() {
    let fixture = start(false).await;

    let res = create(&fixture, json!([draft("A")]), SPEC).await;

    assert!(res.status.as_u16() >= 400);
    let message = res.json().expect("JSON")["message"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        message.contains("Repo") || message.contains("Board"),
        "{message}"
    );
}

/// Die Tickets eines Dokuments
async fn list(fixture: &DocsFixture, path: &str) -> common::Reply {
    fixture
        .server
        .get(&format!("{}/tickets?path={}", fixture.base(), encode(path)))
        .await
}

#[tokio::test]
async fn nennt_uberschrift_und_zeilen_aus_der_quellenzeile_sortiert_nach_anfangszeile() {
    let fixture = start(true).await;
    create(
        &fixture,
        json!([
            draft_with(
                "Spät",
                json!({ "source": { "heading": "Plan", "startLine": 7, "endLine": 9 } })
            ),
            draft_with(
                "Früh",
                json!({ "source": { "heading": "Ziel", "startLine": 3, "endLine": 5 } })
            ),
        ]),
        SPEC,
    )
    .await;

    let res = list(&fixture, SPEC).await;

    assert_eq!(res.status, 200);
    let tickets = res.json().expect("JSON");
    let titles: Vec<&str> = tickets
        .as_array()
        .expect("Tickets")
        .iter()
        .filter_map(|item| item["title"].as_str())
        .collect();
    assert_eq!(titles, vec!["Früh", "Spät"]);
    assert_eq!(tickets[0]["heading"], "Ziel");
    assert_eq!(tickets[0]["startLine"], 3);
    assert_eq!(tickets[0]["endLine"], 5);
    assert_eq!(tickets[0]["status"], "backlog");
    assert_eq!(tickets[0]["column"], "backlog");
    assert_eq!(tickets[1]["heading"], "Plan");
    assert_eq!(tickets[1]["startLine"], 7);
    assert_eq!(tickets[1]["endLine"], 9);
}

#[tokio::test]
async fn behalt_ein_ticket_ohne_quellenzeile_mit_heading_null() {
    let fixture = start(true).await;
    let ids = created_ids(&create(&fixture, json!([draft("A")]), SPEC).await);
    // Die Quellenzeile von Hand aus der Beschreibung entfernt: das Ticket gehört weiter zum
    // Dokument
    let detail = ticket(&fixture, &ids[0]).await;
    fixture
        .backend
        .services
        .tickets
        .update(
            &ids[0],
            &nexus_core::board::ticket_service::TicketPatch {
                description: Some("Ohne Quelle".into()),
                ..Default::default()
            },
            &fixture.backend.actor(),
            &WriteOptions {
                rev: Some(detail.ticket.rev),
                ..WriteOptions::default()
            },
        )
        .await
        .expect("Beschreibung ändern");

    let res = list(&fixture, SPEC).await;

    let tickets = res.json().expect("JSON");
    assert_eq!(tickets.as_array().expect("Tickets").len(), 1);
    assert_eq!(tickets[0]["id"], ids[0]);
    assert_eq!(tickets[0]["heading"], Value::Null);
    assert_eq!(tickets[0]["startLine"], 0);
    assert_eq!(tickets[0]["endLine"], 0);
}

#[tokio::test]
async fn lasst_tickets_anderer_dokumente_weg() {
    let fixture = start(true).await;
    create(&fixture, json!([draft("A")]), SPEC).await;
    let other = "docs/superpowers/specs/2026-10-22-anderes-design.md";
    fixture.write(
        "docs/superpowers/specs",
        "2026-10-22-anderes-design.md",
        "# Anderes\n\n## Ziel\n\nText.\n",
    );

    assert_eq!(list(&fixture, other).await.json(), Some(json!([])));
}

#[tokio::test]
async fn antwortet_ohne_repo_pfad_mit_einer_leeren_liste_statt_mit_einem_fehler() {
    let fixture = start(false).await;

    let res = list(&fixture, SPEC).await;

    assert_eq!(res.status, 200);
    assert_eq!(res.json(), Some(json!([])));
}

#[tokio::test]
async fn reicht_verbindungs_und_programmfehler_weiter_statt_sie_als_ubersprungen_zu_melden() {
    let fixture = start(true).await;
    let source = DocsRootResolver::new(
        Some(&fixture.server.state.board),
        Some(&fixture.server.home),
    )
    .resolve(&fixture.repo.path().to_string_lossy())
    .await
    .expect("Quelle");
    // Eine Laufzeit, die sich nicht aufbauen lässt: jeder Zugriff endet im Verbindungsfehler
    let broken = BoardRuntimeHolder::new(factory(|| -> BoardBackendFuture {
        Box::pin(async {
            Err(nexus_core::HarnessError::new("board-db-unreachable", "socket").into())
        })
    }));
    let request = DocTicketRequest {
        path: Some(SPEC.into()),
        drafts: vec![draft("A"), draft("B")],
    };

    let error = DocTicketService::new()
        .create(&broken, &DocsService::new(), &source, &request)
        .await
        .expect_err("Fehler");

    assert_eq!(
        error.harness().map(|harness| harness.code.as_str()),
        Some("board-db-unreachable")
    );
}
