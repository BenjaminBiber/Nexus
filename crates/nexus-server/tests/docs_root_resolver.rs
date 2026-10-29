//! Welches Board den Repo-Pfad der Dokumente liefert; portiert aus
//! `packages/server/tests/unit/DocsRootResolver.test.ts`.

mod common;

use common::{memory_config, DocsFixture, TestServer};
use nexus_core::board::config::BoardConfig;
use nexus_core::board::config_store::BoardConfigStore;
use nexus_server::docs::{DocsRootResolver, DocsSource, DocsSourceReason};
use serde_json::Value;

/// Der Auflöser über der Board-Laufzeit und dem Benutzerordner des Testservers
fn resolver(server: &TestServer) -> DocsRootResolver<'_> {
    DocsRootResolver::new(Some(&server.state.board), Some(&server.home))
}

/// Quelle mit Board und Repo-Pfad
fn source(root: Option<&str>, id: i32, name: &str, reason: Option<DocsSourceReason>) -> DocsSource {
    DocsSource {
        root: root.map(str::to_string),
        board_id: Some(id),
        board_name: Some(name.to_string()),
        reason,
    }
}

#[tokio::test]
async fn liefert_den_repo_pfad_des_boards_das_zum_ordner_gehort() {
    let fixture = DocsFixture::start(true).await;

    let found = resolver(&fixture.server)
        .resolve(&fixture.repo.path().to_string_lossy())
        .await
        .expect("Quelle");

    assert_eq!(
        found,
        source(Some(&fixture.root()), fixture.board_id, "Sprint", None)
    );
}

#[tokio::test]
async fn meldet_no_board_wenn_kein_board_dem_ordner_zugeordnet_ist() {
    let fixture = DocsFixture::start(false).await;

    let found = resolver(&fixture.server)
        .resolve(&fixture.repo.path().to_string_lossy())
        .await
        .expect("Quelle");

    assert_eq!(found, DocsSource::no_board());
}

#[tokio::test]
async fn meldet_no_repo_path_mit_dem_aktiven_board_wenn_diesem_der_repo_pfad_fehlt() {
    let fixture = DocsFixture::start(false).await;
    let config = BoardConfig {
        active_board: Some(fixture.board_id),
        ..memory_config()
    };
    BoardConfigStore::new(fixture.server.home.clone())
        .write(&config)
        .expect("board.yaml schreiben");

    let found = resolver(&fixture.server)
        .resolve(&fixture.repo.path().to_string_lossy())
        .await
        .expect("Quelle");

    assert_eq!(
        found,
        source(
            None,
            fixture.board_id,
            "Sprint",
            Some(DocsSourceReason::NoRepoPath)
        )
    );
}

#[tokio::test]
async fn meldet_missing_folder_wenn_der_repo_pfad_des_boards_nicht_mehr_existiert() {
    let fixture = DocsFixture::start(true).await;
    let path = fixture.repo.path().to_string_lossy().into_owned();
    fixture.repo.remove();

    let found = resolver(&fixture.server).resolve(&path).await.expect("Quelle");

    assert_eq!(found.root, None);
    assert_eq!(found.board_id, Some(fixture.board_id));
    assert_eq!(found.reason, Some(DocsSourceReason::MissingFolder));
}

#[tokio::test]
async fn liefert_mit_board_aus_der_query_das_genannte_board_auch_bei_fremdem_ordner() {
    let fixture = DocsFixture::start(false).await;
    let (other, _pid) = fixture.server.add_project();
    fixture.backend.set_repo_path(fixture.board_id, other.path());

    let found = resolver(&fixture.server)
        .resolve_for(
            &fixture.repo.path().to_string_lossy(),
            Some(&fixture.board_id.to_string()),
        )
        .await
        .expect("Quelle");

    assert_eq!(
        found,
        source(
            Some(&other.path().to_string_lossy().replace('\\', "/")),
            fixture.board_id,
            "Sprint",
            None
        )
    );
}

#[tokio::test]
async fn fallt_ohne_gultiges_board_auf_das_board_des_ordners_zuruck() {
    let fixture = DocsFixture::start(true).await;
    let root = fixture.repo.path().to_string_lossy().into_owned();
    let resolver = resolver(&fixture.server);

    for board in [None, Some("abc")] {
        let found = resolver.resolve_for(&root, board).await.expect("Quelle");
        assert_eq!(found.board_id, Some(fixture.board_id), "{board:?}");
    }
    let unknown = resolver
        .resolve_for(&root, Some("999"))
        .await
        .expect("Quelle");
    assert_eq!(unknown.root, None);
    assert_eq!(unknown.reason, Some(DocsSourceReason::NoBoard));
}

#[tokio::test]
async fn liest_uber_die_docs_route_mit_board_die_dokumente_aus_dem_repo_des_genannten_boards() {
    let fixture = DocsFixture::start(false).await;
    let (other, _other_pid) = fixture.server.add_project();
    let specs = other.path().join("docs").join("superpowers").join("specs");
    std::fs::create_dir_all(&specs).expect("Ordner");
    std::fs::write(
        specs.join("2026-10-01-fremd-design.md"),
        "# Fremd — Design-Spec\n",
    )
    .expect("Datei");
    fixture.backend.set_repo_path(fixture.board_id, other.path());

    let res = fixture
        .server
        .get(&format!("{}?board={}", fixture.base(), fixture.board_id))
        .await;

    let list = res.json().expect("JSON");
    assert_eq!(list["source"]["boardId"], fixture.board_id);
    assert_eq!(list["source"]["reason"], Value::Null);
    let paths: Vec<&str> = list["docs"]
        .as_array()
        .expect("Dokumente")
        .iter()
        .filter_map(|doc| doc["path"].as_str())
        .collect();
    assert!(paths.contains(&"docs/superpowers/specs/2026-10-01-fremd-design.md"));
    let file = fixture
        .server
        .get(&format!(
            "{}/file?path=docs/superpowers/specs/2026-10-01-fremd-design.md&board={}",
            fixture.base(),
            fixture.board_id
        ))
        .await;
    assert_eq!(file.status, 200);
}

#[tokio::test]
async fn meldet_no_board_ohne_board_laufzeit() {
    let server = TestServer::start();
    let (repo, _pid) = server.add_project();

    let found = DocsRootResolver::new(None, None)
        .resolve(&repo.path().to_string_lossy())
        .await
        .expect("Quelle");

    assert_eq!(found, DocsSource::no_board());
}

